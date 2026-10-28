//! Comandos de las fotos mensuales del patrimonio. **Todos los importes viajan como texto exacto** (`1234.56`).
//!
//! * `tomar_foto_patrimonio`: escribe (manual, o al guardar el capital).
//! * `asegurar_foto_del_mes`: la llama la interfaz al arrancar; solo escribe si falta la del mes.
//! * `obtener_fotos_patrimonio`: lectura.
//!
//! Las funciones `*_en` reciben la fecha y el capital ya leído: así se ejercitan de punta a punta sobre una base
//! temporal con el reloj que la prueba quiera, y los comandos reales solo ponen «hoy» y el capital del disco.

use super::ayudantes::{con_fotos, lee_fotos};
use crate::aplicacion::fotos_patrimonio::{self as caso, FotoCompleta};
use crate::dominio::foto_patrimonio::Origen;
use crate::{db_nosql, ipc};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Debug)]
pub struct PartidaFoto {
    pub rubro: String,
    pub referencia: String,
    pub divisa: String,
    pub importe: String,
    pub importe_dop: String,
}

/// Una foto del patrimonio con sus partidas.
#[derive(Serialize, Deserialize, Debug)]
pub struct FotoPatrimonio {
    pub id: i64,
    pub anio: i32,
    pub mes: u32,
    pub fecha: String,
    pub origen: String,
    pub sellada: bool,
    pub tasa_usd_dop: String,
    pub activos: String,
    pub pasivos: String,
    pub patrimonio: String,
    pub liquido: String,
    pub calidad: String,
    pub partidas: Vec<PartidaFoto>,
}

/// Lo que pasó al asegurar la foto del mes: `creada` es `false` si ya existía (y entonces no se escribió nada).
#[derive(Serialize, Deserialize, Debug)]
pub struct FotoAsegurada {
    pub creada: bool,
    pub foto: Option<FotoPatrimonio>,
}

fn a_dto(c: FotoCompleta) -> FotoPatrimonio {
    let texto = ipc::importe_a_texto;
    let f = c.foto;
    FotoPatrimonio {
        id: f.id,
        anio: f.anio,
        mes: f.mes,
        fecha: f.fecha,
        origen: f.origen,
        sellada: f.sellada,
        tasa_usd_dop: ipc::tasa_a_texto(f.tasa_usd_dop),
        activos: texto(f.activos),
        pasivos: texto(f.pasivos),
        patrimonio: texto(f.patrimonio),
        liquido: texto(f.liquido),
        calidad: f.calidad,
        partidas: c
            .partidas
            .into_iter()
            .map(|p| PartidaFoto {
                rubro: p.rubro,
                referencia: p.referencia,
                divisa: p.divisa.codigo().to_string(),
                importe: texto(p.importe),
                importe_dop: texto(p.importe_dop),
            })
            .collect(),
    }
}

fn hoy() -> NaiveDate {
    chrono::Local::now().date_naive()
}

/// El capital como está en disco; `Err` si existe y no se puede leer (la foto lo cuenta como cero, con aviso).
fn capital_del_disco() -> Result<Value, String> {
    db_nosql::leer_coleccion("capital")
}

pub(crate) fn tomar_en(
    origen: &str,
    tasa_usd_dop: Option<&str>,
    hoy: NaiveDate,
    capital: &Result<Value, String>,
) -> Result<FotoPatrimonio, String> {
    let origen = Origen::desde_peticion(origen).ok_or_else(|| format!("Origen de foto no válido: «{origen}»."))?;
    let tasa = caso::tasa_de_peticion(tasa_usd_dop).map_err(String::from)?;
    con_fotos(|f| caso::tomar_foto(f, capital, hoy, origen, tasa).map(a_dto).map_err(String::from))
}

pub(crate) fn asegurar_en(
    tasa_usd_dop: Option<&str>,
    hoy: NaiveDate,
    capital: &Result<Value, String>,
) -> Result<FotoAsegurada, String> {
    let tasa = caso::tasa_de_peticion(tasa_usd_dop).map_err(String::from)?;
    con_fotos(|f| {
        let a = caso::asegurar_foto_del_mes(f, capital, hoy, tasa).map_err(String::from)?;
        Ok(FotoAsegurada { creada: a.creada, foto: a.foto.map(a_dto) })
    })
}

pub(crate) fn leer_fotos() -> Result<Vec<FotoPatrimonio>, String> {
    lee_fotos(|f| Ok(caso::listar_fotos(f).map_err(String::from)?.into_iter().map(a_dto).collect()))
}

/// Toma la foto del mes a petición del titular (`origen`: `manual`) o tras guardar el capital (`capital`).
/// La tasa USD→DOP es opcional: sin ella se usa la sugerida (60.0).
#[tauri::command]
pub fn tomar_foto_patrimonio(origen: String, tasa_usd_dop: Option<String>) -> Result<FotoPatrimonio, String> {
    tomar_en(&origen, tasa_usd_dop.as_deref(), hoy(), &capital_del_disco())
}

/// La llama la interfaz al arrancar: toma la foto del mes solo si todavía no existe.
#[tauri::command]
pub fn asegurar_foto_del_mes(tasa_usd_dop: Option<String>) -> Result<FotoAsegurada, String> {
    asegurar_en(tasa_usd_dop.as_deref(), hoy(), &capital_del_disco())
}

/// Las fotos guardadas, de la más antigua a la más reciente.
#[tauri::command]
pub fn obtener_fotos_patrimonio() -> Result<Vec<FotoPatrimonio>, String> {
    leer_fotos()
}

/// Tras guardar el capital: toma la foto con el capital recién guardado. **Nunca falla el guardado**: si la foto no se
/// pudo tomar, devuelve el aviso para mostrarlo.
pub(crate) fn foto_tras_guardar_capital(capital: &Value) -> Option<String> {
    tomar_en("capital", None, hoy(), &Ok(capital.clone()))
        .err()
        .map(|e| format!("El capital se guardó, pero no se pudo tomar la foto del patrimonio: {e}"))
}
