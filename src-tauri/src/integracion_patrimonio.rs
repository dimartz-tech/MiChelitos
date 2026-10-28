//! Pruebas de integración del historial del patrimonio: los comandos reales de punta a punta.
//!
//! Cada prueba (prefijo `integracion_`) corre sobre una base SQLite temporal creada migrando desde cero con el esquema
//! real, y un `capital.json` temporal, ambos bajo un `HOME` aislado (ver `caracterizacion::entorno_para_pruebas_externas`).
//! Los datos son inventados y con céntimos poco redondos. El reloj se inyecta: las funciones `*_en` de
//! `comandos::fotos_patrimonio` reciben «hoy», así que un escenario de varios meses es una secuencia de fechas.

use crate::comandos::fotos_patrimonio::{asegurar_en, leer_fotos, tomar_en, FotoPatrimonio};
use crate::dominio::dinero::{Dinero, Divisa, TasaCambio};
use crate::{db_nosql, db_sql, ipc};
use chrono::NaiveDate;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::sync::MutexGuard;

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

fn dia(a: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(a, m, d).unwrap()
}

fn texto(centavos: i64) -> String {
    ipc::importe_a_texto(Dinero::de_centavos(centavos, Divisa::Dop))
}

fn capital_valido() -> Result<Value, String> {
    Ok(json!({
        "certificados": [{"banco": "Banco Uno", "monto": 500000.37, "tasa": 9.5, "vencimiento": "01/01/2031"}],
        "bolsa": [{"emisor": "Bolsa X", "monto": "20000.19", "tasa": 7.0, "vencimiento": "01/01/2031"}],
        "propiedades": {
            "inmobiliario": [{"id": "i1", "nombre": "Casa inventada", "valor_estimado": 4100000.91}],
            "vehiculos": [],
            "maquinaria": []
        }
    }))
}

/// Cuentas, tarjeta y préstamo inventados. Devuelve el patrimonio esperado en centavos **con la tasa dada**.
fn sembrar(tasa: TasaCambio) -> i64 {
    let c = conexion();
    c.execute_batch(
        "INSERT INTO cuentas_ahorro (nombre, divisa, balance_actual) VALUES ('Ahorro inventado', 'DOP', 12345.67);
         INSERT INTO cuentas_ahorro (nombre, divisa, balance_actual) VALUES ('Dólares inventados', 'USD', 987.65);
         INSERT INTO cuentas_ahorro (nombre, divisa, balance_actual, archivada) VALUES ('Vieja en cero', 'DOP', 0, 1);
         INSERT INTO cuentas_ahorro (nombre, divisa, balance_actual, archivada) VALUES ('Olvidada con saldo', 'DOP', 55.55, 1);
         UPDATE cuentas_ahorro SET balance_actual = 321.09 WHERE es_caja_efectivo = 1 AND divisa = 'DOP';
         INSERT INTO tarjetas (entidad, nombre_tarjeta, fecha_corte, fecha_limite_pago, balance_pesos, balance_dolares)
             VALUES ('Banco X', 'Visa inventada', 10, 25, 4321.09, 0);
         INSERT INTO prestamos (tipo_prestamo, monto_prestamo, institucion_financiera, tasa_actual, monto_cuota, dia_pago, saldo_actual)
             VALUES ('consumo', 80000.55, 'Banco Y', 12.5, 3500.25, 15, 70000.55);",
    )
    .unwrap();
    let usd_en_pesos = Dinero::de_centavos(987_65, Divisa::Usd).convertir(Divisa::Dop, tasa).unwrap().centavos();
    let activos = 12_345_67 + 321_09 + 55_55 + usd_en_pesos + 500_000_37 + 20_000_19 + 4_100_000_91;
    let pasivos = 4_321_09 + 70_000_55;
    activos - pasivos
}

