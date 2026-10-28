//! Pruebas del adaptador SQLite del resumen de gastos, sobre una base en memoria con el esquema real.
//! Datos inventados, con céntimos poco redondos.

use super::gastos::AlmacenSqlite;
use crate::db_sql;
use crate::puertos::repositorios::*;
use rusqlite::{params, Connection};

fn base() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute("PRAGMA foreign_keys = ON;", []).unwrap();
    db_sql::crear_esquema(&mut conn).unwrap();
    // `crear_esquema` ya siembra "Transporte"; "Comida" es una categoría propia de las pruebas.
    conn.execute("INSERT OR IGNORE INTO categorias (nombre) VALUES ('Comida');", []).unwrap();
    conn.execute("INSERT OR IGNORE INTO categorias (nombre) VALUES ('Transporte');", []).unwrap();
    conn.execute(
        "INSERT INTO tarjetas (entidad, nombre_tarjeta, fecha_corte, fecha_limite_pago, balance_pesos, balance_dolares,
                               limite_pesos, limite_dolares)
         VALUES ('Banco Ejemplo', 'Tarjeta', 15, 5, 0.0, 0.0, 100000.0, 5000.0);",
        [],
    )
    .unwrap();
    conn
}

fn categoria(conn: &Connection, nombre: &str) -> i64 {
    conn.query_row("SELECT id FROM categorias WHERE nombre = ?;", [nombre], |r| r.get(0)).unwrap()
}

fn gasto(conn: &Connection, fecha: &str, monto: f64, divisa: &str, cat: i64, costo: f64) -> i64 {
    conn.execute(
        "INSERT INTO gastos (fecha, monto, divisa, descripcion, categoria_id, metodo_pago, costo_adicional)
         VALUES (?, ?, ?, 'Prueba', ?, 'transferencia', ?);",
        params![fecha, monto, divisa, cat, costo],
    )
    .unwrap();
    conn.last_insert_rowid()
}

fn leer(conn: &mut Connection) -> Vec<FilaAgregadoGasto> {
    let tx = conn.transaction().unwrap();
    AlmacenSqlite::nuevo(&tx).gastos_agrupados().unwrap()
}

#[test]
fn agrupa_por_dia_categoria_y_divisa_en_centavos_exactos() {
    let mut conn = base();
    let comida = categoria(&conn, "Comida");
    // 0.1 + 0.2 mil veces: sumado como REAL no daría 300.00 exacto.
    for _ in 0..1000 {
        gasto(&conn, "03/09/2026", 0.1, "DOP", comida, 0.0);
        gasto(&conn, "03/09/2026", 0.2, "DOP", comida, 0.0);
    }
    gasto(&conn, "04/09/2026", 1187.53, "DOP", comida, 24.77);
    gasto(&conn, "04/09/2026", 12.34, "USD", comida, 0.0);
    let filas = leer(&mut conn);
    assert_eq!(filas.len(), 3);
    let dia3 = filas.iter().find(|f| f.dia == 3).unwrap();
    assert_eq!((dia3.centavos, dia3.n, dia3.mes, dia3.anio), (30_000, 2000, 9, 2026));
    let dop4 = filas.iter().find(|f| f.dia == 4 && f.divisa == "DOP").unwrap();
    assert_eq!(dop4.centavos, 118_753 + 2_477, "monto + costo adicional");
    let usd4 = filas.iter().find(|f| f.divisa == "USD").unwrap();
    assert_eq!((usd4.centavos, usd4.liquidado_dop_centavos, usd4.liquidado_origen_centavos), (1_234, None, 0));
}

#[test]
fn el_usd_liquidado_trae_su_importe_en_pesos_y_el_pendiente_no() {
    let mut conn = base();
    let comida = categoria(&conn, "Comida");
    let liquidado = gasto(&conn, "05/09/2026", 53.11, "USD", comida, 0.0);
    conn.execute("UPDATE gastos SET estado_conversion = 'liquidado', monto_liquidado = 3177.29 WHERE id = ?;", [liquidado])
        .unwrap();
    let pendiente = gasto(&conn, "05/09/2026", 10.07, "USD", comida, 0.0);
    conn.execute("UPDATE gastos SET estado_conversion = 'pendiente' WHERE id = ?;", [pendiente]).unwrap();
    let filas = leer(&mut conn);
    assert_eq!(filas.len(), 1);
    let f = &filas[0];
    assert_eq!(f.centavos, 5_311 + 1_007);
    assert_eq!(f.liquidado_origen_centavos, 5_311);
    assert_eq!(f.liquidado_dop_centavos, Some(317_729));
    assert_eq!(f.n, 2);
}

