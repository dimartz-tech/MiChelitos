//! Pruebas de caracterización de las sumas y de las comparaciones de saldo
//! sobre columnas `REAL` (deuda técnica A2, primera mitad: fijar antes de
//! centralizar).
//!
//! **No cambian ni juzgan nada de producción: fijan lo que hace hoy.** Todos los
//! importes son inventados. Lo que se mide aquí es la frontera entre el `REAL`
//! que guarda SQLite y los centavos enteros de `Dinero`:
//!
//! * las escrituras que acumulan (`ajustar_saldo`, `ajustar_deuda`) dejan en la
//!   columna exactamente la suma en centavos enteros, por «feos» que sean los
//!   céntimos;
//! * las comparaciones de saldo cero (`archivar_cuenta`, `desactivar_tarjeta`)
//!   se hacen con `ROUND(x * 100) = 0`, no por igualdad de `f64`;
//! * el `REAL` que se lee vuelve a entrar como `Dinero` sin perder un centavo;
//! * **no hay ningún `SUM(`/`AVG(` en SQL de producción**: las sumas viven en
//!   Rust sobre `Dinero` (enteros) y en la interfaz sobre `number` (f64).
//!
//! Lo que muestre una deriva real se nombra `hallazgo_…`: queda fijado tal cual
//! está, no se corrige sin consultar al titular.

use crate::db_sql;
use crate::dominio::dinero::{centavos_desde_texto, Dinero, Divisa};
use crate::puertos::repositorios::{RepositorioCuentas, RepositorioTarjetas, TarjetasParaCierre};
use crate::adaptadores::sqlite::gastos::AlmacenSqlite;
use rusqlite::{params, Connection};

// --- Ayudantes ---

/// Base en memoria con el esquema real (sin tocar HOME ni el disco).
fn base() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute("PRAGMA foreign_keys = ON;", []).unwrap();
    db_sql::crear_esquema(&mut conn).unwrap();
    conn
}

fn crear_cuenta(conn: &Connection, nombre: &str, divisa: &str, balance: f64) -> i64 {
    conn.execute(
        "INSERT INTO cuentas_ahorro (nombre, divisa, balance_actual) VALUES (?, ?, ?);",
        params![nombre, divisa, balance],
    )
    .unwrap();
    conn.last_insert_rowid()
}

fn crear_tarjeta(conn: &Connection, nombre: &str, pesos: f64, dolares: f64) -> i64 {
    conn.execute(
        "INSERT INTO tarjetas (entidad, nombre_tarjeta, fecha_corte, fecha_limite_pago, balance_pesos, balance_dolares,
                               limite_pesos, limite_dolares)
         VALUES ('Banco Ejemplo', ?, 15, 5, ?, ?, 100000.0, 5000.0);",
        params![nombre, pesos, dolares],
    )
    .unwrap();
    conn.last_insert_rowid()
}

fn balance_cuenta(conn: &Connection, id: i64) -> f64 {
    conn.query_row("SELECT balance_actual FROM cuentas_ahorro WHERE id = ?;", [id], |r| r.get(0)).unwrap()
}

fn balance_tarjeta(conn: &Connection, id: i64, columna: &str) -> f64 {
    conn.query_row(&format!("SELECT {columna} FROM tarjetas WHERE id = ?;"), [id], |r| r.get(0)).unwrap()
}

fn dop_c(centavos: i64) -> Dinero {
    Dinero::de_centavos(centavos, Divisa::Dop)
}

fn usd_c(centavos: i64) -> Dinero {
    Dinero::de_centavos(centavos, Divisa::Usd)
}

/// Generador congruencial fijo: mismos «importes feos» en cada ejecución, sin
/// depender de ninguna crate ni del reloj.
struct Lcg(u64);

impl Lcg {
    fn siguiente(&mut self, modulo: u64) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) % modulo
    }
}

/// Centavos «feos» a mano: los que en binario no existen (0.10, 0.07, 19.99…),
/// los de magnitud grande y los que acaban en 5.
const FEOS: [i64; 18] = [
    10, 10, 10, 7, 1_999, 123_456_789, 1, 1, 3, 33, 333, 99_999_999, 1_015, 285, 29, 435, 820, 5,
];

