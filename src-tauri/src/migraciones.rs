//! Ejecutor de migraciones versionadas.
//!
//! Antes, la preparación del esquema aplicaba sus cambios con
//! `let _ = conn.execute(...)`. El patrón nació para tolerar **una** condición
//! esperada —la columna ya existe— pero descartaba **cualquier** error, y eso
//! resultó tener consecuencias reales: sobre una base cuyo esquema no admitía
//! la migración, la preparación devolvía `Ok` y la aplicación arrancaba contra
//! un esquema al que le faltaban columnas. El fallo aparecía después, lejos de
//! su causa, en la primera consulta que tocara una de ellas.
//!
//! Aquí eso se corrige por dos vías que se necesitan mutuamente.
//!
//! **La condición esperada se comprueba, en vez de deducirse de un error.**
//! `anadir_columna` mira si la columna está antes de intentar añadirla. Si no
//! está y el `ALTER` falla, el fallo es real y se propaga con el nombre de la
//! migración y la etapa, sin valores de registros ni rutas personales.
//!
//! **La base lleva su versión.** Sin ella no hay forma de distinguir «esto ya
//! se aplicó» de «esto no se pudo aplicar», que es exactamente la ambigüedad
//! que hacía razonable descartar el error. Se guarda en `PRAGMA user_version`,
//! que viaja dentro del archivo y no necesita tabla propia.
//!
//! Cada migración corre en **su** transacción, junto con el sello de versión:
//! o se aplica entera y queda registrada, o no ocurre nada. Una versión
//! confirmada es un punto válido de reanudación; un fallo a mitad deja la base
//! en la última versión completa, no en un estado intermedio sin nombre.

use rusqlite::{Connection, Transaction};
use std::fmt;

/// Versión de esquema que esta compilación sabe manejar.
pub const VERSION_OBJETIVO: u32 = 3;

#[derive(Debug, PartialEq)]
pub enum ErrorMigracion {
    /// La base viene de una versión más nueva de la aplicación.
    BaseMasNueva { encontrada: u32, soportada: u32 },
    /// Una migración falló. No se aplicó ninguna parte de ella.
    Fallo { version: u32, migracion: &'static str, etapa: String, causa: String },
    /// La estructura no es la que una migración esperaba encontrar.
    EstructuraInesperada { migracion: &'static str, detalle: String },
    Almacenamiento(String),
}

impl fmt::Display for ErrorMigracion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorMigracion::BaseMasNueva { encontrada, soportada } => write!(
                f,
                "La base está en la versión de esquema {} y esta versión de la aplicación solo maneja hasta la {}. Actualiza la aplicación; abrirla así podría dañar los datos.",
                encontrada, soportada
            ),
            ErrorMigracion::Fallo { version, migracion, etapa, causa } => write!(
                f,
                "La migración {} «{}» falló en la etapa «{}»: {}. No se aplicó ninguna parte de ella y la base sigue en la versión anterior.",
                version, migracion, etapa, causa
            ),
            ErrorMigracion::EstructuraInesperada { migracion, detalle } => write!(
                f,
                "La migración «{}» encontró una estructura que no reconoce: {}. Se detuvo sin modificar la base.",
                migracion, detalle
            ),
            ErrorMigracion::Almacenamiento(detalle) => {
                write!(f, "Error de almacenamiento durante la migración: {}", detalle)
            }
        }
    }
}

impl std::error::Error for ErrorMigracion {}

impl From<rusqlite::Error> for ErrorMigracion {
    fn from(e: rusqlite::Error) -> Self {
        ErrorMigracion::Almacenamiento(e.to_string())
    }
}

/// Qué hizo el ejecutor. Sirve para poder afirmarlo en las pruebas y para que
/// una segunda ejecución sin cambios sea comprobable, no una suposición.
#[derive(Debug, PartialEq)]
pub struct Informe {
    pub version_inicial: u32,
    pub version_final: u32,
    pub aplicadas: Vec<&'static str>,
}

type Paso = fn(&Transaction) -> Result<(), ErrorMigracion>;

struct Migracion {
    version: u32,
    nombre: &'static str,
    aplicar: Paso,
}

