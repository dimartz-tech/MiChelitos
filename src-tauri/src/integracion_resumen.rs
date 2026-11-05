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

// --- Tendencia de 6 meses ---

/// Siembra seis meses de historial (abril a septiembre de 2026, más un gasto pequeño en octubre) con los comandos
/// reales. Alimentación sube (≈1000 → ≈2000: más del 10 % y más de DOP 500); Transporte se mantiene (≈150) de mayo a septiembre.
fn sembrar_historial() {
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 5_000_000.0);
    for (fecha, monto) in [
        ("05/04/2026", "1000.17"), ("05/05/2026", "999.91"), ("05/06/2026", "1000.33"),
        ("05/07/2026", "2000.07"), ("05/08/2026", "2100.19"), ("05/09/2026", "1900.43"),
    ] {
        transferencia(fecha, monto, "Alimentación", cuenta);
    }
    for (fecha, monto) in [
        ("09/05/2026", "150.41"), ("09/06/2026", "149.77"), ("09/07/2026", "150.11"),
        ("09/08/2026", "149.87"), ("09/09/2026", "150.23"), ("09/10/2026", "0.77"),
    ] {
        transferencia(fecha, monto, "Transporte", cuenta);
    }
}

/// Lo gastado en pesos (monto + costo adicional) por una categoría en un mes, leído directamente de la tabla.
fn gastado_directo(anio: i32, mes: u32, categoria: Option<&str>) -> i64 {
    let c = conexion();
    let patron = format!("%/{mes:02}/{anio}");
    let (sql, filtro) = match categoria {
        Some(n) => ("SELECT monto, costo_adicional FROM gastos WHERE fecha LIKE ?1 AND categoria_id = ?2;", categoria_por_nombre(n)),
        None => ("SELECT monto, costo_adicional FROM gastos WHERE fecha LIKE ?1 AND ?2 = ?2;", 0),
    };
    let mut stmt = c.prepare(sql).unwrap();
    let filas = stmt.query_map(params![patron, filtro], |r| Ok((r.get::<_, f64>(0)?, r.get::<_, f64>(1)?))).unwrap();
    filas.map(|f| { let (m, k) = f.unwrap(); (m * 100.0).round() as i64 + (k * 100.0).round() as i64 }).sum()
}

fn serie_en_centavos(t: &crate::comandos::resumen::TendenciaResumen) -> Vec<Option<i64>> {
    t.meses.iter().map(|p| p.importe.as_deref().map(centavos)).collect()
}

#[test]
fn integracion_la_tendencia_de_septiembre_coincide_con_los_gastos_de_cada_mes_de_la_base() {
    let _g = entorno_para_pruebas_externas();
    sembrar_historial();
    let r = resumen_en(hoy(2026, 10, 28), 2026, 9, 3, 60.0, vec![]).unwrap();

    let ali = fila(&r, "Alimentación");
    let meses: Vec<(i32, u32)> = ali.tendencia.meses.iter().map(|p| (p.anio, p.mes)).collect();
    assert_eq!(meses, vec![(2026, 4), (2026, 5), (2026, 6), (2026, 7), (2026, 8), (2026, 9)]);
    let esperado: Vec<Option<i64>> = (4..=9).map(|m| Some(gastado_directo(2026, m, Some("Alimentación")))).collect();
    assert_eq!(serie_en_centavos(&ali.tendencia), esperado);
    assert!(ali.tendencia.meses.iter().all(|p| !p.parcial));
    assert_eq!(ali.tendencia.direccion, "sube");
    // El último punto es el importe del mes que muestra la tabla.
    assert_eq!(ali.tendencia.meses[5].importe.as_deref(), Some(ali.medidas.importe.as_str()));

    // Transporte no tuvo nada en abril: cero (existía historial), no ausente.
    let tr = fila(&r, "Transporte");
    assert_eq!(serie_en_centavos(&tr.tendencia)[0], Some(0));
    // El total de cada mes es la suma de las filas y coincide con lo gastado en la base.
    let total = serie_en_centavos(&r.tendencia_total);
    for (i, m) in (4..=9u32).enumerate() {
        let suma: i64 = r.filas.iter().map(|f| serie_en_centavos(&f.tendencia)[i].unwrap()).sum();
        assert_eq!(total[i], Some(suma), "mes {m}");
        assert_eq!(total[i], Some(gastado_directo(2026, m, None)), "mes {m} contra la base");
    }
    assert_eq!(r.tendencia_total.meses[5].importe.as_deref(), Some(r.totales.importe.as_str()));
}

