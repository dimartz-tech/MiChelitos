// Prevención de ventana de consola en Windows en producción
#![cfg_attr(
  all(not(debug_assertions), target_os = "windows"),
  windows_subsystem = "windows"
)]

mod db_sql;
mod db_nosql;

use serde::{Serialize, Deserialize};
use serde_json::Value;
use chrono::{NaiveDate, Local, Datelike};

// --- ESTRUCTURAS DTO (DATA TRANSFER OBJECTS) ---
#[derive(Serialize, Deserialize, Debug)]
pub struct Cliente {
    id: i64,
    rnc: String,
    nombre: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Ingreso {
    id: i64,
    numero_factura: String,
    cliente_id: i64,
    cliente_nombre: String,
    cliente_rnc: String,
    fecha_emision: String,
    estatus: String,
    monto_total: f64,
    porcentaje_retencion: f64,
    monto_retenido: f64,
    institucion_deposito: Option<String>,
    fecha_pago: Option<String>,
    monto_recibido: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IngresoInformal {
    id: i64,
    fecha: String,
    descripcion: String,
    monto: f64,
    estatus: String,
    institucion_deposito: Option<String>,
    fecha_pago: Option<String>,
    monto_recibido: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Gasto {
    id: i64,
    fecha: String,
    monto: f64,
    divisa: String,
    descripcion: String,
    categoria_id: i64,
    categoria_nombre: String,
    metodo_pago: String,
    costo_adicional: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Categoria {
    id: i64,
    nombre: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Tarjeta {
    id: i64,
    entidad: String,
    nombre_tarjeta: String,
    limite: f64,
    balance_actual: f64,
    fecha_corte: i32,
    fecha_limite_pago: i32,
    // Enriquecidos
    alerta_corte: bool,
    alerta_pago: bool,
    dias_corte_msg: String,
    dias_pago_msg: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Suscripcion {
    id: i64,
    plataforma: String,
    monto: f64,
    tarjeta_id: i64,
    frecuencia: String,
    entidad: String,
    nombre_tarjeta: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Prestamo {
    id: i64,
    tipo_prestamo: String,
    monto_prestamo: f64,
    institucion_financiera: String,
    tasa_actual: f64,
    cuotas_totales: Option<i32>,
    cuotas_pendientes: Option<i32>,
    monto_cuota: f64,
    dia_pago: i32,
    // Enriquecidos
    alerta_pago: bool,
    dias_pago_msg: String,
}

// --- COMANDOS: CATEGORÍAS ---
#[tauri::command]
fn obtener_categorias() -> Result<Vec<Categoria>, String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT id, nombre FROM categorias ORDER BY nombre ASC;").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| {
        Ok(Categoria {
            id: row.get(0)?,
            nombre: row.get(1)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r.map_err(|e| e.to_string())?);
    }
    Ok(list)
}

#[tauri::command]
fn crear_categoria(nombre: String) -> Result<Categoria, String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let nombre_clean = nombre.trim();
    if nombre_clean.is_empty() {
        return Err("El nombre de la categoría no puede estar vacío.".to_string());
    }

    // Verificar duplicado
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM categorias WHERE LOWER(nombre) = LOWER(?);",
        [nombre_clean],
        |r| r.get(0),
    ).map_err(|e| e.to_string())?;

    if count > 0 {
        return Err("La categoría ya existe.".to_string());
    }

    conn.execute("INSERT INTO categorias (nombre) VALUES (?);", [nombre_clean]).map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();

    Ok(Categoria {
        id,
        nombre: nombre_clean.to_string(),
    })
}

#[tauri::command]
fn eliminar_categoria(id: i64) -> Result<(), String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;

    // Verificar si es "Otros"
    let nombre: String = conn.query_row(
        "SELECT nombre FROM categorias WHERE id = ?;",
        [id],
        |r| r.get(0)
    ).map_err(|e| e.to_string())?;
    if nombre == "Otros" {
        return Err("No se puede eliminar la categoría de sistema 'Otros'.".to_string());
    }

    // Verificar integridad referencial de gastos
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM gastos WHERE categoria_id = ?;",
        [id],
        |r| r.get(0)
    ).map_err(|e| e.to_string())?;

    if count > 0 {
        return Err("No se puede eliminar la categoría porque tiene gastos registrados asociados.".to_string());
    }

    conn.execute("DELETE FROM categorias WHERE id = ?;", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

// --- COMANDOS: GASTOS ---
#[tauri::command]
fn obtener_gastos() -> Result<Vec<Gasto>, String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT g.id, g.fecha, g.monto, g.divisa, g.descripcion, g.categoria_id, c.nombre, g.metodo_pago, g.costo_adicional
         FROM gastos g
         JOIN categorias c ON g.categoria_id = c.id
         ORDER BY g.id DESC;"
    ).map_err(|e| e.to_string())?;

    let rows = stmt.query_map([], |row| {
        Ok(Gasto {
            id: row.get(0)?,
            fecha: row.get(1)?,
            monto: row.get(2)?,
            divisa: row.get(3)?,
            descripcion: row.get(4)?,
            categoria_id: row.get(5)?,
            categoria_nombre: row.get(6)?,
            metodo_pago: row.get(7)?,
            costo_adicional: row.get(8)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r.map_err(|e| e.to_string())?);
    }
    Ok(list)
}

#[derive(Deserialize)]
struct GastoInput {
    fecha: String,
    monto: f64,
    divisa: String,
    descripcion: String,
    categoria_id: i64,
    metodo_pago: String,
    es_lbtr: bool,
    tarjeta_id: Option<i64>,
}

#[tauri::command]
fn crear_gasto(input: GastoInput) -> Result<i64, String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    
    // Calcular comisiones
    let mut costo_adicional = 0.0;
    if input.metodo_pago == "transferencia" {
        costo_adicional = (input.monto * 0.002).round();
        if input.es_lbtr {
            costo_adicional += 100.00;
        }
    }

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // Si el pago es por tarjeta, debitar/sumar al balance actual de la tarjeta
    if input.metodo_pago == "tarjeta" {
        if let Some(t_id) = input.tarjeta_id {
            tx.execute(
                "UPDATE tarjetas SET balance_actual = balance_actual + ? WHERE id = ?;",
                [input.monto, t_id as f64],
            ).map_err(|e| e.to_string())?;
        }
    }

    tx.execute(
        "INSERT INTO gastos (fecha, monto, divisa, descripcion, categoria_id, metodo_pago, costo_adicional, tarjeta_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?);",
        (
            &input.fecha,
            input.monto,
            &input.divisa,
            &input.descripcion,
            input.categoria_id,
            &input.metodo_pago,
            costo_adicional,
            input.tarjeta_id,
        )
    ).map_err(|e| e.to_string())?;

    let gasto_id = tx.last_insert_rowid();
    tx.commit().map_err(|e| e.to_string())?;

    Ok(gasto_id)
}

// --- COMANDOS: INGRESOS FORMALES ---
#[tauri::command]
fn obtener_ingresos() -> Result<Vec<Ingreso>, String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT i.id, i.numero_factura, i.cliente_id, c.nombre, c.rnc, i.fecha_emision, i.estatus,
                i.monto_total, i.porcentaje_retencion, i.monto_retenido, i.institucion_deposito, i.fecha_pago, i.monto_recibido
         FROM ingresos i
         JOIN clientes c ON i.cliente_id = c.id
         ORDER BY i.id DESC;"
    ).map_err(|e| e.to_string())?;

    let rows = stmt.query_map([], |row| {
        Ok(Ingreso {
            id: row.get(0)?,
            numero_factura: row.get(1)?,
            cliente_id: row.get(2)?,
            cliente_nombre: row.get(3)?,
            cliente_rnc: row.get(4)?,
            fecha_emision: row.get(5)?,
            estatus: row.get(6)?,
            monto_total: row.get(7)?,
            porcentaje_retencion: row.get(8)?,
            monto_retenido: row.get(9)?,
            institucion_deposito: row.get(10)?,
            fecha_pago: row.get(11)?,
            monto_recibido: row.get(12)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r.map_err(|e| e.to_string())?);
    }
    Ok(list)
}

#[derive(Deserialize)]
struct IngresoInput {
    numero_factura: String,
    rnc_cliente: String,
    nombre_cliente: String,
    fecha_emision: String,
    monto_total: f64,
    porcentaje_retencion: f64,
}

#[tauri::command]
fn crear_ingreso(input: IngresoInput) -> Result<i64, String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    
    // Verificar duplicado de factura
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM ingresos WHERE LOWER(numero_factura) = LOWER(?);",
        [&input.numero_factura],
        |r| r.get(0)
    ).map_err(|e| e.to_string())?;

    if count > 0 {
        return Err("El número de factura ya está registrado.".to_string());
    }

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // Obtener o crear cliente
    let cliente_id: i64 = match tx.query_row(
        "SELECT id FROM clientes WHERE rnc = ?;",
        [&input.rnc_cliente],
        |r| r.get(0)
    ) {
        Ok(id) => id,
        Err(_) => {
            tx.execute(
                "INSERT INTO clientes (rnc, nombre) VALUES (?, ?);",
                [&input.rnc_cliente, &input.nombre_cliente]
            ).map_err(|e| e.to_string())?;
            tx.last_insert_rowid()
        }
    };

    let monto_retenido = (input.monto_total * (input.porcentaje_retencion / 100.0)).round();

    tx.execute(
        "INSERT INTO ingresos (numero_factura, cliente_id, fecha_emision, monto_total, porcentaje_retencion, monto_retenido, estatus)
         VALUES (?, ?, ?, ?, ?, ?, 'emitida');",
        (
            &input.numero_factura,
            cliente_id,
            &input.fecha_emision,
            input.monto_total,
            input.porcentaje_retencion,
            monto_retenido,
        )
    ).map_err(|e| e.to_string())?;

    let id = tx.last_insert_rowid();
    tx.commit().map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
fn marcar_ingreso_pagado(id: i64, institucion: String, fecha: String, monto_recibido: f64) -> Result<(), String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE ingresos SET estatus = 'pagada', institucion_deposito = ?, fecha_pago = ?, monto_recibido = ? WHERE id = ?;",
        (institucion, fecha, monto_recibido, id)
    ).map_err(|e| e.to_string())?;
    Ok(())
}

