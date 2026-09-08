//! Representación de importes monetarios.
//!
//! `Dinero` guarda **centavos como entero**, no unidades como coma flotante.
//! Ambas divisas del sistema (DOP y USD) tienen exactamente dos decimales, de
//! modo que un `i64` de centavos representa cualquier importe de forma exacta
//! y la aritmética de sumas y restas no acumula error.
//!
//! La consecuencia de diseño que más importa: un porcentaje sobre un importe
//! **no puede** producir una fracción de centavo silenciosa. Todo cálculo que
//! divida o aplique una tasa pasa por un único punto —[`Dinero::porcentaje`]—
//! donde el redondeo es explícito y verificable.
//!
//! La conversión a `f64` existe solo para la frontera con SQLite, cuyas
//! columnas siguen siendo `REAL` hasta la fase de migración del esquema.

use super::errores::ErrorDominio;
use serde::{Deserialize, Serialize};

/// Centavos en una unidad monetaria.
const CENTAVOS_POR_UNIDAD: f64 = 100.0;

/// Mayor entero que un `f64` representa de forma exacta (2^53 − 1). Más allá
/// de este valor la conversión desde coma flotante deja de ser fiable, así que
/// se rechaza en lugar de saturar en silencio.
const MAX_CENTAVOS_EXACTOS: f64 = 9_007_199_254_740_991.0;

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

/// Tasa de cambio en pesos dominicanos por un dólar (DOP por 1 USD), que es la
/// convención con la que el usuario la introduce en la interfaz.
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

// Sin PartialOrd ni Ord a propósito: ordenar importes de divisas distintas no
// significa nada. Cuando haga falta comparar, será con un método que exija la
// misma divisa, igual que sumar y restar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dinero {
    centavos: i64,
    divisa: Divisa,
}

impl Dinero {
    /// Construye a partir de unidades (pesos o dólares), redondeando a centavos
    /// con la regla comercial: la mitad se aleja del cero.
    ///
    /// Es el constructor de la frontera: los importes que llegan de la interfaz
    /// y de las columnas `REAL` de SQLite entran por aquí.
    pub fn nuevo(unidades: f64, divisa: Divisa) -> Result<Dinero, ErrorDominio> {
        if !unidades.is_finite() {
            return Err(ErrorDominio::MontoInvalido { valor: unidades });
        }
        let centavos = (unidades * CENTAVOS_POR_UNIDAD).round();
        if centavos.abs() > MAX_CENTAVOS_EXACTOS {
            return Err(ErrorDominio::MontoInvalido { valor: unidades });
        }
        Ok(Dinero { centavos: centavos as i64, divisa })
    }

    /// Construye a partir de centavos exactos, sin redondeo ni pérdida.
    pub fn de_centavos(centavos: i64, divisa: Divisa) -> Dinero {
        Dinero { centavos, divisa }
    }

    pub fn cero(divisa: Divisa) -> Dinero {
        Dinero { centavos: 0, divisa }
    }

    pub fn centavos(&self) -> i64 {
        self.centavos
    }

    /// Importe en unidades. Solo para la frontera con SQLite y la interfaz.
    pub fn unidades(&self) -> f64 {
        self.centavos as f64 / CENTAVOS_POR_UNIDAD
    }

    pub fn divisa(&self) -> Divisa {
        self.divisa
    }

    pub fn es_cero(&self) -> bool {
        self.centavos == 0
    }

    pub fn es_negativo(&self) -> bool {
        self.centavos < 0
    }

    pub fn sumar(&self, otro: &Dinero) -> Result<Dinero, ErrorDominio> {
        self.exigir_misma_divisa(otro)?;
        Ok(Dinero { centavos: self.centavos + otro.centavos, divisa: self.divisa })
    }

    pub fn restar(&self, otro: &Dinero) -> Result<Dinero, ErrorDominio> {
        self.exigir_misma_divisa(otro)?;
        Ok(Dinero { centavos: self.centavos - otro.centavos, divisa: self.divisa })
    }