fn catalogo() -> Vec<Migracion> {
    vec![
        Migracion {
            version: 1,
            nombre: "estructura",
            aplicar: crate::db_sql::migracion_1_estructura,
        },
        Migracion {
            version: 2,
            nombre: "transformaciones históricas",
            aplicar: crate::db_sql::migracion_2_transformaciones,
        },
        Migracion {
            version: 3,
            nombre: "importes en centavos exactos",
            aplicar: crate::db_sql::migracion_3_centavos_exactos,
        },
    ]
}

pub fn version_de(conn: &Connection) -> Result<u32, ErrorMigracion> {
    conn.query_row("PRAGMA user_version;", [], |r| r.get::<_, i64>(0))
        .map(|v| v.max(0) as u32)
        .map_err(|e| ErrorMigracion::Almacenamiento(e.to_string()))
}

/// Aplica las migraciones pendientes y devuelve qué se hizo.
///
/// Una base con versión superior a la soportada **se rechaza sin tocarla**:
/// abrirla con una aplicación que no conoce su esquema es la vía más rápida a
/// una corrupción silenciosa.
pub fn ejecutar(conn: &mut Connection) -> Result<Informe, ErrorMigracion> {
    let version_inicial = version_de(conn)?;

    if version_inicial > VERSION_OBJETIVO {
        return Err(ErrorMigracion::BaseMasNueva {
            encontrada: version_inicial,
            soportada: VERSION_OBJETIVO,
        });
    }

    let mut aplicadas = Vec::new();

    for migracion in catalogo() {
        if migracion.version <= version_inicial {
            continue;
        }

        let tx = conn
            .transaction()
            .map_err(|e| ErrorMigracion::Almacenamiento(e.to_string()))?;

        (migracion.aplicar)(&tx).map_err(|e| match e {
            // Se conserva el error de estructura tal cual: ya explica el
            // problema mejor que un envoltorio genérico.
            ErrorMigracion::EstructuraInesperada { .. } => e,
            otro => ErrorMigracion::Fallo {
                version: migracion.version,
                migracion: migracion.nombre,
                etapa: "aplicación".into(),
                causa: otro.to_string(),
            },
        })?;

        // El sello va dentro de la misma transacción que los cambios. Fuera de
        // ella existiría un instante en que la base está migrada y no lo dice,
        // o lo dice sin estarlo.
        tx.execute_batch(&format!("PRAGMA user_version = {};", migracion.version))
            .map_err(|e| ErrorMigracion::Fallo {
                version: migracion.version,
                migracion: migracion.nombre,
                etapa: "sello de versión".into(),
                causa: e.to_string(),
            })?;

        tx.commit().map_err(|e| ErrorMigracion::Fallo {
            version: migracion.version,
            migracion: migracion.nombre,
            etapa: "confirmación".into(),
            causa: e.to_string(),
        })?;

        aplicadas.push(migracion.nombre);
    }

    Ok(Informe { version_inicial, version_final: version_de(conn)?, aplicadas })
}

// --- Utilidades para escribir migraciones ---

/// Añade una columna si no está, y **falla si no puede añadirla**.
///
/// Sustituye al `let _ = ALTER TABLE ...` que descartaba cualquier error. La
/// diferencia está en separar las dos cosas que ese patrón confundía: que la
/// columna ya exista es una condición esperada que se comprueba de antemano;
/// que el `ALTER` falle teniendo que funcionar es un fallo que se propaga.
///
/// Devuelve si hubo que añadirla, para que una migración pueda decidir en
/// consecuencia sin volver a consultar el esquema.
pub fn anadir_columna(
    tx: &Transaction,
    migracion: &'static str,
    tabla: &str,
    columna: &str,
    definicion: &str,
) -> Result<bool, ErrorMigracion> {
    exigir_tabla(tx, migracion, tabla)?;

    if columna_existe(tx, tabla, columna)? {
        return Ok(false);
    }

    tx.execute(&format!("ALTER TABLE {} ADD COLUMN {} {};", tabla, columna, definicion), [])
        .map_err(|e| ErrorMigracion::Fallo {
            version: 0,
            migracion,
            etapa: format!("añadir {}.{}", tabla, columna),
            causa: e.to_string(),
        })?;
    Ok(true)
}

