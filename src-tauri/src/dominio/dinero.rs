use super::errores::ErrorDominio;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Divisa {
    #[serde(rename = "DOP")]
    Dop,
    #[serde(rename = "USD")]
    Usd,
}

impl Divisa {
    pub fn codigo(&self) -> &'static str {
        match self {
            Divisa::Dop => "DOP",
            Divisa::Usd => "USD",
        }
    }

    pub fn desde_codigo(codigo: &str) -> Result<Divisa, ErrorDominio> {
        match codigo.trim().to_uppercase().as_str() {
            "DOP" => Ok(Divisa::Dop),
            "USD" => Ok(Divisa::Usd),
            _ => Err(ErrorDominio::DivisaDesconocida { codigo: codigo.to_string() }),
        }
    }
}

/// Tasa de cambio expresada en pesos dominicanos por un dólar (DOP por 1 USD),
/// que es la convención con la que el usuario la introduce en la interfaz.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TasaCambio(f64);

impl TasaCambio {
    pub fn nueva(valor: f64) -> Result<TasaCambio, ErrorDominio> {
        if !valor.is_finite() {
            return Err(ErrorDominio::TasaDeCambioInvalida { valor });
        }
        if valor <= 0.0 {
            return Err(ErrorDominio::TasaDeCambioRequerida);
        }
        Ok(TasaCambio(valor))
    }

    pub fn valor(&self) -> f64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dinero {
    monto: f64,
    divisa: Divisa,
}

impl Dinero {
    pub fn nuevo(monto: f64, divisa: Divisa) -> Result<Dinero, ErrorDominio> {
        if !monto.is_finite() {
            return Err(ErrorDominio::MontoInvalido { valor: monto });
        }
        Ok(Dinero { monto, divisa })
    }

    pub fn cero(divisa: Divisa) -> Dinero {
        Dinero { monto: 0.0, divisa }
    }

    pub fn monto(&self) -> f64 {
        self.monto
    }

    pub fn divisa(&self) -> Divisa {
        self.divisa
    }

    pub fn es_cero(&self) -> bool {
        self.monto == 0.0
    }

    pub fn sumar(&self, otro: &Dinero) -> Result<Dinero, ErrorDominio> {
        self.exigir_misma_divisa(otro)?;
        Dinero::nuevo(self.monto + otro.monto, self.divisa)
    }

    pub fn restar(&self, otro: &Dinero) -> Result<Dinero, ErrorDominio> {
        self.exigir_misma_divisa(otro)?;
        Dinero::nuevo(self.monto - otro.monto, self.divisa)
    }

    /// Convierte a la divisa destino. Si ya está en esa divisa devuelve el mismo
    /// importe y la tasa se ignora, replicando el flujo de abono en igual moneda.
    pub fn convertir(&self, destino: Divisa, tasa: TasaCambio) -> Result<Dinero, ErrorDominio> {
        if self.divisa == destino {
            return Ok(*self);
        }
        let convertido = match (self.divisa, destino) {
            (Divisa::Usd, Divisa::Dop) => self.monto * tasa.valor(),
            (Divisa::Dop, Divisa::Usd) => self.monto / tasa.valor(),
            _ => unreachable!("la igualdad de divisas ya se descartó arriba"),
        };
        Dinero::nuevo(convertido, destino)
    }

