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
    // Añadidas después de la migración 3, de modo que nacieron fuera de su
    // red. Es el hueco que documenta `politica_redondeo.md`: la migración 9
    // las alcanza, y `toda_columna_real_esta_clasificada` impide que vuelva a
    // pasar.
    ("cuentas_ahorro", "comision_pago_impuestos"),
    ("pagos_tarjeta", "monto_debitado"),
    ("pagos_tarjeta", "comision"),
    ("correcciones", "importe"),
];

/// Columnas `REAL` que **no** son dinero: tasas y porcentajes.
///
/// Existe por simetría con `COLUMNAS_DE_DINERO`, y juntas forman la regla: en
/// un esquema migrado, **toda** columna `REAL` tiene que estar en una de las
/// dos listas. Antes solo había una guardiana —la que impide meter una tasa
/// entre el dinero— y protegía de meter de más, no de olvidar de menos. Cuatro
/// columnas de dinero se olvidaron por ahí.
pub const COLUMNAS_DE_TASA: &[(&str, &str)] = &[
    ("gastos", "tasa_conversion"),
    ("ingresos", "porcentaje_retencion"),
    ("pagos_tarjeta", "tasa_cambio"),
    ("prestamos", "tasa_actual"),
    ("transacciones_cuentas", "tasa_cambio"),
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

const MIG5: &str = "vínculo del abono con lo que lo pagó";

/// Ata cada abono a tarjeta con la cuenta que lo pagó y su comisión.
///
/// Un abono guardaba solo la tarjeta, la fecha, el importe y la divisa. Todo
/// lo demás que la operación movía —de qué cuenta salió el dinero, a qué tasa
/// se convirtió, qué gasto recogió la comisión— quedaba fuera. Por eso no
/// existía reversión: no había forma de saber qué deshacer.
///
/// La tasa, además, vivía dentro del texto de la descripción de la comisión,
/// que es el mismo defecto que se corrigió en los gastos (**H9**). Aquí se lee
/// esa descripción **una sola vez**, para rellenar la columna, y nunca más.
pub fn migracion_5_vinculo_de_abonos(tx: &Transaction) -> Result<(), ErrorMigracion> {
    migraciones::anadir_columna(
        tx, MIG5, "pagos_tarjeta", "cuenta_ahorro_id",
        "INTEGER REFERENCES cuentas_ahorro(id) ON DELETE SET NULL",
    )?;
    migraciones::anadir_columna(tx, MIG5, "pagos_tarjeta", "tasa_cambio", "REAL")?;
    migraciones::anadir_columna(
        tx, MIG5, "pagos_tarjeta", "gasto_comision_id",
        "INTEGER REFERENCES gastos(id) ON DELETE SET NULL",
    )?;

    // El emparejamiento es exacto, no aproximado: la comisión es el 0.20 % del
    // importe ya convertido, de modo que conociendo la tasa se reconstruye el
    // céntimo. La tolerancia de medio céntimo absorbe que los importes
    // antiguos se guardaran sin redondear.
    //
    // Un abono sin comisión —pagado sin cuenta de la que debitar— no encuentra
    // pareja y conserva sus columnas nulas, que es lo correcto: no hubo
    // movimiento de cuenta que deshacer.
    migraciones::paso(
        tx,
        MIG5,
        "vincular abonos históricos con su comisión",
        "WITH comisiones AS (
             SELECT g.id, g.fecha, g.monto, g.cuenta_ahorro_id,
                    CASE WHEN INSTR(g.descripcion, '(Tasa ') > 0
                         THEN CAST(REPLACE(
                                  SUBSTR(g.descripcion, INSTR(g.descripcion, '(Tasa ') + 6),
                                  ')', '') AS REAL)
                         ELSE 1.0 END AS tasa
             FROM gastos g
             WHERE g.descripcion LIKE 'Comisión 0.20% Pago Tarjeta%'
         )
         UPDATE pagos_tarjeta SET
             gasto_comision_id = (
                 SELECT c.id FROM comisiones c
                 WHERE c.fecha = pagos_tarjeta.fecha_pago
                   AND ABS(c.monto - pagos_tarjeta.monto_pagado * c.tasa * 0.002) < 0.005
             ),
             cuenta_ahorro_id = (
                 SELECT c.cuenta_ahorro_id FROM comisiones c
                 WHERE c.fecha = pagos_tarjeta.fecha_pago
                   AND ABS(c.monto - pagos_tarjeta.monto_pagado * c.tasa * 0.002) < 0.005
             ),
             tasa_cambio = (
                 SELECT c.tasa FROM comisiones c
                 WHERE c.fecha = pagos_tarjeta.fecha_pago
                   AND ABS(c.monto - pagos_tarjeta.monto_pagado * c.tasa * 0.002) < 0.005
             )
         WHERE gasto_comision_id IS NULL;",
    )?;

    verificar_vinculos_unicos(tx)
}