#[test]
fn integracion_los_meses_antes_del_historial_son_ausentes_y_la_direccion_dice_datos_insuficientes() {
    let _g = entorno_para_pruebas_externas();
    sembrar_historial();
    // Junio de 2026: la ventana arranca en enero, pero el primer dato es abril.
    let r = resumen_en(hoy(2026, 10, 28), 2026, 6, 3, 60.0, vec![]).unwrap();
    let ali = fila(&r, "Alimentación");
    let serie = serie_en_centavos(&ali.tendencia);
    assert_eq!(&serie[..3], &[None, None, None], "enero a marzo: antes del primer dato");
    assert_eq!(serie[3], Some(gastado_directo(2026, 4, Some("Alimentación"))));
    assert!(ali.tendencia.meses[..3].iter().all(|p| p.importe.is_none()));
    assert_eq!(ali.tendencia.direccion, "datos_insuficientes", "solo 3 meses completos con dato");
    assert_eq!(serie_en_centavos(&r.tendencia_total)[..3], [None, None, None]);
}

#[test]
fn integracion_el_mes_en_curso_va_marcado_y_no_entra_en_la_direccion() {
    let _g = entorno_para_pruebas_externas();
    sembrar_historial();
    // Hoy es el 15/10: octubre es parcial. Transporte lleva ≈150 al mes y solo 0.77 en octubre.
    let r = resumen_en(hoy(2026, 10, 15), 2026, 10, 3, 60.0, vec![]).unwrap();
    let tr = fila(&r, "Transporte");
    let marcas: Vec<bool> = tr.tendencia.meses.iter().map(|p| p.parcial).collect();
    assert_eq!(marcas, vec![false, false, false, false, false, true]);
    assert_eq!(tr.tendencia.meses[5].importe.as_deref(), Some(tr.medidas.importe.as_str()));
    assert_eq!(tr.tendencia.direccion, "estable", "si el parcial contara, parecería una caída");
    assert!(r.tendencia_total.meses[5].parcial);
    // El mismo gasto, visto desde noviembre (octubre ya cerrado), sí cuenta como un mes más y no está marcado.
    let cerrado = resumen_en(hoy(2026, 11, 20), 2026, 10, 3, 60.0, vec![]).unwrap();
    assert!(fila(&cerrado, "Transporte").tendencia.meses.iter().all(|p| !p.parcial));
}

#[test]
fn integracion_las_exclusiones_tambien_quitan_la_serie_del_total() {
    let _g = entorno_para_pruebas_externas();
    sembrar_historial();
    let completo = resumen_en(hoy(2026, 10, 28), 2026, 9, 3, 60.0, vec![]).unwrap();
    let sin = resumen_en(hoy(2026, 10, 28), 2026, 9, 3, 60.0, vec![categoria_por_nombre("Alimentación")]).unwrap();
    assert!(sin.filas.iter().all(|f| f.categoria_nombre != "Alimentación"));
    let a = serie_en_centavos(&completo.tendencia_total);
    let b = serie_en_centavos(&sin.tendencia_total);
    let ali = serie_en_centavos(&fila(&completo, "Alimentación").tendencia);
    for i in 0..6 {
        assert_eq!(b[i].unwrap(), a[i].unwrap() - ali[i].unwrap(), "mes {i}");
    }
}

