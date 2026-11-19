//! Pruebas de integración del pago de préstamos desde una cuenta (`previsualizar_pago_prestamo`,
//! `registrar_pago_prestamo`, `revertir_pago_prestamo`) y de su relación con los gastos y el resumen.
//!
//! Ejercitan los **comandos reales** de punta a punta sobre una base SQLite temporal con el esquema real (el entorno
//! aislado de `caracterizacion`). Todos los datos son inventados, con céntimos poco redondos.
//!
//! Se reconocen por el prefijo `integracion_`: `cargo test integracion_`.

use crate::caracterizacion::entorno_para_pruebas_externas;
use crate::comandos::catalogos::crear_categoria;
use crate::comandos::gastos::eliminar_gasto;
use crate::comandos::prestamos::{
    crear_prestamo, eliminar_prestamo, obtener_movimientos_prestamo, pagar_cuota_prestamo, previsualizar_pago_prestamo,
    registrar_pago_prestamo, revertir_pago_prestamo, PagoPrestamoInput, PrestamoInput,
};
use crate::comandos::resumen::resumen_en;
use crate::integracion_resumen::{conexion, hoy, huella_de_la_base, importe, verificar_forma};
use rusqlite::params;
use std::sync::MutexGuard;

const TIPOS_IPC: &str = include_str!("../../src/js/tipos-ipc.d.ts");
const MOTIVO: &str = "Se registró con la cuenta equivocada";

struct Escenario {
    _guarda: MutexGuard<'static, ()>,
    prestamo: i64,
    cuenta: i64,
    categoria: i64,
}

/// Un préstamo de vehículo con saldo 87 654.32 al 13.5 %, 40 de 48 cuotas y cuota de 3 210.47; una cuenta en pesos con
/// 50 000.00 y la categoría «Prestamo».
fn escenario() -> Escenario {
    let guarda = entorno_para_pruebas_externas();
    let prestamo = crear_prestamo(PrestamoInput {
        tipo_prestamo: "vehiculo".into(),
        monto_prestamo: importe("120000.00"),
        institucion_financiera: "Banco Ejemplo".into(),
        tasa_actual: 13.5,
        cuotas_totales: Some(48),
        cuotas_pendientes: Some(40),
        monto_cuota: importe("3210.47"),
        dia_pago: 15,
        saldo_actual: Some(importe("87654.32")),
        limite_credito: None,
    })
    .unwrap();
    let c = conexion();
    c.execute(
        "INSERT INTO cuentas_ahorro (nombre, divisa, balance_actual) VALUES (?, 'DOP', ?);",
        params!["Cuenta Ahorros DOP", 50_000.0],
    )
    .unwrap();
    let cuenta = c.last_insert_rowid();
    let categoria = crear_categoria("Prestamo".into()).unwrap().id;
    Escenario { _guarda: guarda, prestamo, cuenta, categoria }
}

fn pago(e: &Escenario, tipo: &str, monto: &str, interes: Option<&str>) -> PagoPrestamoInput {
    PagoPrestamoInput {
        prestamo_id: e.prestamo,
        tipo: tipo.into(),
        monto: importe(monto),
        interes: interes.map(importe),
        fecha: "15/11/2026".into(),
        cuenta_id: Some(e.cuenta),
        categoria_id: e.categoria,
    }
}

fn balance(cuenta: i64) -> f64 {
    conexion().query_row("SELECT balance_actual FROM cuentas_ahorro WHERE id = ?;", [cuenta], |r| r.get(0)).unwrap()
}

