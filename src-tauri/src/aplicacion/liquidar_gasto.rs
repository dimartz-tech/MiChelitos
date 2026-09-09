//! Caso de uso: liquidar un consumo pendiente.
//!
//! Cierra el ciclo que abre un consumo hecho en divisa extranjera con una
//! tarjeta que traduce a moneda local. Hasta este momento el importe en moneda
//! local **no existía**: lo fija el emisor, y el titular lo conoce al recibir
//! el estado de cuenta.
//!
//! La entrada es el **importe**, no la tasa. El emisor comunica cuánto cargó,
//! nunca a qué tasa lo hizo; la tasa se deduce y queda como dato derivado.

use super::ErrorAplicacion;
use crate::dominio::conversion::{Conversion, EstadoConversion};
use crate::dominio::dinero::Dinero;
use crate::dominio::errores::ErrorDominio;
use crate::dominio::gasto::MetodoPago;
use crate::puertos::repositorios::*;

pub fn liquidar_gasto(
    gasto_id: i64,
    monto_en_moneda_local: Dinero,
    almacen: &mut impl AlmacenGastos,
) -> Result<Conversion, ErrorAplicacion> {
    let gasto = almacen.obtener(gasto_id)?;

    if !gasto.estado_conversion.esta_pendiente() {
        return Err(ErrorAplicacion::Almacen(ErrorAlmacen::Fallo(
            "El consumo no está pendiente de liquidación.".to_string(),
        )));
    }

    // El importe del emisor manda; la tasa se deduce de los dos importes.
    let conversion = Conversion::desde_importes(gasto.monto, monto_en_moneda_local)?;

    // El saldo se traslada entre divisas de la misma tarjeta, igual que hace
    // el emisor: baja en la de origen y sube en la moneda local.
    match MetodoPago::desde_codigo(&gasto.metodo_pago) {
        Some(MetodoPago::Tarjeta) => {
            let tarjeta_id = gasto.tarjeta_id.ok_or(ErrorDominio::MontoInvalido { valor: 0.0 })?;
            almacen.ajustar_deuda(tarjeta_id, negativo(gasto.monto)?)?;
            almacen.ajustar_deuda(tarjeta_id, conversion.destino())?;
        }
        _ => {
            return Err(ErrorAplicacion::Almacen(ErrorAlmacen::Fallo(
                "Solo los consumos con tarjeta pueden quedar pendientes de liquidación."
                    .to_string(),
            )))
        }
    }

    almacen.liquidar(gasto_id, EstadoConversion::Liquidada(conversion))?;
    Ok(conversion)
}

fn negativo(importe: Dinero) -> Result<Dinero, ErrorAplicacion> {
    Ok(Dinero::cero(importe.divisa()).restar(&importe)?)
}

