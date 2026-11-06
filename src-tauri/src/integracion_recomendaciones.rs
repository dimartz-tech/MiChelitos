//! Pruebas de integración de las recomendaciones del historial del patrimonio (`obtener_recomendaciones`).
//!
//! Ejercitan el **comando real** de punta a punta sobre una base SQLite temporal con el esquema real (el entorno aislado
//! de `caracterizacion`): se siembran categorías, gastos de seis meses, cuentas, una tarjeta, un avance de efectivo (que
//! crea un cargo derivado), presupuestos y dos fotos mensuales con los comandos reales (o SQL, donde el dato no
//! importa), se llama al comando de lectura con «hoy» inyectado y se comprueba el DTO tal como viaja (importes en texto
//! exacto, orden, severidades, campos). Todos los datos son inventados, con céntimos poco redondos.
//!
//! Se reconocen por el prefijo `integracion_`: `cargo test integracion_`.

use crate::caracterizacion::entorno_para_pruebas_externas;
use crate::comandos::catalogos::{crear_categoria, marcar_categoria_fija};
use crate::comandos::fotos_patrimonio::asegurar_en;
use crate::comandos::presupuestos::fijar_presupuesto;
use crate::comandos::recomendaciones::{recomendaciones_en, RecomendacionPatrimonio, ResultadoRecomendaciones};
use crate::integracion_resumen::{conexion, huella_de_la_base, hoy, importe, verificar_forma};
use crate::{db_nosql, db_sql};
use chrono::NaiveDate;
use rusqlite::params;
use serde_json::{json, Value};
use std::sync::MutexGuard;

const TIPOS_IPC: &str = include_str!("../../src/js/tipos-ipc.d.ts");

fn entorno() -> MutexGuard<'static, ()> {
    let guarda = entorno_para_pruebas_externas();
    let ruta = db_nosql::obtener_ruta_nosql("capital");
    assert!(ruta.starts_with(std::env::temp_dir().to_str().unwrap()), "ABORTADO: apuntaría al capital real ({ruta})");
    let _ = std::fs::remove_file(&ruta);
    guarda
}

fn dia(a: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(a, m, d).unwrap()
}

fn capital() -> Result<Value, String> {
    Ok(json!({
        "certificados": [{"banco": "Banco Uno", "monto": 25000.37, "tasa": 9.5, "vencimiento": "01/01/2031"}],
        "bolsa": [],
        "propiedades": {"inmobiliario": [], "vehiculos": [], "maquinaria": []}
    }))
}

fn categoria(nombre: &str) -> i64 {
    crear_categoria(nombre.into()).unwrap().id
}

fn gasto(fecha: &str, monto: &str, divisa: &str, categoria_id: i64) {
    conexion()
        .execute(
            "INSERT INTO gastos (fecha, monto, divisa, descripcion, categoria_id, metodo_pago, costo_adicional)
             VALUES (?, ?, ?, 'Prueba de integración', ?, 'efectivo', 0.0);",
            params![fecha, monto.parse::<f64>().unwrap(), divisa, categoria_id],
        )
        .unwrap();
}

/// El mismo importe en cada uno de los meses indicados (día 9).
fn gastos_mensuales(categoria_id: i64, meses: &[(u32, &str)]) {
    for (mes, monto) in meses {
        gasto(&format!("09/{mes:02}/2026"), monto, "DOP", categoria_id);
    }
}

fn cuenta(nombre: &str, divisa: &str, saldo: f64) -> i64 {
    let c = conexion();
    c.execute("INSERT INTO cuentas_ahorro (nombre, divisa, balance_actual) VALUES (?, ?, ?);", params![nombre, divisa, saldo]).unwrap();
    c.last_insert_rowid()
}

fn tarjeta(pesos: f64, dolares: f64) -> i64 {
    let c = conexion();
    c.execute(
        "INSERT INTO tarjetas (entidad, nombre_tarjeta, fecha_corte, fecha_limite_pago, balance_pesos, balance_dolares,
                               limite_pesos, limite_dolares, politica_liquidacion)
         VALUES ('Banco Ejemplo', 'Tarjeta Ejemplo', 15, 5, ?, ?, 900000.0, 9000.0, 'traduce');",
        params![pesos, dolares],
    )
    .unwrap();
    c.last_insert_rowid()
}

fn en_pesos(c: &crate::comandos::recomendaciones::CifraRecomendacion) -> &str {
    assert_eq!(c.tipo, "importe", "{}", c.etiqueta);
    &c.valor
}

