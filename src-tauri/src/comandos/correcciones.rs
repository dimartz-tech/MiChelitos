//! Comando del registro de casos de corrección.

use serde::{Deserialize, Serialize};

use super::ayudantes::lee_almacen;
use crate::aplicacion;

#[derive(Serialize, Deserialize, Debug)]
pub struct CasoCorreccion {
    pub numero_caso: String,
    pub fecha: String,
    pub tipo: String,
    pub referencia_id: i64,
    pub descripcion: String,
    pub importe: Option<f64>,
    pub divisa: Option<String>,
    pub motivo: String,
}

/// Los casos de corrección, del más reciente al más antiguo.
///
/// **No hay comando para borrarlos.** Es deliberado: son el rastro de lo que
/// se destruyó, y un rastro que se puede borrar no es un rastro.
#[tauri::command]
pub fn obtener_correcciones() -> Result<Vec<CasoCorreccion>, String> {
    lee_almacen(|a| {
        Ok(aplicacion::correcciones::listar_correcciones(a)?
            .into_iter()
            .map(|c| CasoCorreccion {
                numero_caso: c.numero_caso,
                fecha: c.fecha,
                tipo: c.tipo,
                referencia_id: c.referencia_id,
                descripcion: c.descripcion,
                importe: c.importe,
                divisa: c.divisa,
                motivo: c.motivo,
            })
            .collect())
    })
}
