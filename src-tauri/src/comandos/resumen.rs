//! Comando del resumen de gastos por categoría y mes. **Solo lectura**: abre una transacción diferida y no escribe.

use chrono::{Local, NaiveDate};
use serde::Serialize;

use super::ayudantes::lee_almacen;
use crate::aplicacion;
use crate::aplicacion::resumen_gastos::DatosDeResumen;
use crate::dominio::dinero::{Dinero, TasaCambio};
use crate::dominio::resumen_gastos::{Medidas, ResumenMensual, Tendencia, ID_CARGOS_DERIVADOS};
use crate::ipc;

/// Las cifras de una fila o del total. Los importes viajan como texto exacto (`1234.56`), nunca como número.
#[derive(Serialize, Debug)]
pub struct MedidasResumen {
    /// En pesos; los dólares sin liquidar, convertidos con la tasa de referencia (un estimado).
    pub importe: String,
    /// Parte del total del mes en milésimas (1000 = 100 %).
    pub porcentaje_milesimas: i64,
    pub anterior: String,
    /// Puntos básicos (100 = 1 %); `null` si el mes anterior no es positivo.
    pub variacion_anterior_pb: Option<i64>,
    /// Solo en un mes parcial: el mes anterior hasta el mismo día.
    pub anterior_mismo_dia: Option<String>,
    pub variacion_mismo_dia_pb: Option<i64>,
    /// Promedio de los meses previos completos; `null` si no hay ninguno.
    pub promedio: Option<String>,
    pub variacion_promedio_pb: Option<i64>,
    /// Los dólares del mes sin convertir.
    pub usd_aparte: String,
    pub n: i64,
}

/// Un mes de la serie de tendencia.
#[derive(Serialize, Debug)]
pub struct PuntoTendenciaResumen {
    pub anio: i32,
    pub mes: u32,
    /// En pesos, texto exacto. `null`: el mes es anterior al primer dato del historial (ausente, no cero).
    pub importe: Option<String>,
    /// El mes en curso (su cierre aún no ha llegado). Nunca entra en la dirección.
    pub parcial: bool,
}

