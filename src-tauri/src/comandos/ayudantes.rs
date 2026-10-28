//! Los ayudantes que abren la conexión y la transacción de cada comando.
//!
//! Son lo único de `comandos/` que toca `db_sql`: un comando escribe por `con_*` (transacción `IMMEDIATE`) y lista
//! por `lee_*` (diferida). La prueba `los_comandos_no_abren_transacciones_diferidas_a_mano` lo vigila.
//!
//! Todo pasa por `en_transaccion` (la única que abre la conexión); cada agregado solo dice qué adaptador construir.

use crate::adaptadores::sqlite::catalogos::CatalogosSqlite;
use crate::adaptadores::sqlite::cuentas::CuentasSqlite;
use crate::adaptadores::sqlite::fotos::FotosSqlite;
use crate::adaptadores::sqlite::gastos::AlmacenSqlite;
use crate::adaptadores::sqlite::prestamos::PrestamosSqlite;
use crate::adaptadores::sqlite::tarjetas::TarjetasSqlite;
use crate::db_sql;
use chrono::Local;
use rusqlite::{Connection, Transaction};

/// Abre la conexión, corre `f` dentro de una transacción y la confirma solo si salió bien.
///
/// `escritura = true` → `IMMEDIATE`; `false` → diferida (ver `db_sql::transaccion`). Si `f` falla, la transacción se
/// suelta sin confirmar y SQLite la deshace. Es la única función de los comandos que abre la conexión.
fn en_transaccion<T>(escritura: bool, f: impl FnOnce(&Transaction) -> Result<T, String>) -> Result<T, String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    en_transaccion_de(&mut conn, escritura, f)
}

/// El cuerpo de `en_transaccion` sobre una conexión ya abierta (así se prueba con una base temporal).
fn en_transaccion_de<T>(
    conn: &mut Connection,
    escritura: bool,
    f: impl FnOnce(&Transaction) -> Result<T, String>,
) -> Result<T, String> {
    let tx = db_sql::transaccion(conn, escritura).map_err(|e| e.to_string())?;
    let resultado = f(&tx)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(resultado)
}

/// Corre `f` sobre el almacén de catálogos (categorías y clientes) dentro de una transacción y la confirma si salió bien.
pub(super) fn con_catalogos_como<T>(escritura: bool, f: impl FnOnce(&mut CatalogosSqlite) -> Result<T, String>) -> Result<T, String> {
    en_transaccion(escritura, |tx| f(&mut CatalogosSqlite::nuevo(tx)))
}

/// Una operación que **escribe**: transacción `IMMEDIATE` (ver `db_sql::transaccion`).
pub(super) fn con_catalogos<T>(f: impl FnOnce(&mut CatalogosSqlite) -> Result<T, String>) -> Result<T, String> {
    con_catalogos_como(true, f)
}

/// Un listado: transacción diferida, que no ocupa el turno de escritura.
pub(super) fn lee_catalogos<T>(f: impl FnOnce(&mut CatalogosSqlite) -> Result<T, String>) -> Result<T, String> {
    con_catalogos_como(false, f)
}

/// Corre `f` sobre el almacén de gastos, cuentas y facturas dentro de una transacción y la confirma si salió bien.
pub(super) fn con_almacen_como<T>(escritura: bool, f: impl FnOnce(&mut AlmacenSqlite) -> Result<T, String>) -> Result<T, String> {
    en_transaccion(escritura, |tx| f(&mut AlmacenSqlite::nuevo(tx)))
}

/// Una operación que **escribe**: transacción `IMMEDIATE` (ver `db_sql::transaccion`).
pub(super) fn con_almacen<T>(f: impl FnOnce(&mut AlmacenSqlite) -> Result<T, String>) -> Result<T, String> {
    con_almacen_como(true, f)
}

/// Un listado: transacción diferida, que no ocupa el turno de escritura.
pub(super) fn lee_almacen<T>(f: impl FnOnce(&mut AlmacenSqlite) -> Result<T, String>) -> Result<T, String> {
    con_almacen_como(false, f)
}

/// Corre `f` sobre las tarjetas dentro de una transacción y la confirma si salió bien.
pub(super) fn con_tarjetas_como<T>(escritura: bool, f: impl FnOnce(&mut TarjetasSqlite) -> Result<T, String>) -> Result<T, String> {
    en_transaccion(escritura, |tx| f(&mut TarjetasSqlite::nuevo(tx)))
}

/// Una operación que **escribe**: transacción `IMMEDIATE` (ver `db_sql::transaccion`).
pub(super) fn con_tarjetas<T>(f: impl FnOnce(&mut TarjetasSqlite) -> Result<T, String>) -> Result<T, String> {
    con_tarjetas_como(true, f)
}

/// Un listado: transacción diferida, que no ocupa el turno de escritura.
pub(super) fn lee_tarjetas<T>(f: impl FnOnce(&mut TarjetasSqlite) -> Result<T, String>) -> Result<T, String> {
    con_tarjetas_como(false, f)
}

/// Corre `f` sobre los financiamientos dentro de una transacción y la confirma si salió bien.
pub(super) fn con_prestamos_como<T>(escritura: bool, f: impl FnOnce(&mut PrestamosSqlite) -> Result<T, String>) -> Result<T, String> {
    en_transaccion(escritura, |tx| f(&mut PrestamosSqlite::nuevo(tx)))
}

