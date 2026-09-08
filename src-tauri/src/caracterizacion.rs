//! Pruebas de caracterización del vertical de Gastos — Fase 1, paso 1.1.
//!
//! Capturan el comportamiento ACTUAL de `crear_gasto` y `eliminar_gasto`,
//! incluidas sus rarezas, para que cualquier cambio durante la extracción al
//! dominio se manifieste como una prueba en rojo. No juzgan si el
//! comportamiento es correcto: lo fijan.

use crate::db_sql;
use crate::{crear_gasto, eliminar_gasto, GastoInput};
use rusqlite::{params, Connection};
use std::sync::{Mutex, MutexGuard};

static ENTORNO: Mutex<()> = Mutex::new(());

fn raiz_temporal() -> std::path::PathBuf {
    std::env::temp_dir().join("michelitos-caracterizacion")
}

/// Aísla el proceso de la base de datos real y entrega una base vacía recién
/// inicializada.
///
/// Las pruebas se serializan mediante un mutex porque el código bajo prueba
/// resuelve la ruta de la base desde `HOME`, que es estado global del proceso.
/// Esa dependencia global es precisamente lo que la Fase 1 elimina al
/// introducir repositorios inyectables.
fn entorno_aislado() -> MutexGuard<'static, ()> {
    let guarda = ENTORNO.lock().unwrap_or_else(|e| e.into_inner());

    let raiz = raiz_temporal();
    std::fs::create_dir_all(&raiz).expect("crear raíz temporal");
    std::env::set_var("HOME", &raiz);

    // Red de seguridad: si la ruta resuelta no cae dentro del directorio
    // temporal, abortar antes de escribir una sola fila.
    let ruta = db_sql::obtener_ruta_db();
    assert!(
        ruta.starts_with(raiz.to_str().unwrap()),
        "ABORTADO: las pruebas apuntarían a la base real ({})",
        ruta
    );

    let _ = std::fs::remove_file(&ruta);
    db_sql::inicializar_db().expect("inicializar base de prueba");

    guarda
}

// --- Utilidades ---

fn conexion() -> Connection {
    db_sql::obtener_conexion().expect("abrir conexión de prueba")
}

/// Comparación de importes con tolerancia, para no depender de la
/// representación binaria exacta de los f64.
fn assert_importe(obtenido: f64, esperado: f64, contexto: &str) {
    assert!(
        (obtenido - esperado).abs() < 1e-9,
        "{}: se esperaba {:.4} y se obtuvo {:.4}",
        contexto,
        esperado,
        obtenido
    );
}

fn id_categoria(nombre: &str) -> i64 {
    conexion()
        .query_row("SELECT id FROM categorias WHERE nombre = ?;", [nombre], |r| r.get(0))
        .unwrap_or_else(|_| panic!("no existe la categoría '{}'", nombre))
}

fn crear_cuenta(nombre: &str, divisa: &str, balance: f64) -> i64 {
    let c = conexion();
    c.execute(
        "INSERT INTO cuentas_ahorro (nombre, divisa, balance_actual) VALUES (?, ?, ?);",
        params![nombre, divisa, balance],
    )
    .expect("insertar cuenta de prueba");
    c.last_insert_rowid()
}

fn fijar_balance_cuenta(nombre: &str, balance: f64) {
    conexion()
        .execute(
            "UPDATE cuentas_ahorro SET balance_actual = ? WHERE nombre = ?;",
            params![balance, nombre],
        )
        .expect("fijar balance de cuenta");
}

fn balance_cuenta(nombre: &str) -> f64 {
    conexion()
        .query_row(
            "SELECT balance_actual FROM cuentas_ahorro WHERE nombre = ?;",
            [nombre],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| panic!("no existe la cuenta '{}'", nombre))
}

fn crear_tarjeta(balance_pesos: f64, balance_dolares: f64) -> i64 {
    let c = conexion();
    c.execute(
        "INSERT INTO tarjetas (entidad, nombre_tarjeta, fecha_corte, fecha_limite_pago,
                               balance_pesos, balance_dolares, limite_pesos, limite_dolares)
         VALUES ('Banco Ejemplo', 'Tarjeta Ejemplo', 15, 5, ?, ?, 100000.0, 5000.0);",
        params![balance_pesos, balance_dolares],
    )
    .expect("insertar tarjeta de prueba");
    c.last_insert_rowid()
}

fn fijar_balance_tarjeta(id: i64, pesos: f64, dolares: f64) {
    conexion()
        .execute(
            "UPDATE tarjetas SET balance_pesos = ?, balance_dolares = ? WHERE id = ?;",
            params![pesos, dolares, id],
        )
        .expect("fijar balance de tarjeta");
}