/// Comprueba que el nombre corresponde a una **tabla**, no a otra cosa.
///
/// `CREATE TABLE IF NOT EXISTS` no falla si ya existe una vista con ese
/// nombre: simplemente no hace nada. Sin esta comprobación, el `ALTER`
/// posterior fallaría con «no se puede añadir una columna a una vista», que es
/// un mensaje que no dice cómo llegó la base a ese estado.
pub fn exigir_tabla(
    tx: &Transaction,
    migracion: &'static str,
    tabla: &str,
) -> Result<(), ErrorMigracion> {
    let tipo: Option<String> = tx
        .query_row(
            "SELECT type FROM sqlite_master WHERE name = ?;",
            [tabla],
            |r| r.get(0),
        )
        .ok();

    match tipo.as_deref() {
        Some("table") => Ok(()),
        Some(otro) => Err(ErrorMigracion::EstructuraInesperada {
            migracion,
            detalle: format!("se esperaba que «{}» fuera una tabla y es {}", tabla, otro),
        }),
        None => Err(ErrorMigracion::EstructuraInesperada {
            migracion,
            detalle: format!("falta la tabla «{}»", tabla),
        }),
    }
}

fn columna_existe(tx: &Transaction, tabla: &str, columna: &str) -> Result<bool, ErrorMigracion> {
    let cuenta: i64 = tx
        .query_row(
            &format!("SELECT COUNT(*) FROM pragma_table_info('{}') WHERE name = ?;", tabla),
            [columna],
            |r| r.get(0),
        )
        .map_err(|e| ErrorMigracion::Almacenamiento(e.to_string()))?;
    Ok(cuenta > 0)
}

