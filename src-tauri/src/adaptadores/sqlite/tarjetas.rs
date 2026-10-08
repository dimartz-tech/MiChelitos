//! Adaptador SQLite del puerto de alta, límites y consulta de tarjetas.
//!
//! Como los demás, opera **sobre una transacción ya abierta**: quien la abre la confirma.

use crate::puertos::repositorios::*;
use rusqlite::Transaction;

pub struct TarjetasSqlite<'a> {
    tx: &'a Transaction<'a>,
}

impl<'a> TarjetasSqlite<'a> {
    pub fn nuevo(tx: &'a Transaction<'a>) -> Self {
        TarjetasSqlite { tx }
    }
}

fn fallo(e: rusqlite::Error) -> ErrorAlmacen {
    ErrorAlmacen::Fallo(e.to_string())
}

impl CatalogoDeTarjetas for TarjetasSqlite<'_> {
    fn tarjetas(&self) -> Result<Vec<TarjetaLeida>, ErrorAlmacen> {
        let mut stmt = self
            .tx
            .prepare(
                "SELECT id, entidad, nombre_tarjeta, limite_pesos, limite_dolares, limite_sobregiro_pesos, limite_sobregiro_dolares, balance_pesos, balance_dolares, balance_corte_pesos, balance_corte_dolares, fecha_corte, fecha_limite_pago, limite_ajustado_pesos, limite_ajustado_dolares, politica_liquidacion FROM tarjetas;",
            )
            .map_err(fallo)?;
        let filas = stmt
            .query_map([], |r| {
                Ok(TarjetaLeida {
                    id: r.get(0)?,
                    entidad: r.get(1)?,
                    nombre_tarjeta: r.get(2)?,
                    limite_pesos: r.get(3)?,
                    limite_dolares: r.get(4)?,
                    limite_sobregiro_pesos: r.get(5)?,
                    limite_sobregiro_dolares: r.get(6)?,
                    balance_pesos: r.get(7)?,
                    balance_dolares: r.get(8)?,
                    balance_corte_pesos: r.get(9)?,
                    balance_corte_dolares: r.get(10)?,
                    fecha_corte: r.get(11)?,
                    fecha_limite_pago: r.get(12)?,
                    limite_ajustado_pesos: r.get(13)?,
                    limite_ajustado_dolares: r.get(14)?,
                    politica_liquidacion: r.get(15)?,
                })
            })
            .map_err(fallo)?;
        filas.collect::<Result<Vec<_>, _>>().map_err(fallo)
    }

    fn insertar_tarjeta(&mut self, t: &TarjetaNueva) -> Result<i64, ErrorAlmacen> {
        self.tx
            .execute(
                "INSERT INTO tarjetas (entidad, nombre_tarjeta, limite_pesos, limite_dolares, limite_sobregiro_pesos, limite_sobregiro_dolares, balance_pesos, balance_dolares, balance_corte_pesos, balance_corte_dolares, fecha_corte, fecha_limite_pago)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
                (
                    &t.entidad,
                    &t.nombre_tarjeta,
                    t.limite_pesos,
                    t.limite_dolares,
                    t.limite_sobregiro_pesos,
                    t.limite_sobregiro_dolares,
                    t.balance_pesos,
                    t.balance_dolares,
                    t.balance_corte_pesos,
                    t.balance_corte_dolares,
                    t.fecha_corte,
                    t.fecha_limite_pago,
                ),
            )
            .map_err(fallo)?;
        Ok(self.tx.last_insert_rowid())
    }

    fn actualizar_limites(&mut self, l: &LimitesDeTarjeta) -> Result<(), ErrorAlmacen> {
        self.tx
            .execute(
                "UPDATE tarjetas SET limite_pesos = ?, limite_dolares = ?, limite_sobregiro_pesos = ?, limite_sobregiro_dolares = ?, balance_corte_pesos = ?, balance_corte_dolares = ?, limite_ajustado_pesos = ?, limite_ajustado_dolares = ?, politica_liquidacion = ? WHERE id = ?;",
                (
                    l.limite_pesos,
                    l.limite_dolares,
                    l.limite_sobregiro_pesos,
                    l.limite_sobregiro_dolares,
                    l.balance_corte_pesos,
                    l.balance_corte_dolares,
                    l.limite_ajustado_pesos,
                    l.limite_ajustado_dolares,
                    &l.politica_liquidacion,
                    l.id,
                ),
            )
            .map_err(fallo)?;
        Ok(())
    }
}
