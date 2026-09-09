//! Casos de uso: orquestan el dominio y los puertos, sin SQL ni Tauri.
#![allow(dead_code)]

pub mod liquidar_gasto;
pub mod registrar_gasto;
pub mod revertir_gasto;

use crate::dominio::errores::ErrorDominio;
use crate::puertos::repositorios::ErrorAlmacen;
use std::fmt;

/// Un caso de uso puede fallar por una regla de negocio o por la persistencia.
/// Mantenerlos distinguibles permite que la interfaz explique cuál de las dos
/// cosas ocurrió en lugar de mostrar un mensaje genérico.
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorAplicacion {
    Dominio(ErrorDominio),
    Almacen(ErrorAlmacen),
}

impl fmt::Display for ErrorAplicacion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorAplicacion::Dominio(e) => write!(f, "{}", e),
            ErrorAplicacion::Almacen(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for ErrorAplicacion {}

impl From<ErrorDominio> for ErrorAplicacion {
    fn from(e: ErrorDominio) -> Self {
        ErrorAplicacion::Dominio(e)
    }
}

impl From<ErrorAlmacen> for ErrorAplicacion {
    fn from(e: ErrorAlmacen) -> Self {
        ErrorAplicacion::Almacen(e)
    }
}

// Permite que los comandos Tauri, que devuelven Result<_, String>, adopten
// los casos de uso sin cambiar su firma.
impl From<ErrorAplicacion> for String {
    fn from(e: ErrorAplicacion) -> String {
        e.to_string()
    }
}
