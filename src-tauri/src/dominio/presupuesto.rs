//! Presupuestos mensuales por categoría: dominio puro, sin SQL ni Tauri.
//!
//! El titular fija, para una categoría, un **tope mensual en pesos**; el estado del mes compara ese tope con lo gastado
//! en la categoría y dice, de forma descriptiva, si va dentro, cerca o excedido. No recomienda nada: no es asesoría.
//!
//! ## Reglas
//!
//! * **Dinero**: centavos enteros (`i64`) y `i128` para los productos cruzados; ningún `f64` toca un importe. El
//!   presupuesto es mayor que cero y de a lo sumo `TOPE_CENTAVOS` (DOP 999,999,999.99).
//! * **Solo cuentan los gastos en DOP.** Un gasto en dólares **no se convierte** (no hay tasa del mes que valga como
//!   cierre) y **no entra** en `gastado`; el total en USD del mes se informa aparte (`usd_aparte`, sin convertir) para
//!   que no pase inadvertido. Un gasto en dólares liquidado en pesos tampoco entra: la regla es por divisa del gasto.
//! * **Categoría real.** Cuenta todo gasto de la categoría, también el que creó otra operación (comisión de un abono,
//!   cargo de un avance): es un gasto de esa categoría. (El Resumen, en cambio, agrupa esos cargos en una fila aparte.)
//! * **Mes calendario** por la fecha del gasto.
//! * **Estado** (con enteros, sin redondeo): `Excedido` si `gastado ≥ presupuesto`; `Cerca` si `gastado ≥ 80 %` del
//!   presupuesto; `Ok` en otro caso. El porcentaje que se muestra es la parte **truncada** en milésimas (1000 = 100 %),
//!   de modo que 99.96 % no se muestra como 100 % mientras el estado sigue siendo «cerca».
//! * **Restante** = presupuesto − gastado; es **negativo** si se excedió (la magnitud es lo que se pasó).
//! * Una categoría fija (marca `fija`) puede tener presupuesto igual que una variable; la marca no se toca.

use super::dinero::{Dinero, Divisa};
use super::errores::ErrorDominio;
use super::resumen_gastos::FilaAgregadoGasto;
use std::collections::BTreeMap;

/// Tope de un presupuesto mensual, en centavos (DOP 999,999,999.99). Coincide con el `CHECK` de la tabla.
pub const TOPE_CENTAVOS: i64 = 99_999_999_999;

/// Desde qué parte del presupuesto el estado es «cerca», en porcentaje.
pub const UMBRAL_CERCA_PORCIENTO: i128 = 80;

/// Un presupuesto fijado, con los datos de su categoría.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presupuesto {
    pub categoria_id: i64,
    pub categoria_nombre: String,
    /// La marca «fija / recurrente» de la categoría (informativa: no cambia el cálculo).
    pub fija: bool,
    pub centavos: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoPresupuesto {
    Ok,
    Cerca,
    Excedido,
}

impl EstadoPresupuesto {
    pub fn como_texto(self) -> &'static str {
        match self {
            EstadoPresupuesto::Ok => "ok",
            EstadoPresupuesto::Cerca => "cerca",
            EstadoPresupuesto::Excedido => "excedido",
        }
    }
}

/// El presupuesto de una categoría frente a lo gastado en un mes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstadoDeCategoria {
    pub categoria_id: i64,
    pub categoria_nombre: String,
    pub fija: bool,
    pub presupuesto: Dinero,
    /// Gastos en DOP de la categoría en el mes (monto + costo adicional).
    pub gastado: Dinero,
    /// `presupuesto − gastado`; negativo si se excedió.
    pub restante: Dinero,
    /// `gastado / presupuesto` en milésimas, truncado (1000 = 100 %). Puede pasar de 1000.
    pub porcentaje_milesimas: i64,
    pub estado: EstadoPresupuesto,
    /// Gastos en dólares de la categoría en el mes, **sin convertir** y fuera de `gastado`.
    pub usd_aparte: Dinero,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstadoDelMes {
    pub anio: i32,
    pub mes: u32,
    /// Más urgente primero: excedidos, luego cerca, luego ok; dentro de cada grupo el mayor porcentaje, luego el nombre.
    pub filas: Vec<EstadoDeCategoria>,
    /// Suma de los presupuestos de las categorías que tienen uno.
    pub total_presupuesto: Dinero,
    /// Suma de lo gastado **en esas mismas categorías** (no el total del mes).
    pub total_gastado: Dinero,
    pub total_restante: Dinero,
}

