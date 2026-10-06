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

use super::dinero::{Dinero, Divisa};
use super::errores::ErrorDominio;

/// Cómo liquida un emisor los consumos hechos en divisa extranjera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoliticaLiquidacion {
    /// El consumo permanece en su divisa de origen. No hay nada que traducir.
    EnDivisaDeOrigen,
    /// El emisor lo traduce a moneda local en un momento posterior, a una tasa
    /// de referencia propia que no se conoce al comprar.
    TraduceAMonedaLocal,
}

impl PoliticaLiquidacion {
    /// Interpreta el texto almacenado. La ausencia significa la política más
    /// conservadora, que es la que no introduce estados pendientes.
    pub fn desde_codigo(codigo: Option<&str>) -> PoliticaLiquidacion {
        match codigo {
            Some("traduce") => PoliticaLiquidacion::TraduceAMonedaLocal,
            _ => PoliticaLiquidacion::EnDivisaDeOrigen,
        }
    }

    pub fn codigo(&self) -> &'static str {
        match self {
            PoliticaLiquidacion::EnDivisaDeOrigen => "origen",
            PoliticaLiquidacion::TraduceAMonedaLocal => "traduce",
        }
    }

    /// Un consumo queda pendiente solo si el emisor traduce y la compra se
    /// hizo en una divisa distinta de la moneda local.
    pub fn deja_pendiente(&self, divisa_consumo: Divisa, moneda_local: Divisa) -> bool {
        matches!(self, PoliticaLiquidacion::TraduceAMonedaLocal) && divisa_consumo != moneda_local
    }
}

/// Moneda local del sistema. Los emisores traducen a pesos dominicanos.
pub const MONEDA_LOCAL: Divisa = Divisa::Dop;

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

// --- Lo que el listado de tarjetas calcula (A-03: sale de `obtener_tarjetas`) --------------------------

/// Límite efectivo y disponible de una divisa, en unidades y listos para mostrar: `(efectivo, disponible)`.
///
/// Ante datos incoherentes se degrada a la cuenta en bruto (`aprobado + sobregiro - balance`) en lugar de tumbar la
/// consulta entera: una tarjeta rara no puede dejar sin lista a las demás.
pub fn cupo_para_mostrar(
    divisa: Divisa,
    aprobado: f64,
    ajustado: Option<f64>,
    sobregiro: f64,
    balance: f64,
) -> (f64, f64) {
    let construir = || -> Result<(f64, f64), ErrorDominio> {
        let limites = LimitesDivisa::nuevos(
            Dinero::nuevo(aprobado, divisa)?,
            ajustado.map(|a| Dinero::nuevo(a, divisa)).transpose()?,
            Dinero::nuevo(sobregiro, divisa)?,
        )?;
        let saldo = Dinero::nuevo(balance, divisa)?;
        Ok((limites.efectivo().unidades(), limites.disponible(saldo)?.unidades()))
    };
    construir().unwrap_or((aprobado, aprobado + sobregiro - balance))
}

/// Días que faltan para una fecha del mes. El mes se aproxima con 30 días (`(30 - hoy) + fecha`), tal como se ha
/// hecho siempre: es un recordatorio aproximado, no un cálculo de calendario.
pub fn dias_hasta_la_fecha(fecha: i32, dia_actual: i32) -> i32 {
    if fecha >= dia_actual { fecha - dia_actual } else { (30 - dia_actual) + fecha }
}

/// Hay alerta cuando faltan 3 días o menos.
pub const DIAS_DE_ALERTA: i32 = 3;

/// El recordatorio del corte: `(alerta, mensaje)`.
pub fn aviso_de_corte(fecha_corte: i32, dia_actual: i32) -> (bool, String) {
    let dias = dias_hasta_la_fecha(fecha_corte, dia_actual);
    let mensaje = if dias == 0 { "Hoy es la fecha de corte".to_string() } else { format!("Faltan {} días para corte", dias) };
    (dias <= DIAS_DE_ALERTA, mensaje)
}

/// El recordatorio del pago: `(alerta, mensaje)`.
pub fn aviso_de_pago(fecha_limite_pago: i32, dia_actual: i32) -> (bool, String) {
    let dias = dias_hasta_la_fecha(fecha_limite_pago, dia_actual);
    let mensaje = if dias == 0 { "Hoy vence el pago".to_string() } else { format!("Faltan {} días para pagar", dias) };
    (dias <= DIAS_DE_ALERTA, mensaje)
}

