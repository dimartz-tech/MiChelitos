//! Pruebas de caracterización del vertical de Gastos — Fase 1, paso 1.1.
//!
//! Capturan el comportamiento ACTUAL de `crear_gasto` y `eliminar_gasto`,
//! incluidas sus rarezas, para que cualquier cambio durante la extracción al
//! dominio se manifieste como una prueba en rojo. No juzgan si el
//! comportamiento es correcto: lo fijan.

use crate::db_sql;
use crate::{crear_gasto, crear_suscripcion, eliminar_cuenta, eliminar_gasto, procesar_suscripciones, registrar_pago_tarjeta, revertir_abono_tarjeta, GastoInput, crear_ingreso, marcar_ingreso_pagado, eliminar_ingreso, actualizar_ingreso, crear_ingreso_informal, marcar_informal_pagado, eliminar_ingreso_informal, crear_cobro_efectivo_informal, IngresoInput};
use rusqlite::{params, Connection};
use std::sync::{Mutex, MutexGuard};

static ENTORNO: Mutex<()> = Mutex::new(());

/// Toma la guarda del entorno compartido.
///
/// Existe porque **más de un módulo de prueba muta `HOME`**, y un mutex por
/// módulo solo serializaría cada uno consigo mismo. Mientras la ruta de la
/// base se resuelva desde una variable de entorno, esta guarda es lo único
/// que impide que dos pruebas se pisen el directorio.
pub(crate) fn bloquear_entorno() -> MutexGuard<'static, ()> {
    ENTORNO.lock().unwrap_or_else(|e| e.into_inner())
}

fn raiz_temporal() -> std::path::PathBuf {
    std::env::temp_dir().join("michelitos-caracterizacion")
}

/// Aísla el proceso de la base de datos real y entrega una base vacía recién
/// inicializada, sin respaldos heredados de otra prueba.
///
/// Las pruebas se serializan mediante un mutex porque el código bajo prueba
/// resuelve la ruta de la base desde `HOME`, que es estado global del proceso.
/// Esa dependencia global es precisamente lo que la Fase 1 elimina al
/// introducir repositorios inyectables.
fn entorno_aislado() -> MutexGuard<'static, ()> {
    let guarda = bloquear_entorno();

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
    let _ = std::fs::remove_dir_all(crate::respaldo::directorio_de_respaldos());
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

/// Monto, divisa y costo_adicional del último gasto registrado, que es como se
/// asienta la comisión de un abono a tarjeta.
fn ultimo_gasto() -> (f64, String, f64) {
    conexion()
        .query_row(
            "SELECT monto, divisa, costo_adicional FROM gastos ORDER BY id DESC LIMIT 1;",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .expect("leer último gasto")
}

/// Constructor con los valores por defecto de una transferencia ordinaria.
fn ultimo_abono() -> i64 {
    conexion()
        .query_row("SELECT MAX(id) FROM pagos_tarjeta;", [], |r| r.get(0))
        .expect("leer último abono")
}

fn declarar_comision_de_impuestos(cuenta_id: i64, tarifa: f64) {
    conexion()
        .execute(
            "UPDATE cuentas_ahorro SET comision_pago_impuestos = ? WHERE id = ?;",
            params![tarifa, cuenta_id],
        )
        .expect("declarar comisión de impuestos");
}

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
        tasa_cambio: None,
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
        tasa_cambio: None,
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
        tasa_cambio: None,
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
fn c10_renombrar_la_caja_ya_no_impide_que_el_gasto_se_asiente() {
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
        tasa_cambio: None,
    };
    let resultado = crear_gasto(entrada);

    // Antes (H3) el UPDATE buscaba 'Efectivo DOP' por nombre, afectaba a 0
    // filas y devolvía Ok: el gasto quedaba registrado y el saldo intacto.
    // Ahora la caja se localiza por su papel, que el renombrado no altera.
    assert!(resultado.is_ok());
    assert_eq!(total_gastos(), 1, "el gasto queda registrado");
    assert_importe(
        balance_cuenta("Caja Chica DOP"),
        antes - 1200.0,
        "el saldo sí se movió pese al renombrado",
    );
}

#[test]
fn c10b_la_caja_de_efectivo_no_se_puede_eliminar() {
    // La vía por la que H3 era alcanzable desde la interfaz: la guarda de
    // eliminar_cuenta contaba gastos con cuenta_ahorro_id, y los gastos en
    // efectivo lo tenían nulo porque se vinculaban por nombre. La caja se
    // borraba sin que nada lo impidiera.
    let _g = entorno_aislado();
    let caja: i64 = conexion()
        .query_row(
            "SELECT id FROM cuentas_ahorro WHERE es_caja_efectivo = 1 AND divisa = 'DOP';",
            [],
            |r| r.get(0),
        )
        .unwrap();

    let error = eliminar_cuenta(caja).unwrap_err();

    assert!(error.contains("caja de efectivo"), "explica por qué: {error}");
    let sigue: i64 = conexion()
        .query_row("SELECT COUNT(*) FROM cuentas_ahorro WHERE id = ?;", [caja], |r| r.get(0))
        .unwrap();
    assert_eq!(sigue, 1, "la caja sigue ahí");
}

#[test]
fn c10c_los_gastos_en_efectivo_quedan_vinculados_a_la_caja_por_identificador() {
    // Resolución de H3 en los datos: el gasto guarda la referencia real, no
    // solo el texto del método de pago.
    let _g = entorno_aislado();
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
        tasa_cambio: None,
    };
    let gasto = crear_gasto(entrada).unwrap();

    let (cuenta, es_caja): (Option<i64>, i64) = conexion()
        .query_row(
            "SELECT g.cuenta_ahorro_id, COALESCE(c.es_caja_efectivo, 0)
             FROM gastos g LEFT JOIN cuentas_ahorro c ON c.id = g.cuenta_ahorro_id
             WHERE g.id = ?;",
            [gasto],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();

    assert!(cuenta.is_some(), "el gasto en efectivo referencia una cuenta");
    assert_eq!(es_caja, 1, "y esa cuenta es la caja de efectivo");
}

#[test]
fn c11_un_gasto_con_tarjeta_sin_identificador_se_rechaza() {
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
        tasa_cambio: None,
    };
    let resultado = crear_gasto(entrada);

    // Antes (H4) esto devolvía Ok: el gasto se insertaba y ninguna deuda
    // subía, de modo que aparecía en la lista sin corresponderse con ningún
    // saldo. Ahora se rechaza antes de tocar nada.
    let error = resultado.unwrap_err();
    assert!(error.contains("tarjeta"), "el mensaje dice qué falta: {error}");
    assert_eq!(total_gastos(), 0, "no se guardó nada");
    let (pesos, dolares) = balances_tarjeta(tarjeta);
    assert_importe(pesos, 500.0, "deuda en DOP sin cambios");
    assert_importe(dolares, 100.0, "deuda en USD sin cambios");
}

#[test]
fn c12_la_reversion_de_tarjeta_deja_saldo_a_favor_en_vez_de_recortar() {
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
        tasa_cambio: None,
    };
    let gasto = crear_gasto(entrada).unwrap();
    assert_importe(balances_tarjeta(tarjeta).0, 250.0, "deuda tras el gasto");

    // El usuario abona 200 antes de darse cuenta del error de registro.
    fijar_balance_tarjeta(tarjeta, 50.0, 0.0);

    eliminar_gasto(gasto).unwrap();

    // 50 - 150 = -100. Antes el MAX(0.0, ...) lo recortaba a cero y esos 100
    // desaparecían sin registro; hoy quedan como saldo a favor, que es lo que
    // el emisor acredita cuando se ha pagado de más.
    assert_importe(balances_tarjeta(tarjeta).0, -100.0, "saldo a favor, sin recorte");
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
        tasa_cambio: None,
    };
    let resultado = crear_gasto(entrada);

    assert!(resultado.is_ok(), "hoy se acepta cualquier divisa");
    let (pesos, dolares) = balances_tarjeta(tarjeta);
    assert_importe(pesos, 300.0, "se carga al balance en pesos");
    assert_importe(dolares, 0.0, "el balance en dólares no se toca");
}

// =====================================================================
//  Abonos a tarjeta — la otra ruta que calcula el 0.20 % (H8)
// =====================================================================

#[test]
fn c17_el_abono_en_igual_divisa_cobra_la_comision_sobre_el_monto() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(50000.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100000.0);

    // 12 345.67 x 0.20 % = 24.69134, un importe con cinco decimales.
    registrar_pago_tarjeta(
        tarjeta,
        "08/09/2026".to_string(),
        12345.67,
        "DOP".to_string(),
        Some(cuenta),
        0.0,
    )
    .unwrap();

    let (monto_comision, divisa, costo) = ultimo_gasto();
    assert_importe(monto_comision, 24.69, "comisión al centavo");
    assert_eq!(divisa, "DOP");
    assert_importe(costo, 0.0, "la comisión se asienta como monto, no como costo");

    // Saldo: 100 000 − 12 345.67 − 24.69
    assert_importe(balance_cuenta("Cuenta Ahorros DOP"), 87629.64, "saldo debitado");
    assert_importe(balances_tarjeta(tarjeta).0, 37654.33, "deuda reducida");
}

#[test]
fn c18_el_abono_multidivisa_convierte_y_comisiona_al_centavo() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 1000.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100000.0);

    // 250 USD x 60.25 = 15 062.50 DOP; su 0.20 % = 30.125, que se redondea
    // a 30.13. La conversión ocurre antes de comisionar.
    registrar_pago_tarjeta(
        tarjeta,
        "08/09/2026".to_string(),
        250.0,
        "USD".to_string(),
        Some(cuenta),
        60.25,
    )
    .unwrap();

    let (monto_comision, divisa, _) = ultimo_gasto();
    assert_importe(monto_comision, 30.13, "comisión al centavo");
    assert_eq!(divisa, "DOP", "la comisión se registra en pesos");

    // Saldo: 100 000 − 15 062.50 − 30.13
    assert_importe(balance_cuenta("Cuenta Ahorros DOP"), 84907.37, "saldo debitado");
    assert_importe(balances_tarjeta(tarjeta).1, 750.0, "la deuda baja en USD");
}

#[test]
fn c19_un_abono_sin_cuenta_de_origen_no_genera_comision() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(50000.0, 0.0);

    registrar_pago_tarjeta(
        tarjeta,
        "08/09/2026".to_string(),
        10000.0,
        "DOP".to_string(),
        None,
        0.0,
    )
    .unwrap();

    assert_eq!(total_gastos(), 0, "sin cuenta no hay comisión que asentar");
    assert_importe(balances_tarjeta(tarjeta).0, 40000.0, "la deuda sí se reduce");
}

