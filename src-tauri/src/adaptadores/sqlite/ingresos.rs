//! Adaptador SQLite del puerto de facturas (ingresos formales).
//!
//! Se implementa sobre `AlmacenSqlite`, que ya envuelve la transacción y el puerto de cuentas: el cobro de una
//! factura toca las dos cosas y debe ocurrir en la misma transacción.

use super::gastos::AlmacenSqlite;
use crate::puertos::repositorios::*;
use rusqlite::OptionalExtension;

fn fallo(e: rusqlite::Error) -> ErrorAlmacen {
    ErrorAlmacen::Fallo(e.to_string())
}

impl AlmacenIngresos for AlmacenSqlite<'_> {
    fn ingresos(&self) -> Result<Vec<IngresoLeido>, ErrorAlmacen> {
        let mut stmt = self
            .tx
            .prepare(
                "SELECT i.id, i.numero_factura, i.cliente_id, c.nombre, c.rnc, i.fecha_emision, i.estatus,
                        i.monto_total, i.porcentaje_retencion, i.monto_retenido, i.institucion_deposito,
                        i.fecha_pago, i.monto_recibido
                 FROM ingresos i
                 JOIN clientes c ON i.cliente_id = c.id
                 ORDER BY i.id DESC;",
            )
            .map_err(fallo)?;
        let filas = stmt
            .query_map([], |r| {
                Ok(IngresoLeido {
                    id: r.get(0)?,
                    numero_factura: r.get(1)?,
                    cliente_id: r.get(2)?,
                    cliente_nombre: r.get(3)?,
                    cliente_rnc: r.get(4)?,
                    fecha_emision: r.get(5)?,
                    estatus: r.get(6)?,
                    monto_total: r.get(7)?,
                    porcentaje_retencion: r.get(8)?,
                    monto_retenido: r.get(9)?,
                    institucion_deposito: r.get(10)?,
                    fecha_pago: r.get(11)?,
                    monto_recibido: r.get(12)?,
                })
            })
            .map_err(fallo)?;
        filas.collect::<Result<Vec<_>, _>>().map_err(fallo)
    }

    fn factura_existe(&self, numero: &str) -> Result<bool, ErrorAlmacen> {
        let n: i64 = self
            .tx
            .query_row("SELECT COUNT(*) FROM ingresos WHERE LOWER(numero_factura) = LOWER(?);", [numero], |r| r.get(0))
            .map_err(fallo)?;
        Ok(n > 0)
    }

    fn cliente_por_rnc(&self, rnc: &str) -> Result<Option<i64>, ErrorAlmacen> {
        self.tx
            .query_row("SELECT id FROM clientes WHERE rnc = ?;", [rnc], |r| r.get(0))
            .optional()
            .map_err(fallo)
    }

    fn registrar_cliente(&mut self, rnc: &str, nombre: &str) -> Result<i64, ErrorAlmacen> {
        self.tx.execute("INSERT INTO clientes (rnc, nombre) VALUES (?, ?);", [rnc, nombre]).map_err(fallo)?;
        Ok(self.tx.last_insert_rowid())
    }

    fn insertar_factura(&mut self, f: &FacturaNueva) -> Result<i64, ErrorAlmacen> {
        self.tx
            .execute(
                "INSERT INTO ingresos (numero_factura, cliente_id, fecha_emision, monto_total, porcentaje_retencion, monto_retenido, estatus)
                 VALUES (?, ?, ?, ?, ?, ?, 'emitida');",
                (&f.numero_factura, f.cliente_id, &f.fecha_emision, f.monto_total, f.porcentaje_retencion, f.monto_retenido),
            )
            .map_err(fallo)?;
        Ok(self.tx.last_insert_rowid())
    }

    fn estado_de_factura(&self, id: i64) -> Result<Option<EstadoDeFactura>, ErrorAlmacen> {
        self.tx
            .query_row(
                "SELECT numero_factura, estatus, monto_total, institucion_deposito, monto_recibido
                 FROM ingresos WHERE id = ?;",
                [id],
                |r| {
                    Ok(EstadoDeFactura {
                        numero_factura: r.get(0)?,
                        estatus: r.get(1)?,
                        monto_total: r.get(2)?,
                        institucion_deposito: r.get(3)?,
                        monto_recibido: r.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(fallo)
    }

    fn corregir_factura(&mut self, f: &FacturaCorregida) -> Result<(), ErrorAlmacen> {
        self.tx
            .execute(
                "UPDATE ingresos SET numero_factura = ?, cliente_id = ?, fecha_emision = ?,
                                     monto_total = ?, porcentaje_retencion = ?, monto_retenido = ?
                 WHERE id = ?;",
                (&f.numero_factura, f.cliente_id, &f.fecha_emision, f.monto_total, f.porcentaje_retencion, f.monto_retenido, f.id),
            )
            .map_err(fallo)?;
        Ok(())
    }

    fn fijar_recibido(&mut self, id: i64, monto_recibido: f64) -> Result<(), ErrorAlmacen> {
        self.tx
            .execute("UPDATE ingresos SET monto_recibido = ? WHERE id = ?;", (monto_recibido, id))
            .map_err(fallo)?;
        Ok(())
    }

    fn eliminar_factura(&mut self, id: i64) -> Result<(), ErrorAlmacen> {
        self.tx.execute("DELETE FROM ingresos WHERE id = ?;", [id]).map_err(fallo)?;
        Ok(())
    }

    fn marcar_cobrada(
        &mut self,
        id: i64,
        cuenta_id: i64,
        nombre_cuenta: &str,
        fecha: &str,
        monto_recibido: f64,
    ) -> Result<bool, ErrorAlmacen> {
        let filas = self
            .tx
            .execute(
                "UPDATE ingresos SET estatus = 'pagada', institucion_deposito = ?,
                                     cuenta_ahorro_id = ?, fecha_pago = ?, monto_recibido = ?
                 WHERE id = ? AND estatus <> 'pagada';",
                (nombre_cuenta, cuenta_id, fecha, monto_recibido, id),
            )
            .map_err(fallo)?;
        Ok(filas > 0)
    }
}

impl RegistroDeCorrecciones for AlmacenSqlite<'_> {
    fn anotar_caso(&mut self, caso: &CasoAAnotar) -> Result<String, ErrorAlmacen> {
        crate::correcciones::insertar(
            self.tx,
            &crate::correcciones::Correccion {
                tipo: &caso.tipo,
                referencia_id: caso.referencia_id,
                descripcion: caso.descripcion.clone(),
                importe: caso.importe,
                divisa: caso.divisa.clone(),
                motivo: &caso.motivo,
            },
            &caso.motivo,
        )
        .map_err(ErrorAlmacen::Fallo)
    }
}

impl BusquedaDeCuentas for AlmacenSqlite<'_> {
    fn nombre_de_cuenta(&self, cuenta_id: i64) -> Result<String, ErrorAlmacen> {
        self.tx
            .query_row("SELECT nombre FROM cuentas_ahorro WHERE id = ?;", [cuenta_id], |r| r.get::<_, String>(0))
            .optional()
            .map_err(fallo)?
            .ok_or(ErrorAlmacen::NoEncontrado { entidad: "cuenta", id: cuenta_id })
    }

    fn cuenta_por_nombre(&self, nombre: &str) -> Result<Option<i64>, ErrorAlmacen> {
        self.tx
            .query_row("SELECT id FROM cuentas_ahorro WHERE nombre = ?;", [nombre], |r| r.get(0))
            .optional()
            .map_err(fallo)
    }
}

impl AlmacenInformales for AlmacenSqlite<'_> {
    fn informales(&self) -> Result<Vec<InformalLeido>, ErrorAlmacen> {
        let mut stmt = self
            .tx
            .prepare(
                "SELECT id, fecha, descripcion, monto, estatus, institucion_deposito, fecha_pago, monto_recibido
                 FROM ingresos_informales ORDER BY id DESC;",
            )
            .map_err(fallo)?;
        let filas = stmt
            .query_map([], |r| {
                Ok(InformalLeido {
                    id: r.get(0)?,
                    fecha: r.get(1)?,
                    descripcion: r.get(2)?,
                    monto: r.get(3)?,
                    estatus: r.get(4)?,
                    institucion_deposito: r.get(5)?,
                    fecha_pago: r.get(6)?,
                    monto_recibido: r.get(7)?,
                })
            })
            .map_err(fallo)?;
        filas.collect::<Result<Vec<_>, _>>().map_err(fallo)
    }

    fn insertar_informal(&mut self, fecha: &str, descripcion: &str, monto: f64) -> Result<i64, ErrorAlmacen> {
        self.tx
            .execute(
                "INSERT INTO ingresos_informales (fecha, descripcion, monto, estatus) VALUES (?, ?, ?, 'pendiente');",
                (fecha, descripcion, monto),
            )
            .map_err(fallo)?;
        Ok(self.tx.last_insert_rowid())
    }

    fn insertar_cobro_en_efectivo(
        &mut self,
        fecha: &str,
        descripcion: &str,
        monto: f64,
        caja: &str,
    ) -> Result<i64, ErrorAlmacen> {
        self.tx
            .execute(
                "INSERT INTO ingresos_informales (fecha, descripcion, monto, estatus, institucion_deposito, fecha_pago, monto_recibido)
                 VALUES (?, ?, ?, 'pagado', ?, ?, ?);",
                (fecha, descripcion, monto, caja, fecha, monto),
            )
            .map_err(fallo)?;
        Ok(self.tx.last_insert_rowid())
    }

    fn estado_de_informal(&self, id: i64) -> Result<Option<EstadoDeInformal>, ErrorAlmacen> {
        self.tx
            .query_row(
                "SELECT descripcion, estatus, monto, institucion_deposito, monto_recibido
                 FROM ingresos_informales WHERE id = ?;",
                [id],
                |r| {
                    Ok(EstadoDeInformal {
                        descripcion: r.get(0)?,
                        estatus: r.get(1)?,
                        monto: r.get(2)?,
                        institucion_deposito: r.get(3)?,
                        monto_recibido: r.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(fallo)
    }

    fn marcar_informal_cobrado(
        &mut self,
        id: i64,
        cuenta_id: i64,
        nombre_cuenta: &str,
        fecha: &str,
        monto_recibido: f64,
    ) -> Result<bool, ErrorAlmacen> {
        let filas = self
            .tx
            .execute(
                "UPDATE ingresos_informales SET estatus = 'pagado', institucion_deposito = ?,
                                                cuenta_ahorro_id = ?, fecha_pago = ?, monto_recibido = ?
                 WHERE id = ? AND estatus <> 'pagado';",
                (nombre_cuenta, cuenta_id, fecha, monto_recibido, id),
            )
            .map_err(fallo)?;
        Ok(filas > 0)
    }

    fn eliminar_informal(&mut self, id: i64) -> Result<(), ErrorAlmacen> {
        self.tx.execute("DELETE FROM ingresos_informales WHERE id = ?;", [id]).map_err(fallo)?;
        Ok(())
    }
}