/// Valida el monto de un presupuesto: mayor que cero y sin pasar del tope.
pub fn validar_monto(centavos: i64) -> Result<i64, ErrorDominio> {
    if centavos <= 0 {
        return Err(ErrorDominio::PresupuestoNoPositivo);
    }
    if centavos > TOPE_CENTAVOS {
        return Err(ErrorDominio::PresupuestoExcedeElTope);
    }
    Ok(centavos)
}

/// Qué pasa con los presupuestos al fusionar la categoría de origen en la de destino.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresupuestoTrasFusion {
    /// El presupuesto que queda en el destino (`None`: ninguno).
    pub resultante: Option<i64>,
    /// El del origen que se pierde porque el destino ya tenía uno (nunca se suman en silencio).
    pub descartado: Option<i64>,
    /// El destino heredó el del origen.
    pub heredado: bool,
}

/// **Regla de la fusión (decidida y confirmada por el titular). Único lugar donde vive.**
///
/// * Ninguno tiene presupuesto: no hay nada.
/// * Solo el origen lo tiene: pasa al destino (lo hereda).
/// * Solo el destino lo tiene: se conserva.
/// * Ambos lo tienen: se conserva el del destino y el del origen **se descarta** (la vista previa lo avisa con su monto).
///
/// Nunca se suman: sumar dos topes mensuales sería inventar un tercero que el titular no fijó.
pub fn presupuesto_tras_fusion(origen: Option<i64>, destino: Option<i64>) -> PresupuestoTrasFusion {
    match (origen, destino) {
        (None, d) => PresupuestoTrasFusion { resultante: d, descartado: None, heredado: false },
        (Some(o), None) => PresupuestoTrasFusion { resultante: Some(o), descartado: None, heredado: true },
        (Some(o), Some(d)) => PresupuestoTrasFusion { resultante: Some(d), descartado: Some(o), heredado: false },
    }
}

/// El estado según lo gastado, con enteros exactos.
pub fn estado_de(gastado: i64, presupuesto: i64) -> EstadoPresupuesto {
    let (g, p) = (gastado as i128, presupuesto as i128);
    if g >= p {
        EstadoPresupuesto::Excedido
    } else if g * 100 >= p * UMBRAL_CERCA_PORCIENTO {
        EstadoPresupuesto::Cerca
    } else {
        EstadoPresupuesto::Ok
    }
}

fn milesimas_truncadas(gastado: i64, presupuesto: i64) -> i64 {
    if gastado <= 0 || presupuesto <= 0 {
        return 0;
    }
    let m = (gastado as i128) * 1000 / (presupuesto as i128);
    m.min(i64::MAX as i128) as i64
}

fn dop(c: i64) -> Dinero {
    Dinero::de_centavos(c, Divisa::Dop)
}