#[test]
fn c20_el_abono_superior_a_la_deuda_deja_saldo_a_favor() {
    // La otra mitad de H5, y la que perdía dinero sin borrar nada: bastaba
    // abonar más que el balance —pagar el corte mientras entran consumos
    // nuevos, o abonar de más a propósito— para que el exceso se descartara.
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(500.0, 0.0);

    registrar_pago_tarjeta(
        tarjeta,
        "08/09/2026".to_string(),
        800.0,
        "DOP".to_string(),
        None,
        0.0,
    )
    .unwrap();

    assert_importe(balances_tarjeta(tarjeta).0, -300.0, "800 abonados sobre 500 de deuda");
}

#[test]
fn c21_una_transferencia_sin_cuenta_calcula_la_retencion_pero_no_debita() {
    // Contrapartida de C11: el cargo se calcula y se guarda, pero no hay
    // cuenta de la que descontarlo. AfectacionSaldo::Ninguna le pone nombre.
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100000.0);

    let mut entrada = transferencia(10000.0, "Alimentación", "Compra", cuenta);
    entrada.cuenta_ahorro_id = None;
    let id = crear_gasto(entrada).unwrap();

    assert_importe(costo_adicional(id), 20.0, "la retención sí se calcula");
    assert_importe(balance_cuenta("Cuenta Ahorros DOP"), 100000.0, "ningún saldo se movió");
}

// =====================================================================
//  Suscripciones — la regla de idempotencia del cobro automático
// =====================================================================
//
// procesar_suscripciones lee la fecha del sistema directamente, que es lo que
// el puerto Reloj existe para corregir. Mientras no se refactorice, estas
// pruebas derivan sus valores del día de hoy igual que hace el código.

fn dia_de_hoy() -> i32 {
    use chrono::Datelike;
    chrono::Local::now().day() as i32
}

fn hoy_formateado() -> String {
    chrono::Local::now().format("%d/%m/%Y").to_string()
}

fn fijar_ultimo_pago(sub_id: i64, fecha: &str) {
    conexion()
        .execute("UPDATE suscripciones SET fecha_ultimo_pago = ? WHERE id = ?;", params![fecha, sub_id])
        .expect("fijar fecha de último pago");
}

fn ultimo_pago(sub_id: i64) -> Option<String> {
    conexion()
        .query_row("SELECT fecha_ultimo_pago FROM suscripciones WHERE id = ?;", [sub_id], |r| r.get(0))
        .expect("leer fecha de último pago")
}

#[test]
fn s1_una_suscripcion_nunca_cobrada_se_cobra_al_llegar_su_dia() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    // Día 1: siempre alcanzado, sea cual sea la fecha de hoy.
    let sub = crear_suscripcion("Plataforma".into(), 500.0, tarjeta, "mensual".into(), 1, "DOP".into()).unwrap();

    let mensajes = procesar_suscripciones().unwrap();

    assert_eq!(mensajes.len(), 1, "debe generarse un cargo");
    assert_importe(balances_tarjeta(tarjeta).0, 500.0, "la deuda sube");
    assert_eq!(total_gastos(), 1, "se registra el gasto");
    assert_eq!(ultimo_pago(sub), Some(hoy_formateado()), "queda marcada como cobrada");
}

#[test]
fn s2_cobrada_este_mismo_mes_no_vuelve_a_cobrarse() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let sub = crear_suscripcion("Plataforma".into(), 500.0, tarjeta, "mensual".into(), 1, "DOP".into()).unwrap();
    fijar_ultimo_pago(sub, &hoy_formateado());

    let mensajes = procesar_suscripciones().unwrap();

    assert!(mensajes.is_empty(), "no debe cobrar dos veces en el mismo mes");
    assert_importe(balances_tarjeta(tarjeta).0, 0.0, "la deuda no se mueve");
    assert_eq!(total_gastos(), 0);
}

#[test]
fn s3_procesar_dos_veces_seguidas_no_duplica_el_cargo() {
    // Es la garantía que sostiene que la app procese suscripciones en cada
    // arranque sin cobrar de más.
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    crear_suscripcion("Plataforma".into(), 500.0, tarjeta, "mensual".into(), 1, "DOP".into()).unwrap();

    procesar_suscripciones().unwrap();
    let segunda = procesar_suscripciones().unwrap();

    assert!(segunda.is_empty(), "el segundo procesamiento no cobra");
    assert_importe(balances_tarjeta(tarjeta).0, 500.0, "un solo cargo");
    assert_eq!(total_gastos(), 1);
}

#[test]
fn s4_una_anual_cobrada_este_ano_no_vuelve_a_cobrarse() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let sub = crear_suscripcion("Anual".into(), 3600.0, tarjeta, "anual".into(), 1, "DOP".into()).unwrap();
    fijar_ultimo_pago(sub, &hoy_formateado());

    assert!(procesar_suscripciones().unwrap().is_empty());
    assert_importe(balances_tarjeta(tarjeta).0, 0.0, "sin cargo");
}

#[test]
fn s5_el_cargo_en_dolares_solo_mueve_el_balance_en_dolares() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(1000.0, 50.0);
    crear_suscripcion("Plataforma".into(), 15.0, tarjeta, "mensual".into(), 1, "USD".into()).unwrap();

    procesar_suscripciones().unwrap();

    let (pesos, dolares) = balances_tarjeta(tarjeta);
    assert_importe(dolares, 65.0, "sube la deuda en dólares");
    assert_importe(pesos, 1000.0, "la deuda en pesos no se toca");
}

#[test]
fn s6_borrar_y_recrear_reinicia_la_idempotencia_y_vuelve_a_cobrar() {
    // ESTE es el motivo por el que hace falta poder editar: hoy la única
    // manera de cambiar una suscripción es borrarla y crearla de nuevo, y eso
    // pone fecha_ultimo_pago en NULL, con lo que el siguiente procesamiento
    // cobra otra vez el mismo mes.
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let sub = crear_suscripcion("Plataforma".into(), 500.0, tarjeta, "mensual".into(), 1, "DOP".into()).unwrap();

    procesar_suscripciones().unwrap();
    assert_importe(balances_tarjeta(tarjeta).0, 500.0, "primer cargo");

    // El usuario quiere cambiar el monto: borra y vuelve a crear.
    crate::eliminar_suscripcion(sub).unwrap();
    crear_suscripcion("Plataforma".into(), 600.0, tarjeta, "mensual".into(), 1, "DOP".into()).unwrap();

    procesar_suscripciones().unwrap();

    assert_importe(balances_tarjeta(tarjeta).0, 1100.0, "cobro duplicado en el mismo mes");
    assert_eq!(total_gastos(), 2, "dos cargos donde debería haber uno");
}

#[test]
fn s7_editar_una_suscripcion_conserva_la_idempotencia_y_no_recobra() {
    // Contrapartida de S6: el comando de edición existe precisamente para
    // evitar el cobro duplicado que provoca borrar y recrear.
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let sub = crear_suscripcion("Plataforma".into(), 500.0, tarjeta, "mensual".into(), 1, "DOP".into()).unwrap();

    procesar_suscripciones().unwrap();
    assert_importe(balances_tarjeta(tarjeta).0, 500.0, "primer cargo");
    let marca = ultimo_pago(sub);

    crate::actualizar_suscripcion(sub, "Plataforma".into(), 600.0, tarjeta, "mensual".into(), 1, "DOP".into()).unwrap();

    assert_eq!(ultimo_pago(sub), marca, "la marca de idempotencia se conserva");
    procesar_suscripciones().unwrap();
    assert_importe(balances_tarjeta(tarjeta).0, 500.0, "no vuelve a cobrar este mes");
    assert_eq!(total_gastos(), 1, "un solo cargo, frente a los dos de S6");
}

#[test]
fn s8_editar_no_altera_los_cargos_ya_realizados() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let sub = crear_suscripcion("Plataforma".into(), 500.0, tarjeta, "mensual".into(), 1, "DOP".into()).unwrap();
    procesar_suscripciones().unwrap();

    crate::actualizar_suscripcion(sub, "Otro nombre".into(), 999.0, tarjeta, "anual".into(), 20, "USD".into()).unwrap();

    let (monto, _, _) = ultimo_gasto();
    assert_importe(monto, 500.0, "el gasto ya registrado mantiene su importe");
    assert_importe(balances_tarjeta(tarjeta).0, 500.0, "y la deuda tampoco cambia");
}

#[test]
fn s9_editar_una_suscripcion_inexistente_es_error() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    assert!(crate::actualizar_suscripcion(9999, "X".into(), 1.0, tarjeta, "mensual".into(), 1, "DOP".into()).is_err());
}

#[test]
fn c22_una_divisa_no_admitida_se_normaliza_al_persistir() {
    // CAMBIO DE CONDUCTA — Fase 1.7, 2026-09-09.
    //
    // Antes el comando insertaba en gastos.divisa el texto recibido tal cual,
    // así que un valor como "EUR" quedaba almacenado. Al pasar por el tipo
    // Dinero, que solo conoce DOP y USD, se normaliza a "DOP" — que es además
    // como ya se interpretaba a efectos de saldo, según fija C16.
    //
    // Sin impacto sobre los datos existentes: la base solo contiene DOP y USD.
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);

    let entrada = GastoInput {
        fecha: "09/09/2026".to_string(),
        monto: 300.0,
        divisa: "EUR".to_string(),
        descripcion: "Compra en euros".to_string(),
        categoria_id: id_categoria("Otros"),
        metodo_pago: "tarjeta".to_string(),
        es_lbtr: false,
        tarjeta_id: Some(tarjeta),
        cuenta_ahorro_id: None,
        tasa_cambio: None,
    };
    crear_gasto(entrada).unwrap();

    let (_, divisa, _) = ultimo_gasto();
    assert_eq!(divisa, "DOP", "se normaliza en lugar de almacenar 'EUR'");
    assert_importe(balances_tarjeta(tarjeta).0, 300.0, "el saldo ya se trataba como pesos");
}

// =====================================================================
//  Conversión con tasa declarada — de extremo a extremo por el comando
// =====================================================================

