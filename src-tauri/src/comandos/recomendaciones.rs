//! Comando de las recomendaciones del historial del patrimonio. **Solo lectura**: una transacción diferida y ninguna
//! escritura.
//!
//! Son reglas **descriptivas y deterministas**, no asesoría financiera: cada recomendación lleva el dato que la dispara
//! (cifras y periodo) y la regla en palabras. Las reglas y sus umbrales están en `dominio::recomendaciones`. Los importes
//! viajan como texto exacto (`1234.56`), nunca como número; los porcentajes, como texto con un decimal (`81.5`).

use chrono::{Local, NaiveDate};
use serde::Serialize;

use super::ayudantes::lee_fotos_y_almacen;
use crate::aplicacion;
use crate::dominio::dinero::Divisa;
use crate::dominio::recomendaciones::{Cifra, Recomendacion, Recomendaciones, ReglaSinDatos, UsdExcluido, ValorCifra};
use crate::ipc;

/// El aviso que acompaña siempre a las recomendaciones.
pub const AVISO_NO_ASESORIA: &str =
    "Reglas descriptivas, no asesoría financiera: muestran qué dicen tus propios datos y qué regla se cumplió; no te dicen qué hacer ni qué comprar.";

/// Una cifra de respaldo de una recomendación.
#[derive(Serialize, Debug)]
pub struct CifraRecomendacion {
    pub etiqueta: String,
    /// `importe` (pesos, texto exacto), `importe_usd` (dólares sin convertir, texto exacto), `porcentaje` (texto con un decimal, sin el signo %), `tasa` o `texto`.
    pub tipo: String,
    pub valor: String,
}

#[derive(Serialize, Debug)]
pub struct RecomendacionPatrimonio {
    /// Estable: la misma situación da el mismo identificador (`regla:clave`).
    pub id: String,
    /// El código de la regla que la disparó.
    pub regla: String,
    /// `atencion` o `info`.
    pub severidad: String,
    pub titulo: String,
    /// Qué dicen los datos, en palabras.
    pub explicacion: String,
    /// La regla que se cumplió, con sus umbrales, en palabras.
    pub regla_texto: String,
    pub cifras: Vec<CifraRecomendacion>,
    /// La pestaña donde se ve el dato (una ruta de la interfaz).
    pub destino: String,
}

/// Una regla que no pudo evaluarse por falta de datos.
#[derive(Serialize, Debug)]
pub struct ReglaSinDatosPatrimonio {
    pub regla: String,
    pub motivo: String,
}

/// Dólares que se dejaron fuera de las reglas, sin convertir.
#[derive(Serialize, Debug)]
pub struct DolaresExcluidos {
    /// `gastos`, `tarjetas` o `cuentas`.
    pub origen: String,
    /// Dólares, texto exacto.
    pub importe: String,
}

#[derive(Serialize, Debug)]
pub struct ResultadoRecomendaciones {
    pub aviso: String,
    /// El último mes completo, del que leen las reglas de gasto.
    pub anio_gastos: i32,
    pub mes_gastos: u32,
    /// El mes en curso, del que leen los presupuestos.
    pub anio_presupuestos: i32,
    pub mes_presupuestos: u32,
    /// Las visibles, en orden: atención primero, a lo sumo `maximo_por_severidad` de cada severidad.
    pub recomendaciones: Vec<RecomendacionPatrimonio>,
    /// Cuántas más hubo y no se muestran.
    pub omitidas: i64,
    pub maximo_por_severidad: i64,
    /// Cuántas reglas tuvieron datos para evaluarse (se activaran o no).
    pub reglas_evaluadas: i64,
    pub sin_datos: Vec<ReglaSinDatosPatrimonio>,
    pub usd_excluido: Vec<DolaresExcluidos>,
}

/// Milésimas con signo como texto de un decimal: `815` → `81.5`.
fn milesimas_a_texto(m: i64) -> String {
    let signo = if m < 0 { "-" } else { "" };
    let a = m.unsigned_abs();
    format!("{signo}{}.{}", a / 10, a % 10)
}

