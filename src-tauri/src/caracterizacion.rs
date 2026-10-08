//! Pruebas de caracterización del vertical de Gastos — Fase 1, paso 1.1.
//!
//! Capturan el comportamiento ACTUAL de `crear_gasto` y `eliminar_gasto`,
//! incluidas sus rarezas, para que cualquier cambio durante la extracción al
//! dominio se manifieste como una prueba en rojo. No juzgan si el
//! comportamiento es correcto: lo fijan.

use crate::db_sql;
use crate::{crear_gasto, crear_suscripcion, eliminar_cuenta, eliminar_gasto, registrar_pago_tarjeta, revertir_abono_tarjeta, GastoInput, crear_ingreso, marcar_ingreso_pagado, eliminar_ingreso, actualizar_ingreso, crear_ingreso_informal, marcar_informal_pagado, eliminar_ingreso_informal, crear_cobro_efectivo_informal, IngresoInput};
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

/// Raíz temporal **propia de este proceso**.
///
/// El identificador de proceso va en la ruta porque el mutex que serializa
/// estas pruebas solo alcanza al proceso que lo ejecuta: `HOME` es estado
/// global del proceso, no de la máquina. Con una ruta fija, dos ejecuciones
/// simultáneas de `cargo test` —cosa que ocurre en cuanto una herramienta lo
/// lanza mientras hay otra corriendo— se pisaban la misma base y fallaban con
/// «attempt to write a readonly database», que no dice en absoluto lo que
/// pasa.
fn raiz_temporal() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("michelitos-caracterizacion-{}", std::process::id()))
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
/// Un motivo válido para las pruebas que borran un movimiento.
///
/// Las correcciones exigen una explicación de cierta longitud, así que las
/// pruebas la proporcionan igual que lo haría el titular.
/// Un importe tal como llegaría del IPC: por sus dígitos.
fn importe(texto: &str) -> crate::ipc::ImporteDecimal {
    crate::ipc::ImporteDecimal::desde_texto(texto).expect("importe de prueba")
}

/// Un importe en pesos a partir de un literal numérico de prueba, ya como el
/// texto que la interfaz mandaría.
fn monto(unidades: f64) -> crate::ipc::ImporteDecimal {
    importe(&format!("{:.2}", unidades))
}

fn motivo_de_prueba() -> String {
    "Corrección de prueba automatizada del sistema".to_string()
}

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

/// Lee de una tarjeta la columna REAL indicada.
fn columna_tarjeta(id: i64, columna: &str) -> f64 {
    conexion()
        .query_row(&format!("SELECT {columna} FROM tarjetas WHERE id = ?;"), params![id], |r| r.get(0))
        .unwrap()
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
/// El último gasto con su fecha: importe, divisa y fecha.
fn ultimo_gasto_con_fecha() -> (f64, String, String) {
    conexion()
        .query_row(
            "SELECT monto, divisa, fecha FROM gastos ORDER BY id DESC LIMIT 1;",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .expect("leer último gasto")
}

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

fn transferencia(importe_gasto: f64, categoria: &str, descripcion: &str, cuenta_id: i64) -> GastoInput {
    GastoInput {
        fecha: "08/09/2026".to_string(),
        monto: monto(importe_gasto),
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
        monto: monto(75.0),
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
        monto: monto(1200.0),
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
        monto: monto(1200.0),
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
        monto: monto(1200.0),
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
        monto: monto(900.0),
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
        monto: monto(150.0),
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

    eliminar_gasto(gasto, motivo_de_prueba()).unwrap();

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

    eliminar_gasto(gasto, motivo_de_prueba()).unwrap();

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
        monto: monto(300.0),
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
fn c16b_el_abono_decide_el_centimo_con_los_digitos_escritos() {
    // `1000.005` sube a 1000.01: la deuda baja exactamente ese céntimo, y sin cuenta de ahorro no hay comisión.
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(50000.0, 0.0);

    registrar_pago_tarjeta(tarjeta, "08/09/2026".to_string(), importe("1000.005"), "DOP".to_string(), None, 0.0).unwrap();

    assert_importe(balances_tarjeta(tarjeta).0, 50000.0 - 1000.01, "la deuda baja el céntimo decidido con los dígitos");
}

#[test]
fn c16c_un_abono_cero_o_negativo_se_rechaza_sin_mover_nada() {
    // **H resuelto.** Antes la regla «el abono debe ser mayor que cero» vivía solo en la interfaz: por el IPC
    // se podía registrar un abono de 0.00 o uno negativo, que subía la deuda. Ahora la impone el núcleo.
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(50000.0, 0.0);

    for texto in ["0.00", "0.004", "-100.00"] {
        let r = registrar_pago_tarjeta(tarjeta, "08/09/2026".to_string(), importe(texto), "DOP".to_string(), None, 0.0);
        assert!(r.is_err(), "aceptó {texto}");
        assert_importe(balances_tarjeta(tarjeta).0, 50000.0, "la deuda no se movió");
    }
}

#[test]
fn c17_el_abono_en_igual_divisa_cobra_la_comision_sobre_el_monto() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(50000.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100000.0);

    // 12 345.67 x 0.20 % = 24.69134, un importe con cinco decimales.
    registrar_pago_tarjeta(
        tarjeta,
        "08/09/2026".to_string(),
        monto(12345.67),
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
        monto(250.0),
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
        monto(10000.0),
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
        monto(800.0),
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
//  Suscripciones — la fecha manda
// =====================================================================
//
// La red anterior giraba en torno a «¿es un mes nuevo y llegó el día?». Esa
// pregunta daba a cada período una ventana para ser cobrado, y perderla
// borraba el período. Ahora la suscripción guarda la fecha de su próximo
// cobro, y una fecha que ya pasó sigue pasada.

fn procesar_en(anio: i32, mes: u32, dia: u32) -> Vec<String> {
    let reloj = crate::puertos::reloj::RelojFijo::en(anio, mes, dia);
    crate::procesar_suscripciones_con(&reloj).expect("procesar suscripciones")
}

/// Abre la aplicación todos los días del año y cuenta los cargos.
fn cargos_en_el_ano(anio: i32) -> usize {
    let mut n = 0;
    for mes in 1..=12u32 {
        for dia in 1..=crate::dominio::suscripcion::dias_del_mes(anio, mes) {
            n += procesar_en(anio, mes, dia).len();
        }
    }
    n
}

fn proximo_cobro_de(sub_id: i64) -> Option<String> {
    conexion()
        .query_row("SELECT fecha_proximo_cobro FROM suscripciones WHERE id = ?;", [sub_id], |r| r.get(0))
        .expect("leer fecha del próximo cobro")
}

fn ultimo_pago(sub_id: i64) -> Option<String> {
    conexion()
        .query_row("SELECT fecha_ultimo_pago FROM suscripciones WHERE id = ?;", [sub_id], |r| r.get(0))
        .expect("leer fecha de último pago")
}

fn pendientes_de(sub_id: i64, anio: i32, mes: u32, dia: u32) -> Vec<String> {
    let reloj = crate::puertos::reloj::RelojFijo::en(anio, mes, dia);
    crate::suscripciones_con_aviso(&reloj)
        .expect("leer suscripciones")
        .into_iter()
        .find(|s| s.id_para_pruebas() == sub_id)
        .expect("la suscripción")
        .pendientes_para_pruebas()
}

fn avisa_en(anio: i32, mes: u32, dia: u32) -> bool {
    let reloj = crate::puertos::reloj::RelojFijo::en(anio, mes, dia);
    crate::suscripciones_con_aviso(&reloj).expect("leer").iter().any(|s| s.avisa_para_pruebas())
}

/// Una mensual con su fecha ya puesta.
fn suscripcion_mensual(dia: u32, proximo: &str) -> (i64, i64) {
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let sub = crear_suscripcion(
        "Plataforma".into(), monto(500.0), tarjeta, "mensual".into(), dia as i32, "DOP".into(),
        Some(proximo.into()),
    )
    .unwrap();
    (sub, tarjeta)
}

// --- Lo que ya se garantizaba, con el modelo nuevo ---

#[test]
fn s1_una_suscripcion_se_cobra_al_llegar_su_fecha() {
    let _g = entorno_aislado();
    let (sub, tarjeta) = suscripcion_mensual(15, "15/03/2026");

    assert!(procesar_en(2026, 3, 14).is_empty(), "la víspera no");
    let mensajes = procesar_en(2026, 3, 15);

    assert_eq!(mensajes.len(), 1);
    assert_importe(balances_tarjeta(tarjeta).0, 500.0, "la deuda sube");
    assert_eq!(total_gastos(), 1);
    assert_eq!(proximo_cobro_de(sub), Some("15/04/2026".into()), "el puntero avanza");
}

#[test]
fn s2_no_se_cobra_dos_veces_el_mismo_periodo() {
    let _g = entorno_aislado();
    let (_, tarjeta) = suscripcion_mensual(15, "15/03/2026");

    procesar_en(2026, 3, 15);
    procesar_en(2026, 3, 16);
    procesar_en(2026, 4, 1);

    assert_importe(balances_tarjeta(tarjeta).0, 500.0, "un solo cargo");
    assert_eq!(total_gastos(), 1);
}

#[test]
fn s3_procesar_dos_veces_seguidas_no_duplica_el_cargo() {
    // La garantía que sostiene que la app procese suscripciones en cada
    // arranque.
    let _g = entorno_aislado();
    let (_, tarjeta) = suscripcion_mensual(15, "15/03/2026");

    procesar_en(2026, 3, 20);
    let segunda = procesar_en(2026, 3, 20);

    assert!(segunda.is_empty());
    assert_importe(balances_tarjeta(tarjeta).0, 500.0, "un solo cargo");
}

#[test]
fn s4_una_anual_no_vuelve_a_cobrarse_dentro_del_ano() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    crear_suscripcion("Anual".into(), monto(3_600.0), tarjeta, "anual".into(), 5, "DOP".into(),
                      Some("05/07/2026".into())).unwrap();

    procesar_en(2026, 7, 5);
    procesar_en(2026, 12, 31);

    assert_importe(balances_tarjeta(tarjeta).0, 3_600.0, "un solo cargo en el año");
}

#[test]
fn s5_el_cargo_en_dolares_solo_mueve_el_balance_en_dolares() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(1000.0, 50.0);
    crear_suscripcion("Plataforma".into(), monto(15.0), tarjeta, "mensual".into(), 1, "USD".into(),
                      Some("01/03/2026".into())).unwrap();

    procesar_en(2026, 3, 1);

    let (pesos, dolares) = balances_tarjeta(tarjeta);
    assert_importe(dolares, 65.0, "sube la deuda en dólares");
    assert_importe(pesos, 1000.0, "la deuda en pesos no se toca");
}

#[test]
fn s7_editar_conserva_el_puntero_y_no_recobra() {
    // Antes la única forma de cambiar una suscripción era borrarla y
    // recrearla, y eso reiniciaba la idempotencia. El comando de edición
    // existe para evitar ese cobro duplicado.
    let _g = entorno_aislado();
    let (sub, tarjeta) = suscripcion_mensual(15, "15/03/2026");
    procesar_en(2026, 3, 15);
    let puntero = proximo_cobro_de(sub);

    crate::actualizar_suscripcion(sub, "Otro nombre".into(), monto(600.0), tarjeta, "mensual".into(), 15,
                                  "DOP".into(), puntero.clone()).unwrap();

    assert_eq!(proximo_cobro_de(sub), puntero, "el puntero se conserva");
    procesar_en(2026, 3, 20);
    assert_importe(balances_tarjeta(tarjeta).0, 500.0, "no vuelve a cobrar este período");
}

#[test]
fn s8_editar_no_altera_los_cargos_ya_realizados() {
    let _g = entorno_aislado();
    let (sub, tarjeta) = suscripcion_mensual(15, "15/03/2026");
    procesar_en(2026, 3, 15);

    crate::actualizar_suscripcion(sub, "Otro".into(), monto(999.0), tarjeta, "anual".into(), 20,
                                  "USD".into(), Some("20/01/2027".into())).unwrap();

    let (monto, _, _) = ultimo_gasto();
    assert_importe(monto, 500.0, "el gasto ya registrado mantiene su importe");
    assert_importe(balances_tarjeta(tarjeta).0, 500.0, "y la deuda tampoco cambia");
}

#[test]
fn s9_editar_una_suscripcion_inexistente_es_error() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    assert!(crate::actualizar_suscripcion(9999, "X".into(), monto(1.0), tarjeta, "mensual".into(), 1,
                                          "DOP".into(), None).is_err());
}

// --- La ventana que ya no existe ---

#[test]
fn s13b_un_periodo_no_se_pierde_por_abrir_la_aplicacion_tarde() {
    // **CAMBIO DE CONDUCTA — el defecto que costó dinero de verdad.**
    //
    // Antes, cada período tenía una ventana —de su día de facturación al fin
    // de mes— y perderla lo borraba: al llegar el mes siguiente, la marca
    // pasaba a leerse como «ya atendido». Para una del día 30 la ventana era
    // de **un día**, y así se perdió el cargo de Netflix de agosto de 2026.
    //
    // Ahora una fecha que pasó sigue pasada.
    let _g = entorno_aislado();
    let (_, tarjeta) = suscripcion_mensual(30, "30/08/2026");

    // No se abre ni el 30 ni el 31 de agosto. Once días tarde:
    let mensajes = procesar_en(2026, 9, 10);

    assert_eq!(mensajes.len(), 1, "el período de agosto sigue ahí");
    assert_importe(balances_tarjeta(tarjeta).0, 500.0, "el cargo entra");
    let (_, _, fecha) = ultimo_gasto_con_fecha();
    assert_eq!(fecha, "30/08/2026", "y se asienta en su fecha, no en la de hoy");
}

#[test]
fn s13c_dos_periodos_vencidos_no_se_cobran_solos() {
    // **El umbral de la recomendación B.** Con uno no hay ambigüedad. Con
    // varios, la aplicación no sabe si el proveedor los cobró ni si la
    // suscripción siguió activa, y fabricar cargos que quizá no ocurrieron es
    // peor que señalarlos.
    let _g = entorno_aislado();
    let (sub, tarjeta) = suscripcion_mensual(30, "30/07/2026");

    let mensajes = procesar_en(2026, 9, 10);

    assert!(mensajes.is_empty(), "no se cobra ninguno sin confirmar");
    assert_importe(balances_tarjeta(tarjeta).0, 0.0, "ninguno de los dos");
    assert_eq!(pendientes_de(sub, 2026, 9, 10), vec!["30/07/2026", "30/08/2026"]);
}

#[test]
fn s13d_confirmar_un_periodo_lo_asienta_en_su_fecha_y_avanza() {
    let _g = entorno_aislado();
    let (sub, tarjeta) = suscripcion_mensual(30, "30/07/2026");
    let reloj = crate::puertos::reloj::RelojFijo::en(2026, 9, 10);

    let resumen = crate::confirmar_pendiente(sub, &reloj, None).unwrap();

    assert!(resumen.contains("30/07/2026"), "resumen obtenido: {resumen}");
    assert_importe(balances_tarjeta(tarjeta).0, 500.0, "el cargo entra");
    let (_, _, fecha) = ultimo_gasto_con_fecha();
    assert_eq!(fecha, "30/07/2026", "en la fecha del período, no en la de hoy");
    assert_eq!(proximo_cobro_de(sub), Some("30/08/2026".into()));

    // Y al quedar uno solo, el cobro automático vuelve a encargarse.
    assert_eq!(procesar_en(2026, 9, 10).len(), 1);
    assert_importe(balances_tarjeta(tarjeta).0, 1_000.0, "y luego el otro");
}

#[test]
fn s13e_descartar_un_periodo_avanza_sin_cobrar_y_deja_caso() {
    // Descartar es afirmar que el proveedor no lo cobró. Esa afirmación se
    // hace mirando un estado de cuenta, y queda por escrito: si dentro de
    // seis meses la cifra anual no cuadra, esto dirá por qué.
    let _g = entorno_aislado();
    let (sub, tarjeta) = suscripcion_mensual(30, "30/07/2026");
    let reloj = crate::puertos::reloj::RelojFijo::en(2026, 9, 10);

    let resumen = crate::confirmar_pendiente(
        sub, &reloj, Some("El proveedor no cobró ese mes, según el estado".into())).unwrap();

    assert!(resumen.contains("COR-"), "no devolvió el número de caso: {resumen}");
    assert_importe(balances_tarjeta(tarjeta).0, 0.0, "no se cobra nada");
    assert_eq!(total_gastos(), 0);
    assert_eq!(proximo_cobro_de(sub), Some("30/08/2026".into()), "y aun así avanza");

    let casos: i64 = conexion()
        .query_row("SELECT COUNT(*) FROM correcciones WHERE tipo = 'período de suscripción';", [], |r| r.get(0))
        .unwrap();
    assert_eq!(casos, 1, "queda constancia");
}

#[test]
fn s13f_descartar_sin_explicar_se_rechaza() {
    let _g = entorno_aislado();
    let (sub, _) = suscripcion_mensual(30, "30/07/2026");
    let reloj = crate::puertos::reloj::RelojFijo::en(2026, 9, 10);

    assert!(crate::confirmar_pendiente(sub, &reloj, Some("error".into())).is_err());
}

#[test]
fn s13g_no_hay_nada_que_confirmar_cuando_solo_vence_uno() {
    // Con un período vencido se encarga el cobro automático. Dejar que esta
    // vía lo tocara abriría un segundo camino para el mismo hecho.
    let _g = entorno_aislado();
    let (sub, _) = suscripcion_mensual(15, "15/03/2026");
    let reloj = crate::puertos::reloj::RelojFijo::en(2026, 3, 20);

    assert!(crate::confirmar_pendiente(sub, &reloj, None).is_err());
    assert!(pendientes_de(sub, 2026, 3, 20).is_empty());
}

#[test]
fn s13h_la_lista_de_pendientes_tiene_tope() {
    // Una suscripción abandonada años produciría una lista que nadie va a
    // conciliar uno a uno.
    let _g = entorno_aislado();
    let (sub, _) = suscripcion_mensual(15, "15/01/2020");

    let pendientes = pendientes_de(sub, 2026, 1, 15);

    assert_eq!(pendientes.len(), crate::dominio::suscripcion::MAXIMO_DE_PENDIENTES);
}

// --- El día que no existe en el mes ---

#[test]
fn s10b_una_mensual_del_dia_31_se_cobra_los_doce_meses() {
    let _g = entorno_aislado();
    let (_, tarjeta) = suscripcion_mensual(31, "31/01/2026");

    assert_eq!(cargos_en_el_ano(2026), 12, "doce cargos, los mismos que hace el proveedor");
    assert_importe(balances_tarjeta(tarjeta).0, 6_000.0, "ningún mes sin cargar");
}

#[test]
fn s11b_una_mensual_del_dia_30_ya_no_pierde_febrero() {
    let _g = entorno_aislado();
    let (_, _) = suscripcion_mensual(30, "30/01/2026");
    assert_eq!(cargos_en_el_ano(2026), 12, "doce cargos donde antes había once");
}

#[test]
fn s11c_el_cargo_de_febrero_se_asienta_el_ultimo_dia_del_mes() {
    // **La fecha sale del estado de cuenta.** Un cargo del día 29 se generó
    // el 28 de febrero; que se liquidara el 1 de marzo es otra cosa, y
    // pertenece al ciclo de pago de la tarjeta.
    let _g = entorno_aislado();
    let (sub, _) = suscripcion_mensual(30, "30/01/2026");

    procesar_en(2026, 1, 30);
    assert_eq!(proximo_cobro_de(sub), Some("28/02/2026".into()), "recortado al último día");

    procesar_en(2026, 2, 28);
    let (_, _, fecha) = ultimo_gasto_con_fecha();
    assert_eq!(fecha, "28/02/2026");
}

#[test]
fn s11e_el_ancla_devuelve_la_suscripcion_a_su_dia_tras_un_mes_corto() {
    // Si el siguiente se calculara desde el 28 recortado, la suscripción
    // quedaría anclada al 28 para siempre. Por eso el día de facturación
    // sobrevive como ancla: no es una segunda versión de la fecha, es otro
    // hecho.
    let _g = entorno_aislado();
    let (sub, _) = suscripcion_mensual(30, "28/02/2026");

    procesar_en(2026, 2, 28);

    assert_eq!(proximo_cobro_de(sub), Some("30/03/2026".into()), "vuelve al 30");
}

// --- La anual ---

#[test]
fn s12b_una_anual_espera_a_la_fecha_que_tiene_anotada() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    crear_suscripcion("Anual".into(), monto(3_600.0), tarjeta, "anual".into(), 5, "DOP".into(),
                      Some("05/07/2026".into())).unwrap();

    assert!(procesar_en(2026, 1, 5).is_empty(), "enero no dispara nada");
    assert!(procesar_en(2026, 7, 4).is_empty(), "ni la víspera");
    assert_eq!(procesar_en(2026, 7, 5).len(), 1, "el día anotado sí");
}

#[test]
fn s12c_al_cobrar_una_anual_la_fecha_avanza_un_ano() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let sub = crear_suscripcion("Anual".into(), monto(3_600.0), tarjeta, "anual".into(), 5, "DOP".into(),
                                Some("05/07/2026".into())).unwrap();

    procesar_en(2026, 9, 22); // se abre casi tres meses tarde

    assert_eq!(proximo_cobro_de(sub), Some("05/07/2027".into()),
               "se calcula desde la fecha anotada, no desde hoy");
    let (_, _, fecha) = ultimo_gasto_con_fecha();
    assert_eq!(fecha, "05/07/2026", "y el asiento lleva la del vencimiento");
}

#[test]
fn s12e_una_suscripcion_sin_fecha_no_se_cobra() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    crear_suscripcion("Anual".into(), monto(3_600.0), tarjeta, "anual".into(), 5, "DOP".into(), None).unwrap();

    assert!(procesar_en(2026, 12, 31).is_empty());
    assert_importe(balances_tarjeta(tarjeta).0, 0.0, "sin fecha no hay cargo");
}

#[test]
fn s12f_una_fecha_que_no_se_entiende_se_rechaza_al_guardarla() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);

    for mala in ["2026-07-05", "31/02/2026", "1009/2026"] {
        assert!(
            crear_suscripcion("X".into(), monto(100.0), tarjeta, "anual".into(), 5, "DOP".into(),
                              Some(mala.into())).is_err(),
            "aceptó «{mala}»"
        );
    }
}

#[test]
fn s18_el_aviso_se_enciende_una_semana_antes_del_cobro_anual() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    crear_suscripcion("Anual".into(), monto(3_600.0), tarjeta, "anual".into(), 5, "DOP".into(),
                      Some("05/07/2026".into())).unwrap();

    assert!(!avisa_en(2026, 6, 27), "ocho días antes todavía no");
    assert!(avisa_en(2026, 6, 28), "siete días antes sí");
    assert!(avisa_en(2026, 7, 5), "y el mismo día");
    assert!(!avisa_en(2026, 7, 6), "pasada la fecha ya no es aviso, es cobro vencido");
}

#[test]
fn s19_una_mensual_tambien_avisa_una_semana_antes_de_su_cobro() {
    // Decisión del titular (2026-10-02): el aviso vale para todas, para dar tiempo a cambiar
    // la tarjeta que cobra antes de que caiga el cargo. Antes solo avisaban las anuales.
    let _g = entorno_aislado();
    let (_, _) = suscripcion_mensual(5, "05/07/2026");
    assert!(!avisa_en(2026, 6, 27), "ocho días antes todavía no");
    assert!(avisa_en(2026, 6, 28), "siete días antes sí");
    assert!(avisa_en(2026, 7, 5), "y el mismo día");
    assert!(!avisa_en(2026, 7, 6), "pasada la fecha ya no es aviso, es cobro vencido");
}

// --- La marca del último cobro, que dejó de decidir ---

#[test]
fn s14g_la_marca_del_ultimo_cobro_informa_pero_ya_no_decide() {
    // **Dejó de ser el mecanismo de idempotencia.** Desde que la decisión lee
    // `fecha_proximo_cobro`, `fecha_ultimo_pago` solo dice cuándo se cobró la
    // última vez.
    //
    // Eso deja sin efecto el impedimento por marca ilegible que resolvía
    // `s14`: ya no impide nada, y señalarlo sería un aviso que miente. Lo que
    // se conserva es la restricción de esquema (`s14f`).
    let _g = entorno_aislado();
    let (sub, _) = suscripcion_mensual(15, "15/03/2026");

    assert_eq!(ultimo_pago(sub), None, "todavía no se ha cobrado");
    procesar_en(2026, 3, 15);

    // Y al cobrar queda la fecha **del vencimiento**, no la de ejecución.
    assert_eq!(ultimo_pago(sub), Some("15/03/2026".into()));
    assert!(crate::obtener_suscripciones().unwrap()[0].impedimento_para_pruebas().is_none());
}

