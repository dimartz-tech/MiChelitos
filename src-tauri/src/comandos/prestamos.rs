//! Comandos de deudas y financiamientos.

use chrono::{Datelike, Local};
use serde::{Deserialize, Serialize};

use super::ayudantes::{con_prestamos, hoy_dd_mm_aaaa, lee_prestamos};
use crate::aplicacion;
use crate::dominio::tarjeta::MONEDA_LOCAL;
use crate::ipc;
use crate::puertos::repositorios::AlmacenPrestamos;

#[derive(Serialize, Deserialize, Debug)]
pub struct Prestamo {
    pub id: i64,
    pub tipo_prestamo: String,
    pub monto_prestamo: f64,
    pub institucion_financiera: String,
    pub tasa_actual: f64,
    pub cuotas_totales: Option<i32>,
    pub cuotas_pendientes: Option<i32>,
    pub monto_cuota: f64,
    pub dia_pago: i32,
    /// Capital que se debe hoy. Es el pasivo real, no el monto desembolsado.
    pub saldo_actual: f64,
    /// Solo en líneas revolventes, y solo si se ha declarado.
    pub limite_credito: Option<f64>,
    /// Tarjeta que cobra este financiamiento, si es una facilidad suya.
    pub tarjeta_id: Option<i64>,
    // Enriquecidos
    /// Nombre de la tarjeta vinculada, para no consultarla desde la vista.
    pub tarjeta_nombre: Option<String>,
    /// Día de corte. Es el de la tarjeta cuando la facilidad cuelga de una:
    /// no se guarda aparte porque mantener dos copias sincronizadas a mano es
    /// justo la clase de dato que se desincroniza.
    pub dia_corte: Option<i32>,
    /// Cupo por disponer. `None` cuando no es revolvente o falta el límite.
    pub disponible: Option<f64>,
    pub es_revolvente: bool,
    pub alerta_pago: bool,
    pub dias_pago_msg: String,
}

#[tauri::command]
pub fn obtener_prestamos() -> Result<Vec<Prestamo>, String> {
    let dia_actual = Local::now().day() as i32;
    lee_prestamos(|a| {
        Ok(aplicacion::prestamos::listar_prestamos(dia_actual, a)?
            .into_iter()
            .map(|p| Prestamo {
                id: p.leido.id,
                tipo_prestamo: p.leido.tipo_prestamo,
                monto_prestamo: p.leido.monto_prestamo,
                institucion_financiera: p.leido.institucion_financiera,
                tasa_actual: p.leido.tasa_actual,
                cuotas_totales: p.leido.cuotas_totales,
                cuotas_pendientes: p.leido.cuotas_pendientes,
                monto_cuota: p.leido.monto_cuota,
                dia_pago: p.dia_pago,
                saldo_actual: p.saldo_actual,
                limite_credito: p.leido.limite_credito,
                tarjeta_id: p.leido.tarjeta_id,
                tarjeta_nombre: p.leido.tarjeta_nombre,
                dia_corte: p.leido.tarjeta_fecha_corte,
                disponible: p.disponible,
                es_revolvente: p.es_revolvente,
                alerta_pago: p.alerta_pago,
                dias_pago_msg: p.dias_pago_msg,
            })
            .collect())
    })
}

#[derive(Deserialize)]
pub struct PrestamoInput {
    pub tipo_prestamo: String,
    pub monto_prestamo: ipc::ImporteDecimal,
    pub institucion_financiera: String,
    pub tasa_actual: f64,
    pub cuotas_totales: Option<i32>,
    pub cuotas_pendientes: Option<i32>,
    pub monto_cuota: ipc::ImporteDecimal,
    pub dia_pago: i32,
    /// Capital pendiente hoy. Si no se indica se asume el monto íntegro, que
    /// es lo correcto en un financiamiento recién desembolsado.
    pub saldo_actual: Option<ipc::ImporteDecimal>,
    /// Solo en líneas revolventes: el cupo aprobado.
    pub limite_credito: Option<ipc::ImporteDecimal>,
}