#[cfg(test)]
mod tests {
    use super::super::registrar_gasto::{registrar_gasto, DatosGasto};
    use super::*;
    use crate::dominio::dinero::Divisa;
    use crate::dominio::tarjeta::PoliticaLiquidacion;
    use crate::puertos::dobles::AlmacenEnMemoria;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }
    fn usd(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Usd).unwrap()
    }

    /// Dos tarjetas: la 20 traduce a moneda local, la 21 liquida en origen.
    fn almacen() -> AlmacenEnMemoria {
        AlmacenEnMemoria::nuevo()
            .con_categoria(1, "Compras online")
            .con_tarjeta(20, dop(0.0))
            .con_politica(20, PoliticaLiquidacion::TraduceAMonedaLocal)
            .con_tarjeta(21, dop(0.0))
            .con_politica(21, PoliticaLiquidacion::EnDivisaDeOrigen)
    }

    fn compra_en_divisa(tarjeta_id: i64, monto: f64) -> DatosGasto {
        DatosGasto {
            fecha: "10/09/2026".into(),
            monto: usd(monto),
            descripcion: "Compra en el exterior".into(),
            categoria_id: 1,
            metodo: Some(MetodoPago::Tarjeta),
            metodo_texto: "tarjeta".into(),
            es_lbtr: false,
            tarjeta_id: Some(tarjeta_id),
            cuenta_ahorro_id: None,
            tasa_cambio: None,
        }
    }

    // --- Caso A del modelo: el mismo consumo, dos políticas ---

    #[test]
    fn caso_a_con_tarjeta_que_traduce_el_consumo_queda_pendiente() {
        let mut a = almacen();
        let id = registrar_gasto(compra_en_divisa(20, 100.0), &mut a).unwrap();

        let g = a.obtener(id).unwrap();
        assert!(g.estado_conversion.esta_pendiente());
        assert_eq!(a.deuda_en(20, Divisa::Usd), usd(100.0), "la deuda sube en dólares, como hace el emisor");
        assert!(
            g.estado_conversion.conversion().is_none(),
            "el importe en moneda local no existe todavía"
        );
    }

    #[test]
    fn caso_a_con_tarjeta_que_liquida_en_origen_no_hay_nada_pendiente() {
        let mut a = almacen();
        let id = registrar_gasto(compra_en_divisa(21, 100.0), &mut a).unwrap();

        assert_eq!(a.obtener(id).unwrap().estado_conversion, EstadoConversion::NoAplica);
        assert_eq!(a.deuda_en(21, Divisa::Usd), usd(100.0), "y se queda en dólares para siempre");
    }

    #[test]
    fn una_compra_en_moneda_local_nunca_queda_pendiente() {
        let mut a = almacen();
        let mut d = compra_en_divisa(20, 100.0);
        d.monto = dop(6000.0);
        let id = registrar_gasto(d, &mut a).unwrap();

        assert_eq!(a.obtener(id).unwrap().estado_conversion, EstadoConversion::NoAplica);
    }

    // --- El ciclo de liquidación ---

    #[test]
    fn liquidar_traslada_el_saldo_entre_divisas_y_deduce_la_tasa() {
        let mut a = almacen();
        let id = registrar_gasto(compra_en_divisa(20, 100.0), &mut a).unwrap();

        // El emisor informa que cargó 6 050.00 en moneda local.
        let conversion = liquidar_gasto(id, dop(6050.0), &mut a).unwrap();

        assert_eq!(conversion.tasa().valor(), 60.5, "la tasa se deduce, no se pide");
        assert_eq!(a.deuda_en(20, Divisa::Usd), usd(0.0), "baja de la divisa de origen");
        assert_eq!(a.deuda_de(20), dop(6050.0), "y sube en moneda local");
        let g = a.obtener(id).unwrap();
        assert!(!g.estado_conversion.esta_pendiente());
        assert_eq!(g.monto, usd(100.0), "el consumo conserva su divisa de origen");
        assert_eq!(g.monto_debitado(), dop(6050.0));
    }

    // --- Caso B del modelo: la tasa que no se adivina ---

    #[test]
    fn caso_b_el_importe_del_emisor_manda_aunque_sorprenda() {
        let mut a = almacen();
        let id = registrar_gasto(compra_en_divisa(20, 200.0), &mut a).unwrap();

        // El titular esperaba 60.00; el emisor aplicó 61.35.
        let conversion = liquidar_gasto(id, dop(12270.0), &mut a).unwrap();

        assert!((conversion.tasa().valor() - 61.35).abs() < 1e-9);
        assert_eq!(a.deuda_de(20), dop(12270.0), "sin descuadre: nunca hubo cifra estimada");
    }

    // --- Errores ---

    #[test]
    fn no_se_puede_liquidar_lo_que_no_esta_pendiente() {
        let mut a = almacen();
        let id = registrar_gasto(compra_en_divisa(21, 100.0), &mut a).unwrap();
        assert!(liquidar_gasto(id, dop(6050.0), &mut a).is_err());
    }

    #[test]
    fn liquidar_dos_veces_falla_la_segunda() {
        let mut a = almacen();
        let id = registrar_gasto(compra_en_divisa(20, 100.0), &mut a).unwrap();
        liquidar_gasto(id, dop(6050.0), &mut a).unwrap();

        assert!(liquidar_gasto(id, dop(6050.0), &mut a).is_err(), "no se liquida dos veces");
        assert_eq!(a.deuda_de(20), dop(6050.0), "ni se duplica el traslado de saldo");
    }

    #[test]
    fn liquidar_con_un_importe_en_la_misma_divisa_es_error() {
        let mut a = almacen();
        let id = registrar_gasto(compra_en_divisa(20, 100.0), &mut a).unwrap();
        assert!(liquidar_gasto(id, usd(100.0), &mut a).is_err());
    }

    #[test]
    fn liquidar_con_importe_cero_es_error() {
        let mut a = almacen();
        let id = registrar_gasto(compra_en_divisa(20, 100.0), &mut a).unwrap();
        assert!(liquidar_gasto(id, dop(0.0), &mut a).is_err());
    }
}
