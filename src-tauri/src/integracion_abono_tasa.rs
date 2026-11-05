//! Pruebas de integración de la tasa de un abono con cuenta: se guarda **solo cuando hay cruce de divisas**.
//!
//! Ejercitan los comandos reales (`registrar_pago_tarjeta`, `obtener_abonos_tarjeta`, `revertir_abono_tarjeta`) sobre
//! una base SQLite temporal con el esquema real. Se lee la fila de `pagos_tarjeta` y el gasto de la comisión tal como
//! quedan en la base. Todos los datos son inventados.
//!
//! Se reconocen por el prefijo `integracion_`: `cargo test integracion_`.

use crate::caracterizacion::entorno_para_pruebas_externas;
use crate::comandos::abonos::{obtener_abonos_tarjeta, registrar_pago_tarjeta, revertir_abono_tarjeta};
use crate::db_sql;
use crate::ipc::ImporteDecimal;
use rusqlite::{params, Connection};

fn conexion() -> Connection {
    db_sql::obtener_conexion().expect("abrir conexión de prueba")
}

fn importe(texto: &str) -> ImporteDecimal {
    ImporteDecimal::desde_texto(texto).expect("importe de prueba")
}

fn crear_cuenta(nombre: &str, divisa: &str, balance: f64) -> i64 {
    let c = conexion();
    c.execute(
        "INSERT INTO cuentas_ahorro (nombre, divisa, balance_actual) VALUES (?, ?, ?);",
        params![nombre, divisa, balance],
    )
    .unwrap();
    c.last_insert_rowid()
}

fn crear_tarjeta() -> i64 {
    let c = conexion();
    c.execute(
        "INSERT INTO tarjetas (entidad, nombre_tarjeta, fecha_corte, fecha_limite_pago, balance_pesos, balance_dolares,
                               limite_pesos, limite_dolares, politica_liquidacion)
         VALUES ('Banco Ejemplo', 'Tarjeta Ejemplo', 15, 5, 48731.19, 913.47, 900000.0, 9000.0, 'traduce');",
        [],
    )
    .unwrap();
    c.last_insert_rowid()
}

fn centavos(x: f64) -> i64 {
    (x * 100.0).round() as i64
}

fn saldo(id: i64) -> i64 {
    centavos(
        conexion()
            .query_row("SELECT balance_actual FROM cuentas_ahorro WHERE id = ?;", [id], |r| r.get(0))
            .unwrap(),
    )
}

fn saldo_por_nombre(nombre: &str) -> i64 {
    centavos(
        conexion()
            .query_row("SELECT balance_actual FROM cuentas_ahorro WHERE nombre = ?;", [nombre], |r| r.get(0))
            .unwrap(),
    )
}

fn deuda(tarjeta: i64) -> (i64, i64) {
    conexion()
        .query_row("SELECT balance_pesos, balance_dolares FROM tarjetas WHERE id = ?;", [tarjeta], |r| {
            Ok((centavos(r.get(0)?), centavos(r.get(1)?)))
        })
        .unwrap()
}

fn abonar(tarjeta: i64, monto: &str, divisa: &str, cuenta: Option<i64>, tasa: f64) -> Result<(), String> {
    registrar_pago_tarjeta(tarjeta, "03/10/2026".into(), importe(monto), divisa.into(), cuenta, tasa)
}

