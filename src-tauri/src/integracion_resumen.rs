//! Pruebas de integración del resumen de gastos por categoría y mes (`obtener_resumen_gastos`).
//!
//! Ejercitan el **comando real** de punta a punta sobre una base SQLite temporal con el esquema real (el mismo
//! entorno aislado de `caracterizacion`): los datos se siembran con los comandos reales (gastos, abonos con su
//! comisión, avances con su cargo, liquidación de un consumo en dólares), se llama al comando de lectura y se
//! comprueba el DTO tal como viaja (importes en texto exacto, orden, campos). Todos los datos son inventados.
//!
//! Se reconocen por el prefijo `integracion_`: `cargo test integracion_`.

use crate::caracterizacion::entorno_para_pruebas_externas;
use crate::comandos::gastos::{crear_gasto, liquidar_consumo_pendiente, GastoInput};
use crate::comandos::resumen::{obtener_resumen_gastos, resumen_en, ResumenGastos};
use crate::db_sql;
use crate::ipc::ImporteDecimal;
use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde_json::Value;

// --- Ayudantes ---

fn conexion() -> Connection {
    db_sql::obtener_conexion().expect("abrir conexión de prueba")
}

fn importe(texto: &str) -> ImporteDecimal {
    ImporteDecimal::desde_texto(texto).expect("importe de prueba")
}

fn categoria_por_nombre(nombre: &str) -> i64 {
    conexion().query_row("SELECT id FROM categorias WHERE nombre = ?;", [nombre], |r| r.get(0)).unwrap()
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

fn crear_tarjeta(politica: &str) -> i64 {
    let c = conexion();
    c.execute(
        "INSERT INTO tarjetas (entidad, nombre_tarjeta, fecha_corte, fecha_limite_pago, balance_pesos, balance_dolares,
                               limite_pesos, limite_dolares, politica_liquidacion)
         VALUES ('Banco Ejemplo', 'Tarjeta Ejemplo', 15, 5, 0.0, 0.0, 900000.0, 9000.0, ?);",
        [politica],
    )
    .unwrap();
    c.last_insert_rowid()
}

/// Un gasto por transferencia desde una cuenta (la retención del 0.20 % queda como costo adicional).
fn transferencia(fecha: &str, monto: &str, categoria: &str, cuenta: i64) {
    crear_gasto(GastoInput {
        fecha: fecha.into(),
        monto: importe(monto),
        divisa: "DOP".into(),
        descripcion: "Prueba de integración".into(),
        categoria_id: categoria_por_nombre(categoria),
        metodo_pago: "transferencia".into(),
        es_lbtr: false,
        tarjeta_id: None,
        cuenta_ahorro_id: Some(cuenta),
        tasa_cambio: None,
    })
    .unwrap();
}