fn cifra<'a>(r: &'a RecomendacionPatrimonio, etiqueta: &str) -> &'a crate::comandos::recomendaciones::CifraRecomendacion {
    r.cifras.iter().find(|c| c.etiqueta == etiqueta).unwrap_or_else(|| panic!("«{}» no tiene la cifra «{etiqueta}»", r.id))
}

fn por_id<'a>(r: &'a ResultadoRecomendaciones, id: &str) -> &'a RecomendacionPatrimonio {
    r.recomendaciones.iter().find(|x| x.id == id).unwrap_or_else(|| {
        panic!("no hay la recomendación «{id}»; hay {:?}", r.recomendaciones.iter().map(|x| x.id.as_str()).collect::<Vec<_>>())
    })
}

struct Escenario {
    salidas: i64,
    transporte: i64,
}

/// Seis meses de gasto, cuentas, tarjeta, un avance de efectivo en octubre, dos fotos (septiembre y octubre) y, en
/// noviembre, gastos con presupuestos. «Hoy» es el 6 de noviembre de 2026.
///
/// * «Salidas inventadas» (variable): 1000.10 de mayo a julio y 1500.14, 1700.16 y 1900.18 de agosto a octubre.
/// * «Transporte inventado» (variable, plano): 800.07 cada mes.  * «Alquiler inventado» (fija): 6000.31 cada mes.
/// * Octubre: el avance de 9000.13 al 6.25 % deja un cargo derivado de 562.51; y un gasto de USD 45.17.
fn sembrar_escenario() -> Escenario {
    let salidas = categoria("Salidas inventadas");
    let transporte = categoria("Transporte inventado");
    let alquiler = categoria("Alquiler inventado");
    marcar_categoria_fija(alquiler, true).unwrap();

    gastos_mensuales(salidas, &[(5, "1000.10"), (6, "1000.10"), (7, "1000.10"), (8, "1500.14"), (9, "1700.16"), (10, "1900.18")]);
    for mes in 5..=10 {
        gastos_mensuales(transporte, &[(mes, "800.07")]);
        gastos_mensuales(alquiler, &[(mes, "6000.31")]);
    }
    gasto("20/10/2026", "45.17", "USD", salidas);

    let ahorro = cuenta("Ahorro inventado", "DOP", 7500.37);
    cuenta("Dólares inventados", "USD", 120.0);
    let t = tarjeta(3210.45, 25.5);

    // Septiembre: la primera foto, antes del avance.
    asegurar_en(None, dia(2026, 9, 15), &capital()).unwrap();

    // Octubre: el avance (la cuenta recibe 9000.13; la tarjeta debe 9000.13 + 562.51) y la segunda foto.
    crate::comandos::avances::registrar_avance_efectivo(
        t,
        ahorro,
        "12/10/2026".into(),
        importe("9000.13"),
        "DOP".into(),
        "porcentaje".into(),
        Some(6.25),
        None,
        None,
    )
    .unwrap();
    asegurar_en(None, dia(2026, 10, 20), &capital()).unwrap();

    // Noviembre (mes en curso): transporte pasa de su presupuesto, salidas queda cerca; y dólares aparte.
    gasto("03/11/2026", "700.03", "DOP", transporte);
    gasto("04/11/2026", "900.19", "DOP", salidas);
    gasto("05/11/2026", "12.34", "USD", salidas);
    fijar_presupuesto(transporte, importe("650.04")).unwrap();
    fijar_presupuesto(salidas, importe("1000.21")).unwrap();
    Escenario { salidas, transporte }
}

fn hoy_de_prueba() -> NaiveDate {
    hoy(2026, 11, 6)
}

#[test]
fn integracion_sin_datos_dice_que_faltan_y_no_inventa_recomendaciones() {
    let _g = entorno();
    let r = recomendaciones_en(hoy_de_prueba()).unwrap();
    assert!(r.recomendaciones.is_empty());
    assert_eq!((r.omitidas, r.reglas_evaluadas), (0, 0));
    assert_eq!(r.sin_datos.len(), 6, "las seis reglas dicen por qué no tienen datos");
    assert!(r.sin_datos.iter().all(|s| !s.motivo.trim().is_empty()));
    assert!(r.usd_excluido.is_empty());
    assert_eq!((r.anio_gastos, r.mes_gastos, r.anio_presupuestos, r.mes_presupuestos), (2026, 10, 2026, 11));
    assert!(r.aviso.contains("no asesoría financiera"));
}

