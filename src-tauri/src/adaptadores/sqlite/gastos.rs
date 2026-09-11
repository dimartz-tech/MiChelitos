//! Adaptador SQLite de los puertos del vertical de Gastos.
//!
//! Opera **sobre una transacción ya abierta**, que es lo que preserva la
//! atomicidad: el caso de uso mueve saldos y después inserta, y si algo falla
//! quien abrió la transacción la deshace sin que el caso de uso tenga que
//! compensar nada por su cuenta.

use crate::dominio::bonificacion::Bonificacion;
use crate::dominio::conversion::{Conversion, EstadoConversion};
use crate::dominio::tarjeta::PoliticaLiquidacion;
use crate::dominio::dinero::{Dinero, Divisa, TasaCambio};
use crate::puertos::repositorios::*;
use rusqlite::{params, OptionalExtension, Transaction};

pub struct AlmacenSqlite<'a> {
    tx: &'a Transaction<'a>,
}

impl<'a> AlmacenSqlite<'a> {
    pub fn nuevo(tx: &'a Transaction<'a>) -> Self {
        AlmacenSqlite { tx }
    }
}

fn fallo(e: rusqlite::Error) -> ErrorAlmacen {
    ErrorAlmacen::Fallo(e.to_string())
}

/// Interpreta el texto de divisa igual que el resto del sistema: solo se
/// distingue "USD" y cualquier otro valor se trata como pesos, porque
/// `gastos.divisa` es la única columna de divisa del esquema sin `CHECK`.
fn divisa_desde_texto(texto: &str) -> Divisa {
    if texto == "USD" {
        Divisa::Usd
    } else {
        Divisa::Dop
    }
}

impl RepositorioCategorias for AlmacenSqlite<'_> {
    fn nombre(&self, categoria_id: i64) -> Result<Option<String>, ErrorAlmacen> {
        self.tx
            .query_row("SELECT nombre FROM categorias WHERE id = ?;", [categoria_id], |r| {
                r.get::<_, String>(0)
            })
            .optional()
            .map_err(fallo)
    }
}

impl RepositorioGastos for AlmacenSqlite<'_> {
    fn insertar(&mut self, gasto: &GastoAPersistir) -> Result<i64, ErrorAlmacen> {
        self.tx
            .execute(
                "INSERT INTO gastos (fecha, monto, divisa, descripcion, categoria_id, metodo_pago, costo_adicional, tarjeta_id, cuenta_ahorro_id, tasa_conversion, monto_liquidado, divisa_liquidada, estado_conversion)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
                params![
                    gasto.fecha,
                    gasto.monto.unidades(),
                    gasto.monto.divisa().codigo(),
                    gasto.descripcion,
                    gasto.categoria_id,
                    gasto.metodo_pago,
                    gasto.cargos.unidades(),
                    gasto.tarjeta_id,
                    gasto.cuenta_ahorro_id,
                    gasto.estado_conversion.conversion().map(|c| c.tasa().valor()),
                    gasto.estado_conversion.conversion().map(|c| c.destino().unidades()),
                    gasto.estado_conversion.conversion().map(|c| c.destino().divisa().codigo()),
                    gasto.estado_conversion.codigo(),
                ],
            )
            .map_err(fallo)?;
        Ok(self.tx.last_insert_rowid())
    }

    fn obtener(&self, gasto_id: i64) -> Result<GastoGuardado, ErrorAlmacen> {
        let fila = self
            .tx
            .query_row(
                "SELECT monto, divisa, metodo_pago, costo_adicional, tarjeta_id, cuenta_ahorro_id,
                        tasa_conversion, monto_liquidado, divisa_liquidada, estado_conversion
                 FROM gastos WHERE id = ?;",
                [gasto_id],
                |r| {
                    Ok((
                        r.get::<_, f64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, f64>(3)?,
                        r.get::<_, Option<i64>>(4)?,
                        r.get::<_, Option<i64>>(5)?,
                        r.get::<_, Option<f64>>(6)?,
                        r.get::<_, Option<f64>>(7)?,
                        r.get::<_, Option<String>>(8)?,
                        r.get::<_, Option<String>>(9)?,
                    ))
                },
            )
            .optional()
            .map_err(fallo)?
            .ok_or(ErrorAlmacen::NoEncontrado { entidad: "gasto", id: gasto_id })?;

        let divisa = divisa_desde_texto(&fila.1);
        let monto = Dinero::nuevo(fila.0, divisa).map_err(|e| ErrorAlmacen::Fallo(e.to_string()))?;

        // Las tres columnas de conversión van juntas: o están las tres o no
        // hay conversión.
        let conversion = match (fila.6, fila.7, fila.8.as_deref()) {
            (Some(tasa), Some(liquidado), Some(divisa_liq)) => {
                let destino = Dinero::nuevo(liquidado, divisa_desde_texto(divisa_liq))
                    .map_err(|e| ErrorAlmacen::Fallo(e.to_string()))?;
                let tasa = TasaCambio::nueva(tasa)
                    .map_err(|e| ErrorAlmacen::Fallo(e.to_string()))?;
                Some(
                    Conversion::reconstruir(monto, destino, tasa)
                        .map_err(|e| ErrorAlmacen::Fallo(e.to_string()))?,
                )
            }
            _ => None,
        };

        let estado_conversion = match (conversion, fila.9.as_deref()) {
            (Some(c), _) => EstadoConversion::Liquidada(c),
            (None, Some("pendiente")) => EstadoConversion::Pendiente,
            _ => EstadoConversion::NoAplica,
        };

        // Los cargos van en la divisa que se debitó.
        let divisa_cargos = conversion.map(|c| c.destino().divisa()).unwrap_or(divisa);

        Ok(GastoGuardado {
            id: gasto_id,
            monto,
            metodo_pago: fila.2,
            cargos: Dinero::nuevo(fila.3, divisa_cargos)
                .map_err(|e| ErrorAlmacen::Fallo(e.to_string()))?,
            tarjeta_id: fila.4,
            cuenta_ahorro_id: fila.5,
            estado_conversion,
        })
    }

    fn liquidar(
        &mut self,
        gasto_id: i64,
        estado: EstadoConversion,
    ) -> Result<(), ErrorAlmacen> {
        let c = estado.conversion();
        let filas = self
            .tx
            .execute(
                "UPDATE gastos SET tasa_conversion = ?, monto_liquidado = ?, divisa_liquidada = ?, estado_conversion = ? WHERE id = ?;",
                params![
                    c.map(|c| c.tasa().valor()),
                    c.map(|c| c.destino().unidades()),
                    c.map(|c| c.destino().divisa().codigo()),
                    estado.codigo(),
                    gasto_id,
                ],
            )
            .map_err(fallo)?;
        if filas == 0 {
            return Err(ErrorAlmacen::NoEncontrado { entidad: "gasto", id: gasto_id });
        }
        Ok(())
    }

    fn eliminar(&mut self, gasto_id: i64) -> Result<(), ErrorAlmacen> {
        let filas = self
            .tx
            .execute("DELETE FROM gastos WHERE id = ?;", [gasto_id])
            .map_err(fallo)?;
        if filas == 0 {
            return Err(ErrorAlmacen::NoEncontrado { entidad: "gasto", id: gasto_id });
        }
        Ok(())
    }
}

