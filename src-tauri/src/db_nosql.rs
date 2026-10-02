use serde_json::Value;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

pub fn obtener_ruta_nosql(coleccion: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    format!("{}/.michelitos/databases/nosql/{}.json", home, coleccion)
}

/// Lee una colección.
///
/// **Ausente** y **ilegible** son cosas distintas. Un archivo que todavía no existe
/// da la estructura inicial (es el primer arranque). Uno que existe pero no se
/// puede leer o no es JSON válido da un **error**: devolver una colección vacía
/// haría que la interfaz mostrara «sin datos» y que el siguiente guardado
/// sobrescribiera, sin aviso, lo que había (A-01 de la auditoría del 2026-10-02).
pub fn leer_coleccion(coleccion: &str) -> Result<Value, String> {
    let path_str = obtener_ruta_nosql(coleccion);
    let path = Path::new(&path_str);

    if !path.exists() {
        if coleccion == "capital" {
            // Inicializar estructura básica de capital
            return Ok(serde_json::from_str(r#"{
                "propiedades": {
                    "inmobiliario": [],
                    "vehiculos": [],
                    "maquinaria": []
                },
                "certificados": [],
                "bolsa": []
            }"#).unwrap());
        }
        return Ok(Value::Array(Vec::new()));
    }

    let contenido = fs::read_to_string(path).map_err(|e| {
        format!("No se pudo leer «{coleccion}» ({path_str}): {e}. No se modificó nada; revisa el archivo o restaura un respaldo.")
    })?;
    serde_json::from_str(&contenido).map_err(|e| {
        format!("El archivo de «{coleccion}» ({path_str}) está dañado y no es JSON válido ({e}). No se modificó nada; restaura un respaldo.")
    })
}

pub fn guardar_coleccion(coleccion: &str, datos: &Value) -> Result<(), String> {
    let path_str = obtener_ruta_nosql(coleccion);
    let path = Path::new(&path_str);
    
    // Crear directorios si no existen
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // Ruta de archivo temporal en el mismo directorio para reemplazo atómico
    let temp_path_str = format!("{}.tmp", path_str);
    let temp_path = Path::new(&temp_path_str);

    {
        let mut file = File::create(temp_path).map_err(|e| e.to_string())?;
        let json_str = serde_json::to_string_pretty(datos).map_err(|e| e.to_string())?;
        file.write_all(json_str.as_bytes()).map_err(|e| e.to_string())?;
    }

    // Reemplazo atómico (rename en POSIX/macOS)
    fs::rename(temp_path, path).map_err(|e| e.to_string())?;

    Ok(())
}