/// Ejecuta una sentencia propagando el error con su etapa.
pub fn paso(
    tx: &Transaction,
    migracion: &'static str,
    etapa: &str,
    sql: &str,
) -> Result<(), ErrorMigracion> {
    tx.execute_batch(sql).map_err(|e| ErrorMigracion::Fallo {
        version: 0,
        migracion,
        etapa: etapa.to_string(),
        causa: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deja_constancia_de_la_version_de_sqlite_empaquetada() {
        // DROP COLUMN necesita 3.35 y RENAME COLUMN 3.25. La versión que
        // importa es la que rusqlite compila dentro del binario, no la del
        // sqlite3 del sistema.
        println!("SQLite empaquetada: {}", rusqlite::version());
        assert!(rusqlite::version_number() >= 3_035_000, "hace falta 3.35 o superior");
    }

    fn base_en_memoria() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn una_base_vacia_llega_a_la_version_objetivo() {
        let mut c = base_en_memoria();

        let informe = ejecutar(&mut c).expect("migrar base vacía");

        assert_eq!(informe.version_inicial, 0);
        assert_eq!(informe.version_final, VERSION_OBJETIVO);
        assert_eq!(informe.aplicadas.len(), VERSION_OBJETIVO as usize, "se aplicaron todas");
    }

    #[test]
    fn una_segunda_ejecucion_no_repite_nada() {
        let mut c = base_en_memoria();
        ejecutar(&mut c).unwrap();

        let informe = ejecutar(&mut c).expect("segunda pasada");

        assert_eq!(informe.version_inicial, VERSION_OBJETIVO);
        assert_eq!(informe.version_final, VERSION_OBJETIVO);
        assert!(informe.aplicadas.is_empty(), "nada que aplicar");
    }

    #[test]
    fn una_base_mas_nueva_que_la_aplicacion_se_rechaza_sin_tocarla() {
        let mut c = base_en_memoria();
        c.execute_batch(&format!("PRAGMA user_version = {};", VERSION_OBJETIVO + 1)).unwrap();

        let error = ejecutar(&mut c).unwrap_err();

        assert_eq!(
            error,
            ErrorMigracion::BaseMasNueva {
                encontrada: VERSION_OBJETIVO + 1,
                soportada: VERSION_OBJETIVO
            }
        );
        // Y la base no se tocó: sigue sin ninguna tabla del esquema.
        let tablas: i64 = c
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table';", [], |r| r.get(0))
            .unwrap();
        assert_eq!(tablas, 0);
    }

    #[test]
    fn el_mensaje_de_base_mas_nueva_dice_qué_hacer() {
        let error = ErrorMigracion::BaseMasNueva { encontrada: 9, soportada: 2 }.to_string();

        assert!(error.contains("Actualiza la aplicación"), "orienta: {error}");
    }

    #[test]
    fn una_estructura_que_no_es_tabla_detiene_la_migracion() {
        // El caso que hacía que la preparación devolviera Ok sobre un esquema
        // inservible: `tarjetas` existe, pero como vista.
        let mut c = base_en_memoria();
        c.execute_batch(
            "CREATE TABLE origen (id INTEGER PRIMARY KEY, entidad TEXT);
             CREATE VIEW tarjetas AS SELECT id, entidad FROM origen;",
        )
        .unwrap();

        let error = ejecutar(&mut c).unwrap_err();

        match error {
            ErrorMigracion::EstructuraInesperada { detalle, .. } => {
                assert!(detalle.contains("tarjetas"), "nombra la estructura: {detalle}");
                assert!(detalle.contains("view"), "y dice qué es: {detalle}");
            }
            otro => panic!("se esperaba una estructura inesperada, no {otro:?}"),
        }
    }

    #[test]
    fn un_fallo_deja_la_version_anterior_y_ninguna_parte_aplicada() {
        // Misma base con la vista: la migración 1 no puede completarse, así que
        // la versión no avanza y no queda un estado intermedio sin nombre.
        let mut c = base_en_memoria();
        c.execute_batch(
            "CREATE TABLE origen (id INTEGER PRIMARY KEY, entidad TEXT);
             CREATE VIEW tarjetas AS SELECT id, entidad FROM origen;",
        )
        .unwrap();

        assert!(ejecutar(&mut c).is_err());

        assert_eq!(version_de(&c).unwrap(), 0, "la versión no avanzó");
        let categorias: i64 = c
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE name='categorias';", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(categorias, 0, "ni siquiera lo que la migración alcanzó a crear");
    }

    #[test]
    fn anadir_una_columna_que_ya_esta_no_es_un_error_ni_la_duplica() {
        let mut c = base_en_memoria();
        c.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY);").unwrap();
        let tx = c.transaction().unwrap();

        assert!(anadir_columna(&tx, "prueba", "t", "extra", "TEXT").unwrap(), "la primera añade");
        assert!(
            !anadir_columna(&tx, "prueba", "t", "extra", "TEXT").unwrap(),
            "la segunda no hace nada, y tampoco falla"
        );
    }

    #[test]
    fn anadir_una_columna_a_una_tabla_que_no_existe_es_un_error_visible() {
        // Antes esto se descartaba y la preparación seguía como si nada.
        let mut c = base_en_memoria();
        let tx = c.transaction().unwrap();

        let error = anadir_columna(&tx, "prueba", "inexistente", "x", "TEXT").unwrap_err();

        match error {
            ErrorMigracion::EstructuraInesperada { detalle, .. } => {
                assert!(detalle.contains("falta la tabla"), "{detalle}")
            }
            otro => panic!("se esperaba estructura inesperada, no {otro:?}"),
        }
    }

    #[test]
    fn un_alter_invalido_se_propaga_con_su_etapa() {
        let mut c = base_en_memoria();
        c.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY);").unwrap();
        let tx = c.transaction().unwrap();

        // SQLite no permite añadir una columna UNIQUE a una tabla existente.
        let error = anadir_columna(&tx, "prueba", "t", "codigo", "TEXT UNIQUE").unwrap_err();

        match error {
            ErrorMigracion::Fallo { etapa, migracion, .. } => {
                assert_eq!(migracion, "prueba");
                assert!(etapa.contains("t.codigo"), "la etapa localiza el fallo: {etapa}");
            }
            otro => panic!("se esperaba un fallo, no {otro:?}"),
        }
    }
}

#[cfg(test)]
mod tests_centavos {
    use super::*;
    use crate::db_sql::COLUMNAS_DE_DINERO;

