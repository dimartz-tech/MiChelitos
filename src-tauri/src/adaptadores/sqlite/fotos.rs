//! Adaptador SQLite de las fotos del patrimonio y de la lectura que las alimenta.
//!
//! Como los demás, opera **sobre una transacción ya abierta**: quien la abre la confirma. Guardar la foto, reemplazar sus
//! partidas y sellar los meses anteriores ocurren, por tanto, en la misma transacción de escritura.

use crate::dominio::dinero::{Dinero, Divisa, TasaCambio, ESCALA_TASA_CAMBIO};
use crate::dominio::foto_patrimonio::Foto;
use crate::puertos::repositorios::*;
use rusqlite::Transaction;

pub struct FotosSqlite<'a> {
    tx: &'a Transaction<'a>,
}

impl<'a> FotosSqlite<'a> {
    pub fn nuevo(tx: &'a Transaction<'a>) -> Self {
        FotosSqlite { tx }
    }
}

fn fallo(e: rusqlite::Error) -> ErrorAlmacen {
    ErrorAlmacen::Fallo(e.to_string())
}

/// Un importe guardado como REAL exacto al céntimo (las tablas anteriores a la 18) a `Dinero`.
fn dinero_de_real(valor: f64, divisa: Divisa) -> Result<Dinero, ErrorAlmacen> {
    Dinero::nuevo(valor, divisa).map_err(|e| ErrorAlmacen::Fallo(e.to_string()))
}

fn tasa_de_micro(micro: i64) -> Result<TasaCambio, ErrorAlmacen> {
    TasaCambio::nueva(micro as f64 / ESCALA_TASA_CAMBIO as f64).map_err(|e| ErrorAlmacen::Fallo(e.to_string()))
}