// --- (a) Acumulación en escritura: la columna queda con la suma exacta en centavos ---

#[test]
fn ajustar_saldo_acumula_exactamente_en_centavos_con_importes_feos() {
    let mut conn = base();
    let id = crear_cuenta(&conn, "Cuenta Feos", "DOP", 0.0);
    let mut exacto: i64 = 0;
    let mut azar = Lcg(2026);
    {
        let tx = conn.transaction().unwrap();
        let mut almacen = AlmacenSqlite::nuevo(&tx);
        // Primero los feos a mano, ida y vuelta, y luego 3 000 movimientos pseudoaleatorios con signo.
        for _ in 0..50 {
            for c in FEOS {
                almacen.ajustar_saldo(id, dop_c(c)).unwrap();
                exacto += c;
            }
        }
        for paso in 0..3_000 {
            let c = azar.siguiente(5_000_000) as i64 + 1;
            let delta = if paso % 3 == 0 { -c } else { c };
            almacen.ajustar_saldo(id, dop_c(delta)).unwrap();
            exacto += delta;
        }
        tx.commit().unwrap();
    }
    let guardado = balance_cuenta(&conn, id);
    assert_eq!((guardado * 100.0).round() as i64, exacto, "la columna debe ser la suma exacta en centavos");
    assert_eq!(guardado, exacto as f64 / 100.0, "y el REAL debe ser el decimal canónico de esos centavos");
}

#[test]
fn mil_veces_diez_centavos_en_una_cuenta_son_exactamente_cien_pesos() {
    let mut conn = base();
    let id = crear_cuenta(&conn, "Cuenta Diez", "DOP", 0.0);
    let tx = conn.transaction().unwrap();
    {
        let mut almacen = AlmacenSqlite::nuevo(&tx);
        for _ in 0..1_000 {
            almacen.ajustar_saldo(id, dop_c(10)).unwrap();
        }
    }
    tx.commit().unwrap();
    assert_eq!(balance_cuenta(&conn, id), 100.0, "igualdad exacta de f64, no con tolerancia");
}

#[test]
fn ajustar_deuda_acumula_exactamente_en_centavos_en_pesos_y_en_dolares() {
    let mut conn = base();
    let id = crear_tarjeta(&conn, "Tarjeta Feos", 0.0, 0.0);
    let (mut exacto_dop, mut exacto_usd) = (0_i64, 0_i64);
    let mut azar = Lcg(77);
    {
        let tx = conn.transaction().unwrap();
        let mut almacen = AlmacenSqlite::nuevo(&tx);
        for c in FEOS.iter().cycle().take(900) {
            almacen.ajustar_deuda(id, dop_c(*c)).unwrap();
            exacto_dop += c;
            almacen.ajustar_deuda(id, usd_c(*c)).unwrap();
            exacto_usd += c;
        }
        for paso in 0..3_000 {
            let c = azar.siguiente(2_000_000) as i64 + 1;
            let delta = if paso % 4 == 0 { -c } else { c };
            if paso % 2 == 0 {
                almacen.ajustar_deuda(id, dop_c(delta)).unwrap();
                exacto_dop += delta;
            } else {
                almacen.ajustar_deuda(id, usd_c(delta)).unwrap();
                exacto_usd += delta;
            }
        }
        tx.commit().unwrap();
    }
    let dop = balance_tarjeta(&conn, id, "balance_pesos");
    let usd = balance_tarjeta(&conn, id, "balance_dolares");
    assert_eq!(dop, exacto_dop as f64 / 100.0, "pesos: suma exacta en centavos");
    assert_eq!(usd, exacto_usd as f64 / 100.0, "dólares: suma exacta en centavos");
}