    fn exigir_misma_divisa(&self, otro: &Dinero) -> Result<(), ErrorDominio> {
        if self.divisa != otro.divisa {
            return Err(ErrorDominio::DivisasIncompatibles {
                esperada: self.divisa,
                recibida: otro.divisa,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dop(m: f64) -> Dinero {
        Dinero::nuevo(m, Divisa::Dop).unwrap()
    }
    fn usd(m: f64) -> Dinero {
        Dinero::nuevo(m, Divisa::Usd).unwrap()
    }

    // --- Divisa ---

    #[test]
    fn el_codigo_de_divisa_coincide_con_el_almacenado_en_sqlite() {
        assert_eq!(Divisa::Dop.codigo(), "DOP");
        assert_eq!(Divisa::Usd.codigo(), "USD");
    }

    #[test]
    fn la_divisa_se_serializa_como_el_texto_que_espera_la_interfaz() {
        assert_eq!(serde_json::to_string(&Divisa::Dop).unwrap(), "\"DOP\"");
        assert_eq!(serde_json::to_string(&Divisa::Usd).unwrap(), "\"USD\"");
    }

    #[test]
    fn el_codigo_se_interpreta_sin_distinguir_mayusculas_ni_espacios() {
        assert_eq!(Divisa::desde_codigo("dop").unwrap(), Divisa::Dop);
        assert_eq!(Divisa::desde_codigo("  USD ").unwrap(), Divisa::Usd);
    }

    #[test]
    fn una_divisa_no_admitida_es_rechazada() {
        let e = Divisa::desde_codigo("EUR").unwrap_err();
        assert_eq!(e, ErrorDominio::DivisaDesconocida { codigo: "EUR".into() });
    }

    // --- Regla central: no se mezclan divisas ---
    // Es la causa raíz del descuadre corregido a mano en la versión 1.3.4.

    #[test]
    fn sumar_pesos_con_dolares_devuelve_error_en_vez_de_un_total_falso() {
        let e = dop(1000.0).sumar(&usd(50.0)).unwrap_err();
        assert_eq!(
            e,
            ErrorDominio::DivisasIncompatibles { esperada: Divisa::Dop, recibida: Divisa::Usd }
        );
    }

    #[test]
    fn restar_divisas_distintas_tambien_es_error() {
        assert!(usd(50.0).restar(&dop(1000.0)).is_err());
    }

    #[test]
    fn sumar_y_restar_en_la_misma_divisa_opera_con_normalidad() {
        assert_eq!(dop(1000.0).sumar(&dop(250.5)).unwrap(), dop(1250.5));
        assert_eq!(dop(1000.0).restar(&dop(250.5)).unwrap(), dop(749.5));
    }

    #[test]
    fn restar_por_debajo_de_cero_esta_permitido_a_nivel_de_tipo() {
        // El sobregiro de tarjetas es legítimo: el saldo insuficiente lo decide
        // cada caso de uso, no el tipo Dinero.
        assert_eq!(dop(100.0).restar(&dop(150.0)).unwrap().monto(), -50.0);
    }

    // --- Montos inválidos ---

    #[test]
    fn un_monto_no_finito_es_rechazado_al_construirlo() {
        assert!(Dinero::nuevo(f64::NAN, Divisa::Dop).is_err());
        assert!(Dinero::nuevo(f64::INFINITY, Divisa::Dop).is_err());
    }

    #[test]
    fn el_cero_conserva_su_divisa() {
        let c = Dinero::cero(Divisa::Usd);
        assert!(c.es_cero());
        assert_eq!(c.divisa(), Divisa::Usd);
    }

    // --- Tasa de cambio ---

    #[test]
    fn una_tasa_de_cero_pide_la_tasa_en_lugar_de_asumir_una() {
        assert_eq!(TasaCambio::nueva(0.0).unwrap_err(), ErrorDominio::TasaDeCambioRequerida);
    }

    #[test]
    fn una_tasa_negativa_o_no_finita_es_invalida() {
        assert_eq!(
            TasaCambio::nueva(-60.0).unwrap_err(),
            ErrorDominio::TasaDeCambioRequerida
        );
        assert!(matches!(
            TasaCambio::nueva(f64::NAN).unwrap_err(),
            ErrorDominio::TasaDeCambioInvalida { .. }
        ));
    }

    // --- Conversión ---

    #[test]
    fn abonar_500_usd_con_tasa_60_debita_30000_pesos() {
        // Caso del escenario Gherkin de abono multidivisa.
        let tasa = TasaCambio::nueva(60.0).unwrap();
        let equivalente = usd(500.0).convertir(Divisa::Dop, tasa).unwrap();
        assert_eq!(equivalente, dop(30000.0));
    }

    #[test]
    fn la_conversion_inversa_divide_por_la_tasa() {
        let tasa = TasaCambio::nueva(60.0).unwrap();
        assert_eq!(dop(30000.0).convertir(Divisa::Usd, tasa).unwrap(), usd(500.0));
    }

    #[test]
    fn convertir_a_la_misma_divisa_no_altera_el_importe() {
        let tasa = TasaCambio::nueva(60.0).unwrap();
        assert_eq!(usd(500.0).convertir(Divisa::Usd, tasa).unwrap(), usd(500.0));
    }

    #[test]
    fn ida_y_vuelta_recupera_el_importe_original() {
        let tasa = TasaCambio::nueva(58.75).unwrap();
        let vuelta = usd(500.0)
            .convertir(Divisa::Dop, tasa)
            .unwrap()
            .convertir(Divisa::Usd, tasa)
            .unwrap();
        assert!((vuelta.monto() - 500.0).abs() < 1e-9);
    }
}