impl RepositorioDeFotos for FotosSqlite<'_> {
    fn guardar_foto(&mut self, foto: &Foto) -> Result<i64, ErrorAlmacen> {
        // La cláusula `WHERE sellada = 0` es la que hace inmutable al mes cerrado: sin ella el upsert lo pisaría.
        let tocadas = self
            .tx
            .execute(
                "INSERT INTO patrimonio_fotos
                     (anio, mes, fecha, origen, sellada, tasa_usd_dop, activos_c, pasivos_c, patrimonio_c, liquido_c, calidad)
                 VALUES (?, ?, ?, ?, 0, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(anio, mes) DO UPDATE SET
                     fecha = excluded.fecha,
                     origen = excluded.origen,
                     tasa_usd_dop = excluded.tasa_usd_dop,
                     activos_c = excluded.activos_c,
                     pasivos_c = excluded.pasivos_c,
                     patrimonio_c = excluded.patrimonio_c,
                     liquido_c = excluded.liquido_c,
                     calidad = excluded.calidad
                 WHERE patrimonio_fotos.sellada = 0;",
                rusqlite::params![
                    foto.anio,
                    foto.mes,
                    foto.fecha,
                    foto.origen.codigo(),
                    foto.tasa_usd_dop.micro_unidades(),
                    foto.activos.centavos(),
                    foto.pasivos.centavos(),
                    foto.patrimonio.centavos(),
                    foto.liquido.centavos(),
                    foto.calidad.codigo(),
                ],
            )
            .map_err(fallo)?;
        if tocadas == 0 {
            return Err(ErrorAlmacen::FotoSellada { anio: foto.anio, mes: foto.mes });
        }
        let id: i64 = self
            .tx
            .query_row("SELECT id FROM patrimonio_fotos WHERE anio = ? AND mes = ?;", (foto.anio, foto.mes), |r| r.get(0))
            .map_err(fallo)?;
        self.tx.execute("DELETE FROM patrimonio_partidas WHERE foto_id = ?;", [id]).map_err(fallo)?;
        let mut insertar = self
            .tx
            .prepare(
                "INSERT INTO patrimonio_partidas (foto_id, rubro, referencia, divisa, importe_c, importe_dop_c)
                 VALUES (?, ?, ?, ?, ?, ?);",
            )
            .map_err(fallo)?;
        for p in &foto.partidas {
            insertar
                .execute(rusqlite::params![
                    id,
                    p.rubro.codigo(),
                    p.referencia,
                    p.importe.divisa().codigo(),
                    p.importe.centavos(),
                    p.importe_dop.centavos()
                ])
                .map_err(fallo)?;
        }
        Ok(id)
    }

    fn existe_foto(&self, anio: i32, mes: u32) -> Result<bool, ErrorAlmacen> {
        let n: i64 = self
            .tx
            .query_row("SELECT COUNT(*) FROM patrimonio_fotos WHERE anio = ? AND mes = ?;", (anio, mes), |r| r.get(0))
            .map_err(fallo)?;
        Ok(n > 0)
    }

    fn sellar_anteriores(&mut self, anio: i32, mes: u32) -> Result<usize, ErrorAlmacen> {
        self.tx
            .execute(
                "UPDATE patrimonio_fotos SET sellada = 1 WHERE sellada = 0 AND (anio * 12 + mes) < (? * 12 + ?);",
                (anio, mes),
            )
            .map_err(fallo)
    }

    fn fotos(&self) -> Result<Vec<FotoLeida>, ErrorAlmacen> {
        let mut stmt = self
            .tx
            .prepare(
                "SELECT id, anio, mes, fecha, origen, sellada, tasa_usd_dop, activos_c, pasivos_c, patrimonio_c, liquido_c, calidad
                 FROM patrimonio_fotos ORDER BY anio ASC, mes ASC;",
            )
            .map_err(fallo)?;
        let filas = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i32>(1)?,
                    r.get::<_, u32>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, i64>(6)?,
                    [r.get::<_, i64>(7)?, r.get::<_, i64>(8)?, r.get::<_, i64>(9)?, r.get::<_, i64>(10)?],
                    r.get::<_, String>(11)?,
                ))
            })
            .map_err(fallo)?;
        let mut fotos = Vec::new();
        for fila in filas {
            let (id, anio, mes, fecha, origen, sellada, tasa, c, calidad) = fila.map_err(fallo)?;
            let dop = |centavos: i64| Dinero::de_centavos(centavos, Divisa::Dop);
            fotos.push(FotoLeida {
                id,
                anio,
                mes,
                fecha,
                origen,
                sellada: sellada == 1,
                tasa_usd_dop: tasa_de_micro(tasa)?,
                activos: dop(c[0]),
                pasivos: dop(c[1]),
                patrimonio: dop(c[2]),
                liquido: dop(c[3]),
                calidad,
            });
        }
        Ok(fotos)
    }

    fn partidas_de(&self, foto_id: i64) -> Result<Vec<PartidaLeida>, ErrorAlmacen> {
        let mut stmt = self
            .tx
            .prepare(
                "SELECT rubro, referencia, divisa, importe_c, importe_dop_c
                 FROM patrimonio_partidas WHERE foto_id = ? ORDER BY id ASC;",
            )
            .map_err(fallo)?;
        let filas = stmt
            .query_map([foto_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?))
            })
            .map_err(fallo)?;
        let mut partidas = Vec::new();
        for fila in filas {
            let (rubro, referencia, divisa, importe, importe_dop) = fila.map_err(fallo)?;
            let divisa = Divisa::desde_codigo(&divisa).map_err(|e| ErrorAlmacen::Fallo(e.to_string()))?;
            partidas.push(PartidaLeida {
                rubro,
                referencia,
                divisa,
                importe: Dinero::de_centavos(importe, divisa),
                importe_dop: Dinero::de_centavos(importe_dop, Divisa::Dop),
            });
        }
        Ok(partidas)
    }
}