    /// Aplica una tasa proporcional redondeando a centavos.
    ///
    /// **Único punto del dominio donde un cálculo puede perder precisión.**
    /// Concentrarlo aquí es lo que impide que una retención o un descuento
    /// arrastren fracciones de centavo hasta los saldos.
    pub fn porcentaje(&self, tasa: f64) -> Result<Dinero, ErrorDominio> {
        if !tasa.is_finite() {
            return Err(ErrorDominio::MontoInvalido { valor: tasa });
        }
        let centavos = (self.centavos as f64 * tasa).round();
        if centavos.abs() > MAX_CENTAVOS_EXACTOS {
            return Err(ErrorDominio::MontoInvalido { valor: centavos });
        }
        Ok(Dinero { centavos: centavos as i64, divisa: self.divisa })
    }

    /// Convierte a la divisa destino redondeando a centavos. Si ya está en esa
    /// divisa devuelve el mismo importe y la tasa se ignora.
    pub fn convertir(&self, destino: Divisa, tasa: TasaCambio) -> Result<Dinero, ErrorDominio> {
        if self.divisa == destino {
            return Ok(*self);
        }
        let centavos = match (self.divisa, destino) {
            (Divisa::Usd, Divisa::Dop) => self.centavos as f64 * tasa.valor(),
            (Divisa::Dop, Divisa::Usd) => self.centavos as f64 / tasa.valor(),
            _ => unreachable!("la igualdad de divisas ya se descartó arriba"),
        }
        .round();

        if !centavos.is_finite() || centavos.abs() > MAX_CENTAVOS_EXACTOS {
            return Err(ErrorDominio::MontoInvalido { valor: centavos });
        }
        Ok(Dinero { centavos: centavos as i64, divisa: destino })
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

    fn dop(unidades: f64) -> Dinero {
        Dinero::nuevo(unidades, Divisa::Dop).unwrap()
    }
    fn usd(unidades: f64) -> Dinero {
        Dinero::nuevo(unidades, Divisa::Usd).unwrap()
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

    // --- Representación en centavos ---

    #[test]
    fn el_importe_se_guarda_como_centavos_enteros() {
        assert_eq!(dop(1234.56).centavos(), 123_456);
        assert_eq!(dop(0.01).centavos(), 1);
        assert_eq!(dop(-50.0).centavos(), -5_000);
    }

    #[test]
    fn las_unidades_devuelven_el_importe_para_la_frontera_con_sqlite() {
        assert_eq!(dop(1234.56).unidades(), 1234.56);
        assert_eq!(Dinero::de_centavos(1, Divisa::Dop).unidades(), 0.01);
    }

    #[test]
    fn una_fraccion_de_centavo_se_redondea_al_construir() {
        // Es el caso que produjo saldos con decimales sobrantes en producción.
        assert_eq!(dop(20.84688).centavos(), 2085);
        assert_eq!(dop(105.86392).centavos(), 10586);
        assert_eq!(dop(0.005).centavos(), 1, "la mitad se aleja del cero");
        assert_eq!(dop(-0.005).centavos(), -1);
    }

    #[test]
    fn sumar_muchos_centavos_no_acumula_error() {
        // 0.1 + 0.2 != 0.3 en coma flotante; en centavos es exacto.
        let mut total = Dinero::cero(Divisa::Dop);
        for _ in 0..1000 {
            total = total.sumar(&dop(0.10)).unwrap();
        }
        assert_eq!(total.centavos(), 10_000);
        assert_eq!(total.unidades(), 100.0);
    }

    #[test]
    fn un_monto_no_finito_o_fuera_de_rango_es_rechazado() {
        assert!(Dinero::nuevo(f64::NAN, Divisa::Dop).is_err());
        assert!(Dinero::nuevo(f64::INFINITY, Divisa::Dop).is_err());
        assert!(Dinero::nuevo(1e18, Divisa::Dop).is_err(), "no debe saturar en silencio");
    }

    #[test]
    fn el_cero_conserva_su_divisa() {
        let c = Dinero::cero(Divisa::Usd);
        assert!(c.es_cero());
        assert_eq!(c.divisa(), Divisa::Usd);
    }

    // --- Regla central: no se mezclan divisas ---

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
        let r = dop(100.0).restar(&dop(150.0)).unwrap();
        assert_eq!(r.centavos(), -5_000);
        assert!(r.es_negativo());
    }

    // --- Porcentaje: el único punto donde se redondea ---

    #[test]
    fn el_porcentaje_redondea_a_centavos_y_no_a_unidades() {
        // 1250.00 × 0.20 % = 2.50 exactos. Antes se redondeaba a 3.00.
        assert_eq!(dop(1250.0).porcentaje(0.002).unwrap(), dop(2.50));
        // 1200.00 × 0.20 % = 2.40
        assert_eq!(dop(1200.0).porcentaje(0.002).unwrap(), dop(2.40));
        // 100.00 × 0.20 % = 0.20
        assert_eq!(dop(100.0).porcentaje(0.002).unwrap(), dop(0.20));
    }

    #[test]
    fn el_porcentaje_de_un_importe_redondo_es_exacto() {
        assert_eq!(dop(10000.0).porcentaje(0.002).unwrap(), dop(20.0));
    }

    #[test]
    fn el_porcentaje_redondea_la_mitad_alejandose_del_cero() {
        // 12.50 × 0.20 % = 0.025 unidades = 2.5 centavos → 3 centavos
        assert_eq!(dop(12.50).porcentaje(0.002).unwrap().centavos(), 3);
    }

    #[test]
    fn el_porcentaje_conserva_la_divisa() {
        assert_eq!(usd(1000.0).porcentaje(0.002).unwrap().divisa(), Divisa::Usd);
    }

    #[test]
    fn una_tasa_no_finita_es_rechazada() {
        assert!(dop(100.0).porcentaje(f64::NAN).is_err());
    }

    // --- Tasa de cambio ---

    #[test]
    fn una_tasa_de_cero_pide_la_tasa_en_lugar_de_asumir_una() {
        assert_eq!(TasaCambio::nueva(0.0).unwrap_err(), ErrorDominio::TasaDeCambioRequerida);
    }

    #[test]
    fn una_tasa_negativa_o_no_finita_es_invalida() {
        assert_eq!(TasaCambio::nueva(-60.0).unwrap_err(), ErrorDominio::TasaDeCambioRequerida);
        assert!(matches!(
            TasaCambio::nueva(f64::NAN).unwrap_err(),
            ErrorDominio::TasaDeCambioInvalida { .. }
        ));
    }

    // --- Conversión ---

    #[test]
    fn abonar_500_usd_con_tasa_60_debita_30000_pesos() {
        let tasa = TasaCambio::nueva(60.0).unwrap();
        assert_eq!(usd(500.0).convertir(Divisa::Dop, tasa).unwrap(), dop(30000.0));
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
    fn la_conversion_redondea_a_centavos() {
        // 100.00 USD a 58.755 = 5875.50 DOP exactos
        let tasa = TasaCambio::nueva(58.755).unwrap();
        let convertido = usd(100.0).convertir(Divisa::Dop, tasa).unwrap();
        assert_eq!(convertido.centavos(), 587_550);
    }

    #[test]
    fn ida_y_vuelta_recupera_el_importe_original() {
        let tasa = TasaCambio::nueva(58.75).unwrap();
        let vuelta = usd(500.0)
            .convertir(Divisa::Dop, tasa)
            .unwrap()
            .convertir(Divisa::Usd, tasa)
            .unwrap();
        assert_eq!(vuelta, usd(500.0));
    }
}