/// (id, tasa guardada, descripción de la comisión) del último abono.
fn ultimo_abono() -> (i64, Option<f64>, Option<String>) {
    conexion()
        .query_row(
            "SELECT p.id, p.tasa_cambio, g.descripcion FROM pagos_tarjeta p
             LEFT JOIN gastos g ON g.id = p.gasto_comision_id ORDER BY p.id DESC LIMIT 1;",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
}

const SIN_TASA: &str = "Comisión 0.20% Pago Tarjeta";
const MOTIVO: &str = "Corrección de prueba automatizada del sistema";

#[test]
fn integracion_sin_cruce_la_tasa_recibida_no_se_guarda_ni_se_nombra() {
    let _g = entorno_para_pruebas_externas();
    let tarjeta = crear_tarjeta();
    let dop = crear_cuenta("Ahorros DOP", "DOP", 250_000.0);
    let usd = crear_cuenta("Ahorros USD", "USD", 3_000.0);

    // DOP desde cuenta en DOP con una tasa sobrante.
    abonar(tarjeta, "1234.57", "DOP", Some(dop), 61.37).unwrap();
    let (_, tasa, desc) = ultimo_abono();
    assert_eq!((tasa, desc.as_deref()), (None, Some(SIN_TASA)));
    assert_eq!(saldo(dop), 25_000_000 - 123_457 - 247, "1234.57 más 2.47 de comisión");

    // USD desde cuenta en USD con una tasa sobrante.
    abonar(tarjeta, "87.43", "USD", Some(usd), 61.37).unwrap();
    let (_, tasa, desc) = ultimo_abono();
    assert_eq!((tasa, desc.as_deref()), (None, Some(SIN_TASA)));
    assert_eq!(saldo(usd), 300_000 - 8_743 - 17, "87.43 más 0.17 de comisión");

    assert_eq!(deuda(tarjeta), (4_873_119 - 123_457, 91_347 - 8_743));

    // El listado: ambos abonos salen sin tasa.
    let abonos = obtener_abonos_tarjeta(tarjeta).unwrap();
    assert_eq!(abonos.len(), 2);
    assert!(abonos.iter().all(|a| a.tasa_cambio.is_none()));
}

#[test]
fn integracion_con_cruce_la_tasa_se_guarda_se_nombra_y_se_lista() {
    let _g = entorno_para_pruebas_externas();
    let tarjeta = crear_tarjeta();
    let dop = crear_cuenta("Ahorros DOP", "DOP", 250_000.0);
    let usd = crear_cuenta("Ahorros USD", "USD", 3_000.0);

    // USD desde cuenta en DOP: 123.45 x 58.75 = 7252.6875 -> 7252.69; comisión 14.51.
    abonar(tarjeta, "123.45", "USD", Some(dop), 58.75).unwrap();
    let (_, tasa, desc) = ultimo_abono();
    assert_eq!(tasa, Some(58.75));
    assert_eq!(desc.as_deref(), Some("Comisión 0.20% Pago Tarjeta (Tasa 58.75)"));
    assert_eq!(saldo(dop), 25_000_000 - 725_269 - 1_451);

    // DOP desde cuenta en USD: 5875.43 / 58.75 = 100.0073 -> 100.01; comisión 0.20.
    abonar(tarjeta, "5875.43", "DOP", Some(usd), 58.75).unwrap();
    let (_, tasa, desc) = ultimo_abono();
    assert_eq!(tasa, Some(58.75));
    assert_eq!(desc.as_deref(), Some("Comisión 0.20% Pago Tarjeta (Tasa 58.75)"));
    assert_eq!(saldo(usd), 300_000 - 10_001 - 20);

    let abonos = obtener_abonos_tarjeta(tarjeta).unwrap();
    assert_eq!(abonos.iter().map(|a| a.tasa_cambio).collect::<Vec<_>>(), vec![Some(58.75), Some(58.75)]);
}

#[test]
fn integracion_sin_cuenta_sale_de_efectivo_sin_comision_ni_tasa() {
    let _g = entorno_para_pruebas_externas();
    let tarjeta = crear_tarjeta();
    let antes = saldo_por_nombre("Efectivo DOP");

    abonar(tarjeta, "77.77", "DOP", None, 60.0).unwrap();

    let (_, tasa, desc) = ultimo_abono();
    assert_eq!((tasa, desc), (None, None), "sin comisión no hay gasto");
    assert_eq!(saldo_por_nombre("Efectivo DOP"), antes - 7_777);
    assert_eq!(obtener_abonos_tarjeta(tarjeta).unwrap()[0].tasa_cambio, None);
}

#[test]
fn integracion_cruzar_divisas_sin_tasa_falla_y_no_mueve_nada() {
    let _g = entorno_para_pruebas_externas();
    let tarjeta = crear_tarjeta();
    let dop = crear_cuenta("Ahorros DOP", "DOP", 250_000.0);

    let e = abonar(tarjeta, "123.45", "USD", Some(dop), 0.0).unwrap_err();

    assert!(e.contains("tasa de cambio"), "{e}");
    assert_eq!(saldo(dop), 25_000_000);
    assert_eq!(deuda(tarjeta), (4_873_119, 91_347));
    assert!(obtener_abonos_tarjeta(tarjeta).unwrap().is_empty());
}

#[test]
fn integracion_revertir_un_abono_nuevo_sin_tasa_restituye_saldos_y_comision() {
    let _g = entorno_para_pruebas_externas();
    let tarjeta = crear_tarjeta();
    let dop = crear_cuenta("Ahorros DOP", "DOP", 250_000.0);
    abonar(tarjeta, "1234.57", "DOP", Some(dop), 61.37).unwrap();
    let (id, tasa, _) = ultimo_abono();
    assert_eq!(tasa, None);

    revertir_abono_tarjeta(id, MOTIVO.into()).unwrap();

    assert_eq!(saldo(dop), 25_000_000);
    assert_eq!(deuda(tarjeta), (4_873_119, 91_347));
    assert!(obtener_abonos_tarjeta(tarjeta).unwrap().is_empty());
    let comisiones: i64 = conexion()
        .query_row("SELECT COUNT(*) FROM gastos WHERE descripcion LIKE 'Comisión 0.20%';", [], |r| r.get(0))
        .unwrap();
    assert_eq!(comisiones, 0, "la comisión se fue con el abono");
}

#[test]
fn integracion_un_abono_antiguo_con_tasa_1_se_sigue_listando_y_revirtiendo() {
    let _g = entorno_para_pruebas_externas();
    let tarjeta = crear_tarjeta();
    let dop = crear_cuenta("Ahorros DOP", "DOP", 249_998.0);
    let c = conexion();
    // Como los dejó la versión anterior: tasa 1.0 y «Tasa 1» en la descripción de la comisión. Sembrado a mano.
    let categoria: i64 = c.query_row("SELECT id FROM categorias LIMIT 1;", [], |r| r.get(0)).unwrap();
    c.execute(
        "INSERT INTO gastos (fecha, monto, divisa, descripcion, categoria_id, metodo_pago, costo_adicional, cuenta_ahorro_id)
         VALUES ('01/10/2026', 2.47, 'DOP', 'Comisión 0.20% Pago Tarjeta (Tasa 1)', ?, 'transferencia', 0.0, ?);",
        params![categoria, dop],
    )
    .unwrap();
    let gasto = c.last_insert_rowid();
    c.execute(
        "INSERT INTO pagos_tarjeta (tarjeta_id, fecha_pago, monto_pagado, divisa, cuenta_ahorro_id, tasa_cambio,
                                    gasto_comision_id, monto_debitado, comision)
         VALUES (?, '01/10/2026', 1234.57, 'DOP', ?, 1.0, ?, 1234.57, 2.47);",
        params![tarjeta, dop, gasto],
    )
    .unwrap();
    let pago = c.last_insert_rowid();
    drop(c);
    // Se registra uno nuevo sin tasa al lado del antiguo.
    abonar(tarjeta, "10.11", "DOP", Some(dop), 60.0).unwrap();
    let antes = saldo(dop);

    let tasas: Vec<Option<f64>> = obtener_abonos_tarjeta(tarjeta).unwrap().iter().map(|a| a.tasa_cambio).collect();
    assert_eq!(tasas, vec![None, Some(1.0)], "el nuevo sin tasa, el antiguo conserva su 1.0");

    revertir_abono_tarjeta(pago, MOTIVO.into()).unwrap();

    assert_eq!(saldo(dop), antes + 123_457 + 247, "vuelven 1234.57 + 2.47 sin multiplicar por la tasa");
    assert_eq!(obtener_abonos_tarjeta(tarjeta).unwrap().len(), 1);
}
