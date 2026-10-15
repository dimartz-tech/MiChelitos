//! Comandos de bonificaciones (créditos del emisor sobre una tarjeta).

use serde::{Deserialize, Serialize};

use super::ayudantes::{con_almacen, lee_almacen};
use crate::aplicacion;
use crate::aplicacion::registrar_bonificacion::{registrar_bonificacion, revertir_bonificacion, DatosBonificacion};
use crate::dominio::bonificacion::Bonificacion;
use crate::dominio::dinero::Divisa;
use crate::ipc;

#[derive(Serialize, Deserialize, Debug)]
pub struct BonificacionDto {
    pub id: i64,
    pub fecha: String,
    pub tarjeta_id: i64,
    pub entidad: String,
    pub nombre_tarjeta: String,
    pub monto: f64,
    pub divisa: String,
    pub concepto: String,
    pub gasto_id: Option<i64>,
}

#[tauri::command]
pub fn obtener_bonificaciones() -> Result<Vec<BonificacionDto>, String> {
    lee_almacen(|a| {
        Ok(aplicacion::bonificaciones::listar_bonificaciones(a)?
            .into_iter()
            .map(|b| BonificacionDto {
                id: b.id,
                fecha: b.fecha,
                tarjeta_id: b.tarjeta_id,
                entidad: b.entidad,
                nombre_tarjeta: b.nombre_tarjeta,
                monto: b.monto,
                divisa: b.divisa,
                concepto: b.concepto,
                gasto_id: b.gasto_id,
            })
            .collect())
    })
}

/// Registra un crédito del emisor sobre una tarjeta. Reduce su deuda sin
/// alterar el consumo que lo originó.
#[tauri::command]
pub fn crear_bonificacion(
    fecha: String,
    tarjeta_id: i64,
    monto: ipc::ImporteDecimal,
    divisa: String,
    concepto: String,
    gasto_id: Option<i64>,
) -> Result<i64, String> {
    let divisa = if divisa == "USD" { Divisa::Usd } else { Divisa::Dop };
    // El importe llega como se escribió: el céntimo lo decide el núcleo con esos dígitos, y se casa
    // con la divisa declarada (la de la bonificación, que es del titular y no de una fila).
    let bonificacion = Bonificacion::nueva(monto.con_divisa(divisa), &concepto)?;

    con_almacen(|a| Ok(registrar_bonificacion(DatosBonificacion { fecha, tarjeta_id, bonificacion, gasto_id }, a)?))
}

#[tauri::command]
pub fn eliminar_bonificacion(id: i64) -> Result<(), String> {
    con_almacen(|a| Ok(revertir_bonificacion(id, a)?))
}
