//! Exportación legible de la base, con fines de auditoría.
//!
//! Un respaldo (`respaldo.rs`) es la evidencia completa, pero es una base SQLite: no se lee sin herramientas. Esto
//! escribe lo mismo en archivos que abre cualquier hoja de cálculo —un CSV por tabla— y un manifiesto con la huella
//! SHA-256 de cada uno, para poder demostrar después que un archivo no se alteró.
//!
//! * **Coherente:** todas las tablas se leen dentro de una sola transacción de lectura, así que la exportación es
//!   una foto de un instante y no una mezcla de dos.
//! * **Fiel:** cada valor sale tal como está guardado (ver [`valor_a_campo`]). No se redondea ni se reinterpreta nada:
//!   un archivo de auditoría que «arregla» lo que exporta ya no prueba lo que había.
//! * **Entera o nada:** se escribe en una carpeta provisional y solo al final se le da su nombre definitivo. Una
//!   exportación a medias no queda con apariencia de completa.
//! * **Fuera del repositorio:** vive en `~/.michelitos/auditoria/`, junto a los datos, y la poda de respaldos no la
//!   toca. Contiene datos reales en claro.
//!
//! Qué **no** es: una bitácora. Muestra el estado, no quién cambió qué ni cuándo.

use chrono::{DateTime, Local};
use rusqlite::types::ValueRef;
use rusqlite::Connection;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::{Path, PathBuf};

/// Versión del formato de la exportación. Cambia si cambia la forma de los archivos o del manifiesto.
pub const FORMATO: u32 = 1;

/// Marca de orden de bytes de UTF-8: sin ella Excel lee las tildes como Latin-1.
const MARCA_UTF8: &str = "\u{feff}";

/// Fin de línea de RFC 4180.
const FIN_DE_LINEA: &str = "\r\n";

const NOMBRE_MANIFIESTO: &str = "manifiesto.json";
const NOMBRE_HUELLAS: &str = "SHA256SUMS";
const NOMBRE_CAPITAL: &str = "capital.json";

#[derive(Debug)]
pub enum ErrorExportacion {
    /// La base no superó `PRAGMA integrity_check`: no se exporta algo que no se puede dar por bueno.
    BaseDanada { detalle: String },
    Fallo(String),
}

impl fmt::Display for ErrorExportacion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorExportacion::BaseDanada { detalle } => {
                write!(f, "No se exportó: la base no superó la verificación de integridad ({}).", detalle)
            }
            ErrorExportacion::Fallo(detalle) => write!(f, "No se pudo exportar para auditoría: {}", detalle),
        }
    }
}

impl std::error::Error for ErrorExportacion {}

fn fallo(e: impl fmt::Display) -> ErrorExportacion {
    ErrorExportacion::Fallo(e.to_string())
}

/// Un archivo de la exportación, como lo describe el manifiesto.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct ArchivoExportado {
    pub nombre: String,
    /// La tabla de la que sale; `None` para lo que no es una tabla (el capital).
    pub tabla: Option<String>,
    pub filas: Option<u64>,
    pub columnas: Vec<String>,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Serialize, Debug)]
struct Manifiesto<'a> {
    formato: u32,
    aplicacion: &'a str,
    version_de_la_aplicacion: &'a str,
    version_del_esquema: i64,
    generado: String,
    integridad: &'a str,
    notas: Vec<&'a str>,
    archivos: &'a [ArchivoExportado],
}

/// Lo que se le cuenta a quien pidió la exportación.
#[derive(Debug)]
pub struct Exportacion {
    pub carpeta: PathBuf,
    pub archivos: Vec<ArchivoExportado>,
    /// Huella del manifiesto. Anotada fuera de la carpeta, permite detectar que la carpeta entera se rehízo.
    pub huella_del_manifiesto: String,
}

impl Exportacion {
    pub fn filas(&self) -> u64 {
        self.archivos.iter().filter_map(|a| a.filas).sum()
    }
}

/// Lo que explica el manifiesto a quien lo abra dentro de unos años, sin la aplicación delante.
const NOTAS: &[&str] = &[
    "Un CSV por tabla: UTF-8 con marca de orden de bytes, separador coma, fin de línea CRLF (RFC 4180).",
    "Cada valor sale tal como está guardado. Las columnas terminadas en _c o _centavos están en centavos, y tasa_usd_dop de las fotos del patrimonio, en millonésimas.",
    "Un valor nulo es un campo vacío sin comillas; un texto vacío es \"\" con comillas.",
    "Las fechas de los datos están en dd/mm/aaaa, como las guarda la aplicación.",
    "Las filas van en el orden interno de la tabla (rowid).",
    "SHA256SUMS permite verificar los archivos con: shasum -a 256 -c SHA256SUMS",
    "capital.json es una copia del archivo del capital; se lee aparte de la base, no dentro de su transacción.",
    "Esta exportación muestra el estado en un instante. No es una bitácora de cambios.",
];