/// Comprueba que ninguna comisión quedó atada a dos abonos.
///
/// Sin esto, el emparejamiento sería una operación que se da por buena porque
/// no falló. Si dos abonos reclamaran el mismo gasto, revertir uno dejaría al
/// otro apuntando a una fila borrada.
fn verificar_vinculos_unicos(tx: &Transaction) -> Result<(), ErrorMigracion> {
    let duplicados: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM (
                 SELECT gasto_comision_id FROM pagos_tarjeta
                 WHERE gasto_comision_id IS NOT NULL
                 GROUP BY gasto_comision_id HAVING COUNT(*) > 1
             );",
            [],
            |r| r.get(0),
        )
        .map_err(|e| ErrorMigracion::Fallo {
            version: 0,
            migracion: MIG5,
            etapa: "comprobar unicidad del vínculo".to_string(),
            causa: e.to_string(),
        })?;

    if duplicados > 0 {
        return Err(ErrorMigracion::EstructuraInesperada {
            migracion: MIG5,
            detalle: format!(
                "{} comisiones quedaron atadas a más de un abono; el vínculo no es fiable",
                duplicados
            ),
        });
    }
    Ok(())
}

const MIG6: &str = "el abono guarda lo que debitó";

/// Guarda en el abono el importe debitado y su comisión.
///
/// Hasta ahora el abono guardaba el importe pagado y la tasa, y **revertirlo
/// obligaba a recalcular** lo que había salido de la cuenta. Recalcular
/// devuelve lo que hoy creemos que debió salir, no lo que salió: si la regla
/// de redondeo cambia, o el importe se corrigió a mano, la reversión deja un
/// residuo silencioso.
///
/// El relleno usa **el importe y la tasa**, no la comisión. Derivar el débito
/// dividiendo la comisión entre 0.002 solo es exacto mientras la comisión
/// conserve sus decimales: una vez redondeada al céntimo, la división se
/// desvía —42.77 / 0.002 da 21 385.00 para un débito real de 21 384.61—.
///
/// Para las filas anteriores a esta columna el valor es una **reconstrucción**,
/// no un registro: se calcula como se calculó entonces. Es lo mejor
/// disponible, y a partir de aquí deja de hacer falta.
pub fn migracion_6_abono_guarda_lo_debitado(tx: &Transaction) -> Result<(), ErrorMigracion> {
    migraciones::anadir_columna(tx, MIG6, "pagos_tarjeta", "monto_debitado", "REAL")?;
    migraciones::anadir_columna(tx, MIG6, "pagos_tarjeta", "comision", "REAL")?;

    migraciones::paso(
        tx,
        MIG6,
        "reconstruir el débito de los abonos anteriores",
        "UPDATE pagos_tarjeta SET
             monto_debitado = ROUND(monto_pagado * COALESCE(NULLIF(tasa_cambio, 0), 1.0), 2)
         WHERE cuenta_ahorro_id IS NOT NULL AND monto_debitado IS NULL;",
    )?;

    // La comisión sí se toma del gasto que la recogió: ahí está el importe
    // que de verdad se cobró, sin reconstruir nada.
    migraciones::paso(
        tx,
        MIG6,
        "tomar la comisión del gasto que la recogió",
        "UPDATE pagos_tarjeta SET
             comision = (SELECT g.monto FROM gastos g WHERE g.id = pagos_tarjeta.gasto_comision_id)
         WHERE gasto_comision_id IS NOT NULL AND comision IS NULL;",
    )?;

    Ok(())
}

const MIG7: &str = "el cobro apunta a una cuenta, no a un nombre";

