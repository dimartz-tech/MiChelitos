//! Pruebas de integración de la exportación para auditoría (`exportar_auditoria`).
//!
//! Ejercitan el **comando real** de punta a punta sobre una base SQLite temporal con el esquema real (el entorno
//! aislado de `caracterizacion`), sembrada con los comandos reales de `integracion_resumen`. Todos los datos son
//! inventados.
//!
//! Se reconocen por el prefijo `integracion_`: `cargo test integracion_`.

use crate::caracterizacion::entorno_para_pruebas_externas;
use crate::comandos::auditoria::{exportar_auditoria, ExportacionAuditoria};
use crate::exportacion::{directorio_de_auditoria, sha256_de};
use crate::integracion_resumen::{conexion, huella_de_la_base, sembrar, verificar_forma};
use crate::{db_nosql, migraciones};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::MutexGuard;

const TIPOS_IPC: &str = include_str!("../../src/js/tipos-ipc.d.ts");

fn entorno() -> MutexGuard<'static, ()> {
    let guarda = entorno_para_pruebas_externas();
    let destino = directorio_de_auditoria();
    assert!(destino.starts_with(std::env::temp_dir()), "ABORTADO: exportaría junto a los datos reales ({destino:?})");
    let _ = std::fs::remove_dir_all(&destino);
    let _ = std::fs::remove_file(db_nosql::obtener_ruta_nosql("capital"));
    sembrar();
    guarda
}

fn exportar() -> (ExportacionAuditoria, PathBuf) {
    let e = exportar_auditoria().expect("exportar");
    let carpeta = PathBuf::from(&e.carpeta);
    (e, carpeta)
}

fn manifiesto(carpeta: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(carpeta.join("manifiesto.json")).unwrap()).unwrap()
}

fn tablas() -> Vec<String> {
    let c = conexion();
    let mut s = c.prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name;").unwrap();
    let nombres = s.query_map([], |f| f.get::<_, String>(0)).unwrap();
    nombres.map(Result::unwrap).collect()
}

fn filas_de(tabla: &str) -> u64 {
    conexion().query_row(&format!("SELECT COUNT(*) FROM \"{tabla}\";"), [], |f| f.get::<_, i64>(0)).unwrap() as u64
}

/// Las líneas de datos de un CSV sin campos de varias líneas (los datos sembrados no los tienen).
fn lineas_de_datos(carpeta: &Path, archivo: &str) -> Vec<String> {
    let texto = std::fs::read_to_string(carpeta.join(archivo)).unwrap();
    assert!(texto.starts_with('\u{feff}'), "{archivo} debe empezar con la marca de UTF-8");
    texto.split("\r\n").skip(1).filter(|l| !l.is_empty()).map(str::to_string).collect()
}

#[test]
fn integracion_exporta_un_csv_por_cada_tabla_del_esquema_real_con_todas_sus_filas() {
    let _g = entorno();
    let (e, carpeta) = exportar();
    assert!(carpeta.starts_with(directorio_de_auditoria()));

    let m = manifiesto(&carpeta);
    let archivos = m["archivos"].as_array().unwrap();
    let esperadas = tablas();
    assert!(esperadas.len() >= 21, "el esquema real tiene al menos 21 tablas y hay {}", esperadas.len());
    let exportadas: Vec<&str> = archivos.iter().filter_map(|a| a["tabla"].as_str()).collect();
    assert_eq!(exportadas, esperadas.iter().map(String::as_str).collect::<Vec<_>>());

    let mut total = 0;
    for a in archivos {
        let tabla = a["tabla"].as_str().unwrap();
        let nombre = a["nombre"].as_str().unwrap();
        assert_eq!(nombre, format!("{tabla}.csv"));
        let filas = filas_de(tabla);
        assert_eq!(a["filas"], filas, "filas declaradas de {tabla}");
        assert_eq!(lineas_de_datos(&carpeta, nombre).len() as u64, filas, "líneas de {nombre}");
        total += filas;
    }
    assert!(total > 0, "la siembra debe dejar filas que exportar");
    assert_eq!((e.archivos as usize, e.filas as u64), (archivos.len(), total));
    assert_eq!(m["version_del_esquema"], migraciones::VERSION_OBJETIVO);
}

