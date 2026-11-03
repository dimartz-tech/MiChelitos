//! Pruebas de integración del caso de corrección de un traspaso borrado (abordaje B: foto completa en `detalle`).
//!
//! **PROTOTIPO** (rama `modelo/traspasos-b-foto-completa`; migración 19 de prototipo). Ejercitan los comandos reales
//! sobre una base SQLite temporal con el esquema real (mismo entorno aislado de `caracterizacion`): se siembran cuentas,
//! se transfiere, se elimina el traspaso con el comando real, se lista con `obtener_correcciones` y se comprueba que el
//! JSON del detalle permite reconstruir el traspaso completo. Todos los datos son inventados.
//!
//! Se reconocen por el prefijo `integracion_`: `cargo test integracion_`.

use crate::caracterizacion::entorno_para_pruebas_externas;
use crate::comandos::correcciones::obtener_correcciones;
use crate::comandos::cuentas::{
    crear_cuenta, eliminar_transaccion_cuenta, obtener_cuentas, obtener_transacciones_cuentas, transferir_entre_cuentas,
};
use crate::dominio::detalle_traspaso::DetalleDeTraspaso;
use crate::ipc::ImporteDecimal;
use crate::{db_sql, migraciones};
use rusqlite::Connection;
use serde_json::Value;

const MOTIVO: &str = "Traspaso registrado en la cuenta equivocada";
const TIPOS_IPC: &str = include_str!("../../src/js/tipos-ipc.d.ts");

fn importe(texto: &str) -> ImporteDecimal {
    ImporteDecimal::desde_texto(texto).expect("importe de prueba")
}

fn cuenta(nombre: &str, divisa: &str, saldo: &str) -> i64 {
    crear_cuenta(nombre.into(), divisa.into(), importe(saldo), None, None).expect("crear cuenta")
}

fn saldo(id: i64) -> f64 {
    obtener_cuentas().unwrap().into_iter().find(|c| c.id == id).expect("cuenta").balance_actual
}

fn traspasar(fecha: &str, o: i64, d: i64, monto_o: &str, monto_d: &str, cargo: &str, desc: &str) -> i64 {
    transferir_entre_cuentas(fecha.into(), o, d, importe(monto_o), importe(monto_d), importe(cargo), desc.into())
        .expect("transferir");
    obtener_transacciones_cuentas().unwrap().iter().map(|t| t.id).max().expect("hay traspaso")
}

fn detalle_del_caso(indice: usize) -> (Value, DetalleDeTraspaso) {
    let casos = obtener_correcciones().unwrap();
    let texto = casos[indice].detalle.clone().expect("el caso lleva detalle");
    (serde_json::from_str(&texto).expect("JSON bien formado"), DetalleDeTraspaso::desde_json(&texto).unwrap())
}

#[test]
fn integracion_traspaso_usd_a_usd_se_anota_en_usd_y_los_saldos_vuelven() {
    let _g = entorno_para_pruebas_externas();
    let a = cuenta("Origen dólares", "USD", "500.00");
    let b = cuenta("Destino dólares", "USD", "20.00");
    let (sa, sb) = (saldo(a), saldo(b));
    let id = traspasar("13/10/2026", a, b, "200.37", "200.37", "0.00", "Ahorro en dólares");
    assert_eq!(saldo(b), sb + 200.37, "el traspaso mueve el destino");

    let caso = eliminar_transaccion_cuenta(id, MOTIVO.into()).unwrap();

    let casos = obtener_correcciones().unwrap();
    assert_eq!(casos[0].numero_caso, caso);
    assert_eq!(casos[0].divisa.as_deref(), Some("USD"), "ya no dice DOP");
    assert_eq!(casos[0].importe, Some(200.37));
    let (_, d) = detalle_del_caso(0);
    assert_eq!((d.origen.importe.monto.as_str(), d.origen.importe.divisa.as_str()), ("200.37", "USD"));
    assert_eq!((d.destino.importe.monto.as_str(), d.destino.importe.divisa.as_str()), ("200.37", "USD"));
    assert_eq!(d.tasa, None);
    assert_eq!(saldo(a), sa, "saldos como antes");
    assert_eq!(saldo(b), sb);
    assert!(obtener_transacciones_cuentas().unwrap().is_empty(), "el asiento se retiró");
}