// --- COMANDOS: INGRESOS INFORMALES ---
#[tauri::command]
fn obtener_ingresos_informales() -> Result<Vec<IngresoInformal>, String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, fecha, descripcion, monto, estatus, institucion_deposito, fecha_pago, monto_recibido
         FROM ingresos_informales ORDER BY id DESC;"
    ).map_err(|e| e.to_string())?;

    let rows = stmt.query_map([], |row| {
        Ok(IngresoInformal {
            id: row.get(0)?,
            fecha: row.get(1)?,
            descripcion: row.get(2)?,
            monto: row.get(3)?,
            estatus: row.get(4)?,
            institucion_deposito: row.get(5)?,
            fecha_pago: row.get(6)?,
            monto_recibido: row.get(7)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r.map_err(|e| e.to_string())?);
    }
    Ok(list)
}

#[tauri::command]
fn crear_ingreso_informal(fecha: String, descripcion: String, monto: f64) -> Result<i64, String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO ingresos_informales (fecha, descripcion, monto, estatus) VALUES (?, ?, ?, 'pendiente');",
        (&fecha, &descripcion, monto)
    ).map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn marcar_informal_pagado(id: i64, institucion: String, fecha: String, monto_recibido: f64) -> Result<(), String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE ingresos_informales SET estatus = 'pagado', institucion_deposito = ?, fecha_pago = ?, monto_recibido = ? WHERE id = ?;",
        (institucion, fecha, monto_recibido, id)
    ).map_err(|e| e.to_string())?;
    Ok(())
}

