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
pub const VERSION_OBJETIVO: u32 = 12;

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
        Migracion {
            version: 4,
            nombre: "identidad y comisiones de las cuentas",
            aplicar: crate::db_sql::migracion_4_identidad_de_cuentas,
        },
        Migracion {
            version: 5,
            nombre: "vínculo del abono con lo que lo pagó",
            aplicar: crate::db_sql::migracion_5_vinculo_de_abonos,
        },
        Migracion {
            version: 6,
            nombre: "el abono guarda lo que debitó",
            aplicar: crate::db_sql::migracion_6_abono_guarda_lo_debitado,
        },
        Migracion {
            version: 7,
            nombre: "el cobro apunta a una cuenta, no a un nombre",
            aplicar: crate::db_sql::migracion_7_cobro_por_referencia,
        },
        Migracion {
            version: 8,
            nombre: "casos de corrección",
            aplicar: crate::db_sql::migracion_8_casos_de_correccion,
        },
        Migracion {
            version: 9,
            nombre: "las columnas que nacieron fuera de la red",
            aplicar: crate::db_sql::migracion_9_columnas_tardias,
        },
        Migracion {
            version: 10,
            nombre: "el céntimo exacto, sin tolerancia",
            aplicar: crate::db_sql::migracion_10_centimo_exacto,
        },
        Migracion {
            version: 11,
            nombre: "la fracción de céntimo se rechaza al escribir",
            aplicar: crate::db_sql::migracion_11_rechazar_fraccion_de_centimo,
        },
        Migracion {
            version: 12,
            nombre: "la fecha de renovación de las anuales",
            aplicar: crate::db_sql::migracion_12_renovacion_anual,
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

    // **Las migraciones corren con las claves ajenas apagadas, y se comprueban
    // al final de cada una.** No es una licencia: es lo que SQLite exige para
    // reconstruir una tabla, y se descubrió midiendo.
    //
    // Con `foreign_keys` encendido, `ALTER TABLE ... RENAME` reescribe las
    // cláusulas `REFERENCES` de las demás tablas —y lo hace **aunque
    // `legacy_alter_table` esté activo**, que es el detalle que no estaba en
    // ninguna suposición previa: el `PRAGMA` se lee como encendido y no surte
    // efecto—. Renombrar `cuentas_ahorro` dejaba a `gastos` apuntando a una
    // tabla temporal que la migración borra después.
    //
    // A cambio de apagarlas, cada migración termina con `foreign_key_check`
    // **dentro de su transacción**: si dejó una referencia rota, no se
    // confirma. La comprobación pasa de ser por sentencia a ser por
    // migración, que para un cambio de esquema es el grano correcto.
    let apagar = |c: &Connection| {
        let _ = c.execute_batch("PRAGMA foreign_keys = OFF;");
    };
    let encender = |c: &Connection| {
        let _ = c.execute_batch("PRAGMA foreign_keys = ON;");
    };
    apagar(conn);
    let resultado = aplicar_pendientes(conn, version_inicial);
    encender(conn);
    let aplicadas = resultado?;

    Ok(Informe { version_inicial, version_final: version_de(conn)?, aplicadas })
}

fn aplicar_pendientes(
    conn: &mut Connection,
    version_inicial: u32,
) -> Result<Vec<&'static str>, ErrorMigracion> {
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

        verificar_referencias(&tx, &migracion)?;

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

    Ok(aplicadas)
}

