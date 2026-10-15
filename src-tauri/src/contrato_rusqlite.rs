//! Caracterización del contrato con rusqlite/SQLite que la aplicación da por hecho.
//!
//! Se escribió al subir de rusqlite 0.29 a 0.40: cada prueba fija un comportamiento que esa subida podía cambiar,
//! para que la próxima subida lo detecte. No hay código de producción aquí.

use crate::db_sql::{transaccion, ESPERA_OCUPADA_MS};
use rusqlite::{ffi, Connection};

#[test]
fn el_sqlite_empaquetado_es_al_menos_el_de_la_subida() {
    // 3.41.2 era el de rusqlite 0.29; un retroceso sería una bajada de dependencia no intencionada.
    assert!(rusqlite::version_number() >= 3_041_002, "SQLite empaquetada: {}", rusqlite::version());
}

#[test]
fn execute_con_varias_sentencias_es_un_error_y_execute_batch_si_las_ejecuta() {
    // Desde rusqlite 0.35 `execute` falla si queda texto tras la primera sentencia. El código de la app usa
    // `execute_batch` para lo que lleva varias; esto fija ambas mitades.
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch("CREATE TABLE t (v INTEGER);").unwrap();
    assert!(c.execute("INSERT INTO t VALUES (1); INSERT INTO t VALUES (2);", []).is_err());
    assert!(c.prepare("INSERT INTO t VALUES (1); INSERT INTO t VALUES (2);").is_err());
    let n: i64 = c.query_row("SELECT COUNT(*) FROM t;", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0, "la que falla no ejecuta ni la primera");
    c.execute_batch("INSERT INTO t VALUES (1); INSERT INTO t VALUES (2);").unwrap();
    let n: i64 = c.query_row("SELECT COUNT(*) FROM t;", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 2);
}

#[test]
fn el_codigo_de_error_de_ocupada_es_busy_y_su_mensaje_dice_locked() {
    // Desde 0.32 rusqlite abre con códigos extendidos: el primario sigue siendo comparable con `code`.
    let ruta = std::env::temp_dir().join(format!("michelitos-busy-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&ruta);
    let mut a = Connection::open(&ruta).unwrap();
    a.execute_batch("CREATE TABLE t (v INTEGER);").unwrap();
    let tx = transaccion(&mut a, true).unwrap();
    tx.execute("INSERT INTO t VALUES (1);", []).unwrap();
    let b = Connection::open(&ruta).unwrap();
    b.busy_timeout(std::time::Duration::from_millis(0)).unwrap();
    let e = b.execute("INSERT INTO t VALUES (2);", []).expect_err("otro escritor debe fallar");
    match &e {
        rusqlite::Error::SqliteFailure(f, _) => {
            assert_eq!(f.code, rusqlite::ErrorCode::DatabaseBusy, "{e}");
            assert_eq!(f.extended_code & 0xff, ffi::SQLITE_BUSY, "{e}");
        }
        otro => panic!("error inesperado: {otro:?}"),
    }
    assert!(e.to_string().contains("locked"), "{e}");
    drop(tx);
    let _ = std::fs::remove_file(&ruta);
}

#[test]
fn las_violaciones_de_restriccion_conservan_codigo_primario_y_mensaje() {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY, n TEXT UNIQUE, v REAL CHECK (v >= 0));").unwrap();
    c.execute("INSERT INTO t (n, v) VALUES ('a', 1.5);", []).unwrap();
    for (sql, texto, extendido) in [
        ("INSERT INTO t (n, v) VALUES ('a', 1.0);", "UNIQUE constraint failed", ffi::SQLITE_CONSTRAINT_UNIQUE),
        ("INSERT INTO t (n, v) VALUES ('b', -1.0);", "CHECK constraint failed", ffi::SQLITE_CONSTRAINT_CHECK),
    ] {
        let e = c.execute(sql, []).expect_err(sql);
        assert!(e.to_string().contains(texto), "{e}");
        match &e {
            rusqlite::Error::SqliteFailure(f, _) => {
                assert_eq!(f.code, rusqlite::ErrorCode::ConstraintViolation);
                assert_eq!(f.extended_code, extendido);
            }
            otro => panic!("{otro:?}"),
        }
    }
}

#[test]
fn un_disparador_con_raise_abort_llega_como_error_con_su_texto() {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch(
        "CREATE TABLE t (id INTEGER);
         CREATE TRIGGER no_borrar BEFORE DELETE ON t BEGIN SELECT RAISE(ABORT, 'No se borra.'); END;
         INSERT INTO t VALUES (1);",
    )
    .unwrap();
    let e = c.execute("DELETE FROM t;", []).expect_err("el disparador aborta");
    assert!(e.to_string().contains("No se borra."), "{e}");
}

#[test]
fn enteros_sin_signo_se_enlazan_como_i64_explicito() {
    // Desde 0.38 u64/usize no implementan ToSql por defecto: la app (que compila sin ellos) convierte a i64.
    let c = Connection::open_in_memory().unwrap();
    let n: i64 = c.query_row("SELECT ?;", [ESPERA_OCUPADA_MS as i64], |r| r.get(0)).unwrap();
    assert_eq!(n as u64, ESPERA_OCUPADA_MS);
}