/// Vincula el cobro de un ingreso con la cuenta que lo recibió.
///
/// `institucion_deposito` guardaba el **nombre** de la cuenta, y el abono se
/// aplicaba con `UPDATE ... WHERE nombre = ?` descartando el resultado. Si el
/// nombre no coincidía, el ingreso quedaba cobrado y ningún saldo se movía
/// (**H17**). Es el mismo defecto que H3 tenía con la caja de efectivo, y se
/// cierra igual: por referencia, no por texto.
///
/// El nombre se conserva. Sirve para leer el histórico y para los cobros que
/// se registraron contra una cuenta que ya no existe, donde no hay id que
/// poner.
pub fn migracion_7_cobro_por_referencia(tx: &Transaction) -> Result<(), ErrorMigracion> {
    for tabla in ["ingresos", "ingresos_informales"] {
        migraciones::anadir_columna(
            tx, MIG7, tabla, "cuenta_ahorro_id",
            "INTEGER REFERENCES cuentas_ahorro(id) ON DELETE SET NULL",
        )?;

        migraciones::paso(
            tx,
            MIG7,
            &format!("vincular los cobros de {} con su cuenta", tabla),
            &format!(
                "UPDATE {t} SET cuenta_ahorro_id = (
                     SELECT c.id FROM cuentas_ahorro c WHERE c.nombre = {t}.institucion_deposito
                 )
                 WHERE cuenta_ahorro_id IS NULL AND institucion_deposito IS NOT NULL;",
                t = tabla
            ),
        )?;
    }

    Ok(())
}

const MIG8: &str = "casos de corrección";

/// Registro de las correcciones que borran un movimiento.
///
/// **Es una medida temporal, y conviene que conste.** La solución buena son
/// los asientos de compensación de la Fase 8: un movimiento que se anula deja
/// su contrario, y el original sigue ahí. Mientras eso no exista, borrar
/// destruye el rastro, y esto al menos deja constancia de qué se destruyó y
/// por qué.
///
/// Tiene un segundo propósito, tan importante como el primero: **exigir un
/// motivo escrito es fricción deliberada**. Una corrección que cuesta un
/// párrafo se piensa dos veces, y la mayoría de estas situaciones se evitan
/// antes de ocurrir.
pub fn migracion_8_casos_de_correccion(tx: &Transaction) -> Result<(), ErrorMigracion> {
    migraciones::paso(
        tx,
        MIG8,
        "crear la tabla de casos",
        "CREATE TABLE IF NOT EXISTS correcciones (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             numero_caso TEXT UNIQUE NOT NULL,
             fecha TEXT NOT NULL,
             tipo TEXT NOT NULL,
             referencia_id INTEGER NOT NULL,
             descripcion TEXT NOT NULL,
             importe REAL,
             divisa TEXT,
             motivo TEXT NOT NULL
         );",
    )?;

    Ok(())
}

const MIG9: &str = "las columnas que nacieron fuera de la red";

/// Redondea al céntimo las columnas de dinero añadidas **después** de la
/// migración 3.
///
/// La migración 3 lleva todos los importes al céntimo exacto y lo verifica,
/// pero solo alcanza a lo que estaba en `COLUMNAS_DE_DINERO` cuando corrió.
/// Las migraciones 4, 6 y 8 añadieron cuatro columnas de dinero después, y
/// ninguna entró en esa red: nacieron sin redondear y sin verificar.
///
/// Una de ellas, `cuentas_ahorro.comision_pago_impuestos`, además se escribía
/// sin pasar por `Dinero` —esa vía ya está cerrada—, de modo que podía traer
/// un tercer decimal desde la interfaz.
///
/// Reutiliza la misma mecánica y la misma verificación que la 3. Que haga
/// falta una migración aparte para esto es, en sí, el argumento de la regla
/// `toda_columna_real_esta_clasificada_como_dinero_o_como_tasa`: sin ella, la
/// próxima columna de dinero volvería a nacer fuera.
pub fn migracion_9_columnas_tardias(tx: &Transaction) -> Result<(), ErrorMigracion> {
    const TARDIAS: &[(&str, &str)] = &[
        ("cuentas_ahorro", "comision_pago_impuestos"),
        ("pagos_tarjeta", "monto_debitado"),
        ("pagos_tarjeta", "comision"),
        ("correcciones", "importe"),
    ];

    for (tabla, columna) in TARDIAS {
        if !tabla_existe(tx, tabla)? || !columna_existe_en(tx, tabla, columna)? {
            continue;
        }

        migraciones::paso(
            tx,
            MIG9,
            &format!("redondear {}.{} al centavo", tabla, columna),
            &format!(
                "UPDATE {t} SET {c} = ROUND({c}, 2)
                 WHERE {c} IS NOT NULL AND ABS({c} * 100 - ROUND({c} * 100)) > 1e-6;",
                t = tabla,
                c = columna
            ),
        )?;
    }

    // La misma verificación que la 3, ahora sobre la lista completa: la
    // conversión se acepta solo cuando es comprobable.
    verificar_centavos_exactos(tx)
}

