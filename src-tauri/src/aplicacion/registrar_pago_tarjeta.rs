//! Caso de uso: abonar a una tarjeta de crédito.
//!
//! Es el flujo que **atraviesa el hexágono entero**: entra un importe en una
//! divisa, se convierte a otra, se le calcula la comisión bancaria, se debita
//! una cuenta, se acredita la deuda de una tarjeta y queda un gasto derivado.
//! Por eso es el caso elegido para la prueba Gherkin de §6.4: si funciona de
//! extremo a extremo, la arquitectura funciona.
//!
//! Lo que este módulo añade frente al comando que sustituye:
//!
//! * **La tasa deja de ser opcional cuando cruza divisas.** Antes, abonar en
//!   dólares desde una cuenta en pesos sin indicar tasa llegaba hasta el
//!   adaptador y fallaba allí con un error de divisas incompatibles, que no
//!   dice qué falta. Ahora se rechaza aquí, con el error que nombra la causa.
//! * **El abono se inserta completo.** El comando anterior lo insertaba y lo
//!   completaba después con un `UPDATE` para enlazar la comisión; entre las
//!   dos sentencias existía un abono sin vínculo.
//!
//! ## H15 — divergencia declarada respecto al comando anterior
//!
//! El comando aplicaba la tasa **aunque no hubiera cambio de divisa**: un
//! abono en pesos desde una cuenta en pesos al que llegara una tasa de 60
//! debitaba sesenta veces el importe. Era conducta conocida y se conservó
//! deliberadamente durante la Fase 1 —el comentario lo decía—, pero ninguna
//! prueba la cubría.
//!
//! Aquí deja de ocurrir: la conversión depende de que las divisas difieran,
//! no de que llegue una tasa. **Es un cambio de conducta, no una extracción
//! neutra**, y por eso queda dicho aquí y fijado por prueba en lugar de pasar
//! inadvertido. La regla nueva es la correcta: convertir sin cruzar divisas
//! no significa nada, y el importe inflado salía de una cuenta real.

use super::ErrorAplicacion;
use crate::dominio::cargos::TASA_RETENCION;
use crate::dominio::dinero::{Dinero, TasaCambio};
use crate::dominio::errores::ErrorDominio;
use crate::puertos::repositorios::*;

/// Lo que hace falta para registrar un abono.
#[derive(Debug, Clone)]
pub struct DatosPago {
    pub tarjeta_id: i64,
    pub fecha: String,
    pub monto: Dinero,
    /// Cuenta de la que sale el dinero. `None` cuando el abono se hizo por
    /// una vía que la aplicación no lleva —efectivo en caja del banco, por
    /// ejemplo—: entonces solo se reduce la deuda.
    pub cuenta_ahorro_id: Option<i64>,
    /// Tasa a la que el banco convirtió. Obligatoria si el abono va en una
    /// divisa distinta a la de la cuenta que paga.
    pub tasa_cambio: Option<TasaCambio>,
}

/// Lo que el abono movió, para poder informarlo sin volver a consultarlo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PagoRegistrado {
    pub id: i64,
    /// Importe que salió de la cuenta, ya convertido y sin la comisión.
    pub debitado: Option<Dinero>,
    pub comision: Option<Dinero>,
}

