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
}