#[test]
fn s14h_una_marca_ilegible_ya_no_para_ni_duplica_nada() {
    // La contraprueba del cambio: con la marca rota, el cobro sigue
    // gobernado por la fecha. Ni cobra antes de tiempo —que era el defecto
    // `s14`— ni deja de cobrar cuando toca.
    let _g = entorno_aislado();
    let (sub, _) = suscripcion_mensual(15, "15/03/2026");
    conexion()
        .execute("UPDATE suscripciones SET fecha_ultimo_pago = '31/02/2026' WHERE id = ?;", [sub])
        .expect("sembrar una marca con forma válida que no es un día");

    assert!(procesar_en(2026, 3, 14).is_empty(), "no cobra antes de su fecha");
    assert_eq!(procesar_en(2026, 3, 15).len(), 1, "y cobra cuando toca");
    assert!(crate::obtener_suscripciones().unwrap()[0].impedimento_para_pruebas().is_none(),
            "no se señala un impedimento que ya no impide");
}

#[test]
fn s14c_una_suscripcion_sin_fecha_lo_dice_en_la_lista() {
    // Quedarse parada **en silencio** sería peor que cobrar de más: el cargo
    // indebido aparece en el estado de cuenta, la parada no aparece en
    // ninguna parte.
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    crear_suscripcion("Plataforma".into(), monto(500.0), tarjeta, "mensual".into(), 15, "DOP".into(), None).unwrap();

    assert!(procesar_en(2026, 12, 31).is_empty(), "sin fecha no se cobra");

    let impedimento = crate::obtener_suscripciones().unwrap()[0]
        .impedimento_para_pruebas()
        .map(str::to_string);
    assert!(impedimento.is_some(), "la lista no dice que esté parada");
    assert!(impedimento.unwrap().contains("próximo cobro"));
}

#[test]
fn s14d_poner_la_fecha_devuelve_la_suscripcion_al_ciclo() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let sub = crear_suscripcion("Plataforma".into(), monto(500.0), tarjeta, "mensual".into(), 15, "DOP".into(), None).unwrap();
    assert!(procesar_en(2026, 3, 20).is_empty(), "parada");

    crate::corregir_proximo_cobro(sub, "15/03/2026".into()).unwrap();

    assert_eq!(procesar_en(2026, 3, 20).len(), 1, "vuelve a cobrar");
    assert!(crate::obtener_suscripciones().unwrap()[0].impedimento_para_pruebas().is_none());
}

#[test]
fn s14e_corregir_con_una_fecha_que_no_se_entiende_se_rechaza() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let sub = crear_suscripcion("Plataforma".into(), monto(500.0), tarjeta, "mensual".into(), 15, "DOP".into(), None).unwrap();

    for mala in ["2026-02-15", "15-02-2026", "ayer", "", "31/02/2026", "1009/2026"] {
        assert!(crate::corregir_proximo_cobro(sub, mala.into()).is_err(), "aceptó «{mala}»");
    }
    assert!(crate::corregir_proximo_cobro(9999, "15/02/2026".into()).is_err());
}

#[test]
fn s14f_el_esquema_rechaza_una_fecha_sin_forma_de_fecha() {
    // La capa que se conserva. En la base real hay una fecha rota de verdad
    // en `gastos.fecha`: un `10/09/2026` sin la primera barra.
    let _g = entorno_aislado();
    let (sub, _) = suscripcion_mensual(15, "15/03/2026");

    for columna in ["fecha_ultimo_pago", "fecha_proximo_cobro"] {
        for mala in ["1009/2026", "2026-01-10", "ayer", "1/1/2026"] {
            let r = conexion().execute(
                &format!("UPDATE suscripciones SET {columna} = ? WHERE id = ?;"),
                params![mala, sub],
            );
            assert!(r.is_err(), "el esquema aceptó «{mala}» en {columna}");
        }
    }
}

// --- El cobro pasa por Dinero y por el registro de gastos ---

fn id_del_ultimo_gasto() -> i64 {
    conexion()
        .query_row("SELECT id FROM gastos ORDER BY id DESC LIMIT 1;", [], |r| r.get(0))
        .expect("leer el último gasto")
}

#[test]
fn s20_un_cargo_en_dolares_a_una_tarjeta_que_traduce_queda_pendiente_y_se_puede_liquidar() {
    // **Lo que el cobro directo se saltaba.** Sin la política de la tarjeta, el
    // cargo se asentaba como si no hubiera nada que traducir y nunca figuraba
    // como pendiente: no había forma de registrar lo que el emisor cargó de
    // verdad. Diez de las once suscripciones del titular son en dólares.
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    fijar_politica(tarjeta, "traduce");
    crear_suscripcion("Plataforma".into(), monto(15.0), tarjeta, "mensual".into(), 1, "USD".into(),
                      Some("01/03/2026".into())).unwrap();

    procesar_en(2026, 3, 1);

    let gasto = id_del_ultimo_gasto();
    assert_eq!(estado_conversion(gasto).as_deref(), Some("pendiente"), "queda pendiente de liquidar");
    let (pesos, dolares) = balances_tarjeta(tarjeta);
    assert_importe(dolares, 15.0, "la deuda sube en dólares hasta que el emisor informe");
    assert_importe(pesos, 0.0, "y todavía no existe cifra en pesos");

    // El titular registra lo que el emisor cargó: 15 USD a 60.50 = 907.50.
    crate::liquidar_consumo_pendiente(gasto, monto(907.5)).unwrap();

    assert_eq!(estado_conversion(gasto).as_deref(), Some("liquidado"));
    let (pesos, dolares) = balances_tarjeta(tarjeta);
    assert_importe(dolares, 0.0, "baja de la divisa de origen");
    assert_importe(pesos, 907.5, "y sube en moneda local");
}

#[test]
fn s21_un_cargo_en_dolares_a_una_tarjeta_que_conserva_la_divisa_no_queda_pendiente() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    crear_suscripcion("Plataforma".into(), monto(15.0), tarjeta, "mensual".into(), 1, "USD".into(),
                      Some("01/03/2026".into())).unwrap();

    procesar_en(2026, 3, 1);

    assert_ne!(estado_conversion(id_del_ultimo_gasto()).as_deref(), Some("pendiente"));
    assert_importe(balances_tarjeta(tarjeta).1, 15.0, "la deuda queda en dólares");
}

#[test]
fn s22_las_condiciones_de_una_suscripcion_se_validan_con_un_mensaje_que_se_entiende() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let crear = |m: &str, fre: &str, dia: i32, div: &str| {
        crear_suscripcion("X".into(), importe(m), tarjeta, fre.into(), dia, div.into(),
                          Some("01/03/2026".into()))
    };

    // Un importe que no cobra: cero o negativo abonaría a la tarjeta.
    assert!(crear("0.00", "mensual", 1, "DOP").unwrap_err().contains("mayor que cero"));
    assert!(crear("-15.00", "mensual", 1, "DOP").unwrap_err().contains("mayor que cero"));
    // La divisa, la frecuencia y el día, con su nombre y no con una restricción cruda.
    assert!(crear("15.00", "mensual", 1, "EUR").unwrap_err().contains("EUR"));
    assert!(crear("15.00", "semanal", 1, "DOP").unwrap_err().contains("semanal"));
    assert!(crear("15.00", "mensual", 0, "DOP").unwrap_err().contains("entre 1 y 31"));
    assert!(crear("15.00", "mensual", 32, "DOP").unwrap_err().contains("entre 1 y 31"));

    let filas: i64 = conexion().query_row("SELECT COUNT(*) FROM suscripciones;", [], |r| r.get(0)).unwrap();
    assert_eq!(filas, 0, "ninguna llegó a guardarse");
    assert!(crear("15.00", "mensual", 31, "DOP").is_ok(), "el día 31 sí es un día");
}

#[test]
fn s23_el_importe_de_una_suscripcion_lo_deciden_los_digitos_escritos() {
    // Entra por texto, como el resto de importes: 500.005 sube a 500.01.
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let sub = crear_suscripcion("X".into(), importe("500.005"), tarjeta, "mensual".into(), 1,
                                "DOP".into(), Some("01/03/2026".into())).unwrap();

    let guardado: f64 = conexion()
        .query_row("SELECT monto FROM suscripciones WHERE id = ?;", [sub], |r| r.get(0))
        .unwrap();
    assert_importe(guardado, 500.01, "la regla del sistema, sin binario");
}

#[test]
fn s24_editar_una_suscripcion_aplica_las_mismas_reglas() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let sub = crear_suscripcion("X".into(), monto(500.0), tarjeta, "mensual".into(), 1, "DOP".into(),
                                Some("01/03/2026".into())).unwrap();

    let editar = |m: &str, fre: &str, dia: i32| {
        crate::actualizar_suscripcion(sub, "X".into(), importe(m), tarjeta, fre.into(), dia,
                                      "DOP".into(), Some("01/03/2026".into()))
    };
    assert!(editar("-1.00", "mensual", 1).is_err());
    assert!(editar("500.00", "semanal", 1).is_err());
    assert!(editar("500.00", "mensual", 40).is_err());
    let guardado: f64 = conexion()
        .query_row("SELECT monto FROM suscripciones WHERE id = ?;", [sub], |r| r.get(0)).unwrap();
    assert_importe(guardado, 500.0, "los rechazos no tocaron nada");
    assert!(editar("600.00", "mensual", 1).is_ok());
}

#[test]
fn s25_el_mensaje_del_cobro_automatico_lleva_el_importe_con_dos_decimales() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    crear_suscripcion("Plataforma".into(), monto(500.0), tarjeta, "mensual".into(), 1, "DOP".into(),
                      Some("01/03/2026".into())).unwrap();

    let mensajes = procesar_en(2026, 3, 1);

    assert!(mensajes[0].contains("DOP 500.00"), "mensaje obtenido: {}", mensajes[0]);
}

// --- El reloj ---

#[test]
fn s16_el_cargo_se_fecha_en_su_vencimiento_y_no_en_el_dia_del_reloj() {
    // Antes el asiento llevaba la fecha de ejecución. Por eso en la base real
    // un cargo de Google One —que factura el día 9— figura asentado el 11:
    // cuadrarlo contra el estado de cuenta era más difícil de lo necesario.
    let _g = entorno_aislado();
    let (_, _) = suscripcion_mensual(9, "09/06/2026");

    procesar_en(2026, 6, 11);

    let (_, _, fecha) = ultimo_gasto_con_fecha();
    assert_eq!(fecha, "09/06/2026");
}

#[test]
fn s17_sin_categoria_de_suscripciones_se_crea_una_en_vez_de_improvisar() {
    // **CAMBIO DE CONDUCTA.** Antes, si el titular renombraba o borraba las
    // dos categorías que la búsqueda reconoce, el cargo caía en el
    // identificador 1 literal: podía terminar archivado como alquiler o
    // gasolina sin que nada lo dijera.
    //
    // Ahora, agotadas las dos búsquedas, se crea «Suscripciones» en vez de
    // usar lo que haya en la posición 1.
    let _g = entorno_aislado();
    let (_, _) = suscripcion_mensual(1, "01/03/2026");
    conexion()
        .execute_batch(
            "UPDATE categorias SET nombre = 'Servicios en línea' WHERE LOWER(nombre) = 'suscripciones';
             UPDATE categorias SET nombre = 'Varios' WHERE LOWER(nombre) = 'otros';",
        )
        .expect("renombrar categorías");

    procesar_en(2026, 3, 10);

    let (categoria, nombre): (i64, String) = conexion()
        .query_row(
            "SELECT g.categoria_id, c.nombre FROM gastos g
             JOIN categorias c ON c.id = g.categoria_id
             ORDER BY g.id DESC LIMIT 1;",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("leer la categoría del cargo");
    assert_ne!(categoria, 1, "no debe caer en la posición 1, que ahora es «Servicios en línea»");
    assert_eq!(nombre, "Suscripciones", "se crea la categoría en vez de improvisar");
}

#[test]
fn s17b_crear_la_categoria_es_idempotente() {
    // Un segundo cargo, con la categoría ya creada por el primero, no debe
    // duplicarla: el nombre es UNIQUE, y un segundo INSERT fallaría el
    // procesamiento entero si esto no se comprobara primero.
    let _g = entorno_aislado();
    let (_, _) = suscripcion_mensual(1, "01/03/2026");
    let (_, _) = suscripcion_mensual(1, "01/03/2026");
    conexion()
        .execute_batch(
            "UPDATE categorias SET nombre = 'X' WHERE LOWER(nombre) = 'suscripciones';
             UPDATE categorias SET nombre = 'Y' WHERE LOWER(nombre) = 'otros';",
        )
        .expect("renombrar categorías");

    let mensajes = procesar_en(2026, 3, 10);

    assert_eq!(mensajes.len(), 2, "las dos suscripciones se procesan sin error");
    let creadas: i64 = conexion()
        .query_row("SELECT COUNT(*) FROM categorias WHERE nombre = 'Suscripciones';", [], |r| r.get(0))
        .expect("contar categorías");
    assert_eq!(creadas, 1, "una sola categoría, no una por cargo");
}

#[test]
fn s17c_con_otros_disponible_no_se_crea_una_categoria_nueva() {
    // La búsqueda en cascada sigue viva: «Otros» basta y no hace falta crear
    // nada.
    let _g = entorno_aislado();
    let (_, _) = suscripcion_mensual(1, "01/03/2026");
    conexion()
        .execute("UPDATE categorias SET nombre = 'X' WHERE LOWER(nombre) = 'suscripciones';", [])
        .expect("renombrar «Suscripciones»");

    procesar_en(2026, 3, 10);

    let nombre: String = conexion()
        .query_row(
            "SELECT c.nombre FROM gastos g JOIN categorias c ON c.id = g.categoria_id
             ORDER BY g.id DESC LIMIT 1;",
            [],
            |r| r.get(0),
        )
        .expect("leer la categoría del cargo");
    assert_eq!(nombre, "Otros");
    let total: i64 = conexion()
        .query_row("SELECT COUNT(*) FROM categorias WHERE nombre = 'Suscripciones';", [], |r| r.get(0))
        .expect("contar categorías");
    assert_eq!(total, 0, "no se creó nada, porque «Otros» ya bastaba");
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
        monto: monto(300.0),
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

    eliminar_gasto(id, motivo_de_prueba()).unwrap();

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
        monto: monto(100.0),
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
    let tasa = crate::liquidar_consumo_pendiente(id, monto(6050.0)).unwrap();

    assert_importe(tasa, 60.5, "la tasa se deduce del importe, no se pide");
    assert_eq!(estado_conversion(id).as_deref(), Some("liquidado"));
    let (pesos, dolares) = balances_tarjeta(tarjeta);
    assert_importe(dolares, 0.0, "baja de la divisa de origen");
    assert_importe(pesos, 6050.0, "y sube en moneda local");
}

#[test]
fn c28b_el_importe_liquidado_lo_decide_el_nucleo_con_los_digitos_escritos() {
    // El emisor cargó 6 050.005 en pesos: el céntimo sube a 6 050.01 como en el resto de la frontera, y
    // la tasa se deduce de ese importe ya decidido.
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    fijar_politica(tarjeta, "traduce");
    let id = crear_gasto(compra_en_dolares(tarjeta)).unwrap();

    let tasa = crate::liquidar_consumo_pendiente(id, importe("6050.005")).unwrap();

    assert_importe(balances_tarjeta(tarjeta).0, 6050.01, "los pesos reciben el céntimo decidido con los dígitos");
    assert_importe(tasa, 60.5001, "la tasa sale del importe decidido, no del número binario");
}

#[test]
fn c28c_un_importe_liquidado_que_no_es_positivo_no_liquida_nada() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    fijar_politica(tarjeta, "traduce");
    let id = crear_gasto(compra_en_dolares(tarjeta)).unwrap();

    for escrito in ["0.00", "0.004", "-6050.00"] {
        assert!(crate::liquidar_consumo_pendiente(id, importe(escrito)).is_err(), "{escrito} no liquida");
    }
    assert_eq!(estado_conversion(id).as_deref(), Some("pendiente"), "sigue pendiente");
    assert_importe(balances_tarjeta(tarjeta).0, 0.0, "ningún saldo se movió");
}

#[test]
fn c29_no_se_liquida_dos_veces_ni_lo_que_no_esta_pendiente() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    fijar_politica(tarjeta, "traduce");
    let id = crear_gasto(compra_en_dolares(tarjeta)).unwrap();

    crate::liquidar_consumo_pendiente(id, monto(6050.0)).unwrap();
    assert!(crate::liquidar_consumo_pendiente(id, monto(6050.0)).is_err(), "no se liquida dos veces");
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
        monto: monto(1234.56),
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
        "09/09/2026".to_string(), tarjeta, monto(61.73), "DOP".to_string(),
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
        monto: monto(5200.00),
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

    crate::crear_bonificacion("09/09/2026".into(), tarjeta, monto(50.00), "DOP".into(),
        "Recompensa base".into(), Some(gasto)).unwrap();
    crate::crear_bonificacion("09/09/2026".into(), tarjeta, monto(100.00), "DOP".into(),
        "Bonificación de categoría".into(), Some(gasto)).unwrap();

    assert_eq!(crate::obtener_bonificaciones().unwrap().len(), 2);
    assert_importe(deuda_pesos(tarjeta), 10000.0 + 5200.00 - 150.00, "el consumo sube y las dos bonificaciones bajan");
}

#[test]
fn c32_revertir_una_bonificacion_restituye_la_deuda() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(10000.0, 0.0);
    let id = crate::crear_bonificacion("09/09/2026".into(), tarjeta, monto(61.73), "DOP".into(),
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

    assert!(crate::crear_bonificacion("09/09/2026".into(), tarjeta, monto(61.73), "DOP".into(),
        "   ".into(), None).is_err(), "el concepto es obligatorio");
    assert!(crate::crear_bonificacion("09/09/2026".into(), tarjeta, monto(0.0), "DOP".into(),
        "Cashback".into(), None).is_err(), "cero no es una bonificación");
    assert_importe(deuda_pesos(tarjeta), 10000.0, "ningún saldo se movió");
}

#[test]
fn c33b_la_bonificacion_decide_el_centimo_con_los_digitos_escritos_y_se_casa_con_su_divisa() {
    // `1.005` sube a 1.01 como en el resto de la frontera, y la divisa declarada decide a qué saldo
    // de la tarjeta se aplica: la de pesos y la de dólares son independientes.
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(10000.0, 500.0);

    let id = crate::crear_bonificacion("09/09/2026".into(), tarjeta, importe("1.005"), "DOP".into(), "Cashback".into(), None).unwrap();
    crate::crear_bonificacion("09/09/2026".into(), tarjeta, importe("20.10"), "USD".into(), "Cashback en dólares".into(), None).unwrap();

    let (pesos, dolares) = balances_tarjeta(tarjeta);
    assert_importe(pesos, 10000.0 - 1.01, "los pesos bajan el céntimo decidido con los dígitos");
    assert_importe(dolares, 500.0 - 20.10, "los dólares bajan por su cuenta");
    let guardada = crate::obtener_bonificaciones().unwrap().into_iter().find(|b| b.id == id).unwrap();
    assert_importe(guardada.monto, 1.01, "lo guardado es el mismo céntimo");
}

#[test]
fn c33c_un_importe_negativo_o_que_redondea_a_cero_no_es_una_bonificacion() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(10000.0, 0.0);

    for escrito in ["-5.00", "0.00", "0.004"] {
        assert!(
            crate::crear_bonificacion("09/09/2026".into(), tarjeta, importe(escrito), "DOP".into(), "Cashback".into(), None).is_err(),
            "{escrito} no es una bonificación"
        );
    }
    assert!(crate::obtener_bonificaciones().unwrap().is_empty());
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

    crate::declarar_saldo_prestamo(id, importe("96250.75"), Some("30/09/2026".into())).unwrap();

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
        monto_prestamo: monto(100_000.0),
        institucion_financiera: "Banco Ejemplo".into(),
        tasa_actual: 12.0,
        cuotas_totales: Some(60),
        cuotas_pendientes: Some(60),
        monto_cuota: monto(5_000.0),
        dia_pago: 25,
        saldo_actual: None,
        limite_credito: Some(monto(150_000.0)),
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
        monto_cuota: monto(cuota),
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
        monto_cuota: monto(7_000.0),
        dia_pago: 10,
        limite_credito: Some(monto(200_000.0)),
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
    let base = |limite: Option<f64>, tarjeta, dia| crate::ActualizarPrestamoInput {
        id: auto,
        tasa_actual: 12.0,
        monto_cuota: monto(5_000.0),
        dia_pago: dia,
        limite_credito: limite.map(monto),
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
        "13/09/2026".into(), origen, destino, monto(8_000.0), monto(8_000.0), monto(100.0), "Traspaso".into(),
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
        "13/09/2026".into(), origen, destino, monto(6_000.0), monto(100.0), monto(0.0), "Compra de divisa".into(),
    )
    .unwrap();

    let tasa: f64 = conexion()
        .query_row("SELECT tasa_cambio FROM transacciones_cuentas WHERE id = ?;",
                   [ultima_transferencia()], |r| r.get(0))
        .unwrap();
    assert_importe(tasa, 100.0 / 6_000.0, "monto_destino / monto_origen");
}

#[test]
fn c46b_los_tres_importes_de_una_transferencia_deciden_el_centavo_por_su_texto() {
    // `1000.005` por texto sube a 1000.01 (por número bajaba a 1000.0): vale para origen, destino y cargo.
    let _g = entorno_aislado();
    let origen = crear_cuenta("Cuenta Ahorros DOP", "DOP", 5_000.0);
    let destino = crear_cuenta("Cuenta Corriente DOP", "DOP", 0.0);
    crate::transferir_entre_cuentas(
        "13/09/2026".into(), origen, destino, importe("1000.005"), importe("1000.005"), importe("1.005"), "Traspaso".into(),
    )
    .unwrap();

    assert_importe(saldo_cuenta_id(origen), 5_000.0 - 1000.01 - 1.01, "origen y cargo suben el céntimo");
    assert_importe(saldo_cuenta_id(destino), 1000.01, "destino sube el céntimo");
}

#[test]
fn c47_revertir_una_transferencia_devuelve_el_monto_y_el_cargo_al_origen() {
    let _g = entorno_aislado();
    let origen = crear_cuenta("Cuenta Ahorros DOP", "DOP", 50_000.0);
    let destino = crear_cuenta("Cuenta Corriente DOP", "DOP", 10_000.0);
    crate::transferir_entre_cuentas(
        "13/09/2026".into(), origen, destino, monto(8_000.0), monto(8_000.0), monto(100.0), "Traspaso".into(),
    )
    .unwrap();

    crate::eliminar_transaccion_cuenta(ultima_transferencia(), motivo_de_prueba()).unwrap();

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
        "13/09/2026".into(), origen, destino, monto(8_000.0), monto(8_000.0), monto(0.0), "Traspaso".into(),
    )
    .unwrap();

    // El titular gasta 5 000 del destino antes de darse cuenta del error.
    conexion()
        .execute("UPDATE cuentas_ahorro SET balance_actual = 3000.0 WHERE id = ?;", [destino])
        .unwrap();

    crate::eliminar_transaccion_cuenta(ultima_transferencia(), motivo_de_prueba()).unwrap();

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
        "13/09/2026".into(), cuenta, cuenta, monto(8_000.0), monto(8_000.0), monto(100.0), "A sí misma".into(),
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
        "13/09/2026".into(), origen, destino, monto(6_000.0), monto(6_000.0), monto(0.0), "Sin convertir".into(),
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
        "13/09/2026".into(), origen, destino, monto(8_000.0), monto(8_000.0), monto(0.0), "Traspaso".into(),
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
        "13/09/2026".into(), origen, destino, monto(5_000.0), monto(5_000.0), monto(0.0), "Sobregiro".into(),
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
        "13/09/2026".into(), origen, destino, monto(8_000.0), monto(8_000.0), monto(0.0), "Traspaso".into(),
    )
    .unwrap();
    let id = ultima_transferencia();

    crate::eliminar_transaccion_cuenta(id, motivo_de_prueba()).unwrap();

    assert!(crate::eliminar_transaccion_cuenta(id, motivo_de_prueba()).is_err(), "no se revierte dos veces");
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
        // La copia del capital que acompaña a cada base no es otro respaldo.
        .filter(|e| e.path().extension().map(|x| x == "db").unwrap_or(false))
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
        tarjeta, "14/09/2026".to_string(), monto(12_000.0), "DOP".to_string(), Some(cuenta), 0.0,
    )
    .unwrap();
    assert_importe(balances_tarjeta(tarjeta).0, 18_000.0, "la deuda bajó");
    assert_importe(saldo_cuenta_id(cuenta), 87_976.0, "salieron 12 000 + 24");

    let abono = ultimo_abono();
    revertir_abono_tarjeta(abono, motivo_de_prueba()).unwrap();

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
        tarjeta, "14/09/2026".to_string(), monto(100.0), "USD".to_string(), Some(cuenta), 60.0,
    )
    .unwrap();
    assert_importe(balances_tarjeta(tarjeta).1, 400.0, "la deuda en dólares bajó");
    assert_importe(saldo_cuenta_id(cuenta), 93_988.0, "salieron 6 000 + 12");

    revertir_abono_tarjeta(ultimo_abono(), motivo_de_prueba()).unwrap();

    assert_importe(balances_tarjeta(tarjeta).1, 500.0, "vuelve en dólares");
    assert_importe(saldo_cuenta_id(cuenta), 100_000.0, "y a la cuenta vuelven pesos");
}