#[test]
fn la_deuda_a_gran_magnitud_sigue_siendo_exacta_al_centavo() {
    // `ROUND(balance + ?, 2)` suma en binario y redondea en decimal: debe seguir acertando cuando el ulp se acerca
    // al medio centavo. Se mide a 10^9, 10^11 y 9 · 10^12 pesos (un cupo real no pasa de 10^7).
    for base_centavos in [100_000_000_000_i64, 10_000_000_000_000, 900_000_000_000_000] {
        let mut conn = base();
        let id = crear_tarjeta(&conn, "Tarjeta Enorme", base_centavos as f64 / 100.0, 0.0);
        let mut exacto = base_centavos;
        {
            let tx = conn.transaction().unwrap();
            let mut almacen = AlmacenSqlite::nuevo(&tx);
            for c in FEOS.iter().cycle().take(400) {
                almacen.ajustar_deuda(id, dop_c(*c)).unwrap();
                exacto += c;
            }
            tx.commit().unwrap();
        }
        let guardado = balance_tarjeta(&conn, id, "balance_pesos");
        assert_eq!(guardado, exacto as f64 / 100.0, "magnitud {base_centavos} centavos");
    }
}

// --- (a) Lectura: el REAL vuelve a Dinero sin perder un centavo ---

#[test]
fn todo_importe_exacto_al_centavo_sobrevive_al_viaje_real_y_vuelta() {
    // Barrido denso en el rango corriente y barrido pseudoaleatorio hasta ~10^14 centavos. Es la premisa de
    // `Dinero::nuevo(f64)` en la frontera con SQLite: el REAL que se guardó es siempre un centavo canónico.
    for c in -2_000_000_i64..=2_000_000 {
        let leido = Dinero::nuevo(c as f64 / 100.0, Divisa::Dop).unwrap();
        assert_eq!(leido.centavos(), c, "ida y vuelta en {c} centavos");
    }
    let mut azar = Lcg(4242);
    for _ in 0..200_000 {
        let c = azar.siguiente(100_000_000_000_000) as i64;
        let c = if azar.siguiente(2) == 0 { -c } else { c };
        assert_eq!(Dinero::nuevo(c as f64 / 100.0, Divisa::Usd).unwrap().centavos(), c, "{c} centavos");
    }
}

#[test]
fn el_saldo_que_lee_el_adaptador_es_el_guardado_en_centavos() {
    let mut conn = base();
    let id = crear_cuenta(&conn, "Cuenta Lectura", "USD", 1_234_567.89);
    let tx = conn.transaction().unwrap();
    let almacen = AlmacenSqlite::nuevo(&tx);
    assert_eq!(almacen.saldo(id).unwrap(), usd_c(123_456_789));
}

// --- (a) Agregados: ni una sola consulta de producción suma en SQL ---

/// Los fuentes de producción: todo `.rs` bajo `src/` salvo los módulos que solo existen en pruebas, y cada archivo
/// cortado donde empieza su `mod` de pruebas (los `#[cfg(test)]` sueltos, como una constante, no cortan nada).
fn fuentes_de_produccion() -> Vec<(String, String)> {
    fn recorrer(dir: &std::path::Path, salida: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let ruta = e.unwrap().path();
            if ruta.is_dir() {
                recorrer(&ruta, salida);
            } else if ruta.extension().map_or(false, |x| x == "rs") {
                salida.push(ruta);
            }
        }
    }
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut rutas = Vec::new();
    recorrer(&raiz, &mut rutas);
    let solo_pruebas = ["caracterizacion.rs", "contrato_rusqlite.rs", "gherkin.rs", "caso_de_uso_gherkin.rs", "sumas_en_centavos.rs"];
    rutas.sort();
    rutas
        .into_iter()
        .filter(|r| !solo_pruebas.contains(&r.file_name().unwrap().to_str().unwrap()))
        .map(|r| {
            let texto = std::fs::read_to_string(&r).unwrap();
            let produccion = match texto.find("#[cfg(test)]\nmod ") {
                Some(i) => texto[..i].to_string(),
                None => texto,
            };
            (r.strip_prefix(&raiz).unwrap().display().to_string(), produccion)
        })
        .collect()
}