/// Dónde viven las exportaciones: dentro del directorio de la aplicación, nunca en el repositorio.
pub fn directorio_de_auditoria() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".michelitos/auditoria")
}

/// Un campo de CSV según RFC 4180: entre comillas si lleva coma, comillas, salto de línea o espacios en los bordes;
/// las comillas interiores se duplican. Un texto vacío va como `""` para no confundirse con un nulo.
pub fn campo_de_texto(texto: &str) -> String {
    let necesita_comillas = texto.is_empty()
        || texto.contains([',', '"', '\n', '\r'])
        || texto.starts_with(' ')
        || texto.ends_with(' ');
    if necesita_comillas {
        format!("\"{}\"", texto.replace('"', "\"\""))
    } else {
        texto.to_string()
    }
}

/// El valor de una celda, tal como está guardado.
///
/// * Nulo → campo vacío. * Entero → sus dígitos. * Real → la representación decimal más corta que vuelve al mismo
/// `f64` (la de Rust): no se redondea a centavos, porque la suma en coma flotante que haya en la base es justo lo que
/// una auditoría quiere ver. * Texto → [`campo_de_texto`]. * Binario → hexadecimal con el prefijo `x'…'`.
pub fn valor_a_campo(valor: ValueRef<'_>) -> String {
    match valor {
        ValueRef::Null => String::new(),
        ValueRef::Integer(n) => n.to_string(),
        ValueRef::Real(r) => r.to_string(),
        ValueRef::Text(t) => campo_de_texto(&String::from_utf8_lossy(t)),
        ValueRef::Blob(b) => format!("x'{}'", hexadecimal(b)),
    }
}