/// Devuelve (balance_pesos, balance_dolares).
fn balances_tarjeta(id: i64) -> (f64, f64) {
    conexion()
        .query_row(
            "SELECT balance_pesos, balance_dolares FROM tarjetas WHERE id = ?;",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("leer balances de tarjeta")
}

fn costo_adicional(gasto_id: i64) -> f64 {
    conexion()
        .query_row("SELECT costo_adicional FROM gastos WHERE id = ?;", [gasto_id], |r| r.get(0))
        .expect("leer costo_adicional")
}

fn total_gastos() -> i64 {
    conexion()
        .query_row("SELECT COUNT(*) FROM gastos;", [], |r| r.get(0))
        .expect("contar gastos")
}

/// Constructor con los valores por defecto de una transferencia ordinaria.
fn transferencia(monto: f64, categoria: &str, descripcion: &str, cuenta_id: i64) -> GastoInput {
    GastoInput {
        fecha: "08/09/2026".to_string(),
        monto,
        divisa: "DOP".to_string(),
        descripcion: descripcion.to_string(),
        categoria_id: id_categoria(categoria),
        metodo_pago: "transferencia".to_string(),
        es_lbtr: false,
        tarjeta_id: None,
        cuenta_ahorro_id: Some(cuenta_id),
    }
}

// =====================================================================
//  R1, R2, R3 — retención impositiva, exención del TSS y comisión LBTR
// =====================================================================

#[test]
fn c1_transferencia_ordinaria_retiene_el_020_por_ciento() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100000.0);

    let id = crear_gasto(transferencia(10000.0, "Alimentación", "Compra semanal", cuenta)).unwrap();

    assert_importe(costo_adicional(id), 20.0, "retención del 0.20 %");
}

#[test]
fn c2_el_pago_de_tss_esta_exento_de_la_retencion() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100000.0);

    let id = crear_gasto(transferencia(10000.0, "Impuestos", "Pago TSS", cuenta)).unwrap();

    assert_importe(costo_adicional(id), 0.0, "exención del TSS");
}

#[test]
fn c3_la_exencion_exige_tambien_la_categoria_impuestos() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100000.0);

    let id = crear_gasto(transferencia(10000.0, "Alimentación", "Pago TSS", cuenta)).unwrap();

    assert_importe(costo_adicional(id), 20.0, "TSS sin categoría Impuestos no exime");
}

#[test]
fn c4_la_exencion_exige_tambien_la_mencion_de_tss() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100000.0);

    let id = crear_gasto(transferencia(10000.0, "Impuestos", "Pago ITBIS", cuenta)).unwrap();

    assert_importe(costo_adicional(id), 20.0, "impuesto distinto del TSS sí retiene");
}

#[test]
fn c5_el_lbtr_suma_su_comision_de_servicio_a_la_retencion() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100000.0);

    let mut entrada = transferencia(10000.0, "Alimentación", "Pago proveedor", cuenta);
    entrada.es_lbtr = true;
    let id = crear_gasto(entrada).unwrap();

    assert_importe(costo_adicional(id), 120.0, "retención 20 + comisión LBTR 100");
}

#[test]
fn c6_la_exencion_del_tss_no_alcanza_a_la_comision_del_lbtr() {
    // H1 — confirmado correcto por el usuario: la exención es tributaria y no
    // libera del precio de un servicio bancario.
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100000.0);

    let mut entrada = transferencia(10000.0, "Impuestos", "Pago TSS agosto", cuenta);
    entrada.es_lbtr = true;
    let id = crear_gasto(entrada).unwrap();

    assert_importe(costo_adicional(id), 100.0, "solo la comisión de servicio");
    assert_importe(balance_cuenta("Cuenta Ahorros DOP"), 89900.0, "saldo tras el pago");
}

#[test]
fn c7_la_retencion_se_redondea_a_centavos() {
    // CAMBIO DE CONDUCTA CONSENTIDO — 2026-09-08, decisión del usuario.
    //
    // Hasta la Fase 1.2b el cálculo era (monto * 0.002).round(), que redondeaba
    // a unidades enteras, de modo que 1250.00 producía una retención de 3.00.
    // El banco cobra la comisión al centavo, así que el valor correcto es 2.50.
    //
    // Es la única prueba de caracterización cuyo valor esperado cambia. Se
    // documenta aquí en lugar de ajustarse en silencio, conforme al paso 5 del
    // protocolo de pruebas. El cruce contra los importes históricos reales se
    // hará en la fase de migración del esquema.
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100000.0);

    let id = crear_gasto(transferencia(1250.0, "Alimentación", "Compra", cuenta)).unwrap();

    assert_importe(costo_adicional(id), 2.50, "1250.00 x 0.20 % al centavo");
}