#[test]
fn ninguna_consulta_de_produccion_usa_sum_avg_ni_total_en_sql() {
    // Inventario A2: hoy hay CERO agregados SQL sobre REAL. Si alguien añade uno, esta prueba falla a propósito: ese
    // agregado debe nacer leyendo `ROUND(x * 100)` a entero y sumando en centavos, con su prueba de caracterización.
    let mut hallados = Vec::new();
    for (archivo, texto) in fuentes_de_produccion() {
        for (n, linea) in texto.lines().enumerate() {
            let sin_comentario = linea.trim_start();
            if sin_comentario.starts_with("//") || sin_comentario.starts_with('*') {
                continue;
            }
            let mayus = linea.to_uppercase();
            for patron in ["SUM(", "AVG(", "TOTAL("] {
                if mayus.contains(patron) {
                    hallados.push(format!("{archivo}:{}: {}", n + 1, linea.trim()));
                }
            }
        }
    }
    // `total(` también es un método de Rust (`Cargos::total`): solo cuenta lo que esté dentro de una cadena SQL.
    hallados.retain(|h| h.contains('"'));
    assert!(hallados.is_empty(), "agregados SQL nuevos sin caracterizar:\n{}", hallados.join("\n"));
}

#[test]
fn la_lista_de_fuentes_de_produccion_no_esta_vacia_y_cubre_los_adaptadores() {
    // Evita que la prueba anterior pase en vacío si el recorrido se rompe.
    let f = fuentes_de_produccion();
    assert!(f.len() > 40, "se esperaban decenas de archivos y hay {}", f.len());
    for esperado in ["db_sql.rs", "adaptadores/sqlite/gastos.rs", "comandos/gastos.rs", "dominio/dinero.rs"] {
        assert!(f.iter().any(|(a, _)| a == esperado), "falta {esperado}");
    }
}

#[test]
fn sum_de_sqlite_sobre_real_es_exacto_hoy_y_la_suma_ingenua_en_f64_no() {
    // No es código de producción: fija el motor empaquetado, sobre el que se apoyaría un SUM futuro. SQLite 3.43+
    // suma con compensación (Kahan–Babuska) y acierta; la suma `a + b` en f64 que hace la interfaz con `reduce`
    // (JS usa el mismo f64) no. Si una actualización de rusqlite cambiara esto, esta prueba lo diría.
    let conn = base();
    conn.execute("CREATE TEMP TABLE t (v REAL);", []).unwrap();
    for _ in 0..10 {
        conn.execute("INSERT INTO t VALUES (0.10);", []).unwrap();
    }
    let sql: f64 = conn.query_row("SELECT SUM(v) FROM t;", [], |r| r.get(0)).unwrap();
    assert_eq!(sql, 1.0, "SUM de SQLite: diez veces 0.10 es exactamente 1.0");
    let ingenua: f64 = (0..10).fold(0.0, |a, _| a + 0.10);
    assert_ne!(ingenua, 1.0, "la suma ingenua en f64 deriva");
    assert_eq!(ingenua, 0.9999999999999999);
}

#[test]
fn hallazgo_la_suma_ingenua_de_f64_se_aparta_de_la_exacta_con_muchos_importes() {
    // Lo que hace `reduce((s, g) => s + g.monto, 0)` en las vistas, reproducido con el mismo f64. Con 5 000 importes
    // inventados la deriva es de ~10^-4 centavos: invisible con dos decimales, pero rompe una igualdad exacta.
    let conn = base();
    conn.execute("CREATE TEMP TABLE t (v REAL);", []).unwrap();
    let mut azar = Lcg(12345);
    let mut exacto: i64 = 0;
    for _ in 0..5_000 {
        let c = azar.siguiente(123_456_789) as i64;
        exacto += c;
        conn.execute("INSERT INTO t VALUES (?);", [c as f64 / 100.0]).unwrap();
    }
    let valores: Vec<f64> = conn
        .prepare("SELECT v FROM t;")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    let ingenua: f64 = valores.iter().fold(0.0, |a, b| a + b);
    let en_sql: f64 = conn.query_row("SELECT SUM(v) FROM t;", [], |r| r.get(0)).unwrap();
    let en_centavos_sql: i64 = conn.query_row("SELECT SUM(CAST(ROUND(v * 100) AS INTEGER)) FROM t;", [], |r| r.get(0)).unwrap();

    assert_eq!(exacto, 307_370_575_083);
    assert_eq!(en_centavos_sql, exacto, "el patrón seguro: leer a entero y sumar enteros es exacto");
    assert_eq!((en_sql * 100.0).round() as i64, exacto, "SUM de SQLite también cuadra al centavo");
    assert_ne!(ingenua * 100.0, exacto as f64, "la suma ingenua NO da exactamente los centavos");
    assert_eq!((ingenua * 100.0).round() as i64, exacto, "aunque redondeada al centavo sí coincide");
}

