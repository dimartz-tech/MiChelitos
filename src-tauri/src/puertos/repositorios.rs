//! Contratos de persistencia que los casos de uso necesitan.
//!
//! Hay un puerto por raíz de agregado, no uno por tabla. Los casos de uso
//! dependen de estos traits y no de `rusqlite`, que es lo que permite
//! ejercitarlos con dobles en memoria y sin base de datos.
//!
//! **Sobre la atomicidad.** Registrar un gasto toca `Gasto` y además
//! `CuentaAhorro` o `Tarjeta`, es decir, cruza fronteras de agregado. El plan
//! documenta esa desviación deliberada de la ortodoxia: en una aplicación
//! local de un solo usuario sobre SQLite, la consistencia transaccional entre
//! agregados es barata y la eventual sería una complicación gratuita. Quien
//! implementa estos puertos abre la transacción y la confirma; los puertos
//! solo describen las operaciones.

use crate::dominio::dinero::{Dinero, Divisa};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorAlmacen {
    NoEncontrado { entidad: &'static str, id: i64 },
    Fallo(String),
}

impl fmt::Display for ErrorAlmacen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorAlmacen::NoEncontrado { entidad, id } => {
                write!(f, "No se encontró {} con identificador {}.", entidad, id)
            }
            ErrorAlmacen::Fallo(detalle) => write!(f, "Error de almacenamiento: {}", detalle),
        }
    }
}

impl std::error::Error for ErrorAlmacen {}

impl From<ErrorAlmacen> for String {
    fn from(e: ErrorAlmacen) -> String {
        e.to_string()
    }
}

/// Datos de un gasto en el momento de persistirlo.
#[derive(Debug, Clone, PartialEq)]
pub struct GastoAPersistir {
    pub fecha: String,
    pub monto: Dinero,
    pub descripcion: String,
    pub categoria_id: i64,
    pub metodo_pago: String,
    pub cargos: Dinero,
    pub tarjeta_id: Option<i64>,
    pub cuenta_ahorro_id: Option<i64>,
}

/// Lo que hace falta saber de un gasto ya guardado para poder revertirlo.
#[derive(Debug, Clone, PartialEq)]
pub struct GastoGuardado {
    pub id: i64,
    pub monto: Dinero,
    pub metodo_pago: String,
    pub cargos: Dinero,
    pub tarjeta_id: Option<i64>,
    pub cuenta_ahorro_id: Option<i64>,
}

pub trait RepositorioCategorias {
    /// Nombre de la categoría, o `None` si no existe. La ausencia no es un
    /// error: hoy un identificador inexistente hace que no se aplique la
    /// exención y sea la clave foránea la que rechace la inserción.
    fn nombre(&self, categoria_id: i64) -> Result<Option<String>, ErrorAlmacen>;
}

pub trait RepositorioGastos {
    fn insertar(&mut self, gasto: &GastoAPersistir) -> Result<i64, ErrorAlmacen>;
    fn obtener(&self, gasto_id: i64) -> Result<GastoGuardado, ErrorAlmacen>;
    fn eliminar(&mut self, gasto_id: i64) -> Result<(), ErrorAlmacen>;
}

pub trait RepositorioTarjetas {
    /// Suma `delta` a la deuda de la divisa que el importe indica. Un delta
    /// negativo la reduce.
    fn ajustar_deuda(&mut self, tarjeta_id: i64, delta: Dinero) -> Result<(), ErrorAlmacen>;

    /// Reduce la deuda **sin permitir que baje de cero**.
    ///
    /// El recorte es la conducta vigente al revertir un gasto de tarjeta
    /// (**H5**), implementada hoy como `MAX(0.0, ...)` en SQL. Al no aplicarse
    /// también al registrar, crear y revertir dejan de ser operaciones
    /// inversas exactas y la diferencia desaparece sin registro.
    ///
    /// Se le da método propio para que la asimetría sea visible en el
    /// contrato en lugar de quedar enterrada en una consulta.
    fn reducir_deuda_con_recorte(
        &mut self,
        tarjeta_id: i64,
        monto: Dinero,
    ) -> Result<(), ErrorAlmacen>;
}

/// Persistencia que necesita una operación sobre gastos.
///
/// Los puertos siguen siendo uno por raíz de agregado; esto solo expresa que
/// un mismo adaptador los implementa todos sobre una única transacción.
pub trait AlmacenGastos:
    RepositorioCategorias + RepositorioGastos + RepositorioTarjetas + RepositorioCuentas
{
}

impl<T> AlmacenGastos for T where
    T: RepositorioCategorias + RepositorioGastos + RepositorioTarjetas + RepositorioCuentas
{
}

pub trait RepositorioCuentas {
    /// Suma `delta` al saldo. Un delta negativo lo debita.
    fn ajustar_saldo(&mut self, cuenta_id: i64, delta: Dinero) -> Result<(), ErrorAlmacen>;

    /// Igual, pero localizando la cuenta por nombre, que es como el sistema
    /// referencia hoy las cajas de efectivo. Es la causa de H3: si la fila no
    /// existe, la conducta vigente es no hacer nada y no fallar.
    fn ajustar_saldo_de_caja(&mut self, nombre: &str, delta: Dinero)
        -> Result<(), ErrorAlmacen>;

    fn saldo(&self, cuenta_id: i64) -> Result<Dinero, ErrorAlmacen>;

    fn divisa(&self, cuenta_id: i64) -> Result<Divisa, ErrorAlmacen>;
}
