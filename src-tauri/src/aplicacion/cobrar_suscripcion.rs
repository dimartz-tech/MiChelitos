//! Caso de uso: cobrar el período de una suscripción a su tarjeta.
//!
//! Un cargo recurrente **es un consumo con tarjeta**: sube la deuda y queda
//! como gasto. Hasta ahora se escribía con SQL directo, duplicando lo que
//! `registrar_gasto` ya hace, y el importe viajaba como `f64` sin pasar por
//! `Dinero`.
//!
//! ## Lo que el SQL directo se saltaba
//!
//! `registrar_gasto` decide, según la política de la tarjeta, si un consumo en
//! divisa queda **pendiente de liquidar**: un emisor que traduce a moneda
//! local no informa el importe hasta después, y estimarlo nunca cuadraría con
//! el estado de cuenta. El cobro directo cargaba siempre la deuda en la
//! divisa de origen y dejaba el gasto como si no hubiera nada que traducir,
//! de modo que **un cargo en dólares a una tarjeta que traduce jamás podía
//! liquidarse**: no figuraba como pendiente.
//!
//! No es teórico. Diez de las once suscripciones del titular son en dólares y
//! las cobran dos tarjetas; la política se declara por tarjeta y las dos
//! pueden diferir.
//!
//! Delegar en `registrar_gasto` corrige eso sin escribir la regla otra vez, y
//! además hace que cobrar una suscripción a una tarjeta inexistente **falle**:
//! el `UPDATE` directo no se enteraba de que no había actualizado nada.

use super::registrar_gasto::{registrar_gasto, DatosGasto};
use super::ErrorAplicacion;
use crate::dominio::dinero::Dinero;
use crate::dominio::errores::ErrorDominio;
use crate::dominio::gasto::MetodoPago;
use crate::puertos::repositorios::AlmacenGastos;

/// Lo que hace falta para cobrar un período.
#[derive(Debug, Clone)]
pub struct DatosCobro {
    pub plataforma: String,
    pub monto: Dinero,
    pub tarjeta_id: i64,
    /// Fecha **del vencimiento**, no la de ejecución: ver `dominio::suscripcion`.
    pub fecha: String,
    pub categoria_id: i64,
}

/// El texto del gasto que deja un cargo recurrente.
///
/// Es una constante con nombre porque el historial de gastos lo muestra tal
/// cual, y quien busque los cargos de suscripciones lo hace por este prefijo.
pub fn descripcion_del_cargo(plataforma: &str) -> String {
    format!("Cargo recurrente: {}", plataforma)
}

