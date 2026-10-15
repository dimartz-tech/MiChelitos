//! Comandos de categorías y clientes.

use serde::{Deserialize, Serialize};

use super::ayudantes::{con_catalogos, lee_catalogos};
use crate::aplicacion;
use crate::puertos::repositorios::AlmacenCatalogos;

#[derive(Serialize, Deserialize, Debug)]
pub struct Cliente {
    pub id: i64,
    pub rnc: String,
    pub nombre: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Categoria {
    pub id: i64,
    pub nombre: String,
}

#[tauri::command]
pub fn obtener_categorias() -> Result<Vec<Categoria>, String> {
    lee_catalogos(|a| {
        Ok(a.categorias()?.into_iter().map(|c| Categoria { id: c.id, nombre: c.nombre }).collect())
    })
}

#[tauri::command]
pub fn crear_categoria(nombre: String) -> Result<Categoria, String> {
    con_catalogos(|a| {
        let c = aplicacion::catalogos::crear_categoria(&nombre, a)?;
        Ok(Categoria { id: c.id, nombre: c.nombre })
    })
}

#[tauri::command]
pub fn eliminar_categoria(id: i64) -> Result<(), String> {
    con_catalogos(|a| Ok(aplicacion::catalogos::eliminar_categoria(id, a)?))
}

#[tauri::command]
pub fn obtener_clientes() -> Result<Vec<Cliente>, String> {
    lee_catalogos(|a| {
        Ok(a.clientes()?.into_iter().map(|c| Cliente { id: c.id, rnc: c.rnc, nombre: c.nombre }).collect())
    })
}

#[tauri::command]
pub fn crear_cliente(rnc: String, nombre: String) -> Result<Cliente, String> {
    con_catalogos(|a| {
        let c = aplicacion::catalogos::crear_cliente(&rnc, &nombre, a)?;
        Ok(Cliente { id: c.id, rnc: c.rnc, nombre: c.nombre })
    })
}

#[tauri::command]
pub fn eliminar_cliente(id: i64) -> Result<(), String> {
    con_catalogos(|a| Ok(aplicacion::catalogos::eliminar_cliente(id, a)?))
}
