//! Pruebas de integración de la anulación de traspasos con asiento de compensación (modelo C, PROTOTIPO).
//!
//! Comandos reales de punta a punta sobre una base SQLite temporal con el esquema real (entorno aislado de
//! `caracterizacion`). Datos inventados, con céntimos poco redondos. Prefijo `integracion_`.

use crate::comandos::cuentas::{
    eliminar_transaccion_cuenta, obtener_transacciones_cuentas, transferir_entre_cuentas, TransaccionCuenta,
};
use crate::comandos::gastos::{crear_gasto, GastoInput};
use crate::comandos::patrimonio::obtener_patrimonio;
use crate::comandos::resumen::obtener_resumen_gastos;
use crate::{db_nosql, db_sql};
use crate::ipc::ImporteDecimal;
use rusqlite::{params, Connection};
use std::sync::MutexGuard;

const MOTIVO: &str = "Anulación de prueba de integración";

fn entorno() -> MutexGuard<'static, ()> {
    let guarda = crate::caracterizacion::entorno_para_pruebas_externas();
    let ruta = db_nosql::obtener_ruta_nosql("capital");
    assert!(ruta.starts_with(std::env::temp_dir().to_str().unwrap()), "ABORTADO: apuntaría al capital real ({ruta})");
    let _ = std::fs::remove_file(&ruta);
    guarda
}

fn conexion() -> Connection {
    db_sql::obtener_conexion().expect("abrir la base temporal")
}

fn importe(t: &str) -> ImporteDecimal {
    ImporteDecimal::desde_texto(t).unwrap()
}

fn cuenta(nombre: &str, divisa: &str, balance: f64) -> i64 {
    let c = conexion();
    c.execute("INSERT INTO cuentas_ahorro (nombre, divisa, balance_actual) VALUES (?, ?, ?);", params![nombre, divisa, balance]).unwrap();
    c.last_insert_rowid()
}

fn saldo(id: i64) -> f64 {
    conexion().query_row("SELECT balance_actual FROM cuentas_ahorro WHERE id = ?;", [id], |r| r.get(0)).unwrap()
}

fn centavos(x: f64) -> i64 {
    (x * 100.0).round() as i64
}

fn ultimo_traspaso() -> i64 {
    conexion().query_row("SELECT MAX(id) FROM transacciones_cuentas;", [], |r| r.get(0)).unwrap()
}

fn listado() -> Vec<TransaccionCuenta> {
    obtener_transacciones_cuentas().unwrap()
}

fn por_id(id: i64) -> TransaccionCuenta {
    listado().into_iter().find(|t| t.id == id).expect("en el listado")
}

type Caso = (String, i64, String, Option<f64>, Option<String>, String);

fn casos() -> Vec<Caso> {
    let c = conexion();
    let mut s = c.prepare("SELECT numero_caso, referencia_id, descripcion, importe, divisa, tipo FROM correcciones ORDER BY id;").unwrap();
    let v = s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))).unwrap().map(|x| x.unwrap()).collect();
    v
}

fn gasto_de_prueba(cuenta_id: i64) {
    let categoria: i64 = conexion().query_row("SELECT id FROM categorias WHERE nombre = 'Alimentación';", [], |r| r.get(0)).unwrap();
    crear_gasto(GastoInput {
        fecha: "05/10/2026".into(),
        monto: importe("321.47"),
        divisa: "DOP".into(),
        descripcion: "Compra inventada".into(),
        categoria_id: categoria,
        metodo_pago: "transferencia".into(),
        es_lbtr: false,
        tarjeta_id: None,
        cuenta_ahorro_id: Some(cuenta_id),
        tasa_cambio: None,
    })
    .unwrap();
}

#[test]
fn integracion_anular_un_traspaso_deja_original_y_compensacion_con_saldos_exactos() {
    let _g = entorno();
    let a = cuenta("Origen inventado", "DOP", 5_432.19);
    let b = cuenta("Destino inventado", "DOP", 101.07);
    transferir_entre_cuentas("01/11/2026".into(), a, b, importe("1234.57"), importe("1234.57"), importe("13.31"), "Pago inventado".into()).unwrap();
    let id = ultimo_traspaso();
    assert_eq!((centavos(saldo(a)), centavos(saldo(b))), (418_431, 133_564));

    let caso = eliminar_transaccion_cuenta(id, MOTIVO.into()).unwrap();

    assert_eq!((centavos(saldo(a)), centavos(saldo(b))), (543_219, 10_107), "como antes del traspaso");
    let comp_id = ultimo_traspaso();
    assert_eq!(comp_id, id + 1);
    let original = por_id(id);
    let comp = por_id(comp_id);
    assert_eq!((original.estado.as_str(), original.compensa_a), ("anulada", None));
    assert_eq!((comp.estado.as_str(), comp.compensa_a), ("compensacion", Some(id)));
    assert_eq!((comp.cuenta_origen_id, comp.cuenta_destino_id), (b, a));
    assert_eq!((centavos(comp.monto_origen), centavos(comp.monto_destino), centavos(comp.cargo)), (123_457, 123_457, 1_331));
    assert_eq!(comp.descripcion.as_deref(), Some(format!("Compensación de #{id}: Pago inventado").as_str()));
    assert_eq!(listado().iter().map(|t| t.id).collect::<Vec<_>>(), vec![comp_id, id], "la más nueva primero");

    let c = casos();
    assert_eq!(c.len(), 1);
    assert_eq!((c[0].0.as_str(), c[0].1, c[0].5.as_str()), (caso.as_str(), id, "traspaso"));
    assert_eq!(c[0].2, format!("Pago inventado (original #{id}, compensación #{comp_id})"));
    assert_eq!((c[0].3, c[0].4.as_deref()), (Some(1234.57), Some("DOP")));
}