// --- COMANDOS: TARJETAS ---
#[tauri::command]
fn obtener_tarjetas() -> Result<Vec<Tarjeta>, String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT id, entidad, nombre_tarjeta, limite, balance_actual, fecha_corte, fecha_limite_pago FROM tarjetas;").map_err(|e| e.to_string())?;
    
    let hoy = Local::now();
    let dia_actual = hoy.day() as i32;

    let rows = stmt.query_map([], |row| {
        let id: i64 = row.get(0)?;
        let entidad: String = row.get(1)?;
        let nombre_tarjeta: String = row.get(2)?;
        let limite: f64 = row.get(3)?;
        let balance_actual: f64 = row.get(4)?;
        let fecha_corte: i32 = row.get(5)?;
        let fecha_limite_pago: i32 = row.get(6)?;

        // Calcular alertas corte
        let dias_corte = if fecha_corte >= dia_actual {
            fecha_corte - dia_actual
        } else {
            // Asumir un mes promedio de 30 días para cálculo de recordatorio aproximado
            (30 - dia_actual) + fecha_corte
        };

        // Calcular alertas pago
        let dias_pago = if fecha_limite_pago >= dia_actual {
            fecha_limite_pago - dia_actual
        } else {
            (30 - dia_actual) + fecha_limite_pago
        };

        let alerta_corte = dias_corte <= 3;
        let alerta_pago = dias_pago <= 3;

        let dias_corte_msg = if dias_corte == 0 {
            "Hoy es la fecha de corte".to_string()
        } else {
            format!("Faltan {} días para corte", dias_corte)
        };

        let dias_pago_msg = if dias_pago == 0 {
            "Hoy vence el pago".to_string()
        } else {
            format!("Faltan {} días para pagar", dias_pago)
        };

        Ok(Tarjeta {
            id,
            entidad,
            nombre_tarjeta,
            limite,
            balance_actual,
            fecha_corte,
            fecha_limite_pago,
            alerta_corte,
            alerta_pago,
            dias_corte_msg,
            dias_pago_msg,
        })
    }).map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r.map_err(|e| e.to_string())?);
    }
    Ok(list)
}