/// Una operación que **escribe**: transacción `IMMEDIATE` (ver `db_sql::transaccion`).
pub(super) fn con_prestamos<T>(f: impl FnOnce(&mut PrestamosSqlite) -> Result<T, String>) -> Result<T, String> {
    con_prestamos_como(true, f)
}

/// Un listado: transacción diferida, que no ocupa el turno de escritura.
pub(super) fn lee_prestamos<T>(f: impl FnOnce(&mut PrestamosSqlite) -> Result<T, String>) -> Result<T, String> {
    con_prestamos_como(false, f)
}

/// Corre `f` sobre el alta, corrección y consulta de cuentas dentro de una transacción y la confirma si salió bien.
pub(super) fn con_cuentas_como<T>(escritura: bool, f: impl FnOnce(&mut CuentasSqlite) -> Result<T, String>) -> Result<T, String> {
    en_transaccion(escritura, |tx| f(&mut CuentasSqlite::nuevo(tx)))
}

/// Una operación que **escribe**: transacción `IMMEDIATE` (ver `db_sql::transaccion`).
pub(super) fn con_cuentas<T>(f: impl FnOnce(&mut CuentasSqlite) -> Result<T, String>) -> Result<T, String> {
    con_cuentas_como(true, f)
}

/// Un listado: transacción diferida, que no ocupa el turno de escritura.
pub(super) fn lee_cuentas<T>(f: impl FnOnce(&mut CuentasSqlite) -> Result<T, String>) -> Result<T, String> {
    con_cuentas_como(false, f)
}

/// Corre `f` sobre las fotos del patrimonio dentro de una transacción y la confirma si salió bien.
pub(super) fn con_fotos_como<T>(escritura: bool, f: impl FnOnce(&mut FotosSqlite) -> Result<T, String>) -> Result<T, String> {
    en_transaccion(escritura, |tx| f(&mut FotosSqlite::nuevo(tx)))
}

/// Una operación que **escribe**: transacción `IMMEDIATE` (ver `db_sql::transaccion`).
pub(super) fn con_fotos<T>(f: impl FnOnce(&mut FotosSqlite) -> Result<T, String>) -> Result<T, String> {
    con_fotos_como(true, f)
}

/// Un listado: transacción diferida, que no ocupa el turno de escritura.
pub(super) fn lee_fotos<T>(f: impl FnOnce(&mut FotosSqlite) -> Result<T, String>) -> Result<T, String> {
    con_fotos_como(false, f)
}

/// La fecha de hoy como la guarda la aplicación: `dd/mm/aaaa`.
pub(super) fn hoy_dd_mm_aaaa() -> String {
    Local::now().format("%d/%m/%Y").to_string()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::time::Duration;

    fn base(nombre: &str) -> std::path::PathBuf {
        let ruta = std::env::temp_dir().join(format!("michelitos-ayud-{}-{nombre}.db", std::process::id()));
        let _ = std::fs::remove_file(&ruta);
        Connection::open(&ruta).unwrap().execute_batch("CREATE TABLE t (v INTEGER NOT NULL);").unwrap();
        ruta
    }

    fn filas(ruta: &std::path::Path) -> i64 {
        Connection::open(ruta).unwrap().query_row("SELECT COUNT(*) FROM t", [], |f| f.get(0)).unwrap()
    }

    /// ¿Puede otra conexión tomar el turno de escritura (esperando poco) mientras `f` corre?
    fn otro_puede_escribir(ruta: &std::path::Path, escritura: bool) -> bool {
        let mut conn = Connection::open(ruta).unwrap();
        en_transaccion_de(&mut conn, escritura, |_| {
            let mut otra = Connection::open(ruta).unwrap();
            otra.busy_timeout(Duration::from_millis(100)).unwrap();
            let r = otra.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).is_ok();
            Ok(r)
        })
        .unwrap()
    }

    #[test]
    fn en_ok_confirma_y_devuelve_el_valor() {
        let ruta = base("ok");
        let mut conn = Connection::open(&ruta).unwrap();
        let r = en_transaccion_de(&mut conn, true, |tx| {
            tx.execute("INSERT INTO t VALUES (1)", []).map_err(|e| e.to_string())?;
            Ok(7)
        });
        assert_eq!(r, Ok(7));
        assert_eq!(filas(&ruta), 1);
    }

    #[test]
    fn en_err_deshace_y_propaga_el_mensaje() {
        let ruta = base("err");
        let mut conn = Connection::open(&ruta).unwrap();
        let r: Result<(), String> = en_transaccion_de(&mut conn, true, |tx| {
            tx.execute("INSERT INTO t VALUES (1)", []).map_err(|e| e.to_string())?;
            Err("regla de dominio".to_string())
        });
        assert_eq!(r, Err("regla de dominio".to_string()));
        assert_eq!(filas(&ruta), 0, "un Err no debe dejar nada escrito");
    }

    #[test]
    fn una_escritura_ocupa_el_turno_y_una_lectura_no() {
        let ruta = base("turno");
        assert!(!otro_puede_escribir(&ruta, true), "la escritura debe ser IMMEDIATE");
        assert!(otro_puede_escribir(&ruta, false), "la lectura diferida no debe ocupar el turno de escritura");
    }
}