impl ConsultaDePatrimonio for FotosSqlite<'_> {
    fn situacion_para_foto(&self) -> Result<SituacionParaFoto, ErrorAlmacen> {
        let mut situacion = SituacionParaFoto::default();

        let mut cuentas = self
            .tx
            .prepare("SELECT nombre, divisa, balance_actual, es_caja_efectivo, archivada FROM cuentas_ahorro ORDER BY id;")
            .map_err(fallo)?;
        let filas = cuentas
            .query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, f64>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?))
            })
            .map_err(fallo)?;
        for fila in filas {
            let (nombre, divisa, saldo, caja, archivada) = fila.map_err(fallo)?;
            let divisa = Divisa::desde_codigo(&divisa).map_err(|e| ErrorAlmacen::Fallo(e.to_string()))?;
            situacion.cuentas.push(CuentaParaFoto {
                nombre,
                saldo: dinero_de_real(saldo, divisa)?,
                es_caja_efectivo: caja == 1,
                archivada: archivada == 1,
            });
        }

        let mut tarjetas = self
            .tx
            .prepare("SELECT entidad, nombre_tarjeta, activa, balance_pesos, balance_dolares FROM tarjetas ORDER BY id;")
            .map_err(fallo)?;
        let filas = tarjetas
            .query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, f64>(3)?, r.get::<_, f64>(4)?))
            })
            .map_err(fallo)?;
        for fila in filas {
            let (entidad, nombre, activa, pesos, dolares) = fila.map_err(fallo)?;
            situacion.tarjetas.push(TarjetaParaFoto {
                nombre: format!("{entidad} {nombre}"),
                activa: activa == 1,
                deuda_pesos: dinero_de_real(pesos, Divisa::Dop)?,
                deuda_dolares: dinero_de_real(dolares, Divisa::Usd)?,
            });
        }

        let mut prestamos = self
            .tx
            .prepare(
                "SELECT institucion_financiera, tipo_prestamo, COALESCE(saldo_actual, monto_prestamo) FROM prestamos ORDER BY id;",
            )
            .map_err(fallo)?;
        let filas = prestamos
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, f64>(2)?)))
            .map_err(fallo)?;
        for fila in filas {
            let (institucion, tipo, saldo) = fila.map_err(fallo)?;
            situacion.prestamos.push(PrestamoParaFoto {
                nombre: format!("{institucion} ({tipo})"),
                saldo: dinero_de_real(saldo, Divisa::Dop)?,
            });
        }
        Ok(situacion)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::dominio::foto_patrimonio::{Calidad, Origen, Rubro};
    use chrono::NaiveDate;
    use rusqlite::Connection;

    fn base() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        crate::migraciones::ejecutar(&mut c).unwrap();
        c.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        c
    }

    fn foto(anio: i32, mes: u32, origen: Origen, cuenta: i64) -> Foto {
        Foto::construir(
            NaiveDate::from_ymd_opt(anio, mes, 9).unwrap(),
            origen,
            TasaCambio::nueva(58.7312).unwrap(),
            Calidad::Completa,
            vec![
                (Rubro::Cuenta, "Ahorros".into(), Dinero::de_centavos(cuenta, Divisa::Dop)),
                (Rubro::Cuenta, "Dólares".into(), Dinero::de_centavos(1_234_57, Divisa::Usd)),
                (Rubro::Tarjeta, "Visa".into(), Dinero::de_centavos(8_765_43, Divisa::Dop)),
            ],
        )
        .unwrap()
    }

    #[test]
    fn guardar_ida_y_vuelta_conserva_cada_centavo_y_la_tasa() {
        let mut c = base();
        let tx = c.transaction().unwrap();
        let mut r = FotosSqlite::nuevo(&tx);
        let f = foto(2026, 3, Origen::Manual, 321_654_98);
        r.guardar_foto(&f).unwrap();
        let leida = r.fotos().unwrap().remove(0);
        assert_eq!((leida.anio, leida.mes, leida.fecha.as_str(), leida.origen.as_str()), (2026, 3, "09/03/2026", "manual"));
        assert_eq!(leida.tasa_usd_dop, f.tasa_usd_dop);
        assert_eq!((leida.activos, leida.pasivos, leida.patrimonio, leida.liquido), (f.activos, f.pasivos, f.patrimonio, f.liquido));
        let partidas = r.partidas_de(leida.id).unwrap();
        assert_eq!(partidas.len(), 3);
        assert_eq!(partidas[1].divisa, Divisa::Usd);
        assert_eq!(partidas[1].importe_dop, f.partidas[1].importe_dop);
    }

    #[test]
    fn guardar_dos_veces_el_mes_reemplaza_la_foto_y_sus_partidas_sin_duplicar() {
        let mut c = base();
        let tx = c.transaction().unwrap();
        let mut r = FotosSqlite::nuevo(&tx);
        let id1 = r.guardar_foto(&foto(2026, 3, Origen::Real, 100_00)).unwrap();
        let id2 = r.guardar_foto(&foto(2026, 3, Origen::Manual, 555_55)).unwrap();
        assert_eq!(id1, id2, "es la misma fila");
        let fotos = r.fotos().unwrap();
        assert_eq!(fotos.len(), 1);
        assert_eq!(fotos[0].origen, "manual");
        assert_eq!(r.partidas_de(id1).unwrap().len(), 3, "las partidas viejas no se acumularon");
        assert_eq!(r.partidas_de(id1).unwrap()[0].importe.centavos(), 555_55);
    }

    #[test]
    fn sellar_anteriores_sella_solo_los_meses_previos_y_la_sellada_no_se_reemplaza() {
        let mut c = base();
        let tx = c.transaction().unwrap();
        let mut r = FotosSqlite::nuevo(&tx);
        r.guardar_foto(&foto(2025, 12, Origen::Real, 100_01)).unwrap();
        r.guardar_foto(&foto(2026, 1, Origen::Real, 200_02)).unwrap();
        assert_eq!(r.sellar_anteriores(2026, 1).unwrap(), 1, "solo diciembre es anterior a enero");
        let fotos = r.fotos().unwrap();
        assert_eq!((fotos[0].mes, fotos[0].sellada), (12, true));
        assert_eq!((fotos[1].mes, fotos[1].sellada), (1, false));

        let e = r.guardar_foto(&foto(2025, 12, Origen::Manual, 999_99)).unwrap_err();
        assert_eq!(e, ErrorAlmacen::FotoSellada { anio: 2025, mes: 12 });
        let intacta = r.fotos().unwrap().remove(0);
        assert_eq!(intacta.origen, "real");
        assert_eq!(r.partidas_de(intacta.id).unwrap()[0].importe.centavos(), 100_01, "ni la foto ni sus partidas cambiaron");
        assert_eq!(r.sellar_anteriores(2026, 1).unwrap(), 0, "sellar es idempotente");
    }

    #[test]
    fn el_listado_sale_ordenado_por_anio_y_mes_aunque_se_guarde_en_desorden() {
        let mut c = base();
        let tx = c.transaction().unwrap();
        let mut r = FotosSqlite::nuevo(&tx);
        for (a, m) in [(2026, 2), (2025, 11), (2026, 1), (2025, 12)] {
            r.guardar_foto(&foto(a, m, Origen::Real, 100_01)).unwrap();
        }
        let orden: Vec<(i32, u32)> = r.fotos().unwrap().iter().map(|f| (f.anio, f.mes)).collect();
        assert_eq!(orden, vec![(2025, 11), (2025, 12), (2026, 1), (2026, 2)]);
    }

    #[test]
    fn la_situacion_lee_cuentas_tarjetas_y_prestamos_del_esquema_real() {
        let mut c = base();
        c.execute_batch(
            "INSERT INTO cuentas_ahorro (nombre, divisa, balance_actual) VALUES ('Ahorro', 'DOP', 1234.56);
             INSERT INTO cuentas_ahorro (nombre, divisa, balance_actual, archivada) VALUES ('Cerrada', 'DOP', 0, 1);
             INSERT INTO tarjetas (entidad, nombre_tarjeta, fecha_corte, fecha_limite_pago, balance_pesos, balance_dolares) VALUES ('Banco', 'Oro', 5, 20, 100.25, 7.50);
             INSERT INTO prestamos (tipo_prestamo, monto_prestamo, institucion_financiera, tasa_actual, monto_cuota, dia_pago) VALUES ('consumo', 9000.10, 'Otro', 10, 500, 3);",
        )
        .unwrap();
        let tx = c.transaction().unwrap();
        let s = FotosSqlite::nuevo(&tx).situacion_para_foto().unwrap();
        assert!(s.cuentas.iter().any(|x| x.nombre == "Ahorro" && x.saldo.centavos() == 123_456 && !x.archivada));
        assert!(s.cuentas.iter().any(|x| x.nombre == "Cerrada" && x.archivada));
        assert_eq!((s.tarjetas[0].deuda_pesos.centavos(), s.tarjetas[0].deuda_dolares.centavos(), s.tarjetas[0].activa), (10_025, 750, true));
        assert_eq!(s.prestamos[0].saldo.centavos(), 900_010, "sin saldo llevado, el monto");
    }
}