/// El estado de cada presupuesto en el mes `(anio, mes)`. `filas` son los gastos agrupados que lee el puerto.
pub fn estado_del_mes(
    anio: i32,
    mes: u32,
    filas: &[FilaAgregadoGasto],
    presupuestos: &[Presupuesto],
) -> Result<EstadoDelMes, ErrorDominio> {
    if !(1..=12).contains(&mes) {
        return Err(ErrorDominio::PeriodoInvalido { anio, mes });
    }
    // (pesos, dólares) por categoría, en el mes.
    let mut por_categoria: BTreeMap<i64, (i64, i64)> = BTreeMap::new();
    for f in filas.iter().filter(|f| f.anio == anio && f.mes == mes) {
        let e = por_categoria.entry(f.categoria_id).or_insert((0, 0));
        if f.divisa == "USD" {
            e.1 += f.centavos;
        } else {
            e.0 += f.centavos;
        }
    }
    let mut estados: Vec<EstadoDeCategoria> = presupuestos
        .iter()
        .map(|p| {
            let (pesos, dolares) = por_categoria.get(&p.categoria_id).copied().unwrap_or((0, 0));
            EstadoDeCategoria {
                categoria_id: p.categoria_id,
                categoria_nombre: p.categoria_nombre.clone(),
                fija: p.fija,
                presupuesto: dop(p.centavos),
                gastado: dop(pesos),
                restante: dop(p.centavos - pesos),
                porcentaje_milesimas: milesimas_truncadas(pesos, p.centavos),
                estado: estado_de(pesos, p.centavos),
                usd_aparte: Dinero::de_centavos(dolares, Divisa::Usd),
            }
        })
        .collect();
    let severidad = |e: EstadoPresupuesto| match e {
        EstadoPresupuesto::Excedido => 0,
        EstadoPresupuesto::Cerca => 1,
        EstadoPresupuesto::Ok => 2,
    };
    estados.sort_by(|a, b| {
        severidad(a.estado)
            .cmp(&severidad(b.estado))
            .then_with(|| b.porcentaje_milesimas.cmp(&a.porcentaje_milesimas))
            .then_with(|| a.categoria_nombre.cmp(&b.categoria_nombre))
    });
    let total_presupuesto: i64 = estados.iter().map(|e| e.presupuesto.centavos()).sum();
    let total_gastado: i64 = estados.iter().map(|e| e.gastado.centavos()).sum();
    Ok(EstadoDelMes {
        anio,
        mes,
        total_presupuesto: dop(total_presupuesto),
        total_gastado: dop(total_gastado),
        total_restante: dop(total_presupuesto - total_gastado),
        filas: estados,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fila(mes: u32, cat: i64, divisa: &str, centavos: i64, derivado: bool) -> FilaAgregadoGasto {
        FilaAgregadoGasto {
            anio: 2026,
            mes,
            dia: 3,
            categoria_id: cat,
            categoria_nombre: format!("Cat {cat}"),
            divisa: divisa.into(),
            derivado,
            fija: false,
            centavos,
            liquidado_origen_centavos: 0,
            liquidado_dop_centavos: None,
            n: 1,
        }
    }

    fn pres(cat: i64, nombre: &str, centavos: i64) -> Presupuesto {
        Presupuesto { categoria_id: cat, categoria_nombre: nombre.into(), fija: false, centavos }
    }

    #[test]
    fn el_monto_debe_ser_positivo_y_no_pasar_del_tope() {
        assert_eq!(validar_monto(1), Ok(1));
        assert_eq!(validar_monto(TOPE_CENTAVOS), Ok(TOPE_CENTAVOS));
        assert_eq!(validar_monto(0), Err(ErrorDominio::PresupuestoNoPositivo));
        assert_eq!(validar_monto(-1), Err(ErrorDominio::PresupuestoNoPositivo));
        assert_eq!(validar_monto(TOPE_CENTAVOS + 1), Err(ErrorDominio::PresupuestoExcedeElTope));
    }

    #[test]
    fn la_fusion_aplica_la_regla_en_los_cuatro_casos() {
        let r = presupuesto_tras_fusion(None, None);
        assert_eq!((r.resultante, r.descartado, r.heredado), (None, None, false));
        let r = presupuesto_tras_fusion(Some(10_001), None);
        assert_eq!((r.resultante, r.descartado, r.heredado), (Some(10_001), None, true), "el destino hereda");
        let r = presupuesto_tras_fusion(None, Some(20_002));
        assert_eq!((r.resultante, r.descartado, r.heredado), (Some(20_002), None, false), "el destino se queda como está");
        let r = presupuesto_tras_fusion(Some(10_001), Some(20_002));
        assert_eq!((r.resultante, r.descartado, r.heredado), (Some(20_002), Some(10_001), false), "gana el del destino; nunca se suman");
    }

    #[test]
    fn los_umbrales_son_exactos_en_el_limite() {
        // Presupuesto de 1 000.00 (100 000 centavos): 80 % = 80 000; 100 % = 100 000.
        assert_eq!(estado_de(79_999, 100_000), EstadoPresupuesto::Ok);
        assert_eq!(estado_de(80_000, 100_000), EstadoPresupuesto::Cerca);
        assert_eq!(estado_de(99_999, 100_000), EstadoPresupuesto::Cerca);
        assert_eq!(estado_de(100_000, 100_000), EstadoPresupuesto::Excedido, "el 100 % exacto ya es excedido");
        assert_eq!(estado_de(100_001, 100_000), EstadoPresupuesto::Excedido);
        assert_eq!(estado_de(0, 100_000), EstadoPresupuesto::Ok);
        // Un presupuesto en el que el 80 % no cae en centavo entero: 80 % de 1.01 = 0.808 → 81 centavos ya es cerca, 80 no.
        assert_eq!(estado_de(80, 101), EstadoPresupuesto::Ok);
        assert_eq!(estado_de(81, 101), EstadoPresupuesto::Cerca);
        // Importes enormes no desbordan.
        assert_eq!(estado_de(TOPE_CENTAVOS, TOPE_CENTAVOS), EstadoPresupuesto::Excedido);
    }

    #[test]
    fn el_porcentaje_se_trunca_y_nunca_muestra_100_mientras_es_cerca() {
        let r = estado_del_mes(2026, 9, &[fila(9, 1, "DOP", 99_996, false)], &[pres(1, "Comida", 100_000)]).unwrap();
        assert_eq!(r.filas[0].porcentaje_milesimas, 999);
        assert_eq!(r.filas[0].estado, EstadoPresupuesto::Cerca);
        let r = estado_del_mes(2026, 9, &[fila(9, 1, "DOP", 150_000, false)], &[pres(1, "Comida", 100_000)]).unwrap();
        assert_eq!(r.filas[0].porcentaje_milesimas, 1500);
    }

    #[test]
    fn gastado_restante_y_exceso_con_centavos_poco_redondos() {
        let filas = vec![fila(9, 1, "DOP", 41_237, false), fila(9, 1, "DOP", 8_863, true), fila(9, 2, "DOP", 12_345, false)];
        let r = estado_del_mes(2026, 9, &filas, &[pres(1, "Comida", 60_001), pres(2, "Ocio", 10_000)]).unwrap();
        let comida = r.filas.iter().find(|f| f.categoria_id == 1).unwrap();
        assert_eq!(comida.gastado.centavos(), 50_100, "incluye el gasto derivado: es de la categoría");
        assert_eq!(comida.restante.centavos(), 9_901);
        assert_eq!(comida.estado, EstadoPresupuesto::Cerca);
        let ocio = r.filas.iter().find(|f| f.categoria_id == 2).unwrap();
        assert_eq!(ocio.restante.centavos(), -2_345, "negativo: lo que se pasó");
        assert_eq!(ocio.estado, EstadoPresupuesto::Excedido);
        assert_eq!(r.total_presupuesto.centavos(), 70_001);
        assert_eq!(r.total_gastado.centavos(), 62_445);
        assert_eq!(r.total_restante.centavos(), 7_556);
        assert_eq!(r.total_presupuesto.centavos() - r.total_gastado.centavos(), r.total_restante.centavos());
    }

    #[test]
    fn los_dolares_no_cuentan_se_informan_aparte_y_sin_convertir() {
        let filas = vec![fila(9, 1, "DOP", 20_000, false), fila(9, 1, "USD", 5_311, false)];
        let r = estado_del_mes(2026, 9, &filas, &[pres(1, "Suscripciones", 50_000)]).unwrap();
        let f = &r.filas[0];
        assert_eq!(f.gastado.centavos(), 20_000, "los dólares no suman a los pesos");
        assert_eq!(f.usd_aparte.centavos(), 5_311);
        assert_eq!(f.usd_aparte.divisa(), Divisa::Usd);
        assert_eq!(f.estado, EstadoPresupuesto::Ok);
        // Solo dólares: gastado 0, pero se ven aparte.
        let r = estado_del_mes(2026, 9, &[fila(9, 1, "USD", 999_999, false)], &[pres(1, "Suscripciones", 50_000)]).unwrap();
        assert_eq!((r.filas[0].gastado.centavos(), r.filas[0].usd_aparte.centavos()), (0, 999_999));
        assert_eq!(r.filas[0].estado, EstadoPresupuesto::Ok);
    }

    #[test]
    fn solo_cuenta_el_mes_pedido_y_no_el_anterior_ni_otro_anio() {
        let mut otro_anio = fila(9, 1, "DOP", 7_777, false);
        otro_anio.anio = 2025;
        let filas = vec![fila(8, 1, "DOP", 90_000, false), fila(9, 1, "DOP", 1_001, false), fila(10, 1, "DOP", 90_000, false), otro_anio];
        let r = estado_del_mes(2026, 9, &filas, &[pres(1, "Comida", 50_000)]).unwrap();
        assert_eq!(r.filas[0].gastado.centavos(), 1_001);
    }

    #[test]
    fn una_categoria_fija_se_trata_igual_y_conserva_su_marca() {
        let mut p = pres(1, "Seguros", 30_000);
        p.fija = true;
        let r = estado_del_mes(2026, 9, &[fila(9, 1, "DOP", 31_000, false)], &[p]).unwrap();
        assert!(r.filas[0].fija);
        assert_eq!(r.filas[0].estado, EstadoPresupuesto::Excedido);
    }

    #[test]
    fn sin_gasto_el_restante_es_todo_el_presupuesto_y_el_orden_pone_lo_urgente_primero() {
        let filas = vec![fila(9, 2, "DOP", 85_000, false), fila(9, 3, "DOP", 120_000, false), fila(9, 4, "DOP", 99_000, false)];
        let r = estado_del_mes(
            2026,
            9,
            &filas,
            &[pres(1, "Vacía", 10_000), pres(2, "B cerca", 100_000), pres(3, "A excedida", 100_000), pres(4, "C cerca", 100_000)],
        )
        .unwrap();
        let nombres: Vec<&str> = r.filas.iter().map(|f| f.categoria_nombre.as_str()).collect();
        assert_eq!(nombres, vec!["A excedida", "C cerca", "B cerca", "Vacía"]);
        let vacia = r.filas.last().unwrap();
        assert_eq!((vacia.gastado.centavos(), vacia.restante.centavos(), vacia.porcentaje_milesimas), (0, 10_000, 0));
    }

    #[test]
    fn un_mes_invalido_es_un_error_y_sin_presupuestos_no_hay_filas() {
        assert!(matches!(estado_del_mes(2026, 0, &[], &[]), Err(ErrorDominio::PeriodoInvalido { .. })));
        assert!(matches!(estado_del_mes(2026, 13, &[], &[]), Err(ErrorDominio::PeriodoInvalido { .. })));
        let r = estado_del_mes(2026, 9, &[fila(9, 1, "DOP", 5, false)], &[]).unwrap();
        assert!(r.filas.is_empty());
        assert_eq!(r.total_presupuesto.centavos(), 0);
    }

    #[test]
    fn un_gasto_negativo_o_cero_no_produce_porcentaje_negativo() {
        assert_eq!(milesimas_truncadas(-5, 100), 0);
        assert_eq!(milesimas_truncadas(0, 100), 0);
    }
}