#[test]
fn c66_revertir_un_abono_sin_cuenta_solo_repone_la_deuda() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(30_000.0, 0.0);

    registrar_pago_tarjeta(
        tarjeta, "14/09/2026".to_string(), monto(12_000.0), "DOP".to_string(), None, 0.0,
    )
    .unwrap();

    revertir_abono_tarjeta(ultimo_abono(), motivo_de_prueba()).unwrap();

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
        tarjeta, "14/09/2026".to_string(), monto(5_000.0), "USD".to_string(), Some(cuenta), 59.9,
    )
    .unwrap();
    assert_importe(balances_tarjeta(tarjeta).1, -5_000.0, "queda saldo a favor, no cero");

    revertir_abono_tarjeta(ultimo_abono(), motivo_de_prueba()).unwrap();

    assert_importe(balances_tarjeta(tarjeta).1, 0.0, "el saldo a favor se deshace");
    assert_importe(saldo_cuenta_id(cuenta), 500_000.0, "y el dinero vuelve entero");
}

#[test]
fn c68_revertir_dos_veces_falla_la_segunda_sin_duplicar_la_devolucion() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(30_000.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100_000.0);
    registrar_pago_tarjeta(
        tarjeta, "14/09/2026".to_string(), monto(12_000.0), "DOP".to_string(), Some(cuenta), 0.0,
    )
    .unwrap();
    let abono = ultimo_abono();

    revertir_abono_tarjeta(abono, motivo_de_prueba()).unwrap();
    assert!(revertir_abono_tarjeta(abono, motivo_de_prueba()).is_err(), "el abono ya no existe");

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

fn factura(numero: &str, importe_factura: f64, retencion: f64) -> IngresoInput {
    IngresoInput {
        numero_factura: numero.to_string(),
        rnc_cliente: "000000000".to_string(),
        nombre_cliente: "Cliente Ejemplo".to_string(),
        fecha_emision: "16/09/2026".to_string(),
        monto_total: monto(importe_factura),
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

    actualizar_ingreso(id, "A-006".into(), 1, "16/09/2026".into(), monto(1_234.56), 15.0, None, Some(motivo_de_prueba())).unwrap();

    assert_importe(retencion_de(id), 185.18, "crear y corregir no divergen");
}

// --- Cobro ---

#[test]
fn c75_cobrar_una_factura_acredita_la_cuenta_indicada() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso(factura("A-007", 10_000.0, 15.0)).unwrap();

    marcar_ingreso_pagado(id, cuenta, "16/09/2026".into(), monto(8_500.0)).unwrap();

    assert_eq!(estatus_de(id), "pagada");
    assert_importe(saldo_cuenta_id(cuenta), 9_500.0, "entra el neto recibido");
}

#[test]
fn c75b_el_cobro_decide_el_centimo_con_los_digitos_escritos_y_la_fila_guarda_lo_que_se_acredita() {
    // El importe se usa dos veces —se acredita a la cuenta y se guarda en la fila—; ambos salen de la misma
    // decisión, así que no pueden divergir. `8500.005` sube a 8500.01.
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso(factura("A-007", 10_000.0, 15.0)).unwrap();

    marcar_ingreso_pagado(id, cuenta, "16/09/2026".into(), importe("8500.005")).unwrap();

    assert_importe(saldo_cuenta_id(cuenta), 1_000.0 + 8_500.01, "la cuenta recibe el céntimo decidido");
    let fila = crate::obtener_ingresos().unwrap().into_iter().find(|i| i.id == id).unwrap();
    assert_importe(fila.monto_recibido.unwrap(), 8_500.01, "la fila guarda exactamente lo que se acreditó");
}

#[test]
fn c75c_un_cobro_que_no_es_positivo_o_redondea_a_cero_no_acredita_nada() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso(factura("A-007", 10_000.0, 15.0)).unwrap();

    for escrito in ["-100.00", "0.004"] {
        let r = marcar_ingreso_pagado(id, cuenta, "16/09/2026".into(), importe(escrito));
        assert!(r.is_err() || (saldo_cuenta_id(cuenta) - 1_000.0).abs() < 1e-9, "{escrito}: no debe acreditar");
    }
    assert_importe(saldo_cuenta_id(cuenta), 1_000.0, "ningún saldo se movió");
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

    let r = marcar_ingreso_pagado(id, 9_999, "16/09/2026".into(), monto(8_500.0));

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

    let r = marcar_ingreso_pagado(404, cuenta, "16/09/2026".into(), monto(100.0));

    assert!(r.is_err(), "ahora lo dice");
    assert_importe(saldo_cuenta_id(cuenta), 1_000.0, "y no acredita nada");
}

#[test]
fn c78_h19_resuelto_cobrar_en_otra_divisa_se_rechaza() {
    // **H19, resuelto de verdad esta vez.** La versión anterior de este
    // arreglo denominaba el importe **con la divisa de la cuenta**, de modo
    // que la comprobación comparaba esa divisa consigo misma y no podía
    // fallar: 8 500 pesos entraban como 8 500 dólares sin que nada lo dijera.
    //
    // Una factura se emite en moneda local —`ingresos` no tiene columna de
    // divisa— así que cobrarla en una cuenta en dólares exigiría una
    // conversión que nadie ha declarado. Se rechaza en vez de inventarla.
    let _g = entorno_aislado();
    let cuenta_usd = crear_cuenta("Cuenta Ahorros USD", "USD", 100.0);
    let id = crear_ingreso(factura("A-009", 10_000.0, 15.0)).unwrap();

    let r = marcar_ingreso_pagado(id, cuenta_usd, "16/09/2026".into(), monto(8_500.0));

    assert!(r.is_err(), "no se reinterpretan pesos como dólares");
    assert_eq!(estatus_de(id), "emitida", "la factura sigue pendiente");
    assert_importe(saldo_cuenta_id(cuenta_usd), 100.0, "y la cuenta no se toca");
}

#[test]
fn c78b_cobrar_en_una_cuenta_de_la_misma_divisa_funciona() {
    // El contrapunto: la comprobación rechaza lo que no cuadra sin estorbar
    // lo que sí. Sin esta prueba, negarse siempre también pasaría c78.
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso(factura("A-010b", 10_000.0, 15.0)).unwrap();

    marcar_ingreso_pagado(id, cuenta, "16/09/2026".into(), monto(8_500.0)).unwrap();

    assert_importe(saldo_cuenta_id(cuenta), 9_500.0, "entra el neto, sin estorbos");
}

// --- Borrado ---

#[test]
fn c79_borrar_una_factura_cobrada_revierte_el_abono() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso(factura("A-010", 10_000.0, 15.0)).unwrap();
    marcar_ingreso_pagado(id, cuenta, "16/09/2026".into(), monto(8_500.0)).unwrap();

    eliminar_ingreso(id, motivo_de_prueba()).unwrap();

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
    marcar_ingreso_pagado(id, cuenta, "16/09/2026".into(), monto(8_500.0)).unwrap();
    // El titular gasta lo cobrado antes de advertir el error de registro.
    conexion()
        .execute("UPDATE cuentas_ahorro SET balance_actual = 500.0 WHERE id = ?;", params![cuenta])
        .unwrap();

    eliminar_ingreso(id, motivo_de_prueba()).unwrap();

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

    crear_cobro_efectivo_informal("16/09/2026".into(), "Trabajo suelto".into(), monto(2_000.0), "DOP".into())
        .unwrap();

    assert_importe(balance_cuenta("Efectivo DOP"), antes + 2_000.0, "la caja recibe el importe");
}

#[test]
fn c81b_el_cobro_en_efectivo_decide_el_centimo_con_los_digitos_escritos_y_lo_usa_igual_en_los_dos_sitios() {
    // El importe se usa dos veces —el ingreso y el saldo de la caja—: una sola conversión exacta
    // garantiza que ambos reciben el mismo valor. `1.005` sube a 1.01 como en el resto de la frontera.
    let _g = entorno_aislado();
    let antes = balance_cuenta("Efectivo DOP");

    let id = crear_cobro_efectivo_informal("16/09/2026".into(), "Clase suelta".into(), importe("1.005"), "DOP".into()).unwrap();

    let fila = crate::obtener_ingresos_informales().unwrap().into_iter().find(|i| i.id == id).unwrap();
    assert_importe(fila.monto, 1.01, "el ingreso guarda el céntimo decidido");
    assert_importe(fila.monto_recibido.unwrap(), 1.01, "lo recibido es lo mismo");
    assert_eq!(fila.estatus, "pagado");
    assert_importe(balance_cuenta("Efectivo DOP"), antes + 1.01, "la caja recibe exactamente lo mismo");
}

#[test]
fn c81c_un_cobro_en_dolares_entra_en_la_caja_de_dolares_y_no_toca_la_de_pesos() {
    let _g = entorno_aislado();
    let (dop, usd) = (balance_cuenta("Efectivo DOP"), balance_cuenta("Efectivo USD"));

    crear_cobro_efectivo_informal("16/09/2026".into(), "Cobro en dólares".into(), importe("321.10"), "USD".into()).unwrap();

    assert_importe(balance_cuenta("Efectivo USD"), usd + 321.10, "la caja de dólares recibe el importe");
    assert_importe(balance_cuenta("Efectivo DOP"), dop, "la de pesos no se mueve");
}

#[test]
fn c82_h17_el_informal_comparte_el_arreglo() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso_informal("16/09/2026".into(), "Trabajo suelto".into(), monto(2_000.0)).unwrap();

    let r = marcar_informal_pagado(id, 9_999, "16/09/2026".into(), monto(2_000.0));

    assert!(r.is_err(), "el informal falla igual que la factura");
    assert_importe(saldo_cuenta_id(cuenta), 1_000.0, "ningún saldo se movió");
}

#[test]
fn c82b_el_cobro_de_un_informal_decide_el_centimo_con_los_digitos_escritos() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso_informal("16/09/2026".into(), "Trabajo suelto".into(), monto(2_000.0)).unwrap();

    marcar_informal_pagado(id, cuenta, "16/09/2026".into(), importe("2000.005")).unwrap();

    assert_importe(saldo_cuenta_id(cuenta), 1_000.0 + 2_000.01, "la cuenta recibe el céntimo decidido");
    let fila = crate::obtener_ingresos_informales().unwrap().into_iter().find(|i| i.id == id).unwrap();
    assert_importe(fila.monto_recibido.unwrap(), 2_000.01, "la fila guarda exactamente lo que se acreditó");
}

#[test]
fn c83_borrar_un_informal_cobrado_revierte_su_abono() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso_informal("16/09/2026".into(), "Trabajo suelto".into(), monto(2_000.0)).unwrap();
    marcar_informal_pagado(id, cuenta, "16/09/2026".into(), monto(2_000.0)).unwrap();

    eliminar_ingreso_informal(id, motivo_de_prueba()).unwrap();

    assert_importe(saldo_cuenta_id(cuenta), 1_000.0, "el saldo vuelve donde estaba");
}

#[test]
fn c84_un_informal_sin_cobrar_no_mueve_ningun_saldo_al_borrarse() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso_informal("16/09/2026".into(), "Trabajo suelto".into(), monto(2_000.0)).unwrap();

    eliminar_ingreso_informal(id, motivo_de_prueba()).unwrap();

    assert_importe(saldo_cuenta_id(cuenta), 1_000.0, "nunca entró, nada sale");
}

#[test]
fn c84b_el_importe_de_un_informal_lo_decide_el_nucleo_con_los_digitos_escritos() {
    // `crear_ingreso_informal` fue el primer comando de los 15 que seguían recibiendo un `f64` que
    // migró al importe en texto: antes guardaba el número tal cual llegaba, sin que el núcleo
    // decidiera el céntimo; ahora `1.005` sube a 1.01 como en el resto de la frontera.
    let _g = entorno_aislado();
    let id = crear_ingreso_informal("16/09/2026".into(), "Clase suelta".into(), importe("1.005")).unwrap();

    let guardado = crate::obtener_ingresos_informales().unwrap().into_iter().find(|i| i.id == id).unwrap();
    assert_importe(guardado.monto, 1.01, "el céntimo se decide con los dígitos escritos");
    assert_eq!(guardado.estatus, "pendiente");
}

#[test]
fn c84c_un_importe_de_dos_decimales_se_guarda_exacto_y_un_texto_ilegible_no_crea_nada() {
    let _g = entorno_aislado();
    let id = crear_ingreso_informal("16/09/2026".into(), "Clase suelta".into(), importe("75.25")).unwrap();
    let guardado = crate::obtener_ingresos_informales().unwrap().into_iter().find(|i| i.id == id).unwrap();
    assert_importe(guardado.monto, 75.25, "dos decimales: sin pérdida");

    // El texto ilegible falla **antes** de llegar al comando: en la frontera, al leer el argumento.
    assert!(crate::ipc::ImporteDecimal::desde_texto("setenta").is_err());
    assert_eq!(crate::obtener_ingresos_informales().unwrap().len(), 1, "no se creó ningún ingreso");
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
    marcar_ingreso_pagado(id, cuenta, "20/09/2026".into(), monto(8_500.0)).unwrap();
    assert_importe(saldo_cuenta_id(cuenta), 9_500.0, "entró el neto");

    // Eran 12 000, no 10 000. El neto sube de 8 500 a 10 200.
    actualizar_ingreso(id, "B-001".into(), 1, "20/09/2026".into(), monto(12_000.0), 15.0, None, Some(motivo_de_prueba())).unwrap();

    assert_importe(retencion_de(id), 1_800.0, "la retención se recalcula");
    assert_importe(recibido_de(id), 10_200.0, "y lo recibido también");
    assert_importe(saldo_cuenta_id(cuenta), 11_200.0, "la cuenta recibe los 1 700 que faltaban");
}

#[test]
fn c86_corregir_a_la_baja_retira_de_la_cuenta_lo_que_sobraba() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso(factura("B-002", 10_000.0, 15.0)).unwrap();
    marcar_ingreso_pagado(id, cuenta, "20/09/2026".into(), monto(8_500.0)).unwrap();

    actualizar_ingreso(id, "B-002".into(), 1, "20/09/2026".into(), monto(8_000.0), 15.0, None, Some(motivo_de_prueba())).unwrap();

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
    marcar_ingreso_pagado(id, cuenta, "20/09/2026".into(), monto(8_000.0)).unwrap();

    actualizar_ingreso(id, "B-003".into(), 1, "20/09/2026".into(), monto(12_000.0), 15.0, None, Some(motivo_de_prueba())).unwrap();

    assert_importe(recibido_de(id), 10_200.0, "el neto nuevo, entero");
    assert_importe(saldo_cuenta_id(cuenta), 11_200.0, "la cuenta sube los 2 200 que faltaban");
}

#[test]
fn c87a_el_total_corregido_decide_el_centavo_por_su_texto() {
    // `1000.005` por texto sube a 1000.01 (por número bajaba a 1000.0); la fila guarda ese total.
    let _g = entorno_aislado();
    let id = crear_ingreso(factura("B-000", 5_000.0, 15.0)).unwrap();

    actualizar_ingreso(id, "B-000".into(), 1, "20/09/2026".into(), importe("1000.005"), 15.0, None, None).unwrap();

    let total: f64 = conexion()
        .query_row("SELECT monto_total FROM ingresos WHERE id = ?;", params![id], |r| r.get(0))
        .unwrap();
    assert_importe(total, 1000.01, "el total sube el céntimo");
}

#[test]
fn c87b_un_cobro_parcial_declarado_conserva_lo_que_falta() {
    // **La excepción, afirmada.** El neto es 10 200 y solo entraron
    // 9 137,25: queda un resto por cobrar, y la factura lo dice en lugar de
    // darlo por saldado.
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso(factura("B-006", 10_000.0, 15.0)).unwrap();
    marcar_ingreso_pagado(id, cuenta, "20/09/2026".into(), monto(8_500.0)).unwrap();

    actualizar_ingreso(id, "B-006".into(), 1, "20/09/2026".into(), monto(12_000.0), 15.0, Some(importe("9137.25")), Some(motivo_de_prueba()))
        .unwrap();

    assert_importe(recibido_de(id), 9_137.25, "lo que de verdad entró");
    assert_importe(saldo_cuenta_id(cuenta), 10_137.25, "la cuenta sigue a lo recibido");
    assert_importe(12_000.0 - retencion_de(id) - recibido_de(id), 1_062.75, "queda por cobrar");
}

#[test]
fn c87c_un_cobro_parcial_mayor_que_el_neto_se_rechaza() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso(factura("B-007", 10_000.0, 15.0)).unwrap();
    marcar_ingreso_pagado(id, cuenta, "20/09/2026".into(), monto(8_500.0)).unwrap();

    let r = actualizar_ingreso(
        id, "B-007".into(), 1, "20/09/2026".into(), monto(10_000.0), 15.0, Some(importe("9137.25")),
        Some(motivo_de_prueba()),
    );

    assert!(r.is_err(), "cobrar más que el neto no es un cobro parcial");
}

#[test]
fn c88_corregir_sin_cambiar_importes_no_mueve_ningun_saldo() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso(factura("B-004", 10_000.0, 15.0)).unwrap();
    marcar_ingreso_pagado(id, cuenta, "20/09/2026".into(), monto(8_500.0)).unwrap();

    // Solo cambia la fecha.
    actualizar_ingreso(id, "B-004".into(), 1, "21/09/2026".into(), monto(10_000.0), 15.0, None, Some(motivo_de_prueba())).unwrap();

    assert_importe(saldo_cuenta_id(cuenta), 9_500.0, "corregir la fecha no toca la cuenta");
}

#[test]
fn c89_corregir_una_factura_sin_cobrar_no_toca_ninguna_cuenta() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso(factura("B-005", 10_000.0, 15.0)).unwrap();

    actualizar_ingreso(id, "B-005".into(), 1, "20/09/2026".into(), monto(12_000.0), 15.0, None, Some(motivo_de_prueba())).unwrap();

    assert_importe(retencion_de(id), 1_800.0, "las cifras sí cambian");
    assert_importe(saldo_cuenta_id(cuenta), 1_000.0, "pero no hay dinero que ajustar");
}

#[test]
fn c90_corregir_una_factura_inexistente_falla_en_vez_de_callar() {
    let _g = entorno_aislado();
    let r = actualizar_ingreso(404, "X".into(), 1, "20/09/2026".into(), monto(100.0), 15.0, None, Some(motivo_de_prueba()));

    assert!(r.is_err(), "no se corrige lo que no existe");
}

// ---------------------------------------------------------------------------
//  Casos de corrección sobre facturas
//
//  Corregir una factura cobrada mueve un saldo, igual que borrarla. El rastro
//  se exige donde hay riesgo y no donde no lo hay: pedir explicación para
//  cambiar una fecha enseñaría a escribirla sin pensar, que es el modo en que
//  un control de este tipo deja de servir.
// ---------------------------------------------------------------------------

fn casos_abiertos() -> i64 {
    conexion()
        .query_row("SELECT COUNT(*) FROM correcciones;", [], |r| r.get(0))
        .expect("contar casos")
}

fn ultimo_caso() -> (String, String, f64) {
    conexion()
        .query_row(
            "SELECT tipo, descripcion, importe FROM correcciones ORDER BY id DESC LIMIT 1;",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .expect("leer último caso")
}

#[test]
fn c91_corregir_una_factura_cobrada_abre_caso_con_el_ajuste() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso(factura("C-001", 14_400.0, 15.0)).unwrap();
    marcar_ingreso_pagado(id, cuenta, "20/09/2026".into(), monto(12_240.0)).unwrap();
    assert_eq!(casos_abiertos(), 0);

    // Eran 16 000: el neto sube de 12 240 a 13 600.
    actualizar_ingreso(
        id, "C-001".into(), 1, "20/09/2026".into(), monto(16_000.0), 15.0, None,
        Some(motivo_de_prueba()),
    )
    .unwrap();

    assert_eq!(casos_abiertos(), 1, "mover un saldo deja rastro");
    let (tipo, descripcion, importe) = ultimo_caso();
    assert_eq!(tipo, "corrección de factura");
    assert!(descripcion.contains("14400.00"), "descripción: {descripcion}");
    assert!(descripcion.contains("16000.00"), "descripción: {descripcion}");
    assert_importe(importe, 1_360.0, "el caso guarda el ajuste");
}

#[test]
fn c92_corregir_sin_mover_dinero_no_abre_caso() {
    // Cambiar la fecha de una factura cobrada no toca ningún saldo. Exigir
    // explicación aquí sería fricción sin riesgo, y la fricción que no
    // protege solo enseña a escribir motivos de trámite.
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso(factura("C-002", 10_000.0, 15.0)).unwrap();
    marcar_ingreso_pagado(id, cuenta, "20/09/2026".into(), monto(8_500.0)).unwrap();

    actualizar_ingreso(
        id, "C-002".into(), 1, "21/09/2026".into(), monto(10_000.0), 15.0, None, None,
    )
    .unwrap();

    assert_eq!(casos_abiertos(), 0, "sin movimiento no hay caso");
}

#[test]
fn c93_corregir_una_factura_sin_cobrar_tampoco_abre_caso() {
    let _g = entorno_aislado();
    crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso(factura("C-003", 10_000.0, 15.0)).unwrap();

    actualizar_ingreso(
        id, "C-003".into(), 1, "20/09/2026".into(), monto(12_000.0), 15.0, None, None,
    )
    .unwrap();

    assert_eq!(casos_abiertos(), 0, "nada cobrado, nada que mover");
}

#[test]
fn c94_mover_dinero_sin_motivo_se_rechaza_y_no_corrige_nada() {
    // La transacción se deshace entera: ni caso, ni corrección, ni saldo
    // movido. Un rechazo a medias sería peor que no comprobar.
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    let id = crear_ingreso(factura("C-004", 10_000.0, 15.0)).unwrap();
    marcar_ingreso_pagado(id, cuenta, "20/09/2026".into(), monto(8_500.0)).unwrap();

    let r = actualizar_ingreso(
        id, "C-004".into(), 1, "20/09/2026".into(), monto(12_000.0), 15.0, None, None,
    );

    assert!(r.is_err(), "mover un saldo exige explicarlo");
    assert_eq!(casos_abiertos(), 0);
    assert_importe(retencion_de(id), 1_500.0, "la factura no cambió");
    assert_importe(saldo_cuenta_id(cuenta), 9_500.0, "y la cuenta tampoco");
}

// ---------------------------------------------------------------------------
//  La comisión por pago de impuestos pasa por el núcleo
//
//  Era la única vía por la que un importe llegaba a la base sin que `Dinero`
//  decidiera su céntimo: `depurar_datos_de_cuenta` comprobaba que el número
//  fuera finito y no negativo, y lo escribía tal cual.
// ---------------------------------------------------------------------------

fn comision_de(cuenta_id: i64) -> Option<f64> {
    conexion()
        .query_row(
            "SELECT comision_pago_impuestos FROM cuentas_ahorro WHERE id = ?;",
            params![cuenta_id],
            |r| r.get(0),
        )
        .expect("leer comisión")
}

#[test]
fn c94b_el_saldo_inicial_de_una_cuenta_lo_decide_el_nucleo_con_los_digitos_escritos() {
    // `100.005` sube a 100.01: el saldo inicial entra por la misma puerta que el resto de los importes.
    let _g = entorno_aislado();
    crate::crear_cuenta("Cuenta Nueva DOP".into(), "DOP".into(), importe("100.005"), None, None).unwrap();

    assert_importe(balance_cuenta("Cuenta Nueva DOP"), 100.01, "el saldo inicial lleva el céntimo decidido");
}

#[test]
fn c95_una_comision_con_fraccion_de_centimo_se_decide_al_crear() {
    let _g = entorno_aislado();
    let id = crate::crear_cuenta(
        "Cuenta Corriente DOP".into(), "DOP".into(), monto(0.0),
        Some("Banco Ejemplo".into()), Some(importe("75.005")),
    )
    .unwrap();

    // 75.005 se guarda en f64 como 75.00499…, pero al multiplicar por 100 el
    // error se cancela y da 7500.5 exacto: mitad alejándose de cero, sube.
    // Es el mismo fenómeno que documenta `dividir_redondeando`, y por eso la
    // expectativa se calcula, no se intuye.
    assert_importe(comision_de(id).unwrap(), 75.01, "el tercer decimal se decide");
}

#[test]
fn c96_lo_mismo_al_corregir_una_cuenta_existente() {
    // Crear y corregir no pueden divergir: son la misma regla.
    let _g = entorno_aislado();
    let id = crear_cuenta("Cuenta Corriente DOP", "DOP", 0.0);

    crate::actualizar_cuenta(
        id, "Cuenta Corriente DOP".into(), Some("Banco Ejemplo".into()), Some(importe("120.507")),
    )
    .unwrap();

    assert_importe(comision_de(id).unwrap(), 120.51, "sube, como manda la regla");
}

#[test]
fn c97_una_comision_sin_declarar_sigue_siendo_nula_y_no_cero() {
    // Nulo significa «no hay tarifa pactada» y cero significa «el banco no
    // cobra». Pasar por `Dinero` no puede borrar esa distinción.
    let _g = entorno_aislado();
    let id = crate::crear_cuenta(
        "Cuenta Ahorros DOP".into(), "DOP".into(), monto(0.0), None, None,
    )
    .unwrap();

    assert!(comision_de(id).is_none(), "sigue sin declarar");
}

