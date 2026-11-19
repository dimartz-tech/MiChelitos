//! Adaptador SQLite del puerto de financiamientos (préstamos y líneas de crédito).
//!
//! Como los demás, opera **sobre una transacción ya abierta**: quien la abre la confirma.

use crate::dominio::dinero::Dinero;
use crate::puertos::repositorios::*;
use rusqlite::{OptionalExtension, Row, Transaction};

pub struct PrestamosSqlite<'a> {
    tx: &'a Transaction<'a>,
}

impl<'a> PrestamosSqlite<'a> {
    pub fn nuevo(tx: &'a Transaction<'a>) -> Self {
        PrestamosSqlite { tx }
    }
}

fn fallo(e: rusqlite::Error) -> ErrorAlmacen {
    ErrorAlmacen::Fallo(e.to_string())
}

const COLUMNAS_DE_MOVIMIENTO: &str = "id, prestamo_id, fecha, tipo, monto, interes, capital, saldo_resultante, gasto_id";

fn movimiento_de(r: &Row) -> rusqlite::Result<MovimientoDePrestamo> {
    Ok(MovimientoDePrestamo {
        id: r.get(0)?,
        prestamo_id: r.get(1)?,
        fecha: r.get(2)?,
        tipo: r.get(3)?,
        monto: r.get(4)?,
        interes: r.get(5)?,
        capital: r.get(6)?,
        saldo_resultante: r.get(7)?,
        gasto_id: r.get(8)?,
    })
}