#[test]
fn integracion_anular_un_cruce_en_dolares_anota_la_divisa_de_origen_y_restituye_ambas_cuentas() {
    let _g = entorno();
    let usd = cuenta("Dólares inventados", "USD", 777.13);
    let usd2 = cuenta("Otros dólares inventados", "USD", 3.03);
    let pesos = cuenta("Pesos inventados", "DOP", 9_999.99);
    transferir_entre_cuentas("01/11/2026".into(), usd, usd2, importe("200.07"), importe("200.07"), importe("0.00"), "usd".into()).unwrap();
    eliminar_transaccion_cuenta(ultimo_traspaso(), MOTIVO.into()).unwrap();
    transferir_entre_cuentas("02/11/2026".into(), pesos, usd, importe("6013.37"), importe("101.03"), importe("7.77"), "cruce".into()).unwrap();
    eliminar_transaccion_cuenta(ultimo_traspaso(), MOTIVO.into()).unwrap();

    assert_eq!((centavos(saldo(usd)), centavos(saldo(usd2)), centavos(saldo(pesos))), (77_713, 303, 999_999));
    let c = casos();
    assert_eq!((c[0].4.as_deref(), c[0].3), (Some("USD"), Some(200.07)), "antes decía DOP");
    assert_eq!((c[1].4.as_deref(), c[1].3), (Some("DOP"), Some(6013.37)), "la divisa del origen del cruce");
    let comp = listado().into_iter().find(|t| t.estado == "compensacion" && t.cuenta_origen_id == usd).unwrap();
    assert!((comp.tasa_cambio - 6013.37 / 101.03).abs() < 1e-3, "tasa inversa: {}", comp.tasa_cambio);
}

#[test]
fn integracion_las_reglas_de_anulacion_se_cumplen_por_el_comando() {
    let _g = entorno();
    let a = cuenta("Origen inventado", "DOP", 500.01);
    let b = cuenta("Destino inventado", "DOP", 0.0);
    transferir_entre_cuentas("01/11/2026".into(), a, b, importe("100.03"), importe("100.03"), importe("0.00"), "x".into()).unwrap();
    let id = ultimo_traspaso();

    assert_eq!(eliminar_transaccion_cuenta(999, "corto".into()).unwrap_err(), "No se encontró traspaso con identificador 999.");
    assert!(eliminar_transaccion_cuenta(id, "corto".into()).is_err());
    assert_eq!(listado().len(), 1, "un motivo flojo no deja compensación");
    assert!(casos().is_empty());

    eliminar_transaccion_cuenta(id, MOTIVO.into()).unwrap();
    let antes = (centavos(saldo(a)), centavos(saldo(b)), listado().len(), casos().len());
    let e = eliminar_transaccion_cuenta(id, MOTIVO.into()).unwrap_err();
    assert!(e.contains("ya está anulado"), "{e}");
    let e = eliminar_transaccion_cuenta(ultimo_traspaso(), MOTIVO.into()).unwrap_err();
    assert!(e.contains("compensación de un traspaso anulado"), "{e}");
    assert_eq!((centavos(saldo(a)), centavos(saldo(b)), listado().len(), casos().len()), antes, "sin efectos");
}

#[test]
fn integracion_una_cuenta_archivada_rechaza_la_anulacion_hasta_reactivarla() {
    let _g = entorno();
    let a = cuenta("A inventada", "DOP", 1_000.0);
    let b = cuenta("B inventada", "DOP", 0.0);
    transferir_entre_cuentas("01/11/2026".into(), a, b, importe("500.00"), importe("500.00"), importe("0.00"), "ida".into()).unwrap();
    let id = ultimo_traspaso();
    conexion().execute("UPDATE cuentas_ahorro SET balance_actual = 0 WHERE id = ?;", [b]).unwrap();
    crate::comandos::cuentas::archivar_cuenta(b).unwrap();

    let e = eliminar_transaccion_cuenta(id, MOTIVO.into()).unwrap_err();
    assert_eq!(e, "La cuenta «B inventada» está archivada: reactívala antes de usarla en una operación.");
    assert_eq!(listado().len(), 1);
    assert_eq!(listado()[0].estado, "vigente");

    crate::comandos::cuentas::reactivar_cuenta(b).unwrap();
    eliminar_transaccion_cuenta(id, MOTIVO.into()).unwrap();
    assert_eq!(listado().len(), 2);
}

