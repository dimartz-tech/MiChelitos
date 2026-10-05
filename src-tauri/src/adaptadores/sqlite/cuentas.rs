//! Adaptador SQLite del puerto de alta, corrección y consulta de cuentas.
//!
//! Como los demás, opera **sobre una transacción ya abierta**: quien la abre la confirma.

use crate::puertos::repositorios::*;
use rusqlite::Transaction;

pub struct CuentasSqlite<'a> {
    tx: &'a Transaction<'a>,
}

impl<'a> CuentasSqlite<'a> {
    pub fn nuevo(tx: &'a Transaction<'a>) -> Self {
        CuentasSqlite { tx }
    }
}

fn fallo(e: rusqlite::Error) -> ErrorAlmacen {
    ErrorAlmacen::Fallo(e.to_string())
}

impl CatalogoDeCuentas for CuentasSqlite<'_> {
    fn cuentas(&self) -> Result<Vec<CuentaLeida>, ErrorAlmacen> {
        let mut stmt = self
            .tx
            .prepare(
                "SELECT id, nombre, divisa, balance_actual, entidad, comision_pago_impuestos
                 FROM cuentas_ahorro ORDER BY nombre ASC;",
            )
            .map_err(fallo)?;
        let filas = stmt
            .query_map([], |r| {
                Ok(CuentaLeida {
                    id: r.get(0)?,
                    nombre: r.get(1)?,
                    divisa: r.get(2)?,
                    balance_actual: r.get(3)?,
                    entidad: r.get(4)?,
                    comision_pago_impuestos: r.get(5)?,
                })
            })
            .map_err(fallo)?;
        filas.collect::<Result<Vec<_>, _>>().map_err(fallo)
    }

    fn insertar_cuenta(&mut self, c: &CuentaNueva) -> Result<i64, ErrorAlmacen> {
        self.tx
            .execute(
                "INSERT INTO cuentas_ahorro (nombre, divisa, balance_actual, entidad, comision_pago_impuestos)
                 VALUES (?, ?, ?, ?, ?);",
                (&c.nombre, &c.divisa, c.saldo_inicial, &c.entidad, c.comision_pago_impuestos),
            )
            .map_err(fallo)?;
        Ok(self.tx.last_insert_rowid())
    }

    fn corregir_cuenta(&mut self, c: &CuentaCorregida) -> Result<bool, ErrorAlmacen> {
        let filas = self
            .tx
            .execute(
                "UPDATE cuentas_ahorro SET nombre = ?, entidad = ?, comision_pago_impuestos = ?
                 WHERE id = ?;",
                (&c.nombre, &c.entidad, c.comision_pago_impuestos, c.id),
            )
            .map_err(fallo)?;
        Ok(filas > 0)
    }

    fn transacciones(&self) -> Result<Vec<TransaccionLeida>, ErrorAlmacen> {
        let mut stmt = self
            .tx
            .prepare(
                "SELECT t.id, t.fecha, t.cuenta_origen_id, co.nombre, t.cuenta_destino_id, cd.nombre,
                        t.monto_origen, t.monto_destino, t.tasa_cambio, t.cargo, t.descripcion
                 FROM transacciones_cuentas t
                 JOIN cuentas_ahorro co ON t.cuenta_origen_id = co.id
                 JOIN cuentas_ahorro cd ON t.cuenta_destino_id = cd.id
                 ORDER BY t.id DESC;",
            )
            .map_err(fallo)?;
        let filas = stmt
            .query_map([], |r| {
                Ok(TransaccionLeida {
                    id: r.get(0)?,
                    fecha: r.get(1)?,
                    cuenta_origen_id: r.get(2)?,
                    cuenta_origen_nombre: r.get(3)?,
                    cuenta_destino_id: r.get(4)?,
                    cuenta_destino_nombre: r.get(5)?,
                    monto_origen: r.get(6)?,
                    monto_destino: r.get(7)?,
                    tasa_cambio: r.get(8)?,
                    cargo: r.get(9)?,
                    descripcion: r.get(10)?,
                })
            })
            .map_err(fallo)?;
        filas.collect::<Result<Vec<_>, _>>().map_err(fallo)
    }
}
