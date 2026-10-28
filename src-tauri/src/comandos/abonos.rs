//! Comandos de abonos (pagos) a tarjeta.

use serde::{Deserialize, Serialize};

use super::ayudantes::{con_almacen, lee_almacen};
use crate::aplicacion;
use crate::aplicacion::registrar_pago_tarjeta::DatosPago;
use crate::dominio::dinero::{Divisa, TasaCambio};
use crate::ipc;

#[tauri::command]
pub fn registrar_pago_tarjeta(
    id: i64,
    fecha: String,
    monto: ipc::ImporteDecimal,
    divisa: String,
    cuenta_ahorro_id: Option<i64>,
    tasa_cambio: f64
) -> Result<(), String> {
    con_almacen(|a| {
        Ok(aplicacion::abonos::registrar_abono(
            DatosPago {
                tarjeta_id: id,
                fecha,
                // El céntimo lo deciden los dígitos escritos; la divisa es la declarada por el abono.
                monto: monto.con_divisa(Divisa::desde_codigo(&divisa)?),
                cuenta_ahorro_id,
                // Una tasa de cero es como la interfaz dice «no aplica».
                tasa_cambio: if tasa_cambio > 0.0 {
                    Some(TasaCambio::nueva(tasa_cambio)?)
                } else {
                    None
                },
            },
            a,
        )?)
    })
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AbonoTarjeta {
    pub id: i64,
    pub fecha_pago: String,
    pub monto_pagado: f64,
    pub divisa: String,
    pub cuenta_ahorro_id: Option<i64>,
    pub cuenta_nombre: Option<String>,
    pub tasa_cambio: Option<f64>,
}

/// Abonos registrados a una tarjeta, del más reciente al más antiguo.
#[tauri::command]
pub fn obtener_abonos_tarjeta(tarjeta_id: i64) -> Result<Vec<AbonoTarjeta>, String> {
    lee_almacen(|a| {
        Ok(aplicacion::abonos::listar_abonos(tarjeta_id, a)?
            .into_iter()
            .map(|p| AbonoTarjeta {
                id: p.id,
                fecha_pago: p.fecha_pago,
                monto_pagado: p.monto_pagado,
                divisa: p.divisa,
                cuenta_ahorro_id: p.cuenta_ahorro_id,
                cuenta_nombre: p.cuenta_nombre,
                tasa_cambio: p.tasa_cambio,
            })
            .collect())
    })
}

/// Deshace un abono a tarjeta.
///
/// Traducción pura: la reversión vive en el caso de uso, igual que la de los
/// gastos. Devuelve lo que se deshizo para que la interfaz pueda decirlo en
/// vez de limitarse a confirmar que algo pasó.
#[tauri::command]
pub fn revertir_abono_tarjeta(id: i64, motivo: String) -> Result<String, String> {
    con_almacen(|a| {
        let r = aplicacion::abonos::revertir_abono(id, &motivo, a)?;
        let resumen = match r.revertido.devuelto_a_la_cuenta {
            Some(d) => format!(
                "Se repusieron {} {:.2} a la deuda y volvieron {} {:.2} a la cuenta.",
                r.revertido.deuda_restituida.divisa().codigo(),
                r.revertido.deuda_restituida.unidades(),
                d.divisa().codigo(),
                d.unidades()
            ),
            None => format!(
                "Se repusieron {} {:.2} a la deuda. El abono no tenía cuenta asociada.",
                r.revertido.deuda_restituida.divisa().codigo(),
                r.revertido.deuda_restituida.unidades()
            ),
        };
        Ok(format!("{} Caso {}.", resumen, r.caso))
    })
}

/// Una advertencia de la vista previa: el código (para que la interfaz la distinga), su gravedad y el texto ya redactado.
#[derive(Serialize, Deserialize, Debug)]
pub struct AdvertenciaDeAbono {
    pub codigo: String,
    /// `bloqueo` (el registro lo rechazaría), `aviso` o `info`.
    pub gravedad: String,
    pub texto: String,
}

/// La cuenta de la que saldría el dinero.
#[derive(Serialize, Deserialize, Debug)]
pub struct CuentaDeCobroPrevia {
    pub id: i64,
    pub nombre: String,
    pub divisa: String,
    pub saldo_actual: String,
    /// `true` si no se eligió cuenta: es la caja de efectivo de la divisa del abono.
    pub por_defecto: bool,
    pub archivada: bool,
}

/// Lo que haría un abono, antes de registrarlo. **Todos los importes viajan como texto exacto** (`1234.56`).
#[derive(Serialize, Deserialize, Debug)]
pub struct VistaPreviaAbono {
    pub tarjeta_id: i64,
    pub tarjeta_nombre: String,
    pub tarjeta_activa: bool,
    pub divisa: String,
    pub monto: Option<String>,
    pub tipo: String,
    pub tipo_etiqueta: String,
    pub cuenta: Option<CuentaDeCobroPrevia>,
    pub con_comision: bool,
    /// Lo que sale de la cuenta convertido a su divisa, sin comisión.
    pub debitado: Option<String>,
    pub comision: Option<String>,
    pub total_debitado: Option<String>,
    /// La tasa usada; solo cuando el abono cruza divisas.
    pub tasa_cambio: Option<String>,
    pub saldo_resultante: Option<String>,
    pub balance_actual_pesos: String,
    pub balance_actual_dolares: String,
    pub balance_resultante_pesos: String,
    pub balance_resultante_dolares: String,
    pub advertencias: Vec<AdvertenciaDeAbono>,
    /// `false` si alguna advertencia es un bloqueo o falta con qué calcular.
    pub se_puede_confirmar: bool,
}

/// La vista previa de un abono con lo que el formulario tiene escrito. **Solo lectura**: no registra nada.
///
/// El monto llega como texto sin validar a propósito: uno vacío o ilegible es una advertencia de la vista previa, no
/// un error de la llamada. El cálculo es el del registro (`aplicacion::previsualizar_abono`).
#[tauri::command]
pub fn previsualizar_abono(
    id: i64,
    monto: String,
    divisa: String,
    cuenta_ahorro_id: Option<i64>,
    tasa_cambio: f64,
    tipo: String,
) -> Result<VistaPreviaAbono, String> {
    use crate::dominio::abono::TipoDeAbono;
    let datos = aplicacion::previsualizar_abono::DatosDePrevia {
        tarjeta_id: id,
        monto,
        divisa: Divisa::desde_codigo(&divisa).map_err(|e| e.to_string())?,
        cuenta_ahorro_id,
        // Igual que el registro: cero (o menos) es «sin tasa».
        tasa_cambio: if tasa_cambio > 0.0 { Some(tasa_cambio) } else { None },
        tipo: TipoDeAbono::desde_codigo(&tipo),
    };
    lee_almacen(|a| {
        let v = aplicacion::previsualizar_abono::previsualizar_abono(datos, a)?;        Ok(a_vista_previa(v))
    })
}

fn a_vista_previa(v: aplicacion::previsualizar_abono::VistaPreviaDeAbono) -> VistaPreviaAbono {
    let texto = ipc::importe_a_texto;
    VistaPreviaAbono {
        tarjeta_id: v.tarjeta_id,
        tarjeta_nombre: v.tarjeta_nombre,
        tarjeta_activa: v.tarjeta_activa,
        divisa: v.divisa.codigo().to_string(),
        monto: v.monto.map(texto),
        tipo: v.tipo.codigo().to_string(),
        tipo_etiqueta: v.tipo.etiqueta().to_string(),
        cuenta: v.cuenta.map(|c| CuentaDeCobroPrevia {
            id: c.id,
            nombre: c.nombre,
            divisa: c.divisa.codigo().to_string(),
            saldo_actual: texto(c.saldo_actual),
            por_defecto: c.por_defecto,
            archivada: c.archivada,
        }),
        con_comision: v.con_comision,
        debitado: v.calculo.map(|c| texto(c.debitado)),
        comision: v.calculo.and_then(|c| c.comision).map(texto),
        total_debitado: v.calculo.map(|c| texto(c.total_debitado)),
        tasa_cambio: v.tasa.map(ipc::tasa_a_texto),
        saldo_resultante: v.saldo_resultante.map(texto),
        balance_actual_pesos: texto(v.balance_actual_dop),
        balance_actual_dolares: texto(v.balance_actual_usd),
        balance_resultante_pesos: texto(v.balance_resultante_dop),
        balance_resultante_dolares: texto(v.balance_resultante_usd),
        advertencias: v
            .advertencias
            .iter()
            .map(|a| AdvertenciaDeAbono { codigo: a.codigo().to_string(), gravedad: a.gravedad().codigo().to_string(), texto: a.texto() })
            .collect(),
        se_puede_confirmar: v.se_puede_confirmar,
    }
}
