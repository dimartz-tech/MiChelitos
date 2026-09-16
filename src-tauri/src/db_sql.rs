use crate::migraciones::{self, ErrorMigracion};
use rusqlite::{Connection, Result, Transaction};
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

/// Prepara el almacenamiento: respalda, migra y siembra.
///
/// **Respalda antes de tocar nada.** Si la base ya existe y ha cambiado desde
/// el último respaldo, se toma uno consistente y verificado; si no se puede
/// tomar, la preparación se detiene sin haber modificado el esquema. Migrar
/// sin red es precisamente el riesgo del que esto protege, de modo que un
/// fallo aquí es motivo para no continuar, no para seguir con una advertencia.
///
/// La preparación de una instalación nueva no respalda nada: no hay nada que
/// perder todavía.
pub fn inicializar_db() -> Result<()> {
    if let Err(e) = crate::respaldo::respaldar_si_hace_falta("antes de preparar el esquema") {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "No se preparó la base porque antes no se pudo respaldar. {}",
            e
        )));
    }

    let mut conn = obtener_conexion()?;
    crear_esquema(&mut conn)
}

/// Si una tabla tiene esa columna. Para condiciones esperadas dentro de una
/// migración; los fallos reales los propaga quien la llama.
fn columna_existe_en(
    tx: &Transaction,
    tabla: &str,
    columna: &str,
) -> Result<bool, ErrorMigracion> {
    let cuenta: i64 = tx
        .query_row(
            &format!("SELECT COUNT(*) FROM pragma_table_info('{}') WHERE name = ?;", tabla),
            [columna],
            |r| r.get(0),
        )
        .map_err(|e| ErrorMigracion::Almacenamiento(e.to_string()))?;
    Ok(cuenta > 0)
}

const MIG1: &str = "estructura";
const MIG2: &str = "transformaciones históricas";

