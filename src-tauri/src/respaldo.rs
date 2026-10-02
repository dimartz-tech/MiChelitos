//! Respaldos consistentes de la base de datos.
//!
//! Copiar el archivo con `cp` produce un respaldo válido **solo** si nadie
//! está escribiendo. No hay forma de garantizarlo desde fuera: la aplicación
//! abre una conexión por comando, así que una copia tomada en el instante
//! equivocado captura una escritura a medias y da un archivo que se abre pero
//! miente.
//!
//! `VACUUM INTO` no tiene ese problema. SQLite escribe una base nueva y
//! completa desde una vista coherente de la actual, tomando los bloqueos que
//! haga falta. Y como el resultado es una base y no un montón de bytes, se
//! puede **verificar**: aquí se comprueba su integridad y sus claves foráneas
//! antes de darla por buena, porque un respaldo que no se ha abierto nunca no
//! es un respaldo, es un archivo.
//!
//! El respaldo se toma antes de tocar el esquema. Si no se puede tomar, la
//! preparación se detiene: migrar sin red es exactamente el riesgo que esto
//! existe para evitar.

use chrono::Local;
use rusqlite::Connection;
use std::fmt;
use std::path::{Path, PathBuf};

/// Cuántos respaldos se conservan. Los más antiguos se descartan.
const RESPALDOS_A_CONSERVAR: usize = 10;

#[derive(Debug)]
pub enum ErrorRespaldo {
    /// La copia se escribió pero no superó las comprobaciones.
    CopiaCorrupta { detalle: String },
    Fallo(String),
}

impl fmt::Display for ErrorRespaldo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorRespaldo::CopiaCorrupta { detalle } => write!(
                f,
                "El respaldo se creó pero no superó la verificación ({}). Se descartó.",
                detalle
            ),
            ErrorRespaldo::Fallo(detalle) => write!(f, "No se pudo respaldar la base: {}", detalle),
        }
    }
}

impl std::error::Error for ErrorRespaldo {}

fn fallo(e: impl fmt::Display) -> ErrorRespaldo {
    ErrorRespaldo::Fallo(e.to_string())
}

fn ruta_capital() -> PathBuf {
    PathBuf::from(crate::db_nosql::obtener_ruta_nosql("capital"))
}

/// La copia del capital de un respaldo lleva su mismo nombre, con otro sufijo:
/// `michelitos_X.db` → `michelitos_X.capital.json`.
fn copia_de_capital(respaldo: &Path) -> PathBuf {
    respaldo.with_extension("capital.json")
}

/// Dónde viven los respaldos: fuera del directorio de la base, dentro del de
/// la aplicación. Nunca en el repositorio.
pub fn directorio_de_respaldos() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".michelitos/databases/respaldos")
}

/// Respalda la base si hace falta, y devuelve dónde quedó.
///
/// Devuelve `None` cuando no había nada que respaldar: o la base todavía no
/// existe —instalación nueva—, o no ha cambiado desde el último respaldo. Ese
/// segundo caso es lo que evita acumular copias idénticas en cada arranque.
pub fn respaldar_si_hace_falta(motivo: &str) -> Result<Option<PathBuf>, ErrorRespaldo> {
    let origen = crate::db_sql::obtener_ruta_db();
    let origen = Path::new(&origen);

    if !origen.exists() {
        return Ok(None);
    }
    if !ha_cambiado_desde_el_ultimo_respaldo(origen)? {
        return Ok(None);
    }

    respaldar(motivo).map(Some)
}

/// Respalda la base incondicionalmente.
pub fn respaldar(motivo: &str) -> Result<PathBuf, ErrorRespaldo> {
    let destino = tomar_copia(motivo)?;

    if let Err(e) = verificar(&destino) {
        // Un respaldo que no se puede abrir es peor que ninguno: da confianza
        // falsa. Se retira.
        let _ = std::fs::remove_file(&destino);
        let _ = std::fs::remove_file(copia_de_capital(&destino));
        return Err(e);
    }

    podar(&directorio_de_respaldos());
    Ok(destino)
}

