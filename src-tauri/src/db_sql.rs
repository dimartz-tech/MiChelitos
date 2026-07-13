use rusqlite::{Connection, Result};
use std::fs;
use std::path::Path;

pub fn obtener_ruta_db() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    format!("{}/.michelitos/databases/sql/michelitos.db", home)
}

pub fn obtener_conexion() -> Result<Connection> {
    let db_path_str = obtener_ruta_db();
    let db_path = Path::new(&db_path_str);
    
    // Crear directorios si no existen
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }

    let conn = Connection::open(db_path)?;
    conn.execute("PRAGMA foreign_keys = ON;", [])?;
    Ok(conn)
}

pub fn inicializar_db() -> Result<()> {
    let conn = obtener_conexion()?;

    // 1. Tabla de Clientes
    conn.execute(
        "CREATE TABLE IF NOT EXISTS clientes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            rnc TEXT UNIQUE NOT NULL,
            nombre TEXT NOT NULL
        );",
        [],
    )?;

    // 2. Tabla de Ingresos (Facturas)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS ingresos (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            numero_factura TEXT UNIQUE NOT NULL,
            cliente_id INTEGER NOT NULL,
            fecha_emision TEXT NOT NULL,
            estatus TEXT CHECK(estatus IN ('emitida', 'pagada')) NOT NULL DEFAULT 'emitida',
            monto_total REAL NOT NULL,
            porcentaje_retencion REAL NOT NULL DEFAULT 15.00,
            monto_retenido REAL NOT NULL,
            institucion_deposito TEXT,
            fecha_pago TEXT,
            monto_recibido REAL,
            FOREIGN KEY (cliente_id) REFERENCES clientes(id)
        );",
        [],
    )?;

    // 3. Tabla de Categorías de Gastos
    conn.execute(
        "CREATE TABLE IF NOT EXISTS categorias (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            nombre TEXT UNIQUE NOT NULL
        );",
        [],
    )?;

    // 4. Tabla de Tarjetas de Crédito
    conn.execute(
        "CREATE TABLE IF NOT EXISTS tarjetas (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            entidad TEXT NOT NULL,
            nombre_tarjeta TEXT NOT NULL,
            limite REAL NOT NULL,
            balance_actual REAL NOT NULL,
            fecha_corte INTEGER CHECK(fecha_corte BETWEEN 1 AND 31) NOT NULL,
            fecha_limite_pago INTEGER CHECK(fecha_limite_pago BETWEEN 1 AND 31) NOT NULL
        );",
        [],
    )?;

    // 5. Tabla de Gastos
    conn.execute(
        "CREATE TABLE IF NOT EXISTS gastos (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            fecha TEXT NOT NULL,
            monto REAL NOT NULL,
            divisa TEXT NOT NULL DEFAULT 'DOP',
            descripcion TEXT NOT NULL,
            categoria_id INTEGER NOT NULL,
            metodo_pago TEXT CHECK(metodo_pago IN ('efectivo', 'tarjeta', 'transferencia')) NOT NULL DEFAULT 'efectivo',
            costo_adicional REAL NOT NULL DEFAULT 0.0,
            tarjeta_id INTEGER,
            FOREIGN KEY (categoria_id) REFERENCES categorias(id),
            FOREIGN KEY (tarjeta_id) REFERENCES tarjetas(id) ON DELETE SET NULL
        );",
        [],
    )?;

    // 6. Tabla de Pagos de Tarjetas (Abonos)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS pagos_tarjeta (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tarjeta_id INTEGER NOT NULL,
            fecha_pago TEXT NOT NULL,
            monto_pagado REAL NOT NULL,
            FOREIGN KEY (tarjeta_id) REFERENCES tarjetas(id) ON DELETE CASCADE
        );",
        [],
    )?;

    // 7. Tabla de Suscripciones Recurrentes
    conn.execute(
        "CREATE TABLE IF NOT EXISTS suscripciones (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            plataforma TEXT NOT NULL,
            monto REAL NOT NULL,
            tarjeta_id INTEGER NOT NULL,
            frecuencia TEXT CHECK(frecuencia IN ('mensual', 'anual')) NOT NULL DEFAULT 'mensual',
            FOREIGN KEY (tarjeta_id) REFERENCES tarjetas(id) ON DELETE CASCADE
        );",
        [],
    )?;

    // 8. Tabla de Ingresos Informales
    conn.execute(
        "CREATE TABLE IF NOT EXISTS ingresos_informales (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            fecha TEXT NOT NULL,
            descripcion TEXT NOT NULL,
            monto REAL NOT NULL,
            estatus TEXT CHECK(estatus IN ('pendiente', 'pagado')) NOT NULL DEFAULT 'pendiente',
            institucion_deposito TEXT,
            fecha_pago TEXT,
            monto_recibido REAL
        );",
        [],
    )?;

    // 9. Tabla de Préstamos / Deudas
    conn.execute(
        "CREATE TABLE IF NOT EXISTS prestamos (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tipo_prestamo TEXT CHECK(tipo_prestamo IN ('consumo', 'hipotecario', 'vehiculo', 'flexible')) NOT NULL,
            monto_prestamo REAL NOT NULL,
            institucion_financiera TEXT NOT NULL,
            tasa_actual REAL NOT NULL,
            cuotas_totales INTEGER,
            cuotas_pendientes INTEGER,
            monto_cuota REAL NOT NULL,
            dia_pago INTEGER CHECK(dia_pago BETWEEN 1 AND 31) NOT NULL
        );",
        [],
    )?;

    // Insertar categorías por defecto si está vacía
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM categorias;", [], |r| r.get(0))?;
    if count == 0 {
        let categorias_defecto = [
            "Alimentación", "Transporte", "Servicios Públicos", "Alquiler",
            "Suscripciones", "Entretenimiento", "Impuestos", "Seguros",
            "Maquinaria/Equipos", "Otros"
        ];
        for cat in &categorias_defecto {
            conn.execute("INSERT INTO categorias (nombre) VALUES (?);", [cat])?;
        }
    }

    Ok(())
}
