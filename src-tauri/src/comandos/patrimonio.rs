//! Comando del patrimonio: **solo lectura**. Sus dos cifras las compone `dominio::patrimonio`.

use serde::{Deserialize, Serialize};

use super::ayudantes::lee_almacen;
use crate::dominio::patrimonio::{tasa_estimada, Foto};
use crate::{aplicacion, db_nosql, ipc};

#[derive(Serialize, Deserialize, Debug)]
pub struct PartidaPatrimonio {
    /// `activo` o `pasivo`.
    pub tipo: String,
    /// `capital`, `cuenta`, `efectivo`, `tarjeta` o `prestamo`.
    pub rubro: String,
    pub nombre: String,
    pub divisa: String,
    pub importe: String,
    pub importe_dop: String,
}

/// El patrimonio con sus dos cifras. Todos los importes son texto exacto (`1234.56`), en pesos salvo los marcados `_usd`.
#[derive(Serialize, Deserialize, Debug)]
pub struct Patrimonio {
    pub tasa_usd_a_dop: String,
    /// `true`: la tasa es una estimación fija, no una cotización.
    pub tasa_estimada: bool,
    pub capital: String,
    pub liquidez_dop: String,
    pub liquidez_usd: String,
    pub liquidez_usd_en_dop: String,
    pub efectivo_dop: String,
    pub efectivo_usd: String,
    pub tarjetas_dop: String,
    pub tarjetas_usd: String,
    pub tarjetas_usd_en_dop: String,
    pub prestamos: String,
    pub total_pasivos: String,
    /// Capital menos tarjetas y préstamos: la cifra vigente de Resumen.
    pub neto_actual: String,
    /// `neto_actual` más cuentas de ahorro y efectivo.
    pub neto_con_liquidez: String,
    pub partidas: Vec<PartidaPatrimonio>,
}

fn a_dto(f: Foto) -> Patrimonio {
    let t = ipc::importe_a_texto;
    Patrimonio {
        tasa_usd_a_dop: ipc::tasa_a_texto(f.tasa),
        tasa_estimada: true,
        capital: t(f.capital),
        liquidez_dop: t(f.liquidez_dop),
        liquidez_usd: t(f.liquidez_usd),
        liquidez_usd_en_dop: t(f.liquidez_usd_en_dop),
        efectivo_dop: t(f.efectivo_dop),
        efectivo_usd: t(f.efectivo_usd),
        tarjetas_dop: t(f.tarjetas_dop),
        tarjetas_usd: t(f.tarjetas_usd),
        tarjetas_usd_en_dop: t(f.tarjetas_usd_en_dop),
        prestamos: t(f.prestamos),
        total_pasivos: t(f.total_pasivos),
        neto_actual: t(f.neto_actual),
        neto_con_liquidez: t(f.neto_con_liquidez),
        partidas: f
            .partidas
            .into_iter()
            .map(|p| PartidaPatrimonio {
                tipo: p.tipo.codigo().to_string(),
                rubro: p.rubro.to_string(),
                nombre: p.nombre,
                divisa: p.original.divisa().codigo().to_string(),
                importe: t(p.original),
                importe_dop: t(p.en_dop),
            })
            .collect(),
    }
}

/// El patrimonio: capital − deudas, y esa cifra más cuentas y efectivo. **No escribe nada.**
#[tauri::command]
pub fn obtener_patrimonio() -> Result<Patrimonio, String> {
    // El capital se lee y suma como en `obtener_capital`; si el archivo existe pero está dañado, es un error, no un cero.
    let capital = db_nosql::leer_coleccion("capital")?;
    let totales = aplicacion::guardar_capital::totales_de(&capital);
    lee_almacen(|a| Ok(a_dto(aplicacion::patrimonio::obtener_patrimonio(&totales, tasa_estimada(), a)?)))
}