// --- (b) Redondeo ---

#[test]
fn hallazgo_round_de_sql_decide_la_mitad_por_el_binario_y_dinero_por_los_digitos() {
    // `ROUND(x, 2)` de SQLite (usado al acumular y en las migraciones) redondea el VALOR BINARIO: 2.675 y 1.005 bajan.
    // `Dinero::desde_texto` decide por los dígitos y sube (mitad alejándose de cero). En la acumulación en marcha no
    // se manifiesta —se suman centavos exactos y no hay mitades—, solo en migraciones que redondean un producto
    // (`monto_pagado * tasa`, el saldo prorrateado de un préstamo). Fijado, sin corregir.
    let conn = base();
    let sql = |expr: &str| -> f64 { conn.query_row(&format!("SELECT {expr};"), [], |r| r.get(0)).unwrap() };
    assert_eq!(sql("ROUND(2.675, 2)"), 2.67);
    assert_eq!(sql("ROUND(1.005, 2)"), 1.0);
    assert_eq!(sql("ROUND(-1.005, 2)"), -1.0);
    assert_eq!(sql("ROUND(0.285, 2)"), 0.28);
    assert_eq!(sql("ROUND(1.015, 2)"), 1.01);
    assert_eq!(sql("ROUND(0.125, 2)"), 0.13, "una mitad exacta en binario sí se aleja del cero");

    assert_eq!(centavos_desde_texto("2.675").unwrap(), 268);
    assert_eq!(centavos_desde_texto("1.005").unwrap(), 101);
    assert_eq!(centavos_desde_texto("-1.005").unwrap(), -101);
    assert_eq!(centavos_desde_texto("0.285").unwrap(), 29);
    assert_eq!(centavos_desde_texto("1.015").unwrap(), 102);
    assert_eq!(centavos_desde_texto("0.125").unwrap(), 13);
}

#[test]
fn round_de_sql_sin_decimales_aleja_la_mitad_del_cero() {
    // Es el redondeo que usa `ROUND(x * 100) = 0`.
    let conn = base();
    let sql = |expr: &str| -> f64 { conn.query_row(&format!("SELECT {expr};"), [], |r| r.get(0)).unwrap() };
    assert_eq!(sql("ROUND(0.5)"), 1.0);
    assert_eq!(sql("ROUND(-0.5)"), -1.0);
    assert_eq!(sql("ROUND(2.5)"), 3.0);
    assert_eq!(sql("ROUND(0.49999999999999994)"), 1.0, "SQLite redondea por decimales impresos: el valor justo bajo 0.5 ya sube");
}

#[test]
fn hallazgo_dinero_nuevo_por_f64_decide_distinto_que_por_texto_en_las_mitades() {
    // Entre 0.005 y 3999.995 (cada «n.nn5») cuántas mitades baja `Dinero::nuevo(f64)` frente a `desde_texto`. En
    // producción los importes entran por texto y el REAL leído ya es un centavo exacto (sin mitades), así que no
    // llega a verse; el número fija la magnitud del riesgo si algún camino llamara a `nuevo` con una mitad.
    let mut distintas = 0;
    let mut total = 0;
    for medios in 0..400_000_i64 {
        let texto = format!("{}.{:02}5", medios / 100, medios % 100);
        let por_texto = centavos_desde_texto(&texto).unwrap();
        let por_f64 = Dinero::nuevo(texto.parse::<f64>().unwrap(), Divisa::Dop).unwrap().centavos();
        total += 1;
        if por_texto != por_f64 {
            assert_eq!(por_f64, por_texto - 1, "{texto}: la diferencia siempre es bajar un centavo");
            distintas += 1;
        }
    }
    assert_eq!(total, 400_000);
    assert_eq!(distintas, 18_349, "mitades que f64 baja y el texto sube");
}