#[test]
fn c23_un_gasto_en_dolares_desde_cuenta_en_pesos_se_convierte_y_persiste_la_tasa() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100000.0);

    let mut entrada = transferencia(100.0, "Alimentación", "Compra en el exterior", cuenta);
    entrada.divisa = "USD".to_string();
    entrada.tasa_cambio = Some(60.0);
    let id = crear_gasto(entrada).unwrap();

    // 100.00 USD x 60 = 6 000.00; su 0.20 % = 12.00.
    assert_importe(balance_cuenta("Cuenta Ahorros DOP"), 93988.0, "saldo tras el débito");

    let (tasa, liquidado, divisa_liq): (Option<f64>, Option<f64>, Option<String>) = conexion()
        .query_row(
            "SELECT tasa_conversion, monto_liquidado, divisa_liquidada FROM gastos WHERE id = ?;",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();

    assert_importe(tasa.unwrap(), 60.0, "la tasa queda en columna, no en el texto");
    assert_importe(liquidado.unwrap(), 6000.0, "el importe realmente debitado");
    assert_eq!(divisa_liq.as_deref(), Some("DOP"));
    assert_importe(costo_adicional(id), 12.0, "la retención se calcula sobre los pesos");
}

#[test]
fn c24_cruzar_divisas_sin_tasa_falla_sin_mover_nada() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100000.0);

    let mut entrada = transferencia(100.0, "Alimentación", "Compra en el exterior", cuenta);
    entrada.divisa = "USD".to_string();
    let resultado = crear_gasto(entrada);

    assert!(resultado.is_err(), "debe exigir la tasa");
    assert!(resultado.unwrap_err().contains("tasa de cambio"), "el mensaje debe orientar");
    assert_importe(balance_cuenta("Cuenta Ahorros DOP"), 100000.0, "saldo intacto");
    assert_eq!(total_gastos(), 0);
}

#[test]
fn c25_revertir_un_gasto_convertido_restituye_el_saldo_exacto() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100000.0);

    let mut entrada = transferencia(100.0, "Alimentación", "Compra en el exterior", cuenta);
    entrada.divisa = "USD".to_string();
    entrada.tasa_cambio = Some(60.0);
    let id = crear_gasto(entrada).unwrap();

    eliminar_gasto(id).unwrap();

    assert_importe(balance_cuenta("Cuenta Ahorros DOP"), 100000.0, "restitución al centavo");
}

// =====================================================================
//  Liquidación pendiente — ciclo completo por los comandos reales
// =====================================================================

fn fijar_politica(tarjeta_id: i64, politica: &str) {
    conexion()
        .execute("UPDATE tarjetas SET politica_liquidacion = ? WHERE id = ?;", params![politica, tarjeta_id])
        .expect("fijar política de liquidación");
}

fn estado_conversion(gasto_id: i64) -> Option<String> {
    conexion()
        .query_row("SELECT estado_conversion FROM gastos WHERE id = ?;", [gasto_id], |r| r.get(0))
        .expect("leer estado de conversión")
}

fn compra_en_dolares(tarjeta: i64) -> GastoInput {
    GastoInput {
        fecha: "10/09/2026".to_string(),
        monto: 100.0,
        divisa: "USD".to_string(),
        descripcion: "Compra en el exterior".to_string(),
        categoria_id: id_categoria("Otros"),
        metodo_pago: "tarjeta".to_string(),
        es_lbtr: false,
        tarjeta_id: Some(tarjeta),
        cuenta_ahorro_id: None,
        tasa_cambio: None,
    }
}

#[test]
fn c26_una_compra_en_divisa_con_tarjeta_que_traduce_queda_pendiente() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    fijar_politica(tarjeta, "traduce");

    let id = crear_gasto(compra_en_dolares(tarjeta)).unwrap();

    assert_eq!(estado_conversion(id).as_deref(), Some("pendiente"));
    let (pesos, dolares) = balances_tarjeta(tarjeta);
    assert_importe(dolares, 100.0, "la deuda sube en dólares, como hace el emisor");
    assert_importe(pesos, 0.0, "sin cifra en pesos: todavía no existe");
}

#[test]
fn c27_la_misma_compra_con_tarjeta_que_liquida_en_origen_no_queda_pendiente() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);

    let id = crear_gasto(compra_en_dolares(tarjeta)).unwrap();

    assert_eq!(estado_conversion(id), None, "nada que liquidar");
    assert_importe(balances_tarjeta(tarjeta).1, 100.0, "se queda en dólares");
}

#[test]
fn c28_liquidar_traslada_el_saldo_entre_divisas_y_guarda_la_tasa_deducida() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    fijar_politica(tarjeta, "traduce");
    let id = crear_gasto(compra_en_dolares(tarjeta)).unwrap();

    // El emisor informa que cargó 6 050.00 en pesos.
    let tasa = crate::liquidar_consumo_pendiente(id, 6050.0).unwrap();

    assert_importe(tasa, 60.5, "la tasa se deduce del importe, no se pide");
    assert_eq!(estado_conversion(id).as_deref(), Some("liquidado"));
    let (pesos, dolares) = balances_tarjeta(tarjeta);
    assert_importe(dolares, 0.0, "baja de la divisa de origen");
    assert_importe(pesos, 6050.0, "y sube en moneda local");
}

#[test]
fn c29_no_se_liquida_dos_veces_ni_lo_que_no_esta_pendiente() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    fijar_politica(tarjeta, "traduce");
    let id = crear_gasto(compra_en_dolares(tarjeta)).unwrap();

    crate::liquidar_consumo_pendiente(id, 6050.0).unwrap();
    assert!(crate::liquidar_consumo_pendiente(id, 6050.0).is_err(), "no se liquida dos veces");
    assert_importe(balances_tarjeta(tarjeta).0, 6050.0, "ni se duplica el traslado");
}

// =====================================================================
//  Bonificaciones — ciclo completo por los comandos reales
// =====================================================================

fn deuda_pesos(tarjeta: i64) -> f64 {
    balances_tarjeta(tarjeta).0
}

#[test]
fn c30_una_bonificacion_reduce_la_deuda_sin_tocar_el_gasto() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(10000.0, 0.0);

    let entrada = GastoInput {
        fecha: "09/09/2026".to_string(),
        monto: 1234.56,
        divisa: "DOP".to_string(),
        descripcion: "Suscripción".to_string(),
        categoria_id: id_categoria("Suscripciones"),
        metodo_pago: "tarjeta".to_string(),
        es_lbtr: false,
        tarjeta_id: Some(tarjeta),
        cuenta_ahorro_id: None,
        tasa_cambio: None,
    };
    let gasto = crear_gasto(entrada).unwrap();
    assert_importe(deuda_pesos(tarjeta), 11234.56, "el consumo sube la deuda");

    crate::crear_bonificacion(
        "09/09/2026".to_string(), tarjeta, 61.73, "DOP".to_string(),
        "Cashback compra por internet".to_string(), Some(gasto),
    ).unwrap();

    assert_importe(deuda_pesos(tarjeta), 11172.83, "la bonificación la reduce");
    let (monto_gasto, _, _) = ultimo_gasto();
    assert_importe(monto_gasto, 1234.56, "el consumo original no se altera");
}

#[test]
fn c31_un_mismo_gasto_admite_varias_bonificaciones() {
    // Caso real: 1 % base y 2 % de categoría, en dos líneas del mismo día.
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(10000.0, 0.0);

    // El consumo que las genera, con un 3 % repartido en dos créditos.
    let gasto = crear_gasto(GastoInput {
        fecha: "09/09/2026".to_string(),
        monto: 5000.00,
        divisa: "DOP".to_string(),
        descripcion: "Consumo bonificado".to_string(),
        categoria_id: id_categoria("Alimentación"),
        metodo_pago: "tarjeta".to_string(),
        es_lbtr: false,
        tarjeta_id: Some(tarjeta),
        cuenta_ahorro_id: None,
        tasa_cambio: None,
    })
    .unwrap();

    crate::crear_bonificacion("09/09/2026".into(), tarjeta, 50.00, "DOP".into(),
        "Recompensa base".into(), Some(gasto)).unwrap();
    crate::crear_bonificacion("09/09/2026".into(), tarjeta, 100.00, "DOP".into(),
        "Bonificación de categoría".into(), Some(gasto)).unwrap();

    assert_eq!(crate::obtener_bonificaciones().unwrap().len(), 2);
    assert_importe(deuda_pesos(tarjeta), 10000.0 + 5000.00 - 150.00, "el consumo sube y las dos bonificaciones bajan");
}

#[test]
fn c32_revertir_una_bonificacion_restituye_la_deuda() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(10000.0, 0.0);
    let id = crate::crear_bonificacion("09/09/2026".into(), tarjeta, 61.73, "DOP".into(),
        "Cashback".into(), None).unwrap();

    crate::eliminar_bonificacion(id).unwrap();

    assert_importe(deuda_pesos(tarjeta), 10000.0, "restitución exacta");
    assert!(crate::obtener_bonificaciones().unwrap().is_empty());
    assert!(crate::eliminar_bonificacion(id).is_err(), "no se revierte dos veces");
}

#[test]
fn c33_una_bonificacion_sin_concepto_o_en_cero_se_rechaza() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(10000.0, 0.0);

    assert!(crate::crear_bonificacion("09/09/2026".into(), tarjeta, 61.73, "DOP".into(),
        "   ".into(), None).is_err(), "el concepto es obligatorio");
    assert!(crate::crear_bonificacion("09/09/2026".into(), tarjeta, 0.0, "DOP".into(),
        "Cashback".into(), None).is_err(), "cero no es una bonificación");
    assert_importe(deuda_pesos(tarjeta), 10000.0, "ningún saldo se movió");
}

// ---------------------------------------------------------------------------
//  Saldo vivo de financiamientos (C34–C39)
//
//  El pasivo se deducía de `monto_prestamo` y la fracción de cuotas
//  pendientes. En una línea revolvente, que no tiene cuotas contadas, esa
//  fracción no existía y el pasivo quedaba clavado en el monto desembolsado.
// ---------------------------------------------------------------------------

/// Registra un financiamiento. `cuotas` a `None` lo hace una línea revolvente.
fn crear_prestamo_de_prueba(
    tipo: &str,
    monto: f64,
    tasa: f64,
    cuota: f64,
    cuotas: Option<(i32, i32)>,
    limite: Option<f64>,
) -> i64 {
    let (totales, pendientes) = match cuotas {
        Some((t, p)) => (Some(t), Some(p)),
        None => (None, None),
    };
    let c = conexion();
    c.execute(
        "INSERT INTO prestamos (tipo_prestamo, monto_prestamo, institucion_financiera,
                                tasa_actual, cuotas_totales, cuotas_pendientes,
                                monto_cuota, dia_pago, saldo_actual, limite_credito)
         VALUES (?, ?, 'Banco Ejemplo', ?, ?, ?, ?, 25, ?, ?);",
        params![tipo, monto, tasa, totales, pendientes, cuota, monto, limite],
    )
    .expect("insertar financiamiento de prueba");
    c.last_insert_rowid()
}

