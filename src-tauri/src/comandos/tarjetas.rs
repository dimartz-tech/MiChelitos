//! Comandos de tarjetas: listado, alta y límites.

use chrono::{Datelike, Local};
use serde::{Deserialize, Serialize};

use super::ayudantes::{con_tarjetas, lee_tarjetas};
use crate::aplicacion;
use crate::dominio::dinero::Divisa;
use crate::ipc;

#[derive(Serialize, Deserialize, Debug)]
pub struct Tarjeta {
    pub id: i64,
    pub entidad: String,
    pub nombre_tarjeta: String,
    pub limite_pesos: f64,
    pub limite_dolares: f64,
    pub limite_ajustado_pesos: Option<f64>,
    pub limite_ajustado_dolares: Option<f64>,
    pub limite_sobregiro_pesos: f64,
    pub limite_sobregiro_dolares: f64,
    pub balance_pesos: f64,
    pub balance_dolares: f64,
    pub balance_corte_pesos: f64,
    pub balance_corte_dolares: f64,
    pub fecha_corte: i32,
    pub fecha_limite_pago: i32,
    pub politica_liquidacion: String,
    // Enriquecidos
    pub limite_efectivo_pesos: f64,
    pub limite_efectivo_dolares: f64,
    pub disponible_pesos: f64,
    pub disponible_dolares: f64,
    pub alerta_corte: bool,
    pub alerta_pago: bool,
    pub dias_corte_msg: String,
    pub dias_pago_msg: String,
}

#[tauri::command]
pub fn obtener_tarjetas() -> Result<Vec<Tarjeta>, String> {
    let dia_actual = Local::now().day() as i32;
    lee_tarjetas(|a| {
        Ok(aplicacion::tarjetas::listar_tarjetas(dia_actual, a)?
            .into_iter()
            .map(|t| Tarjeta {
                id: t.leida.id,
                entidad: t.leida.entidad,
                nombre_tarjeta: t.leida.nombre_tarjeta,
                limite_pesos: t.leida.limite_pesos,
                limite_dolares: t.leida.limite_dolares,
                limite_ajustado_pesos: t.leida.limite_ajustado_pesos,
                limite_ajustado_dolares: t.leida.limite_ajustado_dolares,
                limite_sobregiro_pesos: t.leida.limite_sobregiro_pesos,
                limite_sobregiro_dolares: t.leida.limite_sobregiro_dolares,
                balance_pesos: t.leida.balance_pesos,
                balance_dolares: t.leida.balance_dolares,
                balance_corte_pesos: t.leida.balance_corte_pesos,
                balance_corte_dolares: t.leida.balance_corte_dolares,
                fecha_corte: t.leida.fecha_corte,
                fecha_limite_pago: t.leida.fecha_limite_pago,
                politica_liquidacion: t.politica_liquidacion,
                limite_efectivo_pesos: t.limite_efectivo_pesos,
                limite_efectivo_dolares: t.limite_efectivo_dolares,
                disponible_pesos: t.disponible_pesos,
                disponible_dolares: t.disponible_dolares,
                alerta_corte: t.alerta_corte,
                alerta_pago: t.alerta_pago,
                dias_corte_msg: t.dias_corte_msg,
                dias_pago_msg: t.dias_pago_msg,
            })
            .collect())
    })
}

/// Una tarjeta desactivada: solo lo necesario para reconocerla y reactivarla. Sin balances.
#[derive(Serialize, Deserialize, Debug)]
pub struct TarjetaInactiva {
    pub id: i64,
    pub entidad: String,
    pub nombre_tarjeta: String,
}

/// Las tarjetas desactivadas, aparte de las activas.
#[tauri::command]
pub fn obtener_tarjetas_inactivas() -> Result<Vec<TarjetaInactiva>, String> {
    lee_tarjetas(|a| {
        Ok(aplicacion::tarjetas::listar_inactivas(a)?
            .into_iter()
            .map(|t| TarjetaInactiva { id: t.id, entidad: t.entidad, nombre_tarjeta: t.nombre_tarjeta })
            .collect())
    })
}

#[tauri::command]
pub fn crear_tarjeta(
    entidad: String,
    nombre: String,
    limite_pesos: ipc::ImporteDecimal,
    limite_dolares: ipc::ImporteDecimal,
    sobregiro_pesos: ipc::ImporteDecimal,
    sobregiro_dolares: ipc::ImporteDecimal,
    balance_pesos: ipc::ImporteDecimal,
    balance_dolares: ipc::ImporteDecimal,
    balance_corte_pesos: ipc::ImporteDecimal,
    balance_corte_dolares: ipc::ImporteDecimal,
    corte: i32,
    pago: i32
) -> Result<i64, String> {
    con_tarjetas(|a| {
        // Cada importe llega como se escribió: el céntimo lo deciden esos dígitos, no un número ya redondeado.
        Ok(aplicacion::tarjetas::crear_tarjeta(
            aplicacion::tarjetas::DatosTarjetaNueva {
                entidad,
                nombre_tarjeta: nombre,
                limite_pesos: limite_pesos.con_divisa(Divisa::Dop),
                limite_dolares: limite_dolares.con_divisa(Divisa::Usd),
                sobregiro_pesos: sobregiro_pesos.con_divisa(Divisa::Dop),
                sobregiro_dolares: sobregiro_dolares.con_divisa(Divisa::Usd),
                balance_pesos: balance_pesos.con_divisa(Divisa::Dop),
                balance_dolares: balance_dolares.con_divisa(Divisa::Usd),
                balance_corte_pesos: balance_corte_pesos.con_divisa(Divisa::Dop),
                balance_corte_dolares: balance_corte_dolares.con_divisa(Divisa::Usd),
                fecha_corte: corte,
                fecha_limite_pago: pago,
            },
            a,
        )?)
    })
}

#[tauri::command]
pub fn actualizar_limites_tarjeta(
    id: i64,
    limite_pesos: ipc::ImporteDecimal,
    limite_dolares: ipc::ImporteDecimal,
    sobregiro_pesos: ipc::ImporteDecimal,
    sobregiro_dolares: ipc::ImporteDecimal,
    balance_corte_pesos: ipc::ImporteDecimal,
    balance_corte_dolares: ipc::ImporteDecimal,
    // `None` es «sin ajuste»; cero es un tope deliberado.
    limite_ajustado_pesos: Option<ipc::ImporteDecimal>,
    limite_ajustado_dolares: Option<ipc::ImporteDecimal>,
    politica_liquidacion: Option<String>
) -> Result<(), String> {
    con_tarjetas(|a| {
        Ok(aplicacion::tarjetas::actualizar_limites(
            aplicacion::tarjetas::DatosLimites {
                id,
                limite_pesos: limite_pesos.con_divisa(Divisa::Dop),
                limite_dolares: limite_dolares.con_divisa(Divisa::Usd),
                sobregiro_pesos: sobregiro_pesos.con_divisa(Divisa::Dop),
                sobregiro_dolares: sobregiro_dolares.con_divisa(Divisa::Usd),
                balance_corte_pesos: balance_corte_pesos.con_divisa(Divisa::Dop),
                balance_corte_dolares: balance_corte_dolares.con_divisa(Divisa::Usd),
                limite_ajustado_pesos: limite_ajustado_pesos.map(|i| i.con_divisa(Divisa::Dop)),
                limite_ajustado_dolares: limite_ajustado_dolares.map(|i| i.con_divisa(Divisa::Usd)),
                politica_liquidacion,
            },
            a,
        )?)
    })
}