#[test]
fn dinero_sin_fraccion_de_centavo_no_se_ve_afectado_por_la_tolerancia_de_la_conversion() {
    // `Dinero::nuevo` NO tiene tolerancia: es `(x * 100).round()`. (`TOLERANCIA_REPRESENTACION_CENTAVOS` es una
    // constante solo de pruebas, que mide la escala de las tasas.) Un REAL con deriva de 1e-9 se absorbe en el
    // centavo vecino; uno a medio centavo no.
    assert_eq!(Dinero::nuevo(0.1 + 0.2, Divisa::Dop).unwrap().centavos(), 30);
    assert_eq!(Dinero::nuevo(0.1 + 0.2 - 0.3, Divisa::Dop).unwrap().centavos(), 0);
    assert_eq!(Dinero::nuevo(0.300_000_001, Divisa::Dop).unwrap().centavos(), 30);
    assert_eq!(Dinero::nuevo(0.304, Divisa::Dop).unwrap().centavos(), 30);
    assert_eq!(Dinero::nuevo(0.306, Divisa::Dop).unwrap().centavos(), 31);
}

// --- (c) Comparaciones de igualdad y de orden ---

#[test]
fn la_literal_de_f64_0_1_mas_0_2_menos_0_3_no_es_cero_y_los_centavos_si() {
    // Por qué existe `ROUND(x * 100) = 0` en lugar de `= 0`.
    assert_ne!(0.1_f64 + 0.2 - 0.3, 0.0);
    assert_eq!(0.1_f64 + 0.2 - 0.3, 5.551115123125783e-17);
    assert!(dop_c(10).sumar(&dop_c(20)).unwrap().restar(&dop_c(30)).unwrap().es_cero());
}

#[test]
fn una_cuenta_con_0_1_mas_0_2_menos_0_3_queda_en_cero_exacto_y_se_archiva() {
    let mut conn = base();
    let id = crear_cuenta(&conn, "Cuenta Casi Cero", "DOP", 0.0);
    let tx = conn.transaction().unwrap();
    let archivada = {
        let mut almacen = AlmacenSqlite::nuevo(&tx);
        almacen.ajustar_saldo(id, dop_c(10)).unwrap();
        almacen.ajustar_saldo(id, dop_c(20)).unwrap();
        almacen.ajustar_saldo(id, dop_c(-30)).unwrap();
        almacen.archivar_cuenta(id).unwrap()
    };
    tx.commit().unwrap();
    assert_eq!(balance_cuenta(&conn, id), 0.0, "el REAL guardado es el cero exacto");
    assert!(archivada);
}

#[test]
fn una_tarjeta_con_0_1_mas_0_2_menos_0_3_en_ambas_divisas_se_desactiva() {
    let mut conn = base();
    let id = crear_tarjeta(&conn, "Tarjeta Casi Cero", 0.0, 0.0);
    let tx = conn.transaction().unwrap();
    let desactivada = {
        let mut almacen = AlmacenSqlite::nuevo(&tx);
        for divisa_c in [dop_c as fn(i64) -> Dinero, usd_c] {
            almacen.ajustar_deuda(id, divisa_c(10)).unwrap();
            almacen.ajustar_deuda(id, divisa_c(20)).unwrap();
            almacen.ajustar_deuda(id, divisa_c(-30)).unwrap();
        }
        almacen.desactivar_tarjeta(id).unwrap()
    };
    tx.commit().unwrap();
    assert_eq!(balance_tarjeta(&conn, id, "balance_pesos"), 0.0);
    assert_eq!(balance_tarjeta(&conn, id, "balance_dolares"), 0.0);
    assert!(desactivada);
}

