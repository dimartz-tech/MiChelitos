//! Comandos del cierre (desactivación) y la reactivación de tarjetas.

use serde::{Deserialize, Serialize};

use super::ayudantes::{con_almacen, hoy_dd_mm_aaaa, lee_almacen};
use crate::aplicacion;

/// Una suscripción que hay que trasladar al cerrar la tarjeta, con lo que se sugiere para ella.
#[derive(Serialize, Deserialize, Debug)]
pub struct LineaDeTraslado {
    pub suscripcion_id: i64,
    pub plataforma: String,
    pub monto: f64,
    pub divisa: String,
    pub frecuencia: String,
    pub fecha_proximo_cobro: Option<String>,
    pub destino_sugerido: Option<i64>,
    /// Los destinos con cupo, del mejor al peor.
    pub elegibles: Vec<i64>,
    pub motivo: String,
}

/// Una tarjeta que puede recibir suscripciones, con lo que le queda de cupo en cada divisa.
#[derive(Serialize, Deserialize, Debug)]
pub struct DestinoDeCierre {
    pub id: i64,
    pub nombre: String,
    pub conserva_dolares: bool,
    pub disponible_pesos: f64,
    pub disponible_dolares: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VistaPreviaDeCierre {
    pub tarjeta_id: i64,
    pub nombre: String,
    pub activa: bool,
    pub balance_pesos: f64,
    pub balance_dolares: f64,
    /// Un mensaje por balance distinto de cero. Con alguno no se puede confirmar el cierre.
    pub bloqueos: Vec<String>,
    pub lineas: Vec<LineaDeTraslado>,
    pub destinos: Vec<DestinoDeCierre>,
}

/// Qué se le muestra al usuario antes de confirmar «Desactivar tarjeta». Solo lectura.
#[tauri::command]
pub fn previsualizar_cierre_tarjeta(id: i64) -> Result<VistaPreviaDeCierre, String> {
    lee_almacen(|a| {
        let v = aplicacion::cierre_de_tarjetas::previsualizar_cierre(id, a)?;
        Ok(VistaPreviaDeCierre {
            tarjeta_id: v.tarjeta_id,
            nombre: v.nombre,
            activa: v.activa,
            balance_pesos: v.balance_dop.unidades(),
            balance_dolares: v.balance_usd.unidades(),
            bloqueos: v.bloqueos,
            lineas: v
                .lineas
                .into_iter()
                .map(|l| LineaDeTraslado {
                    suscripcion_id: l.suscripcion.id,
                    plataforma: l.suscripcion.plataforma,
                    monto: l.suscripcion.monto,
                    divisa: l.suscripcion.divisa,
                    frecuencia: l.suscripcion.frecuencia,
                    fecha_proximo_cobro: l.suscripcion.fecha_proximo_cobro,
                    destino_sugerido: l.sugerencia.destino_id,
                    elegibles: l.sugerencia.elegibles,
                    motivo: l.sugerencia.motivo,
                })
                .collect(),
            destinos: v
                .destinos
                .into_iter()
                .map(|d| DestinoDeCierre {
                    id: d.id,
                    nombre: d.nombre,
                    conserva_dolares: d.conserva_dolares,
                    disponible_pesos: d.disponible_dop.unidades(),
                    disponible_dolares: d.disponible_usd.unidades(),
                })
                .collect(),
        })
    })
}

/// La tarjeta destino que el usuario eligió para una suscripción.
#[derive(Serialize, Deserialize, Debug)]
pub struct TrasladoElegido {
    pub suscripcion_id: i64,
    pub destino_id: i64,
}

/// Desactiva la tarjeta trasladando sus suscripciones. **Todo o nada**, en una sola transacción de escritura.
#[tauri::command]
pub fn cerrar_tarjeta(id: i64, traslados: Vec<TrasladoElegido>) -> Result<String, String> {
    let elegidos: Vec<(i64, i64)> = traslados.iter().map(|t| (t.suscripcion_id, t.destino_id)).collect();
    let hoy = hoy_dd_mm_aaaa();
    con_almacen(|a| {
        let r = aplicacion::cierre_de_tarjetas::cerrar_tarjeta(id, &elegidos, &hoy, a)?;
        Ok(format!(
            "Tarjeta {} desactivada. {} suscripción(es) trasladada(s).",
            r.nombre, r.trasladadas
        ))
    })
}

/// Reactiva una tarjeta desactivada. Ninguna suscripción vuelve sola a ella.
#[tauri::command]
pub fn reactivar_tarjeta(id: i64) -> Result<(), String> {
    con_almacen(|a| Ok(aplicacion::cierre_de_tarjetas::reactivar_tarjeta(id, a)?))
}