fn saldo_prestamo(id: i64) -> f64 {
    conexion()
        .query_row("SELECT saldo_actual FROM prestamos WHERE id = ?;", [id], |r| r.get(0))
        .expect("leer saldo")
}

fn cuotas_pendientes(id: i64) -> Option<i32> {
    conexion()
        .query_row("SELECT cuotas_pendientes FROM prestamos WHERE id = ?;", [id], |r| r.get(0))
        .expect("leer cuotas")
}

#[test]
fn c34_pagar_una_cuota_de_la_linea_revolvente_baja_el_saldo() {
    // El caso que motivó todo: antes el comando llevaba
    // `WHERE cuotas_pendientes IS NOT NULL` y la línea nunca se tocaba.
    let _g = entorno_aislado();
    // 100 000 al 12 % anual: 1 % mensual = 1 000 de interés, 4 000 de capital.
    let id = crear_prestamo_de_prueba("flexible", 100_000.0, 12.0, 5_000.0, None, Some(150_000.0));

    crate::pagar_cuota_prestamo(id, Some("25/09/2026".into())).unwrap();

    assert_importe(saldo_prestamo(id), 96_000.0, "el saldo baja por el capital, no por la cuota");
}

#[test]
fn c35_la_cuota_no_reduce_la_deuda_por_su_importe_entero() {
    let _g = entorno_aislado();
    let id = crear_prestamo_de_prueba("vehiculo", 100_000.0, 12.0, 5_000.0, Some((100, 89)), None);

    crate::pagar_cuota_prestamo(id, Some("17/09/2026".into())).unwrap();

    assert_importe(saldo_prestamo(id), 96_000.0, "5 000 de cuota amortizan 4 000");
    assert_eq!(cuotas_pendientes(id), Some(88), "el contador sí baja de uno en uno");
}

#[test]
fn c36_cada_cuota_queda_asentada_con_su_desglose() {
    let _g = entorno_aislado();
    let id = crear_prestamo_de_prueba("flexible", 100_000.0, 12.0, 5_000.0, None, Some(150_000.0));

    crate::pagar_cuota_prestamo(id, Some("25/09/2026".into())).unwrap();
    crate::pagar_cuota_prestamo(id, Some("25/10/2026".into())).unwrap();

    let movimientos = crate::obtener_movimientos_prestamo(id).unwrap();
    assert_eq!(movimientos.len(), 2, "un asiento por cuota");

    // Vienen en orden descendente: el más reciente primero.
    let ultimo = &movimientos[0];
    assert_eq!(ultimo.tipo, "cuota");
    assert_importe(ultimo.interes, 960.0, "1 % de 96 000");
    assert_importe(ultimo.capital, 4_040.0, "5 000 - 960");
    assert_importe(ultimo.saldo_resultante, 91_960.0, "saldo tras la segunda cuota");
    assert_importe(saldo_prestamo(id), ultimo.saldo_resultante, "el saldo es el del asiento");
}

#[test]
fn c37_declarar_el_saldo_del_estado_lo_fija_y_deja_constancia_de_la_diferencia() {
    // La estimación por cuotas nunca cuadra al centavo con el acreedor. La
    // declaración corrige, pero no en silencio.
    let _g = entorno_aislado();
    let id = crear_prestamo_de_prueba("flexible", 100_000.0, 12.0, 5_000.0, None, Some(150_000.0));
    crate::pagar_cuota_prestamo(id, Some("25/09/2026".into())).unwrap();
    assert_importe(saldo_prestamo(id), 96_000.0, "estimación");

    crate::declarar_saldo_prestamo(id, 96_250.75, Some("30/09/2026".into())).unwrap();

    assert_importe(saldo_prestamo(id), 96_250.75, "manda el estado de cuenta");
    let movimientos = crate::obtener_movimientos_prestamo(id).unwrap();
    assert_eq!(movimientos[0].tipo, "declaracion");
    assert_importe(movimientos[0].monto, 250.75, "la diferencia queda registrada");
}

#[test]
fn c38_una_cuota_que_no_cubre_el_interes_hace_crecer_la_deuda() {
    // No se recorta en cero. Es la misma decisión que en H5: un saldo que se
    // mueve en la dirección incómoda se muestra, no se descarta.
    let _g = entorno_aislado();
    let id = crear_prestamo_de_prueba("flexible", 100_000.0, 12.0, 500.0, None, Some(150_000.0));

    crate::pagar_cuota_prestamo(id, Some("25/09/2026".into())).unwrap();

    assert_importe(saldo_prestamo(id), 100_500.0, "500 de cuota contra 1 000 de interés");
}

#[test]
fn c39_el_limite_solo_se_admite_en_una_linea_revolvente() {
    let _g = entorno_aislado();
    let con_limite = |tipo: &str| crate::crear_prestamo(crate::PrestamoInput {
        tipo_prestamo: tipo.into(),
        monto_prestamo: 100_000.0,
        institucion_financiera: "Banco Ejemplo".into(),
        tasa_actual: 12.0,
        cuotas_totales: Some(60),
        cuotas_pendientes: Some(60),
        monto_cuota: 5_000.0,
        dia_pago: 25,
        saldo_actual: None,
        limite_credito: Some(150_000.0),
    });

    assert!(con_limite("vehiculo").is_err(), "un amortizable no repone cupo");
    assert!(con_limite("flexible").is_ok());
}

// ---------------------------------------------------------------------------
//  Vínculo con la tarjeta que cobra la facilidad (C40–C44)
//
//  Algunas líneas no son productos independientes: cuelgan de una tarjeta,
//  comparten su ciclo de corte y se cobran dentro de su pago mínimo.
// ---------------------------------------------------------------------------

fn vincular_a_tarjeta(prestamo_id: i64, tarjeta_id: Option<i64>) -> Result<(), String> {
    let (tasa, cuota, dia): (f64, f64, i32) = conexion()
        .query_row(
            "SELECT tasa_actual, monto_cuota, dia_pago FROM prestamos WHERE id = ?;",
            [prestamo_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .expect("leer condiciones");
    crate::actualizar_prestamo(crate::ActualizarPrestamoInput {
        id: prestamo_id,
        tasa_actual: tasa,
        monto_cuota: cuota,
        dia_pago: dia,
        limite_credito: None,
        tarjeta_id,
    })
}

fn prestamo_por_id(id: i64) -> crate::Prestamo {
    crate::obtener_prestamos()
        .unwrap()
        .into_iter()
        .find(|p| p.id == id)
        .expect("financiamiento en la lista")
}

#[test]
fn c40_una_facilidad_vinculada_toma_las_fechas_de_su_tarjeta() {
    // Es la razón de ser del vínculo: no duplicar las fechas. Guardarlas dos
    // veces obliga a sincronizarlas a mano, y se desincronizan.
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0); // corte 15, límite de pago 5
    let linea = crear_prestamo_de_prueba("flexible", 100_000.0, 12.0, 5_000.0, None, Some(150_000.0));

    // Antes de vincular manda su propio día, el que se registró: 25.
    assert_eq!(prestamo_por_id(linea).dia_pago, 25);
    assert_eq!(prestamo_por_id(linea).dia_corte, None);

    vincular_a_tarjeta(linea, Some(tarjeta)).unwrap();

    let p = prestamo_por_id(linea);
    assert_eq!(p.dia_pago, 5, "vence cuando vence la tarjeta");
    assert_eq!(p.dia_corte, Some(15), "corta cuando corta la tarjeta");
    assert_eq!(p.tarjeta_nombre.as_deref(), Some("Tarjeta Ejemplo"));
}

#[test]
fn c41_un_financiamiento_sin_vincular_conserva_su_propio_dia_de_pago() {
    let _g = entorno_aislado();
    let auto = crear_prestamo_de_prueba("vehiculo", 100_000.0, 12.0, 5_000.0, Some((100, 89)), None);

    let p = prestamo_por_id(auto);
    assert_eq!(p.dia_pago, 25, "el suyo, no el de ninguna tarjeta");
    assert_eq!(p.dia_corte, None, "un préstamo suelto no tiene corte");
    assert_eq!(p.tarjeta_id, None);
}

#[test]
fn c42_desvincular_devuelve_el_financiamiento_a_sus_propias_fechas() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let linea = crear_prestamo_de_prueba("flexible", 100_000.0, 12.0, 5_000.0, None, None);
    vincular_a_tarjeta(linea, Some(tarjeta)).unwrap();
    assert_eq!(prestamo_por_id(linea).dia_pago, 5);

    vincular_a_tarjeta(linea, None).unwrap();

    let p = prestamo_por_id(linea);
    assert_eq!(p.dia_pago, 25, "vuelve el día registrado");
    assert_eq!(p.dia_corte, None);
}

#[test]
fn c43_actualizar_no_es_una_puerta_trasera_para_mover_el_saldo() {
    // El saldo solo se mueve por una cuota o por una declaración, que dejan
    // asiento. Si `actualizar_prestamo` pudiera tocarlo, habría una vía para
    // cambiarlo sin rastro.
    let _g = entorno_aislado();
    let linea = crear_prestamo_de_prueba("flexible", 100_000.0, 12.0, 5_000.0, None, None);
    crate::pagar_cuota_prestamo(linea, Some("25/09/2026".into())).unwrap();
    let saldo_antes = saldo_prestamo(linea);

    crate::actualizar_prestamo(crate::ActualizarPrestamoInput {
        id: linea,
        tasa_actual: 24.0,
        monto_cuota: 7_000.0,
        dia_pago: 10,
        limite_credito: Some(200_000.0),
        tarjeta_id: None,
    })
    .unwrap();

    let p = prestamo_por_id(linea);
    assert_importe(saldo_prestamo(linea), saldo_antes, "el saldo no se toca");
    assert_importe(p.monto_prestamo, 100_000.0, "el desembolso original es histórico");
    assert_importe(p.tasa_actual, 24.0, "las condiciones sí se corrigen");
    assert_eq!(p.limite_credito, Some(200_000.0), "el límite se puede registrar después");
    assert_eq!(
        crate::obtener_movimientos_prestamo(linea).unwrap().len(),
        1,
        "no aparecen asientos nuevos"
    );
}

