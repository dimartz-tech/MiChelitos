//! Comandos de respaldos de la base y del capital.

use crate::respaldo;

/// Toma un respaldo bajo demanda y devuelve dónde quedó.
///
/// Los respaldos automáticos solo se toman al preparar el esquema. Este
/// comando existe para el momento en que se va a hacer algo arriesgado a mano
/// —conciliar varios saldos, cargar un estado— y se quiere una red antes.
#[tauri::command]
pub fn crear_respaldo() -> Result<String, String> {
    respaldo::respaldar("manual")
        .map(|r| r.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}

/// Nombres de los respaldos disponibles, del más reciente al más antiguo.
#[tauri::command]
pub fn listar_respaldos() -> Vec<String> {
    respaldo::listar()
}

/// Devuelve la base y el capital al estado de un respaldo. Antes toma uno del
/// estado actual, de modo que la restauración se puede deshacer.
#[tauri::command]
pub fn restaurar_respaldo(nombre: String) -> Result<serde_json::Value, String> {
    respaldo::restaurar(&nombre)
        .map(|r| {
            serde_json::json!({
                "respaldo_de_seguridad": r.respaldo_de_seguridad.to_string_lossy(),
                "capital_restaurado": r.capital_restaurado,
                "version_del_esquema": r.version_del_esquema,
            })
        })
        .map_err(|e| e.to_string())
}