#[test]
fn los_derivados_se_identifican_por_sus_enlaces_y_no_por_el_texto() {
    let mut conn = base();
    let comida = categoria(&conn, "Comida");
    let comision = gasto(&conn, "06/09/2026", 24.69, "DOP", comida, 0.0);
    let cargo = gasto(&conn, "06/09/2026", 61.43, "DOP", comida, 0.0);
    gasto(&conn, "06/09/2026", 17.11, "DOP", comida, 0.0); // misma descripción, sin enlace
    conn.execute(
        "INSERT INTO pagos_tarjeta (tarjeta_id, fecha_pago, monto_pagado, divisa, gasto_comision_id)
         VALUES (1, '06/09/2026', 3177.29, 'DOP', ?);",
        [comision],
    )
    .unwrap();
    conn.execute("INSERT INTO cuentas_ahorro (nombre, divisa, balance_actual) VALUES ('Cuenta', 'DOP', 0.0);", []).unwrap();
    conn.execute(
        "INSERT INTO avances_efectivo (tarjeta_id, cuenta_ahorro_id, fecha, monto, divisa, tipo_cargo, cargo, gasto_cargo_id)
         VALUES (1, 1, '06/09/2026', 1000.0, 'DOP', 'exonerado', 61.43, ?);",
        [cargo],
    )
    .unwrap();
    let filas = leer(&mut conn);
    let derivadas = filas.iter().find(|f| f.derivado).unwrap();
    assert_eq!((derivadas.centavos, derivadas.n), (2_469 + 6_143, 2));
    let normales = filas.iter().find(|f| !f.derivado).unwrap();
    assert_eq!((normales.centavos, normales.n), (1_711, 1));
}

#[test]
fn caracterizacion_los_abonos_no_son_gasto_y_la_suma_de_las_filas_es_la_de_los_gastos() {
    let mut conn = base();
    let comida = categoria(&conn, "Comida");
    let transporte = categoria(&conn, "Transporte");
    gasto(&conn, "01/08/2026", 1187.53, "DOP", comida, 0.0);
    gasto(&conn, "15/09/2026", 3177.29, "DOP", transporte, 8.71);
    gasto(&conn, "16/09/2026", 33.33, "DOP", comida, 0.0);
    // Abonos por montos grandes y feos: no deben aparecer en ningún agregado.
    for monto in [9_999.97, 4_321.09] {
        conn.execute(
            "INSERT INTO pagos_tarjeta (tarjeta_id, fecha_pago, monto_pagado, divisa) VALUES (1, '16/09/2026', ?, 'DOP');",
            [monto],
        )
        .unwrap();
    }
    let filas = leer(&mut conn);
    let suma_filas: i64 = filas.iter().map(|f| f.centavos).sum();
    let mut directa = 0i64;
    let mut stmt = conn.prepare("SELECT monto, costo_adicional FROM gastos;").unwrap();
    for par in stmt.query_map([], |r| Ok((r.get::<_, f64>(0)?, r.get::<_, f64>(1)?))).unwrap() {
        let (m, c) = par.unwrap();
        directa += (m * 100.0).round() as i64 + (c * 100.0).round() as i64;
    }
    assert_eq!(suma_filas, directa);
    assert_eq!(suma_filas, 118_753 + 317_729 + 871 + 3_333);
    assert_eq!(filas.iter().map(|f| f.n).sum::<i64>(), 3);
}

#[test]
fn una_fecha_ilegible_falla_en_voz_alta_en_lugar_de_perder_el_gasto() {
    let mut conn = base();
    let comida = categoria(&conn, "Comida");
    gasto(&conn, "ayer", 10.0, "DOP", comida, 0.0);
    let tx = conn.transaction().unwrap();
    let e = AlmacenSqlite::nuevo(&tx).gastos_agrupados().unwrap_err();
    assert!(e.to_string().contains("ayer"), "{e}");
}

#[test]
fn la_fecha_sin_cero_inicial_se_entiende() {
    let mut conn = base();
    let comida = categoria(&conn, "Comida");
    gasto(&conn, "3/9/2026", 10.0, "DOP", comida, 0.0);
    let filas = leer(&mut conn);
    assert_eq!((filas[0].anio, filas[0].mes, filas[0].dia), (2026, 9, 3));
}
