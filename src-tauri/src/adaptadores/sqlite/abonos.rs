//! Consultas SQLite de los abonos a tarjeta, sobre la misma transacción que el resto de los puertos de `AlmacenSqlite`.

use super::gastos::AlmacenSqlite;
use crate::puertos::repositorios::*;
use rusqlite::OptionalExtension;

fn fallo(e: rusqlite::Error) -> ErrorAlmacen {
    ErrorAlmacen::Fallo(e.to_string())
}

impl ConsultaDeAbonos for AlmacenSqlite<'_> {
    fn abonos_de_tarjeta(&self, tarjeta_id: i64) -> Result<Vec<AbonoLeido>, ErrorAlmacen> {
        let mut stmt = self
            .tx
            .prepare(
                "SELECT p.id, p.fecha_pago, p.monto_pagado, p.divisa, p.cuenta_ahorro_id,
                        c.nombre, p.tasa_cambio
                 FROM pagos_tarjeta p
                 LEFT JOIN cuentas_ahorro c ON c.id = p.cuenta_ahorro_id
                 WHERE p.tarjeta_id = ?
                 ORDER BY p.id DESC;",
            )
            .map_err(fallo)?;
        let filas = stmt
            .query_map([tarjeta_id], |r| {
                Ok(AbonoLeido {
                    id: r.get(0)?,
                    fecha_pago: r.get(1)?,
                    monto_pagado: r.get(2)?,
                    divisa: r.get(3)?,
                    cuenta_ahorro_id: r.get(4)?,
                    cuenta_nombre: r.get(5)?,
                    tasa_cambio: r.get(6)?,
                })
            })
            .map_err(fallo)?;
        filas.collect::<Result<Vec<_>, _>>().map_err(fallo)
    }

    fn resumen_de_abono(&self, id: i64) -> Result<Option<ResumenDeAbono>, ErrorAlmacen> {
        self.tx
            .query_row(
                "SELECT fecha_pago, monto_pagado, divisa FROM pagos_tarjeta WHERE id = ?;",
                [id],
                |r| Ok(ResumenDeAbono { fecha_pago: r.get(0)?, monto_pagado: r.get(1)?, divisa: r.get(2)? }),
            )
            .optional()
            .map_err(fallo)
    }

    fn categoria_de_sistema(&self) -> Result<i64, ErrorAlmacen> {
        self.tx
            .query_row("SELECT id FROM categorias WHERE nombre = 'Otros' LIMIT 1;", [], |r| r.get(0))
            .map_err(fallo)
    }
}