pub fn registrar_pago_tarjeta(
    datos: DatosPago,
    categoria_comision_id: i64,
    almacen: &mut impl AlmacenAbonos,
) -> Result<PagoRegistrado, ErrorAplicacion> {
    // El orden importa: primero se resuelve todo lo que puede fallar por una
    // regla, y solo después se toca un saldo. Así una operación rechazada no
    // deja nada a medias ni siquiera antes de la transacción.
    let conversion = match datos.cuenta_ahorro_id {
        None => None,
        Some(cuenta_id) => Some(resolver_conversion(&datos, almacen.divisa(cuenta_id)?)?),
    };

    // 1. La deuda de la tarjeta baja. Sin recorte a cero: si el abono excede
    //    la deuda, el balance queda negativo, que es el saldo a favor que el
    //    emisor acredita de verdad (resolución de H5).
    almacen.ajustar_deuda(datos.tarjeta_id, datos.monto.negado())?;

    // 2. La cuenta paga el importe convertido más su comisión.
    let (debitado, comision, gasto_id) = match (datos.cuenta_ahorro_id, conversion) {
        (Some(cuenta_id), Some(debitado)) => {
            let comision = debitado.porcentaje(TASA_RETENCION)?;
            let total = debitado.sumar(&comision)?;
            almacen.ajustar_saldo(cuenta_id, total.negado())?;

            let gasto_id = almacen.insertar(&GastoAPersistir {
                fecha: datos.fecha.clone(),
                monto: comision,
                descripcion: descripcion_comision(datos.tasa_cambio),
                categoria_id: categoria_comision_id,
                metodo_pago: "transferencia".to_string(),
                cargos: Dinero::cero(comision.divisa()),
                tarjeta_id: None,
                cuenta_ahorro_id: Some(cuenta_id),
                estado_conversion: crate::dominio::conversion::EstadoConversion::NoAplica,
            })?;

            (Some(debitado), Some(comision), Some(gasto_id))
        }
        _ => (None, None, None),
    };

    // 3. El abono se guarda ya enlazado a todo lo que movió.
    let id = almacen.insertar_pago(&PagoAPersistir {
        tarjeta_id: datos.tarjeta_id,
        fecha: datos.fecha,
        monto: datos.monto,
        cuenta_ahorro_id: datos.cuenta_ahorro_id,
        tasa_cambio: datos.tasa_cambio.map(|t| t.valor()),
        monto_debitado: debitado,
        comision,
        gasto_comision_id: gasto_id,
    })?;

    Ok(PagoRegistrado { id, debitado, comision })
}

/// Qué importe sale realmente de la cuenta, en la divisa de la cuenta.
///
/// Es aquí donde se exige la tasa. La condición no es «el abono va en USD»
/// sino «el abono y la cuenta no coinciden»: abonar dólares desde una cuenta
/// en dólares no necesita conversión ninguna, y pedirla sería un estorbo.
fn resolver_conversion(
    datos: &DatosPago,
    divisa_cuenta: crate::dominio::dinero::Divisa,
) -> Result<Dinero, ErrorAplicacion> {
    if datos.monto.divisa() == divisa_cuenta {
        return Ok(datos.monto);
    }

    let tasa = datos.tasa_cambio.ok_or(ErrorDominio::TasaDeCambioRequerida)?;
    Ok(datos.monto.convertir(divisa_cuenta, tasa)?)
}