#[test]
fn con_un_centavo_de_saldo_ni_la_cuenta_se_archiva_ni_la_tarjeta_se_desactiva() {
    let mut conn = base();
    let cuenta = crear_cuenta(&conn, "Cuenta Un Centavo", "DOP", 0.01);
    let a_favor = crear_cuenta(&conn, "Cuenta A Favor", "DOP", -0.01);
    let tarjeta_pesos = crear_tarjeta(&conn, "Tarjeta Un Centavo P", 0.01, 0.0);
    let tarjeta_usd = crear_tarjeta(&conn, "Tarjeta Un Centavo U", 0.0, -0.01);
    let tx = conn.transaction().unwrap();
    let mut almacen = AlmacenSqlite::nuevo(&tx);
    assert!(!almacen.archivar_cuenta(cuenta).unwrap());
    assert!(!almacen.archivar_cuenta(a_favor).unwrap(), "el saldo a favor de un centavo tampoco es cero");
    assert!(!almacen.desactivar_tarjeta(tarjeta_pesos).unwrap());
    assert!(!almacen.desactivar_tarjeta(tarjeta_usd).unwrap(), "con una sola divisa en deuda no se desactiva");
}

#[test]
fn la_base_rechaza_al_escribir_un_real_con_deriva_aunque_sea_de_1e_17() {
    // Red de seguridad del esquema (migración 11): `CHECK (ROUND(v, 2) = v)`. Por eso las comparaciones de abajo
    // solo son alcanzables con la restricción apagada.
    let conn = base();
    let id = crear_cuenta(&conn, "Cuenta Check", "DOP", 0.0);
    for sucio in [0.1_f64 + 0.2, 0.1 + 0.2 - 0.3, 0.004, 1.005, 0.30000000000000004] {
        let r = conn.execute("UPDATE cuentas_ahorro SET balance_actual = ? WHERE id = ?;", params![sucio, id]);
        let mensaje = r.unwrap_err().to_string();
        assert!(mensaje.contains("CHECK constraint failed"), "{sucio}: {mensaje}");
    }
    for limpio in [0.3_f64, 0.0, -0.01, 1234567.89] {
        conn.execute("UPDATE cuentas_ahorro SET balance_actual = ? WHERE id = ?;", params![limpio, id]).unwrap();
    }
}

/// `true` si `archivar_cuenta` acepta una cuenta cuyo REAL guardado vale `valor` (con el CHECK apagado a propósito
/// para poder representar la deriva que el esquema normalmente impide).
fn se_archiva_con(valor: f64) -> bool {
    let mut conn = base();
    conn.execute("PRAGMA ignore_check_constraints = ON;", []).unwrap();
    let id = crear_cuenta(&conn, "Cuenta Umbral", "DOP", valor);
    conn.execute("PRAGMA ignore_check_constraints = OFF;", []).unwrap();
    let tx = conn.transaction().unwrap();
    let mut almacen = AlmacenSqlite::nuevo(&tx);
    almacen.archivar_cuenta(id).unwrap()
}

fn se_desactiva_con(pesos: f64, dolares: f64) -> bool {
    let mut conn = base();
    conn.execute("PRAGMA ignore_check_constraints = ON;", []).unwrap();
    let id = crear_tarjeta(&conn, "Tarjeta Umbral", pesos, dolares);
    conn.execute("PRAGMA ignore_check_constraints = OFF;", []).unwrap();
    let tx = conn.transaction().unwrap();
    let mut almacen = AlmacenSqlite::nuevo(&tx);
    almacen.desactivar_tarjeta(id).unwrap()
}

#[test]
fn archivar_trata_como_cero_la_deriva_de_representacion() {
    assert!(se_archiva_con(0.0));
    assert!(se_archiva_con(-0.0));
    assert!(se_archiva_con(0.1 + 0.2 - 0.3), "5.55e-17");
    assert!(se_archiva_con(0.30000000000000004 - 0.3), "5.55e-17 por la otra vía");
    assert!(se_archiva_con(1e-12));
    assert!(se_archiva_con(-1e-12));
}

#[test]
fn hallazgo_archivar_toma_por_cero_hasta_medio_centavo_exclusive() {
    // El umbral real de `ROUND(x * 100) = 0` es MEDIO CENTAVO, no solo la deriva de representación: 0.004 y
    // -0.004 se archivan; 0.005 y 0.01 no. Inalcanzable con el CHECK activo (un REAL con fracción de centavo no
    // se puede guardar); se deja fijado por si alguna vez se relaja la restricción o entra un respaldo restaurado.
    assert!(se_archiva_con(0.004));
    assert!(se_archiva_con(-0.004));
    assert!(se_archiva_con(0.0049));
    assert!(!se_archiva_con(0.005), "medio centavo exacto ya no es cero (ROUND lo aleja del cero)");
    assert!(!se_archiva_con(-0.005));
    assert!(!se_archiva_con(0.01));
    assert!(!se_archiva_con(-0.01));
    assert!(!se_archiva_con(1e-3 + 0.01));
}

