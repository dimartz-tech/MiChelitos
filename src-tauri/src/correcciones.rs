//! Casos de corrección: qué se borró, por qué, y con qué número.
//!
//! **Medida temporal.** La solución buena son los asientos de compensación de
//! la Fase 8: un movimiento que se anula deja su contrario y el original
//! sobrevive. Mientras eso no exista, borrar destruye el rastro; esto deja
//! constancia de qué se destruyó y por qué.
//!
//! El segundo propósito es tan importante como el primero: **exigir un motivo
//! escrito es fricción deliberada**. Una corrección que cuesta un párrafo se
//! piensa dos veces, y así la mayoría de estas situaciones se evitan antes de
//! ocurrir.

use chrono::{Datelike, Local};
use rusqlite::Transaction;

/// Lo que se anota de una corrección.
pub struct Correccion<'a> {
    /// Qué clase de movimiento se borra: «gasto», «factura», «abono»…
    pub tipo: &'a str,
    pub referencia_id: i64,
    /// Cómo se identificaba el movimiento, para poder reconocerlo después.
    pub descripcion: String,
    pub importe: Option<f64>,
    pub divisa: Option<String>,
    pub motivo: &'a str,
}

/// Anota la corrección y devuelve su número de caso.
///
/// Se llama **antes** de borrar: si el borrado falla, la transacción se
/// deshace entera y no queda un caso huérfano; si el registro falla, no se
/// borra nada.
pub fn registrar(tx: &Transaction, c: Correccion<'_>) -> Result<String, String> {
    // La regla del motivo es del dominio; aquí solo se anota.
    let motivo = crate::dominio::correccion::motivo_de_correccion(c.motivo).map_err(|e| e.to_string())?;
    insertar(tx, &c, &motivo)
}

/// Anota el caso con un motivo **ya validado** y devuelve su número. Lo usa el adaptador del puerto de
/// correcciones, cuyo caso de uso valida el motivo antes de llamar.
pub fn insertar(tx: &Transaction, c: &Correccion<'_>, motivo: &str) -> Result<String, String> {
    let numero = siguiente_numero(tx)?;
    let hoy = Local::now().format("%d/%m/%Y").to_string();

    tx.execute(
        "INSERT INTO correcciones
             (numero_caso, fecha, tipo, referencia_id, descripcion, importe, divisa, motivo)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?);",
        (
            &numero,
            &hoy,
            c.tipo,
            c.referencia_id,
            &c.descripcion,
            c.importe,
            &c.divisa,
            motivo,
        ),
    )
    .map_err(|e| e.to_string())?;

    Ok(numero)
}

