use serde_json::Value;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

pub fn obtener_ruta_nosql(coleccion: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    format!("{}/.michelitos/databases/nosql/{}.json", home, coleccion)
}

pub fn leer_coleccion(coleccion: &str) -> Value {
    let path_str = obtener_ruta_nosql(coleccion);
    let path = Path::new(&path_str);
    
    if !path.exists() {
        if coleccion == "capital" {
            // Inicializar estructura básica de capital
            return serde_json::from_str(r#"{
                "propiedades": {
                    "inmobiliario": [],
                    "vehiculos": [],
                    "maquinaria": []
                },
                "certificados": [],
                "bolsa": []
            }"#).unwrap();
        }
        return Value::Array(Vec::new());
    }

    let contenido = fs::read_to_string(path).unwrap_or_else(|_| "[]".to_string());
    serde_json::from_str(&contenido).unwrap_or_else(|_| {
        if coleccion == "capital" {
            serde_json::json!({
                "propiedades": {
                    "inmobiliario": [],
                    "vehiculos": [],
                    "maquinaria": []
                },
                "certificados": [],
                "bolsa": []
            })
        } else {
            Value::Array(Vec::new())
        }
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