#[test]
fn desactivar_tarjeta_exige_cero_en_las_dos_divisas_con_la_misma_regla_de_medio_centavo() {
    assert!(se_desactiva_con(0.0, 0.0));
    assert!(se_desactiva_con(0.1 + 0.2 - 0.3, 0.1 + 0.2 - 0.3));
    assert!(se_desactiva_con(0.004, -0.004), "hallazgo: bajo medio centavo cuenta como cero");
    assert!(!se_desactiva_con(0.005, 0.0));
    assert!(!se_desactiva_con(0.0, 0.01));
    assert!(!se_desactiva_con(0.0, -0.01));
}

#[test]
fn hallazgo_ajustar_saldo_y_ajustar_deuda_reabsorben_la_deriva_ya_guardada() {
    // Un REAL con deriva (solo posible con el CHECK apagado: restauración de un respaldo antiguo, por ejemplo) no
    // se propaga: `ajustar_saldo` lo lee como Dinero (centavos, `.round()`) y escribe la suma exacta; `ajustar_deuda`
    // lo corrige con `ROUND(.., 2)`. El primer movimiento «limpia» silenciosamente hasta casi medio centavo.
    let mut conn = base();
    conn.execute("PRAGMA ignore_check_constraints = ON;", []).unwrap();
    let cuenta = crear_cuenta(&conn, "Cuenta Sucia", "DOP", 0.30000000000000004);
    let tarjeta = crear_tarjeta(&conn, "Tarjeta Sucia", 0.30000000000000004, 0.304);
    conn.execute("PRAGMA ignore_check_constraints = OFF;", []).unwrap();
    let tx = conn.transaction().unwrap();
    {
        let mut almacen = AlmacenSqlite::nuevo(&tx);
        almacen.ajustar_saldo(cuenta, dop_c(1)).unwrap();
        almacen.ajustar_deuda(tarjeta, dop_c(1)).unwrap();
        almacen.ajustar_deuda(tarjeta, usd_c(1)).unwrap();
    }
    tx.commit().unwrap();
    assert_eq!(balance_cuenta(&conn, cuenta), 0.31);
    assert_eq!(balance_tarjeta(&conn, tarjeta, "balance_pesos"), 0.31);
    assert_eq!(balance_tarjeta(&conn, tarjeta, "balance_dolares"), 0.31, "0.304 + 0.01 = 0.314 -> 0.31");
}

#[test]
fn el_orden_entre_saldos_se_decide_en_centavos_y_no_por_f64() {
    // `Dinero` no implementa `Ord` a propósito; la comparación de dos saldos se hace sobre centavos enteros y es
    // exacta donde f64 se equivoca: 0.1 + 0.2 y 0.3 son el mismo importe.
    assert_ne!(0.1_f64 + 0.2, 0.3);
    assert!(0.1_f64 + 0.2 > 0.3, "por f64 el orden sale al revés");
    let a = dop_c(10).sumar(&dop_c(20)).unwrap();
    assert_eq!(a, dop_c(30));
    assert_eq!(a.centavos().cmp(&30), std::cmp::Ordering::Equal);
    // Y tras el viaje por la columna, igual.
    let mut conn = base();
    let id = crear_cuenta(&conn, "Cuenta Orden", "DOP", 0.0);
    let tx = conn.transaction().unwrap();
    let mut almacen = AlmacenSqlite::nuevo(&tx);
    almacen.ajustar_saldo(id, dop_c(10)).unwrap();
    almacen.ajustar_saldo(id, dop_c(20)).unwrap();
    assert_eq!(almacen.saldo(id).unwrap(), dop_c(30));
    assert_eq!(balance_cuenta(&tx, id), 0.3);
}
