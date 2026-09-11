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

fn columna_existe(conn: &Connection, tabla: &str, columna: &str) -> bool {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({});", tabla)).unwrap();
    let mut rows = stmt.query([]).unwrap();
    while let Some(row) = rows.next().unwrap() {
        let name: String = row.get(1).unwrap();
        if name == columna {
            return true;
        }
    }
    false
}

pub fn inicializar_db() -> Result<()> {
    let conn = obtener_conexion()?;
    crear_esquema(&conn)
}

/// Crea el esquema y las semillas sobre una conexión dada.
///
/// Separarlo de `inicializar_db` permite levantar una base en memoria en las
/// pruebas del adaptador, sin depender de `HOME` ni tocar disco.
pub fn crear_esquema(conn: &Connection) -> Result<()> {

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

    // 4. Tabla de Tarjetas de Crédito (Double-balance version)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS tarjetas (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            entidad TEXT NOT NULL,
            nombre_tarjeta TEXT NOT NULL,
            limite REAL NOT NULL DEFAULT 0.0,
            balance_actual REAL NOT NULL DEFAULT 0.0,
            fecha_corte INTEGER CHECK(fecha_corte BETWEEN 1 AND 31) NOT NULL,
            fecha_limite_pago INTEGER CHECK(fecha_limite_pago BETWEEN 1 AND 31) NOT NULL,
            balance_pesos REAL NOT NULL DEFAULT 0.0,
            balance_dolares REAL NOT NULL DEFAULT 0.0,
            limite_pesos REAL NOT NULL DEFAULT 0.0,
            limite_dolares REAL NOT NULL DEFAULT 0.0,
            limite_sobregiro_pesos REAL NOT NULL DEFAULT 0.0,
            limite_sobregiro_dolares REAL NOT NULL DEFAULT 0.0,
            balance_corte_pesos REAL NOT NULL DEFAULT 0.0,
            balance_corte_dolares REAL NOT NULL DEFAULT 0.0
        );",
        [],
    )?;

    // Migración de la tabla tarjetas si viene de versión anterior
    if !columna_existe(&conn, "tarjetas", "balance_pesos") {
        let _ = conn.execute("ALTER TABLE tarjetas ADD COLUMN balance_pesos REAL NOT NULL DEFAULT 0.0;", []);
        let _ = conn.execute("ALTER TABLE tarjetas ADD COLUMN balance_dolares REAL NOT NULL DEFAULT 0.0;", []);
        let _ = conn.execute("ALTER TABLE tarjetas ADD COLUMN limite_pesos REAL NOT NULL DEFAULT 0.0;", []);
        let _ = conn.execute("ALTER TABLE tarjetas ADD COLUMN limite_dolares REAL NOT NULL DEFAULT 0.0;", []);
        let _ = conn.execute("ALTER TABLE tarjetas ADD COLUMN limite_sobregiro_pesos REAL NOT NULL DEFAULT 0.0;", []);
        let _ = conn.execute("ALTER TABLE tarjetas ADD COLUMN limite_sobregiro_dolares REAL NOT NULL DEFAULT 0.0;", []);
        let _ = conn.execute("ALTER TABLE tarjetas ADD COLUMN balance_corte_pesos REAL NOT NULL DEFAULT 0.0;", []);
        let _ = conn.execute("ALTER TABLE tarjetas ADD COLUMN balance_corte_dolares REAL NOT NULL DEFAULT 0.0;", []);

        // Traspasar datos antiguos si existían
        let _ = conn.execute("UPDATE tarjetas SET balance_pesos = balance_actual, limite_pesos = limite;", []);
    }

    // Límite ajustado: tope opcional que el titular se impone por debajo del
    // aprobado. NULL significa "sin ajuste", lo que deja libre el cero para
    // expresar una tarjeta deliberadamente congelada.
    if !columna_existe(&conn, "tarjetas", "limite_ajustado_pesos") {
        let _ = conn.execute("ALTER TABLE tarjetas ADD COLUMN limite_ajustado_pesos REAL;", []);
        let _ = conn.execute("ALTER TABLE tarjetas ADD COLUMN limite_ajustado_dolares REAL;", []);
    }

    // 5. Cuentas de Ahorro
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cuentas_ahorro (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            nombre TEXT UNIQUE NOT NULL,
            divisa TEXT CHECK(divisa IN ('DOP', 'USD')) NOT NULL DEFAULT 'DOP',
            balance_actual REAL NOT NULL DEFAULT 0.0
        );",
        [],
    )?;

    // 6. Tabla de Gastos
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
            cuenta_ahorro_id INTEGER,
            FOREIGN KEY (categoria_id) REFERENCES categorias(id),
            FOREIGN KEY (tarjeta_id) REFERENCES tarjetas(id) ON DELETE SET NULL,
            FOREIGN KEY (cuenta_ahorro_id) REFERENCES cuentas_ahorro(id) ON DELETE SET NULL
        );",
        [],
    )?;

    // Migración de la tabla gastos
    if !columna_existe(&conn, "gastos", "cuenta_ahorro_id") {
        let _ = conn.execute("ALTER TABLE gastos ADD COLUMN cuenta_ahorro_id INTEGER REFERENCES cuentas_ahorro(id) ON DELETE SET NULL;", []);
    }

    // Conversión de divisa de un gasto pagado desde una cuenta de otra
    // moneda. NULL en las tres columnas significa que no hubo conversión.
    // monto_liquidado guarda el importe que REALMENTE salió de la cuenta, y
    // es el autoritativo para revertir: recalcularlo desde la tasa podría
    // desviarse en centavos.
    if !columna_existe(conn, "gastos", "tasa_conversion") {
        let _ = conn.execute("ALTER TABLE gastos ADD COLUMN tasa_conversion REAL;", []);
        let _ = conn.execute("ALTER TABLE gastos ADD COLUMN monto_liquidado REAL;", []);
        let _ = conn.execute("ALTER TABLE gastos ADD COLUMN divisa_liquidada TEXT;", []);
    }

    // Estado del consumo respecto a la conversión: NULL cuando no aplica,
    // 'pendiente' mientras el emisor no fija el importe en moneda local, y
    // 'liquidado' una vez lo fija.
    if !columna_existe(conn, "gastos", "estado_conversion") {
        let _ = conn.execute("ALTER TABLE gastos ADD COLUMN estado_conversion TEXT;", []);
    }

    // Política de liquidación del emisor. NULL equivale a 'origen', que es la
    // que no introduce consumos pendientes.
    if !columna_existe(conn, "tarjetas", "politica_liquidacion") {
        let _ = conn.execute("ALTER TABLE tarjetas ADD COLUMN politica_liquidacion TEXT;", []);
    }

    // 7. Tabla de Pagos de Tarjetas (Abonos)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS pagos_tarjeta (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tarjeta_id INTEGER NOT NULL,
            fecha_pago TEXT NOT NULL,
            monto_pagado REAL NOT NULL,
            divisa TEXT CHECK(divisa IN ('DOP', 'USD')) NOT NULL DEFAULT 'DOP',
            FOREIGN KEY (tarjeta_id) REFERENCES tarjetas(id) ON DELETE CASCADE
        );",
        [],
    )?;

    // Migración de la tabla pagos_tarjeta
    if !columna_existe(&conn, "pagos_tarjeta", "divisa") {
        let _ = conn.execute("ALTER TABLE pagos_tarjeta ADD COLUMN divisa TEXT CHECK(divisa IN ('DOP', 'USD')) NOT NULL DEFAULT 'DOP';", []);
    }

    // 12. Bonificaciones acreditadas por el emisor sobre una tarjeta.
    // Son créditos aparte, nunca una reducción del consumo original, y un
    // mismo gasto puede generar varias, así que gasto_id no es único ni
    // obligatorio: los estados no dicen a qué consumo corresponde cada uno.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS bonificaciones (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            fecha TEXT NOT NULL,
            tarjeta_id INTEGER NOT NULL,
            monto REAL NOT NULL,
            divisa TEXT CHECK(divisa IN ('DOP', 'USD')) NOT NULL DEFAULT 'DOP',
            concepto TEXT NOT NULL,
            gasto_id INTEGER,
            FOREIGN KEY (tarjeta_id) REFERENCES tarjetas(id) ON DELETE CASCADE,
            FOREIGN KEY (gasto_id) REFERENCES gastos(id) ON DELETE SET NULL
        );",
        [],
    )?;

    // 8. Tabla de Suscripciones Recurrentes
    conn.execute(
        "CREATE TABLE IF NOT EXISTS suscripciones (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            plataforma TEXT NOT NULL,
            monto REAL NOT NULL,
            tarjeta_id INTEGER NOT NULL,
            frecuencia TEXT CHECK(frecuencia IN ('mensual', 'anual')) NOT NULL DEFAULT 'mensual',
            dia_facturacion INTEGER NOT NULL DEFAULT 1,
            fecha_ultimo_pago TEXT,
            divisa TEXT CHECK(divisa IN ('DOP', 'USD')) NOT NULL DEFAULT 'DOP',
            FOREIGN KEY (tarjeta_id) REFERENCES tarjetas(id) ON DELETE CASCADE
        );",
        [],
    )?;

    // Migración de la tabla suscripciones
    if !columna_existe(&conn, "suscripciones", "dia_facturacion") {
        let _ = conn.execute("ALTER TABLE suscripciones ADD COLUMN dia_facturacion INTEGER NOT NULL DEFAULT 1;", []);
        let _ = conn.execute("ALTER TABLE suscripciones ADD COLUMN fecha_ultimo_pago TEXT;", []);
        let _ = conn.execute("ALTER TABLE suscripciones ADD COLUMN divisa TEXT CHECK(divisa IN ('DOP', 'USD')) NOT NULL DEFAULT 'DOP';", []);
    }

    // 9. Tabla de Ingresos Informales
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

    // 10. Tabla de Préstamos / Deudas
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

    // 11. Tabla de Transacciones entre Cuentas
    conn.execute(
        "CREATE TABLE IF NOT EXISTS transacciones_cuentas (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            fecha TEXT NOT NULL,
            cuenta_origen_id INTEGER NOT NULL,
            cuenta_destino_id INTEGER NOT NULL,
            monto_origen REAL NOT NULL,
            monto_destino REAL NOT NULL,
            tasa_cambio REAL NOT NULL,
            cargo REAL NOT NULL DEFAULT 0.0,
            descripcion TEXT,
            FOREIGN KEY(cuenta_origen_id) REFERENCES cuentas_ahorro(id) ON DELETE CASCADE,
            FOREIGN KEY(cuenta_destino_id) REFERENCES cuentas_ahorro(id) ON DELETE CASCADE
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

    // Asegurar que existan las cuentas por defecto de Efectivo
    let _ = conn.execute(
        "INSERT OR IGNORE INTO cuentas_ahorro (nombre, divisa, balance_actual) VALUES ('Efectivo DOP', 'DOP', 0.0);",
        []
    );
    let _ = conn.execute(
        "INSERT OR IGNORE INTO cuentas_ahorro (nombre, divisa, balance_actual) VALUES ('Efectivo USD', 'USD', 0.0);",
        []
    );

    // --- Resolución de H3: la caja de efectivo deja de identificarse por su
    // nombre. Buscarla con `WHERE nombre = 'Efectivo DOP'` hacía que un
    // renombrado —o un borrado, que la guarda de `eliminar_cuenta` no impedía
    // porque ningún gasto la referenciaba por id— dejara el gasto registrado
    // sin mover ningún saldo, devolviendo `Ok`.
    //
    // Se sustituye por un papel explícito en la fila y por la referencia real
    // en cada gasto. Las tres sentencias son idempotentes.
    let _ = conn.execute(
        "ALTER TABLE cuentas_ahorro ADD COLUMN es_caja_efectivo INTEGER NOT NULL DEFAULT 0;",
        [],
    );

    // El nombre se usa una única vez, aquí, para marcar las cajas que ya
    // existían. A partir de este punto el vínculo es el papel, no el texto.
    conn.execute(
        "UPDATE cuentas_ahorro SET es_caja_efectivo = 1
         WHERE nombre IN ('Efectivo DOP', 'Efectivo USD');",
        [],
    )?;

    // Los gastos en efectivo históricos no referenciaban la caja de ninguna
    // forma: se vinculaban por nombre en tiempo de escritura y guardaban
    // `cuenta_ahorro_id` nulo. Se les da la referencia que les corresponde
    // según su divisa.
    conn.execute(
        "UPDATE gastos SET cuenta_ahorro_id = (
             SELECT c.id FROM cuentas_ahorro c
             WHERE c.es_caja_efectivo = 1 AND c.divisa = gastos.divisa
         )
         WHERE metodo_pago = 'efectivo' AND cuenta_ahorro_id IS NULL;",
        [],
    )?;

    Ok(())
}