fn ids_de_partidas_emitidas() -> i64 {
    conexion()
        .query_row("SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name = 'patrimonio_partidas'), 0)", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn integracion_foto_automatica_al_abrir_y_segunda_llamada_idempotente_sin_escribir() {
    let _g = entorno();
    let tasa = TasaCambio::nueva(60.0).unwrap();
    let esperado = sembrar(tasa);

    let a = asegurar_en(None, dia(2026, 3, 5), &capital_valido()).unwrap();
    assert!(a.creada);
    let foto = a.foto.unwrap();
    assert_eq!((foto.origen.as_str(), foto.calidad.as_str(), foto.sellada), ("real", "completa", false));
    assert_eq!(foto.patrimonio, texto(esperado));
    assert_eq!(foto.tasa_usd_dop, "60.00", "la sugerida");
    assert_eq!(foto.fecha, "05/03/2026");

    let partidas_antes = ids_de_partidas_emitidas();
    let b = asegurar_en(None, dia(2026, 3, 28), &capital_valido()).unwrap();
    assert!(!b.creada && b.foto.is_none());
    assert_eq!(ids_de_partidas_emitidas(), partidas_antes, "la segunda llamada no escribió ni una partida");
    assert_eq!(leer_fotos().unwrap().len(), 1);
}

#[test]
fn integracion_las_cuentas_archivadas_y_las_partidas_siguen_la_regla() {
    let _g = entorno();
    sembrar(TasaCambio::nueva(60.0).unwrap());
    let foto = asegurar_en(None, dia(2026, 3, 5), &capital_valido()).unwrap().foto.unwrap();
    let refs: Vec<&str> = foto.partidas.iter().map(|p| p.referencia.as_str()).collect();
    assert!(!refs.contains(&"Vieja en cero"), "archivada en cero no genera partida");
    assert!(refs.contains(&"Olvidada con saldo"), "archivada con saldo sí");
    let rubro = |r: &str| foto.partidas.iter().filter(|p| p.rubro == r).count();
    assert_eq!((rubro("certificado"), rubro("bolsa"), rubro("inmobiliario"), rubro("tarjeta"), rubro("prestamo")), (1, 1, 1, 1, 1));
    assert!(foto.partidas.iter().any(|p| p.rubro == "efectivo" && p.importe == "321.09"));
}

#[test]
fn integracion_varios_meses_sellado_foto_manual_y_tasa_por_foto() {
    let _g = entorno();
    sembrar(TasaCambio::nueva(60.0).unwrap());
    let marzo = asegurar_en(None, dia(2026, 3, 5), &capital_valido()).unwrap().foto.unwrap();

    // Abril: llega el mes siguiente. Marzo se sella; abril nace sin sellar.
    let abril = asegurar_en(Some("57.25"), dia(2026, 4, 2), &capital_valido()).unwrap().foto.unwrap();
    let fotos = leer_fotos().unwrap();
    assert_eq!(fotos.iter().map(|f| (f.mes, f.sellada)).collect::<Vec<_>>(), vec![(3, true), (4, false)]);

    // La tasa va por foto: los mismos dólares valen distinto en cada una, y marzo no se movió.
    let dolares = |f: &FotoPatrimonio| f.partidas.iter().find(|p| p.referencia == "Dólares inventados").unwrap().importe_dop.clone();
    assert_eq!(marzo.tasa_usd_dop, "60.00");
    assert_eq!(abril.tasa_usd_dop, "57.25");
    assert_ne!(dolares(&marzo), dolares(&abril));
    assert_eq!(dolares(&abril), ipc::importe_a_texto(Dinero::de_centavos(987_65, Divisa::Usd).convertir(Divisa::Dop, TasaCambio::nueva(57.25).unwrap()).unwrap()));
    assert_eq!(dolares(&fotos[0]), dolares(&marzo));

    // Foto manual en abril: reemplaza la de abril (una por mes); en marzo, ya sellado, no se puede.
    conexion().execute("UPDATE cuentas_ahorro SET balance_actual = 1.11 WHERE nombre = 'Ahorro inventado'", []).unwrap();
    let manual = tomar_en("manual", Some("58.10"), dia(2026, 4, 20), &capital_valido()).unwrap();
    assert_eq!((manual.origen.as_str(), manual.tasa_usd_dop.as_str()), ("manual", "58.10"));
    assert_eq!(leer_fotos().unwrap().len(), 2, "sigue habiendo una foto por mes");
    let e = tomar_en("manual", None, dia(2026, 3, 30), &capital_valido()).unwrap_err();
    assert!(e.contains("sellada"), "{e}");
    assert_eq!(leer_fotos().unwrap()[0].patrimonio, marzo.patrimonio, "marzo quedó intacto");

    // Mayo sella abril con la foto manual dentro.
    asegurar_en(None, dia(2026, 5, 1), &capital_valido()).unwrap();
    let fotos = leer_fotos().unwrap();
    assert_eq!(fotos.iter().map(|f| (f.mes, f.sellada)).collect::<Vec<_>>(), vec![(3, true), (4, true), (5, false)]);
    assert_eq!(fotos[1].origen, "manual");
}

#[test]
fn integracion_un_origen_o_una_tasa_invalidos_se_rechazan_sin_escribir() {
    let _g = entorno();
    for (origen, tasa) in [("real", None), ("reconstruida", None), ("otro", None), ("manual", Some("abc")), ("manual", Some("0")), ("manual", Some("-3"))] {
        assert!(tomar_en(origen, tasa, dia(2026, 3, 5), &capital_valido()).is_err(), "{origen} {tasa:?}");
    }
    assert!(leer_fotos().unwrap().is_empty());
}

#[test]
fn integracion_un_capital_ilegible_deja_la_foto_con_aviso_y_cuenta_cero() {
    let _g = entorno();
    sembrar(TasaCambio::nueva(60.0).unwrap());
    // El archivo existe pero está dañado: `leer_coleccion` da error, no un capital vacío.
    let ruta = db_nosql::obtener_ruta_nosql("capital");
    std::fs::create_dir_all(std::path::Path::new(&ruta).parent().unwrap()).unwrap();
    std::fs::write(&ruta, "{ esto no es json").unwrap();
    let capital = db_nosql::leer_coleccion("capital");
    assert!(capital.is_err());

    let foto = asegurar_en(None, dia(2026, 3, 5), &capital).unwrap().foto.unwrap();
    assert_eq!(foto.calidad, "con_aviso");
    assert!(!foto.partidas.iter().any(|p| ["certificado", "bolsa", "inmobiliario", "vehiculo", "maquinaria"].contains(&p.rubro.as_str())));
    // Lo que sí se podía leer, se contó.
    assert!(foto.partidas.iter().any(|p| p.referencia == "Ahorro inventado"));
    assert_eq!(std::fs::read_to_string(&ruta).unwrap(), "{ esto no es json", "no se tocó el archivo dañado");
}

#[test]
fn integracion_guardar_el_capital_toma_la_foto_y_si_la_foto_falla_el_guardado_no() {
    let _g = entorno();
    sembrar(TasaCambio::nueva(60.0).unwrap());
    let hoy = chrono::Local::now().date_naive();
    let datos = json!({
        "certificados": [{"banco": "Banco Uno", "monto": "1500.37", "tasa": 9.5, "vencimiento": "01/01/2031"}],
        "bolsa": [], "propiedades": {"inmobiliario": [], "vehiculos": [], "maquinaria": []}
    });

    let aviso = crate::comandos::capital::guardar_capital(datos.clone()).unwrap();
    assert_eq!(aviso, None);
    let fotos = leer_fotos().unwrap();
    assert_eq!(fotos.len(), 1);
    assert_eq!(fotos[0].origen, "capital");
    assert_eq!((fotos[0].anio, fotos[0].mes), (chrono::Datelike::year(&hoy), chrono::Datelike::month(&hoy)));
    assert!(fotos[0].partidas.iter().any(|p| p.rubro == "certificado" && p.importe == "1500.37"));

    // Si la foto no se puede guardar, el capital se guarda igual y se devuelve el aviso.
    conexion().execute_batch("DELETE FROM patrimonio_fotos; CREATE TRIGGER falla_a_proposito BEFORE INSERT ON patrimonio_fotos BEGIN SELECT RAISE(ABORT, 'boom'); END;").unwrap();
    let mut otro = datos.clone();
    otro["certificados"][0]["monto"] = json!("2500.99");
    let aviso = crate::comandos::capital::guardar_capital(otro).unwrap();
    assert!(aviso.unwrap().contains("no se pudo tomar la foto"));
    let guardado = db_nosql::leer_coleccion("capital").unwrap();
    assert_eq!(guardado["certificados"][0]["monto"].as_f64(), Some(2500.99), "el capital sí quedó guardado");
}

#[test]
fn integracion_el_respaldo_y_la_restauracion_conservan_las_fotos() {
    let _g = entorno();
    sembrar(TasaCambio::nueva(60.0).unwrap());
    db_nosql::guardar_coleccion("capital", &capital_valido().unwrap()).unwrap();
    let marzo = asegurar_en(None, dia(2026, 3, 5), &capital_valido()).unwrap().foto.unwrap();

    let copia = crate::respaldo::respaldar("con fotos").unwrap();
    let nombre = copia.file_name().unwrap().to_string_lossy().to_string();

    // Después del respaldo pasa un mes: marzo se sella y nace abril.
    asegurar_en(None, dia(2026, 4, 2), &capital_valido()).unwrap();
    assert_eq!(leer_fotos().unwrap().len(), 2);

    crate::respaldo::restaurar(&nombre).unwrap();
    let fotos = leer_fotos().unwrap();
    assert_eq!(fotos.len(), 1, "la restauración devolvió el historial de entonces");
    assert_eq!(fotos[0].patrimonio, marzo.patrimonio);
    assert_eq!(fotos[0].partidas.len(), marzo.partidas.len());
    assert!(!fotos[0].sellada);
}

#[test]
fn integracion_las_fotos_se_listan_ordenadas_por_anio_y_mes() {
    let _g = entorno();
    sembrar(TasaCambio::nueva(60.0).unwrap());
    for (a, m) in [(2026, 1), (2025, 11), (2025, 12)] {
        // En desorden de calendario: cada una sella las anteriores a ella, no las posteriores.
        tomar_en("manual", None, dia(a, m, 10), &capital_valido()).unwrap();
    }
    let orden: Vec<(i32, u32)> = leer_fotos().unwrap().iter().map(|f| (f.anio, f.mes)).collect();
    assert_eq!(orden, vec![(2025, 11), (2025, 12), (2026, 1)]);
}

// --- Contrato de forma: lo que Rust serializa es lo que declara src/js/tipos-ipc.d.ts ---

const TIPOS_IPC: &str = include_str!("../../src/js/tipos-ipc.d.ts");

/// Los campos `(clave, tipo de TypeScript)` de `export type <nombre> = { ... };`.
fn campos_declarados(nombre: &str) -> Vec<(String, String)> {
    let cabecera = format!("export type {nombre} = {{");
    let linea = TIPOS_IPC.lines().find(|l| l.starts_with(&cabecera)).unwrap_or_else(|| panic!("tipos-ipc.d.ts no declara {nombre}"));
    let cuerpo = linea.trim_start_matches(&cabecera).trim_end().trim_end_matches(';').trim_end_matches('}');
    cuerpo
        .split(';')
        .filter(|c| !c.trim().is_empty())
        .map(|c| {
            let (k, t) = c.split_once(':').expect("campo con clave: tipo");
            (k.trim().to_string(), t.trim().to_string())
        })
        .collect()
}

fn comprobar_forma(nombre: &str, valor: &Value) {
    let objeto = valor.as_object().unwrap_or_else(|| panic!("{nombre} no serializa como objeto"));
    let declarados = campos_declarados(nombre);
    let mut esperadas: Vec<&str> = declarados.iter().map(|(k, _)| k.as_str()).collect();
    let mut reales: Vec<&str> = objeto.keys().map(String::as_str).collect();
    esperadas.sort();
    reales.sort();
    assert_eq!(reales, esperadas, "{nombre}: las claves de Rust y de tipos-ipc.d.ts no coinciden");
    for (clave, tipo) in declarados {
        let v = &objeto[&clave];
        let admite_nulo = tipo.contains("| null");
        let base = tipo.replace("| null", "");
        let base = base.trim();
        let ok = (admite_nulo && v.is_null())
            || match base {
                "string" => v.is_string(),
                "number" => v.is_number(),
                "boolean" => v.is_boolean(),
                b if b.ends_with("[]") => v.is_array(),
                _ => v.is_object(),
            };
        assert!(ok, "{nombre}.{clave}: tipo declarado «{tipo}», valor {v}");
    }
}

#[test]
fn integracion_contrato_de_forma_los_dto_reales_coinciden_con_tipos_ipc() {
    let _g = entorno();
    sembrar(TasaCambio::nueva(60.0).unwrap());
    let asegurada = asegurar_en(None, dia(2026, 3, 5), &capital_valido()).unwrap();
    let valor = serde_json::to_value(&asegurada).unwrap();
    comprobar_forma("FotoAsegurada", &valor);
    comprobar_forma("FotoPatrimonio", &valor["foto"]);
    comprobar_forma("PartidaFoto", &valor["foto"]["partidas"][0]);

    // La rama «ya existía» también respeta la forma (foto: null).
    let repetida = serde_json::to_value(asegurar_en(None, dia(2026, 3, 6), &capital_valido()).unwrap()).unwrap();
    comprobar_forma("FotoAsegurada", &repetida);

    let lista = serde_json::to_value(leer_fotos().unwrap()).unwrap();
    comprobar_forma("FotoPatrimonio", &lista[0]);
    let manual = serde_json::to_value(tomar_en("manual", None, dia(2026, 3, 7), &capital_valido()).unwrap()).unwrap();
    comprobar_forma("FotoPatrimonio", &manual);

    // Los comandos, con las claves de sus argumentos tal como las envía Tauri.
    for linea in [
        "tomar_foto_patrimonio: { args: { origen: string; tasaUsdDop?: string | null }; ret: FotoPatrimonio };",
        "asegurar_foto_del_mes: { args: { tasaUsdDop?: string | null }; ret: FotoAsegurada };",
        "obtener_fotos_patrimonio: { args: {  }; ret: FotoPatrimonio[] };",
        "guardar_capital: { args: { data: any }; ret: string | null };",
    ] {
        assert!(TIPOS_IPC.contains(linea), "tipos-ipc.d.ts no declara: {linea}");
    }
}

#[test]
fn integracion_todo_importe_de_la_foto_viaja_como_texto_exacto() {
    let _g = entorno();
    sembrar(TasaCambio::nueva(60.0).unwrap());
    let foto = asegurar_en(None, dia(2026, 3, 5), &capital_valido()).unwrap().foto.unwrap();
    let es_importe = |s: &str| {
        let (e, d) = s.trim_start_matches('-').split_once('.').unwrap_or_else(|| panic!("{s}"));
        !e.is_empty() && d.len() == 2 && e.chars().all(|c| c.is_ascii_digit()) && d.chars().all(|c| c.is_ascii_digit())
    };
    for s in [&foto.activos, &foto.pasivos, &foto.patrimonio, &foto.liquido] {
        assert!(es_importe(s), "{s}");
    }
    for p in &foto.partidas {
        assert!(es_importe(&p.importe) && es_importe(&p.importe_dop), "{p:?}");
    }
}