pub fn cobrar_suscripcion(
    datos: DatosCobro,
    almacen: &mut impl AlmacenGastos,
) -> Result<i64, ErrorAplicacion> {
    // Se valida antes de tocar nada: un importe que no es positivo no cobraría,
    // abonaría a la tarjeta.
    if datos.monto.es_cero() || datos.monto.es_negativo() {
        return Err(ErrorDominio::SuscripcionSinImporte.into());
    }

    registrar_gasto(
        DatosGasto {
            fecha: datos.fecha,
            monto: datos.monto,
            descripcion: descripcion_del_cargo(&datos.plataforma),
            categoria_id: datos.categoria_id,
            metodo: Some(MetodoPago::Tarjeta),
            metodo_texto: "tarjeta".to_string(),
            es_lbtr: false,
            tarjeta_id: Some(datos.tarjeta_id),
            cuenta_ahorro_id: None,
            tasa_cambio: None,
        },
        almacen,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dominio::conversion::EstadoConversion;
    use crate::dominio::dinero::Divisa;
    use crate::dominio::tarjeta::PoliticaLiquidacion;
    use crate::puertos::dobles::AlmacenEnMemoria;
    use crate::puertos::repositorios::RepositorioGastos;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }
    fn usd(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Usd).unwrap()
    }

    fn almacen() -> AlmacenEnMemoria {
        AlmacenEnMemoria::nuevo()
            .con_categoria(1, "Suscripciones")
            .con_tarjeta(20, dop(0.0))
    }

    fn cobro(monto: Dinero) -> DatosCobro {
        DatosCobro {
            plataforma: "Plataforma".into(),
            monto,
            tarjeta_id: 20,
            fecha: "30/08/2026".into(),
            categoria_id: 1,
        }
    }

    #[test]
    fn el_cobro_sube_la_deuda_y_queda_como_gasto_de_la_tarjeta() {
        let mut a = almacen();
        let id = cobrar_suscripcion(cobro(dop(500.0)), &mut a).unwrap();

        assert_eq!(a.deuda_de(20), dop(500.0));
        let g = a.obtener(id).unwrap();
        assert_eq!(g.monto, dop(500.0));
        assert_eq!(g.tarjeta_id, Some(20));
        assert_eq!(g.metodo_pago, "tarjeta");
        assert!(g.cargos.es_cero(), "un cargo recurrente no devenga cargos de transferencia");
    }

    #[test]
    fn un_cobro_en_dolares_a_una_tarjeta_que_traduce_queda_pendiente_de_liquidar() {
        // **Lo que el SQL directo se saltaba.** Sin esto, el cargo jamás
        // figuraría como pendiente y no podría liquidarse.
        let mut a = almacen().con_politica(20, PoliticaLiquidacion::TraduceAMonedaLocal);
        let id = cobrar_suscripcion(cobro(usd(15.0)), &mut a).unwrap();

        assert_eq!(a.obtener(id).unwrap().estado_conversion, EstadoConversion::Pendiente);
        assert_eq!(a.deuda_en(20, Divisa::Usd), usd(15.0), "la deuda queda en dólares hasta liquidar");
    }

    #[test]
    fn un_cobro_en_dolares_a_una_tarjeta_que_conserva_la_divisa_no_queda_pendiente() {
        let mut a = almacen().con_politica(20, PoliticaLiquidacion::EnDivisaDeOrigen);
        let id = cobrar_suscripcion(cobro(usd(15.0)), &mut a).unwrap();
        assert_eq!(a.obtener(id).unwrap().estado_conversion, EstadoConversion::NoAplica);
    }

    #[test]
    fn un_cobro_en_pesos_nunca_queda_pendiente_ni_aunque_la_tarjeta_traduzca() {
        let mut a = almacen().con_politica(20, PoliticaLiquidacion::TraduceAMonedaLocal);
        let id = cobrar_suscripcion(cobro(dop(500.0)), &mut a).unwrap();
        assert_eq!(a.obtener(id).unwrap().estado_conversion, EstadoConversion::NoAplica);
    }

    #[test]
    fn un_importe_que_no_es_positivo_se_rechaza_sin_mover_nada() {
        for monto in [0.0, -15.0] {
            let mut a = almacen();
            let r = cobrar_suscripcion(cobro(dop(monto)), &mut a);
            assert!(
                matches!(r, Err(ErrorAplicacion::Dominio(ErrorDominio::SuscripcionSinImporte))),
                "aceptó {monto}"
            );
            assert_eq!(a.deuda_de(20), dop(0.0), "la deuda no se movió");
            assert!(a.gastos.is_empty());
        }
    }

    #[test]
    fn cobrar_a_una_tarjeta_inexistente_falla_y_no_deja_gasto() {
        // El `UPDATE` directo no se enteraba de que no había actualizado nada.
        let mut a = almacen();
        let mut d = cobro(dop(500.0));
        d.tarjeta_id = 999;
        assert!(cobrar_suscripcion(d, &mut a).is_err());
        assert!(a.gastos.is_empty(), "sin gasto huérfano");
    }

    #[test]
    fn el_texto_del_cargo_conserva_el_prefijo_por_el_que_se_busca() {
        assert_eq!(descripcion_del_cargo("Netflix"), "Cargo recurrente: Netflix");
    }
}