/// Escribe la copia de la base y del capital, sin verificarla.
fn tomar_copia(motivo: &str) -> Result<PathBuf, ErrorRespaldo> {
    let origen = crate::db_sql::obtener_ruta_db();
    if !Path::new(&origen).exists() {
        return Err(ErrorRespaldo::Fallo("la base todavía no existe".into()));
    }

    let directorio = directorio_de_respaldos();
    std::fs::create_dir_all(&directorio).map_err(fallo)?;

    // Marca de tiempo en ISO, como los commits y el historial. El motivo va en
    // el nombre para que un respaldo se explique solo dentro de un año.
    let marca = Local::now().format("%Y-%m-%dT%H-%M-%S");
    let destino = directorio.join(format!("michelitos_{}_{}.db", marca, sanear(motivo)));

    // VACUUM INTO se niega a sobrescribir. Con marca al segundo, dos respaldos
    // en el mismo segundo son posibles en pruebas; se desambigua en vez de
    // fallar por algo que no le importa a nadie.
    let destino = ruta_libre(destino);

    let conexion = Connection::open(&origen).map_err(fallo)?;
    conexion
        .execute("VACUUM INTO ?;", [destino.to_string_lossy().as_ref()])
        .map_err(fallo)?;

    // El capital vive fuera de SQLite, en un JSON. Una copia sin él dejaría
    // sin recuperar justo lo que no se puede reconstruir desde los gastos.
    let capital = ruta_capital();
    if capital.exists() {
        if let Err(e) = std::fs::copy(&capital, copia_de_capital(&destino)) {
            let _ = std::fs::remove_file(&destino);
            return Err(fallo(e));
        }
    }

    Ok(destino)
}

/// La red de seguridad de una restauración: guarda el estado actual **aunque
/// esté dañado**.
///
/// Restaurar se pide, sobre todo, cuando algo salió mal, y un estado con
/// referencias rotas no supera la verificación de un respaldo normal. Negarse
/// a restaurar por eso dejaría al titular sin salida justo entonces. Lo dañado
/// se conserva igualmente, con «sin verificar» en el nombre para que nadie lo
/// tome por una copia sana, y solo si ni siquiera se puede copiar se aborta.
fn copia_de_seguridad_previa() -> Result<PathBuf, ErrorRespaldo> {
    match tomar_copia("antes de restaurar") {
        Ok(copia) => match verificar(&copia) {
            Ok(()) => {
                podar(&directorio_de_respaldos());
                Ok(copia)
            }
            Err(_) => marcar_sin_verificar(copia),
        },
        Err(e) => {
            // VACUUM INTO no pudo leer la base: se copia el archivo tal cual.
            let origen = PathBuf::from(crate::db_sql::obtener_ruta_db());
            let marca = Local::now().format("%Y-%m-%dT%H-%M-%S");
            let destino = ruta_libre(directorio_de_respaldos().join(format!(
                "michelitos_{}_antes-de-restaurar-sin-verificar.db",
                marca
            )));
            std::fs::copy(&origen, &destino).map(|_| destino).map_err(|_| e)
        }
    }
}

fn marcar_sin_verificar(copia: PathBuf) -> Result<PathBuf, ErrorRespaldo> {
    let nombre = copia.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let nueva = ruta_libre(copia.with_file_name(format!("{nombre}-sin-verificar.db")));
    std::fs::rename(&copia, &nueva).map_err(fallo)?;
    let capital = copia_de_capital(&copia);
    if capital.exists() {
        let _ = std::fs::rename(&capital, copia_de_capital(&nueva));
    }
    Ok(nueva)
}

/// Abre el respaldo y comprueba que sirve.
///
/// `integrity_check` valida la estructura del archivo; `foreign_key_check`
/// valida que las relaciones sigan en pie. Son comprobaciones distintas: una
/// base íntegra puede tener referencias rotas.
fn verificar(ruta: &Path) -> Result<(), ErrorRespaldo> {
    let copia = Connection::open(ruta)
        .map_err(|e| ErrorRespaldo::CopiaCorrupta { detalle: format!("no se puede abrir: {}", e) })?;

    let integridad: String = copia
        .query_row("PRAGMA integrity_check;", [], |r| r.get(0))
        .map_err(|e| ErrorRespaldo::CopiaCorrupta { detalle: e.to_string() })?;
    if integridad != "ok" {
        return Err(ErrorRespaldo::CopiaCorrupta { detalle: integridad });
    }

    let referencias_rotas: i64 = copia
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check;", [], |r| r.get(0))
        .map_err(|e| ErrorRespaldo::CopiaCorrupta { detalle: e.to_string() })?;
    if referencias_rotas > 0 {
        return Err(ErrorRespaldo::CopiaCorrupta {
            detalle: format!("{} referencias rotas", referencias_rotas),
        });
    }

    Ok(())
}