#[test]
fn integracion_contrato_de_forma_del_listado_con_tipos_ipc() {
    let _g = entorno();
    let a = cuenta("A inventada", "DOP", 1_000.0);
    let b = cuenta("B inventada", "DOP", 0.0);
    transferir_entre_cuentas("01/11/2026".into(), a, b, importe("50.03"), importe("50.03"), importe("0.00"), "x".into()).unwrap();
    eliminar_transaccion_cuenta(ultimo_traspaso(), MOTIVO.into()).unwrap();

    let ts = include_str!("../../src/js/tipos-ipc.d.ts");
    let json = serde_json::to_value(listado()).unwrap();
    let declaradas = {
        let cab = "export type TransaccionCuenta = {";
        let ini = ts.find(cab).unwrap() + cab.len();
        let cuerpo = &ts[ini..ts[ini..].find("};").unwrap() + ini];
        let mut v: Vec<String> = cuerpo.split(';').filter_map(|c| c.split(':').next()).map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect();
        v.sort();
        v
    };
    for fila in json.as_array().unwrap() {
        let mut claves: Vec<String> = fila.as_object().unwrap().keys().cloned().collect();
        claves.sort();
        assert_eq!(claves, declaradas);
    }
    assert!(ts.contains("estado: string") && ts.contains("compensa_a: number | null"));
    assert!(ts.contains("eliminar_transaccion_cuenta: { args: { id: number; motivo: string }; ret: string }"));
}

#[test]
fn integracion_las_filas_anuladas_no_cambian_el_resumen_de_gastos_ni_el_patrimonio() {
    let _g = entorno();
    let a = cuenta("A inventada", "DOP", 10_001.37);
    let b = cuenta("B inventada", "USD", 55.55);
    gasto_de_prueba(a);
    let resumen_antes = serde_json::to_value(obtener_resumen_gastos(2026, 10, 3, 60.0, vec![]).unwrap()).unwrap();
    let patrimonio_antes = serde_json::to_value(obtener_patrimonio().unwrap()).unwrap();

    transferir_entre_cuentas("06/10/2026".into(), a, b, importe("6013.37"), importe("101.03"), importe("7.77"), "cruce".into()).unwrap();
    let con_traspaso = serde_json::to_value(obtener_patrimonio().unwrap()).unwrap();
    assert_ne!(con_traspaso, patrimonio_antes, "el traspaso vigente sí mueve el patrimonio (el cargo)");
    eliminar_transaccion_cuenta(ultimo_traspaso(), MOTIVO.into()).unwrap();

    assert_eq!(serde_json::to_value(obtener_resumen_gastos(2026, 10, 3, 60.0, vec![]).unwrap()).unwrap(), resumen_antes);
    assert_eq!(serde_json::to_value(obtener_patrimonio().unwrap()).unwrap(), patrimonio_antes, "dos filas anuladas no cuentan dos veces");
    assert_eq!(listado().len(), 2);
}

#[test]
fn integracion_respaldo_y_restauracion_conservan_estados_y_vinculos() {
    let _g = entorno();
    let a = cuenta("A inventada", "DOP", 2_000.0);
    let b = cuenta("B inventada", "DOP", 0.0);
    transferir_entre_cuentas("01/11/2026".into(), a, b, importe("700.09"), importe("700.09"), importe("3.03"), "x".into()).unwrap();
    let id = ultimo_traspaso();
    eliminar_transaccion_cuenta(id, MOTIVO.into()).unwrap();
    let antes = listado();
    let nombre = {
        let ruta = crate::comandos::respaldos::crear_respaldo().unwrap();
        std::path::Path::new(&ruta).file_name().unwrap().to_string_lossy().to_string()
    };
    transferir_entre_cuentas("02/11/2026".into(), a, b, importe("1.01"), importe("1.01"), importe("0.00"), "posterior".into()).unwrap();
    assert_eq!(listado().len(), 3);

    crate::comandos::respaldos::restaurar_respaldo(nombre).unwrap();

    let despues = listado();
    assert_eq!(despues.len(), 2);
    assert_eq!(serde_json::to_value(&despues).unwrap(), serde_json::to_value(&antes).unwrap());
    assert_eq!((despues[1].estado.as_str(), despues[0].compensa_a), ("anulada", Some(id)));
    assert_eq!((centavos(saldo(a)), centavos(saldo(b))), (200_000, 0));
}
