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

/// Un abono a tarjeta tal como quedó guardado, con todo lo que movió.
///
/// Las tres últimas son opcionales porque un abono puede haberse registrado
/// sin cuenta de la que debitar. En ese caso no hubo movimiento de cuenta ni
/// comisión, y revertirlo solo afecta a la deuda de la tarjeta.
#[derive(Debug, Clone, PartialEq)]
pub struct PagoGuardado {
    pub id: i64,
    pub tarjeta_id: i64,
    pub monto: Dinero,
    pub cuenta_ahorro_id: Option<i64>,
    /// Tasa aplicada al convertir. `None` o 1.0 significan sin conversión.
    pub tasa_cambio: Option<f64>,
    pub gasto_comision_id: Option<i64>,
}

/// Un abono a punto de guardarse, con todo lo que la operación movió.
///
/// Se construye entero antes de insertarlo, incluido el vínculo al gasto de
/// comisión. El comando anterior insertaba el abono primero y lo completaba
/// después con un `UPDATE`, de modo que entre las dos sentencias existía un
/// abono sin vínculo; aquí esa fila intermedia no llega a existir.
#[derive(Debug, Clone, PartialEq)]
pub struct PagoAPersistir {
    pub tarjeta_id: i64,
    pub fecha: String,
    pub monto: Dinero,
    pub cuenta_ahorro_id: Option<i64>,
    pub tasa_cambio: Option<f64>,
    pub gasto_comision_id: Option<i64>,
}

pub trait RepositorioPagosTarjeta {
    fn insertar_pago(&mut self, pago: &PagoAPersistir) -> Result<i64, ErrorAlmacen>;
    fn obtener_pago(&self, pago_id: i64) -> Result<PagoGuardado, ErrorAlmacen>;
    fn eliminar_pago(&mut self, pago_id: i64) -> Result<(), ErrorAlmacen>;
}

/// Persistencia que necesita revertir un abono: el abono, la deuda que
/// restituye, la cuenta a la que devuelve el dinero y el gasto de comisión.
pub trait AlmacenAbonos:
    RepositorioPagosTarjeta + RepositorioTarjetas + RepositorioCuentas + RepositorioGastos
{
}

impl<T> AlmacenAbonos for T where
    T: RepositorioPagosTarjeta + RepositorioTarjetas + RepositorioCuentas + RepositorioGastos
{
}

pub trait RepositorioCuentas {
    /// Suma `delta` al saldo. Un delta negativo lo debita.
    ///
    /// **Un saldo negativo es válido** (resolución de **H10**). Antes existía
    /// un `reducir_saldo_con_recorte` que aplicaba `MAX(0.0, ...)` al revertir
    /// una transferencia, mientras el origen se restituía sin límite. Como el
    /// recorte solo actuaba en un extremo, revertir no deshacía la operación:
    /// inflaba el total repartido entre las dos cuentas.
    ///
    /// Ajustar el saldo es hoy la única vía, y es simétrica.
    fn ajustar_saldo(&mut self, cuenta_id: i64, delta: Dinero) -> Result<(), ErrorAlmacen>;

    /// Identificador de la caja de efectivo de esa divisa.
    ///
    /// Sustituye al antiguo `ajustar_saldo_de_caja(nombre, delta)`, que
    /// localizaba la caja por su nombre literal y, si no la encontraba,
    /// devolvía `Ok` sin mover nada (**H3**). Ahora la caja se identifica por
    /// el papel que cumple, no por su texto, y su ausencia es un error: quien
    /// registra un gasto en efectivo se entera de que no se asentó.
    fn caja(&self, divisa: Divisa) -> Result<i64, ErrorAlmacen>;

    /// Tarifa fija que esta cuenta paga por el servicio de pago de impuestos.
    ///
    /// `None` significa que no hay tarifa pactada, no que sea cero: de esa
    /// distinción depende si la operación paga el porcentaje ordinario o el
    /// precio fijo del servicio.
    fn comision_pago_impuestos(&self, cuenta_id: i64) -> Result<Option<Dinero>, ErrorAlmacen>;

    fn saldo(&self, cuenta_id: i64) -> Result<Dinero, ErrorAlmacen>;

    fn divisa(&self, cuenta_id: i64) -> Result<Divisa, ErrorAlmacen>;

    /// Si la cuenta cumple el papel de caja de efectivo de su divisa.
    ///
    /// La clave foránea no protege a la caja: está declarada
    /// `ON DELETE SET NULL`, de modo que borrarla desvincularía sus gastos en
    /// silencio en lugar de impedir el borrado. La protección tiene que ser
    /// una guarda explícita, y por eso el puerto expone la pregunta.
    fn es_caja(&self, cuenta_id: i64) -> Result<bool, ErrorAlmacen>;

    /// Cuántos gastos referencian la cuenta. Es la guarda vigente del borrado.
    fn gastos_que_referencian(&self, cuenta_id: i64) -> Result<i64, ErrorAlmacen>;

    fn eliminar_cuenta(&mut self, cuenta_id: i64) -> Result<(), ErrorAlmacen>;
}

/// Datos de una transferencia tal como se guardan.
#[derive(Debug, Clone, PartialEq)]
pub struct TransferenciaGuardada {
    pub id: i64,
    pub fecha: String,
    pub origen_id: i64,
    pub destino_id: i64,
    pub monto_origen: Dinero,
    pub monto_destino: Dinero,
    pub cargo: Dinero,
    pub descripcion: String,
}

/// Lo que hay que persistir de una transferencia recién hecha.
pub struct TransferenciaAPersistir {
    pub fecha: String,
    pub origen_id: i64,
    pub destino_id: i64,
    pub monto_origen: Dinero,
    pub monto_destino: Dinero,
    /// Deducida de los dos importes. `None` cuando no cruza divisas.
    pub tasa: Option<f64>,
    pub cargo: Dinero,
    pub descripcion: String,
}

pub trait RepositorioTransferencias {
    fn insertar_transferencia(
        &mut self,
        datos: TransferenciaAPersistir,
    ) -> Result<i64, ErrorAlmacen>;

    fn obtener_transferencia(&self, id: i64) -> Result<TransferenciaGuardada, ErrorAlmacen>;

    fn eliminar_transferencia(&mut self, id: i64) -> Result<(), ErrorAlmacen>;

    /// Cuántas transferencias tienen la cuenta en alguno de sus extremos.
    ///
    /// La guarda de borrado no lo consulta hoy (**H13**), y como la clave
    /// foránea es `ON DELETE CASCADE`, borrar la cuenta se lleva su historial
    /// en silencio. El puerto lo expone para que el caso de uso pueda
    /// decidirlo cuando se resuelva el hallazgo.
    fn transferencias_que_referencian(&self, cuenta_id: i64) -> Result<i64, ErrorAlmacen>;
}

/// Persistencia que necesita una operación sobre transferencias.
pub trait AlmacenTransferencias: RepositorioCuentas + RepositorioTransferencias {}

impl<T> AlmacenTransferencias for T where T: RepositorioCuentas + RepositorioTransferencias {}