/// Si la base se ha modificado después del respaldo más reciente.
///
/// Ante cualquier duda —no hay respaldos, no se puede leer una fecha— la
/// respuesta es que sí: un respaldo de más no cuesta nada y uno de menos sí.
fn ha_cambiado_desde_el_ultimo_respaldo(origen: &Path) -> Result<bool, ErrorRespaldo> {
    let modificada = std::fs::metadata(origen).and_then(|m| m.modified()).ok();
    // Un cambio solo en el capital también merece copia.
    let modificada = match (modificada, std::fs::metadata(ruta_capital()).and_then(|m| m.modified()).ok()) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, _) => a,
    };
    let ultimo = respaldos_existentes()
        .into_iter()
        .filter_map(|r| std::fs::metadata(&r).and_then(|m| m.modified()).ok())
        .max();
    Ok(merece_respaldo(modificada, ultimo))
}

/// Decide si hace falta un respaldo comparando las dos fechas.
///
/// Separada del sistema de archivos para poder probarla sin depender de la
/// granularidad con que cada sistema guarda las marcas de tiempo, que es una
/// fuente clásica de pruebas intermitentes.
fn merece_respaldo(
    modificada: Option<std::time::SystemTime>,
    ultimo_respaldo: Option<std::time::SystemTime>,
) -> bool {
    match (modificada, ultimo_respaldo) {
        // Sin respaldos previos, siempre.
        (_, None) => true,
        // Si no se puede leer la fecha de la base, se respalda: un respaldo de
        // más no cuesta nada y uno de menos sí.
        (None, _) => true,
        (Some(m), Some(r)) => m > r,
    }
}

fn respaldos_existentes() -> Vec<PathBuf> {
    let directorio = directorio_de_respaldos();
    let Ok(entradas) = std::fs::read_dir(&directorio) else {
        return Vec::new();
    };
    let mut rutas: Vec<PathBuf> = entradas
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "db").unwrap_or(false))
        .collect();
    // El nombre empieza por la marca de tiempo, así que ordenar por nombre
    // ordena por antigüedad sin volver a tocar el sistema de archivos.
    rutas.sort();
    rutas
}

fn podar(_directorio: &Path) {
    let existentes = respaldos_existentes();
    if existentes.len() <= RESPALDOS_A_CONSERVAR {
        return;
    }
    for viejo in &existentes[..existentes.len() - RESPALDOS_A_CONSERVAR] {
        let _ = std::fs::remove_file(viejo);
        let _ = std::fs::remove_file(copia_de_capital(viejo));
    }
}

/// Lo que se hizo al restaurar, para que quien lo pidió lo pueda leer.
#[derive(Debug, PartialEq)]
pub struct Restauracion {
    /// Dónde quedó la copia tomada del estado que se sustituyó.
    pub respaldo_de_seguridad: PathBuf,
    /// Si el respaldo traía capital y se restauró. Si no, el actual se deja.
    pub capital_restaurado: bool,
    pub version_del_esquema: u32,
}

/// Respaldos disponibles, del más reciente al más antiguo.
pub fn listar() -> Vec<String> {
    let mut nombres: Vec<String> = respaldos_existentes()
        .iter()
        .filter_map(|r| r.file_name().map(|n| n.to_string_lossy().to_string()))
        .collect();
    nombres.reverse();
    nombres
}