impl AlmacenPrestamos for PrestamosSqlite<'_> {
    fn prestamos(&self) -> Result<Vec<PrestamoLeido>, ErrorAlmacen> {
        let mut stmt = self
            .tx
            .prepare(
                "SELECT p.id, p.tipo_prestamo, p.monto_prestamo, p.institucion_financiera, p.tasa_actual,
                        p.cuotas_totales, p.cuotas_pendientes, p.monto_cuota, p.dia_pago, p.saldo_actual,
                        p.limite_credito, p.tarjeta_id, t.nombre_tarjeta, t.fecha_corte, t.fecha_limite_pago
                 FROM prestamos p LEFT JOIN tarjetas t ON t.id = p.tarjeta_id
                 ORDER BY p.id DESC;",
            )
            .map_err(fallo)?;
        let filas = stmt
            .query_map([], |r| {
                Ok(PrestamoLeido {
                    id: r.get(0)?,
                    tipo_prestamo: r.get(1)?,
                    monto_prestamo: r.get(2)?,
                    institucion_financiera: r.get(3)?,
                    tasa_actual: r.get(4)?,
                    cuotas_totales: r.get(5)?,
                    cuotas_pendientes: r.get(6)?,
                    monto_cuota: r.get(7)?,
                    dia_pago: r.get(8)?,
                    saldo_actual: r.get(9)?,
                    limite_credito: r.get(10)?,
                    tarjeta_id: r.get(11)?,
                    tarjeta_nombre: r.get(12)?,
                    tarjeta_fecha_corte: r.get(13)?,
                    tarjeta_fecha_limite_pago: r.get(14)?,
                })
            })
            .map_err(fallo)?;
        filas.collect::<Result<Vec<_>, _>>().map_err(fallo)
    }

    fn insertar_prestamo(&mut self, p: &PrestamoNuevo) -> Result<i64, ErrorAlmacen> {
        self.tx
            .execute(
                "INSERT INTO prestamos (tipo_prestamo, monto_prestamo, institucion_financiera, tasa_actual, cuotas_totales, cuotas_pendientes, monto_cuota, dia_pago, saldo_actual, limite_credito)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
                (
                    &p.tipo_prestamo,
                    p.monto_prestamo,
                    &p.institucion_financiera,
                    p.tasa_actual,
                    p.cuotas_totales,
                    p.cuotas_pendientes,
                    p.monto_cuota,
                    p.dia_pago,
                    p.saldo_actual,
                    p.limite_credito,
                ),
            )
            .map_err(fallo)?;
        Ok(self.tx.last_insert_rowid())
    }

    fn tipo_de_prestamo(&self, id: i64) -> Result<Option<String>, ErrorAlmacen> {
        self.tx
            .query_row("SELECT tipo_prestamo FROM prestamos WHERE id = ?;", [id], |r| r.get(0))
            .optional()
            .map_err(fallo)
    }

    fn tarjeta_existe(&self, tarjeta_id: i64) -> Result<bool, ErrorAlmacen> {
        let n: i64 = self
            .tx
            .query_row("SELECT COUNT(*) FROM tarjetas WHERE id = ?;", [tarjeta_id], |r| r.get(0))
            .map_err(fallo)?;
        Ok(n > 0)
    }

    fn corregir_prestamo(&mut self, p: &PrestamoCorregido) -> Result<(), ErrorAlmacen> {
        self.tx
            .execute(
                "UPDATE prestamos SET tasa_actual = ?, monto_cuota = ?, dia_pago = ?,
                                      limite_credito = ?, tarjeta_id = ?
                 WHERE id = ?;",
                (p.tasa_actual, p.monto_cuota, p.dia_pago, p.limite_credito, p.tarjeta_id, p.id),
            )
            .map_err(fallo)?;
        Ok(())
    }

    fn estado_de_prestamo(&self, id: i64) -> Result<Option<EstadoDePrestamo>, ErrorAlmacen> {
        self.tx
            .query_row(
                "SELECT saldo_actual, tasa_actual, monto_cuota, cuotas_pendientes, institucion_financiera FROM prestamos WHERE id = ?;",
                [id],
                |r| {
                    Ok(EstadoDePrestamo {
                        saldo: r.get::<_, Option<f64>>(0)?.unwrap_or(0.0),
                        tasa_anual: r.get(1)?,
                        monto_cuota: r.get(2)?,
                        cuotas_pendientes: r.get(3)?,
                        institucion: r.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(fallo)
    }

    fn asentar_movimiento(&mut self, m: &MovimientoNuevo) -> Result<(), ErrorAlmacen> {
        self.tx
            .execute(
                "INSERT INTO movimientos_prestamo (prestamo_id, fecha, tipo, monto, interes, capital, saldo_resultante, gasto_id)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?);",
                (
                    m.prestamo_id,
                    &m.fecha,
                    &m.tipo,
                    m.monto.unidades(),
                    m.interes.unidades(),
                    m.capital.unidades(),
                    m.saldo_resultante.unidades(),
                    m.gasto_id,
                ),
            )
            .map_err(fallo)?;
        self.tx
            .execute(
                "UPDATE prestamos SET saldo_actual = ? WHERE id = ?;",
                (m.saldo_resultante.unidades(), m.prestamo_id),
            )
            .map_err(fallo)?;
        Ok(())
    }

    fn descontar_cuota(&mut self, id: i64) -> Result<(), ErrorAlmacen> {
        // El contador solo existe en los amortizables: la línea revolvente no tiene cuotas contadas.
        self.tx
            .execute(
                "UPDATE prestamos SET cuotas_pendientes = MAX(0, cuotas_pendientes - 1) WHERE id = ? AND cuotas_pendientes IS NOT NULL;",
                [id],
            )
            .map_err(fallo)?;
        Ok(())
    }

    fn movimientos(&self, id: i64) -> Result<Vec<MovimientoDePrestamo>, ErrorAlmacen> {
        let mut stmt = self
            .tx
            .prepare(&format!(
                "SELECT {COLUMNAS_DE_MOVIMIENTO} FROM movimientos_prestamo WHERE prestamo_id = ? ORDER BY id DESC;"
            ))
            .map_err(fallo)?;
        let filas = stmt.query_map([id], movimiento_de).map_err(fallo)?;
        filas.collect::<Result<Vec<_>, _>>().map_err(fallo)
    }

    fn movimiento(&self, id: i64) -> Result<Option<MovimientoDePrestamo>, ErrorAlmacen> {
        self.tx
            .query_row(&format!("SELECT {COLUMNAS_DE_MOVIMIENTO} FROM movimientos_prestamo WHERE id = ?;"), [id], movimiento_de)
            .optional()
            .map_err(fallo)
    }

    fn ultimo_movimiento(&self, prestamo_id: i64) -> Result<Option<i64>, ErrorAlmacen> {
        self.tx
            .query_row("SELECT MAX(id) FROM movimientos_prestamo WHERE prestamo_id = ?;", [prestamo_id], |r| r.get(0))
            .map_err(fallo)
    }

    fn retirar_movimiento(&mut self, id: i64, prestamo_id: i64, saldo_previo: Dinero) -> Result<(), ErrorAlmacen> {
        let borrados = self
            .tx
            .execute("DELETE FROM movimientos_prestamo WHERE id = ? AND prestamo_id = ?;", (id, prestamo_id))
            .map_err(fallo)?;
        if borrados != 1 {
            return Err(ErrorAlmacen::NoEncontrado { entidad: "movimiento de préstamo", id });
        }
        self.tx
            .execute("UPDATE prestamos SET saldo_actual = ? WHERE id = ?;", (saldo_previo.unidades(), prestamo_id))
            .map_err(fallo)?;
        Ok(())
    }

    fn reponer_cuota(&mut self, id: i64) -> Result<(), ErrorAlmacen> {
        self.tx
            .execute(
                "UPDATE prestamos SET cuotas_pendientes = cuotas_pendientes + 1 WHERE id = ? AND cuotas_pendientes IS NOT NULL;",
                [id],
            )
            .map_err(fallo)?;
        Ok(())
    }

    fn eliminar_prestamo(&mut self, id: i64) -> Result<(), ErrorAlmacen> {
        self.tx.execute("DELETE FROM prestamos WHERE id = ?;", [id]).map_err(fallo)?;
        Ok(())
    }
}
