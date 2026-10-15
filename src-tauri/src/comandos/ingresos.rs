//! Comandos de ingresos formales (facturas a clientes).

use serde::{Deserialize, Serialize};

use super::ayudantes::{con_almacen, lee_almacen};
use crate::aplicacion;
use crate::dominio::tarjeta::MONEDA_LOCAL;
use crate::ipc;
use crate::puertos::repositorios::AlmacenIngresos;

#[derive(Serialize, Deserialize, Debug)]
pub struct Ingreso {
    pub id: i64,
    pub numero_factura: String,
    pub cliente_id: i64,
    pub cliente_nombre: String,
    pub cliente_rnc: String,
    pub fecha_emision: String,
    pub estatus: String,
    pub monto_total: f64,
    pub porcentaje_retencion: f64,
    pub monto_retenido: f64,
    pub institucion_deposito: Option<String>,
    pub fecha_pago: Option<String>,
    pub monto_recibido: Option<f64>,
}

#[tauri::command]
pub fn obtener_ingresos() -> Result<Vec<Ingreso>, String> {
    lee_almacen(|a| {
        Ok(AlmacenIngresos::ingresos(a)?
            .into_iter()
            .map(|i| Ingreso {
                id: i.id,
                numero_factura: i.numero_factura,
                cliente_id: i.cliente_id,
                cliente_nombre: i.cliente_nombre,
                cliente_rnc: i.cliente_rnc,
                fecha_emision: i.fecha_emision,
                estatus: i.estatus,
                monto_total: i.monto_total,
                porcentaje_retencion: i.porcentaje_retencion,
                monto_retenido: i.monto_retenido,
                institucion_deposito: i.institucion_deposito,
                fecha_pago: i.fecha_pago,
                monto_recibido: i.monto_recibido,
            })
            .collect())
    })
}

#[derive(Deserialize)]
pub struct IngresoInput {
    pub numero_factura: String,
    pub rnc_cliente: String,
    pub nombre_cliente: String,
    pub fecha_emision: String,
    pub monto_total: ipc::ImporteDecimal,
    pub porcentaje_retencion: f64,
}

#[tauri::command]
pub fn crear_ingreso(input: IngresoInput) -> Result<i64, String> {
    con_almacen(|a| {
        // El total llega como se escribió; el céntimo lo deciden esos dígitos. Una factura es en moneda local.
        Ok(aplicacion::ingresos::crear_ingreso(
            aplicacion::ingresos::DatosFactura {
                numero_factura: input.numero_factura,
                rnc_cliente: input.rnc_cliente,
                nombre_cliente: input.nombre_cliente,
                fecha_emision: input.fecha_emision,
                monto_total: input.monto_total.con_divisa(MONEDA_LOCAL),
                porcentaje_retencion: input.porcentaje_retencion,
            },
            a,
        )?)
    })
}

#[tauri::command]
pub fn marcar_ingreso_pagado(
    id: i64,
    cuenta_ahorro_id: i64,
    fecha: String,
    monto_recibido: ipc::ImporteDecimal,
) -> Result<(), String> {
    con_almacen(|a| {
        // Una factura se emite en moneda local, de modo que su cobro también.
        Ok(aplicacion::ingresos::marcar_ingreso_pagado(
            id,
            cuenta_ahorro_id,
            &fecha,
            monto_recibido.con_divisa(MONEDA_LOCAL),
            a,
        )?)
    })
}

#[tauri::command]
pub fn actualizar_ingreso(
    id: i64,
    numero_factura: String,
    cliente_id: i64,
    fecha_emision: String,
    monto_total: ipc::ImporteDecimal,
    porcentaje_retencion: f64,
    // Importe cobrado cuando no entró el neto entero. `None` es la regla: se
    // da por cobrado el neto completo.
    cobro_parcial: Option<ipc::ImporteDecimal>,
    // Motivo de la corrección. Obligatorio **solo cuando mueve dinero**: si la
    // factura ya se cobró y el ajuste no es cero, hay un saldo que cambia y
    // eso abre un caso. Corregir una fecha o un número de factura no lo pide,
    // porque exigir explicación donde no hay riesgo enseña a escribirla sin
    // pensar.
    motivo: Option<String>,
) -> Result<String, String> {
    con_almacen(|a| {
        let r = aplicacion::ingresos::actualizar_ingreso(
            aplicacion::ingresos::DatosCorreccionDeFactura {
                id,
                numero_factura,
                cliente_id,
                fecha_emision,
                // El total y el cobro parcial llegan como se escribieron; el céntimo lo deciden esos dígitos.
                monto_total: monto_total.con_divisa(MONEDA_LOCAL),
                porcentaje_retencion,
                cobro_parcial: cobro_parcial.map(|c| c.con_divisa(MONEDA_LOCAL)),
                motivo,
            },
            a,
        )?;
        use aplicacion::ingresos::ResultadoDeCorreccion::*;
        Ok(match r {
            Corregida => "Factura corregida.".to_string(),
            Ajustada { cuenta, ajuste, caso } => format!(
                "Factura corregida. Se ajustó «{}» en DOP {:.2}. Caso {}.",
                cuenta,
                ajuste.unidades(),
                caso
            ),
            SinCuentaDeDeposito { caso } => {
                format!("Factura corregida. No tenía cuenta de depósito que ajustar. Caso {}.", caso)
            }
        })
    })
}

#[tauri::command]
pub fn eliminar_ingreso(id: i64, motivo: String) -> Result<String, String> {
    con_almacen(|a| Ok(aplicacion::ingresos::eliminar_ingreso(id, &motivo, a)?))
}