/// La descripción conserva la tasa por compatibilidad con lo ya guardado.
///
/// El dato vive además en su propia columna desde la migración 5; aquí se
/// repite solo para que el histórico se lea igual antes y después del cambio.
fn descripcion_comision(tasa: Option<TasaCambio>) -> String {
    match tasa {
        Some(t) => format!("Comisión 0.20% Pago Tarjeta (Tasa {})", t.valor()),
        None => "Comisión 0.20% Pago Tarjeta".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dominio::dinero::Divisa;
    use crate::puertos::dobles::AlmacenEnMemoria;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }
    fn usd(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Usd).unwrap()
    }

    fn almacen() -> AlmacenEnMemoria {
        AlmacenEnMemoria::nuevo()
            .con_categoria(1, "Otros")
            .con_cuenta(10, "Cuenta Ahorros DOP", dop(500_000.0))
            .con_cuenta(11, "Cuenta Ahorros USD", usd(2_000.0))
            .con_tarjeta(20, dop(0.0))
    }

    fn datos(monto: Dinero, cuenta: Option<i64>, tasa: Option<f64>) -> DatosPago {
        DatosPago {
            tarjeta_id: 20,
            fecha: "16/09/2026".into(),
            monto,
            cuenta_ahorro_id: cuenta,
            tasa_cambio: tasa.map(|t| TasaCambio::nueva(t).unwrap()),
        }
    }

    #[test]
    fn un_abono_en_la_misma_divisa_no_convierte_nada() {
        let mut a = almacen();
        a.ajustar_deuda(20, usd(1_000.0)).unwrap();

        let r = registrar_pago_tarjeta(datos(usd(500.0), Some(11), None), 1, &mut a).unwrap();

        assert_eq!(a.deuda_en(20, Divisa::Usd), usd(500.0));
        assert_eq!(a.saldo_de(11), usd(1_499.0), "500 más 1.00 de comisión");
        assert_eq!(r.comision, Some(usd(1.0)));
    }

    #[test]
    fn cruzar_divisas_sin_tasa_se_rechaza_nombrando_la_causa() {
        let mut a = almacen();
        a.ajustar_deuda(20, usd(1_000.0)).unwrap();

        let r = registrar_pago_tarjeta(datos(usd(500.0), Some(10), None), 1, &mut a);

        assert!(matches!(
            r,
            Err(ErrorAplicacion::Dominio(ErrorDominio::TasaDeCambioRequerida))
        ));
    }

    #[test]
    fn un_abono_rechazado_no_mueve_ningun_saldo() {
        // La regla se comprueba antes de tocar nada, de modo que ni siquiera
        // hace falta una transacción para que esto se cumpla.
        let mut a = almacen();
        a.ajustar_deuda(20, usd(1_000.0)).unwrap();

        let _ = registrar_pago_tarjeta(datos(usd(500.0), Some(10), None), 1, &mut a);

        assert_eq!(a.saldo_de(10), dop(500_000.0));
        assert_eq!(a.deuda_en(20, Divisa::Usd), usd(1_000.0));
        assert_eq!(a.total_gastos(), 0);
    }

    #[test]
    fn un_abono_sin_cuenta_solo_reduce_la_deuda() {
        let mut a = almacen();
        a.ajustar_deuda(20, dop(5_000.0)).unwrap();

        let r = registrar_pago_tarjeta(datos(dop(3_000.0), None, None), 1, &mut a).unwrap();

        assert_eq!(a.deuda_de(20), dop(2_000.0));
        assert_eq!(r.debitado, None, "ninguna cuenta pagó");
        assert_eq!(a.total_gastos(), 0, "y no hay comisión que anotar");
    }

    #[test]
    fn el_abono_queda_enlazado_a_su_comision_desde_el_primer_momento() {
        let mut a = almacen();
        a.ajustar_deuda(20, dop(5_000.0)).unwrap();

        let r = registrar_pago_tarjeta(datos(dop(3_000.0), Some(10), None), 1, &mut a).unwrap();

        let guardado = a.obtener_pago(r.id).unwrap();
        assert!(guardado.gasto_comision_id.is_some(), "sin fila intermedia sin vínculo");
        assert_eq!(guardado.cuenta_ahorro_id, Some(10));
    }

    #[test]
    fn abonar_mas_que_la_deuda_deja_saldo_a_favor_y_no_cero() {
        let mut a = almacen();
        a.ajustar_deuda(20, dop(1_000.0)).unwrap();

        registrar_pago_tarjeta(datos(dop(3_000.0), None, None), 1, &mut a).unwrap();

        assert_eq!(a.deuda_de(20), dop(-2_000.0), "resolución de H5");
    }

    #[test]
    fn h15_una_tasa_sobrante_ya_no_infla_el_debito() {
        // Conducta anterior: 3 000 x 60 = 180 000 debitados de una cuenta en
        // pesos por un abono en pesos. Nadie la probaba, y la interfaz deja
        // el campo de tasa siempre visible, así que era alcanzable tecleando.
        let mut a = almacen();
        a.ajustar_deuda(20, dop(5_000.0)).unwrap();

        let r = registrar_pago_tarjeta(datos(dop(3_000.0), Some(10), Some(60.0)), 1, &mut a)
            .unwrap();

        assert_eq!(r.debitado, Some(dop(3_000.0)), "la tasa se ignora sin cruce de divisa");
        assert_eq!(a.saldo_de(10), dop(496_994.0), "3 000 más 6 de comisión");
    }

    #[test]
    fn registrar_y_revertir_un_abono_deja_todo_como_estaba() {
        use super::super::revertir_pago_tarjeta::revertir_pago_tarjeta;

        let mut a = almacen();
        a.ajustar_deuda(20, usd(1_000.0)).unwrap();
        let deuda = a.deuda_en(20, Divisa::Usd);
        let saldo = a.saldo_de(10);

        let r =
            registrar_pago_tarjeta(datos(usd(500.0), Some(10), Some(60.0)), 1, &mut a).unwrap();
        revertir_pago_tarjeta(r.id, &mut a).unwrap();

        assert_eq!(a.deuda_en(20, Divisa::Usd), deuda);
        assert_eq!(a.saldo_de(10), saldo);
        assert_eq!(a.total_gastos(), 0);
    }
}
