//! Adaptador SQLite del puerto de suscripciones, sobre la misma transacción que el resto de los puertos de
//! `AlmacenSqlite`: el cargo (un gasto con tarjeta) y el avance de la fecha de cobro ocurren en una sola transacción.

use super::gastos::AlmacenSqlite;
use crate::puertos::repositorios::*;
use rusqlite::OptionalExtension;

fn fallo(e: rusqlite::Error) -> ErrorAlmacen {
    ErrorAlmacen::Fallo(e.to_string())
}

impl AlmacenSuscripciones for AlmacenSqlite<'_> {
    fn suscripciones_con_tarjeta(&self) -> Result<Vec<SuscripcionLeida>, ErrorAlmacen> {
        let mut stmt = self
            .tx
            .prepare(
                "SELECT s.id, s.plataforma, s.monto, s.tarjeta_id, s.frecuencia, s.dia_facturacion, s.fecha_ultimo_pago, s.divisa, t.entidad, t.nombre_tarjeta, s.fecha_proximo_cobro
                 FROM suscripciones s
                 JOIN tarjetas t ON s.tarjeta_id = t.id
                 ORDER BY s.plataforma ASC;",
            )
            .map_err(fallo)?;
        let filas = stmt
            .query_map([], |r| {
                Ok(SuscripcionLeida {
                    id: r.get(0)?,
                    plataforma: r.get(1)?,
                    monto: r.get(2)?,
                    tarjeta_id: r.get(3)?,
                    frecuencia: r.get(4)?,
                    dia_facturacion: r.get(5)?,
                    fecha_ultimo_pago: r.get(6)?,
                    divisa: r.get(7)?,
                    entidad: r.get(8)?,
                    nombre_tarjeta: r.get(9)?,
                    fecha_proximo_cobro: r.get(10)?,
                })
            })
            .map_err(fallo)?;
        filas.collect::<Result<Vec<_>, _>>().map_err(fallo)
    }

    fn suscripciones_registradas(&self) -> Result<Vec<SuscripcionRegistrada>, ErrorAlmacen> {
        let mut stmt = self
            .tx
            .prepare(
                "SELECT id, plataforma, monto, tarjeta_id, frecuencia, dia_facturacion,
                        divisa, fecha_proximo_cobro
                 FROM suscripciones ORDER BY id;",
            )
            .map_err(fallo)?;
        let filas = stmt
            .query_map([], |r| {
                Ok(SuscripcionRegistrada {
                    id: r.get(0)?,
                    plataforma: r.get(1)?,
                    monto: r.get(2)?,
                    tarjeta_id: r.get(3)?,
                    frecuencia: r.get(4)?,
                    dia_facturacion: r.get(5)?,
                    divisa: r.get(6)?,
                    fecha_proximo_cobro: r.get(7)?,
                })
            })
            .map_err(fallo)?;
        filas.collect::<Result<Vec<_>, _>>().map_err(fallo)
    }

    fn insertar_suscripcion(&mut self, s: &SuscripcionAGuardar) -> Result<i64, ErrorAlmacen> {
        self.tx
            .execute(
                "INSERT INTO suscripciones (plataforma, monto, tarjeta_id, frecuencia, dia_facturacion, divisa, fecha_proximo_cobro) VALUES (?, ?, ?, ?, ?, ?, ?);",
                (&s.plataforma, s.monto, s.tarjeta_id, &s.frecuencia, s.dia_facturacion, &s.divisa, &s.fecha_proximo_cobro),
            )
            .map_err(fallo)?;
        Ok(self.tx.last_insert_rowid())
    }

    fn editar_suscripcion(&mut self, id: i64, s: &SuscripcionAGuardar) -> Result<bool, ErrorAlmacen> {
        let filas = self
            .tx
            .execute(
                "UPDATE suscripciones SET plataforma = ?, monto = ?, tarjeta_id = ?, frecuencia = ?, dia_facturacion = ?, divisa = ?, fecha_proximo_cobro = ? WHERE id = ?;",
                (&s.plataforma, s.monto, s.tarjeta_id, &s.frecuencia, s.dia_facturacion, &s.divisa, &s.fecha_proximo_cobro, id),
            )
            .map_err(fallo)?;
        Ok(filas > 0)
    }

    fn fijar_proximo_cobro(&mut self, id: i64, fecha: &str) -> Result<bool, ErrorAlmacen> {
        let filas = self
            .tx
            .execute("UPDATE suscripciones SET fecha_proximo_cobro = ? WHERE id = ?;", (fecha, id))
            .map_err(fallo)?;
        Ok(filas > 0)
    }

    fn eliminar_suscripcion(&mut self, id: i64) -> Result<(), ErrorAlmacen> {
        self.tx.execute("DELETE FROM suscripciones WHERE id = ?;", [id]).map_err(fallo)?;
        Ok(())
    }

    fn mover_puntero(&mut self, id: i64, siguiente: Option<&str>, marca_de_cobro: Option<&str>) -> Result<(), ErrorAlmacen> {
        self.tx
            .execute("UPDATE suscripciones SET fecha_proximo_cobro = ? WHERE id = ?;", (siguiente, id))
            .map_err(fallo)?;
        if let Some(marca) = marca_de_cobro {
            self.tx
                .execute("UPDATE suscripciones SET fecha_ultimo_pago = ? WHERE id = ?;", (marca, id))
                .map_err(fallo)?;
        }
        Ok(())
    }

    fn categoria_por_nombre(&self, nombre_en_minusculas: &str) -> Result<Option<i64>, ErrorAlmacen> {
        self.tx
            .query_row("SELECT id FROM categorias WHERE LOWER(nombre) = ?;", [nombre_en_minusculas], |r| r.get(0))
            .optional()
            .map_err(fallo)
    }

    fn crear_categoria(&mut self, nombre: &str) -> Result<i64, ErrorAlmacen> {
        self.tx.execute("INSERT INTO categorias (nombre) VALUES (?);", [nombre]).map_err(fallo)?;
        Ok(self.tx.last_insert_rowid())
    }
}