#[test]
fn c44_actualizar_rechaza_lo_que_no_tiene_sentido() {
    let _g = entorno_aislado();
    let auto = crear_prestamo_de_prueba("vehiculo", 100_000.0, 12.0, 5_000.0, Some((100, 89)), None);
    let base = |limite, tarjeta, dia| crate::ActualizarPrestamoInput {
        id: auto,
        tasa_actual: 12.0,
        monto_cuota: 5_000.0,
        dia_pago: dia,
        limite_credito: limite,
        tarjeta_id: tarjeta,
    };

    assert!(crate::actualizar_prestamo(base(Some(50_000.0), None, 25)).is_err(),
        "un amortizable no repone cupo");
    assert!(crate::actualizar_prestamo(base(None, Some(999_999), 25)).is_err(),
        "no se vincula a una tarjeta inexistente");
    assert!(crate::actualizar_prestamo(base(None, None, 32)).is_err(),
        "el día 32 no existe");
    assert!(crate::actualizar_prestamo(base(None, None, 25)).is_ok());
}

// ---------------------------------------------------------------------------
//  Fase 3.1 — Caracterización del vertical de Cuentas (C45–C54)
//
//  Fija la conducta vigente de transferencias y borrados ANTES de extraer el
//  vertical a puertos y casos de uso. Cuatro de estas pruebas documentan
//  hallazgos —H10 a H13— y afirman lo que el sistema hace hoy, no lo que
//  debería hacer. Cambiar cualquiera de ellas exige una decisión explícita.
// ---------------------------------------------------------------------------

fn saldo_cuenta_id(id: i64) -> f64 {
    conexion()
        .query_row("SELECT balance_actual FROM cuentas_ahorro WHERE id = ?;", [id], |r| r.get(0))
        .expect("leer saldo de cuenta")
}

fn total_transferencias() -> i64 {
    conexion()
        .query_row("SELECT COUNT(*) FROM transacciones_cuentas;", [], |r| r.get(0))
        .expect("contar transferencias")
}

fn ultima_transferencia() -> i64 {
    conexion()
        .query_row("SELECT MAX(id) FROM transacciones_cuentas;", [], |r| r.get(0))
        .expect("última transferencia")
}

#[test]
fn c45_una_transferencia_mueve_los_dos_saldos_y_cobra_el_cargo_al_origen() {
    let _g = entorno_aislado();
    let origen = crear_cuenta("Cuenta Ahorros DOP", "DOP", 50_000.0);
    let destino = crear_cuenta("Cuenta Corriente DOP", "DOP", 10_000.0);

    crate::transferir_entre_cuentas(
        "13/09/2026".into(), origen, destino, 8_000.0, 8_000.0, 100.0, "Traspaso".into(),
    )
    .unwrap();

    assert_importe(saldo_cuenta_id(origen), 41_900.0, "sale el monto y el cargo");
    assert_importe(saldo_cuenta_id(destino), 18_000.0, "entra solo el monto");
    assert_eq!(total_transferencias(), 1);
}

#[test]
fn c46_la_tasa_se_deduce_dividiendo_los_dos_importes() {
    let _g = entorno_aislado();
    let origen = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100_000.0);
    let destino = crear_cuenta("Cuenta Ahorros USD", "USD", 0.0);

    crate::transferir_entre_cuentas(
        "13/09/2026".into(), origen, destino, 6_000.0, 100.0, 0.0, "Compra de divisa".into(),
    )
    .unwrap();

    let tasa: f64 = conexion()
        .query_row("SELECT tasa_cambio FROM transacciones_cuentas WHERE id = ?;",
                   [ultima_transferencia()], |r| r.get(0))
        .unwrap();
    assert_importe(tasa, 100.0 / 6_000.0, "monto_destino / monto_origen");
}

#[test]
fn c47_revertir_una_transferencia_devuelve_el_monto_y_el_cargo_al_origen() {
    let _g = entorno_aislado();
    let origen = crear_cuenta("Cuenta Ahorros DOP", "DOP", 50_000.0);
    let destino = crear_cuenta("Cuenta Corriente DOP", "DOP", 10_000.0);
    crate::transferir_entre_cuentas(
        "13/09/2026".into(), origen, destino, 8_000.0, 8_000.0, 100.0, "Traspaso".into(),
    )
    .unwrap();

    crate::eliminar_transaccion_cuenta(ultima_transferencia()).unwrap();

    assert_importe(saldo_cuenta_id(origen), 50_000.0, "restitución exacta con cargo");
    assert_importe(saldo_cuenta_id(destino), 10_000.0, "restitución exacta");
    assert_eq!(total_transferencias(), 0);
}

#[test]
fn c48_h10_la_reversion_devuelve_los_dos_saldos_aunque_el_destino_quede_negativo() {
    // **H10 resuelto.** Antes el origen se restituía sin límite pero al
    // destino se le aplicaba MAX(0.0, ...), de modo que el total repartido
    // entre las dos cuentas subía y el patrimonio quedaba inflado.
    //
    // Revertir significa «esta transferencia nunca ocurrió»: las dos cuentas
    // vuelven al estado que tenían. Si el destino ya gastó lo recibido, el
    // negativo informa de que faltan movimientos por registrar allí.
    let _g = entorno_aislado();
    let origen = crear_cuenta("Cuenta Ahorros DOP", "DOP", 50_000.0);
    let destino = crear_cuenta("Cuenta Corriente DOP", "DOP", 0.0);
    crate::transferir_entre_cuentas(
        "13/09/2026".into(), origen, destino, 8_000.0, 8_000.0, 0.0, "Traspaso".into(),
    )
    .unwrap();

    // El titular gasta 5 000 del destino antes de darse cuenta del error.
    conexion()
        .execute("UPDATE cuentas_ahorro SET balance_actual = 3000.0 WHERE id = ?;", [destino])
        .unwrap();

    crate::eliminar_transaccion_cuenta(ultima_transferencia()).unwrap();

    assert_importe(saldo_cuenta_id(destino), -5_000.0, "3 000 - 8 000, sin recorte");
    assert_importe(saldo_cuenta_id(origen), 50_000.0, "y el origen se restituye entero");
}

#[test]
fn c49_h11_una_transferencia_de_una_cuenta_a_si_misma_se_rechaza() {
    // **Conducta corregida en la Fase 3.** Antes se aceptaba: la operación
    // restaba `monto + cargo` y sumaba `monto` sobre la misma fila, dejando el
    // saldo alterado por el cargo y un asiento que no representaba nada.
    //
    // El tipo `Transferencia` ya no admite construirla, de modo que H11 no se
    // comprueba en ningún sitio: dejó de ser representable. La prueba cambia
    // de sentido a propósito, y el cambio queda registrado aquí en vez de
    // pasar inadvertido.
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 50_000.0);

    let error = crate::transferir_entre_cuentas(
        "13/09/2026".into(), cuenta, cuenta, 8_000.0, 8_000.0, 100.0, "A sí misma".into(),
    )
    .unwrap_err();

    assert!(error.contains("dos cuentas distintas"), "explica por qué: {error}");
    assert_importe(saldo_cuenta_id(cuenta), 50_000.0, "ningún saldo se movió");
    assert_eq!(total_transferencias(), 0, "ni quedó asiento");
}

#[test]
fn c50_h12_el_importe_de_destino_se_interpreta_en_la_divisa_de_su_cuenta() {
    // **Matiz importante sobre H12.** El dominio impide construir una
    // transferencia cuyo importe lleve la divisa equivocada, pero eso cierra
    // el error del *programador*, no el del *usuario*: el formulario pide dos
    // números sueltos y la divisa se deduce de la cuenta elegida, así que no
    // hay ninguna declaración que contradecir.
    //
    // Quien teclea 6 000 pensando en pesos y elige una cuenta en dólares
    // acredita seis mil dólares. Sigue siendo la conducta vigente y la prueba
    // la fija. La mitigación es de interfaz: mostrar la divisa junto a cada
    // importe y avisar cuando la transferencia cruza divisas.
    let _g = entorno_aislado();
    let origen = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100_000.0);
    let destino = crear_cuenta("Cuenta Ahorros USD", "USD", 0.0);

    crate::transferir_entre_cuentas(
        "13/09/2026".into(), origen, destino, 6_000.0, 6_000.0, 0.0, "Sin convertir".into(),
    )
    .unwrap();

    assert_importe(saldo_cuenta_id(destino), 6_000.0, "seis mil dólares donde había pesos (H12)");
}

#[test]
fn c51_h13_una_cuenta_con_transferencias_no_se_puede_eliminar() {
    // **H13 resuelto.** Antes la guarda contaba gastos pero no transferencias,
    // y como la clave foránea es ON DELETE CASCADE el historial se iba en
    // silencio, dejando a la contraparte con el dinero recibido sin
    // constancia de dónde había salido.
    let _g = entorno_aislado();
    let origen = crear_cuenta("Cuenta Ahorros DOP", "DOP", 50_000.0);
    let destino = crear_cuenta("Cuenta Corriente DOP", "DOP", 0.0);
    crate::transferir_entre_cuentas(
        "13/09/2026".into(), origen, destino, 8_000.0, 8_000.0, 0.0, "Traspaso".into(),
    )
    .unwrap();
    assert_eq!(total_transferencias(), 1);

    let error = crate::eliminar_cuenta(origen).unwrap_err();

    assert!(error.contains("transferencias"), "explica por qué: {error}");
    assert_eq!(total_transferencias(), 1, "el historial sigue ahí");
    assert_importe(saldo_cuenta_id(origen), 42_000.0, "y la cuenta también");
}

#[test]
fn c52_una_cuenta_con_gastos_asociados_no_se_puede_eliminar() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 50_000.0);
    crear_gasto(transferencia(1_000.0, "Alimentación", "Compra", cuenta)).unwrap();

    assert!(crate::eliminar_cuenta(cuenta).is_err(), "la guarda sí cubre los gastos");
}

#[test]
fn c53_una_transferencia_puede_dejar_el_origen_en_negativo() {
    // No se comprueban fondos. Se fija como conducta vigente: decidir si un
    // sobregiro es legítimo corresponde al titular, no a esta prueba.
    let _g = entorno_aislado();
    let origen = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let destino = crear_cuenta("Cuenta Corriente DOP", "DOP", 0.0);

    crate::transferir_entre_cuentas(
        "13/09/2026".into(), origen, destino, 5_000.0, 5_000.0, 0.0, "Sobregiro".into(),
    )
    .unwrap();

    assert_importe(saldo_cuenta_id(origen), -4_000.0, "el origen queda en negativo");
}