#[tauri::command]
fn crear_tarjeta(entidad: String, nombre: String, limite: f64, balance: f64, corte: i32, pago: i32) -> Result<i64, String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO tarjetas (entidad, nombre_tarjeta, limite, balance_actual, fecha_corte, fecha_limite_pago)
         VALUES (?, ?, ?, ?, ?, ?);",
        (entidad, nombre, limite, balance, corte, pago)
    ).map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn actualizar_limite_tarjeta(id: i64, limite: f64) -> Result<(), String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    conn.execute("UPDATE tarjetas SET limite = ? WHERE id = ?;", (limite, id)).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn registrar_pago_tarjeta(id: i64, fecha: String, monto: f64) -> Result<(), String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    tx.execute(
        "UPDATE tarjetas SET balance_actual = balance_actual - ? WHERE id = ?;",
        (monto, id)
    ).map_err(|e| e.to_string())?;

    tx.execute(
        "INSERT INTO pagos_tarjeta (tarjeta_id, fecha_pago, monto_pagado) VALUES (?, ?, ?);",
        (id, &fecha, monto)
    ).map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

// --- COMANDOS: SUSCRIPCIONES ---
#[tauri::command]
fn obtener_suscripciones() -> Result<Vec<Suscripcion>, String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT s.id, s.plataforma, s.monto, s.tarjeta_id, s.frecuencia, t.entidad, t.nombre_tarjeta
         FROM suscripciones s
         JOIN tarjetas t ON s.tarjeta_id = t.id
         ORDER BY s.plataforma ASC;"
    ).map_err(|e| e.to_string())?;

    let rows = stmt.query_map([], |row| {
        Ok(Suscripcion {
            id: row.get(0)?,
            plataforma: row.get(1)?,
            monto: row.get(2)?,
            tarjeta_id: row.get(3)?,
            frecuencia: row.get(4)?,
            entidad: row.get(5)?,
            nombre_tarjeta: row.get(6)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r.map_err(|e| e.to_string())?);
    }
    Ok(list)
}

#[tauri::command]
fn crear_suscripcion(plataforma: String, monto: f64, tarjeta_id: i64, frecuencia: String) -> Result<i64, String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO suscripciones (plataforma, monto, tarjeta_id, frecuencia) VALUES (?, ?, ?, ?);",
        (plataforma, monto, tarjeta_id, frecuencia)
    ).map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn eliminar_suscripcion(id: i64) -> Result<(), String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM suscripciones WHERE id = ?;", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