fn compra_en_dolares(fecha: &str, monto: &str, tarjeta: i64) -> i64 {
    crear_gasto(GastoInput {
        fecha: fecha.into(),
        monto: importe(monto),
        divisa: "USD".into(),
        descripcion: "Compra en el exterior".into(),
        categoria_id: categoria_por_nombre("Otros"),
        metodo_pago: "tarjeta".into(),
        es_lbtr: false,
        tarjeta_id: Some(tarjeta),
        cuenta_ahorro_id: None,
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

fn hoy(a: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(a, m, d).unwrap()
}

/// Siembra agosto y septiembre de 2026 con los comandos reales. Devuelve la cuenta y la tarjeta.
///
/// Septiembre: alimentación 1187.53 + 33.37 (cada transferencia lleva su retención como costo adicional),
/// transporte 3177.29, un consumo USD 53.11 liquidado por el emisor en 3190.67 y otro de USD 12.34 pendiente, un
/// abono de 2111.37 (con su comisión) y un avance de 1187.53 con cargo del 6.25 %.
fn sembrar() -> (i64, i64) {
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 5_000_000.0);
    let tarjeta = crear_tarjeta("traduce");
    // Agosto (mes anterior)
    transferencia("02/08/2026", "310.47", "Alimentación", cuenta);
    transferencia("21/08/2026", "205.91", "Alimentación", cuenta);
    transferencia("10/08/2026", "151.13", "Transporte", cuenta);
    // Septiembre
    transferencia("03/09/2026", "1187.53", "Alimentación", cuenta);
    transferencia("20/09/2026", "33.37", "Alimentación", cuenta);
    transferencia("11/09/2026", "3177.29", "Transporte", cuenta);
    let liquidado = compra_en_dolares("10/09/2026", "53.11", tarjeta);
    liquidar_consumo_pendiente(liquidado, importe("3190.67")).unwrap();
    compra_en_dolares("14/09/2026", "12.34", tarjeta);
    crate::comandos::abonos::registrar_pago_tarjeta(tarjeta, "15/09/2026".into(), importe("2111.37"), "DOP".into(), Some(cuenta), 0.0).unwrap();
    crate::comandos::avances::registrar_avance_efectivo(
        tarjeta,
        cuenta,
        "12/09/2026".into(),
        importe("1187.53"),
        "DOP".into(),
        "porcentaje".into(),
        Some(6.25),
        None,
        None,
    )
    .unwrap();
    (cuenta, tarjeta)
}

fn fila<'a>(r: &'a ResumenGastos, nombre: &str) -> &'a crate::comandos::resumen::FilaResumenGasto {
    r.filas.iter().find(|f| f.categoria_nombre == nombre).unwrap_or_else(|| panic!("no hay la fila «{nombre}»"))
}

/// Pesos de un texto `1234.56` a centavos, sin coma flotante.
fn centavos(t: &str) -> i64 {
    crate::dominio::dinero::centavos_desde_texto(t).unwrap()
}

// --- Flujo completo ---

#[test]
fn integracion_el_comando_resume_septiembre_con_importes_en_texto_exacto() {
    let _g = entorno_para_pruebas_externas();
    sembrar();

    let r = resumen_en(hoy(2026, 10, 28), 2026, 9, 3, 60.0, vec![]).unwrap();

    assert!(!r.parcial);
    assert_eq!(r.hasta_dia, None);
    assert_eq!(r.tasa_usd, "60.00");
    assert_eq!(r.id_cargos_derivados, -1);
    // Agosto es el único previo con datos: el historial corto no se promedia entre tres meses.
    assert_eq!(r.meses_promedio, 1);
    assert_eq!(
        r.meses_disponibles.iter().map(|m| (m.anio, m.mes)).collect::<Vec<_>>(),
        vec![(2026, 9), (2026, 8)]
    );

    // Alimentación: 1187.53 + 33.37 más sus retenciones (0.20 %: 2.38 y 0.07), dos gastos.
    let ali = fila(&r, "Alimentación");
    assert_eq!(ali.medidas.importe, "1223.35");
    assert_eq!(ali.medidas.n, 2);
    assert_eq!(ali.medidas.anterior, "517.41"); // 310.47 + 0.62 + 205.91 + 0.41
    assert_eq!(ali.medidas.usd_aparte, "0.00");
    assert!(ali.medidas.anterior_mismo_dia.is_none(), "septiembre está cerrado");

    // Transporte: 3177.29 + 6.35.
    assert_eq!(fila(&r, "Transporte").medidas.importe, "3183.64");

    // «Otros» recoge los dos consumos en dólares: 3190.67 liquidado + 12.34 * 60 = 740.40; y los 65.45 USD aparte.
    let otros = fila(&r, "Otros");
    assert_eq!(otros.medidas.importe, "3931.07");
    assert_eq!(otros.medidas.usd_aparte, "65.45");
    assert_eq!(otros.medidas.n, 2);

    // Cargos derivados: la comisión del abono (0.20 % de 2111.37 = 4.22) y el cargo del avance (6.25 % de 1187.53 = 74.22).
    let d = r.filas.iter().find(|f| f.derivado).expect("fila de cargos derivados");
    assert_eq!(d.categoria_id, -1);
    assert_eq!(d.categoria_nombre, "Cargos derivados");
    assert_eq!(d.medidas.importe, "78.44");
    assert_eq!(d.medidas.n, 2);
    assert_eq!(r.filas.iter().filter(|f| f.derivado).count(), 1);

    // El total es la suma de las filas, al centavo, y el abono (2111.37) no entra.
    let suma: i64 = r.filas.iter().map(|f| centavos(&f.medidas.importe)).sum();
    assert_eq!(centavos(&r.totales.importe), suma);
    assert_eq!(r.totales.importe, "8416.50");
    assert_eq!(r.totales.usd_aparte, "65.45");
    // Orden: de mayor a menor importe.
    let orden: Vec<i64> = r.filas.iter().map(|f| centavos(&f.medidas.importe)).collect();
    assert!(orden.windows(2).all(|p| p[0] >= p[1]), "{orden:?}");
}