const MIG10: &str = "el céntimo exacto, sin tolerancia";

/// Lleva los importes al céntimo **exacto**, no al céntimo dentro de una
/// tolerancia.
///
/// Las migraciones 3 y 9 redondean solo lo que se desvía más de `1e-6`
/// centavos. Es coherente con la tolerancia de representación del sistema, y
/// aun así deja un resto: sobre una base real quedaban **doce valores** que
/// difieren de su propio redondeo en torno a `1e-12` unidades. No son
/// fracciones de céntimo —su fracción medida es cero— sino **ruido de
/// representación**, casi seguro de balances acumulados en coma flotante.
///
/// Inofensivos hoy, y aun así hay que quitarlos: son la diferencia entre «no
/// se desvía lo bastante para importar» y «es exacto». La migración 11 exige
/// lo segundo, y sin este paso rechazaría doce importes buenos.
pub fn migracion_10_centimo_exacto(tx: &Transaction) -> Result<(), ErrorMigracion> {
    for (tabla, columna) in COLUMNAS_DE_DINERO {
        if !tabla_existe(tx, tabla)? || !columna_existe_en(tx, tabla, columna)? {
            continue;
        }

        migraciones::paso(
            tx,
            MIG10,
            &format!("exactitud de {}.{}", tabla, columna),
            &format!(
                "UPDATE {t} SET {c} = ROUND({c}, 2)
                 WHERE {c} IS NOT NULL AND ROUND({c}, 2) <> {c};",
                t = tabla,
                c = columna
            ),
        )?;
    }

    verificar_centimo_exacto(tx)
}

/// Como `verificar_centavos_exactos`, pero **sin tolerancia**.
///
/// La comparación es la misma que impondrá el `CHECK`, de modo que lo que
/// aquí pasa es exactamente lo que allí pasará. Usar un criterio en la
/// migración y otro en la restricción es la vía a una migración que se acepta
/// y un esquema que luego no admite sus propios datos.
fn verificar_centimo_exacto(tx: &Transaction) -> Result<(), ErrorMigracion> {
    for (tabla, columna) in COLUMNAS_DE_DINERO {
        if !tabla_existe(tx, tabla)? || !columna_existe_en(tx, tabla, columna)? {
            continue;
        }

        let fuera: i64 = tx
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM {t} WHERE {c} IS NOT NULL AND ROUND({c}, 2) <> {c};",
                    t = tabla,
                    c = columna
                ),
                [],
                |r| r.get(0),
            )
            .map_err(|e| ErrorMigracion::Almacenamiento(e.to_string()))?;

        if fuera > 0 {
            return Err(ErrorMigracion::Fallo {
                version: 10,
                migracion: MIG10,
                etapa: format!("verificar {}.{}", tabla, columna),
                causa: format!("quedan {} importes que no son exactos al céntimo", fuera),
            });
        }
    }
    Ok(())
}

const MIG11: &str = "la fracción de céntimo se rechaza al escribir";

/// La restricción que condiciona cada columna de dinero.
///
/// `ROUND(v, 2) = v` y no una comparación sobre `v * 100`. La diferencia no
/// es de estilo: se midió, y las variantes con `* 100` **rechazan céntimos
/// legítimos** —once en un barrido, empezando por 4,77— porque multiplicar
/// por cien introduce el error que se pretendía detectar. `ROUND(v, 2) = v`
/// no rechazó ninguno en doscientos mil valores densos ni por magnitudes
/// hasta 10^14.
fn restriccion_de_centimo(columna: &str) -> String {
    format!("CHECK (ROUND({c}, 2) = {c})", c = columna)
}

