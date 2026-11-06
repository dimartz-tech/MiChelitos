//! Pruebas de integración de renombrar y fusionar categorías (`renombrar_categoria`,
//! `previsualizar_fusion_categorias`, `fusionar_categorias`).
//!
//! Ejercitan los **comandos reales** de punta a punta sobre una base SQLite temporal con el esquema real (el entorno
//! aislado de `caracterizacion`): se siembran categorías y gastos con los comandos reales, se renombra y se fusiona, y
//! se comprueba lo que quedó en la base y lo que ve el resto de la aplicación (el Resumen, el rastro de
//! correcciones, un respaldo restaurado, y las operaciones del sistema que buscan categorías por nombre). Todos los
//! datos son inventados, con céntimos poco redondos.
//!
//! Se reconocen por el prefijo `integracion_`: `cargo test integracion_`.

use crate::caracterizacion::entorno_para_pruebas_externas;
use crate::comandos::catalogos::{
    crear_categoria, eliminar_categoria, fusionar_categorias, marcar_categoria_fija, obtener_categorias,
    previsualizar_fusion_categorias, renombrar_categoria,
};
use crate::comandos::correcciones::obtener_correcciones;
use crate::comandos::gastos::{crear_gasto, GastoInput};
use crate::comandos::resumen::resumen_en;
use crate::db_sql;
use crate::ipc::ImporteDecimal;
use chrono::NaiveDate;
use rusqlite::Connection;

const TIPOS_IPC: &str = include_str!("../../src/js/tipos-ipc.d.ts");

// --- Ayudantes ---

fn conexion() -> Connection {
    db_sql::obtener_conexion().expect("abrir conexión de prueba")
}

fn cat_id(nombre: &str) -> i64 {
    conexion().query_row("SELECT id FROM categorias WHERE nombre = ?;", [nombre], |r| r.get(0)).unwrap()
}

fn existe(nombre: &str) -> bool {
    conexion().query_row("SELECT COUNT(*) FROM categorias WHERE nombre = ?;", [nombre], |r| r.get::<_, i64>(0)).unwrap() > 0
}

fn nueva(nombre: &str) -> i64 {
    crear_categoria(nombre.into()).unwrap().id
}

fn cuenta() -> i64 {
    let c = conexion();
    c.execute(
        "INSERT INTO cuentas_ahorro (nombre, divisa, balance_actual) VALUES ('Cuenta de prueba', 'DOP', 9000000.0);",
        [],
    )
    .unwrap();
    c.last_insert_rowid()
}

fn tarjeta() -> i64 {
    let c = conexion();
    c.execute(
        "INSERT INTO tarjetas (entidad, nombre_tarjeta, fecha_corte, fecha_limite_pago, balance_pesos, balance_dolares,
                               limite_pesos, limite_dolares, politica_liquidacion)
         VALUES ('Banco Ejemplo', 'Tarjeta Ejemplo', 15, 5, 0.0, 0.0, 900000.0, 9000.0, 'traduce');",
        [],
    )
    .unwrap();
    c.last_insert_rowid()
}

/// Un gasto por transferencia desde la cuenta (la retención queda como costo adicional).
fn gasto(fecha: &str, monto: &str, categoria: &str, descripcion: &str, cuenta: i64) -> i64 {
    crear_gasto(GastoInput {
        fecha: fecha.into(),
        monto: ImporteDecimal::desde_texto(monto).unwrap(),
        divisa: "DOP".into(),
        descripcion: descripcion.into(),
        categoria_id: cat_id(categoria),
        metodo_pago: "transferencia".into(),
        es_lbtr: false,
        tarjeta_id: None,
        cuenta_ahorro_id: Some(cuenta),
        tasa_cambio: None,
    })
    .unwrap()
}