/// Devuelve la base y el capital al estado de un respaldo.
///
/// Es de ida, así que se protege por los dos lados: **antes** se comprueba
/// que el respaldo sirve y que no viene de una versión más nueva que la que
/// esta aplicación sabe abrir, y **se respalda el estado actual** para poder
/// deshacer la restauración. Si algo falla antes de sustituir, nada cambió.
///
/// `nombre` es el nombre de archivo tal como lo devuelve [`listar`]; una ruta
/// se rechaza, para que no se pueda restaurar nada de fuera de la carpeta.
pub fn restaurar(nombre: &str) -> Result<Restauracion, ErrorRespaldo> {
    if Path::new(nombre).file_name().map(|n| n.to_string_lossy()) != Some(nombre.into())
        || !nombre.ends_with(".db")
    {
        return Err(fallo("nombre de respaldo no válido"));
    }
    let respaldo = directorio_de_respaldos().join(nombre);
    if !respaldo.exists() {
        return Err(fallo(format!("no existe el respaldo {nombre}")));
    }

    verificar(&respaldo)?;
    let version: u32 = Connection::open(&respaldo)
        .and_then(|c| c.query_row("PRAGMA user_version;", [], |r| r.get(0)))
        .map_err(fallo)?;
    if version > crate::migraciones::VERSION_OBJETIVO {
        return Err(fallo(format!(
            "el respaldo es del esquema {version} y esta versión solo llega al {}",
            crate::migraciones::VERSION_OBJETIVO
        )));
    }

    let destino = PathBuf::from(crate::db_sql::obtener_ruta_db());
    let respaldo_de_seguridad = copia_de_seguridad_previa()?;

    // Se prepara al lado y se sustituye con un renombrado: o queda la base
    // vieja entera o la nueva entera, nunca una mezcla.
    let provisional = destino.with_extension("db.restaurando");
    std::fs::copy(&respaldo, &provisional).map_err(fallo)?;
    if let Err(e) = verificar(&provisional) {
        let _ = std::fs::remove_file(&provisional);
        return Err(e);
    }
    // Un diario de escritura de la base anterior no debe aplicarse a la nueva.
    for sufijo in ["-wal", "-shm"] {
        let mut viejo = destino.clone().into_os_string();
        viejo.push(sufijo);
        let _ = std::fs::remove_file(PathBuf::from(viejo));
    }
    std::fs::rename(&provisional, &destino).map_err(fallo)?;

    let capital_guardado = copia_de_capital(&respaldo);
    let capital_restaurado = capital_guardado.exists();
    if capital_restaurado {
        let capital = ruta_capital();
        let provisional = capital.with_extension("json.restaurando");
        std::fs::copy(&capital_guardado, &provisional).map_err(fallo)?;
        std::fs::rename(&provisional, &capital).map_err(fallo)?;
    }

    Ok(Restauracion { respaldo_de_seguridad, capital_restaurado, version_del_esquema: version })
}

fn sanear(motivo: &str) -> String {
    let limpio: String = motivo
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let limpio = limpio.trim_matches('-').to_string();
    if limpio.is_empty() { "manual".into() } else { limpio }
}

