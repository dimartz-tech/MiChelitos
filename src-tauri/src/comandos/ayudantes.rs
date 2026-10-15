//! Los ayudantes que abren la conexión y la transacción de cada comando.
//!
//! Son lo único de `comandos/` que toca `db_sql`: un comando escribe por `con_*` (transacción `IMMEDIATE`) y lista
//! por `lee_*` (diferida). La prueba `los_comandos_no_abren_transacciones_diferidas_a_mano` lo vigila.

use crate::adaptadores::sqlite::catalogos::CatalogosSqlite;
use crate::adaptadores::sqlite::cuentas::CuentasSqlite;
use crate::adaptadores::sqlite::gastos::AlmacenSqlite;
use crate::adaptadores::sqlite::prestamos::PrestamosSqlite;
use crate::adaptadores::sqlite::tarjetas::TarjetasSqlite;
use crate::db_sql;
use chrono::Local;

/// Corre `f` sobre el almacén de catálogos dentro de una transacción y la confirma si salió bien.
///
/// Es todo lo que los comandos de categorías y clientes tienen de infraestructura; las reglas están en
/// `dominio::catalogo` y `aplicacion::catalogos`.
pub(super) fn con_catalogos_como<T>(
    escritura: bool,
    
    f: impl FnOnce(&mut CatalogosSqlite) -> Result<T, String>,
) -> Result<T, String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = db_sql::transaccion(&mut conn, escritura).map_err(|e| e.to_string())?;
    let resultado = {
        let mut almacen = CatalogosSqlite::nuevo(&tx);
        f(&mut almacen)?
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(resultado)
}

/// Una operación que **escribe**: transacción `IMMEDIATE` (ver `db_sql::transaccion`).
pub(super) fn con_catalogos<T>(f: impl FnOnce(&mut CatalogosSqlite) -> Result<T, String>) -> Result<T, String> {
    con_catalogos_como(true, f)
}

/// Un listado: transacción diferida, que no ocupa el turno de escritura.
pub(super) fn lee_catalogos<T>(f: impl FnOnce(&mut CatalogosSqlite) -> Result<T, String>) -> Result<T, String> {
    con_catalogos_como(false, f)
}

/// Corre `f` sobre el almacén de gastos/cuentas/facturas dentro de una transacción y la confirma si salió bien.
pub(super) fn con_almacen_como<T>(
    escritura: bool,
    f: impl FnOnce(&mut AlmacenSqlite) -> Result<T, String>) -> Result<T, String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = db_sql::transaccion(&mut conn, escritura).map_err(|e| e.to_string())?;
    let resultado = {
        let mut almacen = AlmacenSqlite::nuevo(&tx);
        f(&mut almacen)?
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(resultado)
}

/// Una operación que **escribe**: transacción `IMMEDIATE` (ver `db_sql::transaccion`).
pub(super) fn con_almacen<T>(f: impl FnOnce(&mut AlmacenSqlite) -> Result<T, String>) -> Result<T, String> {
    con_almacen_como(true, f)
}

/// Un listado: transacción diferida, que no ocupa el turno de escritura.
pub(super) fn lee_almacen<T>(f: impl FnOnce(&mut AlmacenSqlite) -> Result<T, String>) -> Result<T, String> {
    con_almacen_como(false, f)
}

/// Como `con_catalogos`, para las tarjetas.
pub(super) fn con_tarjetas_como<T>(
    escritura: bool,
    f: impl FnOnce(&mut TarjetasSqlite) -> Result<T, String>) -> Result<T, String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = db_sql::transaccion(&mut conn, escritura).map_err(|e| e.to_string())?;
    let resultado = {
        let mut almacen = TarjetasSqlite::nuevo(&tx);
        f(&mut almacen)?
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(resultado)
}

/// Una operación que **escribe**: transacción `IMMEDIATE` (ver `db_sql::transaccion`).
pub(super) fn con_tarjetas<T>(f: impl FnOnce(&mut TarjetasSqlite) -> Result<T, String>) -> Result<T, String> {
    con_tarjetas_como(true, f)
}

/// Un listado: transacción diferida, que no ocupa el turno de escritura.
pub(super) fn lee_tarjetas<T>(f: impl FnOnce(&mut TarjetasSqlite) -> Result<T, String>) -> Result<T, String> {
    con_tarjetas_como(false, f)
}

/// Como `con_catalogos`, para los financiamientos.
pub(super) fn con_prestamos_como<T>(
    escritura: bool,
    f: impl FnOnce(&mut PrestamosSqlite) -> Result<T, String>) -> Result<T, String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = db_sql::transaccion(&mut conn, escritura).map_err(|e| e.to_string())?;
    let resultado = {
        let mut almacen = PrestamosSqlite::nuevo(&tx);
        f(&mut almacen)?
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(resultado)
}

/// Una operación que **escribe**: transacción `IMMEDIATE` (ver `db_sql::transaccion`).
pub(super) fn con_prestamos<T>(f: impl FnOnce(&mut PrestamosSqlite) -> Result<T, String>) -> Result<T, String> {
    con_prestamos_como(true, f)
}

/// Un listado: transacción diferida, que no ocupa el turno de escritura.
pub(super) fn lee_prestamos<T>(f: impl FnOnce(&mut PrestamosSqlite) -> Result<T, String>) -> Result<T, String> {
    con_prestamos_como(false, f)
}

/// Como `con_catalogos`, para el alta, corrección y consulta de cuentas.
pub(super) fn con_cuentas_como<T>(
    escritura: bool,
    f: impl FnOnce(&mut CuentasSqlite) -> Result<T, String>) -> Result<T, String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = db_sql::transaccion(&mut conn, escritura).map_err(|e| e.to_string())?;
    let resultado = {
        let mut almacen = CuentasSqlite::nuevo(&tx);
        f(&mut almacen)?
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(resultado)
}

/// Una operación que **escribe**: transacción `IMMEDIATE` (ver `db_sql::transaccion`).
pub(super) fn con_cuentas<T>(f: impl FnOnce(&mut CuentasSqlite) -> Result<T, String>) -> Result<T, String> {
    con_cuentas_como(true, f)
}

/// Un listado: transacción diferida, que no ocupa el turno de escritura.
pub(super) fn lee_cuentas<T>(f: impl FnOnce(&mut CuentasSqlite) -> Result<T, String>) -> Result<T, String> {
    con_cuentas_como(false, f)
}

/// La fecha de hoy como la guarda la aplicación: `dd/mm/aaaa`.
pub(super) fn hoy_dd_mm_aaaa() -> String {
    Local::now().format("%d/%m/%Y").to_string()
}
