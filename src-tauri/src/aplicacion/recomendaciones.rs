//! Caso de uso: las recomendaciones del historial del patrimonio. **Solo lectura.**
//!
//! Lee las fotos mensuales, los gastos agrupados, los presupuestos y la situación actual (cuentas, tarjetas) por sus
//! puertos y deja todas las reglas al dominio (`dominio::recomendaciones`). No escribe nada y no suma ni compara dinero.

use super::ErrorAplicacion;
use crate::dominio::recomendaciones::{evaluar, Entrada, FotoResumen, Recomendaciones};
use crate::puertos::repositorios::*;
use chrono::NaiveDate;

fn a_resumen(f: FotoLeida) -> FotoResumen {
    FotoResumen {
        anio: f.anio,
        mes: f.mes,
        activos: f.activos,
        pasivos: f.pasivos,
        patrimonio: f.patrimonio,
        tasa_usd_dop: f.tasa_usd_dop,
        calidad: f.calidad,
    }
}

/// Las recomendaciones a fecha `hoy`.
pub fn obtener_recomendaciones(
    hoy: NaiveDate,
    fotos: &impl RepositorioDeFotos,
    almacen: &(impl ConsultaDeGastosPorCategoria + ConsultaDePresupuestos + LecturaDePatrimonio),
) -> Result<Recomendaciones, ErrorAplicacion> {
    let fotos: Vec<FotoResumen> = fotos.fotos()?.into_iter().map(a_resumen).collect();
    let gastos = almacen.gastos_agrupados()?;
    let presupuestos = almacen.presupuestos()?;
    let cuentas = almacen.cuentas_de_patrimonio()?;
    let tarjetas = almacen.tarjetas_de_patrimonio()?;
    Ok(evaluar(&Entrada {
        hoy,
        fotos: &fotos,
        gastos: &gastos,
        presupuestos: &presupuestos,
        cuentas: &cuentas,
        tarjetas: &tarjetas,
    })?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dominio::dinero::{Dinero, Divisa, TasaCambio};
    use crate::dominio::patrimonio::{CuentaDePatrimonio, PrestamoDePatrimonio, TarjetaDePatrimonio};
    use crate::dominio::presupuesto::Presupuesto;
    use crate::dominio::recomendaciones::Severidad;
    use crate::dominio::resumen_gastos::FilaAgregadoGasto;
    use crate::puertos::dobles::FotosEnMemoria;

    /// Un almacén de solo lectura con lo que la prueba le ponga.
    #[derive(Default)]
    struct Lecturas {
        gastos: Vec<FilaAgregadoGasto>,
        presupuestos: Vec<Presupuesto>,
        cuentas: Vec<CuentaDePatrimonio>,
        tarjetas: Vec<TarjetaDePatrimonio>,
        fallar_gastos: bool,
    }

    impl ConsultaDeGastosPorCategoria for Lecturas {
        fn gastos_agrupados(&self) -> Result<Vec<FilaAgregadoGasto>, ErrorAlmacen> {
            if self.fallar_gastos {
                return Err(ErrorAlmacen::Fallo("fallo simulado".into()));
            }
            Ok(self.gastos.clone())
        }
    }
    impl ConsultaDePresupuestos for Lecturas {
        fn presupuestos(&self) -> Result<Vec<Presupuesto>, ErrorAlmacen> {
            Ok(self.presupuestos.clone())
        }
    }
    impl LecturaDePatrimonio for Lecturas {
        fn cuentas_de_patrimonio(&self) -> Result<Vec<CuentaDePatrimonio>, ErrorAlmacen> {
            Ok(self.cuentas.clone())
        }
        fn tarjetas_de_patrimonio(&self) -> Result<Vec<TarjetaDePatrimonio>, ErrorAlmacen> {
            Ok(self.tarjetas.clone())
        }
        fn prestamos_de_patrimonio(&self) -> Result<Vec<PrestamoDePatrimonio>, ErrorAlmacen> {
            Ok(vec![])
        }
    }

    fn hoy() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 6).unwrap()
    }

    fn foto(mes: u32, activos: i64, pasivos: i64) -> FotoLeida {
        let dop = |c| Dinero::de_centavos(c, Divisa::Dop);
        FotoLeida {
            id: mes as i64,
            anio: 2026,
            mes,
            fecha: format!("10/{mes:02}/2026"),
            origen: "real".into(),
            sellada: false,
            tasa_usd_dop: TasaCambio::nueva(60.0).unwrap(),
            activos: dop(activos),
            pasivos: dop(pasivos),
            patrimonio: dop(activos - pasivos),
            liquido: dop(0),
            calidad: "completa".into(),
        }
    }

    fn fotos(v: Vec<FotoLeida>) -> FotosEnMemoria {
        FotosEnMemoria { guardadas: v.into_iter().map(|f| (f, vec![])).collect(), ..Default::default() }
    }

    #[test]
    fn junta_fotos_gastos_presupuestos_y_situacion_y_deja_las_reglas_al_dominio() {
        let f = fotos(vec![foto(10, 505_001, 250_003), foto(9, 500_001, 100_003)]);
        let a = Lecturas {
            gastos: vec![FilaAgregadoGasto {
                anio: 2026,
                mes: 11,
                dia: 2,
                categoria_id: 4,
                categoria_nombre: "Ocio inv.".into(),
                divisa: "DOP".into(),
                derivado: false,
                fija: false,
                centavos: 60_001,
                liquidado_origen_centavos: 0,
                liquidado_dop_centavos: None,
                n: 1,
            }],
            presupuestos: vec![Presupuesto { categoria_id: 4, categoria_nombre: "Ocio inv.".into(), fija: false, centavos: 50_000 }],
            ..Default::default()
        };
        let r = obtener_recomendaciones(hoy(), &f, &a).unwrap();
        let ids: Vec<&str> = r.visibles.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, vec!["presupuesto_excedido:4", "variacion_patrimonio:2026-10"]);
        assert!(r.visibles.iter().all(|x| x.severidad == Severidad::Atencion));
    }

    #[test]
    fn no_escribe_nada_en_las_fotos() {
        let f = fotos(vec![foto(10, 505_001, 250_003), foto(9, 500_001, 100_003)]);
        obtener_recomendaciones(hoy(), &f, &Lecturas::default()).unwrap();
        assert_eq!(f.escrituras, 0);
        assert_eq!(f.guardadas.len(), 2);
    }

    #[test]
    fn un_fallo_del_almacen_llega_como_error_de_almacen_sin_resultado_parcial() {
        let f = fotos(vec![]);
        let a = Lecturas { fallar_gastos: true, ..Default::default() };
        let e = obtener_recomendaciones(hoy(), &f, &a).unwrap_err();
        assert!(matches!(e, ErrorAplicacion::Almacen(ErrorAlmacen::Fallo(_))), "{e:?}");
    }

    #[test]
    fn sin_ningun_dato_devuelve_todo_sin_datos_y_ninguna_recomendacion() {
        let r = obtener_recomendaciones(hoy(), &fotos(vec![]), &Lecturas::default()).unwrap();
        assert!(r.visibles.is_empty());
        assert_eq!(r.reglas_evaluadas, 0);
        assert_eq!(r.sin_datos.len(), 6);
    }
}