#[test]
fn integracion_la_suma_de_las_filas_coincide_con_la_suma_directa_de_los_gastos_de_la_base() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    // La cuenta de cada gasto de septiembre leída directamente de la tabla, con los dólares ya en pesos
    // (liquidados por su importe; el resto a 60).
    let c = conexion();
    let mut stmt = c
        .prepare(
            "SELECT monto, costo_adicional, divisa, estado_conversion, monto_liquidado FROM gastos WHERE fecha LIKE '%/09/2026';",
        )
        .unwrap();
    let mut directa = 0i64;
    let filas = stmt
        .query_map([], |r| {
            Ok((r.get::<_, f64>(0)?, r.get::<_, f64>(1)?, r.get::<_, String>(2)?, r.get::<_, Option<String>>(3)?, r.get::<_, Option<f64>>(4)?))
        })
        .unwrap();
    for f in filas {
        let (m, costo, divisa, estado, liquidado) = f.unwrap();
        let m_c = (m * 100.0).round() as i64;
        let costo_c = (costo * 100.0).round() as i64;
        directa += match (divisa.as_str(), estado.as_deref(), liquidado) {
            ("USD", Some("liquidado"), Some(l)) => (l * 100.0).round() as i64 + costo_c * 60,
            ("USD", _, _) => (m_c + costo_c) * 60,
            _ => m_c + costo_c,
        };
    }
    let r = resumen_en(hoy(2026, 10, 28), 2026, 9, 0, 60.0, vec![]).unwrap();
    assert_eq!(centavos(&r.totales.importe), directa);
    // Y los abonos (la tabla de pagos) no suman nada al resumen.
    let abonos: i64 = c.query_row("SELECT COUNT(*) FROM pagos_tarjeta;", [], |r| r.get(0)).unwrap();
    assert_eq!(abonos, 1);
}

#[test]
fn integracion_un_mes_parcial_se_marca_y_compara_contra_el_mismo_tramo_del_anterior() {
    let _g = entorno_para_pruebas_externas();
    sembrar();

    // «Hoy» es el 15/09: septiembre es el mes en curso. Del agosto anterior cuentan solo los días 1 a 15.
    let r = resumen_en(hoy(2026, 9, 15), 2026, 9, 3, 60.0, vec![]).unwrap();

    assert!(r.parcial);
    assert_eq!(r.hasta_dia, Some(15));
    let ali = fila(&r, "Alimentación");
    assert_eq!(ali.medidas.anterior, "517.41", "el mes entero");
    assert_eq!(ali.medidas.anterior_mismo_dia.as_deref(), Some("311.09"), "solo el 02/08 (310.47 + 0.62)");
    let mismo = ali.medidas.anterior_mismo_dia.as_ref().expect("mes parcial: trae mismo día");
    assert!(centavos(mismo) < centavos(&ali.medidas.anterior), "el 21/08 queda fuera del tramo");
    assert!(ali.medidas.variacion_mismo_dia_pb.is_some());
    // El promedio no incluye el mes en curso: solo agosto.
    assert_eq!(r.meses_promedio, 1);
    assert_eq!(ali.medidas.promedio.as_deref(), Some(ali.medidas.anterior.as_str()));
}

