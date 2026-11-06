//! Adaptador SQLite del puerto de catálogos (categorías y clientes).
//!
//! Como el de gastos, opera **sobre una transacción ya abierta**: quien la abre la confirma.

use crate::puertos::repositorios::*;
use rusqlite::{OptionalExtension, Transaction};

pub struct CatalogosSqlite<'a> {
    tx: &'a Transaction<'a>,
}

impl<'a> CatalogosSqlite<'a> {
    pub fn nuevo(tx: &'a Transaction<'a>) -> Self {
        CatalogosSqlite { tx }
    }
}

fn fallo(e: rusqlite::Error) -> ErrorAlmacen {
    ErrorAlmacen::Fallo(e.to_string())
}

impl AlmacenCatalogos for CatalogosSqlite<'_> {
    fn categorias(&self) -> Result<Vec<CategoriaGuardada>, ErrorAlmacen> {
        let mut stmt = self.tx.prepare("SELECT id, nombre, fija FROM categorias ORDER BY nombre ASC;").map_err(fallo)?;
        let filas = stmt
            .query_map([], |r| Ok(CategoriaGuardada { id: r.get(0)?, nombre: r.get(1)?, fija: r.get::<_, i64>(2)? == 1 }))
            .map_err(fallo)?;
        filas.collect::<Result<Vec<_>, _>>().map_err(fallo)
    }

    fn categoria_existe(&self, nombre: &str) -> Result<bool, ErrorAlmacen> {
        let n: i64 = self
            .tx
            .query_row("SELECT COUNT(*) FROM categorias WHERE LOWER(nombre) = LOWER(?);", [nombre], |r| r.get(0))
            .map_err(fallo)?;
        Ok(n > 0)
    }

    fn insertar_categoria(&mut self, nombre: &str) -> Result<i64, ErrorAlmacen> {
        self.tx.execute("INSERT INTO categorias (nombre) VALUES (?);", [nombre]).map_err(fallo)?;
        Ok(self.tx.last_insert_rowid())
    }

    fn nombre_de_categoria(&self, id: i64) -> Result<String, ErrorAlmacen> {
        self.tx
            .query_row("SELECT nombre FROM categorias WHERE id = ?;", [id], |r| r.get::<_, String>(0))
            .optional()
            .map_err(fallo)?
            .ok_or(ErrorAlmacen::NoEncontrado { entidad: "la categoría", id })
    }

    fn gastos_de_categoria(&self, id: i64) -> Result<i64, ErrorAlmacen> {
        self.tx
            .query_row("SELECT COUNT(*) FROM gastos WHERE categoria_id = ?;", [id], |r| r.get(0))
            .map_err(fallo)
    }

    fn eliminar_categoria(&mut self, id: i64) -> Result<(), ErrorAlmacen> {
        self.tx.execute("DELETE FROM categorias WHERE id = ?;", [id]).map_err(fallo)?;
        Ok(())
    }

    fn marcar_categoria_fija(&mut self, id: i64, fija: bool) -> Result<(), ErrorAlmacen> {
        let cambiadas = self
            .tx
            .execute("UPDATE categorias SET fija = ? WHERE id = ?;", rusqlite::params![fija as i64, id])
            .map_err(fallo)?;
        if cambiadas == 0 {
            return Err(ErrorAlmacen::NoEncontrado { entidad: "la categoría", id });
        }
        Ok(())
    }

    fn clientes(&self) -> Result<Vec<ClienteGuardado>, ErrorAlmacen> {
        let mut stmt = self.tx.prepare("SELECT id, rnc, nombre FROM clientes ORDER BY nombre ASC;").map_err(fallo)?;
        let filas = stmt
            .query_map([], |r| Ok(ClienteGuardado { id: r.get(0)?, rnc: r.get(1)?, nombre: r.get(2)? }))
            .map_err(fallo)?;
        filas.collect::<Result<Vec<_>, _>>().map_err(fallo)
    }

    fn rnc_registrado(&self, rnc: &str) -> Result<bool, ErrorAlmacen> {
        let n: i64 = self
            .tx
            .query_row("SELECT COUNT(*) FROM clientes WHERE rnc = ?;", [rnc], |r| r.get(0))
            .map_err(fallo)?;
        Ok(n > 0)
    }

    fn insertar_cliente(&mut self, rnc: &str, nombre: &str) -> Result<i64, ErrorAlmacen> {
        self.tx.execute("INSERT INTO clientes (rnc, nombre) VALUES (?, ?);", [rnc, nombre]).map_err(fallo)?;
        Ok(self.tx.last_insert_rowid())
    }

    fn facturas_de_cliente(&self, id: i64) -> Result<i64, ErrorAlmacen> {
        self.tx
            .query_row("SELECT COUNT(*) FROM ingresos WHERE cliente_id = ?;", [id], |r| r.get(0))
            .map_err(fallo)
    }

    fn eliminar_cliente(&mut self, id: i64) -> Result<(), ErrorAlmacen> {
        self.tx.execute("DELETE FROM clientes WHERE id = ?;", [id]).map_err(fallo)?;
        Ok(())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use rusqlite::Connection;

    fn base() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::db_sql::crear_esquema(&mut conn).unwrap();
        conn
    }

    #[test]
    fn marcar_y_desmarcar_persisten_y_una_nueva_nace_variable() {
        let mut conn = base();
        let tx = conn.transaction().unwrap();
        let mut a = CatalogosSqlite::nuevo(&tx);
        assert!(a.categorias().unwrap().iter().all(|c| !c.fija), "las sembradas no vienen marcadas");
        let id = a.insertar_categoria("Gimnasio inventado").unwrap();
        assert!(!a.categorias().unwrap().iter().find(|c| c.id == id).unwrap().fija);
        a.marcar_categoria_fija(id, true).unwrap();
        let marcadas: Vec<_> = a.categorias().unwrap().into_iter().filter(|c| c.fija).collect();
        assert_eq!(marcadas.len(), 1);
        assert_eq!(marcadas[0].id, id);
        a.marcar_categoria_fija(id, false).unwrap();
        assert!(a.categorias().unwrap().iter().all(|c| !c.fija));
    }

    #[test]
    fn marcar_una_categoria_inexistente_es_no_encontrado() {
        let mut conn = base();
        let tx = conn.transaction().unwrap();
        let mut a = CatalogosSqlite::nuevo(&tx);
        let e = a.marcar_categoria_fija(9_999, true).unwrap_err();
        assert!(matches!(e, ErrorAlmacen::NoEncontrado { id: 9_999, .. }), "{e:?}");
    }
}
