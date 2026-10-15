//! Comandos de gastos: alta, listado, eliminación y liquidación de consumos en divisa.

use serde::{Deserialize, Serialize};

use super::ayudantes::{con_almacen, lee_almacen};
use crate::aplicacion;
use crate::aplicacion::liquidar_gasto::liquidar_gasto;
use crate::aplicacion::registrar_gasto::{registrar_gasto, DatosGasto};
use crate::dominio::dinero::{Divisa, TasaCambio};
use crate::dominio::gasto::MetodoPago;
use crate::dominio::tarjeta::MONEDA_LOCAL;
use crate::ipc;

#[derive(Serialize, Deserialize, Debug)]
pub struct Gasto {
    pub id: i64,
    pub fecha: String,
    pub monto: f64,
    pub divisa: String,
    pub descripcion: String,
    pub categoria_id: i64,
    pub categoria_nombre: String,
    pub metodo_pago: String,
    pub costo_adicional: f64,
    pub tarjeta_id: Option<i64>,
    pub cuenta_ahorro_id: Option<i64>,
    // Conversión de divisa: NULL cuando no aplica.
    pub estado_conversion: Option<String>,
    pub monto_liquidado: Option<f64>,
    pub tasa_conversion: Option<f64>,
}

// --- COMANDOS: GASTOS ---
#[tauri::command]
pub fn obtener_gastos() -> Result<Vec<Gasto>, String> {
    lee_almacen(|a| {
        Ok(aplicacion::gastos::listar_gastos(a)?
            .into_iter()
            .map(|g| Gasto {
                id: g.id,
                fecha: g.fecha,
                monto: g.monto,
                divisa: g.divisa,
                descripcion: g.descripcion,
                categoria_id: g.categoria_id,
                categoria_nombre: g.categoria_nombre,
                metodo_pago: g.metodo_pago,
                costo_adicional: g.costo_adicional,
                tarjeta_id: g.tarjeta_id,
                cuenta_ahorro_id: g.cuenta_ahorro_id,
                estado_conversion: g.estado_conversion,
                monto_liquidado: g.monto_liquidado,
                tasa_conversion: g.tasa_conversion,
            })
            .collect())
    })
}

#[derive(Deserialize)]
pub struct GastoInput {
    pub fecha: String,
    pub monto: ipc::ImporteDecimal,
    pub divisa: String,
    pub descripcion: String,
    pub categoria_id: i64,
    pub metodo_pago: String,
    pub es_lbtr: bool,
    pub tarjeta_id: Option<i64>,
    pub cuenta_ahorro_id: Option<i64>,
    #[serde(default)]
    pub tasa_cambio: Option<f64>,
}

#[tauri::command]
pub fn crear_gasto(input: GastoInput) -> Result<i64, String> {
    // El comando queda reducido a traducción: interpreta la entrada, abre la
    // transacción, delega en el caso de uso y confirma. Ninguna regla vive ya
    // aquí.
    // Se conserva la interpretación vigente de la divisa: la columna
    // gastos.divisa no tiene CHECK y el código solo distingue "USD".
    let divisa = if input.divisa == "USD" { Divisa::Usd } else { Divisa::Dop };
    let metodo = MetodoPago::desde_codigo(&input.metodo_pago);

    let datos = DatosGasto {
        fecha: input.fecha,
        // El importe llega como se escribió; se casa con la divisa declarada del gasto.
        monto: input.monto.con_divisa(divisa),
        descripcion: input.descripcion,
        categoria_id: input.categoria_id,
        metodo,
        metodo_texto: input.metodo_pago,
        es_lbtr: input.es_lbtr,
        tarjeta_id: input.tarjeta_id,
        cuenta_ahorro_id: input.cuenta_ahorro_id,
        // Una tasa de cero o ausente significa "sin conversión"; el caso de
        // uso la exigirá solo si las divisas realmente difieren.
        tasa_cambio: match input.tasa_cambio {
            Some(v) if v > 0.0 => Some(TasaCambio::nueva(v)?),
            _ => None,
        },
    };

    con_almacen(|a| Ok(registrar_gasto(datos, a)?))
}

/// Cierra un consumo pendiente con el importe que el emisor cargó en moneda
/// local. Devuelve la tasa que se dedujo, para poder mostrarla.

#[tauri::command]
pub fn liquidar_consumo_pendiente(id: i64, monto_liquidado: ipc::ImporteDecimal) -> Result<f64, String> {
    con_almacen(|a| {
        // El importe llega como se escribió: el céntimo (y con él la tasa que se deduce) sale de esos dígitos.
        let importe = monto_liquidado.con_divisa(MONEDA_LOCAL);
        Ok(liquidar_gasto(id, importe, a)?.tasa().valor())
    })
}

#[tauri::command]
pub fn eliminar_gasto(id: i64, motivo: String) -> Result<String, String> {
    con_almacen(|a| Ok(aplicacion::gastos::eliminar_gasto(id, &motivo, a)?))
}