/// Migración 1 — estructura.
///
/// Crea las tablas y añade las columnas que falten. **No transforma datos**:
/// esa parte vive en la migración 2, porque mezclarlas hacía imposible saber
/// si un fallo había dejado el esquema a medias o los datos a medias.
pub fn migracion_1_estructura(tx: &Transaction) -> Result<(), ErrorMigracion> {

    // 1. Tabla de Clientes
    tx.execute(
        "CREATE TABLE IF NOT EXISTS clientes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            rnc TEXT UNIQUE NOT NULL,
            nombre TEXT NOT NULL
        );",
        [],
    )?;

    // 2. Tabla de Ingresos (Facturas)
    tx.execute(
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
    tx.execute(
        "CREATE TABLE IF NOT EXISTS categorias (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            nombre TEXT UNIQUE NOT NULL
        );",
        [],
    )?;

    // 4. Tabla de Tarjetas de Crédito (Double-balance version)
    tx.execute(
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
    if !columna_existe_en(tx, "tarjetas", "balance_pesos")? {
        migraciones::anadir_columna(tx, MIG1, "tarjetas", "balance_pesos", "REAL NOT NULL DEFAULT 0.0")?;
        migraciones::anadir_columna(tx, MIG1, "tarjetas", "balance_dolares", "REAL NOT NULL DEFAULT 0.0")?;
        migraciones::anadir_columna(tx, MIG1, "tarjetas", "limite_pesos", "REAL NOT NULL DEFAULT 0.0")?;
        migraciones::anadir_columna(tx, MIG1, "tarjetas", "limite_dolares", "REAL NOT NULL DEFAULT 0.0")?;
        migraciones::anadir_columna(tx, MIG1, "tarjetas", "limite_sobregiro_pesos", "REAL NOT NULL DEFAULT 0.0")?;
        migraciones::anadir_columna(tx, MIG1, "tarjetas", "limite_sobregiro_dolares", "REAL NOT NULL DEFAULT 0.0")?;
        migraciones::anadir_columna(tx, MIG1, "tarjetas", "balance_corte_pesos", "REAL NOT NULL DEFAULT 0.0")?;
        migraciones::anadir_columna(tx, MIG1, "tarjetas", "balance_corte_dolares", "REAL NOT NULL DEFAULT 0.0")?;

        // Traspasar datos antiguos si existían
        // Traslado desde el esquema antiguo de una sola divisa. Solo aplica si
        // esas columnas están: comprobarlo es lo que permite propagar
        // cualquier otro fallo en vez de descartarlo.
        if columna_existe_en(tx, "tarjetas", "balance_actual")? {
            migraciones::paso(
                tx,
                MIG1,
                "trasladar balances de la tarjeta a la columna en pesos",
                "UPDATE tarjetas SET balance_pesos = balance_actual, limite_pesos = limite;",
            )?;
        }
    }

    // Límite ajustado: tope opcional que el titular se impone por debajo del
    // aprobado. NULL significa "sin ajuste", lo que deja libre el cero para
    // expresar una tarjeta deliberadamente congelada.
    if !columna_existe_en(tx, "tarjetas", "limite_ajustado_pesos")? {
        migraciones::anadir_columna(tx, MIG1, "tarjetas", "limite_ajustado_pesos", "REAL")?;
        migraciones::anadir_columna(tx, MIG1, "tarjetas", "limite_ajustado_dolares", "REAL")?;
    }

    // 5. Cuentas de Ahorro
    tx.execute(
        "CREATE TABLE IF NOT EXISTS cuentas_ahorro (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            nombre TEXT UNIQUE NOT NULL,
            divisa TEXT CHECK(divisa IN ('DOP', 'USD')) NOT NULL DEFAULT 'DOP',
            balance_actual REAL NOT NULL DEFAULT 0.0
        );",
        [],
    )?;

    // 6. Tabla de Gastos
    tx.execute(
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
    if !columna_existe_en(tx, "gastos", "cuenta_ahorro_id")? {
        migraciones::anadir_columna(tx, MIG1, "gastos", "cuenta_ahorro_id", "INTEGER REFERENCES cuentas_ahorro(id) ON DELETE SET NULL")?;
    }

    // Conversión de divisa de un gasto pagado desde una cuenta de otra
    // moneda. NULL en las tres columnas significa que no hubo conversión.
    // monto_liquidado guarda el importe que REALMENTE salió de la cuenta, y
    // es el autoritativo para revertir: recalcularlo desde la tasa podría
    // desviarse en centavos.
    if !columna_existe_en(tx, "gastos", "tasa_conversion")? {
        migraciones::anadir_columna(tx, MIG1, "gastos", "tasa_conversion", "REAL")?;
        migraciones::anadir_columna(tx, MIG1, "gastos", "monto_liquidado", "REAL")?;
        migraciones::anadir_columna(tx, MIG1, "gastos", "divisa_liquidada", "TEXT")?;
    }

    // Estado del consumo respecto a la conversión: NULL cuando no aplica,
    // 'pendiente' mientras el emisor no fija el importe en moneda local, y
    // 'liquidado' una vez lo fija.
    if !columna_existe_en(tx, "gastos", "estado_conversion")? {
        migraciones::anadir_columna(tx, MIG1, "gastos", "estado_conversion", "TEXT")?;
    }

    // Política de liquidación del emisor. NULL equivale a 'origen', que es la
    // que no introduce consumos pendientes.
    if !columna_existe_en(tx, "tarjetas", "politica_liquidacion")? {
        migraciones::anadir_columna(tx, MIG1, "tarjetas", "politica_liquidacion", "TEXT")?;
    }

    // 7. Tabla de Pagos de Tarjetas (Abonos)
    tx.execute(
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
    if !columna_existe_en(tx, "pagos_tarjeta", "divisa")? {
        migraciones::anadir_columna(tx, MIG1, "pagos_tarjeta", "divisa", "TEXT CHECK(divisa IN ('DOP', 'USD')) NOT NULL DEFAULT 'DOP'")?;
    }

    // 12. Bonificaciones acreditadas por el emisor sobre una tarjeta.
    // Son créditos aparte, nunca una reducción del consumo original, y un
    // mismo gasto puede generar varias, así que gasto_id no es único ni
    // obligatorio: los estados no dicen a qué consumo corresponde cada uno.
    tx.execute(
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
    tx.execute(
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
    if !columna_existe_en(tx, "suscripciones", "dia_facturacion")? {
        migraciones::anadir_columna(tx, MIG1, "suscripciones", "dia_facturacion", "INTEGER NOT NULL DEFAULT 1")?;
        migraciones::anadir_columna(tx, MIG1, "suscripciones", "fecha_ultimo_pago", "TEXT")?;
        migraciones::anadir_columna(tx, MIG1, "suscripciones", "divisa", "TEXT CHECK(divisa IN ('DOP', 'USD')) NOT NULL DEFAULT 'DOP'")?;
    }

    // 9. Tabla de Ingresos Informales
    tx.execute(
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
    tx.execute(
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
    tx.execute(
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


    // --- Saldo vivo de financiamientos ---
    //
    // El pasivo de un préstamo se deducía de `monto_prestamo` y la fracción de
    // cuotas pendientes. Eso supone amortización lineal, que nunca es el caso,
    // y en una línea de crédito ni siquiera se aplicaba: al no tener cuotas
    // contadas, el pasivo quedaba clavado en el monto desembolsado el primer
    // día y ningún pago lo movía.
    //
    // Se sustituye por un saldo que se lleva. Las sentencias son idempotentes.
    migraciones::anadir_columna(tx, MIG1, "prestamos", "saldo_actual", "REAL")?;
    migraciones::anadir_columna(tx, MIG1, "prestamos", "limite_credito", "REAL")?;

    // Vínculo con la tarjeta que cobra el financiamiento.
    //
    // Algunas facilidades no son productos independientes: viven bajo una
    // tarjeta, comparten su ciclo de corte y se cobran dentro de su pago
    // mínimo. Sin esta referencia, sus fechas habría que duplicarlas —y
    // mantenerlas sincronizadas a mano— y nada advertiría de que su saldo y el
    // balance de la tarjeta pueden solaparse.
    migraciones::anadir_columna(tx, MIG1, "prestamos", "tarjeta_id", "INTEGER REFERENCES tarjetas(id) ON DELETE SET NULL")?;

    // Libro de movimientos del financiamiento. Sin él, el saldo sería un
    // número que muta sin rastro: no se podría reconstruir cómo llegó a valer
    // lo que vale, ni distinguir una cuota de una corrección.
    tx.execute(
        "CREATE TABLE IF NOT EXISTS movimientos_prestamo (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            prestamo_id INTEGER NOT NULL,
            fecha TEXT NOT NULL,
            tipo TEXT CHECK(tipo IN ('cuota', 'declaracion', 'disposicion')) NOT NULL,
            monto REAL NOT NULL,
            interes REAL NOT NULL DEFAULT 0.0,
            capital REAL NOT NULL DEFAULT 0.0,
            saldo_resultante REAL NOT NULL,
            FOREIGN KEY (prestamo_id) REFERENCES prestamos(id) ON DELETE CASCADE
        );",
        [],
    )?;

    // --- Resolución de H3: la caja de efectivo deja de identificarse por su
    // nombre. Buscarla con `WHERE nombre = 'Efectivo DOP'` hacía que un
    // renombrado —o un borrado, que la guarda de `eliminar_cuenta` no impedía
    // porque ningún gasto la referenciaba por id— dejara el gasto registrado
    // sin mover ningún saldo, devolviendo `Ok`.
    //
    // Se sustituye por un papel explícito en la fila y por la referencia real
    // en cada gasto. Las tres sentencias son idempotentes.
    migraciones::anadir_columna(tx, MIG1, "cuentas_ahorro", "es_caja_efectivo", "INTEGER NOT NULL DEFAULT 0")?;
    Ok(())
}

/// Migración 2 — transformaciones históricas.
///
/// Da a los datos ya registrados la forma que el esquema nuevo espera. Todas
/// son de una sola dirección y están acotadas por una condición que deja de
/// cumplirse en cuanto se aplican, de modo que repetirlas no las duplica.
pub fn migracion_2_transformaciones(tx: &Transaction) -> Result<(), ErrorMigracion> {
    // Relleno inicial del saldo vivo. Se parte del mismo valor que la interfaz
    // venía mostrando, para que la migración no haga saltar el patrimonio. Es
    // un punto de partida, no un dato bueno: se corrige declarando el saldo
    // real del estado de cuenta, que es la única cifra que cuadra con el
    // acreedor.
    migraciones::paso(
        tx,
        MIG2,
        "rellenar el saldo vivo de los financiamientos",
        "UPDATE prestamos SET saldo_actual = ROUND(
             CASE
                 WHEN cuotas_totales IS NULL OR cuotas_totales = 0 THEN monto_prestamo
                 ELSE (CAST(cuotas_pendientes AS REAL) / cuotas_totales) * monto_prestamo
             END, 2)
         WHERE saldo_actual IS NULL;",
    )?;

    // El nombre se usa una única vez, aquí, para marcar las cajas que ya
    // existían. A partir de este punto el vínculo es el papel, no el texto.
    // Las cajas nuevas nacen ya marcadas desde las semillas.
    migraciones::paso(
        tx,
        MIG2,
        "marcar las cajas de efectivo existentes",
        "UPDATE cuentas_ahorro SET es_caja_efectivo = 1
         WHERE nombre IN ('Efectivo DOP', 'Efectivo USD');",
    )?;

    // Los gastos en efectivo históricos no referenciaban la caja de ninguna
    // forma: se vinculaban por nombre en tiempo de escritura y guardaban
    // `cuenta_ahorro_id` nulo. Se les da la referencia que les corresponde
    // según su divisa.
    migraciones::paso(
        tx,
        MIG2,
        "vincular los gastos en efectivo con su caja",
        "UPDATE gastos SET cuenta_ahorro_id = (
             SELECT c.id FROM cuentas_ahorro c
             WHERE c.es_caja_efectivo = 1 AND c.divisa = gastos.divisa
         )
         WHERE metodo_pago = 'efectivo' AND cuenta_ahorro_id IS NULL;",
    )?;

    Ok(())
}


/// Columnas que guardan **dinero**, y por tanto deben caer en centavos exactos.
///
/// La lista es explícita a propósito. Barrer todas las columnas `REAL` habría
/// arrastrado las **tasas** —`tasa_actual`, `tasa_cambio`, `tasa_conversion`,
/// `porcentaje_retencion`—, que no son importes y cuya precisión es
/// justamente lo que no hay que recortar: una tasa de 58.9642 redondeada a dos
/// decimales deja de servir para reconstruir una conversión.
pub const COLUMNAS_DE_DINERO: &[(&str, &str)] = &[
    ("cuentas_ahorro", "balance_actual"),
    ("gastos", "monto"),
    ("gastos", "costo_adicional"),
    ("gastos", "monto_liquidado"),
    ("ingresos", "monto_total"),
    ("ingresos", "monto_retenido"),
    ("ingresos", "monto_recibido"),
    ("ingresos_informales", "monto"),
    ("ingresos_informales", "monto_recibido"),
    ("pagos_tarjeta", "monto_pagado"),
    ("prestamos", "monto_prestamo"),
    ("prestamos", "monto_cuota"),
    ("prestamos", "saldo_actual"),
    ("prestamos", "limite_credito"),
    ("suscripciones", "monto"),
    ("tarjetas", "limite"),
    ("tarjetas", "balance_actual"),
    ("tarjetas", "balance_pesos"),
    ("tarjetas", "balance_dolares"),
    ("tarjetas", "limite_pesos"),
    ("tarjetas", "limite_dolares"),
    ("tarjetas", "limite_sobregiro_pesos"),
    ("tarjetas", "limite_sobregiro_dolares"),
    ("tarjetas", "balance_corte_pesos"),
    ("tarjetas", "balance_corte_dolares"),
    ("tarjetas", "limite_ajustado_pesos"),
    ("tarjetas", "limite_ajustado_dolares"),
    ("transacciones_cuentas", "monto_origen"),
    ("transacciones_cuentas", "monto_destino"),
    ("transacciones_cuentas", "cargo"),
    ("movimientos_prestamo", "monto"),
    ("movimientos_prestamo", "interes"),
    ("movimientos_prestamo", "capital"),
    ("movimientos_prestamo", "saldo_resultante"),
    ("bonificaciones", "monto"),
];

const MIG3: &str = "importes en centavos exactos";

/// Migración 3 — todos los importes caen en un centavo exacto.
///
/// El dominio trabaja en centavos enteros desde la Fase 1, pero la base seguía
/// guardando los importes como números con coma. Eso permitió que se colara un
/// tercer decimal, residuo de cuando el 0.20 % se calculaba en tres sitios con
/// dos criterios de redondeo distintos (H8): el gasto quedaba con fracción de
/// centavo y el saldo de la cuenta heredaba la deriva al debitarse.
///
/// Esta migración los lleva al centavo más cercano. **No oculta la
/// diferencia**: al terminar comprueba que no queda ni un importe fuera de
/// centavo y falla si lo hay, de modo que la conversión se acepta solo cuando
/// es verificable, en vez de darse por buena porque no falló.
///
/// Es el paso previo a cambiar el tipo de las columnas: hacerlo con valores ya
/// exactos convierte ese cambio en una operación sin pérdida.
pub fn migracion_3_centavos_exactos(tx: &Transaction) -> Result<(), ErrorMigracion> {
    for (tabla, columna) in COLUMNAS_DE_DINERO {
        // Una columna puede no existir: las heredadas del esquema de una sola
        // divisa desaparecieron, y las nuevas no están en bases antiguas.
        if !tabla_existe(tx, tabla)? || !columna_existe_en(tx, tabla, columna)? {
            continue;
        }

        migraciones::paso(
            tx,
            MIG3,
            &format!("redondear {}.{} al centavo", tabla, columna),
            &format!(
                "UPDATE {t} SET {c} = ROUND({c}, 2)
                 WHERE {c} IS NOT NULL AND ABS({c} * 100 - ROUND({c} * 100)) > 1e-6;",
                t = tabla,
                c = columna
            ),
        )?;
    }

    verificar_centavos_exactos(tx)
}

/// Comprueba que no queda ni un importe fuera de centavo.
///
/// Es la condición que hace la conversión **aceptable**: sin ella, redondear
/// sería una operación que se da por buena porque no falló, que es
/// precisamente la clase de silencio que este proyecto viene retirando.
fn verificar_centavos_exactos(tx: &Transaction) -> Result<(), ErrorMigracion> {
    for (tabla, columna) in COLUMNAS_DE_DINERO {
        if !tabla_existe(tx, tabla)? || !columna_existe_en(tx, tabla, columna)? {
            continue;
        }

        let fuera: i64 = tx
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM {t}
                     WHERE {c} IS NOT NULL AND ABS({c} * 100 - ROUND({c} * 100)) > 1e-6;",
                    t = tabla,
                    c = columna
                ),
                [],
                |r| r.get(0),
            )
            .map_err(|e| ErrorMigracion::Almacenamiento(e.to_string()))?;

        if fuera > 0 {
            return Err(ErrorMigracion::Fallo {
                version: 3,
                migracion: MIG3,
                etapa: format!("verificar {}.{}", tabla, columna),
                causa: format!("quedan {} importes fuera de centavo", fuera),
            });
        }
    }
    Ok(())
}

/// Acceso a la verificación desde las pruebas del ejecutor.
#[cfg(test)]
pub fn verificar_centavos_exactos_para_pruebas(tx: &Transaction) -> Result<(), ErrorMigracion> {
    verificar_centavos_exactos(tx)
}

fn tabla_existe(tx: &Transaction, tabla: &str) -> Result<bool, ErrorMigracion> {
    let n: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?;",
            [tabla],
            |r| r.get(0),
        )
        .map_err(|e| ErrorMigracion::Almacenamiento(e.to_string()))?;
    Ok(n > 0)
}

