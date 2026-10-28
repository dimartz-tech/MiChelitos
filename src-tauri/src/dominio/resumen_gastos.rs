//! El resumen de gastos por categoría y mes: dominio puro, sin SQL ni Tauri.
//!
//! Recibe los gastos **ya agrupados por día, categoría y divisa** (`FilaAgregadoGasto`, lo que lee el puerto) y los
//! convierte en una tabla por categoría con el importe del mes, su peso sobre el total y sus comparaciones: contra el
//! mes anterior (entero y «al mismo día»), contra el promedio de los N meses previos completos y la parte en USD sin
//! convertir. Todo es `Dinero` o enteros: ningún `f64` toca un importe, y los porcentajes salen de
//! `dividir_redondeando` (mitad lejos de cero), no de una división en coma flotante.
//!
//! ## Decisiones del titular que fija este módulo
//!
//! * **USD → DOP.** Un gasto en dólares liquidado con su propio importe (`monto_liquidado`) aporta ese importe en
//!   pesos; el resto de los dólares se convierte **una sola vez por cubo** (categoría y período) con la tasa de
//!   referencia que llega como parámetro. La parte USD se devuelve además aparte, sin convertir (`usd_aparte`).
//! * **Cargos derivados.** Los gastos que creó otra operación (comisión de un abono, cargo de un avance) se agrupan en
//!   una sola fila sintética, `ID_CARGOS_DERIVADOS`, que nunca coincide con una categoría real.
//! * **Los abonos no son gasto**: no llegan aquí; el puerto lee solo `gastos`.
//! * **Mes calendario** por la fecha del gasto.
//! * **El mes en curso es parcial**: se marca, el promedio de meses previos lo excluye por construcción (solo cuenta
//!   meses anteriores al pedido) y la variación se ofrece también contra el mismo tramo del mes anterior.
//! * **Exclusiones** por identificador de categoría (la fila sintética se excluye con `ID_CARGOS_DERIVADOS`).
//!
//! ## Cómo se redondea
//!
//! La conversión redondea una vez por cubo, así que el importe de una fila es exacto respecto de sus datos y el total
//! es **la suma de las filas** (no la conversión del total). El promedio divide entre el número de meses que
//! **existían** en los datos (un historial de dos meses no se promedia entre seis) y el del total es la suma de los
//! de las filas, por la misma razón.

use super::dinero::{dividir_redondeando, Dinero, Divisa, TasaCambio};
use super::errores::ErrorDominio;
use chrono::{Datelike, NaiveDate};
use std::collections::BTreeMap;

/// Identificador de la fila sintética «Cargos derivados». Los de las categorías reales son positivos.
pub const ID_CARGOS_DERIVADOS: i64 = -1;
pub const NOMBRE_CARGOS_DERIVADOS: &str = "Cargos derivados";

/// Los gastos de un día, de una categoría y de una divisa, sumados en centavos enteros.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilaAgregadoGasto {
    pub anio: i32,
    pub mes: u32,
    pub dia: u32,
    pub categoria_id: i64,
    pub categoria_nombre: String,
    /// `USD` o, para cualquier otra cosa, pesos (igual que el resto de la aplicación).
    pub divisa: String,
    /// El gasto lo creó otra operación (comisión de abono, cargo de avance).
    pub derivado: bool,
    /// `monto + costo_adicional`, en la divisa del gasto.
    pub centavos: i64,
    /// De `centavos`, lo que corresponde a gastos USD **liquidados**, en dólares (solo el `monto`).
    pub liquidado_origen_centavos: i64,
    /// Lo que esos gastos liquidados costaron en pesos, según el emisor. `None` si no hay ninguno.
    pub liquidado_dop_centavos: Option<i64>,
    /// Cuántos gastos hay en la fila.
    pub n: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpcionesResumen {
    /// Categorías que no entran en el resumen (por defecto, ninguna). Se compara con el identificador que lleva la
    /// fila **en el resultado**: un cargo derivado se excluye con `ID_CARGOS_DERIVADOS`.
    pub excluir_categorias: Vec<i64>,
}

