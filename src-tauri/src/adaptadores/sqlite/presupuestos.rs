//! Lectura SQLite de los presupuestos mensuales por categoría. **Solo lee.**
//!
//! La escritura (fijar, quitar) vive en `catalogos.rs`, junto a las categorías. El monto es un `INTEGER` en centavos: no
//! hay ninguna suma en SQL (la guarda de `sumas_en_centavos` prohíbe `SUM(` sobre importes); las sumas se hacen en el
//! dominio.

use super::gastos::AlmacenSqlite;
use crate::puertos::repositorios::*;

impl ConsultaDePresupuestos for AlmacenSqlite<'_> {
    fn presupuestos(&self) -> Result<Vec<Presupuesto>, ErrorAlmacen> {
        let fallo = |e: rusqlite::Error| ErrorAlmacen::Fallo(e.to_string());
        let mut stmt = self
            .tx
            .prepare(
                "SELECT p.categoria_id, c.nombre, c.fija, p.monto_centavos
                 FROM presupuestos p JOIN categorias c ON c.id = p.categoria_id
                 ORDER BY c.nombre ASC;",
            )
            .map_err(fallo)?;
        let filas = stmt
            .query_map([], |r| {
                Ok(Presupuesto {
                    categoria_id: r.get(0)?,
                    categoria_nombre: r.get(1)?,
                    fija: r.get::<_, i64>(2)? == 1,
                    centavos: r.get(3)?,
                })
            })
            .map_err(fallo)?;
        filas.collect::<Result<Vec<_>, _>>().map_err(fallo)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::adaptadores::sqlite::catalogos::CatalogosSqlite;
    use crate::db_sql;
    use rusqlite::Connection;

    fn base() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = ON;", []).unwrap();
        db_sql::crear_esquema(&mut conn).unwrap();
        conn
    }

    #[test]
    fn fijar_lee_reemplaza_y_quitar_borra() {
        let mut conn = base();
        let tx = conn.transaction().unwrap();
        let mut c = CatalogosSqlite::nuevo(&tx);
        let id = c.insertar_categoria("Gimnasio inventado").unwrap();
        assert_eq!(c.presupuesto_de_categoria(id).unwrap(), None);
        c.fijar_presupuesto(id, 123_457).unwrap();
        assert_eq!(c.presupuesto_de_categoria(id).unwrap(), Some(123_457));
        c.fijar_presupuesto(id, 9_001).unwrap();
        assert_eq!(c.presupuesto_de_categoria(id).unwrap(), Some(9_001), "reemplaza");
        let almacen = AlmacenSqlite::nuevo(&tx);
        let v = almacen.presupuestos().unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!((v[0].categoria_id, v[0].categoria_nombre.as_str(), v[0].centavos, v[0].fija), (id, "Gimnasio inventado", 9_001, false));
        assert!(c.quitar_presupuesto(id).unwrap());
        assert!(!c.quitar_presupuesto(id).unwrap());
        assert!(almacen.presupuestos().unwrap().is_empty());
    }

    #[test]
    fn una_categoria_inexistente_es_no_encontrado_y_no_escribe() {
        let mut conn = base();
        let tx = conn.transaction().unwrap();
        let mut c = CatalogosSqlite::nuevo(&tx);
        assert!(matches!(c.fijar_presupuesto(9_999, 100), Err(ErrorAlmacen::NoEncontrado { id: 9_999, .. })));
        assert!(matches!(c.quitar_presupuesto(9_999), Err(ErrorAlmacen::NoEncontrado { id: 9_999, .. })));
        assert!(matches!(c.presupuesto_de_categoria(9_999), Err(ErrorAlmacen::NoEncontrado { id: 9_999, .. })));
        assert!(AlmacenSqlite::nuevo(&tx).presupuestos().unwrap().is_empty());
    }

    #[test]
    fn eliminar_la_categoria_borra_su_presupuesto_y_deja_el_de_otra() {
        let mut conn = base();
        let tx = conn.transaction().unwrap();
        let mut c = CatalogosSqlite::nuevo(&tx);
        let a = c.insertar_categoria("Uno inventado").unwrap();
        let b = c.insertar_categoria("Dos inventado").unwrap();
        c.fijar_presupuesto(a, 111).unwrap();
        c.fijar_presupuesto(b, 222).unwrap();
        c.eliminar_categoria(a).unwrap();
        let v = AlmacenSqlite::nuevo(&tx).presupuestos().unwrap();
        assert_eq!(v.iter().map(|p| (p.categoria_id, p.centavos)).collect::<Vec<_>>(), vec![(b, 222)]);
    }

    #[test]
    fn la_lectura_trae_la_marca_fija_y_ordena_por_nombre() {
        let mut conn = base();
        let tx = conn.transaction().unwrap();
        let mut c = CatalogosSqlite::nuevo(&tx);
        let z = c.insertar_categoria("Zeta inventada").unwrap();
        let a = c.insertar_categoria("Alfa inventada").unwrap();
        c.marcar_categoria_fija(z, true).unwrap();
        c.fijar_presupuesto(z, 5).unwrap();
        c.fijar_presupuesto(a, 6).unwrap();
        let v = AlmacenSqlite::nuevo(&tx).presupuestos().unwrap();
        assert_eq!(v.iter().map(|p| (p.categoria_nombre.as_str(), p.fija)).collect::<Vec<_>>(), vec![("Alfa inventada", false), ("Zeta inventada", true)]);
    }
}