/// Semillas de sistema: los registros que la aplicación necesita para operar.
///
/// Van aparte de las migraciones porque no describen una versión del esquema
/// sino un mínimo que debe existir siempre. Las cajas se crean **ya marcadas**
/// con su papel: la migración 2 solo tiene que marcar las que venían de antes.
pub fn sembrar(conn: &Connection) -> Result<()> {
    let categorias: i64 = conn.query_row("SELECT COUNT(*) FROM categorias;", [], |r| r.get(0))?;
    if categorias == 0 {
        let por_defecto = [
            "Alimentación", "Transporte", "Servicios Públicos", "Alquiler",
            "Suscripciones", "Entretenimiento", "Impuestos", "Seguros",
            "Maquinaria/Equipos", "Otros",
        ];
        for categoria in &por_defecto {
            conn.execute("INSERT INTO categorias (nombre) VALUES (?);", [categoria])?;
        }
    }

    for (nombre, divisa) in [("Efectivo DOP", "DOP"), ("Efectivo USD", "USD")] {
        conn.execute(
            "INSERT OR IGNORE INTO cuentas_ahorro (nombre, divisa, balance_actual, es_caja_efectivo)
             VALUES (?, ?, 0.0, 1);",
            [nombre, divisa],
        )?;
    }

    Ok(())
}