#[test]
fn integracion_el_escenario_completo_activa_las_seis_reglas_en_orden() {
    let _g = entorno();
    let e = sembrar_escenario();
    let r = recomendaciones_en(hoy_de_prueba()).unwrap();

    assert_eq!(r.reglas_evaluadas, 6);
    assert!(r.sin_datos.is_empty(), "{:?}", r.sin_datos);
    let ids: Vec<&str> = r.recomendaciones.iter().map(|x| x.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            format!("presupuesto_excedido:{}", e.transporte),
            format!("tendencia_gasto:{}", e.salidas),
            "variacion_patrimonio:2026-10".to_string(),
            "deuda_y_ahorro".to_string(),
            format!("presupuesto_cerca:{}", e.salidas),
            "cargos_derivados:2026-10".to_string(),
            "gasto_fijo_predomina:2026-10".to_string(),
        ],
        "atención antes que informativa; dentro de cada una, por regla"
    );
    let severidades: Vec<&str> = r.recomendaciones.iter().map(|x| x.severidad.as_str()).collect();
    assert_eq!(severidades, vec!["atencion", "atencion", "atencion", "atencion", "info", "info", "info"]);
    assert_eq!(r.omitidas, 0);
    assert_eq!(r.maximo_por_severidad, 5);
}

#[test]
fn integracion_las_cifras_de_cada_regla_son_las_de_los_datos_sembrados() {
    let _g = entorno();
    let e = sembrar_escenario();
    let r = recomendaciones_en(hoy_de_prueba()).unwrap();

    // Presupuestos (noviembre, solo pesos): 700.03 de 650.04 = 107.6 %; 900.19 de 1000.21 = 90.0 %.
    let excedido = por_id(&r, &format!("presupuesto_excedido:{}", e.transporte));
    assert_eq!(excedido.destino, "gastos");
    assert_eq!((en_pesos(cifra(excedido, "Gastado en pesos")), en_pesos(cifra(excedido, "Exceso"))), ("700.03", "49.99"));
    assert_eq!((cifra(excedido, "Avance").tipo.as_str(), cifra(excedido, "Avance").valor.as_str()), ("porcentaje", "107.6"));
    let cerca = por_id(&r, &format!("presupuesto_cerca:{}", e.salidas));
    assert_eq!((en_pesos(cifra(cerca, "Gastado en pesos")), en_pesos(cifra(cerca, "Restante"))), ("900.19", "100.02"));
    assert_eq!(cifra(cerca, "Avance").valor, "90.0");
    let usd = cifra(cerca, "Dólares aparte (no cuentan)");
    assert_eq!((usd.tipo.as_str(), usd.valor.as_str()), ("importe_usd", "12.34"), "los dólares no se convierten ni cuentan");

    // Tendencia (a octubre): reciente (1500.14 + 1700.16 + 1900.18) / 3 = 1700.16 contra 1000.10: +700.06 (70.0 %).
    let sube = por_id(&r, &format!("tendencia_gasto:{}", e.salidas));
    assert_eq!(en_pesos(cifra(sube, "Promedio de los 3 últimos meses")), "1700.16");
    assert_eq!(en_pesos(cifra(sube, "Promedio de los 3 meses anteriores")), "1000.10");
    assert_eq!(en_pesos(cifra(sube, "Diferencia")), "700.06");
    assert_eq!(cifra(sube, "Cambio").valor, "70.0");
    assert_eq!(cifra(sube, "Último mes completo").valor, "10/2026");
    assert!(sube.regla_texto.contains("10 %") && sube.regla_texto.contains("500.00"));
    // El transporte plano y el alquiler fijo no disparan la tendencia.
    assert!(!r.recomendaciones.iter().any(|x| x.id.starts_with("tendencia_gasto") && x.id != sube.id));

    // Variación del patrimonio: el avance mueve +9000.13 en activos y +9562.64 en pasivos: el patrimonio baja 562.51 y lo
    // explican los pasivos.
    let v = por_id(&r, "variacion_patrimonio:2026-10");
    assert_eq!(v.destino, "prestamos");
    assert_eq!(en_pesos(cifra(v, "Cambio del patrimonio")), "-562.51");
    assert_eq!(en_pesos(cifra(v, "Cambio en activos")), "9000.13");
    assert_eq!(en_pesos(cifra(v, "Cambio en pasivos")), "9562.64");
    assert_eq!(cifra(v, "Calidad de las fotos").valor, "completa / completa");
    assert!(v.titulo.contains("bajó") && v.titulo.contains("pasivos"), "{}", v.titulo);

    // Deuda y ahorro: 3210.45 + 9562.64 de deuda frente a 7500.37 + 9000.13 en la cuenta de ahorro; sin costos inventados.
    let d = por_id(&r, "deuda_y_ahorro");
    assert_eq!(en_pesos(cifra(d, "Deuda de tarjetas en pesos")), "12773.09");
    assert_eq!(en_pesos(cifra(d, "Saldo en cuentas de ahorro en pesos")), "16500.50");
    assert!(d.explicacion.contains("no guarda la tasa"));

    // Cargos derivados de octubre: 562.51 de 9263.07 = 6.1 %.
    let c = por_id(&r, "cargos_derivados:2026-10");
    assert_eq!((en_pesos(cifra(c, "Cargos derivados")), en_pesos(cifra(c, "Gasto total en pesos")), cifra(c, "Peso").valor.as_str()), ("562.51", "9263.07", "6.1"));
    assert_eq!(c.severidad, "info");

    // Fijo y variable: 6000.31 fijo (64.8 %) y 3262.76 variable.
    let f = por_id(&r, "gasto_fijo_predomina:2026-10");
    assert_eq!((en_pesos(cifra(f, "Gasto fijo")), en_pesos(cifra(f, "Gasto variable"))), ("6000.31", "3262.76"));
    assert_eq!(cifra(f, "Peso fijo").valor, "64.8");

    // Los dólares se informan aparte, sin convertir: el gasto de octubre (el de noviembre es del mes en curso), la deuda de
    // la tarjeta y la cuenta en dólares.
    let usd: Vec<(&str, &str)> = r.usd_excluido.iter().map(|u| (u.origen.as_str(), u.importe.as_str())).collect();
    assert_eq!(usd, vec![("gastos", "45.17"), ("tarjetas", "25.50"), ("cuentas", "120.00")]);
}