#[test]
fn integracion_las_exclusiones_quitan_filas_incluida_la_de_cargos_derivados() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    let completo = resumen_en(hoy(2026, 10, 28), 2026, 9, 0, 60.0, vec![]).unwrap();

    let sin_ali = resumen_en(hoy(2026, 10, 28), 2026, 9, 0, 60.0, vec![categoria_por_nombre("Alimentación")]).unwrap();
    assert!(sin_ali.filas.iter().all(|f| f.categoria_nombre != "Alimentación"));
    assert_eq!(
        centavos(&sin_ali.totales.importe),
        centavos(&completo.totales.importe) - centavos(&fila(&completo, "Alimentación").medidas.importe)
    );

    let sin_derivados = resumen_en(hoy(2026, 10, 28), 2026, 9, 0, 60.0, vec![-1]).unwrap();
    assert!(sin_derivados.filas.iter().all(|f| !f.derivado));
    assert_eq!(centavos(&sin_derivados.totales.importe), centavos(&completo.totales.importe) - 7_844);
}

#[test]
fn integracion_el_comando_real_con_el_reloj_resume_un_mes_ya_cerrado() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    // El comando que registra Tauri (con la fecha de hoy de la máquina): septiembre de 2026 ya no es el mes en curso.
    let r = obtener_resumen_gastos(2026, 9, 3, 60.0, vec![]).unwrap();
    assert!(!r.parcial);
    assert_eq!(r.totales.importe, "8416.50");
}

#[test]
fn integracion_un_periodo_invalido_o_una_tasa_invalida_son_errores_legibles_sin_escribir() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    let antes = huella_de_la_base();
    let futuro = obtener_resumen_gastos(2999, 1, 3, 60.0, vec![]).unwrap_err();
    assert!(futuro.contains("no es válido"), "{futuro}");
    assert!(obtener_resumen_gastos(2026, 13, 3, 60.0, vec![]).is_err());
    assert!(obtener_resumen_gastos(2026, 9, 3, 0.0, vec![]).is_err(), "una tasa de cero no convierte nada");
    assert_eq!(huella_de_la_base(), antes);
}

// --- Solo lectura ---

#[test]
fn integracion_el_comando_no_escribe_nada() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    let antes = huella_de_la_base();
    assert!(antes.contains("gastos:"), "la huella debe cubrir las tablas");

    for (mes, hoy_) in [((2026, 9), hoy(2026, 10, 28)), ((2026, 9), hoy(2026, 9, 15)), ((2026, 8), hoy(2026, 10, 28))] {
        resumen_en(hoy_, mes.0, mes.1, 3, 60.0, vec![-1]).unwrap();
        assert_eq!(huella_de_la_base(), antes, "el resumen de {mes:?} alteró la base");
    }
    obtener_resumen_gastos(2026, 9, 3, 60.0, vec![]).unwrap();
    assert_eq!(huella_de_la_base(), antes);
}

// --- Contrato de forma con la interfaz ---

/// Los campos que declara `tipos-ipc.d.ts` para `tipo`: `(nombre, tipo TypeScript)`.
fn campos_declarados(dts: &str, tipo: &str) -> Option<Vec<(String, String)>> {
    let inicio = dts.lines().find(|l| l.starts_with(&format!("export type {tipo} = {{")))?;
    let cuerpo = &inicio[inicio.find('{')? + 1..inicio.rfind('}')?];
    let mut campos = Vec::new();
    let (mut profundidad, mut actual) = (0i32, String::new());
    for c in cuerpo.chars() {
        match c {
            '{' | '<' | '(' => profundidad += 1,
            '}' | '>' | ')' => profundidad -= 1,
            _ => {}
        }
        if c == ';' && profundidad == 0 {
            campos.push(std::mem::take(&mut actual));
        } else {
            actual.push(c);
        }
    }
    if !actual.trim().is_empty() {
        campos.push(actual);
    }
    campos
        .into_iter()
        .map(|c| {
            let (n, t) = c.split_once(':')?;
            Some((n.trim().to_string(), t.trim().to_string()))
        })
        .collect()
}

