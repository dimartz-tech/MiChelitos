//! Comandos de categorías y clientes.

use serde::{Deserialize, Serialize};

use super::ayudantes::{con_catalogos, lee_catalogos};
use crate::aplicacion;
use crate::dominio::catalogo::uso_del_sistema;
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
    /// Marca de gasto fijo/recurrente que elige el titular (una categoría nueva es variable).
    pub fija: bool,
    /// Si el sistema la busca por nombre (y por eso no se renombra ni se fusiona como origen): para qué la usa.
    /// `null` en las categorías corrientes.
    pub motivo_sistema: Option<String>,
}

fn categoria(id: i64, nombre: String, fija: bool) -> Categoria {
    let motivo_sistema = uso_del_sistema(&nombre).map(str::to_string);
    Categoria { id, nombre, fija, motivo_sistema }
}

/// Lo que la interfaz muestra antes de confirmar una fusión de categorías. Fechas en `dd/mm/aaaa`.
#[derive(Serialize, Deserialize, Debug)]
pub struct VistaPreviaFusion {
    pub origen_id: i64,
    pub origen_nombre: String,
    pub destino_id: i64,
    pub destino_nombre: String,
    pub gastos: i64,
    pub desde: Option<String>,
    pub hasta: Option<String>,
    pub advertencias: Vec<String>,
    /// La marca «fija / recurrente» de cada una; la fusión conserva la del destino.
    pub origen_fija: bool,
    pub destino_fija: bool,
}

#[tauri::command]
pub fn obtener_categorias() -> Result<Vec<Categoria>, String> {
    lee_catalogos(|a| {
        Ok(a.categorias()?.into_iter().map(|c| categoria(c.id, c.nombre, c.fija)).collect())
    })
}

#[tauri::command]
pub fn crear_categoria(nombre: String) -> Result<Categoria, String> {
    con_catalogos(|a| {
        let c = aplicacion::catalogos::crear_categoria(&nombre, a)?;
        Ok(categoria(c.id, c.nombre, c.fija))
    })
}

/// Marca (`fija = true`) o desmarca una categoría como gasto fijo/recurrente. **Escritura**: solo cambia esa marca, no toca
/// ningún gasto ni saldo. Una categoría que no existe es un error legible.
#[tauri::command]
pub fn marcar_categoria_fija(id: i64, fija: bool) -> Result<Categoria, String> {
    con_catalogos(|a| {
        let c = aplicacion::catalogos::marcar_categoria_fija(id, fija, a)?;
        Ok(categoria(c.id, c.nombre, c.fija))
    })
}

#[tauri::command]
pub fn eliminar_categoria(id: i64) -> Result<(), String> {
    con_catalogos(|a| Ok(aplicacion::catalogos::eliminar_categoria(id, a)?))
}

/// Cambia el nombre de una categoría (ver `aplicacion::catalogos::renombrar_categoria`). Devuelve la categoría ya renombrada.
#[tauri::command]
pub fn renombrar_categoria(id: i64, nombre: String) -> Result<Categoria, String> {
    con_catalogos(|a| {
        let c = aplicacion::catalogos::renombrar_categoria(id, &nombre, a)?;
        Ok(categoria(c.id, c.nombre, c.fija))
    })
}

/// Qué pasaría al fusionar `origen_id` en `destino_id`. **No escribe**: es la lectura que precede a `fusionar_categorias`.
#[tauri::command]
pub fn previsualizar_fusion_categorias(origen_id: i64, destino_id: i64) -> Result<VistaPreviaFusion, String> {
    lee_catalogos(|a| {
        let v = aplicacion::catalogos::previsualizar_fusion_categorias(origen_id, destino_id, a)?;
        Ok(VistaPreviaFusion {
            origen_id: v.origen_id,
            origen_nombre: v.origen_nombre,
            destino_id: v.destino_id,
            destino_nombre: v.destino_nombre,
            gastos: v.gastos,
            desde: v.desde,
            hasta: v.hasta,
            advertencias: v.advertencias,
            origen_fija: v.origen_fija,
            destino_fija: v.destino_fija,
        })
    })
}

/// Fusiona `origen_id` en `destino_id` en una sola transacción: reasigna todos sus gastos, deja el caso de auditoría
/// y elimina el origen. Devuelve cuántos gastos se movieron.
#[tauri::command]
pub fn fusionar_categorias(origen_id: i64, destino_id: i64) -> Result<i64, String> {
    con_catalogos(|a| Ok(aplicacion::catalogos::fusionar_categorias(origen_id, destino_id, a)?))
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
