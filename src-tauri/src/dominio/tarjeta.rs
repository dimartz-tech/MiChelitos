//! Límites de una tarjeta de crédito y el cupo que dejan disponible.
//!
//! Una tarjeta puede tener dos límites en cada divisa:
//!
//! * **Aprobado** — el que concede la entidad emisora.
//! * **Ajustado** — un tope que el propio titular se impone por debajo del
//!   aprobado. Algunos emisores permiten fijarlo desde su aplicación como
//!   medida de control de gasto.
//!
//! El disponible se calcula siempre contra el límite **efectivo**, porque es el
//! que el banco hace valer en el punto de venta. Hasta ahora ese cálculo vivía
//! en el HTML de la vista de tarjetas.

use super::dinero::Dinero;
use super::errores::ErrorDominio;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LimitesDivisa {
    aprobado: Dinero,
    ajustado: Option<Dinero>,
    sobregiro: Dinero,
}

impl LimitesDivisa {
    pub fn nuevos(
        aprobado: Dinero,
        ajustado: Option<Dinero>,
        sobregiro: Dinero,
    ) -> Result<LimitesDivisa, ErrorDominio> {
        // Sumar valida que las tres cifras compartan divisa.
        aprobado.sumar(&sobregiro)?;
        if let Some(a) = ajustado {
            aprobado.sumar(&a)?;
        }
        Ok(LimitesDivisa { aprobado, ajustado, sobregiro })
    }

    pub fn aprobado(&self) -> Dinero {
        self.aprobado
    }

    pub fn ajustado(&self) -> Option<Dinero> {
        self.ajustado
    }

    pub fn sobregiro(&self) -> Dinero {
        self.sobregiro
    }

    pub fn esta_ajustado(&self) -> bool {
        self.ajustado.is_some()
    }

    /// Límite que rige de verdad.
    ///
    /// Un ajuste **por encima del aprobado no se honra**: el banco declina el
    /// consumo igual. Tomar el menor de los dos evita que la aplicación
    /// muestre un cupo que no existe si el titular teclea una cifra de más.
    pub fn efectivo(&self) -> Dinero {
        match self.ajustado {
            Some(a) if a.centavos() < self.aprobado.centavos() => a,
            _ => self.aprobado,
        }
    }

    /// Cupo total de gasto: el límite efectivo más el sobregiro concedido.
    pub fn cupo_total(&self) -> Result<Dinero, ErrorDominio> {
        self.efectivo().sumar(&self.sobregiro)
    }

    /// Lo que queda por consumir. Puede ser negativo si la deuda excede el cupo.
    pub fn disponible(&self, balance: Dinero) -> Result<Dinero, ErrorDominio> {
        self.cupo_total()?.restar(&balance)
    }

    /// Porcentaje del cupo ya consumido, de 0.0 a 1.0 o más si hay exceso.
    /// Devuelve 0.0 cuando no hay cupo, en lugar de dividir por cero.
    pub fn uso(&self, balance: Dinero) -> Result<f64, ErrorDominio> {
        let cupo = self.cupo_total()?;
        if cupo.es_cero() {
            return Ok(0.0);
        }
        Ok(balance.centavos() as f64 / cupo.centavos() as f64)
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
    fn limites(aprobado: f64, ajustado: Option<f64>, sobregiro: f64) -> LimitesDivisa {
        LimitesDivisa::nuevos(dop(aprobado), ajustado.map(dop), dop(sobregiro)).unwrap()
    }

    // --- Límite efectivo ---

    #[test]
    fn sin_ajuste_rige_el_limite_aprobado() {
        let l = limites(70800.0, None, 0.0);
        assert_eq!(l.efectivo(), dop(70800.0));
        assert!(!l.esta_ajustado());
    }

    #[test]
    fn con_ajuste_por_debajo_rige_el_ajustado() {
        let l = limites(70800.0, Some(20000.0), 0.0);
        assert_eq!(l.efectivo(), dop(20000.0));
        assert!(l.esta_ajustado());
    }

    #[test]
    fn un_ajuste_por_encima_del_aprobado_no_concede_cupo_de_mas() {
        // El banco declina igual, así que mostrar 700 800 sería mentir.
        let l = limites(70800.0, Some(700800.0), 0.0);
        assert_eq!(l.efectivo(), dop(70800.0));
    }

    #[test]
    fn un_ajuste_igual_al_aprobado_es_indistinguible_en_el_cupo() {
        let l = limites(70800.0, Some(70800.0), 0.0);
        assert_eq!(l.efectivo(), dop(70800.0));
        assert!(l.esta_ajustado(), "pero sigue constando como ajustado");
    }

    #[test]
    fn un_ajuste_en_cero_congela_la_tarjeta() {
        // Distinto de "sin ajuste": cero es un tope deliberado.
        let l = limites(70800.0, Some(0.0), 0.0);
        assert_eq!(l.efectivo(), dop(0.0));
        assert_eq!(l.disponible(dop(0.0)).unwrap(), dop(0.0));
    }

    // --- Cupo y disponible ---

    #[test]
    fn el_sobregiro_se_suma_al_limite_efectivo() {
        let l = limites(70800.0, Some(20000.0), 5000.0);
        assert_eq!(l.cupo_total().unwrap(), dop(25000.0));
    }

    #[test]
    fn el_disponible_descuenta_la_deuda_del_cupo() {
        let l = limites(70800.0, Some(20000.0), 0.0);
        assert_eq!(l.disponible(dop(15509.52)).unwrap(), dop(4490.48));
    }

    #[test]
    fn el_disponible_puede_ser_negativo_si_la_deuda_excede_el_cupo() {
        let l = limites(70800.0, Some(20000.0), 0.0);
        let d = l.disponible(dop(21000.0)).unwrap();
        assert!(d.es_negativo());
        assert_eq!(d, dop(-1000.0));
    }

    #[test]
    fn ajustar_el_limite_reduce_el_disponible_sin_tocar_el_aprobado() {
        let deuda = dop(15509.52);
        let sin_ajuste = limites(70800.0, None, 0.0);
        let con_ajuste = limites(70800.0, Some(20000.0), 0.0);

        assert_eq!(sin_ajuste.disponible(deuda).unwrap(), dop(55290.48));
        assert_eq!(con_ajuste.disponible(deuda).unwrap(), dop(4490.48));
        assert_eq!(con_ajuste.aprobado(), dop(70800.0), "el aprobado no cambia");
    }

    // --- Uso ---

    #[test]
    fn el_uso_se_calcula_sobre_el_cupo_efectivo() {
        let l = limites(70800.0, Some(20000.0), 0.0);
        assert!((l.uso(dop(10000.0)).unwrap() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn un_cupo_de_cero_no_divide_por_cero() {
        let l = limites(0.0, None, 0.0);
        assert_eq!(l.uso(dop(0.0)).unwrap(), 0.0);
    }

    // --- Divisas ---

    #[test]
    fn los_limites_de_una_divisa_no_admiten_importes_de_otra() {
        assert!(LimitesDivisa::nuevos(dop(70800.0), None, usd(100.0)).is_err());
        assert!(LimitesDivisa::nuevos(dop(70800.0), Some(usd(500.0)), dop(0.0)).is_err());
    }
}