// --- COMANDOS: CAPITAL (NoSQL) ---
#[tauri::command]
fn obtener_capital() -> Result<Value, String> {
    let mut data = db_nosql::leer_coleccion("capital");
    let hoy = Local::now().naive_local().date();

    // Enriquecer Certificados e Inversiones en bolsa con alertas (10 días)
    if let Some(certificados) = data.get_mut("certificados").and_then(|v| v.as_array_mut()) {
        for c in certificados {
            c["alerta_vencimiento"] = serde_json::Value::Bool(false);
            if let Some(venc_str) = c.get("vencimiento").and_then(|v| v.as_str()) {
                if let Ok(venc_date) = NaiveDate::parse_from_str(venc_str, "%d/%m/%Y") {
                    let diff = venc_date.signed_duration_since(hoy).num_days();
                    c["dias_restantes"] = serde_json::Value::Number(serde_json::Number::from(diff));
                    if diff >= 0 && diff <= 10 {
                        c["alerta_vencimiento"] = serde_json::Value::Bool(true);
                        c["alerta_msg"] = serde_json::Value::String(format!("¡Vence en {} días!", diff));
                    } else if diff < 0 {
                        c["alerta_vencimiento"] = serde_json::Value::Bool(true);
                        c["alerta_msg"] = serde_json::Value::String("¡Vencido!".to_string());
                    }
                }
            }
        }
    }

    if let Some(bolsa) = data.get_mut("bolsa").and_then(|v| v.as_array_mut()) {
        for b in bolsa {
            b["alerta_vencimiento"] = serde_json::Value::Bool(false);
            if let Some(venc_str) = b.get("vencimiento").and_then(|v| v.as_str()) {
                if let Ok(venc_date) = NaiveDate::parse_from_str(venc_str, "%d/%m/%Y") {
                    let diff = venc_date.signed_duration_since(hoy).num_days();
                    b["dias_restantes"] = serde_json::Value::Number(serde_json::Number::from(diff));
                    if diff >= 0 && diff <= 10 {
                        b["alerta_vencimiento"] = serde_json::Value::Bool(true);
                        b["alerta_msg"] = serde_json::Value::String(format!("¡Vence en {} días!", diff));
                    } else if diff < 0 {
                        b["alerta_vencimiento"] = serde_json::Value::Bool(true);
                        b["alerta_msg"] = serde_json::Value::String("¡Vencido!".to_string());
                    }
                }
            }
        }
    }

    Ok(data)
}

#[tauri::command]
fn guardar_capital(data: Value) -> Result<(), String> {
    db_nosql::guardar_coleccion("capital", &data)
}

// --- COMANDOS: DEUDAS Y FINANCIAMIENTOS ---
#[tauri::command]
fn obtener_prestamos() -> Result<Vec<Prestamo>, String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, tipo_prestamo, monto_prestamo, institucion_financiera, tasa_actual, cuotas_totales, cuotas_pendientes, monto_cuota, dia_pago FROM prestamos ORDER BY id DESC;"
    ).map_err(|e| e.to_string())?;

    let hoy = Local::now();
    let dia_actual = hoy.day() as i32;

    let rows = stmt.query_map([], |row| {
        let id: i64 = row.get(0)?;
        let tipo_prestamo: String = row.get(1)?;
        let monto_prestamo: f64 = row.get(2)?;
        let institucion_financiera: String = row.get(3)?;
        let tasa_actual: f64 = row.get(4)?;
        let cuotas_totales: Option<i32> = row.get(5)?;
        let cuotas_pendientes: Option<i32> = row.get(6)?;
        let monto_cuota: f64 = row.get(7)?;
        let dia_pago: i32 = row.get(8)?;

        // Calcular recordatorios
        let mut alerta_pago = false;
        let mut dias_pago_msg = "-".to_string();

        let esta_vigente = tipo_prestamo == "flexible" || cuotas_pendientes.unwrap_or(0) > 0;
        if esta_vigente {
            let dias_para_pago = if dia_pago >= dia_actual {
                dia_pago - dia_actual
            } else {
                (30 - dia_actual) + dia_pago
            };
            if dias_para_pago <= 3 {
                alerta_pago = true;
                dias_pago_msg = if dias_para_pago == 0 {
                    "Hoy vence la cuota.".to_string()
                } else {
                    format!("¡Vence en {} días!", dias_para_pago)
                };
            } else {
                dias_pago_msg = format!("Faltan {} días para el pago.", dias_para_pago);
            }
        }

        Ok(Prestamo {
            id,
            tipo_prestamo,
            monto_prestamo,
            institucion_financiera,
            tasa_actual,
            cuotas_totales,
            cuotas_pendientes,
            monto_cuota,
            dia_pago,
            alerta_pago,
            dias_pago_msg,
        })
    }).map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r.map_err(|e| e.to_string())?);
    }
    Ok(list)
}