fn a_cifra(c: &Cifra) -> CifraRecomendacion {
    let (tipo, valor) = match &c.valor {
        ValorCifra::Importe(d) => (if d.divisa() == Divisa::Usd { "importe_usd" } else { "importe" }, ipc::importe_a_texto(*d)),
        ValorCifra::Milesimas(m) => ("porcentaje", milesimas_a_texto(*m)),
        ValorCifra::Tasa(t) => ("tasa", ipc::tasa_a_texto(*t)),
        ValorCifra::Texto(t) => ("texto", t.clone()),
    };
    CifraRecomendacion { etiqueta: c.etiqueta.clone(), tipo: tipo.to_string(), valor }
}

fn a_recomendacion(r: &Recomendacion) -> RecomendacionPatrimonio {
    RecomendacionPatrimonio {
        id: r.id.clone(),
        regla: r.regla.codigo().to_string(),
        severidad: r.severidad.codigo().to_string(),
        titulo: r.titulo.clone(),
        explicacion: r.explicacion.clone(),
        regla_texto: r.regla_texto.clone(),
        cifras: r.cifras.iter().map(a_cifra).collect(),
        destino: r.destino.ruta().to_string(),
    }
}

fn a_sin_datos(s: &ReglaSinDatos) -> ReglaSinDatosPatrimonio {
    ReglaSinDatosPatrimonio { regla: s.regla.codigo().to_string(), motivo: s.motivo.clone() }
}

fn a_dolares(u: &UsdExcluido) -> DolaresExcluidos {
    DolaresExcluidos { origen: u.origen.to_string(), importe: ipc::importe_a_texto(u.importe) }
}

fn a_dto(r: &Recomendaciones) -> ResultadoRecomendaciones {
    ResultadoRecomendaciones {
        aviso: AVISO_NO_ASESORIA.to_string(),
        anio_gastos: r.mes_gastos.0,
        mes_gastos: r.mes_gastos.1,
        anio_presupuestos: r.mes_presupuestos.0,
        mes_presupuestos: r.mes_presupuestos.1,
        recomendaciones: r.visibles.iter().map(a_recomendacion).collect(),
        omitidas: r.omitidas as i64,
        maximo_por_severidad: crate::dominio::recomendaciones::MAXIMO_POR_SEVERIDAD as i64,
        reglas_evaluadas: r.reglas_evaluadas as i64,
        sin_datos: r.sin_datos.iter().map(a_sin_datos).collect(),
        usd_excluido: r.usd_excluido.iter().map(a_dolares).collect(),
    }
}

/// Las recomendaciones para el historial del patrimonio, con la fecha de hoy. **Solo lectura.** Las reglas son
/// descriptivas (ver `dominio::recomendaciones`); con datos insuficientes lo dicen en `sin_datos` en vez de inventar.
#[tauri::command]
pub fn obtener_recomendaciones() -> Result<ResultadoRecomendaciones, String> {
    recomendaciones_en(Local::now().date_naive())
}

/// El cuerpo del comando con «hoy» inyectado (así se prueba un mes concreto sin depender del reloj).
pub(crate) fn recomendaciones_en(hoy: NaiveDate) -> Result<ResultadoRecomendaciones, String> {
    lee_fotos_y_almacen(|fotos, almacen| Ok(a_dto(&aplicacion::recomendaciones::obtener_recomendaciones(hoy, fotos, almacen)?)))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_milesimas_se_escriben_con_un_decimal_y_signo() {
        assert_eq!(milesimas_a_texto(815), "81.5");
        assert_eq!(milesimas_a_texto(1040), "104.0");
        assert_eq!(milesimas_a_texto(0), "0.0");
        assert_eq!(milesimas_a_texto(5), "0.5");
        assert_eq!(milesimas_a_texto(-25), "-2.5");
    }

    #[test]
    fn el_aviso_dice_que_no_es_asesoria() {
        assert!(AVISO_NO_ASESORIA.contains("no asesoría financiera"));
    }
}