#[test]
fn integracion_el_detalle_de_un_dop_a_usd_con_tasa_y_cargo_permite_reconstruir_el_traspaso() {
    let _g = entorno_para_pruebas_externas();
    let pesos = cuenta("Ahorros <Pesos>", "DOP", "90000.00");
    let dolares = cuenta("Dólares 'ahorro'", "USD", "10.00");
    let (sp, sd) = (saldo(pesos), saldo(dolares));
    let id = traspasar("07/10/2026", pesos, dolares, "5875.37", "100.00", "25.55", "Compra de dólares");
    let (sp_tras, sd_tras) = (saldo(pesos), saldo(dolares));
    assert_ne!((sp_tras, sd_tras), (sp, sd));

    eliminar_transaccion_cuenta(id, MOTIVO.into()).unwrap();

    assert_eq!((saldo(pesos), saldo(dolares)), (sp, sd), "los saldos vuelven a lo de antes");
    let casos = obtener_correcciones().unwrap();
    assert_eq!((casos[0].divisa.as_deref(), casos[0].importe), (Some("DOP"), Some(5875.37)));
    let (json, d) = detalle_del_caso(0);

    // La forma documentada del JSON.
    assert_eq!(json["version"], 1);
    assert_eq!(json["traspaso_id"], id);
    assert_eq!(json["fecha"], "07/10/2026");
    assert_eq!(json["descripcion"], "Compra de dólares");
    assert_eq!(json["origen"]["cuenta_id"], pesos);
    assert_eq!(json["origen"]["cuenta_nombre"], "Ahorros <Pesos>");
    assert_eq!(json["origen"]["importe"], serde_json::json!({"monto": "5875.37", "divisa": "DOP"}));
    assert_eq!(json["destino"]["cuenta_id"], dolares);
    assert_eq!(json["destino"]["cuenta_nombre"], "Dólares 'ahorro'");
    assert_eq!(json["destino"]["importe"], serde_json::json!({"monto": "100.00", "divisa": "USD"}));
    assert!(json["tasa"].is_string(), "la tasa viaja como texto exacto");
    assert_eq!(json["cargo"], serde_json::json!({"monto": "25.55", "divisa": "DOP"}));

    // Reconstrucción: rehacer el traspaso con lo que dice el detalle deja los saldos como tras el original.
    let nuevo = traspasar(
        &d.fecha,
        d.origen.cuenta_id,
        d.destino.cuenta_id,
        &d.origen.importe.monto,
        &d.destino.importe.monto,
        &d.cargo.monto,
        d.descripcion.as_deref().unwrap_or(""),
    );
    assert_eq!((saldo(pesos), saldo(dolares)), (sp_tras, sd_tras), "el detalle basta para rehacer el traspaso");
    let t = obtener_transacciones_cuentas().unwrap().into_iter().find(|t| t.id == nuevo).unwrap();
    assert_eq!((t.cuenta_origen_nombre.as_str(), t.cuenta_destino_nombre.as_str()), ("Ahorros <Pesos>", "Dólares 'ahorro'"));
}

#[test]
fn integracion_un_traspaso_sin_descripcion_ni_cargo_deja_descripcion_nula_en_el_detalle() {
    let _g = entorno_para_pruebas_externas();
    let a = cuenta("Uno", "DOP", "1234.56");
    let b = cuenta("Dos", "DOP", "0.00");
    let id = traspasar("01/11/2026", a, b, "333.33", "333.33", "0.00", "");

    eliminar_transaccion_cuenta(id, MOTIVO.into()).unwrap();

    let (json, _) = detalle_del_caso(0);
    assert!(json["descripcion"].is_null());
    assert!(json["tasa"].is_null(), "sin cambio de divisa no hay tasa");
    assert_eq!(obtener_correcciones().unwrap()[0].descripcion, "Traspaso entre cuentas");
}

#[test]
fn integracion_un_motivo_invalido_no_anota_caso_ni_toca_los_saldos() {
    let _g = entorno_para_pruebas_externas();
    let a = cuenta("Uno", "DOP", "1234.56");
    let b = cuenta("Dos", "DOP", "0.00");
    let id = traspasar("01/11/2026", a, b, "100.00", "100.00", "0.00", "x");
    let (sa, sb) = (saldo(a), saldo(b));

    assert!(eliminar_transaccion_cuenta(id, "corto".into()).is_err());

    assert!(obtener_correcciones().unwrap().is_empty());
    assert_eq!((saldo(a), saldo(b)), (sa, sb));
}

#[test]
fn integracion_contrato_de_forma_del_dto_con_tipos_ipc() {
    let _g = entorno_para_pruebas_externas();
    let a = cuenta("Uno", "DOP", "1234.56");
    let b = cuenta("Dos", "USD", "0.00");
    let id = traspasar("01/11/2026", a, b, "590.00", "10.00", "1.10", "contrato");
    eliminar_transaccion_cuenta(id, MOTIVO.into()).unwrap();

    let casos = serde_json::to_value(obtener_correcciones().unwrap()).unwrap();
    let mut reales: Vec<String> = casos[0].as_object().unwrap().keys().cloned().collect();
    reales.sort();
    let cabecera = "export type CasoCorreccion = {";
    let inicio = TIPOS_IPC.find(cabecera).expect("tipos-ipc.d.ts declara CasoCorreccion") + cabecera.len();
    let cuerpo = &TIPOS_IPC[inicio..inicio + TIPOS_IPC[inicio..].find("};").unwrap()];
    let mut declaradas: Vec<String> =
        cuerpo.split(';').filter_map(|c| c.split(':').next()).map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect();
    declaradas.sort();
    assert_eq!(reales, declaradas, "las claves de Rust y de tipos-ipc.d.ts no coinciden");
    assert!(cuerpo.contains("detalle: string | null"), "el detalle viaja como texto JSON o null");
    assert!(casos[0]["detalle"].is_string());
}