// =====================================================================
//  R4, R5, R6 — efecto de cada método de pago sobre los saldos
// =====================================================================

#[test]
fn c8_el_gasto_con_tarjeta_en_dolares_solo_mueve_el_balance_en_dolares() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(500.0, 100.0);

    let entrada = GastoInput {
        fecha: "08/09/2026".to_string(),
        monto: 75.0,
        divisa: "USD".to_string(),
        descripcion: "Suscripción anual".to_string(),
        categoria_id: id_categoria("Suscripciones"),
        metodo_pago: "tarjeta".to_string(),
        es_lbtr: false,
        tarjeta_id: Some(tarjeta),
        cuenta_ahorro_id: None,
    };
    crear_gasto(entrada).unwrap();

    let (pesos, dolares) = balances_tarjeta(tarjeta);
    assert_importe(dolares, 175.0, "la deuda en USD sube");
    assert_importe(pesos, 500.0, "la deuda en DOP no se toca");
}

#[test]
fn c9_el_gasto_en_efectivo_descuenta_de_la_caja_de_su_divisa() {
    let _g = entorno_aislado();
    fijar_balance_cuenta("Efectivo DOP", 5000.0);
    fijar_balance_cuenta("Efectivo USD", 300.0);

    let entrada = GastoInput {
        fecha: "08/09/2026".to_string(),
        monto: 1200.0,
        divisa: "DOP".to_string(),
        descripcion: "Almuerzo".to_string(),
        categoria_id: id_categoria("Alimentación"),
        metodo_pago: "efectivo".to_string(),
        es_lbtr: false,
        tarjeta_id: None,
        cuenta_ahorro_id: None,
    };
    crear_gasto(entrada).unwrap();

    assert_importe(balance_cuenta("Efectivo DOP"), 3800.0, "caja DOP");
    assert_importe(balance_cuenta("Efectivo USD"), 300.0, "caja USD intacta");
    assert_importe(costo_adicional(1), 0.0, "el efectivo no retiene ni comisiona");
}

// =====================================================================
//  Hallazgos H3, H4, H5 — comportamientos que se fijan, no se corrigen
// =====================================================================

#[test]
fn c10_h3_si_la_caja_fue_renombrada_el_gasto_se_registra_sin_mover_saldo() {
    let _g = entorno_aislado();
    conexion()
        .execute(
            "UPDATE cuentas_ahorro SET nombre = 'Caja Chica DOP' WHERE nombre = 'Efectivo DOP';",
            [],
        )
        .unwrap();
    let antes = balance_cuenta("Caja Chica DOP");

    let entrada = GastoInput {
        fecha: "08/09/2026".to_string(),
        monto: 1200.0,
        divisa: "DOP".to_string(),
        descripcion: "Almuerzo".to_string(),
        categoria_id: id_categoria("Alimentación"),
        metodo_pago: "efectivo".to_string(),
        es_lbtr: false,
        tarjeta_id: None,
        cuenta_ahorro_id: None,
    };
    let resultado = crear_gasto(entrada);

    assert!(resultado.is_ok(), "hoy no falla: el UPDATE afecta a 0 filas");
    assert_eq!(total_gastos(), 1, "el gasto queda registrado");
    assert_importe(balance_cuenta("Caja Chica DOP"), antes, "ningún saldo se movió");
}

#[test]
fn c11_h4_un_gasto_con_tarjeta_sin_identificador_no_mueve_ninguna_deuda() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(500.0, 100.0);

    let entrada = GastoInput {
        fecha: "08/09/2026".to_string(),
        monto: 900.0,
        divisa: "DOP".to_string(),
        descripcion: "Compra sin tarjeta indicada".to_string(),
        categoria_id: id_categoria("Otros"),
        metodo_pago: "tarjeta".to_string(),
        es_lbtr: false,
        tarjeta_id: None,
        cuenta_ahorro_id: None,
    };
    let resultado = crear_gasto(entrada);

    assert!(resultado.is_ok(), "hoy se acepta sin tarjeta");
    assert_eq!(total_gastos(), 1, "el gasto queda registrado");
    let (pesos, dolares) = balances_tarjeta(tarjeta);
    assert_importe(pesos, 500.0, "deuda en DOP sin cambios");
    assert_importe(dolares, 100.0, "deuda en USD sin cambios");
}