#[test]
fn c54_revertir_dos_veces_la_misma_transferencia_falla_la_segunda() {
    let _g = entorno_aislado();
    let origen = crear_cuenta("Cuenta Ahorros DOP", "DOP", 50_000.0);
    let destino = crear_cuenta("Cuenta Corriente DOP", "DOP", 10_000.0);
    crate::transferir_entre_cuentas(
        "13/09/2026".into(), origen, destino, 8_000.0, 8_000.0, 0.0, "Traspaso".into(),
    )
    .unwrap();
    let id = ultima_transferencia();

    crate::eliminar_transaccion_cuenta(id).unwrap();

    assert!(crate::eliminar_transaccion_cuenta(id).is_err(), "no se revierte dos veces");
    assert_importe(saldo_cuenta_id(origen), 50_000.0, "sin doble restitución");
}

// ---------------------------------------------------------------------------
//  Errores en la preparación del esquema (C55–C56)
//
//  Antes, `inicializar_db` aplicaba sus migraciones con
//  `let _ = conn.execute(...)`: el patrón nació para tolerar una condición
//  esperada —la columna ya existe— pero descartaba cualquier error. Estas
//  pruebas documentaban que existía un estado en el que la preparación
//  declaraba éxito y dejaba el esquema inservible.
//
//  **Resuelto por el ejecutor de migraciones versionadas.** Ahora la condición
//  esperada se comprueba y el fallo se propaga con su causa. Las pruebas
//  cambian de sentido y lo dicen.
// ---------------------------------------------------------------------------

/// Aísla el entorno **sin** inicializar la base, para poder sembrarla antes.
fn entorno_sin_inicializar() -> (MutexGuard<'static, ()>, String) {
    let guarda = bloquear_entorno();
    let raiz = raiz_temporal();
    std::fs::create_dir_all(&raiz).expect("crear raíz temporal");
    std::env::set_var("HOME", &raiz);

    let ruta = db_sql::obtener_ruta_db();
    assert!(
        ruta.starts_with(raiz.to_str().unwrap()),
        "ABORTADO: las pruebas apuntarían a la base real ({})",
        ruta
    );
    let _ = std::fs::remove_file(&ruta);
    let _ = std::fs::remove_dir_all(crate::respaldo::directorio_de_respaldos());
    if let Some(padre) = std::path::Path::new(&ruta).parent() {
        std::fs::create_dir_all(padre).expect("crear directorio de la base");
    }
    (guarda, ruta)
}

/// Siembra una base en la que `tarjetas` es una VISTA.
///
/// Es el estado en que quedaría tras una migración a medias o una base tocada
/// a mano: `CREATE TABLE IF NOT EXISTS` no falla sobre una vista —no hace
/// nada— y el `ALTER` siguiente no puede aplicarse.
fn sembrar_esquema_inservible(ruta: &str) {
    let c = Connection::open(ruta).expect("abrir base sembrada");
    c.execute_batch(
        "CREATE TABLE origen (id INTEGER PRIMARY KEY, entidad TEXT);
         CREATE VIEW tarjetas AS SELECT id, entidad FROM origen;",
    )
    .expect("sembrar la vista");
}

#[test]
fn c55_inicializar_falla_sobre_un_esquema_que_no_puede_migrar() {
    // **Conducta corregida.** Antes devolvía `Ok` y la aplicación arrancaba
    // contra un esquema al que le faltaban ocho columnas.
    let (_g, ruta) = entorno_sin_inicializar();
    sembrar_esquema_inservible(&ruta);

    let error = db_sql::inicializar_db().unwrap_err().to_string();

    assert!(error.contains("tarjetas"), "nombra la estructura: {error}");
    assert!(error.contains("view"), "y dice qué encontró: {error}");
    assert!(error.contains("sin modificar"), "y que no tocó nada: {error}");
}

#[test]
fn c56_el_fallo_aparece_donde_ocurre_y_la_base_no_avanza_de_version() {
    // **Conducta corregida.** Antes el fallo emergía lejos de su causa, en la
    // primera consulta que tocara una columna ausente. Ahora se detiene en la
    // preparación y la versión de esquema no avanza, de modo que un reintento
    // parte de un estado con nombre.
    let (_g, ruta) = entorno_sin_inicializar();
    sembrar_esquema_inservible(&ruta);

    assert!(db_sql::inicializar_db().is_err(), "se queja donde ocurre");

    let c = Connection::open(&ruta).unwrap();
    assert_eq!(crate::migraciones::version_de(&c).unwrap(), 0, "la versión no avanzó");
    let categorias: i64 = c
        .query_row("SELECT COUNT(*) FROM sqlite_master WHERE name='categorias';", [], |r| r.get(0))
        .unwrap();
    assert_eq!(categorias, 0, "ni quedó a medias lo que la migración alcanzó a crear");
}

#[test]
fn c57_preparar_el_esquema_respalda_antes_de_tocar_una_base_existente() {
    // La garantía que da valor a todo lo demás: cuando hay algo que perder,
    // hay una copia verificada antes de modificar nada.
    let (_g, ruta) = entorno_sin_inicializar();

    // Instalación nueva: no hay nada que respaldar.
    db_sql::inicializar_db().expect("preparar base nueva");
    assert!(
        crate::respaldo::directorio_de_respaldos().read_dir().map(|d| d.count()).unwrap_or(0) == 0,
        "una instalación nueva no genera respaldos: no hay nada que perder"
    );

    // Ahora sí hay datos, y el archivo cambia.
    crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    assert!(std::path::Path::new(&ruta).exists());

    db_sql::inicializar_db().expect("preparar base existente");

    let respaldos: Vec<_> = crate::respaldo::directorio_de_respaldos()
        .read_dir()
        .expect("leer respaldos")
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(respaldos.len(), 1, "se tomó exactamente un respaldo");

    // Y el respaldo es una base utilizable con los datos dentro.
    let copia = Connection::open(respaldos[0].path()).expect("abrir el respaldo");
    let cuentas: i64 = copia
        .query_row("SELECT COUNT(*) FROM cuentas_ahorro WHERE nombre = 'Cuenta Ahorros DOP';",
                   [], |r| r.get(0))
        .expect("consultar el respaldo");
    assert_eq!(cuentas, 1, "los datos están en la copia");
}

#[test]
#[ignore = "simulación manual sobre una copia de una base histórica"]
fn simulacion_base_historica() {
    // La ruta llega por `MICHELITOS_SIMULACION` en vez de por `HOME`: cambiar
    // `HOME` rompe a rustup y el propio `cargo test` deja de arrancar.
    let ruta = std::env::var("MICHELITOS_SIMULACION")
        .expect("indique MICHELITOS_SIMULACION con la ruta de una COPIA");
    let mut c = Connection::open(&ruta).expect("abrir copia");
    let antes: Vec<i64> = ["gastos", "cuentas_ahorro", "tarjetas", "transacciones_cuentas"]
        .iter()
        .map(|t| c.query_row(&format!("SELECT COUNT(*) FROM {};", t), [], |r| r.get(0)).unwrap())
        .collect();

    let informe = crate::migraciones::ejecutar(&mut c).expect("migrar base histórica");
    db_sql::sembrar(&c).expect("sembrar");
    println!("versión final: {}", crate::migraciones::version_de(&c).unwrap());

    let despues: Vec<i64> = ["gastos", "cuentas_ahorro", "tarjetas", "transacciones_cuentas"]
        .iter()
        .map(|t| c.query_row(&format!("SELECT COUNT(*) FROM {};", t), [], |r| r.get(0)).unwrap())
        .collect();

    println!("informe: {:?}", informe);
    println!("filas antes:   {:?}", antes);
    println!("filas después: {:?}", despues);
    assert_eq!(antes, despues, "ninguna fila se perdió ni apareció");
    assert_eq!(informe.version_final, crate::migraciones::VERSION_OBJETIVO);

    let integridad: String = c.query_row("PRAGMA integrity_check;", [], |r| r.get(0)).unwrap();
    assert_eq!(integridad, "ok");
    let rotas: i64 =
        c.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check;", [], |r| r.get(0)).unwrap();
    assert_eq!(rotas, 0, "sin referencias rotas");

    // Segunda pasada: nada que aplicar.
    let segunda = crate::migraciones::ejecutar(&mut c).expect("segunda pasada");
    assert!(segunda.aplicadas.is_empty(), "no se repite nada");
}

// ---------------------------------------------------------------------------
//  Comisión fija por el servicio de pago de impuestos
//
//  Recorre el camino entero —comando, caso de uso, adaptador SQLite— porque la
//  regla vive en el dominio pero la tarifa vive en una columna: probar solo el
//  dominio dejaría sin verificar que el adaptador la lee.
// ---------------------------------------------------------------------------

#[test]
fn c60_un_pago_de_impuestos_cobra_la_tarifa_de_la_cuenta_y_no_retiene() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Corriente DOP", "DOP", 100_000.0);
    declarar_comision_de_impuestos(cuenta, 75.0);

    crear_gasto(transferencia(10_000.0, "Impuestos", "Pago DGII", cuenta)).unwrap();

    // Sale el importe más la tarifa; ni un céntimo de retención.
    assert_importe(saldo_cuenta_id(cuenta), 89_925.0, "10 000 + 75 de servicio");
}

#[test]
fn c61_la_tarifa_no_alcanza_a_los_gastos_que_no_son_impuestos() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Corriente DOP", "DOP", 100_000.0);
    declarar_comision_de_impuestos(cuenta, 75.0);

    crear_gasto(transferencia(10_000.0, "Alimentación", "Compra", cuenta)).unwrap();

    // Retención ordinaria del 0.20 %, sin rastro de la tarifa.
    assert_importe(saldo_cuenta_id(cuenta), 89_980.0, "10 000 + 20 de retención");
}

#[test]
fn c62_sin_tarifa_declarada_un_pago_de_impuestos_no_paga_comision() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100_000.0);

    crear_gasto(transferencia(10_000.0, "Impuestos", "Pago DGII", cuenta)).unwrap();

    assert_importe(saldo_cuenta_id(cuenta), 90_000.0, "exento y sin tarifa pactada");
}