/// Que la migración no haya dejado ninguna referencia rota.
///
/// Sustituye a la comprobación por sentencia que `foreign_keys` daba, y lo
/// hace **antes de confirmar**: una migración que rompe la integridad no se
/// aplica a medias, se deshace entera.
fn verificar_referencias(
    tx: &Transaction,
    migracion: &Migracion,
) -> Result<(), ErrorMigracion> {
    let rotas: i64 = tx
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check;", [], |r| r.get(0))
        .map_err(|e| ErrorMigracion::Almacenamiento(e.to_string()))?;

    if rotas > 0 {
        return Err(ErrorMigracion::Fallo {
            version: migracion.version,
            migracion: migracion.nombre,
            etapa: "integridad referencial".into(),
            causa: format!(
                "quedan {} fila(s) apuntando a algo que no existe. No se confirma nada.",
                rotas
            ),
        });
    }
    Ok(())
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
    use crate::db_sql::{COLUMNAS_DE_DINERO, COLUMNAS_DE_TASA};

    fn base_migrada() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        ejecutar(&mut c).unwrap();
        c
    }

    /// Una base migrada **hasta antes** de que exista la restricción de
    /// céntimo.
    ///
    /// Hace falta porque desde la migración 11 el esquema **rechaza** una
    /// fracción de céntimo, y las pruebas del redondeo necesitan poder
    /// sembrar una. Que ya no se pueda sembrar en una base al día no es un
    /// estorbo de las pruebas: es la garantía nueva, y
    /// `el_esquema_migrado_rechaza_una_fraccion_de_centimo` la afirma.
    fn base_migrada_hasta(version: u32) -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        for migracion in catalogo() {
            if migracion.version > version {
                break;
            }
            let tx = c.transaction().unwrap();
            (migracion.aplicar)(&tx).unwrap();
            tx.execute_batch(&format!("PRAGMA user_version = {};", migracion.version)).unwrap();
            tx.commit().unwrap();
        }
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
        // Se migra **hasta la 10** y después se siembra el valor torcido:
        // desde la 11 el esquema no admitiría una fracción de céntimo, que es
        // justo lo que esta prueba necesita poder escribir.
        let mut c = base_migrada_hasta(10);
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
    fn toda_columna_real_esta_clasificada_como_dinero_o_como_tasa() {
        // **La regla.** En un esquema migrado, cada columna `REAL` tiene que
        // estar declarada en una de las dos listas. Ninguna puede quedarse sin
        // clasificar.
        //
        // Existe porque la guardiana anterior protegía en un solo sentido:
        // impedía meter una tasa entre el dinero, pero no impedía **olvidar**
        // una columna de dinero. Cuatro se olvidaron por ahí —todas añadidas
        // en migraciones posteriores a la 3, que es la que redondea—, y una de
        // ellas dejó abierta una vía por la que entraban fracciones de centavo
        // a la base.
        //
        // El coste de cumplirla es una línea en una lista. El de no tenerla ya
        // se pagó.
        let mut c = base_migrada();
        let tx = c.transaction().unwrap();

        let tablas: Vec<String> = {
            let mut s = tx
                .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%';")
                .unwrap();
            let f = s.query_map([], |r| r.get(0)).unwrap();
            f.map(|x| x.unwrap()).collect()
        };

        let mut sin_clasificar = Vec::new();
        for tabla in &tablas {
            let mut s = tx.prepare(&format!("PRAGMA table_info({});", tabla)).unwrap();
            let columnas: Vec<(String, String)> = s
                .query_map([], |r| Ok((r.get(1)?, r.get(2)?)))
                .unwrap()
                .map(|x| x.unwrap())
                .collect();

            for (columna, tipo) in columnas {
                if !tipo.eq_ignore_ascii_case("REAL") {
                    continue;
                }
                let par = (tabla.as_str(), columna.as_str());
                let es_dinero = COLUMNAS_DE_DINERO.iter().any(|(t, c)| *t == par.0 && *c == par.1);
                let es_tasa = COLUMNAS_DE_TASA.iter().any(|(t, c)| *t == par.0 && *c == par.1);
                if !es_dinero && !es_tasa {
                    sin_clasificar.push(format!("{}.{}", tabla, columna));
                }
            }
        }

        assert!(
            sin_clasificar.is_empty(),
            "columnas REAL sin clasificar: {:?}.\n\
             Declárala en COLUMNAS_DE_DINERO si es un importe —entrará en el \
             redondeo al céntimo— o en COLUMNAS_DE_TASA si no lo es.",
            sin_clasificar
        );
    }


    // --- El tramo 4, fijado por las mediciones que lo decidieron ---
    //
    // El tramo preveía convertir las columnas de dinero a `INTEGER`. Se midió
    // antes de hacerlo y la premisa resultó falsa, de modo que se cerró como
    // el tramo 3: declarándolo innecesario. Estas pruebas conservan las
    // mediciones, porque un argumento que solo vive en un documento deja de
    // comprobarse el día que alguien propone rehacerlo.

    #[test]
    fn el_esquema_migrado_rechaza_una_fraccion_de_centimo() {
        // La garantía nueva, y la razón de todo el tramo: la fracción se
        // rechaza **al escribir**, no solo al migrar.
        let c = base_migrada();
        c.execute_batch("INSERT INTO categorias (nombre) VALUES ('Prueba');").unwrap();
        let con_fraccion = c.execute(
            "INSERT INTO gastos (fecha, monto, divisa, descripcion, categoria_id, metodo_pago)
             VALUES ('13/09/2026', ?, 'DOP', 'Con fracción', 1, 'transferencia');",
            [1_234.567f64],
        );
        assert!(con_fraccion.is_err(), "el esquema aceptó una fracción de céntimo");

        let al_centimo = c.execute(
            "INSERT INTO gastos (fecha, monto, divisa, descripcion, categoria_id, metodo_pago)
             VALUES ('13/09/2026', ?, 'DOP', 'Al céntimo', 1, 'transferencia');",
            [1_234.57f64],
        );
        assert!(al_centimo.is_ok(), "el esquema rechazó un importe bueno: {al_centimo:?}");
    }

    #[test]
    fn la_afinidad_integer_no_habria_restringido_nada() {
        // **La premisa que mató al tramo.** Una columna declarada `INTEGER`
        // acepta 75.005 y lo guarda como `real`: la afinidad solo convierte
        // cuando no pierde. Cambiar el tipo no impedía lo que se quería
        // impedir, y por eso la garantía vive en un `CHECK`.
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE t (v INTEGER);").unwrap();
        c.execute("INSERT INTO t VALUES (?);", [75.005f64]).unwrap();

        let tipo: String = c.query_row("SELECT typeof(v) FROM t;", [], |r| r.get(0)).unwrap();
        assert_eq!(tipo, "real", "la afinidad INTEGER habría restringido, y no lo hace");
    }

    #[test]
    fn leer_centavos_enteros_como_coma_flotante_no_falla_y_multiplica_por_cien() {
        // El coste que habría tenido convertir a `INTEGER`: cualquier lectura
        // `f64` que quedara sin migrar devolvería el entero crudo **sin
        // error**. Cien veces el importe, en silencio.
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE t (v INTEGER); INSERT INTO t VALUES (7500);").unwrap();

        let leido: f64 = c.query_row("SELECT v FROM t;", [], |r| r.get(0)).unwrap();
        assert_eq!(leido, 7_500.0, "75.00 leído como 7500.00, y sin avisar");

        // La dirección contraria sí avisa, y es la única que lo hace.
        let c2 = Connection::open_in_memory().unwrap();
        c2.execute_batch("CREATE TABLE t (v REAL); INSERT INTO t VALUES (75.0);").unwrap();
        let entero: Result<i64, _> = c2.query_row("SELECT v FROM t;", [], |r| r.get(0));
        assert!(entero.is_err(), "leer un REAL como entero tiene que fallar en voz alta");
    }

    #[test]
    fn la_restriccion_no_rechaza_ningun_centimo_legitimo() {
        // `ROUND(v,2) = v` frente a las variantes con `* 100`. **No es una
        // elección de estilo**: multiplicar por cien introduce el error que se
        // pretendía detectar, y las variantes rechazan importes buenos.
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(
            "CREATE TABLE buena (v REAL CHECK (ROUND(v, 2) = v));
             CREATE TABLE por_cien (v REAL CHECK (v * 100 = ROUND(v * 100)));",
        )
        .unwrap();

        let (mut rechaza_buena, mut rechaza_por_cien) = (0u32, 0u32);
        for centavos in 0..50_000i64 {
            let unidades = centavos as f64 / 100.0;
            if c.execute("INSERT INTO buena VALUES (?);", [unidades]).is_err() {
                rechaza_buena += 1;
            }
            if c.execute("INSERT INTO por_cien VALUES (?);", [unidades]).is_err() {
                rechaza_por_cien += 1;
            }
        }

        assert_eq!(rechaza_buena, 0, "la restricción elegida rechazó céntimos legítimos");
        assert!(
            rechaza_por_cien > 0,
            "la variante con * 100 dejó de fallar: si eso cambia, revisa por qué \
             se descartó antes de adoptarla"
        );
    }

    #[test]
    fn la_restriccion_aguanta_las_magnitudes_que_el_sistema_admite() {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE t (v REAL CHECK (ROUND(v, 2) = v));").unwrap();

        let mut centavos = 50_000i64;
        while centavos < 100_000_000_000_000 {
            let unidades = centavos as f64 / 100.0;
            assert!(
                c.execute("INSERT INTO t VALUES (?);", [unidades]).is_ok(),
                "rechazó {} centavos, una magnitud legítima",
                centavos
            );
            centavos = centavos * 3 / 2 + 13;
        }
    }

    #[test]
    fn la_reconstruccion_conserva_las_filas_y_las_referencias() {
        // Reconstruir doce tablas es el coste del tramo. Que no se pierda una
        // fila ni se rompa una referencia es lo que lo hace aceptable, y se
        // comprueba en vez de suponerse.
        let c = base_migrada();
        c.execute_batch(
            "INSERT INTO clientes (rnc, nombre) VALUES ('000000000', 'Cliente de prueba');
             INSERT INTO ingresos (numero_factura, cliente_id, fecha_emision, monto_total,
                                   porcentaje_retencion, monto_retenido)
             VALUES ('A-001', 1, '13/09/2026', 1000.0, 15.0, 150.0);",
        )
        .unwrap();

        let rotas: i64 = c
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check;", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rotas, 0, "la reconstrucción dejó referencias rotas");

        let hijas: i64 = c
            .query_row("SELECT COUNT(*) FROM ingresos WHERE cliente_id = 1;", [], |r| r.get(0))
            .unwrap();
        assert_eq!(hijas, 1, "la fila hija no sobrevivió");

        // Y la clave ajena sigue apuntando a `clientes`, no a una temporal.
        let sql: String = c
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name='ingresos';",
                [], |r| r.get(0))
            .unwrap();
        assert!(sql.contains("REFERENCES clientes"), "la referencia quedó reescrita: {sql}");
        assert!(!sql.contains("_previa"), "quedó apuntando a una tabla temporal: {sql}");
    }

    #[test]
    fn migrar_una_base_ya_migrada_no_vuelve_a_reconstruir() {
        // Idempotencia: la migración 11 mira si la restricción ya está antes
        // de rehacer la tabla. Sin eso, cada arranque reconstruiría doce
        // tablas.
        let mut c = base_migrada();
        let antes: String = c
            .query_row("SELECT sql FROM sqlite_master WHERE name='gastos';", [], |r| r.get(0))
            .unwrap();

        let tx = c.transaction().unwrap();
        crate::db_sql::migracion_11_rechazar_fraccion_de_centimo(&tx).unwrap();
        tx.commit().unwrap();

        let despues: String = c
            .query_row("SELECT sql FROM sqlite_master WHERE name='gastos';", [], |r| r.get(0))
            .unwrap();
        assert_eq!(antes, despues, "reconstruyó una tabla que ya tenía su restricción");
        assert_eq!(antes.matches("CHECK (ROUND(monto, 2) = monto)").count(), 1,
                   "duplicó la restricción");
    }

    #[test]
    fn toda_columna_de_dinero_vive_bajo_su_restriccion() {
        // La contrapartida de `toda_columna_real_esta_clasificada...`: estar
        // en la lista tiene que significar algo en el esquema.
        let c = base_migrada();
        for (tabla, columna) in COLUMNAS_DE_DINERO {
            let sql: String = c
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type='table' AND name=?;",
                    [tabla], |r| r.get(0))
                .unwrap_or_default();
            assert!(
                sql.contains(&format!("CHECK (ROUND({c}, 2) = {c})", c = columna)),
                "{}.{} está declarada como dinero y el esquema no la restringe",
                tabla, columna
            );
        }
    }

    #[test]
    fn una_migracion_que_rompe_una_referencia_no_se_confirma() {
        // Las migraciones corren con las claves ajenas apagadas para poder
        // reconstruir tablas. Esto afirma que apagarlas no es una licencia:
        // `foreign_key_check` cierra la puerta antes del COMMIT.
        let c = base_migrada();
        c.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();
        c.execute_batch(
            "INSERT INTO clientes (rnc, nombre) VALUES ('000000000', 'Cliente de prueba');
             INSERT INTO ingresos (numero_factura, cliente_id, fecha_emision, monto_total,
                                   porcentaje_retencion, monto_retenido)
             VALUES ('A-001', 99, '13/09/2026', 1000.0, 15.0, 150.0);",
        )
        .unwrap();

        let rotas: i64 = c
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check;", [], |r| r.get(0))
            .unwrap();
        assert!(rotas > 0, "la comprobación no ve una referencia rota evidente");
    }


    #[test]
    fn ninguna_acumulacion_en_sql_escribe_sin_redondear() {
        // **La regla que el `CHECK` obliga a tener.** Un `SET v = v + ?`
        // suma en coma flotante y el resultado arrastra ruido: medido, el 13%
        // de esas escrituras produce un valor que el esquema rechaza, y la
        // quinta ya falla. Los doce valores inexactos que la migración 10
        // limpia venían de aquí.
        //
        // La corrección es `ROUND(v ± ?, 2)`, y no decide ningún céntimo: con
        // los dos operandos exactos, lo que corrige está acotado en 1,5·10⁻⁵
        // unidades —por debajo incluso de la tolerancia de representación—.
        let fuentes = [
            ("main.rs", include_str!("main.rs")),
            ("adaptadores/sqlite/gastos.rs", include_str!("adaptadores/sqlite/gastos.rs")),
        ];
        let mut sin_redondear = Vec::new();
        for (nombre, texto) in fuentes {
            for (n, linea) in texto.lines().enumerate() {
                let l = linea.trim();
                if !l.contains("SET ") || !l.contains(" = ") {
                    continue;
                }
                // `SET col = col + ?` sin ROUND alrededor.
                for columna in COLUMNAS_DE_DINERO.iter().map(|(_, c)| *c) {
                    let crudo = format!("SET {c} = {c} ", c = columna);
                    if l.contains(&crudo) {
                        sin_redondear.push(format!("{}:{}: {}", nombre, n + 1, l));
                    }
                }
            }
        }
        assert!(
            sin_redondear.is_empty(),
            "acumulan en SQL sin redondear al céntimo; el esquema las rechazará:\n{}",
            sin_redondear.join("\n")
        );
    }

    #[test]
    fn acumular_redondeando_no_produce_un_valor_que_el_esquema_rechace() {
        // La medición que dictó la regla anterior, en las dos formas.
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(
            "CREATE TABLE crudo (v REAL CHECK (ROUND(v,2) = v));
             CREATE TABLE redondeado (v REAL CHECK (ROUND(v,2) = v));
             INSERT INTO crudo VALUES (0.0);
             INSERT INTO redondeado VALUES (0.0);",
        )
        .unwrap();

        let (mut fallan_crudo, mut fallan_redondeado) = (0u32, 0u32);
        let mut peor_correccion = 0.0f64;
        for i in 1..5_000i64 {
            let sumando = ((i * 7_919) % 1_000_000) as f64 / 100.0;

            if c.execute("UPDATE crudo SET v = v + ?;", [sumando]).is_err() {
                fallan_crudo += 1;
                c.execute("UPDATE crudo SET v = ROUND(v, 2);", []).unwrap();
            }

            let previo: f64 = c.query_row("SELECT v FROM redondeado;", [], |r| r.get(0)).unwrap();
            if c.execute("UPDATE redondeado SET v = ROUND(v + ?, 2);", [sumando]).is_err() {
                fallan_redondeado += 1;
            }
            let ahora: f64 = c.query_row("SELECT v FROM redondeado;", [], |r| r.get(0)).unwrap();
            peor_correccion = peor_correccion.max((ahora - (previo + sumando)).abs());
        }

        assert!(fallan_crudo > 0, "sumar sin redondear dejó de romper el esquema: \
                si eso cambia, esta regla merece revisarse en vez de mantenerse por inercia");
        assert_eq!(fallan_redondeado, 0, "redondear no bastó para satisfacer el esquema");
        assert!(
            peor_correccion < 0.005,
            "el redondeo de la acumulación movió {peor_correccion:.3e} unidades: \
             eso ya no es limpiar ruido, es decidir un céntimo"
        );
    }


    #[test]
    fn la_migracion_12_deriva_la_renovacion_del_dia_de_facturacion_y_no_del_cargo() {
        // **La distinción que importa.** El cargo se anota el día en que se
        // abrió la aplicación, que puede ser posterior al de facturación. El
        // proveedor renueva el suyo, así que la fecha derivada toma
        // `dia_facturacion`. Sobre datos reales esa diferencia era de un día.
        let c = base_migrada();
        c.execute_batch(
            "INSERT INTO tarjetas (entidad, nombre_tarjeta, fecha_corte, fecha_limite_pago)
             VALUES ('Emisor', 'Producto', 5, 25);",
        )
        .unwrap();
        // Se siembra saltándose el comando, para fijar una marca ya existente.
        c.execute_batch(
            "INSERT INTO suscripciones (plataforma, monto, tarjeta_id, frecuencia, dia_facturacion, divisa, fecha_ultimo_pago)
                 VALUES ('Anual tardía', 100.0, 1, 'anual', 10, 'DOP', '11/08/2026'),
                        ('Anual sin marca', 100.0, 1, 'anual', 10, 'DOP', NULL),
                        ('Anual ilegible', 100.0, 1, 'anual', 10, 'DOP', '2026-08-11'),
                        ('Anual en enero 31', 100.0, 1, 'anual', 31, 'DOP', '15/02/2026'),
                        ('Mensual', 100.0, 1, 'mensual', 10, 'DOP', '11/08/2026');
             UPDATE suscripciones SET fecha_renovacion = NULL;",
        )
        .unwrap();

        let mut c = c;
        let tx = c.transaction().unwrap();
        crate::db_sql::migracion_12_renovacion_anual(&tx).unwrap();
        tx.commit().unwrap();

        let leer = |plataforma: &str| -> Option<String> {
            c.query_row(
                "SELECT fecha_renovacion FROM suscripciones WHERE plataforma = ?;",
                [plataforma],
                |r| r.get(0),
            )
            .unwrap()
        };

        assert_eq!(leer("Anual tardía").as_deref(), Some("10/08/2027"),
                   "toma el día 10 de facturación, no el 11 en que se ejecutó");
        assert_eq!(leer("Anual sin marca"), None,
                   "sin marca no hay nada que derivar, y sin fecha no se cobra");
        assert_eq!(leer("Anual ilegible"), None,
                   "una marca ilegible tampoco permite deducir: se queda sin fecha");
        assert_eq!(leer("Anual en enero 31").as_deref(), Some("28/02/2027"),
                   "un día que no existe en el mes se recorta al último");
        assert_eq!(leer("Mensual"), None, "una mensual no usa la fecha");
    }

    #[test]
    fn la_migracion_12_no_reescribe_una_fecha_ya_anotada() {
        let c = base_migrada();
        c.execute_batch(
            "INSERT INTO tarjetas (entidad, nombre_tarjeta, fecha_corte, fecha_limite_pago)
                 VALUES ('Emisor', 'Producto', 5, 25);
             INSERT INTO suscripciones (plataforma, monto, tarjeta_id, frecuencia, dia_facturacion, divisa, fecha_ultimo_pago, fecha_renovacion)
                 VALUES ('Anual', 100.0, 1, 'anual', 10, 'DOP', '11/08/2026', '01/01/2030');",
        )
        .unwrap();

        let mut c = c;
        let tx = c.transaction().unwrap();
        crate::db_sql::migracion_12_renovacion_anual(&tx).unwrap();
        tx.commit().unwrap();

        let f: String = c
            .query_row("SELECT fecha_renovacion FROM suscripciones;", [], |r| r.get(0))
            .unwrap();
        assert_eq!(f, "01/01/2030", "lo anotado por el titular manda sobre lo derivado");
    }

    #[test]
    fn ninguna_columna_esta_en_las_dos_listas() {
        // Clasificarla dos veces sería una contradicción declarada, y la
        // redondearía al céntimo por estar en la primera.
        for (t, c) in COLUMNAS_DE_DINERO {
            assert!(
                !COLUMNAS_DE_TASA.iter().any(|(t2, c2)| t2 == t && c2 == c),
                "{}.{} está declarada como dinero y como tasa a la vez",
                t,
                c
            );
        }
    }

    #[test]
    fn la_verificacion_falla_si_queda_un_importe_fuera_de_centavo() {
        // Se comprueba que la red existe: si el redondeo no hubiera alcanzado
        // a una columna, la migración no se daría por buena. Se siembra antes
        // de la 11 por el mismo motivo que la prueba anterior.
        let mut c = base_migrada_hasta(10);
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
