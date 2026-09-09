//! Una conversión de divisa, entendida como hecho y no como cálculo suelto.
//!
//! Guarda juntos el importe de origen, el de destino y la tasa aplicada, de
//! modo que los tres sean siempre coherentes entre sí. Eso descarta el
//! descuadre silencioso de guardar una tasa que no cuadre con los importes,
//! que nadie detectaría al revisar los datos.
//!
//! Esta versión cubre el caso en que **la tasa es un hecho de la operación**:
//! el titular la conoce porque el banco se la aplica al ejecutar. El caso en
//! que la tasa la fija el emisor más tarde, con su ciclo de liquidación
//! pendiente, está modelado en `modelo_conversion_divisa.md` y llega aparte.

use super::dinero::{Dinero, Divisa, TasaCambio};
use super::errores::ErrorDominio;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Conversion {
    origen: Dinero,
    destino: Dinero,
    tasa: TasaCambio,
}

impl Conversion {
    /// Construye a partir de la tasa declarada, calculando el destino.
    ///
    /// Convertir a la misma divisa no es una conversión: se rechaza en lugar
    /// de devolver una identidad, para que quede claro en el punto de llamada
    /// que no hacía falta tasa alguna.
    pub fn con_tasa(
        origen: Dinero,
        destino: Divisa,
        tasa: TasaCambio,
    ) -> Result<Conversion, ErrorDominio> {
        if origen.divisa() == destino {
            return Err(ErrorDominio::DivisasIncompatibles {
                esperada: destino,
                recibida: origen.divisa(),
            });
        }
        let convertido = origen.convertir(destino, tasa)?;
        Ok(Conversion { origen, destino: convertido, tasa })
    }

    /// Deduce la tasa a partir de los dos importes.
    ///
    /// Es el camino de la liquidación: el emisor comunica cuánto cargó, nunca
    /// a qué tasa lo hizo. El importe manda y la tasa queda como información
    /// derivada.
    pub fn desde_importes(origen: Dinero, destino: Dinero) -> Result<Conversion, ErrorDominio> {
        if origen.divisa() == destino.divisa() {
            return Err(ErrorDominio::DivisasIncompatibles {
                esperada: destino.divisa(),
                recibida: origen.divisa(),
            });
        }
        if origen.es_cero() || destino.es_cero() {
            return Err(ErrorDominio::TasaDeCambioRequerida);
        }
        // TasaCambio se expresa siempre en moneda local por unidad de divisa,
        // así que la razón se toma en ese sentido con independencia de cuál
        // sea el origen.
        let (local, extranjera) = match origen.divisa() {
            Divisa::Usd => (destino.centavos() as f64, origen.centavos() as f64),
            Divisa::Dop => (origen.centavos() as f64, destino.centavos() as f64),
        };
        let tasa = TasaCambio::nueva(local / extranjera)?;
        Ok(Conversion { origen, destino, tasa })
    }

    /// Reconstruye una conversión ya persistida.
    ///
    /// El importe de destino se toma tal como se guardó, porque **es el que
    /// realmente salió de la cuenta**. Recalcularlo desde la tasa podría
    /// desviarse en centavos y dejar la reversión sin cuadrar.
    pub fn reconstruir(
        origen: Dinero,
        destino: Dinero,
        tasa: TasaCambio,
    ) -> Result<Conversion, ErrorDominio> {
        if origen.divisa() == destino.divisa() {
            return Err(ErrorDominio::DivisasIncompatibles {
                esperada: destino.divisa(),
                recibida: origen.divisa(),
            });
        }
        Ok(Conversion { origen, destino, tasa })
    }

    pub fn origen(&self) -> Dinero {
        self.origen
    }

    pub fn destino(&self) -> Dinero {
        self.destino
    }

    pub fn tasa(&self) -> TasaCambio {
        self.tasa
    }
}

/// Situación de un consumo respecto a la conversión de divisa.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EstadoConversion {
    /// Se liquida en su propia divisa. No hay nada que convertir.
    NoAplica,
    /// Hecho en divisa extranjera con una tarjeta que traduce, y el emisor aún
    /// no ha fijado el importe en moneda local. **Ese importe no existe
    /// todavía**: no se estima.
    Pendiente,
    Liquidada(Conversion),
}

impl EstadoConversion {
    pub fn conversion(&self) -> Option<Conversion> {
        match self {
            EstadoConversion::Liquidada(c) => Some(*c),
            _ => None,
        }
    }

    pub fn esta_pendiente(&self) -> bool {
        matches!(self, EstadoConversion::Pendiente)
    }

    pub fn codigo(&self) -> Option<&'static str> {
        match self {
            EstadoConversion::NoAplica => None,
            EstadoConversion::Pendiente => Some("pendiente"),
            EstadoConversion::Liquidada(_) => Some("liquidado"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }
    fn usd(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Usd).unwrap()
    }
    fn tasa(v: f64) -> TasaCambio {
        TasaCambio::nueva(v).unwrap()
    }

    #[test]
    fn convertir_dolares_a_pesos_multiplica_por_la_tasa() {
        let c = Conversion::con_tasa(usd(100.0), Divisa::Dop, tasa(60.0)).unwrap();
        assert_eq!(c.origen(), usd(100.0));
        assert_eq!(c.destino(), dop(6000.0));
        assert_eq!(c.tasa().valor(), 60.0);
    }

