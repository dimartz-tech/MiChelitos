//! Comandos de abonos (pagos) a tarjeta.

use serde::{Deserialize, Serialize};

use super::ayudantes::{con_almacen, lee_almacen};
use crate::aplicacion;
use crate::aplicacion::registrar_pago_tarjeta::DatosPago;
use crate::dominio::dinero::{Divisa, TasaCambio};
use crate::ipc;

#[tauri::command]
pub fn registrar_pago_tarjeta(
    id: i64,
    fecha: String,
    monto: ipc::ImporteDecimal,
    divisa: String,
    cuenta_ahorro_id: Option<i64>,
    tasa_cambio: f64
) -> Result<(), String> {
    con_almacen(|a| {
        Ok(aplicacion::abonos::registrar_abono(
            DatosPago {
                tarjeta_id: id,
                fecha,
                // El céntimo lo deciden los dígitos escritos; la divisa es la declarada por el abono.
                monto: monto.con_divisa(Divisa::desde_codigo(&divisa)?),
                cuenta_ahorro_id,
                // Una tasa de cero es como la interfaz dice «no aplica».
                tasa_cambio: if tasa_cambio > 0.0 {
                    Some(TasaCambio::nueva(tasa_cambio)?)
                } else {
                    None
                },
            },
            a,
        )?)
    })
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AbonoTarjeta {
    pub id: i64,
    pub fecha_pago: String,
    pub monto_pagado: f64,
    pub divisa: String,
    pub cuenta_ahorro_id: Option<i64>,
    pub cuenta_nombre: Option<String>,
    pub tasa_cambio: Option<f64>,
}

/// Abonos registrados a una tarjeta, del más reciente al más antiguo.
#[tauri::command]
pub fn obtener_abonos_tarjeta(tarjeta_id: i64) -> Result<Vec<AbonoTarjeta>, String> {
    lee_almacen(|a| {
        Ok(aplicacion::abonos::listar_abonos(tarjeta_id, a)?
            .into_iter()
            .map(|p| AbonoTarjeta {
                id: p.id,
                fecha_pago: p.fecha_pago,
                monto_pagado: p.monto_pagado,
                divisa: p.divisa,
                cuenta_ahorro_id: p.cuenta_ahorro_id,
                cuenta_nombre: p.cuenta_nombre,
                tasa_cambio: p.tasa_cambio,
            })
            .collect())
    })
}

/// Deshace un abono a tarjeta.
///
/// Traducción pura: la reversión vive en el caso de uso, igual que la de los
/// gastos. Devuelve lo que se deshizo para que la interfaz pueda decirlo en
/// vez de limitarse a confirmar que algo pasó.
#[tauri::command]
pub fn revertir_abono_tarjeta(id: i64, motivo: String) -> Result<String, String> {
    con_almacen(|a| {
        let r = aplicacion::abonos::revertir_abono(id, &motivo, a)?;
        let resumen = match r.revertido.devuelto_a_la_cuenta {
            Some(d) => format!(
                "Se repusieron {} {:.2} a la deuda y volvieron {} {:.2} a la cuenta.",
                r.revertido.deuda_restituida.divisa().codigo(),
                r.revertido.deuda_restituida.unidades(),
                d.divisa().codigo(),
                d.unidades()
            ),
            None => format!(
                "Se repusieron {} {:.2} a la deuda. El abono no tenía cuenta asociada.",
                r.revertido.deuda_restituida.divisa().codigo(),
                r.revertido.deuda_restituida.unidades()
            ),
        };
        Ok(format!("{} Caso {}.", resumen, r.caso))
    })
}