#[test]
fn c12_h5_la_reversion_de_tarjeta_recorta_en_cero_y_pierde_la_diferencia() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(100.0, 0.0);

    let entrada = GastoInput {
        fecha: "08/09/2026".to_string(),
        monto: 150.0,
        divisa: "DOP".to_string(),
        descripcion: "Compra".to_string(),
        categoria_id: id_categoria("Otros"),
        metodo_pago: "tarjeta".to_string(),
        es_lbtr: false,
        tarjeta_id: Some(tarjeta),
        cuenta_ahorro_id: None,
    };
    let gasto = crear_gasto(entrada).unwrap();
    assert_importe(balances_tarjeta(tarjeta).0, 250.0, "deuda tras el gasto");

    // El usuario abona 200 antes de darse cuenta del error de registro.
    fijar_balance_tarjeta(tarjeta, 50.0, 0.0);

    eliminar_gasto(gasto).unwrap();

    // Aritméticamente correspondería 50 - 150 = -100, pero MAX(0.0, ...) lo
    // recorta y esos 100 desaparecen sin dejar registro.
    assert_importe(balances_tarjeta(tarjeta).0, 0.0, "recorte en cero (H5)");
}

#[test]
fn c13_la_reversion_de_una_transferencia_restituye_el_saldo_exacto() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100000.0);

    let gasto = crear_gasto(transferencia(10000.0, "Alimentación", "Compra", cuenta)).unwrap();
    assert_importe(balance_cuenta("Cuenta Ahorros DOP"), 89980.0, "saldo tras el gasto");

    eliminar_gasto(gasto).unwrap();

    assert_importe(balance_cuenta("Cuenta Ahorros DOP"), 100000.0, "saldo restituido");
    assert_eq!(total_gastos(), 0, "el gasto se borra, no se anula");
}

// =====================================================================
//  R7 — atomicidad
// =====================================================================

#[test]
fn c14_un_fallo_a_mitad_de_la_operacion_no_deja_el_saldo_movido() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100000.0);

    // Categoría inexistente: el débito de la cuenta ocurre antes del INSERT,
    // que falla por integridad referencial y debe deshacer la transacción.
    let mut entrada = transferencia(10000.0, "Alimentación", "Compra", cuenta);
    entrada.categoria_id = 999_999;

    let resultado = crear_gasto(entrada);

    assert!(resultado.is_err(), "la operación debe fallar");
    assert_importe(balance_cuenta("Cuenta Ahorros DOP"), 100000.0, "saldo intacto");
    assert_eq!(total_gastos(), 0, "ningún gasto insertado");
}

// =====================================================================
//  Casos límite del cálculo de cargos, fijados antes de extraer el dominio
// =====================================================================

#[test]
fn c15_en_una_transferencia_en_dolares_la_comision_lbtr_se_suma_sin_convertir() {
    // La comisión del LBTR está denominada en pesos, pero el código la suma al
    // costo_adicional del gasto sea cual sea su divisa. Se fija tal cual.
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros USD", "USD", 5000.0);

    let mut entrada = transferencia(1000.0, "Alimentación", "Compra en dólares", cuenta);
    entrada.divisa = "USD".to_string();
    entrada.es_lbtr = true;
    let id = crear_gasto(entrada).unwrap();

    // (1000 * 0.002).round() = 2, más 100 añadidos sin conversión.
    assert_importe(costo_adicional(id), 102.0, "retención en USD + 100 sin convertir");
}

#[test]
fn c16_una_divisa_distinta_de_usd_se_trata_como_pesos() {
    // La columna gastos.divisa no tiene CHECK, a diferencia del resto de
    // tablas, y el código compara únicamente contra "USD".
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);

    let entrada = GastoInput {
        fecha: "08/09/2026".to_string(),
        monto: 300.0,
        divisa: "EUR".to_string(),
        descripcion: "Compra en euros".to_string(),
        categoria_id: id_categoria("Otros"),
        metodo_pago: "tarjeta".to_string(),
        es_lbtr: false,
        tarjeta_id: Some(tarjeta),
        cuenta_ahorro_id: None,
    };
    let resultado = crear_gasto(entrada);

    assert!(resultado.is_ok(), "hoy se acepta cualquier divisa");
    let (pesos, dolares) = balances_tarjeta(tarjeta);
    assert_importe(pesos, 300.0, "se carga al balance en pesos");
    assert_importe(dolares, 0.0, "el balance en dólares no se toca");
}