/// Un número por año: `COR-2026-0001`.
///
/// La secuencia sale del **mayor número vivo del año**, con una limitación
/// que conviene decir en vez de suponer resuelta: **si se borrara el último
/// caso, el siguiente reutilizaría su número**. Dos correcciones distintas
/// compartirían identificador, y el rastro dejaría de servir.
///
/// No se blinda porque blindarlo bien —una secuencia aparte, o derivar el
/// número de un identificador monótono— es trabajo que esta medida temporal
/// no justifica. Lo que sí importa: **los casos no se borran**. La tabla no
/// tiene comando de borrado precisamente por eso, y si algún día lo tuviera,
/// esta función habría que rehacerla antes.
fn siguiente_numero(tx: &Transaction) -> Result<String, String> {
    let anio = Local::now().year();
    let prefijo = format!("COR-{}-", anio);

    let ultimo: Option<String> = tx
        .query_row(
            "SELECT MAX(numero_caso) FROM correcciones WHERE numero_caso LIKE ?;",
            [format!("{}%", prefijo)],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    let secuencia = ultimo
        .and_then(|n| n.rsplit('-').next().and_then(|s| s.parse::<u32>().ok()))
        .unwrap_or(0)
        + 1;

    Ok(format!("{}{:04}", prefijo, secuencia))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn base() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(
            "CREATE TABLE correcciones (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 numero_caso TEXT UNIQUE NOT NULL,
                 fecha TEXT NOT NULL,
                 tipo TEXT NOT NULL,
                 referencia_id INTEGER NOT NULL,
                 descripcion TEXT NOT NULL,
                 importe REAL,
                 divisa TEXT,
                 motivo TEXT NOT NULL
             );",
        )
        .unwrap();
        c
    }

    fn una(motivo: &str) -> Correccion<'_> {
        Correccion {
            tipo: "gasto",
            referencia_id: 1,
            descripcion: "Compra de ejemplo".into(),
            importe: Some(1_000.0),
            divisa: Some("DOP".into()),
            motivo,
        }
    }

    #[test]
    fn el_primer_caso_del_ano_abre_la_secuencia() {
        let mut c = base();
        let tx = c.transaction().unwrap();

        let n = registrar(&tx, una("Se registró dos veces la misma compra")).unwrap();

        assert!(n.ends_with("-0001"), "número obtenido: {n}");
        assert!(n.starts_with("COR-"), "número obtenido: {n}");
    }

    #[test]
    fn los_casos_se_numeran_en_orden_sin_repetirse() {
        let mut c = base();
        let tx = c.transaction().unwrap();

        let a = registrar(&tx, una("Se registró dos veces la misma compra")).unwrap();
        let b = registrar(&tx, una("El importe estaba mal tecleado desde el inicio")).unwrap();

        assert_ne!(a, b);
        assert!(b.ends_with("-0002"), "número obtenido: {b}");
    }

    #[test]
    fn borrar_el_ultimo_caso_haria_reutilizar_su_numero() {
        // **Limitación conocida, fijada por prueba en vez de supuesta
        // resuelta.** La secuencia sale del mayor número vivo, así que borrar
        // el último lo devuelve a circulación. Por eso los casos no se borran
        // y no hay comando que lo permita; si algún día lo hubiera, esta
        // prueba es la que obliga a rehacer la numeración primero.
        let mut c = base();
        let tx = c.transaction().unwrap();
        registrar(&tx, una("Se registró dos veces la misma compra")).unwrap();
        let segundo = registrar(&tx, una("El importe estaba mal tecleado")).unwrap();
        tx.execute("DELETE FROM correcciones WHERE numero_caso = ?;", [&segundo]).unwrap();

        let tercero = registrar(&tx, una("Otra corrección posterior distinta")).unwrap();

        assert_eq!(tercero, segundo, "reutiliza el número: por eso no se borran casos");
    }

    #[test]
    fn un_motivo_de_tramite_se_rechaza() {
        // «error» pasa cualquier comprobación de «no vacío» y no explica nada.
        let mut c = base();
        let tx = c.transaction().unwrap();

        for flojo in ["", "  ", "error", "ok", "me equivoqué"] {
            assert!(registrar(&tx, una(flojo)).is_err(), "aceptó «{flojo}»");
        }
    }

    #[test]
    fn un_motivo_que_explica_se_acepta_y_se_guarda_sin_espacios_sobrantes() {
        let mut c = base();
        let tx = c.transaction().unwrap();

        registrar(&tx, una("   Se registró dos veces la misma compra   ")).unwrap();

        let guardado: String = tx
            .query_row("SELECT motivo FROM correcciones;", [], |r| r.get(0))
            .unwrap();
        assert_eq!(guardado, "Se registró dos veces la misma compra");
    }

    #[test]
    fn el_caso_conserva_de_que_movimiento_se_trataba() {
        // Sin esto el registro diría que se borró algo, pero no qué: el
        // movimiento ya no existe para consultarlo.
        let mut c = base();
        let tx = c.transaction().unwrap();

        registrar(&tx, una("Se registró dos veces la misma compra")).unwrap();

        let (tipo, desc, importe): (String, String, f64) = tx
            .query_row(
                "SELECT tipo, descripcion, importe FROM correcciones;",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(tipo, "gasto");
        assert_eq!(desc, "Compra de ejemplo");
        assert_eq!(importe, 1_000.0);
    }
}
