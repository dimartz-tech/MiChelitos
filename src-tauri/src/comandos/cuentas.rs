//! Comandos de cuentas de ahorro y de transferencias entre ellas.

use serde::{Deserialize, Serialize};

use super::ayudantes::{con_almacen, con_cuentas, lee_cuentas};
use crate::aplicacion;
use crate::aplicacion::transferir::{transferir, DatosTransferencia};
use crate::dominio::dinero::Divisa;
use crate::dominio::tarjeta::MONEDA_LOCAL;
use crate::ipc;
use crate::puertos;
use crate::puertos::repositorios::{CatalogoDeCuentas, RepositorioCuentas};

#[derive(Serialize, Deserialize, Debug)]
pub struct CuentaAhorro {
    pub id: i64,
    pub nombre: String,
    pub divisa: String,
    pub balance_actual: f64,
    /// Entidad con la que se mantiene la cuenta. La declara el titular.
    pub entidad: Option<String>,
    /// Tarifa fija por el servicio de pago de impuestos, si la hay pactada.
    pub comision_pago_impuestos: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TransaccionCuenta {
    pub id: i64,
    pub fecha: String,
    pub cuenta_origen_id: i64,
    pub cuenta_origen_nombre: String,
    pub cuenta_destino_id: i64,
    pub cuenta_destino_nombre: String,
    pub monto_origen: f64,
    pub monto_destino: f64,
    pub tasa_cambio: f64,
    pub cargo: f64,
    pub descripcion: Option<String>,
}

fn a_cuenta_ahorro(c: puertos::repositorios::CuentaLeida) -> CuentaAhorro {
    CuentaAhorro {
        id: c.id,
        nombre: c.nombre,
        divisa: c.divisa,
        balance_actual: c.balance_actual,
        entidad: c.entidad,
        comision_pago_impuestos: c.comision_pago_impuestos,
    }
}

/// Las cuentas **activas**: las que se ofrecen en selectores, saldos y operaciones nuevas.
#[tauri::command]
pub fn obtener_cuentas() -> Result<Vec<CuentaAhorro>, String> {
    lee_cuentas(|a| Ok(a.cuentas()?.into_iter().map(a_cuenta_ahorro).collect()))
}

/// Las cuentas archivadas, solo para administrarlas (consultarlas y reactivarlas).
#[tauri::command]
pub fn obtener_cuentas_archivadas() -> Result<Vec<CuentaAhorro>, String> {
    lee_cuentas(|a| Ok(aplicacion::cuentas::listar_archivadas(a)?.into_iter().map(a_cuenta_ahorro).collect()))
}

#[tauri::command]
pub fn crear_cuenta(
    nombre: String,
    divisa: String,
    balance: ipc::ImporteDecimal,
    entidad: Option<String>,
    comision_pago_impuestos: Option<ipc::ImporteDecimal>,
) -> Result<i64, String> {
    con_cuentas(|a| {
        // Los importes llegan como se escribieron: el céntimo lo deciden esos dígitos. La comisión se cobra
        // en moneda local; el saldo inicial se guarda en las unidades que se escribieron.
        Ok(aplicacion::cuentas::crear_cuenta(
            aplicacion::cuentas::DatosCuentaNueva {
                nombre,
                divisa,
                saldo_inicial: balance.con_divisa(Divisa::Dop),
                entidad,
                comision_pago_impuestos: comision_pago_impuestos.map(|c| c.con_divisa(MONEDA_LOCAL)),
            },
            a,
        )?)
    })
}

/// Actualiza los datos declarativos de una cuenta.
///
/// No toca el balance: para eso están las transferencias y los movimientos,
/// que dejan rastro. Aquí solo se corrigen el nombre, la entidad y la tarifa,
/// que son cosas que el titular sabe y el sistema no puede deducir.
#[tauri::command]
pub fn actualizar_cuenta(
    id: i64,
    nombre: String,
    entidad: Option<String>,
    comision_pago_impuestos: Option<ipc::ImporteDecimal>,
) -> Result<(), String> {
    con_cuentas(|a| {
        Ok(aplicacion::cuentas::actualizar_cuenta(
            aplicacion::cuentas::DatosCuentaCorregida {
                id,
                nombre,
                entidad,
                comision_pago_impuestos: comision_pago_impuestos.map(|c| c.con_divisa(MONEDA_LOCAL)),
            },
            a,
        )?)
    })
}

/// Archiva una cuenta con saldo exactamente cero. No existe el borrado: el historial se conserva.
#[tauri::command]
pub fn archivar_cuenta(id: i64) -> Result<(), String> {
    con_almacen(|a| Ok(aplicacion::cuentas::archivar_cuenta(id, a)?))
}

#[tauri::command]
pub fn reactivar_cuenta(id: i64) -> Result<(), String> {
    con_almacen(|a| Ok(aplicacion::cuentas::reactivar_cuenta(id, a)?))
}

#[tauri::command]
pub fn transferir_entre_cuentas(
    fecha: String,
    origen_id: i64,
    destino_id: i64,
    monto_origen: ipc::ImporteDecimal,
    monto_destino: ipc::ImporteDecimal,
    cargo: ipc::ImporteDecimal,
    descripcion: String
) -> Result<(), String> {
    // Traducción pura: la operación y sus invariantes viven en el dominio y en el caso de uso. Aquí solo se
    // convierten los números en importes de la divisa que corresponde a cada cuenta.
    con_almacen(|almacen| {
        // Las divisas son un hecho de cada cuenta: se leen, no se declaran.
        let divisa_origen = almacen.divisa(origen_id).map_err(|e| e.to_string())?;
        let divisa_destino = almacen.divisa(destino_id).map_err(|e| e.to_string())?;

        transferir(
            DatosTransferencia {
                fecha,
                origen_id,
                destino_id,
                // Los tres importes llegan como se escribieron: el céntimo (y con él la tasa que se
                // deduce entre origen y destino) lo deciden esos dígitos. Cada uno se casa con la divisa
                // de SU cuenta, que se lee, no se declara.
                monto_origen: monto_origen.con_divisa(divisa_origen),
                monto_destino: monto_destino.con_divisa(divisa_destino),
                cargo: cargo.con_divisa(divisa_origen),
                descripcion,
            },
            almacen,
        )?;
        Ok(())
    })
}

#[tauri::command]
pub fn obtener_transacciones_cuentas() -> Result<Vec<TransaccionCuenta>, String> {
    lee_cuentas(|a| {
        Ok(a.transacciones()?
            .into_iter()
            .map(|t| TransaccionCuenta {
                id: t.id,
                fecha: t.fecha,
                cuenta_origen_id: t.cuenta_origen_id,
                cuenta_origen_nombre: t.cuenta_origen_nombre,
                cuenta_destino_id: t.cuenta_destino_id,
                cuenta_destino_nombre: t.cuenta_destino_nombre,
                monto_origen: t.monto_origen,
                monto_destino: t.monto_destino,
                tasa_cambio: t.tasa_cambio,
                cargo: t.cargo,
                descripcion: t.descripcion,
            })
            .collect())
    })
}

#[tauri::command]
pub fn eliminar_transaccion_cuenta(id: i64, motivo: String) -> Result<String, String> {
    con_almacen(|a| Ok(aplicacion::transferir::eliminar_transferencia(id, &motivo, a)?))
}