/// Las cifras de una fila (o del total).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Medidas {
    /// En pesos (los dólares sin liquidar, convertidos con la tasa de referencia).
    pub importe: Dinero,
    /// Parte del total del mes en milésimas (1000 = 100 %). Cero si el total no es positivo.
    pub porcentaje_milesimas: i64,
    pub anterior: Dinero,
    /// Variación contra el mes anterior completo, en puntos básicos. `None` si el anterior no es positivo.
    pub variacion_anterior_pb: Option<i64>,
    /// El mes anterior hasta el mismo día. Solo en un mes parcial.
    pub anterior_mismo_dia: Option<Dinero>,
    pub variacion_mismo_dia_pb: Option<i64>,
    /// Promedio de los meses previos completos. `None` si no hay ninguno con datos.
    pub promedio: Option<Dinero>,
    pub variacion_promedio_pb: Option<i64>,
    /// Los dólares del mes sin convertir (liquidados o no).
    pub usd_aparte: Dinero,
    pub n: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilaResumen {
    pub categoria_id: i64,
    pub categoria_nombre: String,
    pub derivado: bool,
    pub medidas: Medidas,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResumenMensual {
    pub anio: i32,
    pub mes: u32,
    /// El mes pedido es el mes en curso: su cierre aún no ha llegado.
    pub parcial: bool,
    pub hasta_dia: Option<u32>,
    pub tasa_usd: TasaCambio,
    /// Con cuántos meses se promedió realmente (≤ los pedidos si el historial es más corto).
    pub meses_promedio: u32,
    /// Más grande primero; a igual importe, por nombre.
    pub filas: Vec<FilaResumen>,
    /// La suma de las filas.
    pub totales: Medidas,
    /// Los meses con algún gasto (del más reciente al más antiguo), para el selector.
    pub meses_disponibles: Vec<(i32, u32)>,
}

#[derive(Default, Clone)]
struct Acumulado {
    nombre: String,
    derivado: bool,
    dop: i64,
    usd: i64,
    usd_liquidado_origen: i64,
    usd_liquidado_dop: i64,
    n: i64,
}

impl Acumulado {
    /// Pesos del cubo: los pesos directos, lo que fijó el emisor y los dólares restantes convertidos una vez.
    fn en_pesos(&self, tasa: TasaCambio) -> Result<Dinero, ErrorDominio> {
        let sin_liquidar = Dinero::de_centavos(self.usd - self.usd_liquidado_origen, Divisa::Usd);
        let convertido = sin_liquidar.convertir(Divisa::Dop, tasa)?;
        Ok(Dinero::de_centavos(self.dop + self.usd_liquidado_dop + convertido.centavos(), Divisa::Dop))
    }
}

fn id_de_fila(f: &FilaAgregadoGasto) -> i64 {
    if f.derivado {
        ID_CARGOS_DERIVADOS
    } else {
        f.categoria_id
    }
}

fn acumular(
    filas: &[FilaAgregadoGasto],
    opciones: &OpcionesResumen,
    incluir: impl Fn(&FilaAgregadoGasto) -> bool,
) -> BTreeMap<i64, Acumulado> {
    let mut mapa: BTreeMap<i64, Acumulado> = BTreeMap::new();
    for f in filas.iter().filter(|f| incluir(f)) {
        let id = id_de_fila(f);
        if opciones.excluir_categorias.contains(&id) {
            continue;
        }
        let a = mapa.entry(id).or_default();
        if f.derivado {
            a.nombre = NOMBRE_CARGOS_DERIVADOS.to_string();
            a.derivado = true;
        } else if a.nombre.is_empty() {
            a.nombre = f.categoria_nombre.clone();
        }
        if f.divisa.trim().eq_ignore_ascii_case("USD") {
            a.usd += f.centavos;
            a.usd_liquidado_origen += f.liquidado_origen_centavos;
            a.usd_liquidado_dop += f.liquidado_dop_centavos.unwrap_or(0);
        } else {
            a.dop += f.centavos;
        }
        a.n += f.n;
    }
    mapa
}

/// Mes `(anio, mes)` desplazado `atras` meses hacia el pasado.
fn mes_atras(anio: i32, mes: u32, atras: u32) -> (i32, u32) {
    let indice = anio as i64 * 12 + (mes as i64 - 1) - atras as i64;
    ((indice.div_euclid(12)) as i32, (indice.rem_euclid(12) + 1) as u32)
}

/// Variación de `actual` respecto de `referencia` en puntos básicos; `None` si la referencia no es positiva.
fn variacion_pb(actual: Dinero, referencia: Dinero) -> Option<i64> {
    if referencia.centavos() <= 0 {
        return None;
    }
    let diferencia = actual.centavos() as i128 - referencia.centavos() as i128;
    Some(dividir_redondeando(diferencia * 10_000, referencia.centavos() as i128) as i64)
}

fn milesimas(parte: Dinero, total: Dinero) -> i64 {
    if total.centavos() <= 0 {
        return 0;
    }
    dividir_redondeando(parte.centavos() as i128 * 1_000, total.centavos() as i128) as i64
}

fn dop(centavos: i64) -> Dinero {
    Dinero::de_centavos(centavos, Divisa::Dop)
}

/// Ensambla las cifras de una fila a partir de lo ya calculado.
fn medidas(
    importe: Dinero,
    total: Dinero,
    anterior: Dinero,
    mismo_dia: Option<Dinero>,
    promedio: Option<Dinero>,
    usd: i64,
    n: i64,
) -> Medidas {
    Medidas {
        importe,
        porcentaje_milesimas: milesimas(importe, total),
        anterior,
        variacion_anterior_pb: variacion_pb(importe, anterior),
        anterior_mismo_dia: mismo_dia,
        variacion_mismo_dia_pb: mismo_dia.and_then(|m| variacion_pb(importe, m)),
        promedio,
        variacion_promedio_pb: promedio.and_then(|p| variacion_pb(importe, p)),
        usd_aparte: Dinero::de_centavos(usd, Divisa::Usd),
        n,
    }
}

/// El resumen del mes `mes` (año, mes) tal como está a fecha `hoy`.
///
/// `n_promedio` es cuántos meses previos completos promediar (0 = sin promedio). Falla si el mes no existe o es
/// posterior al mes en curso.
pub fn resumen_por_mes(
    filas: &[FilaAgregadoGasto],
    mes: (i32, u32),
    hoy: NaiveDate,
    n_promedio: u32,
    tasa_usd: TasaCambio,
    opciones: &OpcionesResumen,
) -> Result<ResumenMensual, ErrorDominio> {
    let (anio, m) = mes;
    if !(1..=12).contains(&m) || (anio, m) > (hoy.year(), hoy.month()) {
        return Err(ErrorDominio::PeriodoInvalido { anio, mes: m });
    }
    let parcial = (anio, m) == (hoy.year(), hoy.month());
    let hasta_dia = if parcial { Some(hoy.day()) } else { None };

    let (ant_anio, ant_mes) = mes_atras(anio, m, 1);
    let en_mes = |f: &FilaAgregadoGasto, a: i32, mm: u32| f.anio == a && f.mes == mm;

    let actual = acumular(filas, opciones, |f| en_mes(f, anio, m));
    let anterior = acumular(filas, opciones, |f| en_mes(f, ant_anio, ant_mes));
    let mismo_dia = if parcial {
        Some(acumular(filas, opciones, |f| en_mes(f, ant_anio, ant_mes) && f.dia <= hoy.day()))
    } else {
        None
    };

    // Los meses previos que pueden promediarse: los N anteriores al pedido que no son anteriores al primer mes con
    // datos. Todos son completos, porque el pedido no es posterior al mes en curso.
    let primero = filas.iter().map(|f| (f.anio, f.mes)).min();
    let ventana: Vec<(i32, u32)> = (1..=n_promedio)
        .map(|k| mes_atras(anio, m, k))
        .filter(|ym| primero.map_or(false, |p| *ym >= p))
        .collect();
    let meses_promedio = ventana.len() as u32;
    let promedios = if meses_promedio > 0 {
        acumular(filas, opciones, |f| ventana.contains(&(f.anio, f.mes)))
    } else {
        BTreeMap::new()
    };

    let mut ids: Vec<i64> = actual.keys().chain(anterior.keys()).chain(promedios.keys()).copied().collect();
    ids.sort_unstable();
    ids.dedup();

    let cero = Acumulado::default();
    let mut brutas: Vec<(i64, String, bool, Dinero, Dinero, Option<Dinero>, Option<Dinero>, i64, i64)> = Vec::new();
    for id in ids {
        let a = actual.get(&id).unwrap_or(&cero);
        let p = anterior.get(&id).unwrap_or(&cero);
        let nombre = [actual.get(&id), anterior.get(&id), promedios.get(&id)]
            .into_iter()
            .flatten()
            .map(|x| x.nombre.clone())
            .next()
            .unwrap_or_default();
        let derivado = id == ID_CARGOS_DERIVADOS;
        let md = match &mismo_dia {
            Some(mapa) => Some(mapa.get(&id).unwrap_or(&cero).en_pesos(tasa_usd)?),
            None => None,
        };
        let prom = if meses_promedio > 0 {
            let suma = promedios.get(&id).unwrap_or(&cero).en_pesos(tasa_usd)?;
            Some(dop(dividir_redondeando(suma.centavos() as i128, meses_promedio as i128) as i64))
        } else {
            None
        };
        brutas.push((id, nombre, derivado, a.en_pesos(tasa_usd)?, p.en_pesos(tasa_usd)?, md, prom, a.usd, a.n));
    }

    let suma = |f: &dyn Fn(&(i64, String, bool, Dinero, Dinero, Option<Dinero>, Option<Dinero>, i64, i64)) -> i64| -> i64 {
        brutas.iter().map(f).sum()
    };
    let total_importe = dop(suma(&|b| b.3.centavos()));
    let total_anterior = dop(suma(&|b| b.4.centavos()));
    let total_mismo_dia = mismo_dia.as_ref().map(|_| dop(suma(&|b| b.5.map_or(0, |d| d.centavos()))));
    let total_promedio = if meses_promedio > 0 { Some(dop(suma(&|b| b.6.map_or(0, |d| d.centavos())))) } else { None };
    let total_usd = suma(&|b| b.7);
    let total_n = suma(&|b| b.8);

    let mut filas_resumen: Vec<FilaResumen> = brutas
        .into_iter()
        .map(|(id, nombre, derivado, imp, ant, md, prom, usd, n)| FilaResumen {
            categoria_id: id,
            categoria_nombre: nombre,
            derivado,
            medidas: medidas(imp, total_importe, ant, md, prom, usd, n),
        })
        .collect();
    filas_resumen.sort_by(|a, b| {
        b.medidas.importe.centavos().cmp(&a.medidas.importe.centavos()).then_with(|| a.categoria_nombre.cmp(&b.categoria_nombre))
    });

    let mut meses_disponibles: Vec<(i32, u32)> = filas.iter().map(|f| (f.anio, f.mes)).collect();
    meses_disponibles.sort_unstable_by(|a, b| b.cmp(a));
    meses_disponibles.dedup();

    Ok(ResumenMensual {
        anio,
        mes: m,
        parcial,
        hasta_dia,
        tasa_usd,
        meses_promedio,
        filas: filas_resumen,
        totales: medidas(total_importe, total_importe, total_anterior, total_mismo_dia, total_promedio, total_usd, total_n),
        meses_disponibles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tasa() -> TasaCambio {
        TasaCambio::nueva(60.0).unwrap()
    }

    fn fecha(a: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(a, m, d).unwrap()
    }

    fn fila(a: i32, m: u32, d: u32, cat: i64, nombre: &str, divisa: &str, centavos: i64) -> FilaAgregadoGasto {
        FilaAgregadoGasto {
            anio: a,
            mes: m,
            dia: d,
            categoria_id: cat,
            categoria_nombre: nombre.into(),
            divisa: divisa.into(),
            derivado: false,
            centavos,
            liquidado_origen_centavos: 0,
            liquidado_dop_centavos: None,
            n: 1,
        }
    }

    fn pesos(a: i32, m: u32, d: u32, cat: i64, nombre: &str, centavos: i64) -> FilaAgregadoGasto {
        fila(a, m, d, cat, nombre, "DOP", centavos)
    }

    fn resumir(filas: &[FilaAgregadoGasto], mes: (i32, u32), hoy: NaiveDate, n: u32) -> ResumenMensual {
        resumen_por_mes(filas, mes, hoy, n, tasa(), &OpcionesResumen::default()).unwrap()
    }

    fn de<'a>(r: &'a ResumenMensual, id: i64) -> &'a FilaResumen {
        r.filas.iter().find(|f| f.categoria_id == id).unwrap_or_else(|| panic!("no hay la fila {id}"))
    }

    #[test]
    fn mil_veces_diez_y_veinte_centavos_suman_exacto() {
        // 0.1 + 0.2 en coma flotante no es 0.3; en centavos, mil veces tampoco se desvía.
        let mut filas = Vec::new();
        for i in 0..1000 {
            filas.push(pesos(2026, 9, 1 + (i % 28) as u32, 1, "Comida", 10));
            filas.push(pesos(2026, 9, 1 + (i % 28) as u32, 1, "Comida", 20));
        }
        let r = resumir(&filas, (2026, 9), fecha(2026, 10, 28), 0);
        assert_eq!(de(&r, 1).medidas.importe.centavos(), 30_000);
        assert_eq!(r.totales.importe.centavos(), 30_000);
        assert_eq!(r.totales.n, 2000);
    }

    #[test]
    fn la_variacion_contra_un_mes_anterior_en_cero_no_divide() {
        let filas = vec![pesos(2026, 9, 3, 1, "Comida", 118_753)];
        let r = resumir(&filas, (2026, 9), fecha(2026, 10, 28), 3);
        let m = &de(&r, 1).medidas;
        assert_eq!(m.anterior.centavos(), 0);
        assert_eq!(m.variacion_anterior_pb, None);
        assert_eq!(r.totales.variacion_anterior_pb, None);
    }

    #[test]
    fn el_signo_de_la_variacion_es_actual_menos_referencia() {
        let filas = vec![
            pesos(2026, 8, 3, 1, "Comida", 100_000),
            pesos(2026, 9, 3, 1, "Comida", 125_000),
            pesos(2026, 8, 5, 2, "Ocio", 100_000),
            pesos(2026, 9, 5, 2, "Ocio", 80_000),
        ];
        let r = resumir(&filas, (2026, 9), fecha(2026, 10, 28), 0);
        assert_eq!(de(&r, 1).medidas.variacion_anterior_pb, Some(2_500));
        assert_eq!(de(&r, 2).medidas.variacion_anterior_pb, Some(-2_000));
    }

    #[test]
    fn el_promedio_excluye_el_mes_en_curso_y_los_meses_vacios_cuentan_como_cero() {
        // hoy = 12/10: octubre es el mes en curso. Promedio de 3 previos de octubre: jul, ago, sep.
        let filas = vec![
            pesos(2026, 7, 3, 1, "Comida", 30_000),
            pesos(2026, 9, 3, 1, "Comida", 60_000),
            pesos(2026, 10, 2, 1, "Comida", 999_999), // el mes en curso no debe entrar en el promedio
        ];
        let r = resumir(&filas, (2026, 10), fecha(2026, 10, 12), 3);
        assert_eq!(r.meses_promedio, 3);
        assert_eq!(de(&r, 1).medidas.promedio.unwrap().centavos(), 30_000); // (300 + 0 + 600) / 3
    }

    #[test]
    fn un_historial_corto_no_se_promedia_entre_meses_que_no_existian() {
        let filas = vec![pesos(2026, 8, 3, 1, "Comida", 10_000), pesos(2026, 9, 3, 1, "Comida", 30_000), pesos(2026, 10, 1, 1, "Comida", 1)];
        let r = resumir(&filas, (2026, 10), fecha(2026, 10, 12), 6);
        assert_eq!(r.meses_promedio, 2);
        assert_eq!(de(&r, 1).medidas.promedio.unwrap().centavos(), 20_000);
        let sin_historial = resumir(&filas, (2026, 8), fecha(2026, 10, 12), 6);
        assert_eq!(sin_historial.meses_promedio, 0);
        assert_eq!(de(&sin_historial, 1).medidas.promedio, None);
        assert_eq!(de(&sin_historial, 1).medidas.variacion_promedio_pb, None);
    }

    #[test]
    fn el_promedio_se_redondea_a_la_mitad_lejos_de_cero() {
        // (0.01 + 0.02) / 2 = 0.015 -> 0.02
        let filas = vec![pesos(2026, 8, 3, 1, "X", 1), pesos(2026, 9, 3, 1, "X", 2), pesos(2026, 10, 3, 1, "X", 5)];
        let r = resumir(&filas, (2026, 10), fecha(2026, 11, 3), 2);
        assert_eq!(de(&r, 1).medidas.promedio.unwrap().centavos(), 2);
    }

    #[test]
    fn el_mes_parcial_se_marca_y_compara_contra_el_mismo_tramo_del_anterior() {
        let filas = vec![
            pesos(2026, 9, 2, 1, "Comida", 40_000),
            pesos(2026, 9, 20, 1, "Comida", 160_000), // después del día 10: no cuenta en «mismo día»
            pesos(2026, 10, 4, 1, "Comida", 50_000),
        ];
        let r = resumir(&filas, (2026, 10), fecha(2026, 10, 10), 0);
        assert!(r.parcial);
        assert_eq!(r.hasta_dia, Some(10));
        let m = &de(&r, 1).medidas;
        assert_eq!(m.anterior.centavos(), 200_000);
        assert_eq!(m.variacion_anterior_pb, Some(-7_500), "contra el mes entero parece una caída");
        assert_eq!(m.anterior_mismo_dia.unwrap().centavos(), 40_000);
        assert_eq!(m.variacion_mismo_dia_pb, Some(2_500), "al mismo día es un aumento");
        assert_eq!(r.totales.anterior_mismo_dia.unwrap().centavos(), 40_000);
    }

    #[test]
    fn un_mes_cerrado_no_es_parcial_y_no_trae_mismo_dia() {
        let filas = vec![pesos(2026, 9, 2, 1, "Comida", 40_000)];
        let r = resumir(&filas, (2026, 9), fecha(2026, 10, 10), 0);
        assert!(!r.parcial);
        assert_eq!(r.hasta_dia, None);
        assert_eq!(de(&r, 1).medidas.anterior_mismo_dia, None);
        assert_eq!(r.totales.anterior_mismo_dia, None);
    }

    #[test]
    fn el_cambio_de_anio_enlaza_diciembre_con_enero() {
        let filas = vec![pesos(2025, 12, 31, 1, "Comida", 10_000), pesos(2026, 1, 5, 1, "Comida", 15_000)];
        let r = resumir(&filas, (2026, 1), fecha(2026, 2, 10), 1);
        let m = &de(&r, 1).medidas;
        assert_eq!(m.anterior.centavos(), 10_000);
        assert_eq!(m.promedio.unwrap().centavos(), 10_000);
        assert_eq!(mes_atras(2026, 1, 13), (2024, 12));
    }

    #[test]
    fn una_categoria_que_solo_aparece_en_un_mes_sale_con_ceros_en_el_otro() {
        let filas = vec![pesos(2026, 8, 3, 1, "Solo agosto", 70_007), pesos(2026, 9, 3, 2, "Solo septiembre", 90_009)];
        let r = resumir(&filas, (2026, 9), fecha(2026, 10, 28), 0);
        let a = &de(&r, 1).medidas;
        assert_eq!((a.importe.centavos(), a.anterior.centavos(), a.n), (0, 70_007, 0));
        assert_eq!(a.variacion_anterior_pb, Some(-10_000));
        let b = &de(&r, 2).medidas;
        assert_eq!((b.importe.centavos(), b.anterior.centavos()), (90_009, 0));
        assert_eq!(b.variacion_anterior_pb, None);
    }

    #[test]
    fn usd_sin_liquidar_se_convierte_con_la_tasa_de_referencia_y_se_devuelve_aparte() {
        // 12.34 USD * 60 = 740.40 DOP
        let filas = vec![fila(2026, 9, 3, 1, "Compras", "USD", 1_234), pesos(2026, 9, 4, 1, "Compras", 10_001)];
        let r = resumir(&filas, (2026, 9), fecha(2026, 10, 28), 0);
        let m = &de(&r, 1).medidas;
        assert_eq!(m.importe.centavos(), 74_040 + 10_001);
        assert_eq!(m.usd_aparte, Dinero::de_centavos(1_234, Divisa::Usd));
        assert_eq!(r.totales.usd_aparte.centavos(), 1_234);
        assert_eq!(r.totales.usd_aparte.divisa(), Divisa::Usd);
    }

    #[test]
    fn usd_liquidado_usa_su_importe_en_pesos_y_el_resto_la_tasa() {
        // 50.00 USD liquidados por el emisor en 3,177.29 DOP; 10.00 USD sin liquidar a 60 = 600.00 DOP.
        let mut liq = fila(2026, 9, 3, 1, "Compras", "USD", 5_000);
        liq.liquidado_origen_centavos = 5_000;
        liq.liquidado_dop_centavos = Some(317_729);
        let sin = fila(2026, 9, 4, 1, "Compras", "USD", 1_000);
        let r = resumir(&[liq, sin], (2026, 9), fecha(2026, 10, 28), 0);
        let m = &de(&r, 1).medidas;
        assert_eq!(m.importe.centavos(), 317_729 + 60_000);
        assert_eq!(m.usd_aparte.centavos(), 6_000, "los dólares liquidados también van aparte");
    }

    #[test]
    fn el_porcentaje_se_redondea_en_milesimas_y_el_total_es_la_suma_de_las_filas() {
        // 1/3 y 2/3 -> 333 y 667 milésimas.
        let filas = vec![pesos(2026, 9, 3, 1, "A", 100_000), pesos(2026, 9, 3, 2, "B", 200_000)];
        let r = resumir(&filas, (2026, 9), fecha(2026, 10, 28), 0);
        assert_eq!(de(&r, 1).medidas.porcentaje_milesimas, 333);
        assert_eq!(de(&r, 2).medidas.porcentaje_milesimas, 667);
        // mitad exacta: 0.5 milésima sube (1 de 2001 centavos... 1000/2000 -> 500; 1/2000*1000 = 0.5 -> 1)
        let r2 = resumir(&[pesos(2026, 9, 3, 1, "A", 1), pesos(2026, 9, 3, 2, "B", 1_999)], (2026, 9), fecha(2026, 10, 28), 0);
        assert_eq!(de(&r2, 1).medidas.porcentaje_milesimas, 1);
        assert_eq!(r.totales.porcentaje_milesimas, 1000);
    }

    #[test]
    fn el_total_es_la_suma_exacta_de_las_filas_incluso_con_dolares() {
        let filas = vec![
            fila(2026, 9, 1, 1, "A", "USD", 333),
            fila(2026, 9, 1, 2, "B", "USD", 333),
            fila(2026, 9, 1, 3, "C", "USD", 333),
            pesos(2026, 9, 2, 1, "A", 1_187),
        ];
        let tasa_fea = TasaCambio::nueva(58.7351).unwrap();
        let r = resumen_por_mes(&filas, (2026, 9), fecha(2026, 10, 28), 0, tasa_fea, &OpcionesResumen::default()).unwrap();
        let suma: i64 = r.filas.iter().map(|f| f.medidas.importe.centavos()).sum();
        assert_eq!(r.totales.importe.centavos(), suma);
        assert_eq!(r.totales.usd_aparte.centavos(), 999);
    }

    #[test]
    fn los_cargos_derivados_van_a_una_fila_sintetica_aparte_de_su_categoria() {
        let mut comision = pesos(2026, 9, 5, 7, "Servicios", 1_523);
        comision.derivado = true;
        let mut cargo = pesos(2026, 9, 6, 9, "Otros", 4_477);
        cargo.derivado = true;
        let filas = vec![pesos(2026, 9, 5, 7, "Servicios", 20_000), comision, cargo];
        let r = resumir(&filas, (2026, 9), fecha(2026, 10, 28), 0);
        let d = de(&r, ID_CARGOS_DERIVADOS);
        assert!(d.derivado);
        assert_eq!(d.categoria_nombre, "Cargos derivados");
        assert_eq!(d.medidas.importe.centavos(), 6_000);
        assert_eq!(d.medidas.n, 2);
        assert_eq!(de(&r, 7).medidas.importe.centavos(), 20_000, "su categoría real no los absorbe");
        assert!(r.filas.iter().all(|f| f.categoria_id == ID_CARGOS_DERIVADOS || !f.derivado));
        assert!(r.filas.iter().all(|f| f.categoria_id != 9), "la categoría de un derivado no aparece por él");
    }

    #[test]
    fn las_exclusiones_quitan_la_fila_del_total_y_de_los_porcentajes() {
        let filas = vec![pesos(2026, 9, 3, 1, "Comida", 25_000), pesos(2026, 9, 3, 2, "Prestamo", 75_000)];
        let opciones = OpcionesResumen { excluir_categorias: vec![2] };
        let r = resumen_por_mes(&filas, (2026, 9), fecha(2026, 10, 28), 0, tasa(), &opciones).unwrap();
        assert_eq!(r.filas.len(), 1);
        assert_eq!(r.totales.importe.centavos(), 25_000);
        assert_eq!(de(&r, 1).medidas.porcentaje_milesimas, 1000);
        // La fila sintética se excluye con su propio identificador.
        let mut d = pesos(2026, 9, 3, 1, "Comida", 500);
        d.derivado = true;
        let con = vec![pesos(2026, 9, 3, 1, "Comida", 25_000), d];
        let opciones = OpcionesResumen { excluir_categorias: vec![ID_CARGOS_DERIVADOS] };
        let r = resumen_por_mes(&con, (2026, 9), fecha(2026, 10, 28), 0, tasa(), &opciones).unwrap();
        assert_eq!(r.totales.importe.centavos(), 25_000);
    }

    #[test]
    fn los_meses_disponibles_salen_del_mas_reciente_al_mas_antiguo_sin_repetir() {
        let filas = vec![pesos(2026, 7, 1, 1, "A", 1), pesos(2026, 9, 1, 1, "A", 1), pesos(2026, 9, 2, 2, "B", 1), pesos(2025, 12, 1, 1, "A", 1)];
        let r = resumir(&filas, (2026, 9), fecha(2026, 10, 28), 0);
        assert_eq!(r.meses_disponibles, vec![(2026, 9), (2026, 7), (2025, 12)]);
    }

    #[test]
    fn un_mes_inexistente_o_futuro_se_rechaza() {
        let o = OpcionesResumen::default();
        for mes in [(2026, 0), (2026, 13), (2026, 11), (2027, 1)] {
            let e = resumen_por_mes(&[], mes, fecha(2026, 10, 28), 0, tasa(), &o).unwrap_err();
            assert_eq!(e, ErrorDominio::PeriodoInvalido { anio: mes.0, mes: mes.1 });
        }
        assert!(resumen_por_mes(&[], (2026, 10), fecha(2026, 10, 28), 0, tasa(), &o).is_ok());
    }

    #[test]
    fn el_orden_es_de_mayor_a_menor_y_desempata_por_nombre() {
        let filas = vec![pesos(2026, 9, 1, 1, "Zeta", 500), pesos(2026, 9, 1, 2, "Alfa", 500), pesos(2026, 9, 1, 3, "Mayor", 900)];
        let r = resumir(&filas, (2026, 9), fecha(2026, 10, 28), 0);
        let nombres: Vec<&str> = r.filas.iter().map(|f| f.categoria_nombre.as_str()).collect();
        assert_eq!(nombres, vec!["Mayor", "Alfa", "Zeta"]);
    }
}