#[test]
fn integracion_respaldo_y_restauracion_conservan_el_detalle() {
    let _g = entorno_para_pruebas_externas();
    let a = cuenta("Uno", "DOP", "5432.10");
    let b = cuenta("Dos", "USD", "0.00");
    let id1 = traspasar("01/11/2026", a, b, "590.00", "10.00", "1.10", "primero");
    eliminar_transaccion_cuenta(id1, MOTIVO.into()).unwrap();
    let antes = obtener_correcciones().unwrap();
    let nombre = crate::comandos::respaldos::crear_respaldo().unwrap();
    let nombre = std::path::Path::new(&nombre).file_name().unwrap().to_string_lossy().to_string();

    // Después del respaldo ocurre otro borrado, que la restauración debe deshacer.
    let id2 = traspasar("02/11/2026", a, b, "118.00", "2.00", "0.00", "segundo");
    eliminar_transaccion_cuenta(id2, MOTIVO.into()).unwrap();
    assert_eq!(obtener_correcciones().unwrap().len(), 2);

    let r = crate::comandos::respaldos::restaurar_respaldo(nombre).unwrap();

    assert_eq!(r["version_del_esquema"], migraciones::VERSION_OBJETIVO);
    let despues = obtener_correcciones().unwrap();
    assert_eq!(despues.len(), 1, "volvió al estado del respaldo");
    assert_eq!(despues[0].detalle, antes[0].detalle, "el detalle sobrevive al respaldo y a la restauración");
    assert!(DetalleDeTraspaso::desde_json(despues[0].detalle.as_deref().unwrap()).is_ok());
}

#[test]
fn integracion_migrar_una_base_v18_con_casos_viejos_los_conserva_sin_detalle_y_los_nuevos_lo_llevan() {
    let _g = entorno_para_pruebas_externas();
    // Se rehace la base como la dejaría la versión 18 (instalada), con casos de antes del detalle.
    let ruta = db_sql::obtener_ruta_db();
    assert!(ruta.starts_with(std::env::temp_dir().to_str().unwrap()), "ABORTADO: apuntaría a la base real ({ruta})");
    let _ = std::fs::remove_file(&ruta);
    for sufijo in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{ruta}{sufijo}"));
    }
    {
        let mut c = Connection::open(&ruta).unwrap();
        migraciones::migrar_hasta(&mut c, 18);
        assert_eq!(migraciones::version_de(&c).unwrap(), 18);
        c.execute_batch(
            "INSERT INTO cuentas_ahorro (nombre, divisa, balance_actual) VALUES ('Ahorro inventado', 'DOP', 12345.67), ('Dólares inventados', 'USD', 987.65);
             INSERT INTO correcciones (numero_caso, fecha, tipo, referencia_id, descripcion, importe, divisa, motivo)
                 VALUES ('COR-2026-0001', '01/10/2026', 'traspaso', 4, 'Traspaso viejo', 1187.53, 'DOP', 'Motivo anterior al detalle'),
                        ('COR-2026-0002', '02/10/2026', 'gasto', 9, 'Gasto viejo', 33.37, 'DOP', 'Otro motivo anterior al detalle');",
        )
        .unwrap();
    }

    db_sql::inicializar_db().expect("la base v18 migra a v19");

    let viejos = obtener_correcciones().unwrap();
    assert_eq!(viejos.len(), 2);
    assert!(viejos.iter().all(|c| c.detalle.is_none()), "los casos viejos quedan sin detalle");
    assert_eq!((viejos[1].descripcion.as_str(), viejos[1].importe), ("Traspaso viejo", Some(1187.53)));

    let ahorro = obtener_cuentas().unwrap().into_iter().find(|c| c.nombre == "Ahorro inventado").unwrap().id;
    let dolares = obtener_cuentas().unwrap().into_iter().find(|c| c.nombre == "Dólares inventados").unwrap().id;
    let id = traspasar("03/11/2026", ahorro, dolares, "5875.37", "100.00", "0.00", "nuevo");
    eliminar_transaccion_cuenta(id, MOTIVO.into()).unwrap();
    let todos = obtener_correcciones().unwrap();
    assert_eq!(todos.len(), 3);
    assert!(todos[0].detalle.is_some(), "el caso nuevo sí lleva detalle");
    assert!(todos[1].detalle.is_none() && todos[2].detalle.is_none());
}