/// Prepara el esquema completo sobre una conexión dada.
///
/// Separarlo permite levantar una base en memoria en las pruebas del
/// adaptador, sin depender de `HOME` ni tocar disco.
const MIG4: &str = "identidad y comisiones de las cuentas";

/// Da a cada cuenta una entidad emisora y su comisión por pago de impuestos.
///
/// Las dos columnas quedan **vacías a propósito**. Rellenarlas exigiría
/// deducir el banco a partir del nombre que el titular le puso a la cuenta, y
/// eso significaría escribir en el repositorio —que es público— el mapa de con
/// qué entidades opera. La entidad es un dato suyo, no del programa: la
/// declara él desde la interfaz.
///
/// `comision_pago_impuestos` es nula mientras no se declare, y nulo no
/// significa cero: significa «esta cuenta no tiene una comisión fija pactada»,
/// que es distinto de «cobra cero». La diferencia importa porque de ella
/// depende si se aplica la retención ordinaria o el precio fijo del servicio.
pub fn migracion_4_identidad_de_cuentas(tx: &Transaction) -> Result<(), ErrorMigracion> {
    migraciones::anadir_columna(tx, MIG4, "cuentas_ahorro", "entidad", "TEXT")?;
    migraciones::anadir_columna(
        tx,
        MIG4,
        "cuentas_ahorro",
        "comision_pago_impuestos",
        "REAL",
    )?;
    Ok(())
}

pub fn crear_esquema(conn: &mut Connection) -> Result<()> {
    migraciones::ejecutar(conn)
        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
    sembrar(conn)
}