#[test]
fn c63_la_tarifa_es_fija_y_no_depende_del_monto() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Corriente DOP", "DOP", 1_000_000.0);
    declarar_comision_de_impuestos(cuenta, 75.0);

    crear_gasto(transferencia(500_000.0, "Impuestos", "Pago DGII", cuenta)).unwrap();

    // Con la retención ordinaria habrían salido 1 000 pesos en vez de 75.
    assert_importe(saldo_cuenta_id(cuenta), 499_925.0, "misma tarifa que en un pago pequeño");
}

// ---------------------------------------------------------------------------
//  Reversión de un abono a tarjeta
//
//  Recorre el camino entero porque es donde vive el riesgo: el caso de uso ya
//  está probado con el doble, pero lo que hacía imposible revertir era que el
//  registro no guardara de qué cuenta salió el dinero. Estas pruebas verifican
//  que ahora lo guarda y que deshacerlo devuelve todo a su sitio.
// ---------------------------------------------------------------------------

#[test]
fn c64_registrar_y_revertir_un_abono_deja_tarjeta_y_cuenta_como_estaban() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(30_000.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100_000.0);

    registrar_pago_tarjeta(
        tarjeta, "14/09/2026".to_string(), 12_000.0, "DOP".to_string(), Some(cuenta), 0.0,
    )
    .unwrap();
    assert_importe(balances_tarjeta(tarjeta).0, 18_000.0, "la deuda bajó");
    assert_importe(saldo_cuenta_id(cuenta), 87_976.0, "salieron 12 000 + 24");

    let abono = ultimo_abono();
    revertir_abono_tarjeta(abono).unwrap();

    assert_importe(balances_tarjeta(tarjeta).0, 30_000.0, "la deuda vuelve");
    assert_importe(saldo_cuenta_id(cuenta), 100_000.0, "y el dinero también");
    assert_eq!(total_gastos(), 0, "la comisión deja de existir");
}

#[test]
fn c65_revertir_un_abono_en_divisa_devuelve_los_pesos_que_salieron() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 500.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100_000.0);

    registrar_pago_tarjeta(
        tarjeta, "14/09/2026".to_string(), 100.0, "USD".to_string(), Some(cuenta), 60.0,
    )
    .unwrap();
    assert_importe(balances_tarjeta(tarjeta).1, 400.0, "la deuda en dólares bajó");
    assert_importe(saldo_cuenta_id(cuenta), 93_988.0, "salieron 6 000 + 12");

    revertir_abono_tarjeta(ultimo_abono()).unwrap();

    assert_importe(balances_tarjeta(tarjeta).1, 500.0, "vuelve en dólares");
    assert_importe(saldo_cuenta_id(cuenta), 100_000.0, "y a la cuenta vuelven pesos");
}

#[test]
fn c66_revertir_un_abono_sin_cuenta_solo_repone_la_deuda() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(30_000.0, 0.0);

    registrar_pago_tarjeta(
        tarjeta, "14/09/2026".to_string(), 12_000.0, "DOP".to_string(), None, 0.0,
    )
    .unwrap();

    revertir_abono_tarjeta(ultimo_abono()).unwrap();

    assert_importe(balances_tarjeta(tarjeta).0, 30_000.0, "la deuda vuelve entera");
    assert_eq!(total_gastos(), 0, "nunca hubo comisión que borrar");
}

#[test]
fn c67_revertir_un_abono_que_dejo_saldo_a_favor_lo_deshace_sin_recorte() {
    // El caso que motivó todo esto: un abono mayor que la deuda. Con el
    // recorte de antes el dinero salía de la cuenta y no llegaba a la tarjeta,
    // y no había forma de deshacerlo.
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 500_000.0);

    registrar_pago_tarjeta(
        tarjeta, "14/09/2026".to_string(), 5_000.0, "USD".to_string(), Some(cuenta), 59.9,
    )
    .unwrap();
    assert_importe(balances_tarjeta(tarjeta).1, -5_000.0, "queda saldo a favor, no cero");

    revertir_abono_tarjeta(ultimo_abono()).unwrap();

    assert_importe(balances_tarjeta(tarjeta).1, 0.0, "el saldo a favor se deshace");
    assert_importe(saldo_cuenta_id(cuenta), 500_000.0, "y el dinero vuelve entero");
}

#[test]
fn c68_revertir_dos_veces_falla_la_segunda_sin_duplicar_la_devolucion() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(30_000.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100_000.0);
    registrar_pago_tarjeta(
        tarjeta, "14/09/2026".to_string(), 12_000.0, "DOP".to_string(), Some(cuenta), 0.0,
    )
    .unwrap();
    let abono = ultimo_abono();

    revertir_abono_tarjeta(abono).unwrap();
    assert!(revertir_abono_tarjeta(abono).is_err(), "el abono ya no existe");

    assert_importe(saldo_cuenta_id(cuenta), 100_000.0, "sin doble devolución");
}

// ===========================================================================
//  FASE 4.1 — Caracterización del vertical de Ingresos
//
//  Fija la conducta vigente antes de extraer nada. Cinco de estas pruebas
//  documentan defectos, no aciertos: se escriben para que la extracción no
//  los corrija por accidente y para que corregirlos sea una decisión visible.
//
//  Tres de los cinco son **reapariciones** de defectos ya resueltos en otros
//  verticales —el redondeo a unidades de H8, la cuenta localizada por su
//  nombre de H3, el recorte a cero de H5 y H10—. Que el mismo error viva en
//  cuatro sitios distintos es, en sí, el hallazgo más informativo: no había
//  nada compartido que impidiera repetirlo.
// ===========================================================================

fn factura(numero: &str, monto: f64, retencion: f64) -> IngresoInput {
    IngresoInput {
        numero_factura: numero.to_string(),
        rnc_cliente: "000000000".to_string(),
        nombre_cliente: "Cliente Ejemplo".to_string(),
        fecha_emision: "16/09/2026".to_string(),
        monto_total: monto,
        porcentaje_retencion: retencion,
    }
}

fn retencion_de(id: i64) -> f64 {
    conexion()
        .query_row("SELECT monto_retenido FROM ingresos WHERE id = ?;", params![id], |r| r.get(0))
        .expect("leer retención")
}

fn estatus_de(id: i64) -> String {
    conexion()
        .query_row("SELECT estatus FROM ingresos WHERE id = ?;", params![id], |r| r.get(0))
        .expect("leer estatus")
}

// --- Emisión y retención ---

#[test]
fn c70_la_factura_calcula_su_retencion_al_emitirse() {
    let _g = entorno_aislado();
    let id = crear_ingreso(factura("A-001", 10_000.0, 15.0)).unwrap();

    assert_importe(retencion_de(id), 1_500.0, "15 % de 10 000");
    assert_eq!(estatus_de(id), "emitida");
}

#[test]
fn c71_h16_resuelto_la_retencion_se_decide_al_centimo() {
    // **H16 resuelto.** Antes: `.round()` sobre pesos, que dejaba el 15 % de
    // 1 234.56 —185.184— en 185.00 y perdía céntimos en cada factura, siempre
    // en la misma dirección.
    //
    // Ahora usa el mismo núcleo que el resto del sistema, que es la razón de
    // haberlo extraído: una regla que existe dos veces se corrige una vez y
    // sigue mal en la otra. Es lo que había pasado con H8.
    let _g = entorno_aislado();
    let id = crear_ingreso(factura("A-002", 1_234.56, 15.0)).unwrap();

    assert_importe(retencion_de(id), 185.18, "al céntimo, como el 0.20 %");
}

#[test]
fn c72_el_numero_de_factura_no_se_puede_repetir_ni_cambiando_mayusculas() {
    let _g = entorno_aislado();
    crear_ingreso(factura("A-003", 1_000.0, 15.0)).unwrap();

    assert!(crear_ingreso(factura("a-003", 2_000.0, 15.0)).is_err(), "compara sin distinguir");
}

#[test]
fn c73_el_cliente_se_crea_una_sola_vez_por_rnc() {
    let _g = entorno_aislado();
    crear_ingreso(factura("A-004", 1_000.0, 15.0)).unwrap();
    crear_ingreso(factura("A-005", 2_000.0, 15.0)).unwrap();

    let clientes: i64 = conexion()
        .query_row("SELECT COUNT(*) FROM clientes;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(clientes, 1, "el segundo reutiliza el cliente del primero");
}

#[test]
fn c74_corregir_una_factura_usa_la_misma_regla_que_al_crearla() {
    let _g = entorno_aislado();
    let id = crear_ingreso(factura("A-006", 1_000.0, 15.0)).unwrap();

    actualizar_ingreso(id, "A-006".into(), 1, "16/09/2026".into(), 1_234.56, 15.0, None).unwrap();

    assert_importe(retencion_de(id), 185.18, "crear y corregir no divergen");
}

// --- Cobro ---

#[test]
fn c75_cobrar_una_factura_acredita_la_cuenta_indicada() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso(factura("A-007", 10_000.0, 15.0)).unwrap();

    marcar_ingreso_pagado(id, cuenta, "16/09/2026".into(), 8_500.0).unwrap();

    assert_eq!(estatus_de(id), "pagada");
    assert_importe(saldo_cuenta_id(cuenta), 9_500.0, "entra el neto recibido");
}

#[test]
fn c76_h17_resuelto_cobrar_a_una_cuenta_inexistente_falla() {
    // **H17 resuelto.** Antes la cuenta se localizaba por su nombre y el
    // resultado se descartaba con `let _ =`: la factura quedaba cobrada y
    // ningún saldo se movía. Ahora se referencia por identificador y su
    // ausencia es un error, como en H3 con la caja de efectivo.
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso(factura("A-008", 10_000.0, 15.0)).unwrap();

    let r = marcar_ingreso_pagado(id, 9_999, "16/09/2026".into(), 8_500.0);

    assert!(r.is_err(), "no se cobra contra una cuenta que no existe");
    assert_eq!(estatus_de(id), "emitida", "la factura sigue pendiente");
    assert_importe(saldo_cuenta_id(cuenta), 1_000.0, "ningún saldo se movió");
}

#[test]
fn c77_h18_resuelto_cobrar_una_factura_inexistente_falla() {
    // **H18 resuelto.** El `UPDATE` afectaba a cero filas y devolvía `Ok`:
    // el sistema no distinguía entre haber cobrado y no haber encontrado
    // nada que cobrar.
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);

    let r = marcar_ingreso_pagado(404, cuenta, "16/09/2026".into(), 100.0);

    assert!(r.is_err(), "ahora lo dice");
    assert_importe(saldo_cuenta_id(cuenta), 1_000.0, "y no acredita nada");
}

