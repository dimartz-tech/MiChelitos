//! Bonificaciones que un emisor acredita sobre una tarjeta: cashback,
//! devoluciones promocionales y recompensas convertidas en dinero.
//!
//! **Es un crédito aparte, nunca una reducción del consumo original.** Así se
//! conserva cuánto se gastó realmente y cuánto se bonificó, cada cosa con su
//! propia fecha — que es además cómo lo reporta el emisor.
//!
//! Un mismo consumo puede generar **varias** bonificaciones. Se observó en un
//! estado real: una compra recibió por separado el porcentaje base y el de
//! bonificación de categoría, en dos líneas distintas del mismo día. Por eso
//! el vínculo con el gasto es de uno a muchos y además opcional: los estados
//! no dicen a qué consumo corresponde cada crédito.

use super::dinero::Dinero;
use super::errores::ErrorDominio;

#[derive(Debug, Clone, PartialEq)]
pub struct Bonificacion {
    monto: Dinero,
    concepto: String,
}

impl Bonificacion {
    /// El importe se expresa **en positivo**, como lo informa el emisor. Su
    /// efecto sobre la deuda es negativo, y de eso se encarga
    /// [`Bonificacion::efecto_sobre_deuda`].
    pub fn nueva(monto: Dinero, concepto: &str) -> Result<Bonificacion, ErrorDominio> {
        if monto.es_negativo() || monto.es_cero() {
            return Err(ErrorDominio::MontoInvalido { valor: monto.unidades() });
        }
        let concepto = concepto.trim();
        if concepto.is_empty() {
            return Err(ErrorDominio::ConceptoVacio);
        }
        Ok(Bonificacion { monto, concepto: concepto.to_string() })
    }

    pub fn monto(&self) -> Dinero {
        self.monto
    }

    pub fn concepto(&self) -> &str {
        &self.concepto
    }

    /// Lo que hay que sumar a la deuda de la tarjeta: el importe en negativo.
    pub fn efecto_sobre_deuda(&self) -> Result<Dinero, ErrorDominio> {
        Dinero::cero(self.monto.divisa()).restar(&self.monto)
    }

    /// ¿Corresponde este crédito a aplicar `tasa` sobre `consumo`?
    ///
    /// Es la comparación que de verdad sirve para contrastar lo abonado con
    /// la tabla de beneficios del emisor. Compara **importes redondeados a
    /// centavos**, no porcentajes: el emisor redondea el crédito, de modo que
    /// 728.14 al 5 % da 36.41 y no 36.407, y una comparación de porcentajes
    /// nunca daría exacta.
    pub fn coincide_con_tasa(&self, consumo: Dinero, tasa: f64) -> bool {
        match consumo.porcentaje(tasa) {
            Ok(esperado) => esperado == self.monto,
            Err(_) => false,
        }
    }

    /// Porcentaje aproximado que representa sobre un consumo, para mostrarlo.
    ///
    /// Es orientativo: el redondeo a centavos hace que casi nunca coincida
    /// con la tasa nominal. Para comprobar si cuadra con una tasa concreta,
    /// usar [`Bonificacion::coincide_con_tasa`].
    pub fn porcentaje_sobre(&self, consumo: Dinero) -> Option<f64> {
        if consumo.divisa() != self.monto.divisa() || consumo.es_cero() {
            return None;
        }
        Some(self.monto.centavos() as f64 / consumo.centavos() as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::super::dinero::Divisa;
    use super::*;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }
    fn usd(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Usd).unwrap()
    }

    #[test]
    fn una_bonificacion_se_guarda_en_positivo_y_su_efecto_es_negativo() {
        let b = Bonificacion::nueva(dop(36.41), "Cashback compra por internet").unwrap();
        assert_eq!(b.monto(), dop(36.41));
        assert_eq!(b.efecto_sobre_deuda().unwrap(), dop(-36.41));
    }

