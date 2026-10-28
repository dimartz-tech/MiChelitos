//! Lectura SQLite de los gastos para el resumen por categoría y mes. **Solo lee.**
//!
//! Cada gasto sale de la base como enteros (`CAST(ROUND(importe * 100) AS INTEGER)`: el centavo exacto que ya
//! guarda la columna `REAL`) y se agrupa **en Rust**, sobre enteros. No hay ninguna suma en SQL: la guarda de
//! `sumas_en_centavos` prohíbe `SUM(`/`AVG(`/`TOTAL(` sobre importes, porque un `SUM` de `REAL` acumula error binario.
//!
//! Un gasto es derivado si alguna de las columnas de `db_sql::GASTOS_DERIVADOS` lo señala: se identifica por esos
//! enlaces, nunca por el texto de su descripción. Los abonos (`pagos_tarjeta`) no se leen como gasto.

use super::gastos::AlmacenSqlite;
use crate::db_sql::GASTOS_DERIVADOS;
use crate::puertos::repositorios::*;
use chrono::{Datelike, NaiveDate};
use std::collections::BTreeMap;

fn fallo(e: rusqlite::Error) -> ErrorAlmacen {
    ErrorAlmacen::Fallo(e.to_string())
}

/// Una fila por gasto, con enteros.
struct GastoCrudo {
    id: i64,
    fecha: String,
    categoria_id: i64,
    categoria_nombre: String,
    divisa: String,
    centavos: i64,
    cargo: i64,
    liquidado_dop: Option<i64>,
    derivado: bool,
}

impl ConsultaDeGastosPorCategoria for AlmacenSqlite<'_> {
    fn gastos_agrupados(&self) -> Result<Vec<FilaAgregadoGasto>, ErrorAlmacen> {
        // Tabla y columna son constantes de `GASTOS_DERIVADOS`, nunca entrada externa.
        let derivado = GASTOS_DERIVADOS
            .iter()
            .map(|(tabla, columna, _)| format!("EXISTS (SELECT 1 FROM {tabla} WHERE {columna} = g.id)"))
            .collect::<Vec<_>>()
            .join(" OR ");
        let sql = format!(
            "SELECT g.id, g.fecha, g.categoria_id, c.nombre, g.divisa,
                    CAST(ROUND(g.monto * 100) AS INTEGER),
                    CAST(ROUND(g.costo_adicional * 100) AS INTEGER),
                    CASE WHEN g.estado_conversion = 'liquidado' AND g.monto_liquidado IS NOT NULL
                         THEN CAST(ROUND(g.monto_liquidado * 100) AS INTEGER) END,
                    ({derivado})
             FROM gastos g
             JOIN categorias c ON c.id = g.categoria_id
             ORDER BY g.id;"
        );
        let mut stmt = self.tx.prepare(&sql).map_err(fallo)?;
        let crudos = stmt
            .query_map([], |r| {
                Ok(GastoCrudo {
                    id: r.get(0)?,
                    fecha: r.get(1)?,
                    categoria_id: r.get(2)?,
                    categoria_nombre: r.get(3)?,
                    divisa: r.get(4)?,
                    centavos: r.get(5)?,
                    cargo: r.get(6)?,
                    liquidado_dop: r.get(7)?,
                    derivado: r.get(8)?,
                })
            })
            .map_err(fallo)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(fallo)?;

        // Agrupa por (año, mes, día, categoría, divisa, derivado) sobre enteros.
        let mut grupos: BTreeMap<(i32, u32, u32, i64, String, bool), FilaAgregadoGasto> = BTreeMap::new();
        for g in crudos {
            let fecha = NaiveDate::parse_from_str(g.fecha.trim(), "%d/%m/%Y").map_err(|_| {
                ErrorAlmacen::Fallo(format!(
                    "El gasto {} tiene una fecha que no se entiende («{}»): el resumen la necesita en dd/mm/aaaa.",
                    g.id, g.fecha
                ))
            })?;
            let usd = g.divisa.trim().eq_ignore_ascii_case("USD");
            let divisa = if usd { "USD".to_string() } else { "DOP".to_string() };
            let clave = (fecha.year(), fecha.month(), fecha.day(), g.categoria_id, divisa.clone(), g.derivado);
            let fila = grupos.entry(clave).or_insert_with(|| FilaAgregadoGasto {
                anio: fecha.year(),
                mes: fecha.month(),
                dia: fecha.day(),
                categoria_id: g.categoria_id,
                categoria_nombre: g.categoria_nombre.clone(),
                divisa,
                derivado: g.derivado,
                centavos: 0,
                liquidado_origen_centavos: 0,
                liquidado_dop_centavos: None,
                n: 0,
            });
            // El costo adicional se suma al total; si el gasto USD está liquidado, solo su `monto` queda del lado del
            // emisor (el costo se convierte con la tasa de referencia, junto con los dólares sin liquidar).
            fila.centavos += g.centavos + g.cargo;
            if usd {
                if let Some(dop) = g.liquidado_dop {
                    fila.liquidado_origen_centavos += g.centavos;
                    fila.liquidado_dop_centavos = Some(fila.liquidado_dop_centavos.unwrap_or(0) + dop);
                }
            }
            fila.n += 1;
        }
        Ok(grupos.into_values().collect())
    }
}