#[test]
fn integracion_la_tendencia_no_escribe_nada() {
    let _g = entorno_para_pruebas_externas();
    sembrar_historial();
    let antes = huella_de_la_base();
    for (hoy_, mes) in [(hoy(2026, 10, 28), 9), (hoy(2026, 10, 15), 10), (hoy(2026, 10, 28), 6), (hoy(2026, 10, 28), 4)] {
        resumen_en(hoy_, 2026, mes, 3, 60.0, vec![-1]).unwrap();
        assert_eq!(huella_de_la_base(), antes, "el mes {mes} alteró la base");
    }
    obtener_resumen_gastos(2026, 9, 6, 60.0, vec![]).unwrap();
    assert_eq!(huella_de_la_base(), antes);
}

#[test]
fn integracion_el_dto_de_la_tendencia_tiene_la_forma_de_tipos_ipc_y_un_vocabulario_controlado() {
    let _g = entorno_para_pruebas_externas();
    sembrar_historial();
    let dts = include_str!("../../src/js/tipos-ipc.d.ts");
    assert!(dts.contains("export type TendenciaResumen = {") && dts.contains("export type PuntoTendenciaResumen = {"));
    assert!(dts.contains("tendencia_total: TendenciaResumen"), "el total declara su tendencia");
    for (hoy_, mes) in [(hoy(2026, 10, 28), 9), (hoy(2026, 10, 15), 10), (hoy(2026, 10, 28), 5)] {
        let dto = resumen_en(hoy_, 2026, mes, 3, 60.0, vec![]).unwrap();
        let json = serde_json::to_value(&dto).unwrap();
        verificar_forma(dts, "ResumenGastos", &json, "ResumenGastos");
        let tendencias = json["filas"].as_array().unwrap().iter().map(|f| &f["tendencia"]).chain(std::iter::once(&json["tendencia_total"]));
        for t in tendencias {
            let meses = t["meses"].as_array().unwrap();
            assert_eq!(meses.len(), 6);
            assert!(matches!(t["direccion"].as_str().unwrap(), "sube" | "baja" | "estable" | "datos_insuficientes"));
            // El motivo solo existe con «estable» y su vocabulario es controlado.
            match (t["direccion"].as_str().unwrap(), &t["motivo_estable"]) {
                ("estable", Value::String(m)) => assert!(matches!(m.as_str(), "poco_cambio" | "dentro_del_umbral"), "{m}"),
                (otra, Value::Null) => assert_ne!(otra, "estable", "un «estable» sin motivo"),
                (d, m) => panic!("dirección {d} con motivo {m}"),
            }
            for p in meses {
                // Los importes viajan como texto exacto con dos decimales, o null si el mes es anterior al historial.
                match &p["importe"] {
                    Value::Null => {}
                    Value::String(t) => assert!(t.split('.').nth(1).map_or(false, |d| d.len() == 2), "{t}"),
                    otro => panic!("importe de tendencia no es texto: {otro}"),
                }
            }
        }
    }
}


// --- Mínimo absoluto de la tendencia (DOP 500.00) ---

/// Seis meses (abril a septiembre de 2026) en tres categorías, con céntimos poco redondos:
/// * Alimentación: ≈1000 → ≈2000 (+100 %, ≈1000 pesos): sube.
/// * Transporte: ≈200 → ≈300 (+50 %, ≈100 pesos): más del 10 % pero menos de DOP 500: estable por poco cambio.
/// * Otros: ≈3000 → ≈3100 (+3 %): estable, dentro del umbral.
fn sembrar_minimo_absoluto() {
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 50_000_000.0);
    let series: [(&str, [&str; 6]); 3] = [
        ("Alimentación", ["1000.17", "999.91", "1000.33", "2000.07", "2100.19", "1900.43"]),
        ("Transporte", ["200.13", "199.87", "200.29", "300.11", "299.83", "300.47"]),
        ("Otros", ["3000.19", "2999.83", "3000.37", "3100.21", "3099.79", "3100.41"]),
    ];
    for (categoria, importes) in series {
        for (i, monto) in importes.iter().enumerate() {
            transferencia(&format!("07/{:02}/2026", 4 + i), monto, categoria, cuenta);
        }
    }
}