    #[test]
    fn convertir_pesos_a_dolares_divide_por_la_tasa() {
        let c = Conversion::con_tasa(dop(6000.0), Divisa::Usd, tasa(60.0)).unwrap();
        assert_eq!(c.destino(), usd(100.0));
    }

    #[test]
    fn el_destino_se_redondea_a_centavos() {
        // 100.00 USD x 58.755 = 5 875.50 exactos
        let c = Conversion::con_tasa(usd(100.0), Divisa::Dop, tasa(58.755)).unwrap();
        assert_eq!(c.destino().centavos(), 587_550);
    }

    #[test]
    fn convertir_a_la_misma_divisa_es_error() {
        // No es una conversión; exigir tasa ahí sería pedir un dato inútil.
        assert!(Conversion::con_tasa(dop(100.0), Divisa::Dop, tasa(60.0)).is_err());
    }

    #[test]
    fn una_tasa_de_cero_no_llega_a_construirse() {
        assert_eq!(TasaCambio::nueva(0.0).unwrap_err(), ErrorDominio::TasaDeCambioRequerida);
    }

    #[test]
    fn reconstruir_conserva_el_importe_guardado_sin_recalcularlo() {
        // Aunque la tasa no reproduzca exactamente el destino, manda el
        // importe que realmente salió de la cuenta.
        let c = Conversion::reconstruir(usd(100.0), dop(6000.01), tasa(60.0)).unwrap();
        assert_eq!(c.destino(), dop(6000.01));
    }

    #[test]
    fn reconstruir_con_divisas_iguales_es_error() {
        assert!(Conversion::reconstruir(dop(100.0), dop(6000.0), tasa(60.0)).is_err());
    }

    #[test]
    fn ida_y_vuelta_recupera_el_importe_original() {
        let t = tasa(58.75);
        let ida = Conversion::con_tasa(usd(500.0), Divisa::Dop, t).unwrap();
        let vuelta = Conversion::con_tasa(ida.destino(), Divisa::Usd, t).unwrap();
        assert_eq!(vuelta.destino(), usd(500.0));
    }

    // --- Tasa deducida a partir de los importes ---

    #[test]
    fn deducir_la_tasa_de_una_liquidacion_en_pesos() {
        // El emisor informa: 100.00 USD se cargaron como 6 050.00 DOP.
        let c = Conversion::desde_importes(usd(100.0), dop(6050.0)).unwrap();
        assert_eq!(c.tasa().valor(), 60.5);
        assert_eq!(c.destino(), dop(6050.0), "el importe del emisor manda");
    }

    #[test]
    fn la_tasa_deducida_conserva_los_decimales_que_haga_falta() {
        // 200.00 USD -> 12 270.00 DOP implica 61.35
        let c = Conversion::desde_importes(usd(200.0), dop(12270.0)).unwrap();
        assert!((c.tasa().valor() - 61.35).abs() < 1e-9);
    }

    #[test]
    fn deducir_en_sentido_inverso_da_la_misma_convencion_de_tasa() {
        // La tasa siempre es moneda local por unidad de divisa extranjera.
        let c = Conversion::desde_importes(dop(6050.0), usd(100.0)).unwrap();
        assert_eq!(c.tasa().valor(), 60.5);
    }

    #[test]
    fn no_se_deduce_tasa_de_importes_en_la_misma_divisa() {
        assert!(Conversion::desde_importes(dop(100.0), dop(6050.0)).is_err());
    }

    #[test]
    fn no_se_deduce_tasa_si_algun_importe_es_cero() {
        assert!(Conversion::desde_importes(usd(0.0), dop(6050.0)).is_err());
        assert!(Conversion::desde_importes(usd(100.0), dop(0.0)).is_err());
    }

    #[test]
    fn deducir_y_volver_a_aplicar_reproduce_el_importe() {
        let c = Conversion::desde_importes(usd(100.0), dop(6050.0)).unwrap();
        let reaplicada = Conversion::con_tasa(usd(100.0), Divisa::Dop, c.tasa()).unwrap();
        assert_eq!(reaplicada.destino(), dop(6050.0));
    }

    // --- Estado de conversión ---

    #[test]
    fn un_estado_pendiente_no_tiene_conversion_ni_importe_local() {
        let e = EstadoConversion::Pendiente;
        assert!(e.esta_pendiente());
        assert!(e.conversion().is_none(), "el importe en moneda local no existe todavía");
        assert_eq!(e.codigo(), Some("pendiente"));
    }

    #[test]
    fn un_estado_liquidado_expone_su_conversion() {
        let c = Conversion::desde_importes(usd(100.0), dop(6050.0)).unwrap();
        let e = EstadoConversion::Liquidada(c);
        assert!(!e.esta_pendiente());
        assert_eq!(e.conversion().unwrap().destino(), dop(6050.0));
        assert_eq!(e.codigo(), Some("liquidado"));
    }

    #[test]
    fn el_estado_que_no_aplica_no_se_persiste_como_texto() {
        assert_eq!(EstadoConversion::NoAplica.codigo(), None);
    }
}
