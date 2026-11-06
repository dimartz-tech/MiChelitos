//! Casos de uso de los presupuestos mensuales por categoría: fijar, quitar y ver el estado de un mes.
//!
//! Fijar y quitar escriben (la transacción la abre quien llama); el estado **solo lee**. Las cuentas están en
//! `dominio::presupuesto`. Este módulo no suma ni compara dinero.

use super::ErrorAplicacion;
use crate::dominio::presupuesto::{self, EstadoDelMes, Presupuesto};
use crate::puertos::repositorios::*;

/// Fija (crea o reemplaza) el presupuesto mensual de una categoría. Una categoría inexistente es `NoEncontrado`; un monto
/// que no es positivo o pasa del tope, un error de dominio, y en ambos casos no se escribe nada. La marca «fija» de la
/// categoría no se toca.
pub fn fijar_presupuesto(
    categoria_id: i64,
    centavos: i64,
    almacen: &mut impl AlmacenCatalogos,
) -> Result<Presupuesto, ErrorAplicacion> {
    let categoria_nombre = almacen.nombre_de_categoria(categoria_id)?;
    let centavos = presupuesto::validar_monto(centavos)?;
    almacen.fijar_presupuesto(categoria_id, centavos)?;
    let fija = almacen.categorias()?.iter().find(|c| c.id == categoria_id).map(|c| c.fija).unwrap_or(false);
    Ok(Presupuesto { categoria_id, categoria_nombre, fija, centavos })
}

/// Quita el presupuesto de una categoría; devuelve si lo había (quitar uno inexistente no es un error).
pub fn quitar_presupuesto(categoria_id: i64, almacen: &mut impl AlmacenCatalogos) -> Result<bool, ErrorAplicacion> {
    Ok(almacen.quitar_presupuesto(categoria_id)?)
}