#[test]
fn c98_una_comision_negativa_se_sigue_rechazando() {
    let _g = entorno_aislado();
    let r = crate::crear_cuenta(
        "Cuenta Ahorros DOP".into(), "DOP".into(), monto(0.0), None, Some(importe("-1.00")),
    );

    assert!(r.is_err());
}


// =====================================================================
//  Capital — la alerta de vencimiento no debe sobrevivir a un guardado
// =====================================================================

fn capital_con_una_entrada(coleccion: &str, vencimiento: &str) -> serde_json::Value {
    serde_json::json!({
        "propiedades": {"inmobiliario": [], "vehiculos": [], "maquinaria": []},
        "certificados": if coleccion == "certificados" {
            serde_json::json!([{"banco": "Banco Ejemplo", "monto": 10000.0, "tasa": 8.0, "vencimiento": vencimiento}])
        } else { serde_json::json!([]) },
        "bolsa": if coleccion == "bolsa" {
            serde_json::json!([{"emisor": "Emisor Ejemplo", "monto": 5000.0, "tasa": 6.0, "vencimiento": vencimiento}])
        } else { serde_json::json!([]) },
    })
}

fn escribir_capital_de_prueba(datos: &serde_json::Value) {
    crate::db_nosql::guardar_coleccion("capital", datos).expect("sembrar capital");
}

/// Lo que hay en el archivo, sin pasar por `obtener_capital`: es la única
/// forma de ver si un campo calculado quedó grabado.
fn capital_en_disco() -> serde_json::Value {
    crate::db_nosql::leer_coleccion("capital").expect("leer capital")
}

#[test]
fn c99_un_certificado_lejos_de_vencer_no_lleva_alerta() {
    let _g = entorno_aislado();
    escribir_capital_de_prueba(&capital_con_una_entrada("certificados", "31/12/2030"));

    let leido = crate::obtener_capital().unwrap();
    let c = &leido["certificados"][0];
    assert_eq!(c["alerta_vencimiento"], false);
    assert!(c.get("alerta_msg").is_none());
}

#[test]
fn c100_un_certificado_que_vence_en_diez_dias_avisa_con_la_cuenta_regresiva() {
    let _g = entorno_aislado();
    let hoy = chrono::Local::now().naive_local().date();
    let vence = (hoy + chrono::Duration::days(10)).format("%d/%m/%Y").to_string();
    escribir_capital_de_prueba(&capital_con_una_entrada("certificados", &vence));

    let leido = crate::obtener_capital().unwrap();
    let c = &leido["certificados"][0];
    assert_eq!(c["alerta_vencimiento"], true);
    assert_eq!(c["dias_restantes"], 10);
    assert_eq!(c["alerta_msg"], "¡Vence en 10 días!");
}

#[test]
fn c101_un_certificado_vencido_lo_dice_sin_recortar_los_dias() {
    let _g = entorno_aislado();
    let hoy = chrono::Local::now().naive_local().date();
    let vencio = (hoy - chrono::Duration::days(5)).format("%d/%m/%Y").to_string();
    escribir_capital_de_prueba(&capital_con_una_entrada("certificados", &vencio));

    let leido = crate::obtener_capital().unwrap();
    let c = &leido["certificados"][0];
    assert_eq!(c["alerta_vencimiento"], true);
    assert_eq!(c["dias_restantes"], -5, "negativo, sin recortar");
    assert_eq!(c["alerta_msg"], "¡Vencido!");
}

#[test]
fn c102_lo_mismo_vale_para_una_inversion_de_bolsa() {
    let _g = entorno_aislado();
    let hoy = chrono::Local::now().naive_local().date();
    let vencio = (hoy - chrono::Duration::days(1)).format("%d/%m/%Y").to_string();
    escribir_capital_de_prueba(&capital_con_una_entrada("bolsa", &vencio));

    let leido = crate::obtener_capital().unwrap();
    let b = &leido["bolsa"][0];
    assert_eq!(b["alerta_vencimiento"], true);
    assert_eq!(b["alerta_msg"], "¡Vencido!");
}

#[test]
fn c103_guardar_no_persiste_la_alerta_que_obtener_calculo() {
    // **CAMBIO DE CONDUCTA — el defecto de fondo.**
    //
    // Antes: `obtener_capital` calculaba la alerta y la escribía en el mismo
    // `Value`. Como las seis acciones de la vista de capital hacen
    // leer → mutar una colección → guardar el objeto entero, ese cálculo
    // quedaba grabado en el archivo. Confirmado contra la base real: una
    // inversión de bolsa ya tenía `alerta_vencimiento` y `dias_restantes` en
    // disco, calculados el día de la última escritura y nunca más.
    let _g = entorno_aislado();
    let hoy = chrono::Local::now().naive_local().date();
    let vencio = (hoy - chrono::Duration::days(3)).format("%d/%m/%Y").to_string();
    escribir_capital_de_prueba(&capital_con_una_entrada("bolsa", &vencio));

    // Exactamente el patrón de la interfaz: leer todo, guardar todo.
    let leido = crate::obtener_capital().unwrap();
    assert_eq!(leido["bolsa"][0]["alerta_vencimiento"], true, "obtener sí la calcula");
    crate::guardar_capital(leido).unwrap();

    let en_disco = capital_en_disco();
    let b = &en_disco["bolsa"][0];
    assert!(b.get("alerta_vencimiento").is_none(), "no debe quedar en el archivo");
    assert!(b.get("dias_restantes").is_none());
    assert!(b.get("alerta_msg").is_none());
    // Y lo declarado por el titular sigue intacto.
    assert_eq!(b["emisor"], "Emisor Ejemplo");
    assert_eq!(b["vencimiento"], vencio);
}

#[test]
fn c104_guardar_limpia_certificados_y_bolsa_por_igual() {
    let _g = entorno_aislado();
    let mut datos = capital_con_una_entrada("certificados", "01/01/2020");
    datos["bolsa"] = serde_json::json!([{"emisor": "X", "monto": 1.0, "tasa": 1.0, "vencimiento": "01/01/2020"}]);
    escribir_capital_de_prueba(&datos);

    let leido = crate::obtener_capital().unwrap();
    crate::guardar_capital(leido).unwrap();

    let en_disco = capital_en_disco();
    for coleccion in ["certificados", "bolsa"] {
        let entrada = &en_disco[coleccion][0];
        assert!(entrada.get("alerta_vencimiento").is_none(), "{coleccion} quedó con alerta");
    }
}

#[test]
fn c105_guardar_no_falla_si_los_campos_nunca_llegaron_a_calcularse() {
    // Un alta nueva no pasa por `obtener_capital` con esa entrada todavía
    // dentro: `retirar_campos_calculados` no puede asumir que el campo existe.
    let _g = entorno_aislado();
    let datos = serde_json::json!({
        "propiedades": {"inmobiliario": [], "vehiculos": [], "maquinaria": []},
        "certificados": [{"banco": "Nuevo", "monto": 500.0, "tasa": 5.0, "vencimiento": "01/01/2030"}],
        "bolsa": [],
    });

    assert!(crate::guardar_capital(datos).is_ok());
    let en_disco = capital_en_disco();
    assert_eq!(en_disco["certificados"][0]["banco"], "Nuevo");
}

#[test]
fn c106_una_propiedad_no_lleva_ni_lleva_campos_calculados() {
    // Las propiedades no tienen vencimiento; la limpieza no debe tocarlas.
    let _g = entorno_aislado();
    let datos = serde_json::json!({
        "propiedades": {"inmobiliario": [{"id": "1", "nombre": "Casa", "subtipo": "residencial", "valor_estimado": 100000.0}],
                          "vehiculos": [], "maquinaria": []},
        "certificados": [],
        "bolsa": [],
    });

    crate::guardar_capital(datos).unwrap();
    let en_disco = capital_en_disco();
    assert_eq!(en_disco["propiedades"]["inmobiliario"][0]["nombre"], "Casa");
}

#[test]
fn c107_guardar_capital_rechaza_lo_que_antes_aceptaba_sin_comprobar() {
    // **CAMBIO DE CONDUCTA — cierra la divergencia declarada en la Fase 6.**
    //
    // Antes el capital era la única vía de dinero que no pasaba por `Dinero`:
    // un monto negativo, con fracción de céntimo o ilegible, una tasa negativa
    // y una fecha que no existe se guardaban tal cual. Ahora se rechazan, y
    // el archivo queda como estaba.
    let _g = entorno_aislado();
    let bueno = capital_con_una_entrada("certificados", "31/12/2030");
    escribir_capital_de_prueba(&bueno);

    for (que, entrada) in [
        ("monto negativo", serde_json::json!({"banco": "X", "monto": -500.0, "tasa": 5.0, "vencimiento": "31/12/2030"})),
        ("fracción de céntimo", serde_json::json!({"banco": "X", "monto": 500.005, "tasa": 5.0, "vencimiento": "31/12/2030"})),
        ("monto ilegible", serde_json::json!({"banco": "X", "monto": "abc", "tasa": 5.0, "vencimiento": "31/12/2030"})),
        ("tasa negativa", serde_json::json!({"banco": "X", "monto": 500.0, "tasa": -1.0, "vencimiento": "31/12/2030"})),
        ("fecha inexistente", serde_json::json!({"banco": "X", "monto": 500.0, "tasa": 5.0, "vencimiento": "31/02/2030"})),
        ("fecha en otro formato", serde_json::json!({"banco": "X", "monto": 500.0, "tasa": 5.0, "vencimiento": "2030-12-31"})),
    ] {
        let mut datos = bueno.clone();
        datos["certificados"].as_array_mut().unwrap().push(entrada);
        let r = crate::guardar_capital(datos);
        assert!(r.is_err(), "aceptó {que}");
        assert!(r.unwrap_err().contains("Certificado 2"), "el error no dice cuál entrada: {que}");
    }
    assert_eq!(capital_en_disco(), bueno, "un guardado rechazado no toca el archivo");
}

#[test]
fn c108b_un_importe_escrito_como_texto_se_guarda_como_numero_y_lo_deciden_los_digitos() {
    // El formato en disco no cambia —la aplicación instalada lee el mismo
    // archivo—, pero al entrar, el céntimo lo deciden los dígitos.
    let _g = entorno_aislado();
    let mut datos = capital_con_una_entrada("certificados", "31/12/2030");
    datos["bolsa"] = serde_json::json!([{"emisor": "Emisor Ejemplo", "monto": "1234.565", "tasa": 6.25, "vencimiento": "01/01/2031"}]);

    crate::guardar_capital(datos).unwrap();

    let en_disco = capital_en_disco();
    assert_eq!(en_disco["bolsa"][0]["monto"], serde_json::json!(1234.57), "un número, y el 5 sube");
    assert!(en_disco["bolsa"][0]["monto"].is_number());
}

#[test]
fn c109b_una_entrada_antigua_mal_formada_no_bloquea_borrar_otra() {
    // Sin edición en la interfaz, la única salida para una entrada mala es
    // borrarla; exigir todo cada vez impediría borrar cualquier otra antes.
    let _g = entorno_aislado();
    let antigua = serde_json::json!({"banco": "Antiguo", "monto": -500.005, "tasa": -1.0, "vencimiento": "no es una fecha"});
    let mut guardado = capital_con_una_entrada("certificados", "31/12/2030");
    guardado["certificados"].as_array_mut().unwrap().push(antigua.clone());
    // Se siembra saltándose el comando, como si viniera de antes.
    crate::db_nosql::guardar_coleccion("capital", &guardado).unwrap();

    // Se borra la buena y la antigua vuelve tal cual, como hace la interfaz.
    let mut datos = crate::obtener_capital().unwrap();
    datos["certificados"].as_array_mut().unwrap().remove(0);
    assert!(crate::guardar_capital(datos).is_ok(), "no debe bloquear por una entrada que no se tocó");
    assert_eq!(capital_en_disco()["certificados"][0]["banco"], "Antiguo");

    // Y borrar la antigua también se puede.
    let mut datos = crate::obtener_capital().unwrap();
    datos["certificados"].as_array_mut().unwrap().clear();
    assert!(crate::guardar_capital(datos).is_ok());
}

#[test]
fn c110b_obtener_capital_trae_los_totales_sumados_en_centavos() {
    let _g = entorno_aislado();
    let datos = serde_json::json!({
        "propiedades": {"inmobiliario": [{"id": "1", "nombre": "Casa", "subtipo": "r", "valor_estimado": 100000.10},
                                          {"id": "2", "nombre": "Local", "subtipo": "c", "valor_estimado": 200000.20}],
                          "vehiculos": [], "maquinaria": []},
        "certificados": [{"banco": "A", "monto": 0.1, "tasa": 8.0, "vencimiento": "31/12/2030"},
                         {"banco": "B", "monto": 0.2, "tasa": 8.0, "vencimiento": "31/12/2030"}],
        "bolsa": [],
    });
    crate::guardar_capital(datos).unwrap();

    let t = &crate::obtener_capital().unwrap()["totales"];

    assert_eq!(t["certificados"], serde_json::json!(0.3), "0.1 + 0.2, sin el ruido de la coma flotante");
    assert_eq!(t["inmobiliario"], serde_json::json!(300000.3));
    assert_eq!(t["patrimonio"], serde_json::json!(300000.6));
}

#[test]
fn c111b_los_totales_no_se_guardan_aunque_la_interfaz_los_devuelva() {
    // Igual que la alerta de vencimiento: se calculan al leer, y la interfaz
    // guarda el documento entero de vuelta.
    let _g = entorno_aislado();
    escribir_capital_de_prueba(&capital_con_una_entrada("certificados", "31/12/2030"));

    let leido = crate::obtener_capital().unwrap();
    assert!(leido.get("totales").is_some(), "obtener sí los trae");
    crate::guardar_capital(leido).unwrap();

    assert!(capital_en_disco().get("totales").is_none(), "no deben quedar en el archivo");
}

#[test]
fn c112b_un_identificador_de_bien_repetido_se_rechaza() {
    let _g = entorno_aislado();
    let datos = serde_json::json!({
        "propiedades": {"inmobiliario": [{"id": "7", "nombre": "A", "subtipo": "r", "valor_estimado": 10.0}],
                          "vehiculos": [{"id": "7", "nombre": "B", "subtipo": "s", "valor_estimado": 20.0}],
                          "maquinaria": []},
        "certificados": [], "bolsa": [],
    });
    assert!(crate::guardar_capital(datos).unwrap_err().contains("repetido"));
}

// =====================================================================
//  Avance de efectivo — la tarjeta pone dinero en una cuenta
// =====================================================================

/// Un avance con los valores de siempre; cada prueba cambia solo lo suyo.
fn avance(
    tarjeta: i64,
    cuenta: i64,
    monto: &str,
    tipo: &str,
    porcentaje: Option<f64>,
    fijo: Option<&str>,
) -> Result<String, String> {
    crate::registrar_avance_efectivo(
        tarjeta,
        cuenta,
        "01/10/2026".into(),
        importe(monto),
        "DOP".into(),
        tipo.into(),
        porcentaje,
        fijo.map(importe),
        None,
    )
}

fn filas_de_avances() -> i64 {
    conexion()
        .query_row("SELECT COUNT(*) FROM avances_efectivo;", [], |r| r.get(0))
        .expect("contar avances")
}

#[test]
fn c108_un_avance_porcentual_sube_la_deuda_por_importe_y_cargo_y_la_cuenta_recibe_el_importe() {
    // El caso real más reciente del titular: un cargo del 6.25 %.
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(5_000.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);

    let resumen = avance(tarjeta, cuenta, "12500.00", "porcentaje", Some(6.25), None).unwrap();

    assert_importe(balances_tarjeta(tarjeta).0, 18_281.25, "5 000 + 12 500 + 781,25 de cargo");
    assert_importe(saldo_cuenta_id(cuenta), 13_500.0, "la cuenta recibe el importe, sin el cargo");
    assert!(resumen.contains("781.25"), "el resumen debe decir el cargo: {resumen}");
}

#[test]
fn c109_el_cargo_queda_como_gasto_de_la_tarjeta_y_como_registro_del_avance() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 0.0);

    avance(tarjeta, cuenta, "12500.00", "porcentaje", Some(8.0), None).unwrap();

    assert_eq!(total_gastos(), 1, "el cargo cuenta como gasto");
    let (monto, divisa, _) = ultimo_gasto();
    assert_importe(monto, 1_000.0, "el gasto es el cargo, no el importe");
    assert_eq!(divisa, "DOP");

    let (tipo, tasa, cargo, gasto_id): (String, Option<f64>, f64, Option<i64>) = conexion()
        .query_row(
            "SELECT tipo_cargo, tasa, cargo, gasto_cargo_id FROM avances_efectivo;",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!((tipo.as_str(), tasa), ("porcentaje", Some(8.0)));
    assert_importe(cargo, 1_000.0, "cargo guardado");
    assert!(gasto_id.is_some(), "el avance queda enlazado a su gasto");
}

#[test]
fn c110_un_cargo_fijo_se_suma_tal_cual() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 0.0);

    avance(tarjeta, cuenta, "4600.00", "fijo", None, Some("300.00")).unwrap();

    assert_importe(balances_tarjeta(tarjeta).0, 4_900.0, "importe más cargo fijo");
    assert_importe(saldo_cuenta_id(cuenta), 4_600.0, "la cuenta recibe el importe");
    let tasa: Option<f64> = conexion()
        .query_row("SELECT tasa FROM avances_efectivo;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(tasa, None, "un cargo fijo no tiene tasa");
}

#[test]
fn c111_un_avance_exonerado_no_paga_cargo_ni_genera_gasto() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 0.0);

    avance(tarjeta, cuenta, "2400.00", "exonerado", None, None).unwrap();

    assert_importe(balances_tarjeta(tarjeta).0, 2_400.0, "solo sube el importe");
    assert_importe(saldo_cuenta_id(cuenta), 2_400.0, "y la cuenta lo recibe");
    assert_eq!(total_gastos(), 0, "sin cargo no hay gasto");
    assert_eq!(filas_de_avances(), 1);
}

#[test]
fn c112_la_nota_de_exoneracion_se_guarda_sin_espacios_sobrantes() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 0.0);

    crate::registrar_avance_efectivo(
        tarjeta, cuenta, "01/10/2026".into(), importe("1350.00"), "DOP".into(),
        "exonerado".into(), None, None, Some("  Promoción de la entidad  ".into()),
    )
    .unwrap();

    let nota: Option<String> = conexion()
        .query_row("SELECT nota FROM avances_efectivo;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(nota.as_deref(), Some("Promoción de la entidad"));
}

#[test]
fn c113_una_cuenta_en_otra_divisa_se_rechaza_sin_mover_nada() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(1_000.0, 0.0);
    let cuenta_usd = crear_cuenta("Cuenta Ahorros USD", "USD", 50.0);

    let r = avance(tarjeta, cuenta_usd, "500.00", "exonerado", None, None);

    assert!(r.unwrap_err().contains("misma divisa"));
    assert_importe(balances_tarjeta(tarjeta).0, 1_000.0, "la deuda no se movió");
    assert_importe(saldo_cuenta_id(cuenta_usd), 50.0, "la cuenta tampoco");
    assert_eq!(filas_de_avances(), 0);
}

#[test]
fn c114_un_porcentaje_fuera_de_la_banda_se_rechaza() {
    // 0.8 por 8 es el tecleo que la banda existe para atrapar: el importe
    // entraría en la deuda sin que nada lo cuestionara.
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 0.0);

    for p in [0.8, 5.99, 10.01, 80.0] {
        let r = avance(tarjeta, cuenta, "1350.00", "porcentaje", Some(p), None);
        assert!(r.is_err(), "aceptó {p} %");
    }
    assert_importe(balances_tarjeta(tarjeta).0, 0.0, "nada se movió");
    assert_eq!(filas_de_avances(), 0);
}

#[test]
fn c115_los_extremos_de_la_banda_se_aceptan() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 0.0);

    assert!(avance(tarjeta, cuenta, "1350.00", "porcentaje", Some(6.0), None).is_ok());
    assert!(avance(tarjeta, cuenta, "1350.00", "porcentaje", Some(10.0), None).is_ok());
}

#[test]
fn c116_los_datos_contradictorios_se_rechazan_en_vez_de_elegir_uno() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 0.0);

    // Un porcentaje con un cargo fijo, una exoneración con valor, un tipo
    // que no existe, y un tipo sin su valor.
    assert!(avance(tarjeta, cuenta, "1350.00", "porcentaje", Some(8.0), Some("100.00")).is_err());
    assert!(avance(tarjeta, cuenta, "1350.00", "exonerado", Some(8.0), None).is_err());
    assert!(avance(tarjeta, cuenta, "1350.00", "gratis", None, None).is_err());
    assert!(avance(tarjeta, cuenta, "1350.00", "porcentaje", None, None).is_err());
    assert!(avance(tarjeta, cuenta, "1350.00", "fijo", None, None).is_err());
    assert_eq!(filas_de_avances(), 0);
}

#[test]
fn c117_un_cargo_fijo_de_cero_se_rechaza_porque_eso_es_una_exoneracion() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 0.0);

    assert!(avance(tarjeta, cuenta, "1350.00", "fijo", None, Some("0.00")).is_err());
}

#[test]
fn c118_el_centimo_lo_deciden_los_digitos_escritos() {
    // Entra por texto, como el resto de importes que no pasan por el
    // formulario: 1000.005 sube a 1000.01.
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 0.0);

    avance(tarjeta, cuenta, "1000.005", "exonerado", None, None).unwrap();

    assert_importe(saldo_cuenta_id(cuenta), 1_000.01, "la regla del sistema, sin binario");
}

#[test]
fn c119_una_fecha_ilegible_o_inexistente_se_rechaza() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 0.0);

    for fecha in ["2026-10-01", "1/10/2026", "31/02/2026", "ayer", ""] {
        let r = crate::registrar_avance_efectivo(
            tarjeta, cuenta, fecha.into(), importe("100.00"), "DOP".into(),
            "exonerado".into(), None, None, None,
        );
        assert!(r.is_err(), "aceptó la fecha «{fecha}»");
    }
    assert_eq!(filas_de_avances(), 0);
}

#[test]
fn c120_revertir_un_avance_deja_tarjeta_cuenta_y_gastos_como_estaban_y_abre_un_caso() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(5_000.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 1_000.0);
    avance(tarjeta, cuenta, "12500.00", "porcentaje", Some(6.25), None).unwrap();
    let id: i64 = conexion().query_row("SELECT id FROM avances_efectivo;", [], |r| r.get(0)).unwrap();

    let resumen = crate::revertir_avance_efectivo(id, motivo_de_prueba()).unwrap();

    assert_importe(balances_tarjeta(tarjeta).0, 5_000.0, "la deuda vuelve, cargo incluido");
    assert_importe(saldo_cuenta_id(cuenta), 1_000.0, "la cuenta devuelve el importe");
    assert_eq!(total_gastos(), 0, "el gasto del cargo desaparece");
    assert_eq!(filas_de_avances(), 0);
    assert!(resumen.contains("COR-"), "debe informar el número de caso: {resumen}");
    let casos: i64 = conexion()
        .query_row("SELECT COUNT(*) FROM correcciones WHERE tipo = 'avance de efectivo';", [], |r| r.get(0))
        .unwrap();
    assert_eq!(casos, 1, "queda constancia de qué se deshizo");
}

#[test]
fn c121_revertir_sin_explicar_se_rechaza_y_no_mueve_nada() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 0.0);
    avance(tarjeta, cuenta, "1350.00", "exonerado", None, None).unwrap();
    let id: i64 = conexion().query_row("SELECT id FROM avances_efectivo;", [], |r| r.get(0)).unwrap();

    assert!(crate::revertir_avance_efectivo(id, "error".into()).is_err());

    assert_importe(balances_tarjeta(tarjeta).0, 1_350.0, "sigue todo como estaba");
    assert_eq!(filas_de_avances(), 1);
}

#[test]
fn c122_revertir_dos_veces_falla_la_segunda_sin_duplicar_la_devolucion() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 500.0);
    avance(tarjeta, cuenta, "1350.00", "exonerado", None, None).unwrap();
    let id: i64 = conexion().query_row("SELECT id FROM avances_efectivo;", [], |r| r.get(0)).unwrap();

    crate::revertir_avance_efectivo(id, motivo_de_prueba()).unwrap();
    assert!(crate::revertir_avance_efectivo(id, motivo_de_prueba()).is_err());

    assert_importe(balances_tarjeta(tarjeta).0, 0.0, "una sola devolución");
    assert_importe(saldo_cuenta_id(cuenta), 500.0, "una sola devolución");
}

#[test]
fn c123_el_gasto_del_cargo_no_se_puede_borrar_por_separado() {
    // Bajaría la deuda por el cargo y dejaría el avance registrado con un
    // cargo que ya no existe.
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 0.0);
    avance(tarjeta, cuenta, "12500.00", "porcentaje", Some(8.0), None).unwrap();
    let gasto_id: i64 = conexion()
        .query_row("SELECT gasto_cargo_id FROM avances_efectivo;", [], |r| r.get(0))
        .unwrap();

    let r = crate::eliminar_gasto(gasto_id, motivo_de_prueba());

    assert!(r.unwrap_err().contains("avance de efectivo"));
    assert_importe(balances_tarjeta(tarjeta).0, 13_500.0, "la deuda no se movió");
    assert_eq!(total_gastos(), 1);
    let casos: i64 = conexion().query_row("SELECT COUNT(*) FROM correcciones;", [], |r| r.get(0)).unwrap();
    assert_eq!(casos, 0, "un borrado rechazado no abre caso");
}