/// Impide guardar una fracción de céntimo, en el propio esquema.
///
/// ## Por qué aquí y no en el tipo de la columna
///
/// Estaba previsto convertir las columnas a `INTEGER` y guardar centavos. **Se
/// midió antes de hacerlo, y la premisa era falsa**: en SQLite la afinidad
/// `INTEGER` no restringe nada. Una columna declarada `INTEGER` acepta 75.005
/// y lo guarda como `real`, porque la afinidad solo convierte cuando la
/// conversión no pierde. El cambio de tipo no habría impedido lo que se
/// quería impedir.
///
/// Y no compraba nada por otro lado: `REAL` representa centavos exactos hasta
/// 2,5·10^16, y sumar un millón de filas mezclando magnitudes se desvió cero
/// centavos. Lo único que cambiaba era el modo de fallo, y a peor: leer un
/// `INTEGER` como `f64` devuelve el entero crudo sin error —inflación de 100
/// veces en silencio— y escribir un `i64` en una columna `REAL` hace lo mismo.
/// Solo la dirección contraria, leer un `REAL` como `i64`, falla en voz alta.
///
/// De modo que el tramo se cierra como se cerró el tercero: **declarando
/// innecesaria la conversión** y quedándose con lo que sí hacía falta, que era
/// rechazar la fracción **al escribir** y no solo al migrar.
///
/// ## Por qué reconstruir
///
/// SQLite no tiene `ALTER TABLE ADD CONSTRAINT`. La única vía es rehacer la
/// tabla, y se hace insertando restricciones **de tabla** al final de la lista
/// de columnas en lugar de tocar la definición de cada columna: es una sola
/// inserción antes del paréntesis final, en vez de analizar sintaxis que ya
/// incluye claves ajenas, valores por defecto y otros `CHECK`.
///
/// Las filas se copian por nombre de columna y se comprueba el recuento antes
/// y después. Esta base no tiene índices ni disparadores de usuario —solo los
/// automáticos de `UNIQUE`, que renacen con la definición—, y eso también se
/// comprueba en vez de suponerse: si aparecieran, la migración se detiene.
pub fn migracion_11_rechazar_fraccion_de_centimo(
    tx: &Transaction,
) -> Result<(), ErrorMigracion> {
    // **Los dos `PRAGMA` del procedimiento de SQLite, y por qué cada uno.**
    //
    // `legacy_alter_table`: sin él, `RENAME TO` reescribe las cláusulas
    // `REFERENCES` de las **otras** tablas para que sigan apuntando al nombre
    // nuevo. Es lo correcto para un renombrado de verdad y lo contrario de lo
    // que hace falta aquí: al renombrar `cuentas_ahorro` a `..._previa`,
    // `gastos` pasaba a referenciar una tabla que esta misma migración borra
    // después. Se descubrió porque la suite entera se cayó de golpe.
    //
    // `defer_foreign_keys`: entre el renombrado y la creación hay un instante
    // sin la tabla referenciada. Difiere la comprobación al `COMMIT`, que es
    // cuando el esquema vuelve a ser coherente. `foreign_keys` no vale: dentro
    // de una transacción no hace nada, y esto corre dentro de una.
    tx.execute_batch("PRAGMA legacy_alter_table = ON; PRAGMA defer_foreign_keys = ON;")
        .map_err(|e| ErrorMigracion::Almacenamiento(e.to_string()))?;
    let resultado = reconstruir_con_restricciones(tx);
    let _ = tx.execute_batch("PRAGMA legacy_alter_table = OFF;");
    resultado?;

    verificar_restricciones_presentes(tx)
}