impl RepositorioBonificaciones for AlmacenSqlite<'_> {
    fn insertar_bonificacion(&mut self, b: &BonificacionAPersistir) -> Result<i64, ErrorAlmacen> {
        self.tx
            .execute(
                "INSERT INTO bonificaciones (fecha, tarjeta_id, monto, divisa, concepto, gasto_id)
                 VALUES (?, ?, ?, ?, ?, ?);",
                params![
                    b.fecha,
                    b.tarjeta_id,
                    b.bonificacion.monto().unidades(),
                    b.bonificacion.monto().divisa().codigo(),
                    b.bonificacion.concepto(),
                    b.gasto_id,
                ],
            )
            .map_err(fallo)?;
        Ok(self.tx.last_insert_rowid())
    }

    fn obtener_bonificacion(&self, id: i64) -> Result<BonificacionGuardada, ErrorAlmacen> {
        let fila: Option<(i64, f64, String, String)> = self
            .tx
            .query_row(
                "SELECT tarjeta_id, monto, divisa, concepto FROM bonificaciones WHERE id = ?;",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()
            .map_err(fallo)?;

        let (tarjeta_id, monto, divisa, concepto) =
            fila.ok_or(ErrorAlmacen::NoEncontrado { entidad: "bonificación", id })?;
        let monto = Dinero::nuevo(monto, divisa_desde_texto(&divisa))
            .map_err(|e| ErrorAlmacen::Fallo(e.to_string()))?;
        let bonificacion = Bonificacion::nueva(monto, &concepto)
            .map_err(|e| ErrorAlmacen::Fallo(e.to_string()))?;
        Ok(BonificacionGuardada { id, tarjeta_id, bonificacion })
    }

    fn eliminar_bonificacion(&mut self, id: i64) -> Result<(), ErrorAlmacen> {
        let filas = self
            .tx
            .execute("DELETE FROM bonificaciones WHERE id = ?;", [id])
            .map_err(fallo)?;
        if filas == 0 {
            return Err(ErrorAlmacen::NoEncontrado { entidad: "bonificación", id });
        }
        Ok(())
    }
}