#[test]
fn c124_una_cuenta_que_recibio_un_avance_no_se_puede_eliminar() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 0.0);
    avance(tarjeta, cuenta, "1350.00", "exonerado", None, None).unwrap();

    let r = crate::eliminar_cuenta(cuenta);
    assert!(r.unwrap_err().contains("avances de efectivo"));

    // Revertido el avance, la guarda se levanta.
    let id: i64 = conexion().query_row("SELECT id FROM avances_efectivo;", [], |r| r.get(0)).unwrap();
    crate::revertir_avance_efectivo(id, motivo_de_prueba()).unwrap();
    assert!(crate::eliminar_cuenta(cuenta).is_ok());
}

#[test]
fn c125_el_esquema_rechaza_un_avance_que_se_contradice() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 0.0);
    let insertar = |monto: f64, tipo: &str, tasa: Option<f64>, cargo: f64, fecha: &str| {
        conexion().execute(
            "INSERT INTO avances_efectivo
                 (tarjeta_id, cuenta_ahorro_id, fecha, monto, divisa, tipo_cargo, tasa, cargo)
             VALUES (?, ?, ?, ?, 'DOP', ?, ?, ?);",
            params![tarjeta, cuenta, fecha, monto, tipo, tasa, cargo],
        )
    };

    assert!(insertar(100.005, "exonerado", None, 0.0, "01/10/2026").is_err(), "fracción de céntimo");
    assert!(insertar(0.0, "exonerado", None, 0.0, "01/10/2026").is_err(), "importe cero");
    assert!(insertar(100.0, "exonerado", None, -1.0, "01/10/2026").is_err(), "cargo negativo");
    assert!(insertar(100.0, "fijo", Some(8.0), 5.0, "01/10/2026").is_err(), "fijo con tasa");
    assert!(insertar(100.0, "porcentaje", None, 8.0, "01/10/2026").is_err(), "porcentual sin tasa");
    assert!(insertar(100.0, "exonerado", None, 0.0, "2026-10-01").is_err(), "fecha sin forma");
    assert!(insertar(100.0, "porcentaje", Some(8.0), 8.0, "01/10/2026").is_ok(), "el válido entra");
}

#[test]
fn c126_la_lista_de_avances_de_una_tarjeta_los_devuelve_del_mas_reciente() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let otra = crear_tarjeta(0.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 0.0);
    avance(tarjeta, cuenta, "1350.00", "exonerado", None, None).unwrap();
    avance(tarjeta, cuenta, "2750.00", "porcentaje", Some(6.25), None).unwrap();
    avance(otra, cuenta, "3900.00", "exonerado", None, None).unwrap();

    let lista = crate::obtener_avances_tarjeta(tarjeta).unwrap();

    assert_eq!(lista.len(), 2, "solo los de esa tarjeta");
    assert_importe(lista[0].monto, 2_750.0, "el más reciente primero");
    assert_eq!(lista[0].tasa, Some(6.25));
    assert_eq!(lista[0].cuenta_nombre, "Cuenta Ahorros DOP");
}

#[test]
fn c127_simular_enseña_lo_que_el_avance_va_a_mover_sin_guardar_nada() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 0.0);

    let s = crate::simular_avance_efectivo(
        importe("12500.00"), "DOP".into(), "porcentaje".into(), Some(6.25), None,
    )
    .unwrap();

    assert_importe(s.monto, 12_500.0, "monto");
    assert_importe(s.cargo, 781.25, "cargo");
    assert_importe(s.a_la_tarjeta, 13_281.25, "lo que sube la deuda");
    assert_importe(s.a_la_cuenta, 12_500.0, "lo que recibe la cuenta");
    assert_eq!(filas_de_avances(), 0, "simular no guarda nada");
    assert_importe(balances_tarjeta(tarjeta).0, 0.0, "ni mueve saldos");
    assert_importe(saldo_cuenta_id(cuenta), 0.0, "ni mueve saldos");
}

#[test]
fn c128_la_cifra_que_se_simula_es_exactamente_la_que_se_asienta() {
    // La razón de que exista `simular`: la interfaz enseña una cifra para
    // confirmar, y esa cifra no puede diferir de la que luego se guarda. Se
    // comprueba con importes que redondean, no solo con los redondos.
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 0.0);

    for (monto, p) in [("1234.56", 8.5), ("0.50", 7.0), ("99999.99", 6.25), ("1000.005", 9.99)] {
        let s = crate::simular_avance_efectivo(
            importe(monto), "DOP".into(), "porcentaje".into(), Some(p), None,
        )
        .unwrap();
        let antes = balances_tarjeta(tarjeta).0;

        avance(tarjeta, cuenta, monto, "porcentaje", Some(p), None).unwrap();

        assert_importe(
            balances_tarjeta(tarjeta).0 - antes,
            s.a_la_tarjeta,
            &format!("la deuda con monto {monto} al {p} %"),
        );
        let cargo: f64 = conexion()
            .query_row("SELECT cargo FROM avances_efectivo ORDER BY id DESC LIMIT 1;", [], |r| r.get(0))
            .unwrap();
        assert_importe(cargo, s.cargo, &format!("el cargo con monto {monto} al {p} %"));
    }
}

#[test]
fn c129_simular_rechaza_lo_mismo_que_registrar() {
    let _g = entorno_aislado();
    // Las mismas reglas, por el mismo camino: banda, contradicciones, cero.
    assert!(crate::simular_avance_efectivo(importe("1350.00"), "DOP".into(), "porcentaje".into(), Some(0.8), None).is_err());
    assert!(crate::simular_avance_efectivo(importe("1350.00"), "DOP".into(), "fijo".into(), None, None).is_err());
    assert!(crate::simular_avance_efectivo(importe("1350.00"), "DOP".into(), "exonerado".into(), Some(8.0), None).is_err());
    assert!(crate::simular_avance_efectivo(importe("0.00"), "DOP".into(), "exonerado".into(), None, None).is_err());
    assert!(crate::simular_avance_efectivo(importe("1350.00"), "EUR".into(), "exonerado".into(), None, None).is_err());
}

// =====================================================================
//  Eliminar una cuenta — todas las relaciones que la referencian
// =====================================================================
//
// Las claves ajenas de facturas cobradas, ingresos informales y abonos son
// `SET NULL`: borrar la cuenta se permitía y dejaba el cobro «pagado» sin
// constancia de dónde entró el dinero. Se reprodujo antes de corregir: una
// factura cobrada, la cuenta borrada sin error, la factura pagada con
// `cuenta_ahorro_id` nulo, y borrar esa factura después sin devolver nada a
// ninguna parte.

#[test]
fn c130_una_cuenta_con_una_factura_cobrada_en_ella_no_se_puede_eliminar() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100.0);
    let id = crear_ingreso(factura("Z-001", 5_000.0, 15.0)).unwrap();
    marcar_ingreso_pagado(id, cuenta, "20/09/2026".into(), monto(4_250.0)).unwrap();

    let error = crate::eliminar_cuenta(cuenta).unwrap_err();

    assert!(error.contains("facturas"), "explica por qué: {error}");
    let destino: Option<i64> = conexion()
        .query_row("SELECT cuenta_ahorro_id FROM ingresos WHERE id = ?;", [id], |r| r.get(0))
        .unwrap();
    assert_eq!(destino, Some(cuenta), "la factura conserva su cuenta de depósito");
}

#[test]
fn c131_revertida_la_factura_la_cuenta_vuelve_a_poder_eliminarse() {
    // La guarda no puede volverse un candado: al deshacer el cobro, se levanta.
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100.0);
    let id = crear_ingreso(factura("Z-002", 5_000.0, 15.0)).unwrap();
    marcar_ingreso_pagado(id, cuenta, "20/09/2026".into(), monto(4_250.0)).unwrap();
    assert!(crate::eliminar_cuenta(cuenta).is_err());

    eliminar_ingreso(id, motivo_de_prueba()).unwrap();

    assert!(crate::eliminar_cuenta(cuenta).is_ok());
}

#[test]
fn c132_una_cuenta_con_un_ingreso_informal_cobrado_en_ella_no_se_puede_eliminar() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100.0);
    let id = crear_ingreso_informal("20/09/2026".into(), "Trabajo puntual".into(), monto(3_000.0)).unwrap();
    marcar_informal_pagado(id, cuenta, "20/09/2026".into(), monto(3_000.0)).unwrap();

    let error = crate::eliminar_cuenta(cuenta).unwrap_err();

    assert!(error.contains("informales"), "explica por qué: {error}");
}

#[test]
fn c133_una_cuenta_que_pago_un_abono_no_se_elimina_aunque_borren_el_gasto_de_su_comision() {
    // Antes solo la frenaba, de rebote, el gasto de la comisión del abono. Ese
    // gasto ya no se puede borrar por separado (c136), pero la guarda de la
    // cuenta no debe depender de eso: se reproduce el estado de una base a la
    // que se le quitó la comisión por otra vía, con SQL directo.
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(1_000.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 5_000.0);
    registrar_pago_tarjeta(tarjeta, "20/09/2026".into(), monto(500.0), "DOP".into(), Some(cuenta), 0.0).unwrap();
    let gasto_comision: i64 = conexion()
        .query_row("SELECT gasto_comision_id FROM pagos_tarjeta;", [], |r| r.get(0))
        .unwrap();
    conexion().execute("DELETE FROM gastos WHERE id = ?;", [gasto_comision]).unwrap();
    assert_eq!(total_gastos(), 0, "la comisión ya no existe como gasto");

    let error = crate::eliminar_cuenta(cuenta).unwrap_err();

    assert!(error.contains("abonos"), "explica por qué: {error}");
}

#[test]
fn c134_las_guardas_que_ya_existian_siguen_diciendo_lo_mismo() {
    // La lista sustituyó a tres comprobaciones sueltas; los mensajes que ya
    // conocía el titular no cambian.
    let _g = entorno_aislado();
    let origen = crear_cuenta("Origen", "DOP", 1_000.0);
    let destino = crear_cuenta("Destino", "DOP", 0.0);
    crate::transferir_entre_cuentas("20/09/2026".into(), origen, destino, monto(500.0), monto(500.0), monto(0.0), "x".into()).unwrap();

    let error = crate::eliminar_cuenta(destino).unwrap_err();

    assert!(error.contains("transferencias"), "{error}");
    assert!(error.contains("historial"), "{error}");
}

#[test]
fn c135_una_cuenta_sin_ninguna_relacion_si_se_elimina() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 0.0);
    assert!(crate::eliminar_cuenta(cuenta).is_ok());
}

#[test]
fn c136_la_comision_de_un_abono_no_se_puede_borrar_por_separado() {
    // Antes se permitía: la cuenta recuperaba la comisión, el abono conservaba
    // anotado que había salido, y al revertirlo se devolvía otra vez. Ver
    // `abonos_y_su_comision.md`.
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(30_000.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100_000.0);
    registrar_pago_tarjeta(
        tarjeta, "14/09/2026".to_string(), monto(12_000.0), "DOP".to_string(), Some(cuenta), 0.0,
    )
    .unwrap();
    let gasto_comision: i64 = conexion()
        .query_row("SELECT gasto_comision_id FROM pagos_tarjeta;", [], |r| r.get(0))
        .unwrap();

    let r = eliminar_gasto(gasto_comision, motivo_de_prueba());

    assert!(r.unwrap_err().contains("comisión de un abono"));
    assert_importe(saldo_cuenta_id(cuenta), 87_976.0, "la cuenta no se movió");
    assert_eq!(total_gastos(), 1, "la comisión sigue existiendo");
    let vinculo: Option<i64> = conexion()
        .query_row("SELECT gasto_comision_id FROM pagos_tarjeta;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(vinculo, Some(gasto_comision), "y el abono sigue enlazado a ella");
    let casos: i64 = conexion().query_row("SELECT COUNT(*) FROM correcciones;", [], |r| r.get(0)).unwrap();
    assert_eq!(casos, 0, "un borrado rechazado no abre caso");
}

#[test]
fn c137_tras_el_rechazo_revertir_el_abono_deja_la_cuenta_exactamente_como_estaba() {
    // La vía correcta sigue funcionando y no devuelve nada de más.
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(30_000.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 100_000.0);
    registrar_pago_tarjeta(
        tarjeta, "14/09/2026".to_string(), monto(12_000.0), "DOP".to_string(), Some(cuenta), 0.0,
    )
    .unwrap();
    let abono = ultimo_abono();
    let gasto_comision: i64 = conexion()
        .query_row("SELECT gasto_comision_id FROM pagos_tarjeta;", [], |r| r.get(0))
        .unwrap();
    assert!(eliminar_gasto(gasto_comision, motivo_de_prueba()).is_err());

    revertir_abono_tarjeta(abono, motivo_de_prueba()).unwrap();

    assert_importe(saldo_cuenta_id(cuenta), 100_000.0, "ni un centavo de más");
    assert_importe(balances_tarjeta(tarjeta).0, 30_000.0, "la deuda vuelve");
    assert_eq!(total_gastos(), 0, "y la comisión se fue con el abono");
}

#[test]
fn c138_un_gasto_con_bonificacion_se_puede_borrar_porque_el_vinculo_solo_informa() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(10_000.0, 0.0);
    let categoria: i64 = conexion()
        .query_row("SELECT id FROM categorias ORDER BY id LIMIT 1;", [], |r| r.get(0))
        .unwrap();
    let gasto = crear_gasto(GastoInput {
        fecha: "09/09/2026".to_string(),
        monto: monto(1_234.56),
        divisa: "DOP".to_string(),
        descripcion: "Compra".to_string(),
        categoria_id: categoria,
        metodo_pago: "tarjeta".to_string(),
        es_lbtr: false,
        tarjeta_id: Some(tarjeta),
        cuenta_ahorro_id: None,
        tasa_cambio: None,
    })
    .unwrap();
    crate::crear_bonificacion(
        "09/09/2026".to_string(), tarjeta, monto(61.73), "DOP".to_string(),
        "Cashback".to_string(), Some(gasto),
    )
    .unwrap();

    let r = eliminar_gasto(gasto, motivo_de_prueba());

    assert!(r.is_ok(), "la bonificación no es una operación que dependa del gasto: {r:?}");
}

