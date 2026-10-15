//! Caso de uso: leer el rastro de correcciones.
//!
//! **No hay caso de uso para borrarlo.** Es deliberado: son el rastro de lo que se destruyó, y un rastro que se
//! puede borrar no es un rastro.

use super::ErrorAplicacion;
use crate::puertos::repositorios::{CasoLeido, ConsultaDeCorrecciones};

/// Los casos de corrección, del más reciente al más antiguo.
pub fn listar_correcciones(almacen: &impl ConsultaDeCorrecciones) -> Result<Vec<CasoLeido>, ErrorAplicacion> {
    Ok(almacen.casos()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::puertos::dobles::AlmacenEnMemoria;
    use crate::puertos::repositorios::{CasoAAnotar, RegistroDeCorrecciones};

    fn caso(descripcion: &str) -> CasoAAnotar {
        CasoAAnotar {
            tipo: "gasto".into(),
            referencia_id: 1,
            descripcion: descripcion.into(),
            importe: Some(10.0),
            divisa: Some("DOP".into()),
            motivo: "motivo de prueba suficiente".into(),
        }
    }

    #[test]
    fn sin_casos_la_lista_esta_vacia() {
        assert!(listar_correcciones(&AlmacenEnMemoria::nuevo()).unwrap().is_empty());
    }

    #[test]
    fn el_mas_reciente_va_primero() {
        let mut a = AlmacenEnMemoria::nuevo();
        a.anotar_caso(&caso("viejo")).unwrap();
        a.anotar_caso(&caso("nuevo")).unwrap();

        let lista = listar_correcciones(&a).unwrap();

        assert_eq!(lista.iter().map(|c| c.descripcion.as_str()).collect::<Vec<_>>(), ["nuevo", "viejo"]);
    }
}