#[test]
fn c78_h19_resuelto_el_importe_se_acredita_en_la_divisa_de_su_cuenta() {
    // **H19 resuelto.** `ingresos` sigue sin columna de divisa —el importe es
    // implícitamente local—, pero el tipo `Deposito` exige que coincida con
    // la de la cuenta. El caso de sumar pesos a un saldo en dólares ya no se
    // puede construir. Es el camino por el que se cerró H2.
    //
    // Que el importe entre en una cuenta USD es correcto **si se interpreta
    // como dólares**: lo que no puede es entrar como pesos.
    let _g = entorno_aislado();
    let cuenta_usd = crear_cuenta("Cuenta Ahorros USD", "USD", 100.0);
    let id = crear_ingreso(factura("A-009", 10_000.0, 15.0)).unwrap();

    marcar_ingreso_pagado(id, cuenta_usd, "16/09/2026".into(), 50.0).unwrap();

    assert_importe(saldo_cuenta_id(cuenta_usd), 150.0, "50 dólares, no 50 pesos");
}

// --- Borrado ---

#[test]
fn c79_borrar_una_factura_cobrada_revierte_el_abono() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso(factura("A-010", 10_000.0, 15.0)).unwrap();
    marcar_ingreso_pagado(id, cuenta, "16/09/2026".into(), 8_500.0).unwrap();

    eliminar_ingreso(id).unwrap();

    assert_importe(saldo_cuenta_id(cuenta), 1_000.0, "el saldo vuelve donde estaba");
}

#[test]
fn c80_h20_resuelto_borrar_una_factura_no_recorta_el_saldo() {
    // **H20 resuelto, con las mismas condiciones que H5 y H10.** Antes el
    // `MAX(0.0, ...)` dejaba la cuenta en cero y la diferencia desaparecía
    // sin registro. Ahora el saldo queda negativo, que es el estado
    // verdadero: el dinero se cobró y se gastó.
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 0.0);
    let id = crear_ingreso(factura("A-011", 10_000.0, 15.0)).unwrap();
    marcar_ingreso_pagado(id, cuenta, "16/09/2026".into(), 8_500.0).unwrap();
    // El titular gasta lo cobrado antes de advertir el error de registro.
    conexion()
        .execute("UPDATE cuentas_ahorro SET balance_actual = 500.0 WHERE id = ?;", params![cuenta])
        .unwrap();

    eliminar_ingreso(id).unwrap();

    // 500 - 8 500 = -8 000. Lo que falta por reponer, dicho en vez de
    // tragado.
    assert_importe(saldo_cuenta_id(cuenta), -8_000.0, "sin recorte");
}

// --- Ingresos informales ---

#[test]
fn c81_un_cobro_informal_en_efectivo_entra_en_la_caja_de_su_divisa() {
    // La caja de efectivo la siembra `inicializar_db`; crearla aquí violaría
    // la unicidad del nombre, así que se parte de la que ya existe.
    let _g = entorno_aislado();
    let antes = balance_cuenta("Efectivo DOP");

    crear_cobro_efectivo_informal("16/09/2026".into(), "Trabajo suelto".into(), 2_000.0, "DOP".into())
        .unwrap();

    assert_importe(balance_cuenta("Efectivo DOP"), antes + 2_000.0, "la caja recibe el importe");
}

#[test]
fn c82_h17_el_informal_comparte_el_arreglo() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso_informal("16/09/2026".into(), "Trabajo suelto".into(), 2_000.0).unwrap();

    let r = marcar_informal_pagado(id, 9_999, "16/09/2026".into(), 2_000.0);

    assert!(r.is_err(), "el informal falla igual que la factura");
    assert_importe(saldo_cuenta_id(cuenta), 1_000.0, "ningún saldo se movió");
}

#[test]
fn c83_borrar_un_informal_cobrado_revierte_su_abono() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso_informal("16/09/2026".into(), "Trabajo suelto".into(), 2_000.0).unwrap();
    marcar_informal_pagado(id, cuenta, "16/09/2026".into(), 2_000.0).unwrap();

    eliminar_ingreso_informal(id).unwrap();

    assert_importe(saldo_cuenta_id(cuenta), 1_000.0, "el saldo vuelve donde estaba");
}

#[test]
fn c84_un_informal_sin_cobrar_no_mueve_ningun_saldo_al_borrarse() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso_informal("16/09/2026".into(), "Trabajo suelto".into(), 2_000.0).unwrap();

    eliminar_ingreso_informal(id).unwrap();

    assert_importe(saldo_cuenta_id(cuenta), 1_000.0, "nunca entró, nada sale");
}

// ---------------------------------------------------------------------------
//  Corregir una factura ya cobrada
//
//  Antes no se podía: la interfaz solo ofrecía el botón de corregir mientras
//  la factura estuviera emitida. Una vez cobrada, un error de importe quedaba
//  congelado.
//
//  El problema de fondo es que corregirla no es reescribir cifras: hay dinero
//  en una cuenta que dependía de ellas.
// ---------------------------------------------------------------------------

fn recibido_de(id: i64) -> f64 {
    conexion()
        .query_row("SELECT monto_recibido FROM ingresos WHERE id = ?;", params![id], |r| r.get(0))
        .expect("leer recibido")
}

#[test]
fn c85_corregir_al_alza_una_factura_cobrada_acredita_la_diferencia() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso(factura("B-001", 10_000.0, 15.0)).unwrap();
    marcar_ingreso_pagado(id, cuenta, "20/09/2026".into(), 8_500.0).unwrap();
    assert_importe(saldo_cuenta_id(cuenta), 9_500.0, "entró el neto");

    // Eran 12 000, no 10 000. El neto sube de 8 500 a 10 200.
    actualizar_ingreso(id, "B-001".into(), 1, "20/09/2026".into(), 12_000.0, 15.0, None).unwrap();

    assert_importe(retencion_de(id), 1_800.0, "la retención se recalcula");
    assert_importe(recibido_de(id), 10_200.0, "y lo recibido también");
    assert_importe(saldo_cuenta_id(cuenta), 11_200.0, "la cuenta recibe los 1 700 que faltaban");
}

#[test]
fn c86_corregir_a_la_baja_retira_de_la_cuenta_lo_que_sobraba() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso(factura("B-002", 10_000.0, 15.0)).unwrap();
    marcar_ingreso_pagado(id, cuenta, "20/09/2026".into(), 8_500.0).unwrap();

    actualizar_ingreso(id, "B-002".into(), 1, "20/09/2026".into(), 8_000.0, 15.0, None).unwrap();

    assert_importe(saldo_cuenta_id(cuenta), 7_800.0, "se retiran los 1 700 de más");
    assert_importe(recibido_de(id), 6_800.0, "lo recibido baja con el neto");
}

#[test]
fn c87_corregir_da_por_cobrado_el_neto_entero_aunque_faltara_algo() {
    // **La regla.** Si al corregir no se dice otra cosa, la factura pasa a
    // estar cobrada por su neto nuevo: corregir el monto es normalmente
    // decir «este era el importe, y se cobró».
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso(factura("B-003", 10_000.0, 15.0)).unwrap();
    // Neto de 8 500, pero solo entraron 8 000.
    marcar_ingreso_pagado(id, cuenta, "20/09/2026".into(), 8_000.0).unwrap();

    actualizar_ingreso(id, "B-003".into(), 1, "20/09/2026".into(), 12_000.0, 15.0, None).unwrap();

    assert_importe(recibido_de(id), 10_200.0, "el neto nuevo, entero");
    assert_importe(saldo_cuenta_id(cuenta), 11_200.0, "la cuenta sube los 2 200 que faltaban");
}

#[test]
fn c87b_un_cobro_parcial_declarado_conserva_lo_que_falta() {
    // **La excepción, afirmada.** El neto es 10 200 y solo entraron 9 000:
    // quedan 1 200 por cobrar, y la factura lo dice en lugar de darlo por
    // saldado.
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso(factura("B-006", 10_000.0, 15.0)).unwrap();
    marcar_ingreso_pagado(id, cuenta, "20/09/2026".into(), 8_500.0).unwrap();

    actualizar_ingreso(id, "B-006".into(), 1, "20/09/2026".into(), 12_000.0, 15.0, Some(9_000.0))
        .unwrap();

    assert_importe(recibido_de(id), 9_000.0, "lo que de verdad entró");
    assert_importe(saldo_cuenta_id(cuenta), 10_000.0, "la cuenta sigue a lo recibido");
    assert_importe(12_000.0 - retencion_de(id) - recibido_de(id), 1_200.0, "queda por cobrar");
}

#[test]
fn c87c_un_cobro_parcial_mayor_que_el_neto_se_rechaza() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso(factura("B-007", 10_000.0, 15.0)).unwrap();
    marcar_ingreso_pagado(id, cuenta, "20/09/2026".into(), 8_500.0).unwrap();

    let r = actualizar_ingreso(
        id, "B-007".into(), 1, "20/09/2026".into(), 10_000.0, 15.0, Some(9_000.0),
    );

    assert!(r.is_err(), "cobrar más que el neto no es un cobro parcial");
}

#[test]
fn c88_corregir_sin_cambiar_importes_no_mueve_ningun_saldo() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso(factura("B-004", 10_000.0, 15.0)).unwrap();
    marcar_ingreso_pagado(id, cuenta, "20/09/2026".into(), 8_500.0).unwrap();

    // Solo cambia la fecha.
    actualizar_ingreso(id, "B-004".into(), 1, "21/09/2026".into(), 10_000.0, 15.0, None).unwrap();

    assert_importe(saldo_cuenta_id(cuenta), 9_500.0, "corregir la fecha no toca la cuenta");
}

#[test]
fn c89_corregir_una_factura_sin_cobrar_no_toca_ninguna_cuenta() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso(factura("B-005", 10_000.0, 15.0)).unwrap();

    actualizar_ingreso(id, "B-005".into(), 1, "20/09/2026".into(), 12_000.0, 15.0, None).unwrap();

    assert_importe(retencion_de(id), 1_800.0, "las cifras sí cambian");
    assert_importe(saldo_cuenta_id(cuenta), 1_000.0, "pero no hay dinero que ajustar");
}

#[test]
fn c90_corregir_una_factura_inexistente_falla_en_vez_de_callar() {
    let _g = entorno_aislado();
    let r = actualizar_ingreso(404, "X".into(), 1, "20/09/2026".into(), 100.0, 15.0, None);

    assert!(r.is_err(), "no se corrige lo que no existe");
}