#[test]
#[ignore = "manual: vuelca los datos de una base REAL para comparar vistas (herramientas/comparar_vistas)"]
fn volcado_de_datos_para_comparar_vistas() {
    // Lo que leen las vistas del frontend, tal como lo devuelven los comandos,
    // escrito en un JSON que **queda en tu equipo**: el repositorio no contiene
    // datos, solo este código. Uso y limpieza en `herramientas/comparar_vistas/README.md`.
    //
    //   VOLCADO_DB=/ruta/copia.db VOLCADO_CAPITAL=/ruta/capital.json \
    //   VOLCADO_SALIDA=/ruta/volcado.json cargo test volcado_de_datos -- --ignored
    //
    // Trabaja siempre sobre una COPIA en un HOME temporal: no toca la base viva.
    use serde_json::{json, to_value, Map, Value};
    let var = |n: &str| std::env::var(n).unwrap_or_else(|_| panic!("falta la variable {n}"));
    let (db_copia, capital, salida) = (var("VOLCADO_DB"), var("VOLCADO_CAPITAL"), var("VOLCADO_SALIDA"));

    let _g = bloquear_entorno();
    let raiz = std::env::temp_dir().join(format!("volcado-vistas-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&raiz);
    std::fs::create_dir_all(&raiz).unwrap();
    std::env::set_var("HOME", &raiz);
    let destino = std::path::PathBuf::from(db_sql::obtener_ruta_db());
    std::fs::create_dir_all(destino.parent().unwrap()).unwrap();
    std::fs::copy(&db_copia, &destino).unwrap();
    let json_capital = std::path::PathBuf::from(crate::db_nosql::obtener_ruta_nosql("capital"));
    std::fs::create_dir_all(json_capital.parent().unwrap()).unwrap();
    std::fs::copy(&capital, &json_capital).unwrap();
    db_sql::inicializar_db().expect("preparar el esquema sobre la copia");

    let mut v = Map::new();
    macro_rules! leer { ($n:expr, $e:expr) => {
        v.insert($n.to_string(), to_value($e.unwrap_or_else(|e| panic!("{} falló: {e}", $n))).unwrap());
    }; }
    leer!("obtener_categorias", crate::obtener_categorias());
    leer!("obtener_clientes", crate::obtener_clientes());
    leer!("obtener_ingresos", crate::obtener_ingresos());
    leer!("obtener_ingresos_informales", crate::obtener_ingresos_informales());
    leer!("obtener_gastos", crate::obtener_gastos());
    leer!("obtener_tarjetas", crate::obtener_tarjetas());
    leer!("obtener_cuentas", crate::obtener_cuentas());
    leer!("obtener_transacciones_cuentas", crate::obtener_transacciones_cuentas());
    leer!("obtener_suscripciones", crate::obtener_suscripciones());
    leer!("obtener_prestamos", crate::obtener_prestamos());
    leer!("obtener_bonificaciones", crate::obtener_bonificaciones());
    leer!("obtener_correcciones", crate::obtener_correcciones());
    v.insert("obtener_capital".into(), crate::obtener_capital().expect("capital"));
    v.insert("listar_respaldos".into(), to_value(crate::listar_respaldos()).unwrap());

    let mut por_tarjeta = Map::new();
    for t in v["obtener_tarjetas"].as_array().unwrap().clone() {
        let id = t["id"].as_i64().unwrap();
        por_tarjeta.insert(id.to_string(), json!({
            "abonos": to_value(crate::obtener_abonos_tarjeta(id).expect("abonos")).unwrap(),
            "avances": to_value(crate::obtener_avances_tarjeta(id).expect("avances")).unwrap(),
        }));
    }
    v.insert("por_tarjeta".into(), Value::Object(por_tarjeta));
    let mut por_prestamo = Map::new();
    for p in v["obtener_prestamos"].as_array().unwrap().clone() {
        let id = p["id"].as_i64().unwrap();
        por_prestamo.insert(id.to_string(), to_value(crate::obtener_movimientos_prestamo(id).expect("movimientos")).unwrap());
    }
    v.insert("por_prestamo".into(), Value::Object(por_prestamo));

    std::fs::write(&salida, serde_json::to_string(&Value::Object(v)).unwrap()).unwrap();
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn c95_crear_tarjeta_guarda_cada_importe_por_su_texto_y_en_su_columna() {
    // Ocho importes distintos, para que un cruce de columnas se note; `.005` sube por texto.
    let _g = entorno_aislado();
    let id = crate::crear_tarjeta(
        "Banco".into(), "Visa".into(),
        importe("1000.005"), importe("2000.005"), importe("3000.005"), importe("4000.005"),
        importe("5000.005"), importe("6000.005"), importe("7000.005"), importe("8000.005"),
        15, 5,
    )
    .unwrap();

    assert_importe(columna_tarjeta(id, "limite_pesos"), 1000.01, "límite DOP");
    assert_importe(columna_tarjeta(id, "limite_dolares"), 2000.01, "límite USD");
    assert_importe(columna_tarjeta(id, "limite_sobregiro_pesos"), 3000.01, "sobregiro DOP");
    assert_importe(columna_tarjeta(id, "limite_sobregiro_dolares"), 4000.01, "sobregiro USD");
    assert_importe(columna_tarjeta(id, "balance_pesos"), 5000.01, "balance DOP");
    assert_importe(columna_tarjeta(id, "balance_dolares"), 6000.01, "balance USD");
    assert_importe(columna_tarjeta(id, "balance_corte_pesos"), 7000.01, "corte DOP");
    assert_importe(columna_tarjeta(id, "balance_corte_dolares"), 8000.01, "corte USD");
}

#[test]
fn c96_actualizar_limites_guarda_cada_importe_por_su_texto_y_distingue_sin_ajuste_de_cero() {
    let _g = entorno_aislado();
    let id = crear_tarjeta(0.0, 0.0);

    crate::actualizar_limites_tarjeta(
        id,
        importe("1000.005"), importe("2000.005"), importe("3000.005"), importe("4000.005"),
        importe("5000.005"), importe("6000.005"),
        Some(importe("0.00")), None, None,
    )
    .unwrap();

    assert_importe(columna_tarjeta(id, "limite_pesos"), 1000.01, "límite DOP");
    assert_importe(columna_tarjeta(id, "limite_dolares"), 2000.01, "límite USD");
    assert_importe(columna_tarjeta(id, "limite_sobregiro_pesos"), 3000.01, "sobregiro DOP");
    assert_importe(columna_tarjeta(id, "limite_sobregiro_dolares"), 4000.01, "sobregiro USD");
    assert_importe(columna_tarjeta(id, "balance_corte_pesos"), 5000.01, "corte DOP");
    assert_importe(columna_tarjeta(id, "balance_corte_dolares"), 6000.01, "corte USD");
    assert_importe(columna_tarjeta(id, "limite_ajustado_pesos"), 0.0, "cero es un tope deliberado");
    let sin: Option<f64> = conexion()
        .query_row("SELECT limite_ajustado_dolares FROM tarjetas WHERE id = ?;", params![id], |r| r.get(0))
        .unwrap();
    assert_eq!(sin, None, "None es «sin ajuste»");
}

#[test]
fn c96b_el_limite_ajustado_tambien_decide_el_centavo_por_su_texto() {
    let _g = entorno_aislado();
    let id = crear_tarjeta(0.0, 0.0);
    crate::actualizar_limites_tarjeta(
        id,
        importe("9000"), importe("9000"), importe("0"), importe("0"), importe("0"), importe("0"),
        Some(importe("1000.005")), Some(importe("2000.005")), None,
    )
    .unwrap();
    assert_importe(columna_tarjeta(id, "limite_ajustado_pesos"), 1000.01, "ajustado DOP");
    assert_importe(columna_tarjeta(id, "limite_ajustado_dolares"), 2000.01, "ajustado USD");
}

#[test]
fn c97_el_gasto_decide_el_centavo_por_su_texto_y_la_divisa_la_declara_el_gasto() {
    // `75.005` por texto sube a 75.01 (por número bajaba a 75.00); en USD cae en la deuda en dólares.
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 100.0);
    let entrada = GastoInput { monto: importe("75.005"), ..compra_en_dolares(tarjeta) };
    crear_gasto(entrada).unwrap();

    let (pesos, dolares) = balances_tarjeta(tarjeta);
    assert_importe(dolares, 175.01, "la deuda en USD sube el céntimo");
    assert_importe(pesos, 0.0, "la de DOP no se toca");
}

#[test]
fn c98_la_factura_decide_el_centavo_y_la_retencion_por_el_texto_del_total() {
    // 1000.005 por texto sube a 1000.01 (por número bajaba a 1000.0); la retención del 50 % se decide sobre ese total.
    let _g = entorno_aislado();
    let id = crear_ingreso(IngresoInput {
        numero_factura: "T-001".to_string(),
        rnc_cliente: "000000000".to_string(),
        nombre_cliente: "Cliente Ejemplo".to_string(),
        fecha_emision: "16/09/2026".to_string(),
        monto_total: importe("1000.005"),
        porcentaje_retencion: 50.0,
    })
    .unwrap();

    let total: f64 = conexion()
        .query_row("SELECT monto_total FROM ingresos WHERE id = ?;", params![id], |r| r.get(0))
        .unwrap();
    assert_importe(total, 1000.01, "el total sube el céntimo");
    assert_importe(retencion_de(id), 500.01, "50 % de 1000.01 = 500.005 → 500.01 (sobre 1000.0 habría sido 500.00)");
}

#[test]
fn c110_el_financiamiento_guarda_sus_importes_por_el_texto_y_el_saldo_ausente_es_el_monto() {
    // `.005` sube por texto; sin saldo declarado se asume el monto ya decidido al céntimo.
    let _g = entorno_aislado();
    let id = crate::crear_prestamo(crate::PrestamoInput {
        tipo_prestamo: "flexible".into(),
        monto_prestamo: importe("2000.005"),
        institucion_financiera: "Banco Ejemplo".into(),
        tasa_actual: 12.0,
        cuotas_totales: None,
        cuotas_pendientes: None,
        monto_cuota: importe("50.005"),
        dia_pago: 25,
        saldo_actual: None,
        limite_credito: Some(importe("3000.005")),
    })
    .unwrap();

    let leer = |col: &str| -> f64 {
        conexion()
            .query_row(&format!("SELECT {col} FROM prestamos WHERE id = ?;"), [id], |r| r.get(0))
            .unwrap()
    };
    assert_importe(leer("monto_prestamo"), 2000.01, "monto");
    assert_importe(leer("saldo_actual"), 2000.01, "saldo ausente = monto");
    assert_importe(leer("monto_cuota"), 50.01, "cuota");
    assert_importe(leer("limite_credito"), 3000.01, "límite");

    crate::actualizar_prestamo(crate::ActualizarPrestamoInput {
        id, tasa_actual: 12.0, monto_cuota: importe("60.005"), dia_pago: 25,
        limite_credito: Some(importe("4000.005")), tarjeta_id: None,
    })
    .unwrap();
    assert_importe(leer("monto_cuota"), 60.01, "cuota corregida");
    assert_importe(leer("limite_credito"), 4000.01, "límite corregido");
}

// --- A-01: un capital ilegible no se convierte en un capital vacío ---

fn ruta_del_capital() -> std::path::PathBuf {
    std::path::PathBuf::from(crate::db_nosql::obtener_ruta_nosql("capital"))
}

/// Deja el capital como si nunca hubiera existido (archivo o directorio). `entorno_aislado` solo reinicia
/// la base SQL: un capital sembrado por otra prueba seguiría ahí, y uno roto de esta rompería a las demás.
fn quitar_capital() {
    let ruta = ruta_del_capital();
    if ruta.is_dir() { let _ = std::fs::remove_dir_all(&ruta); } else { let _ = std::fs::remove_file(&ruta); }
    let _ = std::fs::remove_file(format!("{}.tmp", ruta.display()));
}

fn capital_valido() -> serde_json::Value {
    serde_json::json!({ "propiedades": { "inmobiliario": [], "vehiculos": [], "maquinaria": [] }, "certificados": [], "bolsa": [] })
}

#[test]
fn c111_sin_archivo_de_capital_se_da_la_estructura_inicial() {
    let _g = entorno_aislado();
    quitar_capital();
    assert!(!ruta_del_capital().exists());
    let c = crate::db_nosql::leer_coleccion("capital").expect("un archivo ausente no es un error");
    assert!(c["certificados"].as_array().unwrap().is_empty());
    assert!(crate::obtener_capital().is_ok());
    // Una colección ausente que no es el capital empieza vacía.
    assert_eq!(crate::db_nosql::leer_coleccion("otra").unwrap(), serde_json::json!([]));
    quitar_capital();
}

#[test]
fn c112_un_capital_con_json_dañado_da_error_y_no_se_sobrescribe() {
    let _g = entorno_aislado();
    quitar_capital();
    let ruta = ruta_del_capital();
    std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
    let dañado = r#"{"certificados": [ {"nombre": "sintético", "monto": 1"#; // truncado
    std::fs::write(&ruta, dañado).unwrap();

    let e = crate::db_nosql::leer_coleccion("capital").expect_err("JSON dañado debe fallar");
    assert!(e.contains("dañado"), "mensaje: {e}");
    assert!(crate::obtener_capital().is_err(), "la pestaña no debe mostrar «sin datos»");
    let g = crate::guardar_capital(capital_valido());
    assert!(g.is_err(), "no se guarda sobre un archivo que no se pudo leer");

    assert_eq!(std::fs::read_to_string(&ruta).unwrap(), dañado, "el original quedó intacto");
    assert!(!std::path::Path::new(&format!("{}.tmp", ruta.display())).exists(), "ni siquiera se escribió el temporal");
    quitar_capital();
}

#[test]
fn c113_un_capital_vacio_de_cero_bytes_tambien_es_un_archivo_dañado() {
    let _g = entorno_aislado();
    quitar_capital();
    let ruta = ruta_del_capital();
    std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
    std::fs::write(&ruta, "").unwrap();

    assert!(crate::db_nosql::leer_coleccion("capital").is_err());
    assert!(crate::guardar_capital(capital_valido()).is_err());
    assert_eq!(std::fs::read(&ruta).unwrap().len(), 0, "sigue vacío: no se tocó");
    quitar_capital();
}

#[test]
fn c114_un_error_de_lectura_del_archivo_tampoco_produce_un_guardado() {
    // Un directorio donde debería haber un archivo: existe, pero no se puede leer como texto.
    let _g = entorno_aislado();
    quitar_capital();
    let ruta = ruta_del_capital();
    std::fs::create_dir_all(&ruta).unwrap();

    let e = crate::db_nosql::leer_coleccion("capital").expect_err("no se puede leer");
    assert!(e.contains("No se pudo leer"), "mensaje: {e}");
    assert!(crate::guardar_capital(capital_valido()).is_err());
    assert!(ruta.is_dir(), "lo que había sigue ahí");
    quitar_capital();
}

#[test]
fn c115_con_el_archivo_sano_lee_y_guarda_como_siempre() {
    let _g = entorno_aislado();
    quitar_capital();
    crate::guardar_capital(capital_valido()).expect("guardar");
    assert!(crate::db_nosql::leer_coleccion("capital").is_ok());
    assert!(crate::obtener_capital().is_ok());
    quitar_capital();
}

// --- A-03, vertical «catálogos» (categorías y clientes): caracterización antes de extraer ---
//
// Fijan lo que los seis comandos hacen HOY, con sus mensajes exactos, para que la extracción a
// dominio / caso de uso / adaptador no cambie nada observable.

fn nombres_de_categorias() -> Vec<String> {
    crate::obtener_categorias().unwrap().into_iter().map(|c| c.nombre).collect()
}

#[test]
fn k1_una_categoria_se_crea_recortada_y_se_lista_ordenada_por_nombre() {
    let _g = entorno_aislado();
    let c = crate::crear_categoria("  Zapatería  ".into()).unwrap();
    assert_eq!(c.nombre, "Zapatería");
    assert!(c.id > 0);
    crate::crear_categoria("Academia".into()).unwrap();
    let nombres = nombres_de_categorias();
    let a = nombres.iter().position(|n| n == "Academia").unwrap();
    let z = nombres.iter().position(|n| n == "Zapatería").unwrap();
    assert!(a < z, "orden ascendente por nombre: {nombres:?}");
    let mut ordenados = nombres.clone();
    ordenados.sort();
    assert_eq!(nombres, ordenados, "el orden es el binario de SQLite");
}

#[test]
fn k2_una_categoria_vacia_o_repetida_se_rechaza_con_su_mensaje() {
    let _g = entorno_aislado();
    assert_eq!(crate::crear_categoria("   ".into()).unwrap_err(), "El nombre de la categoría no puede estar vacío.");
    crate::crear_categoria("Mascotas".into()).unwrap();
    for repetido in ["Mascotas", "mascotas", "MASCOTAS", "  mascotas "] {
        assert_eq!(crate::crear_categoria(repetido.into()).unwrap_err(), "La categoría ya existe.", "aceptó «{repetido}»");
    }
    let antes = nombres_de_categorias().len();
    assert!(crate::crear_categoria("Otros".into()).is_err(), "«Otros» ya existe de fábrica");
    assert_eq!(nombres_de_categorias().len(), antes);
}

fn gasto_en_categoria(categoria_id: i64) {
    conexion()
        .execute(
            "INSERT INTO gastos (fecha, monto, divisa, descripcion, categoria_id, metodo_pago, costo_adicional)
             VALUES ('08/09/2026', 10.0, 'DOP', 'Prueba', ?, 'efectivo', 0.0);",
            [categoria_id],
        )
        .expect("insertar gasto de prueba");
}

#[test]
fn k3_eliminar_categoria_respeta_otros_los_gastos_y_borra_la_que_esta_libre() {
    let _g = entorno_aislado();
    let otros = crate::obtener_categorias().unwrap().into_iter().find(|c| c.nombre == "Otros").expect("«Otros» existe de fábrica").id;
    assert_eq!(
        crate::eliminar_categoria(otros).unwrap_err(),
        "No se puede eliminar la categoría de sistema 'Otros'."
    );

    let con_gastos = crate::crear_categoria("Con gastos".into()).unwrap().id;
    gasto_en_categoria(con_gastos);
    assert_eq!(
        crate::eliminar_categoria(con_gastos).unwrap_err(),
        "No se puede eliminar la categoría porque tiene gastos registrados asociados."
    );
    assert!(nombres_de_categorias().contains(&"Con gastos".to_string()));

    let libre = crate::crear_categoria("Libre".into()).unwrap().id;
    crate::eliminar_categoria(libre).unwrap();
    assert!(!nombres_de_categorias().contains(&"Libre".to_string()));
}

#[test]
fn k4_eliminar_una_categoria_inexistente_falla_sin_borrar_nada() {
    let _g = entorno_aislado();
    let antes = nombres_de_categorias().len();
    assert!(crate::eliminar_categoria(987_654).is_err());
    assert_eq!(nombres_de_categorias().len(), antes);
}

#[test]
fn k5_un_cliente_se_crea_recortado_y_se_lista_por_nombre() {
    let _g = entorno_aislado();
    let a = crate::crear_cliente("  101010101 ".into(), "  Beta SRL ".into()).unwrap();
    assert_eq!((a.rnc.as_str(), a.nombre.as_str()), ("101010101", "Beta SRL"));
    crate::crear_cliente("202020202".into(), "Alfa SRL".into()).unwrap();
    let nombres: Vec<String> = crate::obtener_clientes().unwrap().into_iter().map(|c| c.nombre).collect();
    assert_eq!(nombres, vec!["Alfa SRL".to_string(), "Beta SRL".to_string()]);
}

#[test]
fn k6_un_cliente_sin_datos_o_con_rnc_repetido_se_rechaza_con_su_mensaje() {
    let _g = entorno_aislado();
    for (rnc, nombre) in [("", "Algo"), ("123", ""), ("  ", "  ")] {
        assert_eq!(
            crate::crear_cliente(rnc.into(), nombre.into()).unwrap_err(),
            "RNC y nombre no pueden estar vacíos."
        );
    }
    crate::crear_cliente("303030303".into(), "Gamma".into()).unwrap();
    assert_eq!(
        crate::crear_cliente(" 303030303 ".into(), "Otro nombre".into()).unwrap_err(),
        "Ya existe un cliente con este RNC."
    );
    assert_eq!(crate::obtener_clientes().unwrap().len(), 1);
}

#[test]
fn k7_eliminar_cliente_exige_que_no_tenga_facturas() {
    let _g = entorno_aislado();
    let con_factura = crear_ingreso(factura("K-001", 1000.0, 15.0)).unwrap();
    let _ = con_factura;
    let cliente = crate::obtener_clientes().unwrap().into_iter().next().expect("la factura creó su cliente").id;
    assert_eq!(
        crate::eliminar_cliente(cliente).unwrap_err(),
        "No se puede eliminar el cliente porque tiene facturas registradas."
    );
    assert_eq!(crate::obtener_clientes().unwrap().len(), 1);

    let libre = crate::crear_cliente("404040404".into(), "Delta".into()).unwrap().id;
    crate::eliminar_cliente(libre).unwrap();
    assert_eq!(crate::obtener_clientes().unwrap().len(), 1);
}

// --- A-03, vertical «cuentas»: caracterización antes de extraer ---
//
// Fijan lo que `crear_cuenta`, `actualizar_cuenta`, `obtener_cuentas` y `obtener_transacciones_cuentas`
// hacen HOY. (Transferir y eliminar cuentas ya pasan por casos de uso y tienen sus pruebas.)

fn columnas_de_cuenta(id: i64) -> (String, Option<String>, Option<f64>, f64) {
    conexion()
        .query_row(
            "SELECT nombre, entidad, comision_pago_impuestos, balance_actual FROM cuentas_ahorro WHERE id = ?;",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("leer cuenta")
}

fn total_de_cuentas() -> usize {
    crate::obtener_cuentas().unwrap().len()
}

#[test]
fn m1_una_cuenta_se_crea_recortada_con_su_entidad_y_su_saldo() {
    let _g = entorno_aislado();
    let id = crate::crear_cuenta(
        "  Cuenta Prueba  ".into(), "USD".into(), importe("250.50"), Some("  Banco Ejemplo ".into()), Some(importe("12.00")),
    )
    .unwrap();
    let (nombre, entidad, comision, saldo) = columnas_de_cuenta(id);
    assert_eq!(nombre, "Cuenta Prueba");
    assert_eq!(entidad.as_deref(), Some("Banco Ejemplo"));
    assert_importe(comision.unwrap(), 12.0, "comisión");
    assert_importe(saldo, 250.5, "saldo inicial");
    let divisa: String = conexion()
        .query_row("SELECT divisa FROM cuentas_ahorro WHERE id = ?;", params![id], |r| r.get(0))
        .unwrap();
    assert_eq!(divisa, "USD");
}

#[test]
fn m2_una_entidad_en_blanco_se_guarda_como_nula_igual_que_una_ausente() {
    let _g = entorno_aislado();
    let a = crate::crear_cuenta("Con espacios".into(), "DOP".into(), monto(0.0), Some("   ".into()), None).unwrap();
    let b = crate::crear_cuenta("Sin entidad".into(), "DOP".into(), monto(0.0), None, None).unwrap();
    assert_eq!(columnas_de_cuenta(a).1, None);
    assert_eq!(columnas_de_cuenta(b).1, None);
}

#[test]
fn m3_crear_una_cuenta_rechaza_nombre_vacio_y_comision_negativa_con_su_mensaje_y_en_ese_orden() {
    let _g = entorno_aislado();
    assert_eq!(
        crate::crear_cuenta("   ".into(), "DOP".into(), monto(0.0), None, None).unwrap_err(),
        "El nombre de la cuenta no puede estar vacío."
    );
    assert_eq!(
        crate::crear_cuenta("Negativa".into(), "DOP".into(), monto(0.0), None, Some(importe("-0.01"))).unwrap_err(),
        "La comisión por pago de impuestos no puede ser negativa."
    );
    // El nombre se comprueba primero.
    assert_eq!(
        crate::crear_cuenta(" ".into(), "DOP".into(), monto(0.0), None, Some(importe("-5"))).unwrap_err(),
        "El nombre de la cuenta no puede estar vacío."
    );
    // Una comisión de cero es válida (el banco no cobra), y distinta de ninguna.
    let id = crate::crear_cuenta("Cero".into(), "DOP".into(), monto(0.0), None, Some(importe("0.00"))).unwrap();
    assert_importe(columnas_de_cuenta(id).2.unwrap(), 0.0, "cero declarado");
    assert_eq!(total_de_cuentas(), 1 + cuentas_de_fabrica(), "las rechazadas no dejaron nada");
}

fn cuentas_de_fabrica() -> usize {
    crate::obtener_cuentas().unwrap().iter().filter(|c| c.nombre.starts_with("Efectivo")).count()
}

#[test]
fn m4_una_divisa_ilegal_o_un_nombre_repetido_los_rechaza_el_esquema_y_no_dejan_nada() {
    let _g = entorno_aislado();
    let antes = total_de_cuentas();
    let e = crate::crear_cuenta("Euro".into(), "EUR".into(), monto(0.0), None, None).unwrap_err();
    assert!(e.contains("CHECK constraint failed"), "mensaje: {e}");
    crate::crear_cuenta("Repetida".into(), "DOP".into(), monto(0.0), None, None).unwrap();
    let e = crate::crear_cuenta("Repetida".into(), "DOP".into(), monto(0.0), None, None).unwrap_err();
    assert!(e.contains("UNIQUE constraint failed"), "mensaje: {e}");
    assert_eq!(total_de_cuentas(), antes + 1);
}

#[test]
fn m5_actualizar_una_cuenta_corrige_nombre_entidad_y_comision_sin_tocar_el_saldo() {
    let _g = entorno_aislado();
    let id = crear_cuenta("Original", "DOP", 777.0);
    crate::actualizar_cuenta(id, "  Renombrada ".into(), Some(" Banco Nuevo ".into()), Some(importe("5.00"))).unwrap();
    let (nombre, entidad, comision, saldo) = columnas_de_cuenta(id);
    assert_eq!((nombre.as_str(), entidad.as_deref()), ("Renombrada", Some("Banco Nuevo")));
    assert_importe(comision.unwrap(), 5.0, "comisión");
    assert_importe(saldo, 777.0, "el saldo no se toca");

    // Sin entidad ni comisión las deja nulas: corregir con vacío borra lo declarado.
    crate::actualizar_cuenta(id, "Renombrada".into(), Some("  ".into()), None).unwrap();
    let (_, entidad, comision, _) = columnas_de_cuenta(id);
    assert_eq!(entidad, None);
    assert!(comision.is_none());
}

#[test]
fn m6_actualizar_rechaza_vacio_negativo_e_inexistente_con_su_mensaje() {
    let _g = entorno_aislado();
    let id = crear_cuenta("Estable", "DOP", 10.0);
    assert_eq!(
        crate::actualizar_cuenta(id, "  ".into(), None, None).unwrap_err(),
        "El nombre de la cuenta no puede estar vacío."
    );
    assert_eq!(
        crate::actualizar_cuenta(id, "Estable".into(), None, Some(importe("-1"))).unwrap_err(),
        "La comisión por pago de impuestos no puede ser negativa."
    );
    assert_eq!(
        crate::actualizar_cuenta(987_654, "Nada".into(), None, None).unwrap_err(),
        "No se encontró la cuenta 987654."
    );
    assert_eq!(columnas_de_cuenta(id).0, "Estable", "lo rechazado no cambió nada");
}

#[test]
fn m7_las_cuentas_se_listan_por_nombre_con_todos_sus_campos() {
    let _g = entorno_aislado();
    crate::crear_cuenta("Zeta".into(), "USD".into(), importe("1.50"), Some("Banco Z".into()), Some(importe("3.00"))).unwrap();
    crate::crear_cuenta("Alfa".into(), "DOP".into(), importe("2.25"), None, None).unwrap();
    let todas = crate::obtener_cuentas().unwrap();
    let nombres: Vec<&str> = todas.iter().map(|c| c.nombre.as_str()).collect();
    let mut ordenados = nombres.clone();
    ordenados.sort();
    assert_eq!(nombres, ordenados, "orden binario por nombre");
    let zeta = todas.iter().find(|c| c.nombre == "Zeta").unwrap();
    assert_eq!((zeta.divisa.as_str(), zeta.entidad.as_deref()), ("USD", Some("Banco Z")));
    assert_importe(zeta.balance_actual, 1.5, "saldo");
    assert_importe(zeta.comision_pago_impuestos.unwrap(), 3.0, "comisión");
    let alfa = todas.iter().find(|c| c.nombre == "Alfa").unwrap();
    assert!(alfa.entidad.is_none() && alfa.comision_pago_impuestos.is_none());
}

#[test]
fn m8_las_transacciones_entre_cuentas_salen_de_la_mas_nueva_a_la_mas_vieja_con_los_nombres() {
    let _g = entorno_aislado();
    let a = crear_cuenta("Origen DOP", "DOP", 1000.0);
    let b = crear_cuenta("Destino USD", "USD", 0.0);
    crate::transferir_entre_cuentas("01/09/2026".into(), a, b, importe("600.00"), importe("10.00"), importe("1.00"), "Primera".into()).unwrap();
    crate::transferir_entre_cuentas("02/09/2026".into(), a, b, importe("120.00"), importe("2.00"), importe("0.00"), "Segunda".into()).unwrap();
    let t = crate::obtener_transacciones_cuentas().unwrap();
    assert_eq!(t.len(), 2);
    assert_eq!(t[0].descripcion.as_deref(), Some("Segunda"), "la más nueva primero");
    assert_eq!(t[1].descripcion.as_deref(), Some("Primera"));
    assert_eq!((t[0].cuenta_origen_nombre.as_str(), t[0].cuenta_destino_nombre.as_str()), ("Origen DOP", "Destino USD"));
    assert_eq!((t[0].cuenta_origen_id, t[0].cuenta_destino_id), (a, b));
    assert_importe(t[1].monto_origen, 600.0, "origen");
    assert_importe(t[1].monto_destino, 10.0, "destino");
    assert_importe(t[1].tasa_cambio, 10.0 / 600.0, "tasa");
    assert_importe(t[1].cargo, 1.0, "cargo");
}

// --- A-03, vertical «ingresos formales» (alta y cobro): caracterización antes de extraer ---
//
// Completan c70–c78b (que ya cubren lo principal) con los mensajes exactos, la forma de la fila, el listado y la
// atomicidad. Fijan lo que `crear_ingreso`, `marcar_ingreso_pagado` y `obtener_ingresos` hacen HOY.

fn fila_de_factura(id: i64) -> (String, i64, String, f64, f64, f64, String) {
    conexion()
        .query_row(
            "SELECT numero_factura, cliente_id, fecha_emision, monto_total, porcentaje_retencion, monto_retenido, estatus
             FROM ingresos WHERE id = ?;",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
        )
        .expect("leer factura")
}

fn input_de_factura(numero: &str, rnc: &str, nombre: &str, total: &str, porcentaje: f64) -> IngresoInput {
    IngresoInput {
        numero_factura: numero.to_string(),
        rnc_cliente: rnc.to_string(),
        nombre_cliente: nombre.to_string(),
        fecha_emision: "05/10/2026".to_string(),
        monto_total: importe(total),
        porcentaje_retencion: porcentaje,
    }
}

fn clientes_con_rnc(rnc: &str) -> i64 {
    conexion().query_row("SELECT COUNT(*) FROM clientes WHERE rnc = ?;", params![rnc], |r| r.get(0)).unwrap()
}

#[test]
fn n1_una_factura_se_guarda_emitida_con_su_cliente_nuevo_y_su_retencion() {
    let _g = entorno_aislado();
    let id = crear_ingreso(input_de_factura("N-001", "131313131", "Cliente Nuevo", "2000", 15.0)).unwrap();
    let (numero, cliente_id, fecha, total, porcentaje, retenido, estatus) = fila_de_factura(id);
    assert_eq!((numero.as_str(), fecha.as_str(), estatus.as_str()), ("N-001", "05/10/2026", "emitida"));
    assert_importe(total, 2000.0, "total");
    assert_importe(porcentaje, 15.0, "porcentaje");
    assert_importe(retenido, 300.0, "retención");
    let (rnc, nombre): (String, String) = conexion()
        .query_row("SELECT rnc, nombre FROM clientes WHERE id = ?;", params![cliente_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap();
    assert_eq!((rnc.as_str(), nombre.as_str()), ("131313131", "Cliente Nuevo"));
}

#[test]
fn n2_el_cliente_existente_se_reutiliza_por_rnc_y_conserva_su_nombre() {
    let _g = entorno_aislado();
    let a = crear_ingreso(input_de_factura("N-002", "141414141", "Nombre Original", "100", 0.0)).unwrap();
    let b = crear_ingreso(input_de_factura("N-003", "141414141", "Otro Nombre", "100", 0.0)).unwrap();
    assert_eq!(fila_de_factura(a).1, fila_de_factura(b).1, "mismo cliente");
    assert_eq!(clientes_con_rnc("141414141"), 1);
    let nombre: String = conexion()
        .query_row("SELECT nombre FROM clientes WHERE rnc = '141414141';", [], |r| r.get(0))
        .unwrap();
    assert_eq!(nombre, "Nombre Original", "el nombre de la segunda factura no lo cambia");
}

#[test]
fn n3_una_factura_repetida_se_rechaza_con_su_mensaje_antes_de_crear_ningun_cliente() {
    let _g = entorno_aislado();
    crear_ingreso(input_de_factura("N-004", "151515151", "Primero", "100", 0.0)).unwrap();
    for repetido in ["N-004", "n-004"] {
        let e = crear_ingreso(input_de_factura(repetido, "161616161", "Segundo", "100", 0.0)).unwrap_err();
        assert_eq!(e, "El número de factura ya está registrado.");
    }
    assert_eq!(clientes_con_rnc("161616161"), 0, "no se creó el cliente de la rechazada");
}

#[test]
fn n4_hallazgo_la_factura_acepta_un_porcentaje_fuera_de_rango_y_datos_vacios() {
    // **HALLAZGOS, sin corregir** (protocolo del proyecto: documentar y fijar; el cambio se consulta).
    // `crear_ingreso` no valida que el porcentaje de retención esté entre 0 y 100 —acepta 150 y -5, con una
    // retención mayor que el total o negativa— ni que número de factura, RNC y nombre no estén vacíos (el
    // formulario sí los exige, el comando no). Esta prueba describe el comportamiento ACTUAL: si se decide
    // rechazarlos, se invierte.
    let _g = entorno_aislado();
    let alto = crear_ingreso(input_de_factura("N-005", "171717171", "Alto", "200", 150.0)).unwrap();
    assert_importe(fila_de_factura(alto).5, 300.0, "150 % de 200: la retención supera el total");
    let negativo = crear_ingreso(input_de_factura("N-006", "181818181", "Negativo", "200", -5.0)).unwrap();
    assert_importe(fila_de_factura(negativo).5, -10.0, "-5 %: retención negativa");
    assert!(crear_ingreso(input_de_factura("", "", "", "0", 0.0)).is_ok(), "datos vacíos aceptados");
}

#[test]
fn n5_cobrar_una_factura_la_deja_pagada_con_la_cuenta_la_fecha_y_el_importe() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Cobro", "DOP", 500.0);
    let id = crear_ingreso(input_de_factura("N-007", "191919191", "Cli", "2000", 15.0)).unwrap();
    marcar_ingreso_pagado(id, cuenta, "06/10/2026".into(), importe("1700.50")).unwrap();
    let (estatus, institucion, cuenta_id, fecha, recibido): (String, String, i64, String, f64) = conexion()
        .query_row(
            "SELECT estatus, institucion_deposito, cuenta_ahorro_id, fecha_pago, monto_recibido FROM ingresos WHERE id = ?;",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    assert_eq!((estatus.as_str(), institucion.as_str(), cuenta_id, fecha.as_str()), ("pagada", "Cuenta Cobro", cuenta, "06/10/2026"));
    assert_importe(recibido, 1700.5, "recibido");
    assert_importe(saldo_cuenta_id(cuenta), 2200.5, "la cuenta recibe lo mismo que la fila guarda");
}

#[test]
fn n6_los_rechazos_del_cobro_dicen_su_causa_y_no_dejan_nada_a_medias() {
    let _g = entorno_aislado();
    let dop = crear_cuenta("Cuenta DOP", "DOP", 100.0);
    let usd = crear_cuenta("Cuenta USD", "USD", 100.0);
    let id = crear_ingreso(input_de_factura("N-008", "202020202", "Cli", "1500", 0.0)).unwrap();
    let cobrar = |factura: i64, cuenta: i64, monto: &str| marcar_ingreso_pagado(factura, cuenta, "06/10/2026".into(), importe(monto));

    assert_eq!(cobrar(id, 9_999, "10").unwrap_err(), "No se encontró la cuenta 9999.");
    assert_eq!(cobrar(404, dop, "10").unwrap_err(), "No se encontró una factura 404 pendiente de cobro.");
    assert_eq!(
        cobrar(id, usd, "10").unwrap_err(),
        "No se pueden combinar montos en USD y DOP: indique una tasa de cambio para convertirlos."
    );
    assert_eq!(cobrar(id, dop, "-10").unwrap_err(), "El monto -10 no es un número válido.");
    assert_eq!(estatus_de(id), "emitida", "la factura sigue pendiente");
    assert_importe(saldo_cuenta_id(dop), 100.0, "ningún saldo se movió");
    assert_importe(saldo_cuenta_id(usd), 100.0, "ningún saldo se movió");

    cobrar(id, dop, "10").unwrap();
    assert_eq!(
        cobrar(id, dop, "10").unwrap_err(),
        format!("No se encontró una factura {id} pendiente de cobro."),
        "cobrar dos veces no acredita dos veces"
    );
    assert_importe(saldo_cuenta_id(dop), 110.0, "solo se acreditó una vez");
}

#[test]
fn n7_las_facturas_se_listan_de_la_mas_nueva_a_la_mas_vieja_con_los_datos_de_su_cliente() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Lista", "DOP", 0.0);
    let a = crear_ingreso(input_de_factura("N-009", "212121212", "Cliente Uno", "300", 10.0)).unwrap();
    let b = crear_ingreso(input_de_factura("N-010", "232323232", "Cliente Dos", "400", 0.0)).unwrap();
    marcar_ingreso_pagado(a, cuenta, "07/10/2026".into(), importe("270")).unwrap();

    let lista = crate::obtener_ingresos().unwrap();
    assert_eq!(lista.iter().map(|i| i.id).collect::<Vec<_>>(), vec![b, a], "la más nueva primero");
    let uno = &lista[1];
    assert_eq!((uno.numero_factura.as_str(), uno.cliente_nombre.as_str(), uno.cliente_rnc.as_str()), ("N-009", "Cliente Uno", "212121212"));
    assert_eq!(uno.estatus, "pagada");
    assert_eq!((uno.institucion_deposito.as_deref(), uno.fecha_pago.as_deref()), (Some("Cuenta Lista"), Some("07/10/2026")));
    assert_importe(uno.monto_total, 300.0, "total");
    assert_importe(uno.monto_retenido, 30.0, "retenido");
    assert_importe(uno.monto_recibido.unwrap(), 270.0, "recibido");
    let dos = &lista[0];
    assert_eq!(dos.estatus, "emitida");
    assert!(dos.institucion_deposito.is_none() && dos.fecha_pago.is_none() && dos.monto_recibido.is_none());
}

// --- A-03, vertical «ingresos formales» (parte 2: corregir y eliminar): caracterización antes de extraer ---
//
// Completan c85–c94 y c79/c80 con los mensajes exactos, la forma del caso de corrección y los bordes.

fn casos_de_correccion() -> Vec<(String, String, i64, String, Option<f64>, Option<String>)> {
    conexion()
        .prepare("SELECT numero_caso, tipo, referencia_id, descripcion, importe, divisa FROM correcciones ORDER BY id;")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

fn factura_cobrada(numero: &str, cuenta_nombre: &str, saldo: f64) -> (i64, i64) {
    let cuenta = crear_cuenta(cuenta_nombre, "DOP", saldo);
    let id = crear_ingreso(input_de_factura(numero, "909090909", "Cli", "2000", 15.0)).unwrap();
    marcar_ingreso_pagado(id, cuenta, "06/10/2026".into(), importe("1700")).unwrap();
    (id, cuenta)
}

#[test]
fn o1_corregir_una_factura_inexistente_dice_su_mensaje() {
    let _g = entorno_aislado();
    let e = actualizar_ingreso(404, "X".into(), 1, "06/10/2026".into(), importe("10"), 0.0, None, None).unwrap_err();
    assert_eq!(e, "No se encontró la factura 404.");
}

#[test]
fn o2_los_tres_resumenes_de_corregir_son_los_de_siempre() {
    let _g = entorno_aislado();
    // Sin cobrar: no hay dinero que mover.
    let libre = crear_ingreso(input_de_factura("O-001", "808080808", "Cli", "100", 0.0)).unwrap();
    let r = actualizar_ingreso(libre, "O-001".into(), fila_de_factura(libre).1, "06/10/2026".into(), importe("200"), 0.0, None, None).unwrap();
    assert_eq!(r, "Factura corregida.");

    // Cobrada y el ajuste mueve dinero: dice la cuenta, el ajuste con dos decimales y el caso.
    let (id, cuenta) = factura_cobrada("O-002", "Cuenta Resumen", 100.0);
    let cliente = fila_de_factura(id).1;
    let r = actualizar_ingreso(id, "O-002".into(), cliente, "06/10/2026".into(), importe("3000"), 15.0, None, Some(motivo_de_prueba())).unwrap();
    assert_eq!(r, "Factura corregida. Se ajustó «Cuenta Resumen» en DOP 850.00. Caso COR-2026-0001.".replace("2026", &chrono::Local::now().format("%Y").to_string()));
    assert_importe(saldo_cuenta_id(cuenta), 100.0 + 1700.0 + 850.0, "la cuenta recibió el ajuste");

    // Cobrada sin cuenta de depósito que ajustar.
    conexion().execute("UPDATE ingresos SET institucion_deposito = NULL WHERE id = ?;", params![id]).unwrap();
    let r = actualizar_ingreso(id, "O-002".into(), cliente, "06/10/2026".into(), importe("4000"), 15.0, None, Some(motivo_de_prueba())).unwrap();
    assert!(r.starts_with("Factura corregida. No tenía cuenta de depósito que ajustar. Caso COR-"), "{r}");
}

#[test]
fn o3_si_la_cuenta_de_deposito_ya_no_existe_corregir_falla_y_no_deja_nada() {
    let _g = entorno_aislado();
    let (id, cuenta) = factura_cobrada("O-003", "Cuenta Que Se Va", 100.0);
    let cliente = fila_de_factura(id).1;
    conexion().execute("UPDATE cuentas_ahorro SET nombre = 'Otro Nombre' WHERE id = ?;", params![cuenta]).unwrap();

    let e = actualizar_ingreso(id, "O-003".into(), cliente, "06/10/2026".into(), importe("3000"), 15.0, None, Some(motivo_de_prueba())).unwrap_err();
    assert_eq!(
        e,
        "La factura se cobró en «Cuenta Que Se Va», que ya no existe. Corrige o recrea esa cuenta antes de modificar la factura."
    );
    assert_importe(fila_de_factura(id).3, 2000.0, "la factura no cambió (transacción deshecha)");
    assert!(casos_de_correccion().is_empty(), "ni dejó un caso huérfano");
}

#[test]
fn o4_eliminar_una_factura_inexistente_o_con_motivo_corto_dice_su_mensaje() {
    let _g = entorno_aislado();
    assert_eq!(
        eliminar_ingreso(404, motivo_de_prueba()).unwrap_err(),
        "No se encontró factura con identificador 404."
    );
    let id = crear_ingreso(input_de_factura("O-004", "707070707", "Cli", "100", 0.0)).unwrap();
    let e = eliminar_ingreso(id, "  corto ".into()).unwrap_err();
    assert!(e.starts_with("Explica la corrección en al menos 15 caracteres."), "{e}");
    assert!(e.contains("«corto»"), "el motivo recortado va en el mensaje: {e}");
    assert_eq!(fila_de_factura(id).0, "O-004", "no se borró nada");
}

#[test]
fn o5_eliminar_deja_un_caso_con_la_factura_el_total_y_la_divisa_y_devuelve_su_numero() {
    let _g = entorno_aislado();
    let id = crear_ingreso(input_de_factura("O-005", "606060606", "Cli", "250", 0.0)).unwrap();
    let caso = eliminar_ingreso(id, motivo_de_prueba()).unwrap();
    let casos = casos_de_correccion();
    assert_eq!(casos.len(), 1);
    let (numero, tipo, referencia, descripcion, importe_caso, divisa) = &casos[0];
    assert_eq!(&caso, numero);
    assert_eq!((tipo.as_str(), *referencia, descripcion.as_str(), divisa.as_deref()), ("factura", id, "Factura O-005", Some("DOP")));
    assert_importe(importe_caso.unwrap(), 250.0, "total de la factura");
}

#[test]
fn o6_eliminar_una_factura_cobrada_cuya_cuenta_ya_no_existe_la_borra_igual_sin_mover_saldos() {
    // Comportamiento ACTUAL, distinto de corregir (que sí falla): el borrado ajusta «por nombre» y, si no hay
    // cuenta con ese nombre, no hace nada y sigue. Se documenta tal cual.
    let _g = entorno_aislado();
    let (id, cuenta) = factura_cobrada("O-006", "Cuenta Desaparecida", 100.0);
    conexion().execute("UPDATE cuentas_ahorro SET nombre = 'Otro Nombre' WHERE id = ?;", params![cuenta]).unwrap();
    let saldo_antes = saldo_cuenta_id(cuenta);

    eliminar_ingreso(id, motivo_de_prueba()).unwrap();

    assert_importe(saldo_cuenta_id(cuenta), saldo_antes, "ninguna cuenta se tocó");
    assert_eq!(crate::obtener_ingresos().unwrap().len(), 0, "la factura se borró");
}

#[test]
fn o7_eliminar_una_factura_sin_cobrar_no_toca_ningun_saldo() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Intacta", "DOP", 321.0);
    let id = crear_ingreso(input_de_factura("O-007", "505050505", "Cli", "100", 0.0)).unwrap();
    eliminar_ingreso(id, motivo_de_prueba()).unwrap();
    assert_importe(saldo_cuenta_id(cuenta), 321.0, "sin cobro no hay nada que revertir");
}

// --- A-03, vertical «ingresos informales»: caracterización antes de extraer ---
//
// Completan c81–c84b con los mensajes exactos, el listado, la forma de las filas y los bordes. Fijan lo que
// los cinco comandos hacen HOY.

fn informal_por_id(id: i64) -> crate::IngresoInformal {
    crate::obtener_ingresos_informales().unwrap().into_iter().find(|i| i.id == id).expect("informal")
}

#[test]
fn p1_un_informal_se_crea_pendiente_y_se_lista_de_el_mas_nuevo_al_mas_viejo() {
    let _g = entorno_aislado();
    let a = crear_ingreso_informal("01/10/2026".into(), "Primero".into(), importe("100")).unwrap();
    let b = crear_ingreso_informal("02/10/2026".into(), "Segundo".into(), importe("200")).unwrap();
    let lista = crate::obtener_ingresos_informales().unwrap();
    assert_eq!(lista.iter().map(|i| i.id).collect::<Vec<_>>(), vec![b, a]);
    let primero = informal_por_id(a);
    assert_eq!((primero.fecha.as_str(), primero.descripcion.as_str(), primero.estatus.as_str()), ("01/10/2026", "Primero", "pendiente"));
    assert_importe(primero.monto, 100.0, "monto");
    assert!(primero.institucion_deposito.is_none() && primero.fecha_pago.is_none() && primero.monto_recibido.is_none());
}

#[test]
fn p2_hallazgo_un_informal_acepta_monto_cero_o_negativo_y_datos_vacios() {
    // **HALLAZGO, sin corregir** (protocolo: documentar y fijar; el cambio se consulta). `crear_ingreso_informal`
    // no valida que el monto sea positivo ni que fecha y descripción no estén vacías (el formulario sí). Esta
    // prueba describe el comportamiento ACTUAL: si se decide rechazarlos, se invierte.
    let _g = entorno_aislado();
    let cero = crear_ingreso_informal("01/10/2026".into(), "Cero".into(), importe("0")).unwrap();
    let negativo = crear_ingreso_informal("01/10/2026".into(), "Negativo".into(), importe("-50")).unwrap();
    let vacio = crear_ingreso_informal("".into(), "".into(), importe("1")).unwrap();
    assert_importe(informal_por_id(cero).monto, 0.0, "cero aceptado");
    assert_importe(informal_por_id(negativo).monto, -50.0, "un ingreso negativo aceptado");
    assert_eq!(informal_por_id(vacio).descripcion, "", "descripción vacía aceptada");
}

#[test]
fn p3_cobrar_un_informal_deja_la_fila_pagada_con_la_cuenta_la_fecha_y_el_importe() {
    let _g = entorno_aislado();
    let cuenta = crear_cuenta("Cuenta Informal", "DOP", 50.0);
    let id = crear_ingreso_informal("01/10/2026".into(), "Trabajo".into(), importe("300")).unwrap();
    marcar_informal_pagado(id, cuenta, "03/10/2026".into(), importe("275.50")).unwrap();
    let f = informal_por_id(id);
    assert_eq!((f.estatus.as_str(), f.institucion_deposito.as_deref(), f.fecha_pago.as_deref()), ("pagado", Some("Cuenta Informal"), Some("03/10/2026")));
    assert_importe(f.monto_recibido.unwrap(), 275.5, "recibido");
    assert_importe(saldo_cuenta_id(cuenta), 325.5, "la cuenta recibe lo mismo que la fila guarda");
}

#[test]
fn p4_los_rechazos_del_cobro_informal_dicen_su_causa_y_no_dejan_nada_a_medias() {
    let _g = entorno_aislado();
    let dop = crear_cuenta("Cuenta DOP", "DOP", 100.0);
    let usd = crear_cuenta("Cuenta USD", "USD", 100.0);
    let id = crear_ingreso_informal("01/10/2026".into(), "Trabajo".into(), importe("300")).unwrap();
    let cobrar = |informal: i64, cuenta: i64, monto: &str| marcar_informal_pagado(informal, cuenta, "03/10/2026".into(), importe(monto));

    assert_eq!(cobrar(id, 9_999, "10").unwrap_err(), "No se encontró la cuenta 9999.");
    assert_eq!(cobrar(404, dop, "10").unwrap_err(), "No se encontró un ingreso 404 pendiente de cobro.");
    assert_eq!(
        cobrar(id, usd, "10").unwrap_err(),
        "No se pueden combinar montos en USD y DOP: indique una tasa de cambio para convertirlos."
    );
    assert_eq!(cobrar(id, dop, "-10").unwrap_err(), "El monto -10 no es un número válido.");
    assert_eq!(informal_por_id(id).estatus, "pendiente");
    assert_importe(saldo_cuenta_id(dop), 100.0, "ningún saldo se movió");

    cobrar(id, dop, "10").unwrap();
    assert_eq!(cobrar(id, dop, "10").unwrap_err(), format!("No se encontró un ingreso {id} pendiente de cobro."));
    assert_importe(saldo_cuenta_id(dop), 110.0, "solo se acreditó una vez");
}

#[test]
fn p5_el_cobro_en_efectivo_deja_el_ingreso_pagado_en_la_caja_y_toda_divisa_que_no_es_usd_va_a_pesos() {
    let _g = entorno_aislado();
    let (dop, usd) = (balance_cuenta("Efectivo DOP"), balance_cuenta("Efectivo USD"));
    let a = crear_cobro_efectivo_informal("04/10/2026".into(), "En pesos".into(), importe("40"), "DOP".into()).unwrap();
    let b = crear_cobro_efectivo_informal("04/10/2026".into(), "En dólares".into(), importe("5"), "USD".into()).unwrap();
    let c = crear_cobro_efectivo_informal("04/10/2026".into(), "Divisa rara".into(), importe("7"), "EUR".into()).unwrap();

    let fa = informal_por_id(a);
    assert_eq!((fa.estatus.as_str(), fa.institucion_deposito.as_deref(), fa.fecha_pago.as_deref()), ("pagado", Some("Efectivo DOP"), Some("04/10/2026")));
    assert_importe(fa.monto, 40.0, "monto");
    assert_importe(fa.monto_recibido.unwrap(), 40.0, "recibido");
    assert_eq!(informal_por_id(b).institucion_deposito.as_deref(), Some("Efectivo USD"));
    assert_eq!(informal_por_id(c).institucion_deposito.as_deref(), Some("Efectivo DOP"), "otra divisa cae en pesos");
    assert_importe(balance_cuenta("Efectivo DOP"), dop + 47.0, "pesos: 40 + 7");
    assert_importe(balance_cuenta("Efectivo USD"), usd + 5.0, "dólares: 5");
}

#[test]
fn p6_hallazgo_sin_caja_de_efectivo_el_cobro_se_registra_igual_y_no_mueve_ningun_saldo() {
    // **HALLAZGO, sin corregir** (el mismo hueco que H3 resolvió para los gastos en efectivo): este comando sigue
    // localizando la caja **por su nombre** y, si no la encuentra, el ingreso queda «pagado» sin que ningún saldo
    // se mueva ni nadie se entere. Esta prueba describe el comportamiento ACTUAL.
    let _g = entorno_aislado();
    conexion().execute("UPDATE cuentas_ahorro SET nombre = 'Caja renombrada' WHERE nombre = 'Efectivo DOP';", []).unwrap();
    let total_antes: f64 = conexion().query_row("SELECT SUM(balance_actual) FROM cuentas_ahorro;", [], |r| r.get(0)).unwrap();

    let id = crear_cobro_efectivo_informal("04/10/2026".into(), "Sin caja".into(), importe("90"), "DOP".into()).unwrap();

    assert_eq!(informal_por_id(id).estatus, "pagado", "el ingreso consta como cobrado");
    let total_despues: f64 = conexion().query_row("SELECT SUM(balance_actual) FROM cuentas_ahorro;", [], |r| r.get(0)).unwrap();
    assert_importe(total_despues, total_antes, "pero ninguna cuenta recibió nada");
}

#[test]
fn p7_eliminar_un_informal_dice_su_causa_y_deja_un_caso_con_su_descripcion_y_monto() {
    let _g = entorno_aislado();
    assert_eq!(
        eliminar_ingreso_informal(404, motivo_de_prueba()).unwrap_err(),
        "No se encontró ingreso informal con identificador 404."
    );
    let id = crear_ingreso_informal("01/10/2026".into(), "Clase suelta".into(), importe("120")).unwrap();
    let e = eliminar_ingreso_informal(id, " corto ".into()).unwrap_err();
    assert!(e.starts_with("Explica la corrección en al menos 15 caracteres.") && e.contains("«corto»"), "{e}");
    assert_eq!(crate::obtener_ingresos_informales().unwrap().len(), 1, "no se borró nada");

    let caso = eliminar_ingreso_informal(id, motivo_de_prueba()).unwrap();
    let casos = casos_de_correccion();
    let (numero, tipo, referencia, descripcion, importe_caso, divisa) = &casos[0];
    assert_eq!(&caso, numero);
    assert_eq!((tipo.as_str(), *referencia, descripcion.as_str(), divisa.as_deref()), ("ingreso informal", id, "Clase suelta", Some("DOP")));
    assert_importe(importe_caso.unwrap(), 120.0, "monto del ingreso");
    assert!(crate::obtener_ingresos_informales().unwrap().is_empty());
}

#[test]
fn p8_eliminar_un_cobro_en_efectivo_revierte_la_caja_y_con_la_cuenta_desaparecida_borra_igual() {
    let _g = entorno_aislado();
    let antes = balance_cuenta("Efectivo DOP");
    let id = crear_cobro_efectivo_informal("04/10/2026".into(), "En caja".into(), importe("60"), "DOP".into()).unwrap();
    eliminar_ingreso_informal(id, motivo_de_prueba()).unwrap();
    assert_importe(balance_cuenta("Efectivo DOP"), antes, "la caja vuelve donde estaba");

    // Con la cuenta renombrada el borrado no encuentra adónde revertir: borra igual y no mueve saldos.
    let otro = crear_cobro_efectivo_informal("04/10/2026".into(), "Otra".into(), importe("30"), "DOP".into()).unwrap();
    conexion().execute("UPDATE cuentas_ahorro SET nombre = 'Caja renombrada' WHERE nombre = 'Efectivo DOP';", []).unwrap();
    let con_cobro: f64 = conexion().query_row("SELECT balance_actual FROM cuentas_ahorro WHERE nombre = 'Caja renombrada';", [], |r| r.get(0)).unwrap();
    eliminar_ingreso_informal(otro, motivo_de_prueba()).unwrap();
    let despues: f64 = conexion().query_row("SELECT balance_actual FROM cuentas_ahorro WHERE nombre = 'Caja renombrada';", [], |r| r.get(0)).unwrap();
    assert_importe(despues, con_cobro, "ninguna cuenta se tocó");
}

#[test]
fn p9_eliminar_un_cobro_en_efectivo_en_dolares_revierte_la_caja_de_dolares() {
    let _g = entorno_aislado();
    let (dop, usd) = (balance_cuenta("Efectivo DOP"), balance_cuenta("Efectivo USD"));
    let id = crear_cobro_efectivo_informal("04/10/2026".into(), "En dólares".into(), importe("5"), "USD".into()).unwrap();
    assert_importe(balance_cuenta("Efectivo USD"), usd + 5.0, "entró en la caja de dólares");

    eliminar_ingreso_informal(id, motivo_de_prueba()).unwrap();

    assert_importe(balance_cuenta("Efectivo USD"), usd, "la caja de dólares vuelve donde estaba");
    assert_importe(balance_cuenta("Efectivo DOP"), dop, "la de pesos no se toca");
}

// --- A-03, vertical «préstamos»: caracterización antes de extraer ---
//
// Completan c34–c44 con los mensajes exactos, los valores por omisión, el contador de cuotas, el listado con sus
// recordatorios y el borrado. Fijan lo que los siete comandos hacen HOY.

fn entrada_de_prestamo(tipo: &str, monto: &str, cuotas: Option<(i32, i32)>, cuota: &str, dia: i32) -> crate::PrestamoInput {
    crate::PrestamoInput {
        tipo_prestamo: tipo.into(),
        monto_prestamo: importe(monto),
        institucion_financiera: "Banco Ejemplo".into(),
        tasa_actual: 12.0,
        cuotas_totales: cuotas.map(|c| c.0),
        cuotas_pendientes: cuotas.map(|c| c.1),
        monto_cuota: importe(cuota),
        dia_pago: dia,
        saldo_actual: None,
        limite_credito: None,
    }
}

type FilaDePrestamo = (String, f64, Option<i32>, Option<i32>, f64, i32, Option<f64>, Option<f64>);

fn fila_de_prestamo(id: i64) -> FilaDePrestamo {
    conexion()
        .query_row(
            "SELECT tipo_prestamo, monto_prestamo, cuotas_totales, cuotas_pendientes, monto_cuota, dia_pago, saldo_actual, limite_credito
             FROM prestamos WHERE id = ?;",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)),
        )
        .expect("leer préstamo")
}

#[test]
fn q1_crear_un_prestamo_guarda_sus_cifras_y_el_saldo_por_omision_es_el_monto() {
    let _g = entorno_aislado();
    let id = crate::crear_prestamo(entrada_de_prestamo("vehiculo", "50000.50", Some((60, 58)), "1250.25", 15)).unwrap();
    let (tipo, monto, totales, pendientes, cuota, dia, saldo, limite) = fila_de_prestamo(id);
    assert_eq!((tipo.as_str(), totales, pendientes, dia, limite), ("vehiculo", Some(60), Some(58), 15, None));
    assert_importe(monto, 50000.5, "monto");
    assert_importe(cuota, 1250.25, "cuota");
    assert_importe(saldo.unwrap(), 50000.5, "sin saldo declarado se asume el monto íntegro");

    let con_saldo = crate::crear_prestamo(crate::PrestamoInput { saldo_actual: Some(importe("40000")), ..entrada_de_prestamo("consumo", "50000", Some((12, 12)), "100", 5) }).unwrap();
    assert_importe(fila_de_prestamo(con_saldo).6.unwrap(), 40000.0, "el saldo declarado manda");
}

#[test]
fn q2_un_prestamo_flexible_ignora_las_cuotas_que_se_le_den() {
    let _g = entorno_aislado();
    let id = crate::crear_prestamo(entrada_de_prestamo("flexible", "1000", Some((10, 20)), "50", 5)).unwrap();
    let (_, _, totales, pendientes, ..) = fila_de_prestamo(id);
    assert_eq!((totales, pendientes), (None, None), "ni siquiera valida que 20 > 10: no hay cuotas que contar");
}

#[test]
fn q3_crear_rechaza_lo_que_no_tiene_sentido_con_su_mensaje_y_en_su_orden() {
    let _g = entorno_aislado();
    let e = |entrada: crate::PrestamoInput| crate::crear_prestamo(entrada).unwrap_err();
    assert_eq!(e(entrada_de_prestamo("vehiculo", "100", None, "10", 5)), "Las cuotas totales son requeridas.");
    assert_eq!(
        e(crate::PrestamoInput { cuotas_pendientes: None, ..entrada_de_prestamo("vehiculo", "100", Some((12, 12)), "10", 5) }),
        "Las cuotas pendientes son requeridas."
    );
    assert_eq!(
        e(entrada_de_prestamo("vehiculo", "100", Some((12, 13)), "10", 5)),
        "Error: El número de cuotas pendientes no puede ser mayor al número total de cuotas del préstamo."
    );
    for dia in [0, 32, -1] {
        assert_eq!(e(entrada_de_prestamo("vehiculo", "100", Some((12, 12)), "10", dia)), "El día de pago debe ser un día válido del mes (1-31).");
    }
    assert_eq!(
        e(crate::PrestamoInput { limite_credito: Some(importe("500")), ..entrada_de_prestamo("vehiculo", "100", Some((12, 12)), "10", 5) }),
        "Solo una línea revolvente tiene límite de crédito."
    );
    // El orden: primero las cuotas, después el día, al final el límite.
    assert_eq!(e(entrada_de_prestamo("vehiculo", "100", None, "10", 0)), "Las cuotas totales son requeridas.");
    let n: i64 = conexion().query_row("SELECT COUNT(*) FROM prestamos;", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0, "lo rechazado no dejó nada");
}

#[test]
fn q4_actualizar_rechaza_inexistente_tarjeta_inexistente_dia_y_limite_con_su_mensaje() {
    let _g = entorno_aislado();
    let auto = crear_prestamo_de_prueba("vehiculo", 100_000.0, 12.0, 5_000.0, Some((100, 89)), None);
    let linea = crear_prestamo_de_prueba("flexible", 100_000.0, 12.0, 5_000.0, None, Some(150_000.0));
    let base = |id: i64| crate::ActualizarPrestamoInput { id, tasa_actual: 12.0, monto_cuota: importe("5000"), dia_pago: 25, limite_credito: None, tarjeta_id: None };
    let e = |entrada: crate::ActualizarPrestamoInput| crate::actualizar_prestamo(entrada).unwrap_err();

    assert_eq!(e(base(987_654)), "No se encontró el financiamiento 987654.");
    assert_eq!(e(crate::ActualizarPrestamoInput { dia_pago: 32, ..base(auto) }), "El día de pago debe ser un día válido del mes (1-31).");
    assert_eq!(e(crate::ActualizarPrestamoInput { dia_pago: 0, ..base(987_654) }), "El día de pago debe ser un día válido del mes (1-31).", "el día se comprueba antes de buscar el financiamiento");
    assert_eq!(e(crate::ActualizarPrestamoInput { limite_credito: Some(importe("1")), ..base(auto) }), "Solo una línea revolvente tiene límite de crédito.");
    assert_eq!(e(crate::ActualizarPrestamoInput { tarjeta_id: Some(999_999), ..base(linea) }), "No se encontró la tarjeta 999999.");
    // Con la línea sí puede llevar límite, y se guarda.
    crate::actualizar_prestamo(crate::ActualizarPrestamoInput { limite_credito: Some(importe("200000")), ..base(linea) }).unwrap();
    assert_importe(fila_de_prestamo(linea).7.unwrap(), 200_000.0, "límite corregido");
}

#[test]
fn q5_pagar_y_declarar_sobre_un_financiamiento_inexistente_dicen_su_mensaje_y_no_asientan_nada() {
    let _g = entorno_aislado();
    assert_eq!(crate::pagar_cuota_prestamo(404, None).unwrap_err(), "No se encontró el financiamiento 404.");
    assert_eq!(crate::declarar_saldo_prestamo(404, importe("10"), None).unwrap_err(), "No se encontró el financiamiento 404.");
    assert!(crate::obtener_movimientos_prestamo(404).unwrap().is_empty(), "sin financiamiento no hay movimientos");
}

#[test]
fn q6_el_contador_de_cuotas_baja_de_uno_en_uno_sin_pasar_de_cero_y_la_linea_no_lo_tiene() {
    let _g = entorno_aislado();
    let amortizable = crear_prestamo_de_prueba("vehiculo", 10_000.0, 12.0, 1_000.0, Some((12, 1)), None);
    crate::pagar_cuota_prestamo(amortizable, Some("01/10/2026".into())).unwrap();
    assert_eq!(cuotas_pendientes(amortizable), Some(0));
    crate::pagar_cuota_prestamo(amortizable, Some("01/11/2026".into())).unwrap();
    assert_eq!(cuotas_pendientes(amortizable), Some(0), "no baja de cero");
    assert_eq!(crate::obtener_movimientos_prestamo(amortizable).unwrap().len(), 2, "pero el pago se asienta igual");

    let linea = crear_prestamo_de_prueba("flexible", 10_000.0, 12.0, 1_000.0, None, Some(20_000.0));
    crate::pagar_cuota_prestamo(linea, Some("01/10/2026".into())).unwrap();
    assert_eq!(cuotas_pendientes(linea), None, "la línea no cuenta cuotas");
}

#[test]
fn q7_la_fecha_por_omision_de_un_pago_es_hoy_y_una_declaracion_deja_el_desglose_en_cero_interes() {
    let _g = entorno_aislado();
    let id = crear_prestamo_de_prueba("flexible", 10_000.0, 12.0, 1_000.0, None, Some(20_000.0));
    crate::pagar_cuota_prestamo(id, None).unwrap();
    crate::declarar_saldo_prestamo(id, importe("9500"), None).unwrap();
    let hoy = chrono::Local::now().format("%d/%m/%Y").to_string();
    let movs = crate::obtener_movimientos_prestamo(id).unwrap();
    assert_eq!(movs.len(), 2);
    assert!(movs.iter().all(|m| m.fecha == hoy), "sin fecha, hoy");
    let declaracion = &movs[0];
    assert_eq!(declaracion.tipo, "declaracion");
    assert_importe(declaracion.interes, 0.0, "una declaración no genera interés");
    assert_importe(declaracion.capital, -(declaracion.monto), "el capital es la diferencia con el signo cambiado");
    assert_importe(declaracion.saldo_resultante, 9_500.0, "saldo declarado");
}

#[test]
fn q8_el_listado_trae_el_cupo_la_tarjeta_y_los_recordatorios_de_pago() {
    use chrono::Datelike;
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 0.0); // corte 15, límite de pago 5
    let dia_hoy = chrono::Local::now().day() as i32;
    let linea = crear_prestamo_de_prueba("flexible", 100_000.0, 12.0, 5_000.0, None, Some(150_000.0));
    let auto = crear_prestamo_de_prueba("vehiculo", 100_000.0, 12.0, 5_000.0, Some((100, 0)), None);
    let por_pagar = crear_prestamo_de_prueba("consumo", 20_000.0, 12.0, 500.0, Some((10, 5)), None);
    conexion().execute("UPDATE prestamos SET dia_pago = ? WHERE id = ?;", params![dia_hoy, por_pagar]).unwrap();
    conexion().execute("UPDATE prestamos SET tarjeta_id = ? WHERE id = ?;", params![tarjeta, linea]).unwrap();

    let lista = crate::obtener_prestamos().unwrap();
    assert_eq!(lista.iter().map(|p| p.id).collect::<Vec<_>>(), vec![por_pagar, auto, linea], "el más nuevo primero");
    let l = lista.iter().find(|p| p.id == linea).unwrap();
    assert!(l.es_revolvente);
    assert_importe(l.disponible.unwrap(), 50_000.0, "cupo = límite - saldo");
    assert_eq!((l.tarjeta_id, l.tarjeta_nombre.as_deref(), l.dia_corte), (Some(tarjeta), Some("Tarjeta Ejemplo"), Some(15)));
    assert_eq!(l.dia_pago, 5, "manda el vencimiento de la tarjeta");

    let a = lista.iter().find(|p| p.id == auto).unwrap();
    assert!(!a.es_revolvente && a.disponible.is_none());
    assert_eq!((a.alerta_pago, a.dias_pago_msg.as_str()), (false, "-"), "sin cuotas pendientes no hay recordatorio");

    let p = lista.iter().find(|p| p.id == por_pagar).unwrap();
    assert_eq!((p.alerta_pago, p.dias_pago_msg.as_str()), (true, "Hoy vence la cuota."));
}

#[test]
fn q9_eliminar_un_financiamiento_no_tiene_guardas_y_se_lleva_sus_movimientos() {
    let _g = entorno_aislado();
    let id = crear_prestamo_de_prueba("flexible", 10_000.0, 12.0, 1_000.0, None, Some(20_000.0));
    crate::pagar_cuota_prestamo(id, Some("01/10/2026".into())).unwrap();
    crate::eliminar_prestamo(id).unwrap();
    assert!(crate::obtener_prestamos().unwrap().is_empty());
    assert!(crate::obtener_movimientos_prestamo(id).unwrap().is_empty(), "el libro se va con él (ON DELETE CASCADE)");
    assert!(crate::eliminar_prestamo(id).is_ok(), "borrar uno que ya no está no es un error");
}

// --- A-03, vertical «tarjetas» (parte 1: listar, crear y límites): caracterización antes de extraer ---
//
// Fijan lo que `obtener_tarjetas`, `crear_tarjeta` y `actualizar_limites_tarjeta` hacen HOY.

fn alta_de_tarjeta(entidad: &str, nombre: &str, limite: &str, corte: i32, pago: i32) -> Result<i64, String> {
    crate::crear_tarjeta(
        entidad.into(), nombre.into(),
        importe(limite), importe("2000"), importe("300"), importe("400"),
        importe("500"), importe("600"), importe("700"), importe("800"),
        corte, pago,
    )
}

type FilaDeTarjeta = (String, String, f64, f64, f64, f64, f64, f64, f64, f64, i32, i32, Option<f64>, Option<f64>, Option<String>);

fn fila_de_tarjeta(id: i64) -> FilaDeTarjeta {
    conexion()
        .query_row(
            "SELECT entidad, nombre_tarjeta, limite_pesos, limite_dolares, limite_sobregiro_pesos, limite_sobregiro_dolares,
                    balance_pesos, balance_dolares, balance_corte_pesos, balance_corte_dolares, fecha_corte, fecha_limite_pago,
                    limite_ajustado_pesos, limite_ajustado_dolares, politica_liquidacion
             FROM tarjetas WHERE id = ?;",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?, r.get(11)?, r.get(12)?, r.get(13)?, r.get(14)?)),
        )
        .expect("leer tarjeta")
}

#[test]
fn r1_una_tarjeta_se_guarda_con_cada_importe_en_su_columna_y_sin_ajuste_ni_politica() {
    let _g = entorno_aislado();
    let id = alta_de_tarjeta("Banco Ejemplo", "Visa Ejemplo", "1000.50", 15, 5).unwrap();
    let f = fila_de_tarjeta(id);
    assert_eq!((f.0.as_str(), f.1.as_str(), f.10, f.11), ("Banco Ejemplo", "Visa Ejemplo", 15, 5));
    for (esperado, obtenido, columna) in [
        (1000.5, f.2, "límite DOP"), (2000.0, f.3, "límite USD"), (300.0, f.4, "sobregiro DOP"), (400.0, f.5, "sobregiro USD"),
        (500.0, f.6, "balance DOP"), (600.0, f.7, "balance USD"), (700.0, f.8, "corte DOP"), (800.0, f.9, "corte USD"),
    ] {
        assert_importe(obtenido, esperado, columna);
    }
    assert_eq!((f.12, f.13), (None, None), "sin límite ajustado");
}

#[test]
fn r2_hallazgo_crear_tarjeta_no_recorta_los_textos_ni_valida_los_importes_solo_los_dias_los_valida_el_esquema() {
    // **HALLAZGO, sin corregir** (protocolo: documentar y fijar; el cambio se consulta). El comando no recorta ni valida
    // entidad y nombre (acepta vacíos y con espacios) ni que los límites sean positivos (acepta un límite negativo). Los
    // días de corte y de pago los rechaza el esquema, con el texto crudo de SQLite. Esta prueba describe el comportamiento
    // ACTUAL.
    let _g = entorno_aislado();
    let id = alta_de_tarjeta("  ", "", "-100", 15, 5).unwrap();
    let f = fila_de_tarjeta(id);
    assert_eq!((f.0.as_str(), f.1.as_str()), ("  ", ""), "ni se recorta ni se exige");
    assert_importe(f.2, -100.0, "límite negativo aceptado");

    for (corte, pago) in [(0, 5), (32, 5), (15, 0), (15, 32)] {
        let e = alta_de_tarjeta("B", "T", "100", corte, pago).unwrap_err();
        assert!(e.contains("CHECK constraint failed"), "{corte}/{pago}: {e}");
    }
    let n: i64 = conexion().query_row("SELECT COUNT(*) FROM tarjetas;", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1, "las rechazadas no dejaron nada");
}

#[test]
fn r3_actualizar_limites_guarda_cada_importe_el_ajuste_que_se_borra_y_la_politica_se_normaliza() {
    let _g = entorno_aislado();
    let id = alta_de_tarjeta("B", "T", "100", 15, 5).unwrap();
    crate::actualizar_limites_tarjeta(
        id, importe("10"), importe("20"), importe("30"), importe("40"), importe("50"), importe("60"),
        Some(importe("7")), Some(importe("0")), Some("traduce".into()),
    )
    .unwrap();
    let f = fila_de_tarjeta(id);
    for (esperado, obtenido, columna) in [(10.0, f.2, "límite DOP"), (20.0, f.3, "límite USD"), (30.0, f.4, "sobregiro DOP"), (40.0, f.5, "sobregiro USD"), (50.0, f.8, "corte DOP"), (60.0, f.9, "corte USD")] {
        assert_importe(obtenido, esperado, columna);
    }
    assert_eq!((f.12, f.13), (Some(7.0), Some(0.0)), "el cero es un tope deliberado, distinto de «sin ajuste»");
    assert_eq!(f.14.as_deref(), Some("traduce"));
    assert_importe(f.6, 500.0, "el balance no se toca");

    crate::actualizar_limites_tarjeta(id, importe("1"), importe("1"), importe("1"), importe("1"), importe("1"), importe("1"), None, None, Some("cualquier cosa".into())).unwrap();
    let f = fila_de_tarjeta(id);
    assert_eq!((f.12, f.13), (None, None), "sin ajuste se borra el que hubiera");
    assert_eq!(f.14.as_deref(), Some("origen"), "una política desconocida cae en «origen»");
    crate::actualizar_limites_tarjeta(id, importe("1"), importe("1"), importe("1"), importe("1"), importe("1"), importe("1"), None, None, None).unwrap();
    assert_eq!(fila_de_tarjeta(id).14.as_deref(), Some("origen"), "ausente también");
}

#[test]
fn r4_hallazgo_actualizar_los_limites_de_una_tarjeta_inexistente_no_dice_nada() {
    // **HALLAZGO, sin corregir**: el `UPDATE` afecta a cero filas y el comando devuelve `Ok`, como pasaba con H18 en las
    // facturas. Describe el comportamiento ACTUAL.
    let _g = entorno_aislado();
    assert!(crate::actualizar_limites_tarjeta(
        987_654, importe("1"), importe("1"), importe("1"), importe("1"), importe("1"), importe("1"), None, None, None,
    )
    .is_ok());
}

#[test]
fn r5_el_listado_trae_el_cupo_la_politica_y_las_alertas_de_corte_y_pago() {
    use chrono::Datelike;
    let _g = entorno_aislado();
    let hoy = chrono::Local::now().day() as i32;
    // Corte hoy, pago dentro de 10 días (o su equivalente con el mes de 30 días).
    let pago = if hoy + 10 <= 31 { hoy + 10 } else { hoy + 10 - 31 };
    let id = alta_de_tarjeta("Banco Ejemplo", "Visa Ejemplo", "1000", hoy, pago.max(1)).unwrap();
    crate::actualizar_limites_tarjeta(id, importe("1000"), importe("2000"), importe("300"), importe("400"), importe("700"), importe("800"), Some(importe("800")), None, Some("traduce".into())).unwrap();

    let t = crate::obtener_tarjetas().unwrap().into_iter().find(|t| t.id == id).unwrap();
    assert_eq!(t.politica_liquidacion, "traduce");
    assert_importe(t.limite_efectivo_pesos, 800.0, "el límite ajustado manda");
    assert_importe(t.limite_efectivo_dolares, 2000.0, "sin ajuste, el aprobado");
    assert_importe(t.disponible_pesos, 800.0 + 300.0 - 500.0, "efectivo + sobregiro - balance");
    assert_importe(t.disponible_dolares, 2000.0 + 400.0 - 600.0, "ídem en dólares");
    assert!(t.alerta_corte);
    assert_eq!(t.dias_corte_msg, "Hoy es la fecha de corte");
    let dias_pago = if pago.max(1) >= hoy { pago.max(1) - hoy } else { (30 - hoy) + pago.max(1) };
    assert_eq!(t.alerta_pago, dias_pago <= 3);
    assert_eq!(t.dias_pago_msg, if dias_pago == 0 { "Hoy vence el pago".to_string() } else { format!("Faltan {} días para pagar", dias_pago) });
}

#[test]
fn r6_un_limite_ajustado_mayor_que_el_aprobado_no_lo_supera_y_el_listado_no_falla() {
    // El límite efectivo nunca supera al aprobado, aunque el ajustado guardado lo supere.
    let _g = entorno_aislado();
    let id = alta_de_tarjeta("B", "T", "1000", 15, 5).unwrap();
    conexion().execute("UPDATE tarjetas SET limite_ajustado_pesos = 5000.0 WHERE id = ?;", params![id]).unwrap();
    let t = crate::obtener_tarjetas().unwrap().into_iter().find(|t| t.id == id).unwrap();
    assert_importe(t.limite_efectivo_pesos, 1000.0, "el aprobado en bruto");
    assert_importe(t.disponible_pesos, 1000.0 + 300.0 - 500.0, "aprobado + sobregiro - balance");
}

#[test]
fn r7_las_tarjetas_se_listan_en_el_orden_en_que_se_crearon_y_una_politica_ausente_es_origen() {
    let _g = entorno_aislado();
    let a = alta_de_tarjeta("B", "Primera", "100", 15, 5).unwrap();
    let b = alta_de_tarjeta("B", "Segunda", "100", 15, 5).unwrap();
    conexion().execute("UPDATE tarjetas SET politica_liquidacion = NULL WHERE id = ?;", params![a]).unwrap();
    let lista = crate::obtener_tarjetas().unwrap();
    assert_eq!(lista.iter().map(|t| t.id).collect::<Vec<_>>(), vec![a, b], "orden de creación, sin ORDER BY");
    assert_eq!(lista[0].politica_liquidacion, "origen");
}

// --- A-03, vertical «tarjetas» (parte 2: abonos): caracterización antes de extraer ---
//
// Completan c17–c20 y c63–c67 (el registro y la reversión ya viven en casos de uso) con lo que sigue en `main.rs`:
// el listado, la categoría de la comisión, el caso de corrección y los textos del resumen.

#[test]
fn ab1_los_abonos_de_una_tarjeta_salen_del_mas_nuevo_al_mas_viejo_con_su_cuenta_y_su_tasa() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(0.0, 500.0);
    let cuenta = crear_cuenta("Cuenta Abonos", "DOP", 100_000.0);
    registrar_pago_tarjeta(tarjeta, "01/10/2026".to_string(), monto(100.0), "USD".to_string(), Some(cuenta), 60.0).unwrap();
    registrar_pago_tarjeta(tarjeta, "02/10/2026".to_string(), monto(200.0), "USD".to_string(), None, 0.0).unwrap();

    let abonos = crate::obtener_abonos_tarjeta(tarjeta).unwrap();
    assert_eq!(abonos.len(), 2);
    let (nuevo, viejo) = (&abonos[0], &abonos[1]);
    assert_eq!((nuevo.fecha_pago.as_str(), nuevo.divisa.as_str(), nuevo.cuenta_ahorro_id, nuevo.cuenta_nombre.as_deref()), ("02/10/2026", "USD", None, None));
    assert_importe(nuevo.monto_pagado, 200.0, "monto del más nuevo");
    assert!(nuevo.tasa_cambio.is_none(), "sin cuenta no hay tasa");
    assert_eq!((viejo.fecha_pago.as_str(), viejo.cuenta_ahorro_id, viejo.cuenta_nombre.as_deref()), ("01/10/2026", Some(cuenta), Some("Cuenta Abonos")));
    assert_importe(viejo.tasa_cambio.unwrap(), 60.0, "la tasa del abono multidivisa");
}

#[test]
fn ab2_solo_se_listan_los_abonos_de_la_tarjeta_pedida() {
    let _g = entorno_aislado();
    let a = crear_tarjeta(1_000.0, 0.0);
    let b = crear_tarjeta(1_000.0, 0.0);
    registrar_pago_tarjeta(a, "01/10/2026".to_string(), monto(10.0), "DOP".to_string(), None, 0.0).unwrap();
    assert_eq!(crate::obtener_abonos_tarjeta(a).unwrap().len(), 1);
    assert!(crate::obtener_abonos_tarjeta(b).unwrap().is_empty());
    assert!(crate::obtener_abonos_tarjeta(987_654).unwrap().is_empty(), "una tarjeta inexistente no es un error");
}

#[test]
fn ab3_la_comision_de_un_abono_se_asienta_como_gasto_de_la_categoria_otros() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(10_000.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Abonos", "DOP", 50_000.0);
    registrar_pago_tarjeta(tarjeta, "01/10/2026".to_string(), monto(5_000.0), "DOP".to_string(), Some(cuenta), 0.0).unwrap();
    let (categoria, cuenta_gasto): (i64, Option<i64>) = conexion()
        .query_row("SELECT categoria_id, cuenta_ahorro_id FROM gastos ORDER BY id DESC LIMIT 1;", [], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap();
    let otros: i64 = conexion().query_row("SELECT id FROM categorias WHERE nombre = 'Otros';", [], |r| r.get(0)).unwrap();
    assert_eq!(categoria, otros, "la comisión va a «Otros»");
    assert_eq!(cuenta_gasto, Some(cuenta));
}

#[test]
fn ab4_revertir_un_abono_inexistente_o_con_motivo_corto_dice_su_mensaje_y_no_toca_nada() {
    let _g = entorno_aislado();
    assert_eq!(
        revertir_abono_tarjeta(404, motivo_de_prueba()).unwrap_err(),
        "No se encontró abono con identificador 404."
    );
    let tarjeta = crear_tarjeta(10_000.0, 0.0);
    registrar_pago_tarjeta(tarjeta, "01/10/2026".to_string(), monto(1_000.0), "DOP".to_string(), None, 0.0).unwrap();
    let e = revertir_abono_tarjeta(ultimo_abono(), "corto".into()).unwrap_err();
    assert!(e.starts_with("Explica la corrección en al menos 15 caracteres.") && e.contains("«corto»"), "{e}");
    assert_importe(balances_tarjeta(tarjeta).0, 9_000.0, "la deuda no se movió");
    assert!(casos_de_correccion().is_empty(), "ni se abrió un caso");
}

#[test]
fn ab5_revertir_deja_un_caso_con_el_abono_y_cuenta_lo_devuelto_con_los_textos_de_siempre() {
    let _g = entorno_aislado();
    let tarjeta = crear_tarjeta(10_000.0, 0.0);
    let cuenta = crear_cuenta("Cuenta Abonos", "DOP", 50_000.0);
    registrar_pago_tarjeta(tarjeta, "03/10/2026".to_string(), monto(5_432.0), "DOP".to_string(), Some(cuenta), 0.0).unwrap();
    let con_cuenta = revertir_abono_tarjeta(ultimo_abono(), motivo_de_prueba()).unwrap();
    let anio = chrono::Local::now().format("%Y").to_string();
    assert_eq!(con_cuenta, format!("Se repusieron DOP 5432.00 a la deuda y volvieron DOP 5442.86 a la cuenta. Caso COR-{anio}-0001."));

    registrar_pago_tarjeta(tarjeta, "04/10/2026".to_string(), monto(700.0), "DOP".to_string(), None, 0.0).unwrap();
    let sin_cuenta = revertir_abono_tarjeta(ultimo_abono(), motivo_de_prueba()).unwrap();
    assert_eq!(sin_cuenta, format!("Se repusieron DOP 700.00 a la deuda. El abono no tenía cuenta asociada. Caso COR-{anio}-0002."));

    let casos = casos_de_correccion();
    let (_, tipo, _, descripcion, importe_caso, divisa) = &casos[0];
    assert_eq!((tipo.as_str(), descripcion.as_str(), divisa.as_deref()), ("abono", "Abono del 03/10/2026", Some("DOP")));
    assert_importe(importe_caso.unwrap(), 5_432.0, "monto del abono");
}

