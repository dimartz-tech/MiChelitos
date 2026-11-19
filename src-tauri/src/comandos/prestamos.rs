//! Comandos de deudas y financiamientos.

use chrono::{Datelike, Local};
use serde::{Deserialize, Serialize};

use super::ayudantes::{con_prestamos, con_prestamos_y_almacen, hoy_dd_mm_aaaa, lee_prestamos};
use crate::aplicacion;
use crate::aplicacion::pagos_de_prestamo::{self as pagos, OrigenDelPago, PagoAPrever, PagoPrevisto};
use crate::dominio::prestamo::TipoPago;
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

#[derive(Serialize, Clone, Debug)]
pub struct MovimientoPrestamo {
    pub id: i64,
    pub fecha: String,
    pub tipo: String,
    pub monto: f64,
    pub interes: f64,
    pub capital: f64,
    pub saldo_resultante: f64,
    /// El gasto que sacó el dinero de una cuenta; `None` en conciliaciones y en cuotas registradas sin cuenta.
    pub gasto_id: Option<i64>,
    /// La cuenta (o la caja de efectivo) de la que salió ese gasto. La interfaz propone la del último pago.
    pub cuenta_id: Option<i64>,
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
                gasto_id: m.gasto_id,
                cuenta_id: m.cuenta_id,
            })
            .collect())
    })
}

#[tauri::command]
pub fn eliminar_prestamo(id: i64) -> Result<(), String> {
    con_prestamos(|a| Ok(aplicacion::prestamos::eliminar_prestamo(id, a)?))
}

/// Un pago de préstamo tal como lo envía la interfaz.
#[derive(Deserialize, Debug)]
pub struct PagoPrestamoInput {
    pub prestamo_id: i64,
    /// «cuota» o «abono_capital».
    pub tipo: String,
    pub monto: ipc::ImporteDecimal,
    /// El interés que dice el recibo; `null` para estimarlo.
    pub interes: Option<ipc::ImporteDecimal>,
    /// `dd/mm/aaaa`.
    pub fecha: String,
    /// La cuenta de la que sale; `null` para pagarlo de la caja de efectivo en pesos.
    pub cuenta_id: Option<i64>,
    pub categoria_id: i64,
}

/// Cómo queda (o quedaría) el préstamo con el pago. Importes en texto exacto.
#[derive(Serialize, Deserialize, Debug)]
pub struct PrevisionPagoPrestamo {
    pub interes: String,
    pub capital: String,
    pub interes_estimado: bool,
    pub saldo_previo: String,
    pub saldo_resultante: String,
    pub cuotas_pendientes_despues: Option<i32>,
}

fn a_prevision(p: PagoPrevisto) -> PrevisionPagoPrestamo {
    PrevisionPagoPrestamo {
        interes: ipc::importe_a_texto(p.interes),
        capital: ipc::importe_a_texto(p.capital),
        interes_estimado: p.interes_estimado,
        saldo_previo: ipc::importe_a_texto(p.saldo_previo),
        saldo_resultante: ipc::importe_a_texto(p.saldo_resultante),
        cuotas_pendientes_despues: p.cuotas_pendientes_despues,
    }
}

fn pago_a_prever(
    prestamo_id: i64,
    tipo: &str,
    monto: &ipc::ImporteDecimal,
    interes: Option<&ipc::ImporteDecimal>,
) -> Result<PagoAPrever, String> {
    Ok(PagoAPrever {
        prestamo_id,
        tipo: TipoPago::desde_codigo(tipo).map_err(|e| e.to_string())?,
        monto: monto.con_divisa(MONEDA_LOCAL),
        interes: interes.map(|i| i.con_divisa(MONEDA_LOCAL)),
    })
}

/// Cómo quedaría el préstamo con un pago, sin escribir nada.
#[tauri::command]
pub fn previsualizar_pago_prestamo(
    prestamo_id: i64,
    tipo: String,
    monto: ipc::ImporteDecimal,
    interes: Option<ipc::ImporteDecimal>,
) -> Result<PrevisionPagoPrestamo, String> {
    let pago = pago_a_prever(prestamo_id, &tipo, &monto, interes.as_ref())?;
    lee_prestamos(|a| Ok(a_prevision(pagos::prever_pago(&pago, a)?)))
}

/// Paga un préstamo desde una cuenta (o la caja de efectivo): saca el dinero como gasto y lo asienta en el préstamo,
/// en la misma transacción.
#[tauri::command]
pub fn registrar_pago_prestamo(input: PagoPrestamoInput) -> Result<PrevisionPagoPrestamo, String> {
    let pago = pago_a_prever(input.prestamo_id, &input.tipo, &input.monto, input.interes.as_ref())?;
    let datos = pagos::DatosPagoPrestamo {
        pago,
        fecha: input.fecha,
        origen: input.cuenta_id.map(OrigenDelPago::Cuenta).unwrap_or(OrigenDelPago::Efectivo),
        categoria_id: input.categoria_id,
    };
    con_prestamos_y_almacen(|p, a| Ok(a_prevision(pagos::registrar_pago_prestamo(datos, p, a)?.prevision)))
}

/// Revierte el último pago de un préstamo: devuelve el dinero, el saldo y la cuota, y deja un caso de corrección.
/// Devuelve el número del caso.
#[tauri::command]
pub fn revertir_pago_prestamo(movimiento_id: i64, motivo: String) -> Result<String, String> {
    con_prestamos_y_almacen(|p, a| Ok(pagos::revertir_pago_prestamo(movimiento_id, &motivo, p, a)?))
}