fn ruta_libre(propuesta: PathBuf) -> PathBuf {
    if !propuesta.exists() {
        return propuesta;
    }
    for n in 2..100 {
        let alternativa = propuesta.with_file_name(format!(
            "{}-{}.db",
            propuesta.file_stem().unwrap_or_default().to_string_lossy(),
            n
        ));
        if !alternativa.exists() {
            return alternativa;
        }
    }
    propuesta
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prepara un HOME temporal propio y devuelve la guarda del entorno.
    fn entorno() -> std::sync::MutexGuard<'static, ()> {
        let guarda = crate::caracterizacion::bloquear_entorno();
        // Nota: se comparte la guarda con las pruebas de caracterización
        // porque ambas mutan `HOME`.
        let raiz = std::env::temp_dir().join("michelitos-respaldo");
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(&raiz).unwrap();
        std::env::set_var("HOME", &raiz);
        guarda
    }

    fn crear_base_con_datos() {
        let ruta = crate::db_sql::obtener_ruta_db();
        std::fs::create_dir_all(Path::new(&ruta).parent().unwrap()).unwrap();
        let c = Connection::open(&ruta).unwrap();
        c.execute_batch(
            "CREATE TABLE IF NOT EXISTS prueba (id INTEGER PRIMARY KEY, valor TEXT);
             INSERT INTO prueba (valor) VALUES ('uno'), ('dos');",
        )
        .unwrap();
    }

    #[test]
    fn el_respaldo_es_una_base_abrible_con_los_mismos_datos() {
        let _g = entorno();
        crear_base_con_datos();

        let destino = respaldar("prueba").expect("respaldar");

        assert!(destino.exists());
        let copia = Connection::open(&destino).unwrap();
        let filas: i64 =
            copia.query_row("SELECT COUNT(*) FROM prueba;", [], |r| r.get(0)).unwrap();
        assert_eq!(filas, 2, "los datos viajaron");
    }

    #[test]
    fn el_nombre_lleva_marca_de_tiempo_iso_y_motivo() {
        let _g = entorno();
        crear_base_con_datos();

        let destino = respaldar("antes de migrar").expect("respaldar");
        let nombre = destino.file_name().unwrap().to_string_lossy().to_string();

        assert!(nombre.starts_with("michelitos_20"), "marca ISO: {nombre}");
        assert!(nombre.ends_with("_antes-de-migrar.db"), "motivo saneado: {nombre}");
    }

    #[test]
    fn no_se_respalda_una_base_que_no_existe() {
        let _g = entorno();

        assert_eq!(respaldar_si_hace_falta("arranque").unwrap(), None, "nada que copiar");
        assert!(respaldar("arranque").is_err(), "y pedirlo de frente es un error");
    }

    #[test]
    fn no_se_repite_el_respaldo_si_la_base_no_cambio() {
        let _g = entorno();
        crear_base_con_datos();

        let primero = respaldar_si_hace_falta("arranque").unwrap();
        assert!(primero.is_some(), "el primero sí se toma");

        let segundo = respaldar_si_hace_falta("arranque").unwrap();
        assert_eq!(segundo, None, "sin cambios no se acumulan copias idénticas");
        assert_eq!(respaldos_existentes().len(), 1);
    }

    #[test]
    fn la_decision_de_respaldar_depende_de_cual_fecha_es_mayor() {
        use std::time::{Duration, SystemTime};
        let antes = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let despues = SystemTime::UNIX_EPOCH + Duration::from_secs(2_000);

        assert!(merece_respaldo(Some(despues), Some(antes)), "la base cambió después");
        assert!(!merece_respaldo(Some(antes), Some(despues)), "el respaldo es posterior");
        assert!(!merece_respaldo(Some(antes), Some(antes)), "misma fecha, sin cambios");
        assert!(merece_respaldo(Some(antes), None), "sin respaldos previos, siempre");
        assert!(merece_respaldo(None, Some(despues)), "ante la duda, se respalda");
    }

    #[test]
    fn solo_se_conservan_los_mas_recientes() {
        let _g = entorno();
        crear_base_con_datos();

        for n in 0..RESPALDOS_A_CONSERVAR + 4 {
            respaldar(&format!("n{n}")).expect("respaldar");
        }

        assert_eq!(respaldos_existentes().len(), RESPALDOS_A_CONSERVAR, "se poda");
    }

    #[test]
    fn una_copia_que_no_supera_la_verificacion_no_se_conserva() {
        // Se comprueba la reacción del verificador ante un archivo que no es
        // una base: es lo que quedaría de una copia truncada.
        let _g = entorno();
        let directorio = directorio_de_respaldos();
        std::fs::create_dir_all(&directorio).unwrap();
        let falso = directorio.join("truncado.db");
        std::fs::write(&falso, b"esto no es una base de datos").unwrap();

        assert!(verificar(&falso).is_err(), "no se da por bueno lo que no se puede abrir");
    }

    #[test]
    fn el_motivo_vacio_no_produce_un_nombre_roto() {
        assert_eq!(sanear("   "), "manual");
        assert_eq!(sanear("///"), "manual");
        assert_eq!(sanear("Antes de Migrar 3"), "Antes-de-Migrar-3");
    }

    fn escribir_capital(contenido: &str) {
        crate::db_nosql::guardar_coleccion("capital", &serde_json::from_str(contenido).unwrap())
            .unwrap();
    }

    fn valores() -> Vec<String> {
        let c = Connection::open(crate::db_sql::obtener_ruta_db()).unwrap();
        let mut s = c.prepare("SELECT valor FROM prueba ORDER BY id;").unwrap();
        let v = s.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect();
        v
    }

    #[test]
    fn el_respaldo_lleva_una_copia_del_capital() {
        let _g = entorno();
        crear_base_con_datos();
        escribir_capital(r#"{"certificados":[{"monto":500}]}"#);

        let destino = respaldar("prueba").unwrap();

        let copia = std::fs::read_to_string(copia_de_capital(&destino)).unwrap();
        assert!(copia.contains("500"), "el capital viaja con la base");
    }

    #[test]
    fn sin_capital_no_se_inventa_una_copia() {
        let _g = entorno();
        crear_base_con_datos();

        let destino = respaldar("prueba").unwrap();

        assert!(!copia_de_capital(&destino).exists());
    }

    #[test]
    fn podar_se_lleva_tambien_la_copia_del_capital() {
        let _g = entorno();
        crear_base_con_datos();
        escribir_capital(r#"{"certificados":[]}"#);

        for n in 0..RESPALDOS_A_CONSERVAR + 3 {
            respaldar(&format!("n{n}")).unwrap();
        }

        let sueltas = std::fs::read_dir(directorio_de_respaldos())
            .unwrap()
            .filter(|e| e.as_ref().unwrap().path().to_string_lossy().ends_with(".capital.json"))
            .count();
        assert_eq!(sueltas, RESPALDOS_A_CONSERVAR, "ni una copia de capital huérfana");
    }

    #[test]
    fn restaurar_devuelve_base_y_capital_y_deja_red_para_deshacer() {
        let _g = entorno();
        crear_base_con_datos();
        escribir_capital(r#"{"certificados":[{"monto":500}]}"#);
        let bueno = respaldar("bueno").unwrap();
        let nombre = bueno.file_name().unwrap().to_string_lossy().to_string();

        // Se estropea todo después del respaldo.
        let c = Connection::open(crate::db_sql::obtener_ruta_db()).unwrap();
        c.execute("DELETE FROM prueba;", []).unwrap();
        c.execute("INSERT INTO prueba (valor) VALUES ('roto');", []).unwrap();
        drop(c);
        escribir_capital(r#"{"certificados":[]}"#);

        let r = restaurar(&nombre).expect("restaurar");

        assert_eq!(valores(), vec!["uno", "dos"], "la base volvió");
        assert!(r.capital_restaurado);
        assert!(crate::db_nosql::leer_coleccion("capital").unwrap().to_string().contains("500"));
        // Lo que había antes de restaurar se puede recuperar.
        let seguridad = Connection::open(&r.respaldo_de_seguridad).unwrap();
        let valor: String =
            seguridad.query_row("SELECT valor FROM prueba;", [], |r| r.get(0)).unwrap();
        assert_eq!(valor, "roto", "el estado sustituido quedó respaldado");
    }

    #[test]
    fn un_respaldo_sin_capital_no_borra_el_capital_actual() {
        let _g = entorno();
        crear_base_con_datos();
        let bueno = respaldar("bueno").unwrap();
        escribir_capital(r#"{"certificados":[{"monto":700}]}"#);

        let r = restaurar(&bueno.file_name().unwrap().to_string_lossy()).unwrap();

        assert!(!r.capital_restaurado, "y así se dice");
        assert!(crate::db_nosql::leer_coleccion("capital").unwrap().to_string().contains("700"));
    }

    #[test]
    fn no_se_restaura_un_respaldo_de_un_esquema_mas_nuevo() {
        let _g = entorno();
        crear_base_con_datos();
        let futuro = respaldar("futuro").unwrap();
        Connection::open(&futuro)
            .unwrap()
            .execute_batch(&format!(
                "PRAGMA user_version = {};",
                crate::migraciones::VERSION_OBJETIVO + 1
            ))
            .unwrap();
        Connection::open(crate::db_sql::obtener_ruta_db())
            .unwrap()
            .execute("INSERT INTO prueba (valor) VALUES ('intacto');", [])
            .unwrap();

        let r = restaurar(&futuro.file_name().unwrap().to_string_lossy());

        assert!(r.is_err());
        assert_eq!(valores(), vec!["uno", "dos", "intacto"], "no se tocó nada");
    }

    #[test]
    fn un_respaldo_corrupto_no_sustituye_a_la_base() {
        let _g = entorno();
        crear_base_con_datos();
        let malo = directorio_de_respaldos().join("michelitos_2026-01-01T00-00-00_roto.db");
        std::fs::create_dir_all(malo.parent().unwrap()).unwrap();
        std::fs::write(&malo, b"esto no es una base").unwrap();

        assert!(restaurar("michelitos_2026-01-01T00-00-00_roto.db").is_err());
        assert_eq!(valores(), vec!["uno", "dos"]);
    }

    #[test]
    fn un_respaldo_con_referencias_rotas_se_rechaza_sin_tomar_otro_respaldo() {
        let _g = entorno();
        crear_base_con_datos();
        let roto = directorio_de_respaldos().join("michelitos_2026-01-01T00-00-00_rotas.db");
        std::fs::create_dir_all(roto.parent().unwrap()).unwrap();
        let c = Connection::open(&roto).unwrap();
        c.execute_batch(
            "PRAGMA foreign_keys = OFF;
             CREATE TABLE padre (id INTEGER PRIMARY KEY);
             CREATE TABLE hijo (padre_id INTEGER REFERENCES padre(id));
             INSERT INTO hijo VALUES (99);",
        )
        .unwrap();
        drop(c);

        assert!(restaurar("michelitos_2026-01-01T00-00-00_rotas.db").is_err());
        assert_eq!(respaldos_existentes().len(), 1, "no se tomó copia de seguridad en vano");
        assert_eq!(valores(), vec!["uno", "dos"]);
    }

    #[test]
    fn solo_se_restauran_nombres_de_la_carpeta_de_respaldos() {
        let _g = entorno();
        crear_base_con_datos();

        // Existe de verdad, para que solo la extensión pueda rechazarlo.
        let ajeno = directorio_de_respaldos().join("notas.txt");
        std::fs::create_dir_all(ajeno.parent().unwrap()).unwrap();
        std::fs::write(&ajeno, b"x").unwrap();

        for nombre in ["../databases/sql/michelitos.db", "/etc/passwd", "notas.txt", "", "a/b.db"] {
            assert!(restaurar(nombre).is_err(), "rechazado: {nombre:?}");
        }
    }

    #[test]
    fn listar_devuelve_del_mas_reciente_al_mas_antiguo() {
        let _g = entorno();
        crear_base_con_datos();
        respaldar("a").unwrap();
        respaldar("b").unwrap();

        let nombres = listar();

        assert_eq!(nombres.len(), 2);
        assert!(nombres[0] > nombres[1], "{nombres:?}");
    }

    #[test]
    fn se_puede_restaurar_aunque_el_estado_actual_tenga_referencias_rotas() {
        let _g = entorno();
        crear_base_con_datos();
        let bueno = respaldar("bueno").unwrap();
        // Se daña el estado actual: una referencia que apunta a nada.
        let c = Connection::open(crate::db_sql::obtener_ruta_db()).unwrap();
        c.execute_batch(
            "PRAGMA foreign_keys = OFF;
             CREATE TABLE padre (id INTEGER PRIMARY KEY);
             CREATE TABLE hijo (padre_id INTEGER REFERENCES padre(id));
             INSERT INTO hijo VALUES (99);",
        )
        .unwrap();
        drop(c);

        let r = restaurar(&bueno.file_name().unwrap().to_string_lossy())
            .expect("restaurar no se bloquea por lo dañado que está lo actual");

        assert_eq!(valores(), vec!["uno", "dos"], "la base volvió");
        let nombre = r.respaldo_de_seguridad.file_name().unwrap().to_string_lossy().to_string();
        assert!(nombre.contains("sin-verificar"), "la copia dañada no se hace pasar por sana: {nombre}");
        assert!(r.respaldo_de_seguridad.exists(), "pero se conserva, por si hay que rescatar algo");
        let rescatada: i64 = Connection::open(&r.respaldo_de_seguridad)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM hijo;", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rescatada, 1, "con el estado dañado dentro");
    }

    #[test]
    fn la_copia_sin_verificar_no_se_puede_volver_a_restaurar() {
        let _g = entorno();
        crear_base_con_datos();
        let bueno = respaldar("bueno").unwrap();
        Connection::open(crate::db_sql::obtener_ruta_db())
            .unwrap()
            .execute_batch(
                "PRAGMA foreign_keys = OFF;
                 CREATE TABLE padre (id INTEGER PRIMARY KEY);
                 CREATE TABLE hijo (padre_id INTEGER REFERENCES padre(id));
                 INSERT INTO hijo VALUES (99);",
            )
            .unwrap();
        let r = restaurar(&bueno.file_name().unwrap().to_string_lossy()).unwrap();
        let sin_verificar = r.respaldo_de_seguridad.file_name().unwrap().to_string_lossy().to_string();

        assert!(restaurar(&sin_verificar).is_err(), "solo se restaura lo que se puede verificar");
        assert_eq!(valores(), vec!["uno", "dos"], "y la base no se tocó");
    }

    #[test]
    fn la_copia_de_seguridad_de_un_estado_sano_lleva_el_nombre_de_siempre() {
        let _g = entorno();
        crear_base_con_datos();
        let bueno = respaldar("bueno").unwrap();

        let r = restaurar(&bueno.file_name().unwrap().to_string_lossy()).unwrap();

        let nombre = r.respaldo_de_seguridad.file_name().unwrap().to_string_lossy().to_string();
        assert!(nombre.ends_with("_antes-de-restaurar.db"), "{nombre}");
    }
}