    fn base_migrada() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        ejecutar(&mut c).unwrap();
        c
    }

    fn fuera_de_centavo(c: &Connection, tabla: &str, columna: &str) -> i64 {
        c.query_row(
            &format!(
                "SELECT COUNT(*) FROM {t} WHERE {c} IS NOT NULL
                 AND ABS({c} * 100 - ROUND({c} * 100)) > 1e-6;",
                t = tabla,
                c = columna
            ),
            [],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn un_importe_con_fraccion_de_centavo_queda_en_el_centavo_mas_cercano() {
        // Reproduce la forma del caso real: un gasto por transferencia cuyo
        // importe arrastra un tercer decimal.
        // Se migra entero y después se siembra el valor torcido, para poder
        // aplicar la migración 3 a mano y observar su efecto.
        let mut c = base_migrada();
        c.execute_batch(
            "INSERT INTO categorias (nombre) VALUES ('Prueba');
             INSERT INTO gastos (fecha, monto, divisa, descripcion, categoria_id, metodo_pago)
             VALUES ('13/09/2026', 1234.567, 'DOP', 'Con fracción', 1, 'transferencia');",
        )
        .unwrap();
        assert_eq!(fuera_de_centavo(&c, "gastos", "monto"), 1, "sembrado fuera de centavo");

        let tx = c.transaction().unwrap();
        crate::db_sql::migracion_3_centavos_exactos(&tx).unwrap();
        tx.commit().unwrap();

        let monto: f64 =
            c.query_row("SELECT monto FROM gastos;", [], |r| r.get(0)).unwrap();
        assert!((monto - 1234.57).abs() < 1e-9, "al centavo más cercano, obtenido {monto}");
        assert_eq!(fuera_de_centavo(&c, "gastos", "monto"), 0);
    }

    #[test]
    fn las_tasas_no_se_redondean_al_centavo() {
        // La distinción que hace explícita la lista: una tasa de 58.9642 sirve
        // para reconstruir una conversión; redondeada a 58.96 deja de servir.
        let mut c = base_migrada();
        c.execute_batch(
            "INSERT INTO prestamos (tipo_prestamo, monto_prestamo, institucion_financiera,
                                    tasa_actual, monto_cuota, dia_pago)
             VALUES ('consumo', 1000.0, 'Banco Ejemplo', 18.755, 100.0, 5);",
        )
        .unwrap();

        let tx = c.transaction().unwrap();
        crate::db_sql::migracion_3_centavos_exactos(&tx).unwrap();
        tx.commit().unwrap();

        let tasa: f64 = c.query_row("SELECT tasa_actual FROM prestamos;", [], |r| r.get(0)).unwrap();
        assert!((tasa - 18.755).abs() < 1e-9, "la tasa conserva su precisión: {tasa}");
    }

    #[test]
    fn ninguna_columna_de_dinero_queda_fuera_de_centavo_tras_migrar() {
        let c = base_migrada();

        for (tabla, columna) in COLUMNAS_DE_DINERO {
            let existe: i64 = c
                .query_row(
                    &format!("SELECT COUNT(*) FROM pragma_table_info('{}') WHERE name = ?;", tabla),
                    [columna],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            if existe == 0 {
                continue;
            }
            assert_eq!(
                fuera_de_centavo(&c, tabla, columna),
                0,
                "{}.{} debería estar en centavos exactos",
                tabla,
                columna
            );
        }
    }

    #[test]
    fn la_lista_de_dinero_no_incluye_ninguna_tasa() {
        // Una tasa en esta lista perdería precisión en silencio. La prueba lo
        // impide de forma que no dependa de que alguien lo recuerde.
        for (tabla, columna) in COLUMNAS_DE_DINERO {
            assert!(
                !columna.contains("tasa") && !columna.contains("porcentaje"),
                "{}.{} parece una tasa y no debería redondearse al centavo",
                tabla,
                columna
            );
        }
    }

    #[test]
    fn la_verificacion_falla_si_queda_un_importe_fuera_de_centavo() {
        // Se comprueba que la red existe: si el redondeo no hubiera alcanzado
        // a una columna, la migración no se daría por buena.
        let mut c = base_migrada();
        c.execute_batch(
            "INSERT INTO cuentas_ahorro (nombre, divisa, balance_actual)
             VALUES ('Cuenta Ejemplo', 'DOP', 100.005);",
        )
        .unwrap();

        let tx = c.transaction().unwrap();
        let resultado = super::super::db_sql::verificar_centavos_exactos_para_pruebas(&tx);

        assert!(resultado.is_err(), "la verificación tiene que rechazarlo");
    }
}
