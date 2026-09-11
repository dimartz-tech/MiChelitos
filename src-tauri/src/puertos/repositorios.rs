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

use crate::dominio::bonificacion::Bonificacion;
use crate::dominio::conversion::EstadoConversion;
use crate::dominio::tarjeta::PoliticaLiquidacion;
use crate::dominio::dinero::{Dinero, Divisa};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorAlmacen {
    NoEncontrado { entidad: &'static str, id: i64 },
    /// No hay caja de efectivo para esa divisa.
    ///
    /// Tiene variante propia porque no se identifica por id: la caja es un
    /// papel, no una fila concreta. Antes esta situación no era un error sino
    /// un `Ok` que no movía ningún saldo, que es lo que describía **H3**.
    CajaDeEfectivoAusente { divisa: Divisa },
    Fallo(String),
}

impl fmt::Display for ErrorAlmacen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorAlmacen::NoEncontrado { entidad, id } => {
                write!(f, "No se encontró {} con identificador {}.", entidad, id)
            }
            ErrorAlmacen::CajaDeEfectivoAusente { divisa } => write!(
                f,
                "No hay una caja de efectivo en {}. Crea la cuenta de efectivo antes de registrar un gasto en esa divisa.",
                divisa.codigo()
            ),
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
    /// Siempre en la divisa que se debita, que con conversión es la de
    /// destino y sin ella la del propio gasto.
    pub cargos: Dinero,
    pub tarjeta_id: Option<i64>,
    pub cuenta_ahorro_id: Option<i64>,
    pub estado_conversion: EstadoConversion,
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
    pub estado_conversion: EstadoConversion,
}

impl GastoGuardado {
    /// Importe que salió de la cuenta, sin contar cargos. Con conversión es
    /// el de destino; sin ella, el propio monto del gasto.
    pub fn monto_debitado(&self) -> Dinero {
        match self.estado_conversion.conversion() {
            Some(c) => c.destino(),
            None => self.monto,
        }
    }
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

    /// Cierra un consumo pendiente registrando la conversión que aplicó el
    /// emisor.
    fn liquidar(
        &mut self,
        gasto_id: i64,
        estado: EstadoConversion,
    ) -> Result<(), ErrorAlmacen>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct BonificacionAPersistir {
    pub fecha: String,
    pub tarjeta_id: i64,
    pub bonificacion: Bonificacion,
    pub gasto_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BonificacionGuardada {
    pub id: i64,
    pub tarjeta_id: i64,
    pub bonificacion: Bonificacion,
}

// Los nombres llevan sufijo para no colisionar con los de RepositorioGastos:
// un mismo adaptador implementa ambos puertos.
pub trait RepositorioBonificaciones {
    fn insertar_bonificacion(&mut self, b: &BonificacionAPersistir) -> Result<i64, ErrorAlmacen>;
    fn obtener_bonificacion(&self, id: i64) -> Result<BonificacionGuardada, ErrorAlmacen>;
    fn eliminar_bonificacion(&mut self, id: i64) -> Result<(), ErrorAlmacen>;
}

pub trait RepositorioTarjetas {
    /// Suma `delta` a la deuda de la divisa que el importe indica. Un delta
    /// negativo la reduce.
    ///
    /// **Un balance negativo es válido y significa saldo a favor del titular**
    /// (resolución de **H5**). Antes existía un `reducir_deuda_con_recorte`
    /// que aplicaba `MAX(0.0, ...)`: la deuda no bajaba de cero y el exceso
    /// desaparecía sin registro. Como esa regla solo se aplicaba al reducir y
    /// nunca al aumentar, registrar y revertir no eran inversas exactas.
    ///
    /// Ajustar la deuda es hoy la única vía, y es simétrica: lo que un delta
    /// suma, su negado lo resta. Quien necesite tratar el saldo a favor de
    /// forma distinta lo decide al leerlo, no al escribirlo.
    fn ajustar_deuda(&mut self, tarjeta_id: i64, delta: Dinero) -> Result<(), ErrorAlmacen>;

    /// Deuda vigente en la divisa indicada.
    ///
    /// Puede ser negativa: eso es saldo a favor del titular. Existe para que
    /// el contrato pueda comprobar el efecto de `ajustar_deuda` sobre las dos
    /// implementaciones; sin lector, la divergencia que causó **H5** solo era
    /// observable en el doble y no en SQLite, que es donde vivía.
    fn deuda(&self, tarjeta_id: i64, divisa: Divisa) -> Result<Dinero, ErrorAlmacen>;

    /// Cómo liquida esta tarjeta los consumos en divisa extranjera.
    fn politica(&self, tarjeta_id: i64) -> Result<PoliticaLiquidacion, ErrorAlmacen>;
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

/// Persistencia que necesita una operación sobre bonificaciones: el crédito
/// en sí y la deuda de la tarjeta que reduce.
pub trait AlmacenBonificaciones: RepositorioBonificaciones + RepositorioTarjetas {}

impl<T> AlmacenBonificaciones for T where T: RepositorioBonificaciones + RepositorioTarjetas {}

pub trait RepositorioCuentas {
    /// Suma `delta` al saldo. Un delta negativo lo debita.
    fn ajustar_saldo(&mut self, cuenta_id: i64, delta: Dinero) -> Result<(), ErrorAlmacen>;

    /// Identificador de la caja de efectivo de esa divisa.
    ///
    /// Sustituye al antiguo `ajustar_saldo_de_caja(nombre, delta)`, que
    /// localizaba la caja por su nombre literal y, si no la encontraba,
    /// devolvía `Ok` sin mover nada (**H3**). Ahora la caja se identifica por
    /// el papel que cumple, no por su texto, y su ausencia es un error: quien
    /// registra un gasto en efectivo se entera de que no se asentó.
    fn caja(&self, divisa: Divisa) -> Result<i64, ErrorAlmacen>;

    fn saldo(&self, cuenta_id: i64) -> Result<Dinero, ErrorAlmacen>;

    fn divisa(&self, cuenta_id: i64) -> Result<Divisa, ErrorAlmacen>;
}