#[test]
fn integracion_es_de_solo_lectura_y_determinista() {
    let _g = entorno();
    sembrar_escenario();
    let antes = huella_de_la_base();
    let a = serde_json::to_value(recomendaciones_en(hoy_de_prueba()).unwrap()).unwrap();
    let b = serde_json::to_value(recomendaciones_en(hoy_de_prueba()).unwrap()).unwrap();
    assert_eq!(a, b, "mismos datos y misma fecha, mismo resultado");
    assert_eq!(huella_de_la_base(), antes, "no escribió nada");
    // Y tampoco deja una transacción de escritura abierta: otra conexión puede escribir de inmediato.
    let c = db_sql::obtener_conexion().unwrap();
    c.execute("INSERT INTO categorias (nombre) VALUES ('Otra inventada');", []).unwrap();
}

#[test]
fn integracion_un_mes_posterior_cambia_el_periodo_y_enero_mira_diciembre() {
    let _g = entorno();
    sembrar_escenario();
    // En diciembre el mes de referencia es noviembre (completo): los presupuestos se leen en diciembre (sin gasto).
    let r = recomendaciones_en(hoy(2026, 12, 3)).unwrap();
    assert_eq!((r.anio_gastos, r.mes_gastos, r.anio_presupuestos, r.mes_presupuestos), (2026, 11, 2026, 12));
    assert!(!r.recomendaciones.iter().any(|x| x.id.starts_with("presupuesto_")), "diciembre aún no gasta nada");
    let r = recomendaciones_en(hoy(2027, 1, 9)).unwrap();
    assert_eq!((r.anio_gastos, r.mes_gastos), (2026, 12));
}

#[test]
fn integracion_se_muestran_a_lo_sumo_cinco_por_severidad_y_se_cuentan_las_omitidas() {
    let _g = entorno();
    for i in 1..=7 {
        let id = categoria(&format!("Categoría inventada {i}"));
        gasto("02/11/2026", &format!("{}.{:02}", 100 + i, 10 + i), "DOP", id);
        fijar_presupuesto(id, importe("50.03")).unwrap();
    }
    let r = recomendaciones_en(hoy_de_prueba()).unwrap();
    assert_eq!(r.recomendaciones.len(), 5);
    assert_eq!(r.omitidas, 2);
    assert!(r.recomendaciones.iter().all(|x| x.severidad == "atencion" && x.regla == "presupuesto"));
    // Los de mayor avance primero: el gasto más alto es la categoría 7.
    assert!(r.recomendaciones[0].titulo.contains("Categoría inventada 7"), "{}", r.recomendaciones[0].titulo);
}

