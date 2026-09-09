// Prevención de ventana de consola en Windows en producción
#![cfg_attr(
  all(not(debug_assertions), target_os = "windows"),
  windows_subsystem = "windows"
)]

mod db_sql;
mod db_nosql;

mod adaptadores;
mod aplicacion;
mod dominio;
mod puertos;

#[cfg(test)]
mod caracterizacion;

use serde::{Serialize, Deserialize};
use serde_json::Value;
use chrono::{NaiveDate, Local, Datelike};

use dominio::cargos::TASA_RETENCION;
use dominio::dinero::{Dinero, Divisa, TasaCambio};
use dominio::gasto::MetodoPago;
use adaptadores::sqlite::gastos::AlmacenSqlite;
use aplicacion::registrar_gasto::{registrar_gasto, DatosGasto};
use aplicacion::revertir_gasto::revertir_gasto;
use dominio::tarjeta::{LimitesDivisa, PoliticaLiquidacion, MONEDA_LOCAL};
use aplicacion::liquidar_gasto::liquidar_gasto;
use aplicacion::registrar_bonificacion::{registrar_bonificacion, revertir_bonificacion, DatosBonificacion};
use dominio::bonificacion::Bonificacion;

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
    tarjeta_id: Option<i64>,
    cuenta_ahorro_id: Option<i64>,
    // Conversión de divisa: NULL cuando no aplica.
    estado_conversion: Option<String>,
    monto_liquidado: Option<f64>,
    tasa_conversion: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BonificacionDto {
    id: i64,
    fecha: String,
    tarjeta_id: i64,
    entidad: String,
    nombre_tarjeta: String,
    monto: f64,
    divisa: String,
    concepto: String,
    gasto_id: Option<i64>,
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
    limite_pesos: f64,
    limite_dolares: f64,
    limite_ajustado_pesos: Option<f64>,
    limite_ajustado_dolares: Option<f64>,
    limite_sobregiro_pesos: f64,
    limite_sobregiro_dolares: f64,
    balance_pesos: f64,
    balance_dolares: f64,
    balance_corte_pesos: f64,
    balance_corte_dolares: f64,
    fecha_corte: i32,
    fecha_limite_pago: i32,
    politica_liquidacion: String,
    // Enriquecidos
    limite_efectivo_pesos: f64,
    limite_efectivo_dolares: f64,
    disponible_pesos: f64,
    disponible_dolares: f64,
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
    dia_facturacion: i32,
    fecha_ultimo_pago: Option<String>,
    divisa: String,
    entidad: String,
    nombre_tarjeta: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CuentaAhorro {
    id: i64,
    nombre: String,
    divisa: String,
    balance_actual: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TransaccionCuenta {
    id: i64,
    fecha: String,
    cuenta_origen_id: i64,
    cuenta_origen_nombre: String,
    cuenta_destino_id: i64,
    cuenta_destino_nombre: String,
    monto_origen: f64,
    monto_destino: f64,
    tasa_cambio: f64,
    cargo: f64,
    descripcion: Option<String>,
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

/// Límite efectivo y disponible de una divisa, resueltos por el dominio.
/// Devuelve `(efectivo, disponible)` en unidades, listos para el DTO.
fn cupo(
    divisa: Divisa,
    aprobado: f64,
    ajustado: Option<f64>,
    sobregiro: f64,
    balance: f64,
) -> (f64, f64) {
    let construir = || -> Result<(f64, f64), dominio::errores::ErrorDominio> {
        let limites = LimitesDivisa::nuevos(
            Dinero::nuevo(aprobado, divisa)?,
            ajustado.map(|a| Dinero::nuevo(a, divisa)).transpose()?,
            Dinero::nuevo(sobregiro, divisa)?,
        )?;
        let saldo = Dinero::nuevo(balance, divisa)?;
        Ok((limites.efectivo().unidades(), limites.disponible(saldo)?.unidades()))
    };
    construir().unwrap_or((aprobado, aprobado + sobregiro - balance))
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
        "SELECT g.id, g.fecha, g.monto, g.divisa, g.descripcion, g.categoria_id, c.nombre, g.metodo_pago, g.costo_adicional, g.tarjeta_id, g.cuenta_ahorro_id, g.estado_conversion, g.monto_liquidado, g.tasa_conversion
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
            tarjeta_id: row.get(9)?,
            cuenta_ahorro_id: row.get(10)?,
            estado_conversion: row.get(11)?,
            monto_liquidado: row.get(12)?,
            tasa_conversion: row.get(13)?,
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
    cuenta_ahorro_id: Option<i64>,
    #[serde(default)]
    tasa_cambio: Option<f64>,
}

#[tauri::command]
fn crear_gasto(input: GastoInput) -> Result<i64, String> {
    // El comando queda reducido a traducción: interpreta la entrada, abre la
    // transacción, delega en el caso de uso y confirma. Ninguna regla vive ya
    // aquí.
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;

    // Se conserva la interpretación vigente de la divisa: la columna
    // gastos.divisa no tiene CHECK y el código solo distingue "USD".
    let divisa = if input.divisa == "USD" { Divisa::Usd } else { Divisa::Dop };
    let metodo = MetodoPago::desde_codigo(&input.metodo_pago);

    let datos = DatosGasto {
        fecha: input.fecha,
        monto: Dinero::nuevo(input.monto, divisa)?,
        descripcion: input.descripcion,
        categoria_id: input.categoria_id,
        metodo,
        metodo_texto: input.metodo_pago,
        es_lbtr: input.es_lbtr,
        tarjeta_id: input.tarjeta_id,
        cuenta_ahorro_id: input.cuenta_ahorro_id,
        // Una tasa de cero o ausente significa "sin conversión"; el caso de
        // uso la exigirá solo si las divisas realmente difieren.
        tasa_cambio: match input.tasa_cambio {
            Some(v) if v > 0.0 => Some(TasaCambio::nueva(v)?),
            _ => None,
        },
    };

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let id = {
        let mut almacen = AlmacenSqlite::nuevo(&tx);
        registrar_gasto(datos, &mut almacen)?
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(id)
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
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    
    tx.execute(
        "UPDATE ingresos SET estatus = 'pagada', institucion_deposito = ?, fecha_pago = ?, monto_recibido = ? WHERE id = ?;",
        (&institucion, &fecha, monto_recibido, id)
    ).map_err(|e| e.to_string())?;

    // Incrementar balance de la cuenta de ahorro/efectivo si su nombre coincide
    let _ = tx.execute(
        "UPDATE cuentas_ahorro SET balance_actual = balance_actual + ? WHERE nombre = ?;",
        (monto_recibido, &institucion)
    );

    tx.commit().map_err(|e| e.to_string())?;
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
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    
    tx.execute(
        "UPDATE ingresos_informales SET estatus = 'pagado', institucion_deposito = ?, fecha_pago = ?, monto_recibido = ? WHERE id = ?;",
        (&institucion, &fecha, monto_recibido, id)
    ).map_err(|e| e.to_string())?;

    // Incrementar balance de la cuenta de ahorro/efectivo si su nombre coincide
    let _ = tx.execute(
        "UPDATE cuentas_ahorro SET balance_actual = balance_actual + ? WHERE nombre = ?;",
        (monto_recibido, &institucion)
    );

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

// --- COMANDOS: TARJETAS ---
#[tauri::command]
fn obtener_tarjetas() -> Result<Vec<Tarjeta>, String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, entidad, nombre_tarjeta, limite_pesos, limite_dolares, limite_sobregiro_pesos, limite_sobregiro_dolares, balance_pesos, balance_dolares, balance_corte_pesos, balance_corte_dolares, fecha_corte, fecha_limite_pago, limite_ajustado_pesos, limite_ajustado_dolares, politica_liquidacion FROM tarjetas;"
    ).map_err(|e| e.to_string())?;
    
    let hoy = Local::now();
    let dia_actual = hoy.day() as i32;

    let rows = stmt.query_map([], |row| {
        let id: i64 = row.get(0)?;
        let entidad: String = row.get(1)?;
        let nombre_tarjeta: String = row.get(2)?;
        let limite_pesos: f64 = row.get(3)?;
        let limite_dolares: f64 = row.get(4)?;
        let limite_sobregiro_pesos: f64 = row.get(5)?;
        let limite_sobregiro_dolares: f64 = row.get(6)?;
        let balance_pesos: f64 = row.get(7)?;
        let balance_dolares: f64 = row.get(8)?;
        let balance_corte_pesos: f64 = row.get(9)?;
        let balance_corte_dolares: f64 = row.get(10)?;
        let fecha_corte: i32 = row.get(11)?;
        let fecha_limite_pago: i32 = row.get(12)?;
        let limite_ajustado_pesos: Option<f64> = row.get(13)?;
        let limite_ajustado_dolares: Option<f64> = row.get(14)?;
        let politica_liquidacion: Option<String> = row.get(15)?;

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

        // El cupo lo resuelve el dominio, no la vista. Ante datos corruptos
        // se degrada al límite en bruto en lugar de tumbar la consulta.
        let (efectivo_dop, disponible_dop) =
            cupo(Divisa::Dop, limite_pesos, limite_ajustado_pesos, limite_sobregiro_pesos, balance_pesos);
        let (efectivo_usd, disponible_usd) =
            cupo(Divisa::Usd, limite_dolares, limite_ajustado_dolares, limite_sobregiro_dolares, balance_dolares);

        Ok(Tarjeta {
            id,
            entidad,
            nombre_tarjeta,
            limite_pesos,
            limite_dolares,
            limite_ajustado_pesos,
            limite_ajustado_dolares,
            limite_sobregiro_pesos,
            limite_sobregiro_dolares,
            balance_pesos,
            balance_dolares,
            balance_corte_pesos,
            balance_corte_dolares,
            fecha_corte,
            fecha_limite_pago,
            politica_liquidacion: PoliticaLiquidacion::desde_codigo(politica_liquidacion.as_deref())
                .codigo()
                .to_string(),
            limite_efectivo_pesos: efectivo_dop,
            limite_efectivo_dolares: efectivo_usd,
            disponible_pesos: disponible_dop,
            disponible_dolares: disponible_usd,
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
fn crear_tarjeta(
    entidad: String,
    nombre: String,
    limite_pesos: f64,
    limite_dolares: f64,
    sobregiro_pesos: f64,
    sobregiro_dolares: f64,
    balance_pesos: f64,
    balance_dolares: f64,
    balance_corte_pesos: f64,
    balance_corte_dolares: f64,
    corte: i32,
    pago: i32
) -> Result<i64, String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO tarjetas (entidad, nombre_tarjeta, limite_pesos, limite_dolares, limite_sobregiro_pesos, limite_sobregiro_dolares, balance_pesos, balance_dolares, balance_corte_pesos, balance_corte_dolares, fecha_corte, fecha_limite_pago)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
        (entidad, nombre, limite_pesos, limite_dolares, sobregiro_pesos, sobregiro_dolares, balance_pesos, balance_dolares, balance_corte_pesos, balance_corte_dolares, corte, pago)
    ).map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn actualizar_limites_tarjeta(
    id: i64,
    limite_pesos: f64,
    limite_dolares: f64,
    sobregiro_pesos: f64,
    sobregiro_dolares: f64,
    balance_corte_pesos: f64,
    balance_corte_dolares: f64,
    limite_ajustado_pesos: Option<f64>,
    limite_ajustado_dolares: Option<f64>,
    politica_liquidacion: Option<String>
) -> Result<(), String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE tarjetas SET limite_pesos = ?, limite_dolares = ?, limite_sobregiro_pesos = ?, limite_sobregiro_dolares = ?, balance_corte_pesos = ?, balance_corte_dolares = ?, limite_ajustado_pesos = ?, limite_ajustado_dolares = ?, politica_liquidacion = ? WHERE id = ?;",
        (limite_pesos, limite_dolares, sobregiro_pesos, sobregiro_dolares, balance_corte_pesos, balance_corte_dolares, limite_ajustado_pesos, limite_ajustado_dolares,
         PoliticaLiquidacion::desde_codigo(politica_liquidacion.as_deref()).codigo(), id)
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn registrar_pago_tarjeta(
    id: i64,
    fecha: String,
    monto: f64,
    divisa: String,
    cuenta_ahorro_id: Option<i64>,
    tasa_cambio: f64
) -> Result<(), String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    if divisa == "USD" {
        tx.execute(
            "UPDATE tarjetas SET balance_dolares = MAX(0.0, balance_dolares - ?) WHERE id = ?;",
            (monto, id)
        ).map_err(|e| e.to_string())?;
    } else {
        tx.execute(
            "UPDATE tarjetas SET balance_pesos = MAX(0.0, balance_pesos - ?) WHERE id = ?;",
            (monto, id)
        ).map_err(|e| e.to_string())?;
    }

    tx.execute(
        "INSERT INTO pagos_tarjeta (tarjeta_id, fecha_pago, monto_pagado, divisa) VALUES (?, ?, ?, ?);",
        (id, &fecha, monto, &divisa)
    ).map_err(|e| e.to_string())?;

    if let Some(c_id) = cuenta_ahorro_id {
        let divisa_pago = if divisa == "USD" { Divisa::Usd } else { Divisa::Dop };
        let monto_pago = Dinero::nuevo(monto, divisa_pago)?;

        // Importe que realmente sale de la cuenta. Cuando media una tasa de
        // cambio, la conversión se redondea a centavos antes de comisionar.
        let (debitado, descripcion) = if tasa_cambio > 0.0 {
            let en_pesos = match divisa_pago {
                Divisa::Usd => monto_pago.convertir(Divisa::Dop, TasaCambio::nueva(tasa_cambio)?)?,
                // Conducta vigente: si llega una tasa con un abono ya en pesos,
                // el código la aplicaba igualmente. Se conserva.
                Divisa::Dop => Dinero::nuevo(monto * tasa_cambio, Divisa::Dop)?,
            };
            (en_pesos, format!("Comisión 0.20% Pago Tarjeta (Tasa {})", tasa_cambio))
        } else {
            (monto_pago, "Comisión 0.20% Pago Tarjeta".to_string())
        };

        // Misma regla de redondeo que crear_gasto: una sola política para el
        // 0.20 % en todo el sistema.
        let comision = debitado.porcentaje(TASA_RETENCION)?;
        let total = debitado.sumar(&comision)?;

        tx.execute(
            "UPDATE cuentas_ahorro SET balance_actual = balance_actual - ? WHERE id = ?;",
            (total.unidades(), c_id),
        )
        .map_err(|e| e.to_string())?;

        tx.execute(
            "INSERT INTO gastos (fecha, monto, divisa, descripcion, categoria_id, metodo_pago, costo_adicional, tarjeta_id, cuenta_ahorro_id)
             VALUES (?, ?, ?, ?, (SELECT id FROM categorias WHERE nombre = 'Otros' LIMIT 1), 'transferencia', 0.0, NULL, ?);",
            (&fecha, comision.unidades(), comision.divisa().codigo(), &descripcion, c_id),
        )
        .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

// --- COMANDOS: SUSCRIPCIONES ---
#[tauri::command]
fn obtener_suscripciones() -> Result<Vec<Suscripcion>, String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT s.id, s.plataforma, s.monto, s.tarjeta_id, s.frecuencia, s.dia_facturacion, s.fecha_ultimo_pago, s.divisa, t.entidad, t.nombre_tarjeta
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
            dia_facturacion: row.get(5)?,
            fecha_ultimo_pago: row.get(6)?,
            divisa: row.get(7)?,
            entidad: row.get(8)?,
            nombre_tarjeta: row.get(9)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r.map_err(|e| e.to_string())?);
    }
    Ok(list)
}

#[tauri::command]
fn crear_suscripcion(plataforma: String, monto: f64, tarjeta_id: i64, frecuencia: String, dia_facturacion: i32, divisa: String) -> Result<i64, String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO suscripciones (plataforma, monto, tarjeta_id, frecuencia, dia_facturacion, divisa) VALUES (?, ?, ?, ?, ?, ?);",
        (plataforma, monto, tarjeta_id, frecuencia, dia_facturacion, divisa)
    ).map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

/// Edita una suscripción **conservando `fecha_ultimo_pago`**.
///
/// Esa preservación es el motivo de existir del comando: la única alternativa
/// hasta ahora era borrar y volver a crear, lo que reinicia el marcador de
/// idempotencia y hace que el siguiente procesamiento cobre otra vez el mismo
/// mes. Los cargos ya realizados son gastos independientes y no se tocan: lo
/// que se edita es la configuración de los cobros futuros.
#[tauri::command]
fn actualizar_suscripcion(
    id: i64,
    plataforma: String,
    monto: f64,
    tarjeta_id: i64,
    frecuencia: String,
    dia_facturacion: i32,
    divisa: String,
) -> Result<(), String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let filas = conn
        .execute(
            "UPDATE suscripciones SET plataforma = ?, monto = ?, tarjeta_id = ?, frecuencia = ?, dia_facturacion = ?, divisa = ? WHERE id = ?;",
            (plataforma, monto, tarjeta_id, frecuencia, dia_facturacion, divisa, id),
        )
        .map_err(|e| e.to_string())?;
    if filas == 0 {
        return Err("No se encontró la suscripción que se intenta editar.".to_string());
    }
    Ok(())
}

#[tauri::command]
fn eliminar_suscripcion(id: i64) -> Result<(), String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM suscripciones WHERE id = ?;", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn procesar_suscripciones() -> Result<Vec<String>, String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    
    // Obtener la fecha actual
    let hoy = Local::now().naive_local();
    let hoy_fecha_str = hoy.format("%d/%m/%Y").to_string(); // Formato estándar usado en el frontend
    let dia_actual = hoy.day() as i32;
    let mes_actual = hoy.month();
    let anio_actual = hoy.year();

    let mut stmt = conn.prepare(
        "SELECT id, plataforma, monto, tarjeta_id, frecuencia, dia_facturacion, fecha_ultimo_pago, divisa FROM suscripciones;"
    ).map_err(|e| e.to_string())?;

    struct SubRecord {
        id: i64,
        plataforma: String,
        monto: f64,
        tarjeta_id: i64,
        frecuencia: String,
        dia_facturacion: i32,
        fecha_ultimo_pago: Option<String>,
        divisa: String,
    }

    let rows = stmt.query_map([], |row| {
        Ok(SubRecord {
            id: row.get(0)?,
            plataforma: row.get(1)?,
            monto: row.get(2)?,
            tarjeta_id: row.get(3)?,
            frecuencia: row.get(4)?,
            dia_facturacion: row.get(5)?,
            fecha_ultimo_pago: row.get(6)?,
            divisa: row.get(7)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut suscripciones = Vec::new();
    for r in rows {
        suscripciones.push(r.map_err(|e| e.to_string())?);
    }
    drop(stmt);

    // Buscar o crear la categoría de Suscripciones en el sistema
    let categoria_suscripciones_id: i64 = match conn.query_row(
        "SELECT id FROM categorias WHERE LOWER(nombre) = 'suscripciones';",
        [],
        |r| r.get(0)
    ) {
        Ok(id) => id,
        Err(_) => {
            // Si no existe, usar la categoría "Otros" o crear "Suscripciones"
            match conn.query_row(
                "SELECT id FROM categorias WHERE LOWER(nombre) = 'otros';",
                [],
                |r| r.get(0)
            ) {
                Ok(id) => id,
                Err(_) => 1 // Fallback al ID 1
            }
        }
    };

    let mut mensajes_cargo = Vec::new();

    for sub in suscripciones {
        // Determinar si corresponde realizar el cargo automático
        let mut requiere_cargo = false;
        
        if let Some(ref ultimo_pago) = sub.fecha_ultimo_pago {
            // Intentar parsear el último pago
            let partes: Vec<&str> = ultimo_pago.split('/').collect();
            if partes.len() == 3 {
                if let (Ok(_p_dia), Ok(p_mes), Ok(p_anio)) = (partes[0].parse::<i32>(), partes[1].parse::<u32>(), partes[2].parse::<i32>()) {
                    if sub.frecuencia == "mensual" {
                        if (anio_actual > p_anio || (anio_actual == p_anio && mes_actual > p_mes)) && dia_actual >= sub.dia_facturacion {
                            requiere_cargo = true;
                        }
                    } else if sub.frecuencia == "anual" {
                        if anio_actual > p_anio && dia_actual >= sub.dia_facturacion {
                            requiere_cargo = true;
                        }
                    }
                }
            } else {
                requiere_cargo = true;
            }
        } else {
            if dia_actual >= sub.dia_facturacion {
                requiere_cargo = true;
            }
        }

        if requiere_cargo {
            let tx = conn.transaction().map_err(|e| e.to_string())?;

            // 1. Cargar a la tarjeta correspondiente
            if sub.divisa == "USD" {
                tx.execute(
                    "UPDATE tarjetas SET balance_dolares = balance_dolares + ? WHERE id = ?;",
                    (sub.monto, sub.tarjeta_id)
                ).map_err(|e| e.to_string())?;
            } else {
                tx.execute(
                    "UPDATE tarjetas SET balance_pesos = balance_pesos + ? WHERE id = ?;",
                    (sub.monto, sub.tarjeta_id)
                ).map_err(|e| e.to_string())?;
            }

            // 2. Registrar en gastos
            tx.execute(
                "INSERT INTO gastos (fecha, monto, divisa, descripcion, categoria_id, metodo_pago, costo_adicional, tarjeta_id)
                 VALUES (?, ?, ?, ?, ?, 'tarjeta', 0.0, ?);",
                (&hoy_fecha_str, sub.monto, &sub.divisa, format!("Cargo recurrente: {}", sub.plataforma), categoria_suscripciones_id, sub.tarjeta_id)
            ).map_err(|e| e.to_string())?;

            // 3. Actualizar fecha de último pago
            tx.execute(
                "UPDATE suscripciones SET fecha_ultimo_pago = ? WHERE id = ?;",
                (&hoy_fecha_str, sub.id)
            ).map_err(|e| e.to_string())?;

            tx.commit().map_err(|e| e.to_string())?;

            mensajes_cargo.push(format!("Cargo automático realizado para {} ({} {})", sub.plataforma, sub.divisa, sub.monto));
        }
    }

    Ok(mensajes_cargo)
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

#[tauri::command]
fn obtener_clientes() -> Result<Vec<Cliente>, String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT id, rnc, nombre FROM clientes ORDER BY nombre ASC;").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| {
        Ok(Cliente {
            id: row.get(0)?,
            rnc: row.get(1)?,
            nombre: row.get(2)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r.map_err(|e| e.to_string())?);
    }
    Ok(list)
}

#[tauri::command]
fn crear_cliente(rnc: String, nombre: String) -> Result<Cliente, String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let rnc_clean = rnc.trim();
    let nombre_clean = nombre.trim();
    if rnc_clean.is_empty() || nombre_clean.is_empty() {
        return Err("RNC y nombre no pueden estar vacíos.".to_string());
    }

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM clientes WHERE rnc = ?;",
        [rnc_clean],
        |r| r.get(0)
    ).map_err(|e| e.to_string())?;

    if count > 0 {
        return Err("Ya existe un cliente con este RNC.".to_string());
    }

    conn.execute(
        "INSERT INTO clientes (rnc, nombre) VALUES (?, ?);",
        [rnc_clean, nombre_clean]
    ).map_err(|e| e.to_string())?;

    Ok(Cliente {
        id: conn.last_insert_rowid(),
        rnc: rnc_clean.to_string(),
        nombre: nombre_clean.to_string(),
    })
}

#[tauri::command]
fn eliminar_cliente(id: i64) -> Result<(), String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM ingresos WHERE cliente_id = ?;",
        [id],
        |r| r.get(0)
    ).map_err(|e| e.to_string())?;

    if count > 0 {
        return Err("No se puede eliminar el cliente porque tiene facturas registradas.".to_string());
    }

    conn.execute("DELETE FROM clientes WHERE id = ?;", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn obtener_cuentas() -> Result<Vec<CuentaAhorro>, String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT id, nombre, divisa, balance_actual FROM cuentas_ahorro ORDER BY nombre ASC;").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| {
        Ok(CuentaAhorro {
            id: row.get(0)?,
            nombre: row.get(1)?,
            divisa: row.get(2)?,
            balance_actual: row.get(3)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r.map_err(|e| e.to_string())?);
    }
    Ok(list)
}

#[tauri::command]
fn crear_cuenta(nombre: String, divisa: String, balance: f64) -> Result<i64, String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let nombre_clean = nombre.trim();
    if nombre_clean.is_empty() {
        return Err("El nombre de la cuenta no puede estar vacío.".to_string());
    }

    conn.execute(
        "INSERT INTO cuentas_ahorro (nombre, divisa, balance_actual) VALUES (?, ?, ?);",
        (nombre_clean, divisa, balance)
    ).map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn eliminar_cuenta(id: i64) -> Result<(), String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM gastos WHERE cuenta_ahorro_id = ?;",
        [id],
        |r| r.get(0)
    ).map_err(|e| e.to_string())?;

    if count > 0 {
        return Err("No se puede eliminar la cuenta porque tiene transferencias registradas en gastos.".to_string());
    }

    conn.execute("DELETE FROM cuentas_ahorro WHERE id = ?;", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn transferir_entre_cuentas(
    fecha: String,
    origen_id: i64,
    destino_id: i64,
    monto_origen: f64,
    monto_destino: f64,
    cargo: f64,
    descripcion: String
) -> Result<(), String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    tx.execute(
        "UPDATE cuentas_ahorro SET balance_actual = balance_actual - ? WHERE id = ?;",
        (monto_origen + cargo, origen_id)
    ).map_err(|e| e.to_string())?;

    tx.execute(
        "UPDATE cuentas_ahorro SET balance_actual = balance_actual + ? WHERE id = ?;",
        (monto_destino, destino_id)
    ).map_err(|e| e.to_string())?;

    let tasa_cambio = if monto_origen > 0.0 { monto_destino / monto_origen } else { 1.0 };
    tx.execute(
        "INSERT INTO transacciones_cuentas (fecha, cuenta_origen_id, cuenta_destino_id, monto_origen, monto_destino, tasa_cambio, cargo, descripcion)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?);",
        (fecha, origen_id, destino_id, monto_origen, monto_destino, tasa_cambio, cargo, &descripcion)
    ).map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn obtener_transacciones_cuentas() -> Result<Vec<TransaccionCuenta>, String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT t.id, t.fecha, t.cuenta_origen_id, co.nombre, t.cuenta_destino_id, cd.nombre, t.monto_origen, t.monto_destino, t.tasa_cambio, t.cargo, t.descripcion
         FROM transacciones_cuentas t
         JOIN cuentas_ahorro co ON t.cuenta_origen_id = co.id
         JOIN cuentas_ahorro cd ON t.cuenta_destino_id = cd.id
         ORDER BY t.id DESC;"
    ).map_err(|e| e.to_string())?;

    let rows = stmt.query_map([], |row| {
        Ok(TransaccionCuenta {
            id: row.get(0)?,
            fecha: row.get(1)?,
            cuenta_origen_id: row.get(2)?,
            cuenta_origen_nombre: row.get(3)?,
            cuenta_destino_id: row.get(4)?,
            cuenta_destino_nombre: row.get(5)?,
            monto_origen: row.get(6)?,
            monto_destino: row.get(7)?,
            tasa_cambio: row.get(8)?,
            cargo: row.get(9)?,
            descripcion: row.get(10)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r.map_err(|e| e.to_string())?);
    }
    Ok(list)
}

#[tauri::command]
fn actualizar_ingreso(
    id: i64,
    numero_factura: String,
    cliente_id: i64,
    fecha_emision: String,
    monto_total: f64,
    porcentaje_retencion: f64
) -> Result<(), String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let monto_retenido = (monto_total * (porcentaje_retencion / 100.0)).round();
    conn.execute(
        "UPDATE ingresos SET numero_factura = ?, cliente_id = ?, fecha_emision = ?, monto_total = ?, porcentaje_retencion = ?, monto_retenido = ? WHERE id = ?;",
        (numero_factura, cliente_id, fecha_emision, monto_total, porcentaje_retencion, monto_retenido, id)
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn crear_cobro_efectivo_informal(fecha: String, descripcion: String, monto: f64, divisa: String) -> Result<i64, String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    
    let cuenta_efectivo = if divisa == "USD" { "Efectivo USD" } else { "Efectivo DOP" };
    
    tx.execute(
        "INSERT INTO ingresos_informales (fecha, descripcion, monto, estatus, institucion_deposito, fecha_pago, monto_recibido)
         VALUES (?, ?, ?, 'pagado', ?, ?, ?);",
        (&fecha, &descripcion, monto, cuenta_efectivo, &fecha, monto)
    ).map_err(|e| e.to_string())?;
    
    let id = tx.last_insert_rowid();
    
    tx.execute(
        "UPDATE cuentas_ahorro SET balance_actual = balance_actual + ? WHERE nombre = ?;",
        (monto, cuenta_efectivo)
    ).map_err(|e| e.to_string())?;
    
    tx.commit().map_err(|e| e.to_string())?;
    Ok(id)
}

/// Cierra un consumo pendiente con el importe que el emisor cargó en moneda
/// local. Devuelve la tasa que se dedujo, para poder mostrarla.
#[tauri::command]
fn obtener_bonificaciones() -> Result<Vec<BonificacionDto>, String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT b.id, b.fecha, b.tarjeta_id, t.entidad, t.nombre_tarjeta, b.monto, b.divisa, b.concepto, b.gasto_id
             FROM bonificaciones b JOIN tarjetas t ON t.id = b.tarjeta_id
             ORDER BY b.id DESC;",
        )
        .map_err(|e| e.to_string())?;
    let filas = stmt
        .query_map([], |r| {
            Ok(BonificacionDto {
                id: r.get(0)?,
                fecha: r.get(1)?,
                tarjeta_id: r.get(2)?,
                entidad: r.get(3)?,
                nombre_tarjeta: r.get(4)?,
                monto: r.get(5)?,
                divisa: r.get(6)?,
                concepto: r.get(7)?,
                gasto_id: r.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut lista = Vec::new();
    for f in filas {
        lista.push(f.map_err(|e| e.to_string())?);
    }
    Ok(lista)
}

/// Registra un crédito del emisor sobre una tarjeta. Reduce su deuda sin
/// alterar el consumo que lo originó.
#[tauri::command]
fn crear_bonificacion(
    fecha: String,
    tarjeta_id: i64,
    monto: f64,
    divisa: String,
    concepto: String,
    gasto_id: Option<i64>,
) -> Result<i64, String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let divisa = if divisa == "USD" { Divisa::Usd } else { Divisa::Dop };
    let bonificacion = Bonificacion::nueva(Dinero::nuevo(monto, divisa)?, &concepto)?;

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let id = {
        let mut almacen = AlmacenSqlite::nuevo(&tx);
        registrar_bonificacion(
            DatosBonificacion { fecha, tarjeta_id, bonificacion, gasto_id },
            &mut almacen,
        )?
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
fn eliminar_bonificacion(id: i64) -> Result<(), String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    {
        let mut almacen = AlmacenSqlite::nuevo(&tx);
        revertir_bonificacion(id, &mut almacen)?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn liquidar_consumo_pendiente(id: i64, monto_liquidado: f64) -> Result<f64, String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let tasa = {
        let mut almacen = AlmacenSqlite::nuevo(&tx);
        let importe = Dinero::nuevo(monto_liquidado, MONEDA_LOCAL)?;
        liquidar_gasto(id, importe, &mut almacen)?.tasa().valor()
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(tasa)
}

#[tauri::command]
fn eliminar_gasto(id: i64) -> Result<(), String> {
    // Traducción pura, igual que crear_gasto. La reversión y el recorte en
    // cero de la deuda de tarjeta (H5) viven ahora en el caso de uso y en el
    // puerto, no en esta consulta.
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    {
        let mut almacen = AlmacenSqlite::nuevo(&tx);
        revertir_gasto(id, &mut almacen)?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn eliminar_transaccion_cuenta(id: i64) -> Result<(), String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    
    let (origen_id, destino_id, monto_origen, monto_destino, cargo): (i64, i64, f64, f64, f64) = tx.query_row(
        "SELECT cuenta_origen_id, cuenta_destino_id, monto_origen, monto_destino, cargo FROM transacciones_cuentas WHERE id = ?;",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
    ).map_err(|e| e.to_string())?;
    
    tx.execute(
        "UPDATE cuentas_ahorro SET balance_actual = balance_actual + ? WHERE id = ?;",
        (monto_origen + cargo, origen_id)
    ).map_err(|e| e.to_string())?;
    
    tx.execute(
        "UPDATE cuentas_ahorro SET balance_actual = MAX(0.0, balance_actual - ?) WHERE id = ?;",
        (monto_destino, destino_id)
    ).map_err(|e| e.to_string())?;
    
    tx.execute("DELETE FROM transacciones_cuentas WHERE id = ?;", [id]).map_err(|e| e.to_string())?;
    
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn eliminar_ingreso_informal(id: i64) -> Result<(), String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    
    let (estatus, institucion_deposito, monto_recibido): (String, Option<String>, Option<f64>) = tx.query_row(
        "SELECT estatus, institucion_deposito, monto_recibido FROM ingresos_informales WHERE id = ?;",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))
    ).map_err(|e| e.to_string())?;
    
    if estatus == "pagado" {
        if let Some(ref inst) = institucion_deposito {
            if !inst.is_empty() {
                tx.execute(
                    "UPDATE cuentas_ahorro SET balance_actual = MAX(0.0, balance_actual - ?) WHERE nombre = ?;",
                    (monto_recibido.unwrap_or(0.0), inst)
                ).map_err(|e| e.to_string())?;
            }
        }
    }
    
    tx.execute("DELETE FROM ingresos_informales WHERE id = ?;", [id]).map_err(|e| e.to_string())?;
    
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn eliminar_ingreso(id: i64) -> Result<(), String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    
    let (estatus, institucion_deposito, monto_recibido): (String, Option<String>, Option<f64>) = tx.query_row(
        "SELECT estatus, institucion_deposito, monto_recibido FROM ingresos WHERE id = ?;",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))
    ).map_err(|e| e.to_string())?;
    
    if estatus == "pagada" {
        if let Some(ref inst) = institucion_deposito {
            if !inst.is_empty() {
                tx.execute(
                    "UPDATE cuentas_ahorro SET balance_actual = MAX(0.0, balance_actual - ?) WHERE nombre = ?;",
                    (monto_recibido.unwrap_or(0.0), inst)
                ).map_err(|e| e.to_string())?;
            }
        }
    }
    
    tx.execute("DELETE FROM ingresos WHERE id = ?;", [id]).map_err(|e| e.to_string())?;
    
    tx.commit().map_err(|e| e.to_string())?;
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
            actualizar_ingreso,
            marcar_ingreso_pagado,
            obtener_ingresos_informales,
            crear_ingreso_informal,
            marcar_informal_pagado,
            obtener_tarjetas,
            crear_tarjeta,
            actualizar_limites_tarjeta,
            registrar_pago_tarjeta,
            obtener_suscripciones,
            crear_suscripcion,
            actualizar_suscripcion,
            eliminar_suscripcion,
            procesar_suscripciones,
            obtener_capital,
            guardar_capital,
            obtener_prestamos,
            crear_prestamo,
            pagar_cuota_prestamo,
            eliminar_prestamo,
            obtener_clientes,
            crear_cliente,
            eliminar_cliente,
            obtener_cuentas,
            crear_cuenta,
            eliminar_cuenta,
            transferir_entre_cuentas,
            obtener_transacciones_cuentas,
            crear_cobro_efectivo_informal,
            eliminar_gasto,
            liquidar_consumo_pendiente,
            obtener_bonificaciones,
            crear_bonificacion,
            eliminar_bonificacion,
            eliminar_transaccion_cuenta,
            eliminar_ingreso_informal,
            eliminar_ingreso
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}
