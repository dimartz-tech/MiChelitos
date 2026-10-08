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
}

impl CategoriaDeSistema for AlmacenSqlite<'_> {
    fn categoria_de_sistema(&self) -> Result<i64, ErrorAlmacen> {
        self.tx
            .query_row("SELECT id FROM categorias WHERE nombre = 'Otros' LIMIT 1;", [], |r| r.get(0))
            .map_err(fallo)
    }
}

impl ConsultaDeAvances for AlmacenSqlite<'_> {
    fn avances_de_tarjeta(&self, tarjeta_id: i64) -> Result<Vec<AvanceLeido>, ErrorAlmacen> {
        let mut stmt = self
            .tx
            .prepare(
                "SELECT a.id, a.fecha, a.monto, a.divisa, a.tipo_cargo, a.tasa, a.cargo,
                        a.cuenta_ahorro_id, c.nombre, a.nota
                 FROM avances_efectivo a
                 JOIN cuentas_ahorro c ON c.id = a.cuenta_ahorro_id
                 WHERE a.tarjeta_id = ?
                 ORDER BY a.id DESC;",
            )
            .map_err(fallo)?;
        let filas = stmt
            .query_map([tarjeta_id], |r| {
                Ok(AvanceLeido {
                    id: r.get(0)?,
                    fecha: r.get(1)?,
                    monto: r.get(2)?,
                    divisa: r.get(3)?,
                    tipo_cargo: r.get(4)?,
                    tasa: r.get(5)?,
                    cargo: r.get(6)?,
                    cuenta_ahorro_id: r.get(7)?,
                    cuenta_nombre: r.get(8)?,
                    nota: r.get(9)?,
                })
            })
            .map_err(fallo)?;
        filas.collect::<Result<Vec<_>, _>>().map_err(fallo)
    }

    fn resumen_de_avance(&self, id: i64) -> Result<Option<ResumenDeAvance>, ErrorAlmacen> {
        self.tx
            .query_row(
                "SELECT fecha, monto, divisa FROM avances_efectivo WHERE id = ?;",
                [id],
                |r| Ok(ResumenDeAvance { fecha: r.get(0)?, monto: r.get(1)?, divisa: r.get(2)? }),
            )
            .optional()
            .map_err(fallo)
    }
}

impl ConsultaDeBonificaciones for AlmacenSqlite<'_> {
    fn bonificaciones(&self) -> Result<Vec<BonificacionLeida>, ErrorAlmacen> {
        let mut stmt = self
            .tx
            .prepare(
                "SELECT b.id, b.fecha, b.tarjeta_id, t.entidad, t.nombre_tarjeta, b.monto, b.divisa, b.concepto, b.gasto_id
                 FROM bonificaciones b JOIN tarjetas t ON t.id = b.tarjeta_id
                 ORDER BY b.id DESC;",
            )
            .map_err(fallo)?;
        let filas = stmt
            .query_map([], |r| {
                Ok(BonificacionLeida {
                    id: r.get(0)?,
                    fecha: r.get(1)?,
                    tarjeta_id: r.get(2)?,
                    entidad: r.get(3)?,
                    nombre_tarjeta: r.get(4)?,
                    monto: r.get(5)?,
                    divisa: r.get(6)?,
                    concepto: r.get(7)?,
                    gasto_id: r.get(8)?,
                })
            })
            .map_err(fallo)?;
        filas.collect::<Result<Vec<_>, _>>().map_err(fallo)
    }
}
