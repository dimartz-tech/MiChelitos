//! Caso de uso: el resumen de gastos por categoría y mes. **Solo lectura.**
//!
//! Lee los gastos agrupados por el puerto y deja las cuentas al dominio (`dominio::resumen_gastos`). No escribe nada.

use super::ErrorAplicacion;
use crate::dominio::dinero::TasaCambio;
use crate::dominio::resumen_gastos::{resumen_por_mes, OpcionesResumen, ResumenMensual};
use crate::puertos::repositorios::ConsultaDeGastosPorCategoria;
use chrono::NaiveDate;

pub struct DatosDeResumen {
    /// `(año, mes)` pedido.
    pub mes: (i32, u32),
    pub hoy: NaiveDate,
    pub n_promedio: u32,
    /// Tasa de referencia DOP por 1 USD para los dólares sin liquidar.
    pub tasa_usd: TasaCambio,
    pub excluir_categorias: Vec<i64>,
}

pub fn obtener_resumen_gastos(
    datos: DatosDeResumen,
    almacen: &impl ConsultaDeGastosPorCategoria,
) -> Result<ResumenMensual, ErrorAplicacion> {
    let filas = almacen.gastos_agrupados()?;
    Ok(resumen_por_mes(
        &filas,
        datos.mes,
        datos.hoy,
        datos.n_promedio,
        datos.tasa_usd,
        &OpcionesResumen { excluir_categorias: datos.excluir_categorias },
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dominio::resumen_gastos::FilaAgregadoGasto;
    use crate::puertos::dobles::AlmacenEnMemoria;

    fn fila(mes: u32, cat: i64, centavos: i64) -> FilaAgregadoGasto {
        FilaAgregadoGasto {
            anio: 2026,
            mes,
            dia: 3,
            categoria_id: cat,
            categoria_nombre: format!("Cat {cat}"),
            divisa: "DOP".into(),
            derivado: false,
            centavos,
            liquidado_origen_centavos: 0,
            liquidado_dop_centavos: None,
            n: 1,
        }
    }

    fn datos(mes: u32) -> DatosDeResumen {
        DatosDeResumen {
            mes: (2026, mes),
            hoy: NaiveDate::from_ymd_opt(2026, 10, 28).unwrap(),
            n_promedio: 2,
            tasa_usd: TasaCambio::nueva(60.0).unwrap(),
            excluir_categorias: vec![],
        }
    }

    #[test]
    fn lee_del_puerto_y_calcula_en_el_dominio() {
        let a = AlmacenEnMemoria::nuevo().con_filas_agregadas(vec![fila(7, 1, 100_001), fila(8, 1, 200_003), fila(9, 1, 318_729)]);
        let r = obtener_resumen_gastos(datos(9), &a).unwrap();
        assert_eq!(r.filas.len(), 1);
        let m = &r.filas[0].medidas;
        assert_eq!(m.importe.centavos(), 318_729);
        assert_eq!(m.anterior.centavos(), 200_003);
        assert_eq!(m.promedio.unwrap().centavos(), 150_002); // (1000.01 + 2000.03) / 2 = 1500.02
    }

    #[test]
    fn un_periodo_invalido_llega_como_error_de_dominio() {
        let a = AlmacenEnMemoria::nuevo();
        let e = obtener_resumen_gastos(datos(11), &a).unwrap_err();
        assert!(matches!(e, ErrorAplicacion::Dominio(_)), "{e:?}");
    }

    #[test]
    fn las_exclusiones_llegan_al_dominio() {
        let a = AlmacenEnMemoria::nuevo().con_filas_agregadas(vec![fila(9, 1, 100_001), fila(9, 2, 200_003)]);
        let mut d = datos(9);
        d.excluir_categorias = vec![2];
        let r = obtener_resumen_gastos(d, &a).unwrap();
        assert_eq!(r.totales.importe.centavos(), 100_001);
    }
}