    #[test]
    fn el_efecto_conserva_la_divisa() {
        let b = Bonificacion::nueva(usd(2.50), "Cashback").unwrap();
        assert_eq!(b.efecto_sobre_deuda().unwrap().divisa(), Divisa::Usd);
    }

    #[test]
    fn un_importe_negativo_o_cero_no_es_una_bonificacion() {
        assert!(Bonificacion::nueva(dop(-10.0), "Cashback").is_err());
        assert!(Bonificacion::nueva(dop(0.0), "Cashback").is_err());
    }

    #[test]
    fn el_concepto_no_puede_quedar_vacio() {
        // Es lo único que distingue un cashback de una promoción o de una
        // recompensa: sin él, el asiento no se puede interpretar después.
        assert_eq!(Bonificacion::nueva(dop(10.0), "   ").unwrap_err(), ErrorDominio::ConceptoVacio);
    }

    #[test]
    fn el_concepto_se_recorta() {
        let b = Bonificacion::nueva(dop(10.0), "  Cashback Personalizado  ").unwrap();
        assert_eq!(b.concepto(), "Cashback Personalizado");
    }

    // --- Contraste con la tabla de beneficios del emisor ---

    #[test]
    fn reconoce_una_bonificacion_que_cuadra_con_la_tasa_prometida() {
        // Caso real: 36.41 sobre 728.14. El 5 % exacto son 36.407, que el
        // emisor redondea a 36.41; comparar porcentajes daría 5.000412 %.
        let b = Bonificacion::nueva(dop(36.41), "Cashback compra por internet").unwrap();
        assert!(b.coincide_con_tasa(dop(728.14), 0.05));
        assert!(!b.coincide_con_tasa(dop(728.14), 0.03), "descarta una tasa que no es");
    }

    #[test]
    fn reconoce_el_porcentaje_base_y_el_de_bonificacion_por_separado() {
        // Caso real: un mismo consumo generó dos créditos en el mismo día,
        // 1 % base y 2 % de categoría, que juntos son el 3 % de comida.
        let consumo = dop(8640.00);
        let base = Bonificacion::nueva(dop(86.40), "Recompensas Qik Rebate").unwrap();
        let extra = Bonificacion::nueva(dop(172.80), "Cashback Personalizado").unwrap();

        assert!(base.coincide_con_tasa(consumo, 0.01));
        assert!(extra.coincide_con_tasa(consumo, 0.02));
        assert_eq!(base.monto().sumar(&extra.monto()).unwrap(), dop(259.20));
    }

    #[test]
    fn una_tasa_que_no_se_aplico_no_coincide() {
        // Caso real: una compra que debió recibir el 5 % y no recibió nada.
        // Al registrar cero no existe bonificación, así que se detecta por
        // ausencia y no por un crédito que no cuadre.
        let b = Bonificacion::nueva(dop(105.07), "Cashback").unwrap();
        assert!(b.coincide_con_tasa(dop(2101.42), 0.05));
        assert!(!b.coincide_con_tasa(dop(3174.67), 0.05));
    }

    #[test]
    fn el_porcentaje_mostrado_es_orientativo_por_el_redondeo() {
        let b = Bonificacion::nueva(dop(36.41), "Cashback").unwrap();
        let pct = b.porcentaje_sobre(dop(728.14)).unwrap();
        assert!((pct - 0.05).abs() < 1e-4, "cerca del 5 %, pero no exacto: {:.6}", pct);
    }

    #[test]
    fn no_calcula_porcentaje_sobre_un_consumo_de_otra_divisa() {
        let b = Bonificacion::nueva(dop(36.41), "Cashback").unwrap();
        assert!(b.porcentaje_sobre(usd(10.0)).is_none());
    }

    #[test]
    fn no_divide_por_un_consumo_de_cero() {
        let b = Bonificacion::nueva(dop(36.41), "Cashback").unwrap();
        assert!(b.porcentaje_sobre(dop(0.0)).is_none());
    }
}