/// Todo el contenido de la base como texto, tabla por tabla.
fn huella_de_la_base() -> String {
    let c = conexion();
    let tablas: Vec<String> = c
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name;")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let mut huella = String::new();
    for tabla in tablas {
        let mut stmt = c.prepare(&format!("SELECT * FROM \"{tabla}\";")).unwrap();
        let columnas = stmt.column_count();
        let filas: Vec<String> = stmt
            .query_map([], |r| {
                let mut f = String::new();
                for i in 0..columnas {
                    f.push_str(&format!("{:?};", r.get::<_, rusqlite::types::Value>(i)?));
                }
                Ok(f)
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        huella.push_str(&format!("{tabla}:{}\n", filas.join("|")));
    }
    huella
}

/// Los gastos con TODAS sus columnas menos la categoría, ordenados por id.
fn gastos_sin_categoria() -> Vec<String> {
    let c = conexion();
    let mut stmt = c
        .prepare(
            "SELECT id, fecha, monto, divisa, descripcion, metodo_pago, costo_adicional, tarjeta_id, cuenta_ahorro_id
             FROM gastos ORDER BY id;",
        )
        .unwrap();
    stmt.query_map([], |r| {
        let mut f = String::new();
        for i in 0..9 {
            f.push_str(&format!("{:?};", r.get::<_, rusqlite::types::Value>(i)?));
        }
        Ok(f)
    })
    .unwrap()
    .collect::<Result<_, _>>()
    .unwrap()
}

/// La suma de monto y costo adicional de todos los gastos, en centavos enteros (sumada en Rust, no en SQL).
fn total_en_centavos() -> i64 {
    let c = conexion();
    let mut stmt = c.prepare("SELECT monto, costo_adicional FROM gastos;").unwrap();
    let filas: Vec<(f64, f64)> =
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().collect::<Result<_, _>>().unwrap();
    filas.iter().map(|(m, c)| (m * 100.0).round() as i64 + (c * 100.0).round() as i64).sum()
}

fn gastos_de(categoria: i64) -> i64 {
    conexion().query_row("SELECT COUNT(*) FROM gastos WHERE categoria_id = ?;", [categoria], |r| r.get(0)).unwrap()
}

fn centavos(t: &str) -> i64 {
    crate::dominio::dinero::centavos_desde_texto(t).unwrap()
}

/// «Renta» (creada) y «Alquiler» (de fábrica) con gastos en septiembre y agosto de 2026. Devuelve la cuenta.
fn sembrar_renta_y_alquiler() -> i64 {
    let cuenta = cuenta();
    nueva("Renta");
    gasto("03/09/2026", "1347.29", "Renta", "Renta de septiembre", cuenta);
    gasto("03/08/2026", "1347.29", "Renta", "Renta de agosto", cuenta);
    gasto("21/07/2026", "1301.77", "Renta", "Renta de julio", cuenta);
    gasto("20/09/2026", "873.61", "Alquiler", "Salón para la fiesta", cuenta);
    gasto("11/09/2026", "412.83", "Alimentación", "Mercado", cuenta);
    cuenta
}

// --- Renombrar ---

#[test]
fn integracion_renombrar_cambia_el_nombre_y_los_gastos_siguen_donde_estaban() {
    let _g = entorno_para_pruebas_externas();
    sembrar_renta_y_alquiler();
    let renta = cat_id("Renta");
    let antes = gastos_sin_categoria();
    let total = total_en_centavos();

    let c = renombrar_categoria(renta, "  Vivienda  ".into()).unwrap();

    assert_eq!((c.id, c.nombre.as_str()), (renta, "Vivienda"));
    assert_eq!(c.motivo_sistema, None);
    assert!(!existe("Renta") && existe("Vivienda"));
    assert_eq!(cat_id("Vivienda"), renta, "el id no cambia");
    assert_eq!(gastos_de(renta), 3, "los gastos siguen en la misma categoría");
    assert_eq!(gastos_sin_categoria(), antes, "ninguna otra columna cambió");
    assert_eq!(total_en_centavos(), total);
    // El listado lo refleja.
    assert!(obtener_categorias().unwrap().iter().any(|c| c.id == renta && c.nombre == "Vivienda"));
}

#[test]
fn integracion_renombrar_ignora_mayusculas_y_tildes_aunque_el_unique_de_sqlite_no_lo_haga() {
    let _g = entorno_para_pruebas_externas();
    nueva("Mascotas");
    let aviso = nueva("Salud y bienestar");
    let antes = huella_de_la_base();

    // «Alimentación» ya existe de fábrica; el UNIQUE de SQLite dejaría pasar estas variantes, la regla no.
    for variante in ["alimentación", "ALIMENTACIÓN", "Alimentacion", " alimentacion "] {
        let e = renombrar_categoria(aviso, variante.into()).unwrap_err();
        assert_eq!(e, "La categoría ya existe.", "{variante}");
    }
    assert_eq!(huella_de_la_base(), antes, "un rechazo no escribe nada");

    // Su propio nombre con otra capitalización sí se puede.
    let c = renombrar_categoria(aviso, "SALUD Y BIENESTAR".into()).unwrap();
    assert_eq!(c.nombre, "SALUD Y BIENESTAR");
    assert!(existe("SALUD Y BIENESTAR") && !existe("Salud y bienestar"));
}

#[test]
fn integracion_renombrar_rechaza_vacio_largo_reservado_y_las_del_sistema() {
    let _g = entorno_para_pruebas_externas();
    let ajena = nueva("Mascotas");
    let antes = huella_de_la_base();

    assert_eq!(renombrar_categoria(ajena, "   ".into()).unwrap_err(), "El nombre de la categoría no puede estar vacío.");
    assert!(renombrar_categoria(ajena, "x".repeat(61)).unwrap_err().contains("60 caracteres"));
    assert!(renombrar_categoria(ajena, "Cargos derivados".into()).unwrap_err().contains("nombre reservado"));
    assert!(renombrar_categoria(ajena, "  CARGOS DERIVADOS ".into()).unwrap_err().contains("nombre reservado"));
    assert!(renombrar_categoria(999_999, "Nueva".into()).is_err(), "una inexistente");
    for sistema in ["Otros", "Suscripciones", "Impuestos"] {
        let e = renombrar_categoria(cat_id(sistema), "Otro nombre".into()).unwrap_err();
        assert!(e.contains("el sistema la usa para"), "{sistema}: {e}");
        assert!(e.contains(&format!("«{sistema}»")), "{e}");
    }

    assert_eq!(huella_de_la_base(), antes, "ningún rechazo escribe nada");
}

// --- Fusionar ---

#[test]
fn integracion_fusionar_mueve_todos_los_gastos_y_conserva_todo_lo_demas() {
    let _g = entorno_para_pruebas_externas();
    sembrar_renta_y_alquiler();
    let (renta, alquiler) = (cat_id("Renta"), cat_id("Alquiler"));
    let antes = gastos_sin_categoria();
    let total = total_en_centavos();
    let en_alquiler = gastos_de(alquiler);

    let movidos = fusionar_categorias(renta, alquiler).unwrap();

    assert_eq!(movidos, 3);
    assert!(!existe("Renta"), "el origen se eliminó");
    assert_eq!(gastos_de(alquiler), en_alquiler + 3);
    assert_eq!(gastos_de(renta), 0);
    assert_eq!(gastos_sin_categoria(), antes, "monto, fecha, descripción, método, costo y vínculos intactos");
    assert_eq!(total_en_centavos(), total, "el total no cambia");
    let huerfanos: i64 = conexion()
        .query_row("SELECT COUNT(*) FROM gastos g LEFT JOIN categorias c ON c.id = g.categoria_id WHERE c.id IS NULL;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(huerfanos, 0, "ningún gasto quedó apuntando a una categoría que no existe");
    assert_eq!(
        conexion().query_row("PRAGMA foreign_key_check;", [], |r| r.get::<_, String>(0)).ok(),
        None,
        "la base queda íntegra"
    );
}

#[test]
fn integracion_el_resumen_suma_las_dos_categorias_en_una_sola_fila() {
    let _g = entorno_para_pruebas_externas();
    sembrar_renta_y_alquiler();
    let hoy = NaiveDate::from_ymd_opt(2026, 10, 28).unwrap();
    let fila = |r: &crate::comandos::resumen::ResumenGastos, n: &str| {
        r.filas.iter().find(|f| f.categoria_nombre == n).map(|f| (centavos(&f.medidas.importe), f.medidas.n))
    };

    let antes = resumen_en(hoy, 2026, 9, 3, 60.0, vec![]).unwrap();
    let (renta_c, renta_n) = fila(&antes, "Renta").expect("septiembre tiene Renta");
    let (alq_c, alq_n) = fila(&antes, "Alquiler").expect("septiembre tiene Alquiler");
    let total_antes: i64 = antes.filas.iter().map(|f| centavos(&f.medidas.importe)).sum();

    fusionar_categorias(cat_id("Renta"), cat_id("Alquiler")).unwrap();

    let despues = resumen_en(hoy, 2026, 9, 3, 60.0, vec![]).unwrap();
    assert!(fila(&despues, "Renta").is_none(), "la fila de la categoría fusionada desaparece");
    assert_eq!(fila(&despues, "Alquiler"), Some((renta_c + alq_c, renta_n + alq_n)), "las dos se suman en una fila");
    let total_despues: i64 = despues.filas.iter().map(|f| centavos(&f.medidas.importe)).sum();
    assert_eq!(total_despues, total_antes, "el total del mes no cambia");
}

#[test]
fn integracion_la_fusion_deja_su_caso_en_obtener_correcciones() {
    let _g = entorno_para_pruebas_externas();
    sembrar_renta_y_alquiler();
    assert!(obtener_correcciones().unwrap().is_empty());

    fusionar_categorias(cat_id("Renta"), cat_id("Alquiler")).unwrap();

    let casos = obtener_correcciones().unwrap();
    assert_eq!(casos.len(), 1);
    let c = &casos[0];
    assert_eq!(c.tipo, "categoria");
    assert_eq!(c.descripcion, "Fusión de «Renta» en «Alquiler» (3 gastos)");
    assert_eq!((c.importe, c.divisa.as_deref(), c.detalle.as_deref()), (None, None, None));
    assert_eq!(c.referencia_id, cat_id("Alquiler"));
    assert!(c.numero_caso.starts_with("COR-"), "{}", c.numero_caso);
    assert!(crate::dominio::correccion::motivo_de_correccion(&c.motivo).is_ok());
}

#[test]
fn integracion_la_vista_previa_informa_y_no_escribe_nada() {
    let _g = entorno_para_pruebas_externas();
    sembrar_renta_y_alquiler();
    nueva("Vacía");
    let (renta, alquiler, vacia) = (cat_id("Renta"), cat_id("Alquiler"), cat_id("Vacía"));
    let antes = huella_de_la_base();

    let v = previsualizar_fusion_categorias(renta, alquiler).unwrap();
    assert_eq!((v.origen_nombre.as_str(), v.destino_nombre.as_str(), v.gastos), ("Renta", "Alquiler", 3));
    assert_eq!((v.desde.as_deref(), v.hasta.as_deref()), (Some("21/07/2026"), Some("03/09/2026")));
    assert!(v.advertencias.is_empty());

    let v = previsualizar_fusion_categorias(vacia, alquiler).unwrap();
    assert_eq!((v.gastos, v.desde, v.hasta), (0, None, None));
    assert_eq!(v.advertencias, vec!["«Vacía» no tiene gastos: solo se eliminará la categoría.".to_string()]);

    assert!(previsualizar_fusion_categorias(renta, renta).is_err());
    assert!(previsualizar_fusion_categorias(cat_id("Otros"), renta).unwrap_err().contains("el sistema la usa para"));
    assert!(previsualizar_fusion_categorias(999_999, renta).is_err());

    assert_eq!(huella_de_la_base(), antes, "la vista previa no escribe ni un byte");
}

#[test]
fn integracion_una_fusion_que_falla_a_medias_no_deja_nada_a_medias() {
    let _g = entorno_para_pruebas_externas();
    sembrar_renta_y_alquiler();
    let (renta, alquiler) = (cat_id("Renta"), cat_id("Alquiler"));
    // Fallo inyectado en el último paso (borrar el origen), cuando el caso ya se anotó y los gastos ya se movieron.
    conexion()
        .execute_batch("CREATE TRIGGER inyectar_fallo BEFORE DELETE ON categorias BEGIN SELECT RAISE(ABORT, 'fallo inyectado'); END;")
        .unwrap();
    let con_el_disparador = huella_de_la_base();

    let e = fusionar_categorias(renta, alquiler).unwrap_err();

    assert!(e.contains("fallo inyectado"), "{e}");
    assert_eq!(huella_de_la_base(), con_el_disparador, "ni gastos movidos, ni caso anotado, ni categoría borrada");
    assert_eq!(gastos_de(renta), 3);
    assert!(obtener_correcciones().unwrap().is_empty());
}

#[test]
fn integracion_las_del_sistema_no_se_fusionan_como_origen_pero_si_reciben() {
    let _g = entorno_para_pruebas_externas();
    sembrar_renta_y_alquiler();
    let renta = cat_id("Renta");
    let antes = huella_de_la_base();

    for sistema in ["Otros", "Suscripciones", "Impuestos"] {
        let e = fusionar_categorias(cat_id(sistema), renta).unwrap_err();
        assert!(e.contains("el sistema la usa para"), "{sistema}: {e}");
    }
    assert_eq!(huella_de_la_base(), antes);

    // Como destino, «Otros» recibe los gastos de «Renta».
    let otros = cat_id("Otros");
    let en_otros = gastos_de(otros);
    assert_eq!(fusionar_categorias(renta, otros).unwrap(), 3);
    assert_eq!(gastos_de(otros), en_otros + 3);
    assert!(existe("Otros") && !existe("Renta"));
}

// --- La marca «fija / recurrente» (categorías fijas, esquema 20) ---

fn es_fija(nombre: &str) -> bool {
    obtener_categorias().unwrap().into_iter().find(|c| c.nombre == nombre).unwrap().fija
}

#[test]
fn integracion_renombrar_no_toca_la_marca_fija() {
    let _g = entorno_para_pruebas_externas();
    sembrar_renta_y_alquiler();
    let renta = cat_id("Renta");
    marcar_categoria_fija(renta, true).unwrap();

    let c = renombrar_categoria(renta, "Vivienda".into()).unwrap();

    assert!(c.fija, "el resultado trae la marca");
    assert!(es_fija("Vivienda"), "la marca sobrevive al cambio de nombre");
    assert!(!es_fija("Alquiler"));
}

#[test]
fn integracion_la_fusion_conserva_la_marca_del_destino_y_la_vista_previa_avisa_si_difieren() {
    for (origen_fija, destino_fija) in [(true, false), (false, true), (true, true), (false, false)] {
        let _g = entorno_para_pruebas_externas();
        sembrar_renta_y_alquiler();
        let (renta, alquiler) = (cat_id("Renta"), cat_id("Alquiler"));
        marcar_categoria_fija(renta, origen_fija).unwrap();
        marcar_categoria_fija(alquiler, destino_fija).unwrap();
        let antes = huella_de_la_base();

        let v = previsualizar_fusion_categorias(renta, alquiler).unwrap();
        assert_eq!((v.origen_fija, v.destino_fija), (origen_fija, destino_fija));
        assert_eq!(v.advertencias.len(), usize::from(origen_fija != destino_fija), "{origen_fija}/{destino_fija}: {:?}", v.advertencias);
        if origen_fija != destino_fija {
            assert!(v.advertencias[0].contains("tomarán la marca de «Alquiler»"), "{:?}", v.advertencias);
        }
        assert_eq!(huella_de_la_base(), antes, "la vista previa no escribe");

        fusionar_categorias(renta, alquiler).unwrap();
        assert_eq!(es_fija("Alquiler"), destino_fija, "origen {origen_fija}, destino {destino_fija}: manda el destino");
        assert!(!existe("Renta"));
    }
}

// --- Contrato, respaldo y operaciones del sistema ---

/// Las claves que declara `export type <nombre> = { … }` en `tipos-ipc.d.ts`, ordenadas.
fn claves_declaradas(nombre: &str) -> Vec<String> {
    let cabecera = format!("export type {nombre} = {{");
    let inicio = TIPOS_IPC.find(&cabecera).unwrap_or_else(|| panic!("tipos-ipc.d.ts no declara {nombre}")) + cabecera.len();
    let cuerpo = &TIPOS_IPC[inicio..inicio + TIPOS_IPC[inicio..].find("};").unwrap()];
    let mut claves: Vec<String> =
        cuerpo.split(';').filter_map(|c| c.split(':').next()).map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect();
    claves.sort();
    claves
}

fn claves_reales(v: &serde_json::Value) -> Vec<String> {
    let mut claves: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
    claves.sort();
    claves
}

#[test]
fn integracion_contrato_de_forma_de_los_dto_con_tipos_ipc() {
    let _g = entorno_para_pruebas_externas();
    sembrar_renta_y_alquiler();

    let c = serde_json::to_value(renombrar_categoria(cat_id("Renta"), "Vivienda".into()).unwrap()).unwrap();
    assert_eq!(claves_reales(&c), claves_declaradas("Categoria"));
    assert!(c["motivo_sistema"].is_null());
    let otros = obtener_categorias().unwrap().into_iter().find(|c| c.nombre == "Otros").unwrap();
    assert!(serde_json::to_value(&otros).unwrap()["motivo_sistema"].is_string(), "las del sistema llevan su motivo");

    let v = serde_json::to_value(previsualizar_fusion_categorias(cat_id("Vivienda"), cat_id("Alquiler")).unwrap()).unwrap();
    assert_eq!(claves_reales(&v), claves_declaradas("VistaPreviaFusion"));
    assert!(v["gastos"].is_i64() && v["desde"].is_string() && v["advertencias"].is_array());

    for firma in [
        "renombrar_categoria: { args: { id: number; nombre: string }; ret: Categoria }",
        "previsualizar_fusion_categorias: { args: { origenId: number; destinoId: number }; ret: VistaPreviaFusion }",
        "fusionar_categorias: { args: { origenId: number; destinoId: number }; ret: number }",
    ] {
        assert!(TIPOS_IPC.contains(firma), "tipos-ipc.d.ts debe declarar: {firma}");
    }
}

#[test]
fn integracion_respaldo_y_restauracion_conservan_la_fusion_y_la_deshacen() {
    let _g = entorno_para_pruebas_externas();
    sembrar_renta_y_alquiler();
    fusionar_categorias(cat_id("Renta"), cat_id("Alquiler")).unwrap();
    let con_la_fusion = huella_de_la_base();
    let nombre = crate::comandos::respaldos::crear_respaldo().unwrap();
    let nombre = std::path::Path::new(&nombre).file_name().unwrap().to_string_lossy().to_string();

    // Después del respaldo: otra categoría se renombra y se fusiona.
    nueva("Mascotas");
    renombrar_categoria(cat_id("Mascotas"), "Animales".into()).unwrap();
    fusionar_categorias(cat_id("Animales"), cat_id("Alquiler")).unwrap();
    assert_eq!(obtener_correcciones().unwrap().len(), 2);

    let r = crate::comandos::respaldos::restaurar_respaldo(nombre).unwrap();

    assert_eq!(r["version_del_esquema"], crate::migraciones::VERSION_OBJETIVO);
    assert_eq!(huella_de_la_base(), con_la_fusion, "la base volvió exactamente al estado del respaldo");
    assert!(!existe("Renta") && !existe("Animales"));
    let casos = obtener_correcciones().unwrap();
    assert_eq!(casos.len(), 1);
    assert_eq!(casos[0].descripcion, "Fusión de «Renta» en «Alquiler» (3 gastos)");
}

#[test]
fn integracion_las_categorias_del_sistema_siguen_funcionando_tras_renombrar_y_fusionar_otras() {
    let _g = entorno_para_pruebas_externas();
    let cuenta = cuenta();
    let tarjeta = tarjeta();
    nueva("Mascotas");
    nueva("Veterinaria");
    gasto("04/09/2026", "311.47", "Mascotas", "Alimento", cuenta);
    renombrar_categoria(cat_id("Mascotas"), "Animales".into()).unwrap();
    fusionar_categorias(cat_id("Animales"), cat_id("Veterinaria")).unwrap();
    eliminar_categoria(cat_id("Veterinaria")).unwrap_err(); // tiene un gasto: sigue protegida por su guarda propia

    // Un cargo recurrente automático cae en «Suscripciones».
    crate::comandos::suscripciones::crear_suscripcion(
        "Plataforma de prueba".into(),
        ImporteDecimal::desde_texto("523.17").unwrap(),
        tarjeta,
        "mensual".into(),
        15,
        "DOP".into(),
        Some("15/10/2026".into()),
    )
    .unwrap();
    let reloj = crate::puertos::reloj::RelojFijo::en(2026, 10, 15);
    let cobro = crate::comandos::suscripciones::procesar_suscripciones_con(&reloj).unwrap();
    assert!(cobro.fallos.is_empty(), "{:?}", cobro.fallos);
    assert_eq!(cobro.cargos.len(), 1);
    let (categoria, monto): (String, f64) = conexion()
        .query_row(
            "SELECT c.nombre, g.monto FROM gastos g JOIN categorias c ON c.id = g.categoria_id
             WHERE g.descripcion LIKE '%Plataforma de prueba%' ORDER BY g.id DESC LIMIT 1;",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((categoria.as_str(), monto), ("Suscripciones", 523.17));

    // La comisión de un abono cae en «Otros».
    crate::comandos::abonos::registrar_pago_tarjeta(tarjeta, "16/10/2026".into(), ImporteDecimal::desde_texto("211.37").unwrap(), "DOP".into(), Some(cuenta), 0.0).unwrap();
    let comision: Option<String> = conexion()
        .query_row(
            "SELECT c.nombre FROM pagos_tarjeta p JOIN gastos g ON g.id = p.gasto_comision_id
             JOIN categorias c ON c.id = g.categoria_id ORDER BY p.id DESC LIMIT 1;",
            [],
            |r| r.get(0),
        )
        .ok();
    assert_eq!(comision.as_deref(), Some("Otros"), "la comisión del abono sigue asentándose en «Otros»");
}