/// El estado de los presupuestos en el mes `(anio, mes)`. **Solo lectura.**
pub fn estado_de_presupuestos(
    anio: i32,
    mes: u32,
    gastos: &impl ConsultaDeGastosPorCategoria,
    presupuestos: &impl ConsultaDePresupuestos,
) -> Result<EstadoDelMes, ErrorAplicacion> {
    let filas = gastos.gastos_agrupados()?;
    let fijados = presupuestos.presupuestos()?;
    Ok(presupuesto::estado_del_mes(anio, mes, &filas, &fijados)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dominio::errores::ErrorDominio;
    use crate::dominio::presupuesto::EstadoPresupuesto;
    use crate::dominio::resumen_gastos::FilaAgregadoGasto;
    use crate::puertos::dobles::{AlmacenEnMemoria, CatalogosEnMemoria};

    fn almacen() -> CatalogosEnMemoria {
        CatalogosEnMemoria::nuevo().con_categoria("Comida").con_categoria("Ocio")
    }

    fn fila(cat: i64, divisa: &str, centavos: i64) -> FilaAgregadoGasto {
        FilaAgregadoGasto {
            anio: 2026,
            mes: 9,
            dia: 4,
            categoria_id: cat,
            categoria_nombre: "x".into(),
            divisa: divisa.into(),
            derivado: false,
            fija: false,
            centavos,
            liquidado_origen_centavos: 0,
            liquidado_dop_centavos: None,
            n: 1,
        }
    }

    #[test]
    fn fijar_crea_y_volver_a_fijar_reemplaza_sin_duplicar() {
        let mut a = almacen();
        let comida = a.id_de_categoria("Comida");
        let p = fijar_presupuesto(comida, 45_123, &mut a).unwrap();
        assert_eq!((p.categoria_nombre.as_str(), p.centavos, p.fija), ("Comida", 45_123, false));
        fijar_presupuesto(comida, 50_007, &mut a).unwrap();
        assert_eq!(a.presupuestos.len(), 1);
        assert_eq!(a.presupuestos[&comida], 50_007);
    }

    #[test]
    fn fijar_rechaza_montos_invalidos_y_categorias_inexistentes_sin_escribir() {
        let mut a = almacen();
        let comida = a.id_de_categoria("Comida");
        for m in [0, -1, i64::MIN] {
            let e = fijar_presupuesto(comida, m, &mut a).unwrap_err();
            assert!(matches!(e, ErrorAplicacion::Dominio(ErrorDominio::PresupuestoNoPositivo)), "{e:?}");
        }
        let e = fijar_presupuesto(comida, crate::dominio::presupuesto::TOPE_CENTAVOS + 1, &mut a).unwrap_err();
        assert!(matches!(e, ErrorAplicacion::Dominio(ErrorDominio::PresupuestoExcedeElTope)), "{e:?}");
        let e = fijar_presupuesto(999, 100, &mut a).unwrap_err();
        assert!(matches!(e, ErrorAplicacion::Almacen(ErrorAlmacen::NoEncontrado { id: 999, .. })), "{e:?}");
        assert!(a.presupuestos.is_empty());
    }

    #[test]
    fn fijar_en_una_categoria_fija_funciona_y_no_cambia_su_marca() {
        let mut a = almacen();
        let comida = a.id_de_categoria("Comida");
        a.categorias.iter_mut().find(|c| c.id == comida).unwrap().fija = true;
        let p = fijar_presupuesto(comida, 777, &mut a).unwrap();
        assert!(p.fija);
        assert!(a.categorias.iter().find(|c| c.id == comida).unwrap().fija);
    }

    #[test]
    fn quitar_dice_si_habia_y_es_idempotente() {
        let mut a = almacen();
        let (comida, ocio) = (a.id_de_categoria("Comida"), a.id_de_categoria("Ocio"));
        fijar_presupuesto(comida, 500, &mut a).unwrap();
        fijar_presupuesto(ocio, 600, &mut a).unwrap();
        assert!(quitar_presupuesto(comida, &mut a).unwrap());
        assert!(!quitar_presupuesto(comida, &mut a).unwrap());
        assert_eq!(a.presupuestos.keys().copied().collect::<Vec<_>>(), vec![ocio]);
        let e = quitar_presupuesto(999, &mut a).unwrap_err();
        assert!(matches!(e, ErrorAplicacion::Almacen(ErrorAlmacen::NoEncontrado { id: 999, .. })), "{e:?}");
    }

    #[test]
    fn el_estado_une_gastos_y_presupuestos_y_no_escribe() {
        let mut c = almacen();
        let (comida, ocio) = (c.id_de_categoria("Comida"), c.id_de_categoria("Ocio"));
        fijar_presupuesto(comida, 10_000, &mut c).unwrap();
        fijar_presupuesto(ocio, 5_000, &mut c).unwrap();
        let g = AlmacenEnMemoria::default().con_filas_agregadas(vec![fila(comida, "DOP", 8_123), fila(ocio, "DOP", 5_017), fila(ocio, "USD", 1_199)]);
        let antes = c.clone();
        let r = estado_de_presupuestos(2026, 9, &g, &c).unwrap();
        assert_eq!(r.filas.len(), 2);
        assert_eq!((r.filas[0].categoria_nombre.as_str(), r.filas[0].estado), ("Ocio", EstadoPresupuesto::Excedido));
        assert_eq!((r.filas[1].categoria_nombre.as_str(), r.filas[1].estado), ("Comida", EstadoPresupuesto::Cerca));
        assert_eq!(r.filas[0].usd_aparte.centavos(), 1_199);
        assert_eq!(c.presupuestos, antes.presupuestos);
    }

    #[test]
    fn el_estado_de_un_mes_invalido_es_un_error_de_dominio() {
        let c = almacen();
        let g = AlmacenEnMemoria::default();
        let e = estado_de_presupuestos(2026, 13, &g, &c).unwrap_err();
        assert!(matches!(e, ErrorAplicacion::Dominio(ErrorDominio::PeriodoInvalido { .. })), "{e:?}");
    }
}
