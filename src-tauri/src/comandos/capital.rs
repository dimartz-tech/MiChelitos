//! Comandos del capital (colección NoSQL): lectura con alertas de vencimiento y guardado.

use chrono::{Local, NaiveDate};
use serde_json::Value;

use crate::{aplicacion, db_nosql, dominio};

// --- COMANDOS: CAPITAL (NoSQL) ---
#[tauri::command]
pub fn obtener_capital() -> Result<Value, String> {
    let mut data = db_nosql::leer_coleccion("capital")?;
    let hoy = Local::now().naive_local().date();

    for coleccion in ["certificados", "bolsa"] {
        if let Some(entradas) = data.get_mut(coleccion).and_then(|v| v.as_array_mut()) {
            for entrada in entradas {
                marcar_alerta_de_vencimiento(entrada, hoy);
            }
        }
    }

    // Los totales se suman aquí, en centavos, y no en la vista: una suma de
    // decimales en JavaScript arrastra ruido y es una regla sin pruebas.
    let totales = aplicacion::guardar_capital::totales_de(&data);
    data["totales"] = aplicacion::guardar_capital::totales_como_json(&totales);

    Ok(data)
}

/// Añade al `Value` de una entrada las tres claves de presentación que
/// dependen de la fecha de hoy. **Ninguna sobrevive a un guardado**: ver
/// `retirar_campos_calculados` y `dominio::capital`.
fn marcar_alerta_de_vencimiento(entrada: &mut Value, hoy: NaiveDate) {
    entrada["alerta_vencimiento"] = serde_json::Value::Bool(false);
    let Some(venc_str) = entrada.get("vencimiento").and_then(|v| v.as_str()) else {
        return;
    };
    let Ok(venc_date) = NaiveDate::parse_from_str(venc_str, "%d/%m/%Y") else {
        return;
    };
    let alerta = dominio::capital::calcular(venc_date, hoy);
    entrada["dias_restantes"] = serde_json::Value::Number(alerta.dias_restantes.into());
    entrada["alerta_vencimiento"] = serde_json::Value::Bool(alerta.activa());
    if let Some(mensaje) = alerta.mensaje() {
        entrada["alerta_msg"] = serde_json::Value::String(mensaje);
    }
}

#[tauri::command]
pub fn guardar_capital(data: Value) -> Result<(), String> {
    // Se exige lo que entra o cambia contra lo que ya estaba guardado; ver
    // `aplicacion::guardar_capital`.
    // Si el archivo existe pero no se puede leer, **no se guarda**: sustituirlo por lo que llegue destruiría lo que hubiera.
    let guardado = db_nosql::leer_coleccion("capital")?;
    let limpio = aplicacion::guardar_capital::preparar_para_guardar(data, &guardado)
        .map_err(|e| e.to_string())?;
    db_nosql::guardar_coleccion("capital", &limpio)
}