fn reconstruir_con_restricciones(tx: &Transaction) -> Result<(), ErrorMigracion> {
    let mut por_tabla: std::collections::BTreeMap<&str, Vec<&str>> = Default::default();
    for (tabla, columna) in COLUMNAS_DE_DINERO {
        por_tabla.entry(tabla).or_default().push(columna);
    }

    for (tabla, columnas) in por_tabla {
        if !tabla_existe(tx, tabla)? {
            continue;
        }

        // Un índice o un disparador de usuario no sobreviviría a la
        // reconstrucción. Hoy no los hay; si algún día los hubiera, esto para
        // la migración en vez de perderlos sin decirlo.
        let propios: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE tbl_name = ? AND type IN ('index', 'trigger')
                   AND name NOT LIKE 'sqlite_autoindex_%';",
                [tabla],
                |r| r.get(0),
            )
            .map_err(|e| ErrorMigracion::Almacenamiento(e.to_string()))?;
        if propios > 0 {
            return Err(ErrorMigracion::Fallo {
                version: 11,
                migracion: MIG11,
                etapa: format!("reconstruir {}", tabla),
                causa: format!(
                    "{} tiene {} índice(s) o disparador(es) propios;                      la reconstrucción los perdería. Recréalos aquí primero.",
                    tabla, propios
                ),
            });
        }

        let sql_actual: String = tx
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?;",
                [tabla],
                |r| r.get(0),
            )
            .map_err(|e| ErrorMigracion::Almacenamiento(e.to_string()))?;

        let presentes: Vec<&str> = columnas
            .iter()
            .copied()
            .filter(|c| columna_existe_en(tx, tabla, c).unwrap_or(false))
            .collect();
        if presentes.is_empty() {
            continue;
        }

        // Idempotente: si ya lleva sus restricciones, no se toca.
        if presentes.iter().all(|c| sql_actual.contains(&restriccion_de_centimo(c))) {
            continue;
        }

        let sql_nuevo = con_restricciones(&sql_actual, &presentes).ok_or_else(|| {
            ErrorMigracion::Fallo {
                version: 11,
                migracion: MIG11,
                etapa: format!("reescribir la definición de {}", tabla),
                causa: "no se encontró el paréntesis que cierra la lista de columnas".into(),
            }
        })?;

        let antes: i64 = tx
            .query_row(&format!("SELECT COUNT(*) FROM {};", tabla), [], |r| r.get(0))
            .map_err(|e| ErrorMigracion::Almacenamiento(e.to_string()))?;

        let nombres: Vec<String> = {
            let mut s = tx
                .prepare(&format!("SELECT name FROM pragma_table_info('{}');", tabla))
                .map_err(|e| ErrorMigracion::Almacenamiento(e.to_string()))?;
            let it = s
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(|e| ErrorMigracion::Almacenamiento(e.to_string()))?;
            it.filter_map(|x| x.ok()).collect()
        };
        let lista = nombres.join(", ");

        migraciones::paso(
            tx,
            MIG11,
            &format!("reconstruir {} con sus restricciones", tabla),
            &format!(
                "ALTER TABLE {t} RENAME TO {t}_previa;
                 {creacion};
                 INSERT INTO {t} ({lista}) SELECT {lista} FROM {t}_previa;
                 DROP TABLE {t}_previa;",
                t = tabla,
                creacion = sql_nuevo,
                lista = lista
            ),
        )?;

        let despues: i64 = tx
            .query_row(&format!("SELECT COUNT(*) FROM {};", tabla), [], |r| r.get(0))
            .map_err(|e| ErrorMigracion::Almacenamiento(e.to_string()))?;
        if antes != despues {
            return Err(ErrorMigracion::Fallo {
                version: 11,
                migracion: MIG11,
                etapa: format!("verificar {}", tabla),
                causa: format!("entraron {} filas y salieron {}", antes, despues),
            });
        }
    }

    Ok(())
}