#[test]
fn integracion_los_destinos_son_rutas_de_la_interfaz() {
    let _g = entorno();
    sembrar_escenario();
    let r = recomendaciones_en(hoy_de_prueba()).unwrap();
    let rutas = ["dashboard", "ingresos", "gastos", "tarjetas", "cuentas", "efectivo", "suscripciones", "capital", "prestamos", "resumen", "evolucion", "ajustes"];
    for x in &r.recomendaciones {
        assert!(rutas.contains(&x.destino.as_str()), "{}: destino «{}» no es una pestaña", x.id, x.destino);
    }
}

#[test]
fn integracion_el_dto_serializado_tiene_exactamente_las_claves_de_tipos_ipc() {
    let _g = entorno();
    sembrar_escenario();
    let r = serde_json::to_value(recomendaciones_en(hoy_de_prueba()).unwrap()).unwrap();
    verificar_forma(TIPOS_IPC, "ResultadoRecomendaciones", &r, "ResultadoRecomendaciones");
    let lista = r["recomendaciones"].as_array().unwrap();
    assert!(!lista.is_empty());
    for (i, x) in lista.iter().enumerate() {
        verificar_forma(TIPOS_IPC, "RecomendacionPatrimonio", x, &format!("recomendaciones[{i}]"));
        for (j, c) in x["cifras"].as_array().unwrap().iter().enumerate() {
            verificar_forma(TIPOS_IPC, "CifraRecomendacion", c, &format!("recomendaciones[{i}].cifras[{j}]"));
        }
    }
    for (i, u) in r["usd_excluido"].as_array().unwrap().iter().enumerate() {
        verificar_forma(TIPOS_IPC, "DolaresExcluidos", u, &format!("usd_excluido[{i}]"));
    }
    // Las reglas sin datos también tienen su forma (se prueban con una base vacía en otro escenario).
    let vacia_firma = "obtener_recomendaciones: { args: {  }; ret: ResultadoRecomendaciones }";
    assert!(TIPOS_IPC.contains(vacia_firma), "tipos-ipc.d.ts debe declarar: {vacia_firma}");
}

#[test]
fn integracion_la_forma_de_las_reglas_sin_datos_coincide_con_tipos_ipc() {
    let _g = entorno();
    let r = serde_json::to_value(recomendaciones_en(hoy_de_prueba()).unwrap()).unwrap();
    verificar_forma(TIPOS_IPC, "ResultadoRecomendaciones", &r, "ResultadoRecomendaciones");
    for (i, s) in r["sin_datos"].as_array().unwrap().iter().enumerate() {
        verificar_forma(TIPOS_IPC, "ReglaSinDatosPatrimonio", s, &format!("sin_datos[{i}]"));
    }
}

#[test]
fn integracion_todo_importe_del_dto_es_texto_exacto_y_los_porcentajes_llevan_un_decimal() {
    let _g = entorno();
    sembrar_escenario();
    let r = recomendaciones_en(hoy_de_prueba()).unwrap();
    let importe_exacto = |t: &str| {
        let t = t.strip_prefix('-').unwrap_or(t);
        let (e, d) = t.split_once('.').unwrap_or_else(|| panic!("«{t}» no es un importe exacto"));
        !e.is_empty() && e.chars().all(|c| c.is_ascii_digit()) && d.len() == 2 && d.chars().all(|c| c.is_ascii_digit())
    };
    let porcentaje_exacto = |t: &str| {
        let t = t.strip_prefix('-').unwrap_or(t);
        let (e, d) = t.split_once('.').unwrap_or_else(|| panic!("«{t}» no es un porcentaje"));
        !e.is_empty() && e.chars().all(|c| c.is_ascii_digit()) && d.len() == 1 && d.chars().all(|c| c.is_ascii_digit())
    };
    let mut importes = 0;
    for x in &r.recomendaciones {
        for c in &x.cifras {
            match c.tipo.as_str() {
                "importe" | "importe_usd" => {
                    importes += 1;
                    assert!(importe_exacto(&c.valor), "{}: «{}» = «{}»", x.id, c.etiqueta, c.valor);
                }
                "porcentaje" => assert!(porcentaje_exacto(&c.valor), "{}: «{}» = «{}»", x.id, c.etiqueta, c.valor),
                "tasa" | "texto" => {}
                otro => panic!("tipo de cifra desconocido: {otro}"),
            }
        }
    }
    assert!(importes >= 20, "el escenario debe ejercitar muchos importes ({importes})");
}