#[tauri::command]
pub fn crear_prestamo(input: PrestamoInput) -> Result<i64, String> {
    con_prestamos(|a| {
        // Los importes llegan como se escribieron; cada uno se convierte una vez al guardar. Todo es en moneda local.
        Ok(aplicacion::prestamos::crear_prestamo(
            aplicacion::prestamos::DatosPrestamoNuevo {
                tipo_prestamo: input.tipo_prestamo,
                monto_prestamo: input.monto_prestamo.con_divisa(MONEDA_LOCAL),
                institucion_financiera: input.institucion_financiera,
                tasa_actual: input.tasa_actual,
                cuotas_totales: input.cuotas_totales,
                cuotas_pendientes: input.cuotas_pendientes,
                monto_cuota: input.monto_cuota.con_divisa(MONEDA_LOCAL),
                dia_pago: input.dia_pago,
                saldo_actual: input.saldo_actual.map(|s| s.con_divisa(MONEDA_LOCAL)),
                limite_credito: input.limite_credito.map(|l| l.con_divisa(MONEDA_LOCAL)),
            },
            a,
        )?)
    })
}

#[derive(Deserialize)]
pub struct ActualizarPrestamoInput {
    pub id: i64,
    pub tasa_actual: f64,
    pub monto_cuota: ipc::ImporteDecimal,
    pub dia_pago: i32,
    pub limite_credito: Option<ipc::ImporteDecimal>,
    pub tarjeta_id: Option<i64>,
}

/// Corrige las condiciones de un financiamiento ya registrado.
///
/// Existe porque los datos que definen un financiamiento **cambian**: la tasa
/// se revisa, la cuota se recalcula y el límite de una línea se amplía. Sin
/// esta vía, corregir cualquiera de ellos obligaba a borrar el registro y
/// crearlo de nuevo, lo que se lleva por delante el libro de movimientos.
///
/// Deliberadamente **no** toca dos campos. `monto_prestamo` es un hecho
/// histórico, el desembolso original, y no se reescribe. `saldo_actual` tiene
/// su propia vía en `declarar_saldo_prestamo`, que deja asiento de la
/// diferencia; permitir editarlo aquí sería una puerta trasera para moverlo
/// sin dejar rastro.
#[tauri::command]
pub fn actualizar_prestamo(input: ActualizarPrestamoInput) -> Result<(), String> {
    con_prestamos(|a| {
        Ok(aplicacion::prestamos::actualizar_prestamo(
            aplicacion::prestamos::DatosPrestamoCorregido {
                id: input.id,
                tasa_actual: input.tasa_actual,
                monto_cuota: input.monto_cuota.con_divisa(MONEDA_LOCAL),
                dia_pago: input.dia_pago,
                limite_credito: input.limite_credito.map(|l| l.con_divisa(MONEDA_LOCAL)),
                tarjeta_id: input.tarjeta_id,
            },
            a,
        )?)
    })
}

#[tauri::command]
pub fn pagar_cuota_prestamo(id: i64, fecha: Option<String>) -> Result<(), String> {
    con_prestamos(|a| {
        let fecha = fecha.unwrap_or_else(hoy_dd_mm_aaaa);
        Ok(aplicacion::prestamos::pagar_cuota(id, &fecha, a)?)
    })
}

/// Fija el saldo al que dice el estado de cuenta.
///
/// Es el punto de reconciliación: el saldo que lleva la aplicación se estima cuota a cuota, y ninguna estimación
/// cuadra al centavo con el acreedor. La declaración no corrige la estimación en silencio: queda asentada como un
/// movimiento propio, con la diferencia que introdujo.
#[tauri::command]
pub fn declarar_saldo_prestamo(id: i64, saldo: ipc::ImporteDecimal, fecha: Option<String>) -> Result<(), String> {
    con_prestamos(|a| {
        let fecha = fecha.unwrap_or_else(hoy_dd_mm_aaaa);
        Ok(aplicacion::prestamos::declarar_saldo(id, saldo.con_divisa(MONEDA_LOCAL), &fecha, a)?)
    })
}

#[derive(Serialize)]
pub struct MovimientoPrestamo {
    pub id: i64,
    pub fecha: String,
    pub tipo: String,
    pub monto: f64,
    pub interes: f64,
    pub capital: f64,
    pub saldo_resultante: f64,
}

#[tauri::command]
pub fn obtener_movimientos_prestamo(id: i64) -> Result<Vec<MovimientoPrestamo>, String> {
    lee_prestamos(|a| {
        Ok(a.movimientos(id)?
            .into_iter()
            .map(|m| MovimientoPrestamo {
                id: m.id,
                fecha: m.fecha,
                tipo: m.tipo,
                monto: m.monto,
                interes: m.interes,
                capital: m.capital,
                saldo_resultante: m.saldo_resultante,
            })
            .collect())
    })
}

#[tauri::command]
pub fn eliminar_prestamo(id: i64) -> Result<(), String> {
    con_prestamos(|a| Ok(aplicacion::prestamos::eliminar_prestamo(id, a)?))
}