#[test]
fn integracion_los_gastos_exportados_son_los_de_la_base_valor_a_valor() {
    let _g = entorno();
    let (_e, carpeta) = exportar();
    let c = conexion();
    let mut s = c.prepare("SELECT id, monto, divisa FROM gastos ORDER BY rowid;").unwrap();
    let esperado: Vec<(i64, f64, String)> = s.query_map([], |f| Ok((f.get(0)?, f.get(1)?, f.get(2)?))).unwrap().map(Result::unwrap).collect();
    assert!(!esperado.is_empty());

    let texto = std::fs::read_to_string(carpeta.join("gastos.csv")).unwrap();
    let cabecera: Vec<&str> = texto.trim_start_matches('\u{feff}').split("\r\n").next().unwrap().split(',').collect();
    let (i_id, i_monto, i_divisa) = (
        cabecera.iter().position(|c| *c == "id").unwrap(),
        cabecera.iter().position(|c| *c == "monto").unwrap(),
        cabecera.iter().position(|c| *c == "divisa").unwrap(),
    );
    let lineas = lineas_de_datos(&carpeta, "gastos.csv");
    assert_eq!(lineas.len(), esperado.len());
    for (linea, (id, monto, divisa)) in lineas.iter().zip(&esperado) {
        // Las descripciones sembradas no llevan comas, así que partir por coma es exacto aquí.
        let campos: Vec<&str> = linea.split(',').collect();
        assert_eq!(campos.len(), cabecera.len(), "{linea}");
        assert_eq!(campos[i_id], id.to_string());
        // El importe vuelve al mismo f64: no se redondeó ni se reformateó.
        assert_eq!(campos[i_monto].parse::<f64>().unwrap().to_bits(), monto.to_bits(), "{linea}");
        assert_eq!(campos[i_divisa], divisa);
    }
}

#[test]
fn integracion_exportar_no_cambia_ni_una_fila_de_la_base() {
    let _g = entorno();
    let antes = huella_de_la_base();
    exportar();
    assert_eq!(huella_de_la_base(), antes);
}

#[test]
fn integracion_las_huellas_delatan_un_archivo_alterado() {
    let _g = entorno();
    let (e, carpeta) = exportar();
    let m = manifiesto(&carpeta);
    assert_eq!(sha256_de(&std::fs::read(carpeta.join("manifiesto.json")).unwrap()), e.huella_del_manifiesto);
    let gastos = m["archivos"].as_array().unwrap().iter().find(|a| a["nombre"] == "gastos.csv").unwrap();
    let ruta = carpeta.join("gastos.csv");
    assert_eq!(gastos["sha256"], sha256_de(&std::fs::read(&ruta).unwrap()));

    // Alguien cambia un importe a mano: hay que quitar antes la protección de solo lectura.
    let mut permisos = std::fs::metadata(&ruta).unwrap().permissions();
    assert!(permisos.readonly());
    #[allow(clippy::permissions_set_readonly_false)]
    permisos.set_readonly(false);
    std::fs::set_permissions(&ruta, permisos).unwrap();
    let mut contenido = std::fs::read(&ruta).unwrap();
    contenido.extend_from_slice(b"999,alterado\r\n");
    std::fs::write(&ruta, &contenido).unwrap();
    assert_ne!(gastos["sha256"], sha256_de(&std::fs::read(&ruta).unwrap()), "la huella debe dejar de coincidir");
}

#[test]
fn integracion_el_capital_va_en_la_exportacion_solo_si_existe() {
    let _g = entorno();
    let (_e, carpeta) = exportar();
    assert!(!carpeta.join("capital.json").exists());
    drop(_g);

    let _g = entorno();
    let ruta = db_nosql::obtener_ruta_nosql("capital");
    std::fs::create_dir_all(Path::new(&ruta).parent().unwrap()).unwrap();
    std::fs::write(&ruta, b"{\"certificados\": []}").unwrap();
    // Otra carpeta: la exportación anterior pudo caer en el mismo segundo.
    let _ = std::fs::remove_dir_all(directorio_de_auditoria());
    let (e, carpeta) = exportar();
    assert_eq!(std::fs::read(carpeta.join("capital.json")).unwrap(), b"{\"certificados\": []}");
    let m = manifiesto(&carpeta);
    let ultimo = m["archivos"].as_array().unwrap().last().unwrap().clone();
    assert_eq!((ultimo["nombre"].as_str(), ultimo["tabla"].is_null()), (Some("capital.json"), true));
    assert_eq!(e.archivos as usize, tablas().len() + 1);
    let _ = std::fs::remove_file(&ruta);
}

#[test]
fn integracion_contrato_de_forma_de_la_exportacion_con_tipos_ipc() {
    let _g = entorno();
    let (e, _carpeta) = exportar();
    verificar_forma(TIPOS_IPC, "ExportacionAuditoria", &serde_json::to_value(&e).unwrap(), "ExportacionAuditoria");
    let firma = "exportar_auditoria: { args: {  }; ret: ExportacionAuditoria }";
    assert!(TIPOS_IPC.contains(firma), "tipos-ipc.d.ts debe declarar: {firma}");
}