fn hexadecimal(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn sha256_de(bytes: &[u8]) -> String {
    hexadecimal(&Sha256::digest(bytes))
}

/// El identificador entre comillas dobles, como lo pide SQL: los nombres salen de `sqlite_master`, no de la interfaz,
/// pero se citan igual para que una tabla con un nombre raro no rompa la consulta.
fn citar(identificador: &str) -> String {
    format!("\"{}\"", identificador.replace('"', "\"\""))
}

/// El nombre de archivo de una tabla. Solo letras, dígitos y guion bajo: una tabla no decide una ruta.
fn nombre_de_archivo(tabla: &str) -> String {
    let limpio: String = tabla.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    format!("{}.csv", limpio)
}

/// El CSV de una tabla y cuántas filas lleva.
fn csv_de_tabla(conn: &Connection, tabla: &str) -> Result<(String, Vec<String>, u64), ErrorExportacion> {
    let consulta = format!("SELECT * FROM {} ORDER BY rowid;", citar(tabla));
    let mut sentencia = match conn.prepare(&consulta) {
        Ok(s) => s,
        // Una tabla sin rowid: se ordena por su primera columna.
        Err(_) => conn.prepare(&format!("SELECT * FROM {} ORDER BY 1;", citar(tabla))).map_err(fallo)?,
    };
    let columnas: Vec<String> = sentencia.column_names().iter().map(|c| c.to_string()).collect();

    let mut texto = String::from(MARCA_UTF8);
    texto.push_str(&columnas.iter().map(|c| campo_de_texto(c)).collect::<Vec<_>>().join(","));
    texto.push_str(FIN_DE_LINEA);

    let mut filas = 0u64;
    let mut cursor = sentencia.query([]).map_err(fallo)?;
    while let Some(fila) = cursor.next().map_err(fallo)? {
        let campos: Result<Vec<String>, _> = (0..columnas.len()).map(|i| fila.get_ref(i).map(valor_a_campo)).collect();
        texto.push_str(&campos.map_err(fallo)?.join(","));
        texto.push_str(FIN_DE_LINEA);
        filas += 1;
    }
    Ok((texto, columnas, filas))
}

fn escribir(carpeta: &Path, nombre: &str, contenido: &[u8]) -> Result<(u64, String), ErrorExportacion> {
    let ruta = carpeta.join(nombre);
    std::fs::write(&ruta, contenido).map_err(fallo)?;
    solo_lectura(&ruta)?;
    Ok((contenido.len() as u64, sha256_de(contenido)))
}

/// Un archivo de auditoría no se edita: se le quita el permiso de escritura para que no ocurra por descuido.
fn solo_lectura(ruta: &Path) -> Result<(), ErrorExportacion> {
    let mut permisos = std::fs::metadata(ruta).map_err(fallo)?.permissions();
    permisos.set_readonly(true);
    std::fs::set_permissions(ruta, permisos).map_err(fallo)
}

/// Exporta la base y el capital a una carpeta nueva y fechada dentro de [`directorio_de_auditoria`].
pub fn exportar() -> Result<Exportacion, ErrorExportacion> {
    let ruta_db = crate::db_sql::obtener_ruta_db();
    if !Path::new(&ruta_db).exists() {
        return Err(ErrorExportacion::Fallo("la base todavía no existe".into()));
    }
    let mut conn = crate::db_sql::obtener_conexion().map_err(fallo)?;
    let capital = PathBuf::from(crate::db_nosql::obtener_ruta_nosql("capital"));
    exportar_en(&mut conn, &capital, &directorio_de_auditoria(), Local::now())
}

/// El cuerpo de [`exportar`], con la conexión, el capital, el destino y el reloj dados (así se prueba).
pub fn exportar_en(
    conn: &mut Connection,
    capital: &Path,
    directorio: &Path,
    ahora: DateTime<Local>,
) -> Result<Exportacion, ErrorExportacion> {
    std::fs::create_dir_all(directorio).map_err(fallo)?;
    let nombre = format!("auditoria_{}", ahora.format("%Y-%m-%dT%H-%M-%S"));
    let definitiva = directorio.join(&nombre);
    if definitiva.exists() {
        return Err(ErrorExportacion::Fallo(format!("ya existe una exportación llamada {}", nombre)));
    }
    let provisional = directorio.join(format!("{}.parcial", nombre));
    if provisional.exists() {
        retirar(&provisional);
    }
    std::fs::create_dir_all(&provisional).map_err(fallo)?;

    match escribir_todo(conn, capital, &provisional, ahora) {
        Ok((archivos, huella_del_manifiesto)) => {
            std::fs::rename(&provisional, &definitiva).map_err(|e| {
                retirar(&provisional);
                fallo(e)
            })?;
            Ok(Exportacion { carpeta: definitiva, archivos, huella_del_manifiesto })
        }
        Err(e) => {
            retirar(&provisional);
            Err(e)
        }
    }
}

/// Borra una carpeta provisional. Sus archivos son de solo lectura, pero borrar depende del permiso de la carpeta.
fn retirar(carpeta: &Path) {
    let _ = std::fs::remove_dir_all(carpeta);
}

fn escribir_todo(
    conn: &mut Connection,
    capital: &Path,
    carpeta: &Path,
    ahora: DateTime<Local>,
) -> Result<(Vec<ArchivoExportado>, String), ErrorExportacion> {
    // Una sola transacción de lectura para todas las tablas: la exportación es una foto, no una película.
    let tx = crate::db_sql::transaccion(conn, false).map_err(fallo)?;

    let integridad: String = tx.query_row("PRAGMA integrity_check;", [], |f| f.get(0)).map_err(fallo)?;
    if integridad != "ok" {
        return Err(ErrorExportacion::BaseDanada { detalle: integridad });
    }
    let version_del_esquema: i64 = tx.query_row("PRAGMA user_version;", [], |f| f.get(0)).map_err(fallo)?;

    let tablas: Vec<String> = {
        let mut sentencia = tx.prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name;").map_err(fallo)?;
        let nombres = sentencia.query_map([], |f| f.get::<_, String>(0)).map_err(fallo)?;
        nombres.collect::<Result<_, _>>().map_err(fallo)?
    };

    let mut archivos = Vec::new();
    for tabla in &tablas {
        let (texto, columnas, filas) = csv_de_tabla(&tx, tabla)?;
        let nombre = nombre_de_archivo(tabla);
        if archivos.iter().any(|a: &ArchivoExportado| a.nombre == nombre) {
            return Err(ErrorExportacion::Fallo(format!("dos tablas darían el mismo archivo ({})", nombre)));
        }
        let (bytes, sha256) = escribir(carpeta, &nombre, texto.as_bytes())?;
        archivos.push(ArchivoExportado { nombre, tabla: Some(tabla.clone()), filas: Some(filas), columnas, bytes, sha256 });
    }
    drop(tx);

    // El capital vive fuera de la base. Si todavía no existe, no se inventa uno vacío: simplemente no va.
    if capital.exists() {
        let contenido = std::fs::read(capital).map_err(fallo)?;
        let (bytes, sha256) = escribir(carpeta, NOMBRE_CAPITAL, &contenido)?;
        archivos.push(ArchivoExportado {
            nombre: NOMBRE_CAPITAL.to_string(),
            tabla: None,
            filas: None,
            columnas: Vec::new(),
            bytes,
            sha256,
        });
    }

    let manifiesto = Manifiesto {
        formato: FORMATO,
        aplicacion: "MiChelitos",
        version_de_la_aplicacion: env!("CARGO_PKG_VERSION"),
        version_del_esquema,
        generado: ahora.to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
        integridad: "ok",
        notas: NOTAS.to_vec(),
        archivos: &archivos,
    };
    let mut texto = serde_json::to_string_pretty(&manifiesto).map_err(fallo)?;
    texto.push('\n');
    let (_, huella_del_manifiesto) = escribir(carpeta, NOMBRE_MANIFIESTO, texto.as_bytes())?;

    // El formato de `shasum`: huella, dos espacios, nombre. El manifiesto va incluido.
    let mut huellas = String::new();
    for a in &archivos {
        huellas.push_str(&format!("{}  {}\n", a.sha256, a.nombre));
    }
    huellas.push_str(&format!("{}  {}\n", huella_del_manifiesto, NOMBRE_MANIFIESTO));
    escribir(carpeta, NOMBRE_HUELLAS, huellas.as_bytes())?;

    Ok((archivos, huella_del_manifiesto))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn carpeta(nombre: &str) -> PathBuf {
        let raiz = std::env::temp_dir().join(format!("michelitos-exportacion-{}-{}", std::process::id(), nombre));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(&raiz).unwrap();
        raiz
    }

    fn base(raiz: &Path) -> Connection {
        let c = Connection::open(raiz.join("base.db")).unwrap();
        c.execute_batch(
            "PRAGMA user_version = 7;
             CREATE TABLE movimientos (id INTEGER PRIMARY KEY, descripcion TEXT, monto REAL, monto_c INTEGER, nota TEXT, sello BLOB);
             INSERT INTO movimientos VALUES (1, 'Colmado, \"La Esquina\"', 0.1, 10, NULL, x'00ff');
             INSERT INTO movimientos VALUES (2, 'dos
líneas', 1234.5, 123450, '', NULL);
             INSERT INTO movimientos VALUES (3, 'Año: señal ñ', -7.25, -725, ' borde ', NULL);
             CREATE TABLE vacia (id INTEGER PRIMARY KEY, nombre TEXT);
             CREATE TABLE sin_rowid (clave TEXT PRIMARY KEY, valor INTEGER) WITHOUT ROWID;
             INSERT INTO sin_rowid VALUES ('b', 2), ('a', 1);",
        )
        .unwrap();
        c
    }

    fn momento() -> DateTime<Local> {
        Local.with_ymd_and_hms(2031, 2, 3, 4, 5, 6).unwrap()
    }

    fn exportar_prueba(nombre: &str) -> (PathBuf, Exportacion) {
        let raiz = carpeta(nombre);
        let mut conn = base(&raiz);
        let capital = raiz.join("capital.json");
        std::fs::write(&capital, b"{\"efectivo\": \"12.34\"}").unwrap();
        let e = exportar_en(&mut conn, &capital, &raiz.join("auditoria"), momento()).expect("exportar");
        (raiz, e)
    }

    #[test]
    fn el_campo_de_texto_sigue_rfc_4180() {
        assert_eq!(campo_de_texto("simple"), "simple");
        assert_eq!(campo_de_texto("con, coma"), "\"con, coma\"");
        assert_eq!(campo_de_texto("dice \"hola\""), "\"dice \"\"hola\"\"\"");
        assert_eq!(campo_de_texto("dos\nlíneas"), "\"dos\nlíneas\"");
        assert_eq!(campo_de_texto("retorno\r"), "\"retorno\r\"");
        assert_eq!(campo_de_texto(" borde"), "\" borde\"");
        assert_eq!(campo_de_texto("borde "), "\"borde \"");
        assert_eq!(campo_de_texto("entre medias"), "entre medias");
    }

    #[test]
    fn un_nulo_y_un_texto_vacio_no_se_confunden() {
        assert_eq!(valor_a_campo(ValueRef::Null), "");
        assert_eq!(valor_a_campo(ValueRef::Text(b"")), "\"\"");
    }

    #[test]
    fn cada_valor_sale_como_esta_guardado() {
        assert_eq!(valor_a_campo(ValueRef::Integer(-725)), "-725");
        assert_eq!(valor_a_campo(ValueRef::Real(1234.5)), "1234.5");
        // La suma en coma flotante no se maquilla: es lo que hay en la base.
        assert_eq!(valor_a_campo(ValueRef::Real(0.1 + 0.2)), "0.30000000000000004");
        assert_eq!(valor_a_campo(ValueRef::Blob(&[0, 255, 16])), "x'00ff10'");
    }

    #[test]
    fn la_huella_es_sha_256() {
        // Vector de prueba conocido de SHA-256.
        assert_eq!(sha256_de(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(sha256_de(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }

    #[test]
    fn el_csv_lleva_marca_cabecera_y_una_linea_por_fila() {
        let (_raiz, e) = exportar_prueba("csv");
        let texto = std::fs::read_to_string(e.carpeta.join("movimientos.csv")).unwrap();
        let esperado = "\u{feff}id,descripcion,monto,monto_c,nota,sello\r\n\
                        1,\"Colmado, \"\"La Esquina\"\"\",0.1,10,,x'00ff'\r\n\
                        2,\"dos\nlíneas\",1234.5,123450,\"\",\r\n\
                        3,Año: señal ñ,-7.25,-725,\" borde \",\r\n";
        assert_eq!(texto, esperado);
    }

    #[test]
    fn una_tabla_vacia_exporta_solo_su_cabecera() {
        let (_raiz, e) = exportar_prueba("vacia");
        assert_eq!(std::fs::read_to_string(e.carpeta.join("vacia.csv")).unwrap(), "\u{feff}id,nombre\r\n");
        let a = e.archivos.iter().find(|a| a.nombre == "vacia.csv").unwrap();
        assert_eq!((a.filas, a.columnas.clone()), (Some(0), vec!["id".to_string(), "nombre".to_string()]));
    }

    #[test]
    fn una_tabla_sin_rowid_tambien_se_exporta_y_en_orden() {
        let (_raiz, e) = exportar_prueba("sinrowid");
        assert_eq!(std::fs::read_to_string(e.carpeta.join("sin_rowid.csv")).unwrap(), "\u{feff}clave,valor\r\na,1\r\nb,2\r\n");
    }

    #[test]
    fn la_carpeta_lleva_la_fecha_y_no_queda_nada_provisional() {
        let (raiz, e) = exportar_prueba("carpeta");
        assert_eq!(e.carpeta, raiz.join("auditoria").join("auditoria_2031-02-03T04-05-06"));
        let nombres: Vec<String> =
            std::fs::read_dir(raiz.join("auditoria")).unwrap().map(|d| d.unwrap().file_name().to_string_lossy().to_string()).collect();
        assert_eq!(nombres, vec!["auditoria_2031-02-03T04-05-06".to_string()]);
    }

    #[test]
    fn el_manifiesto_describe_cada_archivo_con_su_huella_real() {
        let (_raiz, e) = exportar_prueba("manifiesto");
        let crudo = std::fs::read(e.carpeta.join(NOMBRE_MANIFIESTO)).unwrap();
        assert_eq!(sha256_de(&crudo), e.huella_del_manifiesto);
        let m: serde_json::Value = serde_json::from_slice(&crudo).unwrap();
        assert_eq!(m["formato"], FORMATO);
        assert_eq!(m["version_del_esquema"], 7);
        assert_eq!(m["version_de_la_aplicacion"], env!("CARGO_PKG_VERSION"));
        assert_eq!(m["integridad"], "ok");
        assert!(m["generado"].as_str().unwrap().starts_with("2031-02-03T04:05:06"));
        let archivos = m["archivos"].as_array().unwrap();
        assert_eq!(
            archivos.iter().map(|a| a["nombre"].as_str().unwrap()).collect::<Vec<_>>(),
            vec!["movimientos.csv", "sin_rowid.csv", "vacia.csv", "capital.json"]
        );
        for a in archivos {
            let contenido = std::fs::read(e.carpeta.join(a["nombre"].as_str().unwrap())).unwrap();
            assert_eq!(a["sha256"], sha256_de(&contenido), "huella de {}", a["nombre"]);
            assert_eq!(a["bytes"], contenido.len() as u64);
        }
        assert_eq!(archivos[0]["filas"], 3);
        assert_eq!(archivos[3]["filas"], serde_json::Value::Null);
        assert_eq!(e.filas(), 5);
    }

    #[test]
    fn las_huellas_sueltas_cubren_todos_los_archivos_y_el_manifiesto() {
        let (_raiz, e) = exportar_prueba("huellas");
        let texto = std::fs::read_to_string(e.carpeta.join(NOMBRE_HUELLAS)).unwrap();
        let lineas: Vec<&str> = texto.lines().collect();
        assert_eq!(lineas.len(), e.archivos.len() + 1);
        for linea in &lineas {
            let (huella, nombre) = linea.split_once("  ").expect("huella, dos espacios, nombre");
            assert_eq!(huella, sha256_de(&std::fs::read(e.carpeta.join(nombre)).unwrap()), "{nombre}");
        }
        assert!(lineas.last().unwrap().ends_with("  manifiesto.json"));
    }

    #[test]
    fn el_capital_se_copia_byte_a_byte_y_si_falta_no_se_inventa() {
        let (_raiz, e) = exportar_prueba("capital");
        assert_eq!(std::fs::read(e.carpeta.join(NOMBRE_CAPITAL)).unwrap(), b"{\"efectivo\": \"12.34\"}");

        let raiz = carpeta("sincapital");
        let mut conn = base(&raiz);
        let e = exportar_en(&mut conn, &raiz.join("no-existe.json"), &raiz.join("auditoria"), momento()).unwrap();
        assert!(!e.carpeta.join(NOMBRE_CAPITAL).exists());
        assert!(e.archivos.iter().all(|a| a.nombre != NOMBRE_CAPITAL));
    }

    #[test]
    fn los_archivos_quedan_de_solo_lectura() {
        let (_raiz, e) = exportar_prueba("sololectura");
        for nombre in ["movimientos.csv", NOMBRE_CAPITAL, NOMBRE_MANIFIESTO, NOMBRE_HUELLAS] {
            assert!(std::fs::metadata(e.carpeta.join(nombre)).unwrap().permissions().readonly(), "{nombre}");
        }
    }

    #[test]
    fn no_se_pisa_una_exportacion_del_mismo_segundo() {
        let (raiz, e) = exportar_prueba("repetida");
        let antes = std::fs::read(e.carpeta.join(NOMBRE_MANIFIESTO)).unwrap();
        let mut conn = Connection::open(raiz.join("base.db")).unwrap();
        let r = exportar_en(&mut conn, &raiz.join("capital.json"), &raiz.join("auditoria"), momento());
        assert!(matches!(r, Err(ErrorExportacion::Fallo(ref d)) if d.contains("ya existe")), "{r:?}");
        assert_eq!(std::fs::read(e.carpeta.join(NOMBRE_MANIFIESTO)).unwrap(), antes);
    }

    #[test]
    fn si_falla_a_medias_no_queda_carpeta_alguna() {
        let raiz = carpeta("fallo");
        let mut conn = base(&raiz);
        // Un «capital» que es un directorio: se puede comprobar que existe pero no leer como archivo.
        let capital = raiz.join("capital-es-carpeta");
        std::fs::create_dir_all(&capital).unwrap();
        let r = exportar_en(&mut conn, &capital, &raiz.join("auditoria"), momento());
        assert!(r.is_err());
        assert_eq!(std::fs::read_dir(raiz.join("auditoria")).unwrap().count(), 0, "no debe quedar ni la provisional");
    }

    #[test]
    fn la_exportacion_no_escribe_en_la_base() {
        let raiz = carpeta("nolee");
        let mut conn = base(&raiz);
        let antes = sha256_de(&std::fs::read(raiz.join("base.db")).unwrap());
        exportar_en(&mut conn, &raiz.join("capital.json"), &raiz.join("auditoria"), momento()).unwrap();
        drop(conn);
        assert_eq!(sha256_de(&std::fs::read(raiz.join("base.db")).unwrap()), antes);
    }

    #[test]
    fn el_nombre_de_archivo_no_sale_de_la_carpeta() {
        assert_eq!(nombre_de_archivo("gastos"), "gastos.csv");
        assert_eq!(nombre_de_archivo("../fuera"), "___fuera.csv");
        assert_eq!(nombre_de_archivo("con espacio"), "con_espacio.csv");
    }
}