impl RepositorioTarjetas for AlmacenSqlite<'_> {
    fn ajustar_deuda(&mut self, tarjeta_id: i64, delta: Dinero) -> Result<(), ErrorAlmacen> {
        let sql = match delta.divisa() {
            Divisa::Usd => "UPDATE tarjetas SET balance_dolares = balance_dolares + ? WHERE id = ?;",
            Divisa::Dop => "UPDATE tarjetas SET balance_pesos = balance_pesos + ? WHERE id = ?;",
        };
        let filas =
            self.tx.execute(sql, params![delta.unidades(), tarjeta_id]).map_err(fallo)?;
        if filas == 0 {
            return Err(ErrorAlmacen::NoEncontrado { entidad: "tarjeta", id: tarjeta_id });
        }
        Ok(())
    }

    fn deuda(&self, tarjeta_id: i64, divisa: Divisa) -> Result<Dinero, ErrorAlmacen> {
        let columna = match divisa {
            Divisa::Usd => "balance_dolares",
            Divisa::Dop => "balance_pesos",
        };
        let balance: Option<f64> = self
            .tx
            .query_row(
                &format!("SELECT {columna} FROM tarjetas WHERE id = ?;"),
                [tarjeta_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(fallo)?;
        let balance =
            balance.ok_or(ErrorAlmacen::NoEncontrado { entidad: "tarjeta", id: tarjeta_id })?;
        Dinero::nuevo(balance, divisa).map_err(|e| ErrorAlmacen::Fallo(e.to_string()))
    }

    fn politica(&self, tarjeta_id: i64) -> Result<PoliticaLiquidacion, ErrorAlmacen> {
        let codigo: Option<String> = self
            .tx
            .query_row(
                "SELECT politica_liquidacion FROM tarjetas WHERE id = ?;",
                [tarjeta_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(fallo)?
            .ok_or(ErrorAlmacen::NoEncontrado { entidad: "tarjeta", id: tarjeta_id })?;
        Ok(PoliticaLiquidacion::desde_codigo(codigo.as_deref()))
    }
}

impl RepositorioCuentas for AlmacenSqlite<'_> {
    fn ajustar_saldo(&mut self, cuenta_id: i64, delta: Dinero) -> Result<(), ErrorAlmacen> {
        // La divisa se comprueba aquí y no en SQL. El esquema guarda un solo
        // balance por cuenta, así que sin esta verificación se podrían restar
        // dólares de un saldo en pesos, que es exactamente H2.
        let actual = self.saldo(cuenta_id)?;
        let nuevo = actual.sumar(&delta).map_err(|e| ErrorAlmacen::Fallo(e.to_string()))?;
        self.tx
            .execute(
                "UPDATE cuentas_ahorro SET balance_actual = ? WHERE id = ?;",
                params![nuevo.unidades(), cuenta_id],
            )
            .map_err(fallo)?;
        Ok(())
    }

    fn caja(&self, divisa: Divisa) -> Result<i64, ErrorAlmacen> {
        // Se busca por papel, no por nombre. Renombrar la caja ya no la
        // desvincula, y si no hay ninguna la operación falla en lugar de
        // devolver Ok sin mover nada (H3).
        self.tx
            .query_row(
                "SELECT id FROM cuentas_ahorro WHERE es_caja_efectivo = 1 AND divisa = ?;",
                [divisa.codigo()],
                |r| r.get(0),
            )
            .optional()
            .map_err(fallo)?
            .ok_or(ErrorAlmacen::CajaDeEfectivoAusente { divisa })
    }

    fn saldo(&self, cuenta_id: i64) -> Result<Dinero, ErrorAlmacen> {
        let fila: Option<(f64, String)> = self
            .tx
            .query_row(
                "SELECT balance_actual, divisa FROM cuentas_ahorro WHERE id = ?;",
                [cuenta_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(fallo)?;

        let (balance, divisa) =
            fila.ok_or(ErrorAlmacen::NoEncontrado { entidad: "cuenta", id: cuenta_id })?;
        Dinero::nuevo(balance, divisa_desde_texto(&divisa))
            .map_err(|e| ErrorAlmacen::Fallo(e.to_string()))
    }

    fn divisa(&self, cuenta_id: i64) -> Result<Divisa, ErrorAlmacen> {
        self.saldo(cuenta_id).map(|s| s.divisa())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_sql;
    use crate::puertos::contrato::{verificar, Semilla};
    use rusqlite::Connection;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }

    /// Base en memoria con el esquema real. No depende de HOME ni toca disco.
    fn base_sembrada() -> (Connection, Semilla) {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = ON;", []).unwrap();
        db_sql::crear_esquema(&conn).unwrap();

        conn.execute(
            "INSERT INTO cuentas_ahorro (nombre, divisa, balance_actual) VALUES ('Cuenta Contrato', 'DOP', 100000.0);",
            [],
        )
        .unwrap();
        let cuenta_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO tarjetas (entidad, nombre_tarjeta, fecha_corte, fecha_limite_pago, balance_pesos, limite_pesos)
             VALUES ('Banco Ejemplo', 'Tarjeta Ejemplo', 15, 5, 500.0, 100000.0);",
            [],
        )
        .unwrap();
        let tarjeta_id = conn.last_insert_rowid();

        let categoria_id: i64 = conn
            .query_row("SELECT id FROM categorias WHERE nombre = 'Alimentación';", [], |r| r.get(0))
            .unwrap();

        // La crea y la marca la propia inicialización del esquema.
        let caja_id: i64 = conn
            .query_row(
                "SELECT id FROM cuentas_ahorro WHERE es_caja_efectivo = 1 AND divisa = 'DOP';",
                [],
                |r| r.get(0),
            )
            .unwrap();

        let semilla = Semilla {
            categoria_id,
            categoria_nombre: "Alimentación".into(),
            cuenta_id,
            cuenta_saldo: dop(100000.0),
            caja_id,
            caja_saldo: dop(0.0),
            tarjeta_id,
            tarjeta_deuda: dop(500.0),
        };
        (conn, semilla)
    }

    #[test]
    fn sin_caja_marcada_el_adaptador_falla_en_vez_de_no_hacer_nada() {
        // H3 en la implementación real: se retira el papel de caja y la
        // consulta deja de devolver una fila. Antes esto era un Ok silencioso.
        let (mut conn, _) = base_sembrada();
        conn.execute("UPDATE cuentas_ahorro SET es_caja_efectivo = 0;", []).unwrap();
        let tx = conn.transaction().unwrap();
        let almacen = AlmacenSqlite::nuevo(&tx);

        assert!(matches!(
            almacen.caja(Divisa::Dop),
            Err(ErrorAlmacen::CajaDeEfectivoAusente { divisa: Divisa::Dop })
        ));
    }

    #[test]
    fn renombrar_la_caja_ya_no_la_desvincula() {
        // El caso concreto que H3 describía: con la búsqueda por nombre, esto
        // dejaba los gastos en efectivo sin asentar y sin aviso.
        let (mut conn, semilla) = base_sembrada();
        conn.execute(
            "UPDATE cuentas_ahorro SET nombre = 'Caja Chica' WHERE id = ?;",
            [semilla.caja_id],
        )
        .unwrap();
        let tx = conn.transaction().unwrap();
        let almacen = AlmacenSqlite::nuevo(&tx);

        assert_eq!(almacen.caja(Divisa::Dop).unwrap(), semilla.caja_id);
    }

    #[test]
    fn el_adaptador_sqlite_satisface_el_contrato() {
        let (mut conn, semilla) = base_sembrada();
        let tx = conn.transaction().unwrap();
        {
            let mut almacen = AlmacenSqlite::nuevo(&tx);
            verificar(&mut almacen, &semilla, "SQLite");
        }
        tx.commit().unwrap();
    }

    #[test]
    fn una_transaccion_sin_confirmar_no_deja_rastro() {
        // Es la garantía en la que se apoya el caso de uso para no compensar
        // por su cuenta cuando algo falla a mitad.
        let (mut conn, semilla) = base_sembrada();
        {
            let tx = conn.transaction().unwrap();
            {
                let mut a = AlmacenSqlite::nuevo(&tx);
                a.ajustar_saldo(semilla.cuenta_id, dop(-50000.0)).unwrap();
                a.insertar(&GastoAPersistir {
                    fecha: "09/09/2026".into(),
                    monto: dop(50000.0),
                    descripcion: "Se deshará".into(),
                    categoria_id: semilla.categoria_id,
                    metodo_pago: "transferencia".into(),
                    cargos: dop(0.0),
                    tarjeta_id: None,
                    cuenta_ahorro_id: Some(semilla.cuenta_id),
                    estado_conversion: EstadoConversion::NoAplica,
                })
                .unwrap();
            }
            // Se descarta sin confirmar.
        }

        let saldo: f64 = conn
            .query_row("SELECT balance_actual FROM cuentas_ahorro WHERE id = ?;", [semilla.cuenta_id], |r| r.get(0))
            .unwrap();
        let gastos: i64 = conn.query_row("SELECT COUNT(*) FROM gastos;", [], |r| r.get(0)).unwrap();

        assert_eq!(saldo, 100000.0, "el saldo vuelve a su valor original");
        assert_eq!(gastos, 0, "ningún gasto persistió");
    }
}