/// Comprueba que `valor` tiene exactamente las claves (y los tipos básicos) que declara `dts` para `tipo`, y baja a
/// los tipos anidados que `dts` también declara.
fn verificar_forma(dts: &str, tipo: &str, valor: &Value, ruta: &str) {
    let declarados = campos_declarados(dts, tipo).unwrap_or_else(|| panic!("tipos-ipc.d.ts no declara «{tipo}»"));
    let objeto = valor.as_object().unwrap_or_else(|| panic!("{ruta} no es un objeto"));
    let mut reales: Vec<&str> = objeto.keys().map(String::as_str).collect();
    let mut esperados: Vec<&str> = declarados.iter().map(|(n, _)| n.as_str()).collect();
    reales.sort_unstable();
    esperados.sort_unstable();
    assert_eq!(reales, esperados, "{ruta}: las claves de Rust y las de tipos-ipc.d.ts no coinciden");

    for (nombre, ts) in &declarados {
        let v = &objeto[nombre];
        let admite_null = ts.contains("| null");
        let base = ts.replace(" | null", "");
        let base = base.trim().trim_end_matches("[]").trim_start_matches('(').trim_end_matches(')').trim();
        let ruta_campo = format!("{ruta}.{nombre}");
        if v.is_null() {
            assert!(admite_null, "{ruta_campo} es null pero el tipo ({ts}) no lo admite");
            continue;
        }
        match base {
            "string" => assert!(v.is_string(), "{ruta_campo} debe ser texto ({ts}) y es {v}"),
            "number" => assert!(v.is_number(), "{ruta_campo} debe ser número ({ts}) y es {v}"),
            "boolean" => assert!(v.is_boolean(), "{ruta_campo} debe ser booleano ({ts}) y es {v}"),
            otro if dts.contains(&format!("export type {otro} = {{")) => {
                if let Some(lista) = v.as_array() {
                    for (i, e) in lista.iter().enumerate() {
                        verificar_forma(dts, otro, e, &format!("{ruta_campo}[{i}]"));
                    }
                } else {
                    verificar_forma(dts, otro, v, &ruta_campo);
                }
            }
            otro => panic!("{ruta_campo}: tipo «{otro}» que esta prueba no sabe verificar"),
        }
    }
}

#[test]
fn integracion_el_dto_serializado_tiene_exactamente_las_claves_de_tipos_ipc() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    let dts = include_str!("../../src/js/tipos-ipc.d.ts");

    // Mes cerrado y mes parcial: las opciones (`null` / texto) deben encajar con lo declarado en ambos.
    for (hoy_, mes) in [(hoy(2026, 10, 28), 9), (hoy(2026, 9, 15), 9)] {
        let dto = resumen_en(hoy_, 2026, mes, 3, 60.0, vec![]).unwrap();
        let json = serde_json::to_value(&dto).unwrap();
        verificar_forma(dts, "ResumenGastos", &json, "ResumenGastos");
        assert!(!json["filas"].as_array().unwrap().is_empty());
    }
    // Y el comando declara que devuelve ese tipo.
    let linea = dts.lines().find(|l| l.contains("obtener_resumen_gastos:")).expect("comando declarado");
    assert!(linea.contains("ret: ResumenGastos"), "{linea}");
    assert!(linea.contains("nPromedio: number") && linea.contains("tasaUsd: number") && linea.contains("excluirCategorias: number[]"), "{linea}");
}

#[test]
fn integracion_ningun_importe_viaja_como_numero() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    let dto = resumen_en(hoy(2026, 10, 28), 2026, 9, 3, 60.0, vec![]).unwrap();
    let json = serde_json::to_value(&dto).unwrap();
    for medidas in std::iter::once(&json["totales"]).chain(json["filas"].as_array().unwrap().iter().map(|f| &f["medidas"])) {
        for campo in ["importe", "anterior", "usd_aparte"] {
            let t = medidas[campo].as_str().unwrap_or_else(|| panic!("{campo} debe ser texto"));
            assert!(t.contains('.') && t.split('.').nth(1).unwrap().len() == 2, "{campo} = {t}");
        }
        for campo in ["anterior_mismo_dia", "promedio"] {
            assert!(medidas[campo].is_null() || medidas[campo].is_string(), "{campo}");
        }
    }
}