#[derive(Deserialize)]
struct PrestamoInput {
    tipo_prestamo: String,
    monto_prestamo: f64,
    institucion_financiera: String,
    tasa_actual: f64,
    cuotas_totales: Option<i32>,
    cuotas_pendientes: Option<i32>,
    monto_cuota: f64,
    dia_pago: i32,
}

#[tauri::command]
fn crear_prestamo(input: PrestamoInput) -> Result<i64, String> {
    // 1. Excepción: Validación estricta de cuotas en tipo no flexible
    let (c_totales, c_pendientes) = if input.tipo_prestamo == "flexible" {
        (None, None)
    } else {
        let tot = input.cuotas_totales.ok_or_else(|| "Las cuotas totales son requeridas.".to_string())?;
        let pend = input.cuotas_pendientes.ok_or_else(|| "Las cuotas pendientes son requeridas.".to_string())?;
        (Some(tot), Some(pend))
    };

    // 2. Excepción: Cuotas pendientes no pueden superar a las totales
    if let (Some(t), Some(p)) = (c_totales, c_pendientes) {
        if p > t {
            return Err("Error: El número de cuotas pendientes no puede ser mayor al número total de cuotas del préstamo.".to_string());
        }
    }

    if !(1..=31).contains(&input.dia_pago) {
        return Err("El día de pago debe ser un día válido del mes (1-31).".to_string());
    }

    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO prestamos (tipo_prestamo, monto_prestamo, institucion_financiera, tasa_actual, cuotas_totales, cuotas_pendientes, monto_cuota, dia_pago)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?);",
        (
            &input.tipo_prestamo,
            input.monto_prestamo,
            &input.institucion_financiera,
            input.tasa_actual,
            c_totales,
            c_pendientes,
            input.monto_cuota,
            input.dia_pago,
        )
    ).map_err(|e| e.to_string())?;

    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn pagar_cuota_prestamo(id: i64) -> Result<(), String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE prestamos SET cuotas_pendientes = MAX(0, cuotas_pendientes - 1) WHERE id = ? AND cuotas_pendientes IS NOT NULL;",
        [id]
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn eliminar_prestamo(id: i64) -> Result<(), String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM prestamos WHERE id = ?;", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

// --- PUNTO DE ENTRADA PRINCIPAL ---
fn main() {
    // Inicializar bases de datos antes del arranque
    db_sql::inicializar_db().expect("Error al inicializar la base de datos SQLite");
    
    // Iniciar app de escritorio Tauri
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            obtener_categorias,
            crear_categoria,
            eliminar_categoria,
            obtener_gastos,
            crear_gasto,
            obtener_ingresos,
            crear_ingreso,
            marcar_ingreso_pagado,
            obtener_ingresos_informales,
            crear_ingreso_informal,
            marcar_informal_pagado,
            obtener_tarjetas,
            crear_tarjeta,
            actualizar_limite_tarjeta,
            registrar_pago_tarjeta,
            obtener_suscripciones,
            crear_suscripcion,
            eliminar_suscripcion,
            obtener_capital,
            guardar_capital,
            obtener_prestamos,
            crear_prestamo,
            pagar_cuota_prestamo,
            eliminar_prestamo
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}