#[test]
fn integracion_el_minimo_absoluto_separa_el_cambio_grande_en_pesos_del_grande_solo_en_porcentaje() {
    let _g = entorno_para_pruebas_externas();
    sembrar_minimo_absoluto();
    let antes = huella_de_la_base();
    let r = resumen_en(hoy(2026, 10, 28), 2026, 9, 3, 60.0, vec![]).unwrap();
    let json = serde_json::to_value(&r).unwrap();

    let ali = fila(&r, "Alimentación");
    assert_eq!((ali.tendencia.direccion.as_str(), ali.tendencia.motivo_estable.as_deref()), ("sube", None));
    let tr = fila(&r, "Transporte");
    assert_eq!((tr.tendencia.direccion.as_str(), tr.tendencia.motivo_estable.as_deref()), ("estable", Some("poco_cambio")));
    let otros = fila(&r, "Otros");
    assert_eq!((otros.tendencia.direccion.as_str(), otros.tendencia.motivo_estable.as_deref()), ("estable", Some("dentro_del_umbral")));
    // El total (≈6200 → ≈7400: +19 %, ≈1200 pesos) también sube, con la misma regla.
    assert_eq!((r.tendencia_total.direccion.as_str(), r.tendencia_total.motivo_estable.as_deref()), ("sube", None));

    // El DTO tal como viaja: el campo existe en cada tendencia, como texto o null.
    for f in json["filas"].as_array().unwrap() {
        assert!(f["tendencia"].get("motivo_estable").is_some(), "falta motivo_estable en {}", f["categoria_nombre"]);
    }
    assert_eq!(json["filas"].as_array().unwrap().iter().find(|f| f["categoria_nombre"] == "Transporte").unwrap()["tendencia"]["motivo_estable"], "poco_cambio");
    assert!(json["tendencia_total"]["motivo_estable"].is_null());
    let dts = include_str!("../../src/js/tipos-ipc.d.ts");
    assert!(dts.contains("motivo_estable: string | null"), "tipos-ipc.d.ts declara el motivo");
    verificar_forma(dts, "ResumenGastos", &json, "ResumenGastos");

    // Pedir el resumen no escribe nada.
    assert_eq!(huella_de_la_base(), antes);
}

#[test]
fn integracion_la_baja_con_cambio_pequeno_en_pesos_es_estable_y_con_cambio_grande_baja() {
    let _g = entorno_para_pruebas_externas();
    let cuenta = crear_cuenta("Cuenta Ahorros DOP", "DOP", 50_000_000.0);
    // Transporte cae de ≈300 a ≈200 (−33 %, ≈100 pesos); Alimentación de ≈2000 a ≈1000 (−50 %, ≈1000 pesos).
    for (categoria, importes) in [
        ("Transporte", ["300.13", "299.87", "300.29", "200.11", "199.83", "200.47"]),
        ("Alimentación", ["2000.17", "1999.91", "2000.33", "1000.07", "1100.19", "900.43"]),
    ] {
        for (i, monto) in importes.iter().enumerate() {
            transferencia(&format!("07/{:02}/2026", 4 + i), monto, categoria, cuenta);
        }
    }
    let r = resumen_en(hoy(2026, 10, 28), 2026, 9, 3, 60.0, vec![]).unwrap();
    assert_eq!(fila(&r, "Transporte").tendencia.direccion, "estable");
    assert_eq!(fila(&r, "Transporte").tendencia.motivo_estable.as_deref(), Some("poco_cambio"));
    assert_eq!(fila(&r, "Alimentación").tendencia.direccion, "baja");
    assert_eq!(fila(&r, "Alimentación").tendencia.motivo_estable, None);
}
