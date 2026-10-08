//! Caso de uso del listado de bonificaciones. (Registrar y revertir viven en `registrar_bonificacion`.)
//!
//! Lo fija la prueba de caracterización `bo1`.

use super::ErrorAplicacion;
use crate::puertos::repositorios::*;

/// Las bonificaciones, de la más nueva a la más vieja, con los datos de su tarjeta.
pub fn listar_bonificaciones(almacen: &impl ConsultaDeBonificaciones) -> Result<Vec<BonificacionLeida>, ErrorAplicacion> {
    Ok(almacen.bonificaciones()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aplicacion::registrar_bonificacion::{registrar_bonificacion, DatosBonificacion};
    use crate::dominio::bonificacion::Bonificacion;
    use crate::dominio::dinero::{Dinero, Divisa};
    use crate::puertos::dobles::AlmacenEnMemoria;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }

    #[test]
    fn el_listado_va_de_la_mas_nueva_a_la_mas_vieja_con_su_monto_divisa_y_concepto() {
        let mut a = AlmacenEnMemoria::nuevo().con_tarjeta(20, dop(10_000.0));
        for (concepto, monto) in [("Primera", 120.5), ("Segunda", 5.0)] {
            let bonificacion = Bonificacion::nueva(dop(monto), concepto).unwrap();
            registrar_bonificacion(DatosBonificacion { fecha: "01/10/2026".into(), tarjeta_id: 20, bonificacion, gasto_id: None }, &mut a).unwrap();
        }
        let lista = listar_bonificaciones(&a).unwrap();
        assert_eq!(lista.iter().map(|b| b.concepto.as_str()).collect::<Vec<_>>(), vec!["Segunda", "Primera"]);
        assert_eq!((lista[1].monto, lista[1].divisa.as_str(), lista[1].tarjeta_id), (120.5, "DOP", 20));
    }

    #[test]
    fn sin_bonificaciones_la_lista_esta_vacia() {
        assert!(listar_bonificaciones(&AlmacenEnMemoria::nuevo()).unwrap().is_empty());
    }
}