fn prestamo(id: i64) -> (f64, Option<i32>) {
    conexion()
        .query_row("SELECT saldo_actual, cuotas_pendientes FROM prestamos WHERE id = ?;", [id], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
}

#[test]
fn integracion_pagar_una_cuota_saca_el_dinero_y_amortiza_en_una_sola_operacion() {
    let e = escenario();
    let previsto = previsualizar_pago_prestamo(e.prestamo, "cuota".into(), importe("3210.47"), None).unwrap();
    assert_eq!(
        (previsto.interes.as_str(), previsto.capital.as_str(), previsto.saldo_previo.as_str(), previsto.saldo_resultante.as_str()),
        ("986.11", "2224.36", "87654.32", "85429.96")
    );
    assert!(previsto.interes_estimado);
    assert_eq!(previsto.cuotas_pendientes_despues, Some(39));

    let r = registrar_pago_prestamo(pago(&e, "cuota", "3210.47", None)).unwrap();
    assert_eq!(serde_json::to_value(&r).unwrap(), serde_json::to_value(&previsto).unwrap(), "lo previsto es lo asentado");

    // La cuenta pierde la cuota y la retención del 0.20 % de una transferencia (6.42).
    assert_eq!(balance(e.cuenta), 46_783.11);
    assert_eq!(prestamo(e.prestamo), (85_429.96, Some(39)));

    let movimientos = obtener_movimientos_prestamo(e.prestamo).unwrap();
    assert_eq!(movimientos.len(), 1);
    let m = &movimientos[0];
    assert_eq!((m.tipo.as_str(), m.fecha.as_str(), m.monto, m.interes, m.capital), ("cuota", "15/11/2026", 3_210.47, 986.11, 2_224.36));
    let gasto_id = m.gasto_id.expect("el pago guarda su gasto");
    let (categoria, metodo, cuenta, cargo, descripcion): (i64, String, i64, f64, String) = conexion()
        .query_row(
            "SELECT categoria_id, metodo_pago, cuenta_ahorro_id, costo_adicional, descripcion FROM gastos WHERE id = ?;",
            [gasto_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    assert_eq!((categoria, metodo.as_str(), cuenta, cargo), (e.categoria, "transferencia", e.cuenta, 6.42));
    assert_eq!(descripcion, "Cuota de préstamo · Banco Ejemplo");
}

#[test]
fn integracion_el_interes_del_recibo_y_el_abono_a_capital() {
    let e = escenario();
    registrar_pago_prestamo(pago(&e, "cuota", "3210.47", Some("1002.03"))).unwrap();
    assert_eq!(prestamo(e.prestamo), (85_445.88, Some(39)));
    registrar_pago_prestamo(pago(&e, "abono_capital", "5000.03", None)).unwrap();
    assert_eq!(prestamo(e.prestamo), (80_445.85, Some(39)), "el abono a capital no consume cuota");
    let tipos: Vec<String> = obtener_movimientos_prestamo(e.prestamo).unwrap().into_iter().map(|m| m.tipo).collect();
    assert_eq!(tipos, ["abono_capital", "cuota"]);
}

#[test]
fn integracion_si_algo_falla_a_mitad_no_queda_ni_el_gasto_ni_el_movimiento() {
    let e = escenario();
    let antes = huella_de_la_base();
    // Una categoría que no existe hace fallar la inserción del gasto **después** de mover el saldo de la cuenta.
    let mut malo = pago(&e, "cuota", "3210.47", None);
    malo.categoria_id = 9_999;
    assert!(registrar_pago_prestamo(malo).is_err());
    // Y un tipo desconocido, un importe cero o un interés imposible se rechazan antes de tocar nada.
    for (tipo, monto, interes) in [("declaracion", "10.00", None), ("cuota", "0.00", None), ("cuota", "10.00", Some("10.01"))] {
        let r = registrar_pago_prestamo(pago(&e, tipo, monto, interes));
        assert!(r.is_err(), "{tipo} {monto} {interes:?}");
    }
    assert_eq!(huella_de_la_base(), antes, "ni saldo, ni gasto, ni movimiento");
}

#[test]
fn integracion_el_gasto_de_un_pago_no_se_borra_suelto() {
    let e = escenario();
    registrar_pago_prestamo(pago(&e, "cuota", "3210.47", None)).unwrap();
    let gasto_id = obtener_movimientos_prestamo(e.prestamo).unwrap()[0].gasto_id.unwrap();
    let antes = huella_de_la_base();
    let error = eliminar_gasto(gasto_id, MOTIVO.into()).unwrap_err();
    assert!(error.contains("pago de un préstamo"), "{error}");
    assert_eq!(huella_de_la_base(), antes);
}

#[test]
fn integracion_revertir_devuelve_dinero_saldo_y_cuota_y_deja_un_caso() {
    let e = escenario();
    let antes_de_pagar = (balance(e.cuenta), prestamo(e.prestamo));
    registrar_pago_prestamo(pago(&e, "cuota", "3210.47", None)).unwrap();
    let m = obtener_movimientos_prestamo(e.prestamo).unwrap().remove(0);

    let caso = revertir_pago_prestamo(m.id, MOTIVO.into()).unwrap();

    assert!(caso.starts_with("COR-"), "{caso}");
    assert_eq!((balance(e.cuenta), prestamo(e.prestamo)), antes_de_pagar);
    assert!(obtener_movimientos_prestamo(e.prestamo).unwrap().is_empty());
    let c = conexion();
    let gastos: i64 = c.query_row("SELECT COUNT(*) FROM gastos WHERE id = ?;", [m.gasto_id.unwrap()], |r| r.get(0)).unwrap();
    assert_eq!(gastos, 0);
    let (tipo, referencia, importe_caso, motivo, detalle): (String, i64, f64, String, String) = c
        .query_row(
            "SELECT tipo, referencia_id, importe, motivo, detalle FROM correcciones WHERE numero_caso = ?;",
            [&caso],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    assert_eq!((tipo.as_str(), referencia, importe_caso, motivo.as_str()), ("pago_prestamo", m.id, 3_210.47, MOTIVO));
    let detalle: serde_json::Value = serde_json::from_str(&detalle).unwrap();
    assert_eq!(detalle["gasto_id"], m.gasto_id.unwrap());
}

#[test]
fn integracion_solo_se_revierte_el_ultimo_pago_con_cuenta() {
    let e = escenario();
    registrar_pago_prestamo(pago(&e, "cuota", "3210.47", None)).unwrap();
    let primero = obtener_movimientos_prestamo(e.prestamo).unwrap()[0].id;
    registrar_pago_prestamo(pago(&e, "abono_capital", "1000.07", None)).unwrap();
    let antes = huella_de_la_base();
    assert!(revertir_pago_prestamo(primero, MOTIVO.into()).unwrap_err().contains("último movimiento"));
    assert_eq!(huella_de_la_base(), antes);

    // Una cuota del botón antiguo no sacó dinero de ninguna cuenta: no hay nada que devolver.
    pagar_cuota_prestamo(e.prestamo, Some("20/11/2026".into())).unwrap();
    let vieja = obtener_movimientos_prestamo(e.prestamo).unwrap()[0].clone();
    assert_eq!(vieja.gasto_id, None);
    assert!(revertir_pago_prestamo(vieja.id, MOTIVO.into()).unwrap_err().contains("conciliando"));
}

#[test]
fn integracion_el_pago_cuenta_como_gasto_de_su_categoria_y_no_como_cargo_derivado() {
    let e = escenario();
    registrar_pago_prestamo(pago(&e, "cuota", "3210.47", None)).unwrap();
    let r = resumen_en(hoy(2026, 11, 20), 2026, 11, 0, 60.0, vec![]).unwrap();
    let fila = r.filas.iter().find(|f| f.categoria_id == e.categoria).expect("fila de la categoría del pago");
    assert!(!fila.derivado);
    assert!(r.filas.iter().all(|f| !f.derivado), "la retención va con el gasto, no como cargo derivado");
}

#[test]
fn integracion_borrar_el_prestamo_conserva_el_gasto_que_ya_salio_de_la_cuenta() {
    let e = escenario();
    registrar_pago_prestamo(pago(&e, "cuota", "3210.47", None)).unwrap();
    let gasto_id = obtener_movimientos_prestamo(e.prestamo).unwrap()[0].gasto_id.unwrap();
    eliminar_prestamo(e.prestamo).unwrap();
    assert_eq!(balance(e.cuenta), 46_783.11, "el dinero ya salió");
    // Sin el préstamo, el gasto vuelve a ser un gasto corriente que se corrige por su vía.
    eliminar_gasto(gasto_id, MOTIVO.into()).unwrap();
    assert_eq!(balance(e.cuenta), 50_000.0);
}

fn caja_dop() -> i64 {
    conexion()
        .query_row("SELECT id FROM cuentas_ahorro WHERE es_caja_efectivo = 1 AND divisa = 'DOP';", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn integracion_la_caja_se_marca_en_las_cuentas_y_elegida_como_cuenta_paga_en_efectivo() {
    let e = escenario();
    let caja = caja_dop();
    let cuentas = crate::comandos::cuentas::obtener_cuentas().unwrap();
    let cajas: Vec<i64> = {
        let c = conexion();
        let mut s = c.prepare("SELECT id FROM cuentas_ahorro WHERE es_caja_efectivo = 1 ORDER BY nombre;").unwrap();
        let v = s.query_map([], |r| r.get(0)).unwrap();
        v.map(Result::unwrap).collect()
    };
    assert!(cajas.contains(&caja));
    assert_eq!(
        cuentas.iter().filter(|c| c.es_caja_efectivo).map(|c| c.id).collect::<Vec<_>>(),
        cajas,
        "las cajas (una por divisa) llevan la marca y ninguna cuenta bancaria"
    );
    assert!(cuentas.iter().any(|c| c.id == e.cuenta && !c.es_caja_efectivo));
    conexion().execute("UPDATE cuentas_ahorro SET balance_actual = 9000.0 WHERE id = ?;", [caja]).unwrap();

    let mut desde_la_caja = pago(&e, "cuota", "1500.00", None);
    desde_la_caja.cuenta_id = Some(caja);
    registrar_pago_prestamo(desde_la_caja).unwrap();

    assert_eq!(balance(caja), 7_500.0, "sale el importe justo, sin la retención de una transferencia");
    let m = obtener_movimientos_prestamo(e.prestamo).unwrap().remove(0);
    let (metodo, cargo): (String, f64) = conexion()
        .query_row("SELECT metodo_pago, costo_adicional FROM gastos WHERE id = ?;", [m.gasto_id.unwrap()], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap();
    assert_eq!((metodo.as_str(), cargo), ("efectivo", 0.0));
    assert_eq!(m.cuenta_id, Some(caja));
}

#[test]
fn integracion_cada_pago_dice_de_que_cuenta_salio_y_la_conciliacion_no() {
    let e = escenario();
    registrar_pago_prestamo(pago(&e, "cuota", "3210.47", None)).unwrap();
    let mut en_efectivo = pago(&e, "abono_capital", "100.03", None);
    en_efectivo.cuenta_id = None;
    conexion().execute("UPDATE cuentas_ahorro SET balance_actual = 500.0 WHERE id = ?;", [caja_dop()]).unwrap();
    registrar_pago_prestamo(en_efectivo).unwrap();
    crate::comandos::prestamos::declarar_saldo_prestamo(e.prestamo, importe("85000.03"), Some("30/11/2026".into())).unwrap();

    let cuentas: Vec<Option<i64>> = obtener_movimientos_prestamo(e.prestamo).unwrap().into_iter().map(|m| m.cuenta_id).collect();
    assert_eq!(cuentas, vec![None, Some(caja_dop()), Some(e.cuenta)], "del más reciente al más antiguo");
}

#[test]
fn integracion_contrato_de_forma_de_los_pagos_con_tipos_ipc() {
    let e = escenario();
    let r = registrar_pago_prestamo(pago(&e, "cuota", "3210.47", None)).unwrap();
    verificar_forma(TIPOS_IPC, "PrevisionPagoPrestamo", &serde_json::to_value(&r).unwrap(), "PrevisionPagoPrestamo");
    let m = serde_json::to_value(&obtener_movimientos_prestamo(e.prestamo).unwrap()[0]).unwrap();
    verificar_forma(TIPOS_IPC, "MovimientoPrestamo", &m, "MovimientoPrestamo");
    for firma in [
        "previsualizar_pago_prestamo: { args: { prestamoId: number; tipo: string; monto: string; interes?: string | null }; ret: PrevisionPagoPrestamo }",
        "registrar_pago_prestamo: { args: { input: PagoPrestamoInput }; ret: PrevisionPagoPrestamo }",
        "revertir_pago_prestamo: { args: { movimientoId: number; motivo: string }; ret: string }",
    ] {
        assert!(TIPOS_IPC.contains(firma), "tipos-ipc.d.ts debe declarar: {firma}");
    }
}
