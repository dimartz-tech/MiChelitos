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

    if let Err(e) = verificar(&destino) {
        // Un respaldo que no se puede abrir es peor que ninguno: da confianza
        // falsa. Se retira.
        let _ = std::fs::remove_file(&destino);
        return Err(e);
    }

    podar(&directorio);
    Ok(destino)
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
    }
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
}
