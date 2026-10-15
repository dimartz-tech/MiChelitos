//! Comandos de ingresos informales y cobros en efectivo.

use serde::{Deserialize, Serialize};

use super::ayudantes::{con_almacen, lee_almacen};
use crate::aplicacion;
use crate::dominio::dinero::Divisa;
use crate::dominio::tarjeta::MONEDA_LOCAL;
use crate::ipc;
use crate::puertos::repositorios::AlmacenInformales;

#[derive(Serialize, Deserialize, Debug)]
pub struct IngresoInformal {
    pub id: i64,
    pub fecha: String,
    pub descripcion: String,
    pub monto: f64,
    pub estatus: String,
    pub institucion_deposito: Option<String>,
    pub fecha_pago: Option<String>,
    pub monto_recibido: Option<f64>,
}

#[tauri::command]
pub fn obtener_ingresos_informales() -> Result<Vec<IngresoInformal>, String> {
    lee_almacen(|a| {
        Ok(AlmacenInformales::informales(a)?
            .into_iter()
            .map(|i| IngresoInformal {
                id: i.id,
                fecha: i.fecha,
                descripcion: i.descripcion,
                monto: i.monto,
                estatus: i.estatus,
                institucion_deposito: i.institucion_deposito,
                fecha_pago: i.fecha_pago,
                monto_recibido: i.monto_recibido,
            })
            .collect())
    })
}

#[tauri::command]
pub fn crear_ingreso_informal(fecha: String, descripcion: String, monto: ipc::ImporteDecimal) -> Result<i64, String> {
    // El importe llega como se escribió y el núcleo decide el céntimo con esos dígitos (`1.005` sube a 1.01).
    con_almacen(|a| {
        Ok(aplicacion::informales::crear_ingreso_informal(&fecha, &descripcion, monto.con_divisa(MONEDA_LOCAL), a)?)
    })
}

#[tauri::command]
pub fn marcar_informal_pagado(
    id: i64,
    cuenta_ahorro_id: i64,
    fecha: String,
    monto_recibido: ipc::ImporteDecimal,
) -> Result<(), String> {
    con_almacen(|a| {
        // Un ingreso informal es en moneda local, de modo que su cobro también.
        Ok(aplicacion::informales::marcar_informal_pagado(
            id,
            cuenta_ahorro_id,
            &fecha,
            monto_recibido.con_divisa(MONEDA_LOCAL),
            a,
        )?)
    })
}

#[tauri::command]
pub fn crear_cobro_efectivo_informal(fecha: String, descripcion: String, monto: ipc::ImporteDecimal, divisa: String) -> Result<i64, String> {
    con_almacen(|a| {
        // El importe llega como se escribió y se usa **dos veces** (el ingreso y la caja): una sola conversión
        // garantiza que ambos reciben el mismo valor. Solo «USD» va a la caja de dólares; cualquier otra divisa,
        // a la de pesos (así ha sido siempre).
        let divisa = if divisa == "USD" { Divisa::Usd } else { Divisa::Dop };
        Ok(aplicacion::informales::crear_cobro_efectivo_informal(&fecha, &descripcion, monto.con_divisa(divisa), a)?)
    })
}

#[tauri::command]
pub fn eliminar_ingreso_informal(id: i64, motivo: String) -> Result<String, String> {
    con_almacen(|a| Ok(aplicacion::informales::eliminar_ingreso_informal(id, &motivo, a)?))
}