#[cfg(test)]
mod tests_del_listado {
    use super::*;

    #[test]
    fn los_dias_se_cuentan_con_un_mes_de_30() {
        assert_eq!(dias_hasta_la_fecha(10, 10), 0);
        assert_eq!(dias_hasta_la_fecha(15, 10), 5);
        assert_eq!(dias_hasta_la_fecha(2, 28), 4, "(30 - 28) + 2");
        assert_eq!(dias_hasta_la_fecha(1, 31), 0, "(30 - 31) + 1: el mes de 30 días no cuadra con el 31, y así ha sido siempre");
    }

    #[test]
    fn la_alerta_salta_a_tres_dias_o_menos_y_el_mensaje_distingue_hoy() {
        assert_eq!(aviso_de_corte(10, 10), (true, "Hoy es la fecha de corte".into()));
        assert_eq!(aviso_de_corte(13, 10), (true, "Faltan 3 días para corte".into()));
        assert_eq!(aviso_de_corte(14, 10), (false, "Faltan 4 días para corte".into()));
        assert_eq!(aviso_de_pago(10, 10), (true, "Hoy vence el pago".into()));
        assert_eq!(aviso_de_pago(13, 10), (true, "Faltan 3 días para pagar".into()));
        assert_eq!(aviso_de_pago(14, 10), (false, "Faltan 4 días para pagar".into()));
    }

    #[test]
    fn el_cupo_resuelve_el_efectivo_y_el_disponible_y_ante_datos_raros_cae_a_la_cuenta_en_bruto() {
        assert_eq!(cupo_para_mostrar(Divisa::Dop, 1000.0, Some(800.0), 300.0, 500.0), (800.0, 600.0));
        assert_eq!(cupo_para_mostrar(Divisa::Usd, 2000.0, None, 400.0, 600.0), (2000.0, 1800.0));
        // Un importe imposible (NaN) no se puede construir: se degrada a la cuenta en bruto.
        let (efectivo, _) = cupo_para_mostrar(Divisa::Dop, f64::NAN, None, 0.0, 0.0);
        assert!(efectivo.is_nan());
        // Un balance que no cabe en centavos enteros tampoco: la cuenta en bruto se hace con los números tal cual.
        let (efectivo, disponible) = cupo_para_mostrar(Divisa::Dop, 1000.0, None, 300.0, 1e300);
        assert_eq!(efectivo, 1000.0);
        assert_eq!(disponible, 1000.0 + 300.0 - 1e300, "aprobado + sobregiro - balance, sin dominio");
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

    // --- Política de liquidación ---

    #[test]
    fn la_politica_ausente_es_la_que_no_deja_pendientes() {
        assert_eq!(PoliticaLiquidacion::desde_codigo(None), PoliticaLiquidacion::EnDivisaDeOrigen);
        assert_eq!(
            PoliticaLiquidacion::desde_codigo(Some("desconocida")),
            PoliticaLiquidacion::EnDivisaDeOrigen
        );
    }

    #[test]
    fn el_codigo_de_politica_va_y_vuelve() {
        for p in [PoliticaLiquidacion::EnDivisaDeOrigen, PoliticaLiquidacion::TraduceAMonedaLocal] {
            assert_eq!(PoliticaLiquidacion::desde_codigo(Some(p.codigo())), p);
        }
    }

    #[test]
    fn solo_deja_pendiente_quien_traduce_y_solo_en_divisa_extranjera() {
        let traduce = PoliticaLiquidacion::TraduceAMonedaLocal;
        let origen = PoliticaLiquidacion::EnDivisaDeOrigen;

        assert!(traduce.deja_pendiente(Divisa::Usd, MONEDA_LOCAL), "compra en USD con tarjeta que traduce");
        assert!(!traduce.deja_pendiente(Divisa::Dop, MONEDA_LOCAL), "compra en la propia moneda local");
        assert!(!origen.deja_pendiente(Divisa::Usd, MONEDA_LOCAL), "la otra política nunca deja pendientes");
        assert!(!origen.deja_pendiente(Divisa::Dop, MONEDA_LOCAL));
    }
}
