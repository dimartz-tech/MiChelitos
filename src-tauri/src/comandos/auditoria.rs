//! Comando de exportación para auditoría.

use crate::exportacion;
use serde::{Deserialize, Serialize};

/// Lo que se le dice a la interfaz tras exportar: dónde quedó y cuánto lleva.
#[derive(Serialize, Deserialize, Debug)]
pub struct ExportacionAuditoria {
    /// Ruta de la carpeta, para mostrarla: la ventana no abre archivos.
    pub carpeta: String,
    /// Archivos de datos (un CSV por tabla, más el capital si existe); sin contar el manifiesto ni las huellas.
    pub archivos: u32,
    /// Filas en total, sumando todas las tablas.
    pub filas: u32,
    /// SHA-256 del manifiesto. Anotada fuera de la carpeta, delata que la carpeta entera se rehízo.
    pub huella_del_manifiesto: String,
}

/// Escribe la base y el capital en archivos legibles (un CSV por tabla), con un manifiesto y la huella de cada uno.
///
/// No modifica nada: lee todas las tablas en una sola transacción de lectura. Cada llamada deja una carpeta nueva y
/// fechada; las anteriores no se tocan ni se podan.
#[tauri::command]
pub fn exportar_auditoria() -> Result<ExportacionAuditoria, String> {
    exportacion::exportar()
        .map(|e| ExportacionAuditoria {
            carpeta: e.carpeta.to_string_lossy().to_string(),
            archivos: e.archivos.len() as u32,
            filas: e.filas() as u32,
            huella_del_manifiesto: e.huella_del_manifiesto,
        })
        .map_err(|e| e.to_string())
}