/// Inserta las restricciones de tabla antes del paréntesis que cierra la
/// lista de columnas.
///
/// Trabaja sobre el texto que SQLite guarda, y por eso busca el paréntesis
/// **por equilibrio** en vez de tomar el último carácter: una definición puede
/// terminar con cláusulas después del paréntesis.
fn con_restricciones(sql: &str, columnas: &[&str]) -> Option<String> {
    let bytes = sql.as_bytes();
    let apertura = sql.find('(')?;
    let mut profundidad = 0usize;
    let mut cierre = None;
    for (i, b) in bytes.iter().enumerate().skip(apertura) {
        match b {
            b'(' => profundidad += 1,
            b')' => {
                profundidad -= 1;
                if profundidad == 0 {
                    cierre = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let cierre = cierre?;
    let anadido: String = columnas
        .iter()
        .map(|c| format!(",
            {}", restriccion_de_centimo(c)))
        .collect();
    Some(format!("{}{}{}", &sql[..cierre], anadido, &sql[cierre..]))
}

/// Que cada columna de dinero viva bajo su restricción, dicho por el esquema.
///
/// Se comprueba leyendo `sqlite_master` y no confiando en que la
/// reconstrucción hiciera lo que decía: es la misma razón por la que la
/// migración 3 verifica en vez de darse por buena porque no falló.
fn verificar_restricciones_presentes(tx: &Transaction) -> Result<(), ErrorMigracion> {
    for (tabla, columna) in COLUMNAS_DE_DINERO {
        if !tabla_existe(tx, tabla)? || !columna_existe_en(tx, tabla, columna)? {
            continue;
        }
        let sql: String = tx
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?;",
                [tabla],
                |r| r.get(0),
            )
            .map_err(|e| ErrorMigracion::Almacenamiento(e.to_string()))?;
        if !sql.contains(&restriccion_de_centimo(columna)) {
            return Err(ErrorMigracion::Fallo {
                version: 11,
                migracion: MIG11,
                etapa: format!("verificar {}.{}", tabla, columna),
                causa: "la columna quedó sin su restricción de céntimo".into(),
            });
        }
    }
    Ok(())
}

const MIG12: &str = "la fecha de renovación de las anuales";

/// Las suscripciones anuales anotan **cuándo renuevan**, en vez de deducirlo.
///
/// La regla anterior intentaba deducir el vencimiento de una anual a partir
/// del año del último cobro y no podía, porque le faltaba el mes: una
/// cobrada en julio volvía a cobrarse el 5 de enero, seis meses antes.
///
/// ## De dónde sale la fecha de las que ya existen
///
/// Del último cobro más un año, tomando el **día de facturación** y no el día
/// en que se ejecutó el cargo. Son dos cosas distintas: el cargo se anota el
/// día en que se abrió la aplicación, que puede ser posterior. El proveedor
/// renueva el suyo.
///
/// Si el día no existe en ese mes, se usa el último del mes. Y si no hay
/// último cobro, la columna queda en `NULL`: **una anual sin fecha no se
/// cobra**, y el titular la anota. Inventar una fecha para poder cobrar sería
/// exactamente el error que esta migración corrige.
pub fn migracion_12_renovacion_anual(tx: &Transaction) -> Result<(), ErrorMigracion> {
    if !columna_existe_en(tx, "suscripciones", "fecha_renovacion")? {
        migraciones::anadir_columna(tx, MIG12, "suscripciones", "fecha_renovacion", "TEXT")?;
    }

    let pendientes: Vec<(i64, String, i64)> = {
        let mut s = tx
            .prepare(
                "SELECT id, fecha_ultimo_pago, dia_facturacion FROM suscripciones
                 WHERE frecuencia = 'anual'
                   AND fecha_renovacion IS NULL
                   AND fecha_ultimo_pago IS NOT NULL;",
            )
            .map_err(|e| ErrorMigracion::Almacenamiento(e.to_string()))?;
        let it = s
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|e| ErrorMigracion::Almacenamiento(e.to_string()))?;
        it.filter_map(|x| x.ok()).collect()
    };

    for (id, ultimo, dia_facturacion) in pendientes {
        let Some(fecha) = renovacion_derivada(&ultimo, dia_facturacion) else {
            // Una marca ilegible no permite deducir nada, y no es motivo para
            // detener la migración: la suscripción se queda sin fecha, que es
            // el estado que no cobra.
            continue;
        };

        tx.execute(
            "UPDATE suscripciones SET fecha_renovacion = ? WHERE id = ?;",
            (&fecha, id),
        )
        .map_err(|e| ErrorMigracion::Almacenamiento(e.to_string()))?;
    }

    Ok(())
}

/// `dd/mm/aaaa` del último cobro, más un año, con el día de facturación.
fn renovacion_derivada(ultimo_cobro: &str, dia_facturacion: i64) -> Option<String> {
    let partes: Vec<&str> = ultimo_cobro.split('/').collect();
    if partes.len() != 3 {
        return None;
    }
    let mes: u32 = partes[1].parse().ok()?;
    let anio: i32 = partes[2].parse().ok()?;
    let dia = u32::try_from(dia_facturacion).ok()?;

    let anio = anio + 1;
    let ultimo_del_mes = match mes {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if anio % 4 == 0 && (anio % 100 != 0 || anio % 400 == 0) => 29,
        2 => 28,
        _ => return None,
    };
    Some(format!("{:02}/{:02}/{}", dia.clamp(1, ultimo_del_mes), mes, anio))
}

pub fn crear_esquema(conn: &mut Connection) -> Result<()> {
    migraciones::ejecutar(conn)
        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
    sembrar(conn)
}