/// Los 6 meses que terminan en el mes pedido (del más antiguo al pedido) y su dirección.
#[derive(Serialize, Debug)]
pub struct TendenciaResumen {
    pub meses: Vec<PuntoTendenciaResumen>,
    /// `sube`, `baja`, `estable` o `datos_insuficientes` (regla en `dominio::resumen_gastos`).
    pub direccion: String,
    /// Solo con `direccion == "estable"`: `dentro_del_umbral` (el cambio no pasa del 10 %) o `poco_cambio` (pasa del
    /// 10 % pero la diferencia es menor de DOP 500.00). `null` en cualquier otra dirección.
    pub motivo_estable: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct FilaResumenGasto {
    /// `id_cargos_derivados` para la fila sintética; en otro caso, el de la categoría.
    pub categoria_id: i64,
    pub categoria_nombre: String,
    pub derivado: bool,
    pub medidas: MedidasResumen,
    pub tendencia: TendenciaResumen,
}

#[derive(Serialize, Debug)]
pub struct MesDisponible {
    pub anio: i32,
    pub mes: u32,
}

#[derive(Serialize, Debug)]
pub struct ResumenGastos {
    pub anio: i32,
    pub mes: u32,
    /// El mes pedido es el mes en curso.
    pub parcial: bool,
    pub hasta_dia: Option<u32>,
    /// La tasa de referencia usada (DOP por 1 USD), para rotular los importes convertidos como «estimado».
    pub tasa_usd: String,
    /// Con cuántos meses previos se promedió realmente.
    pub meses_promedio: u32,
    pub id_cargos_derivados: i64,
    pub filas: Vec<FilaResumenGasto>,
    /// La suma de las filas.
    pub totales: MedidasResumen,
    /// La tendencia del total: la suma, mes a mes, de la de las filas.
    pub tendencia_total: TendenciaResumen,
    /// Los meses con algún gasto, del más reciente al más antiguo.
    pub meses_disponibles: Vec<MesDisponible>,
}

/// El resumen del mes (`anio`, `mes`) con los `n_promedio` meses previos completos como referencia.
///
/// `tasa_usd` es la tasa de referencia para los dólares sin liquidar; `excluir_categorias` son identificadores de
/// categoría que no entran (la fila de cargos derivados se excluye con su `id_cargos_derivados`). **Solo lectura.**
#[tauri::command]
pub fn obtener_resumen_gastos(
    anio: i32,
    mes: u32,
    n_promedio: u32,
    tasa_usd: f64,
    excluir_categorias: Vec<i64>,
) -> Result<ResumenGastos, String> {
    resumen_en(Local::now().date_naive(), anio, mes, n_promedio, tasa_usd, excluir_categorias)
}

/// El cuerpo del comando con «hoy» inyectado (así se prueba un mes parcial sin depender del reloj).
pub(crate) fn resumen_en(
    hoy: NaiveDate,
    anio: i32,
    mes: u32,
    n_promedio: u32,
    tasa_usd: f64,
    excluir_categorias: Vec<i64>,
) -> Result<ResumenGastos, String> {
    let tasa = TasaCambio::nueva(tasa_usd).map_err(|e| e.to_string())?;
    let datos = DatosDeResumen { mes: (anio, mes), hoy, n_promedio, tasa_usd: tasa, excluir_categorias };
    lee_almacen(|a| Ok(a_dto(aplicacion::resumen_gastos::obtener_resumen_gastos(datos, a)?)))
}

fn a_medidas(m: &Medidas) -> MedidasResumen {
    let texto = |d: Dinero| ipc::importe_a_texto(d);
    MedidasResumen {
        importe: texto(m.importe),
        porcentaje_milesimas: m.porcentaje_milesimas,
        anterior: texto(m.anterior),
        variacion_anterior_pb: m.variacion_anterior_pb,
        anterior_mismo_dia: m.anterior_mismo_dia.map(texto),
        variacion_mismo_dia_pb: m.variacion_mismo_dia_pb,
        promedio: m.promedio.map(texto),
        variacion_promedio_pb: m.variacion_promedio_pb,
        usd_aparte: texto(m.usd_aparte),
        n: m.n,
    }
}

fn a_tendencia(t: &Tendencia) -> TendenciaResumen {
    TendenciaResumen {
        meses: t
            .meses
            .iter()
            .map(|p| PuntoTendenciaResumen {
                anio: p.anio,
                mes: p.mes,
                importe: p.importe.map(ipc::importe_a_texto),
                parcial: p.parcial,
            })
            .collect(),
        direccion: t.direccion.como_texto().to_string(),
        motivo_estable: t.motivo_estable.map(|m| m.como_texto().to_string()),
    }
}

fn a_dto(r: ResumenMensual) -> ResumenGastos {
    ResumenGastos {
        anio: r.anio,
        mes: r.mes,
        parcial: r.parcial,
        hasta_dia: r.hasta_dia,
        tasa_usd: ipc::tasa_a_texto(r.tasa_usd),
        meses_promedio: r.meses_promedio,
        id_cargos_derivados: ID_CARGOS_DERIVADOS,
        filas: r
            .filas
            .iter()
            .map(|f| FilaResumenGasto {
                categoria_id: f.categoria_id,
                categoria_nombre: f.categoria_nombre.clone(),
                derivado: f.derivado,
                medidas: a_medidas(&f.medidas),
                tendencia: a_tendencia(&f.tendencia),
            })
            .collect(),
        totales: a_medidas(&r.totales),
        tendencia_total: a_tendencia(&r.tendencia_total),
        meses_disponibles: r.meses_disponibles.iter().map(|(a, m)| MesDisponible { anio: *a, mes: *m }).collect(),
    }
}
