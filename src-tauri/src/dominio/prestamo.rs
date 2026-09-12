//! Financiamientos: préstamos amortizables y líneas de crédito.
//!
//! El sistema guardaba de cada financiamiento el **monto original** y, para
//! calcular el pasivo, lo multiplicaba por la fracción de cuotas pendientes.
//! Esa aproximación tiene dos problemas y este módulo existe por ambos.
//!
//! El primero es que **supone amortización lineal**. En un préstamo real las
//! primeras cuotas son casi todo interés, de modo que el capital que se debe
//! es siempre mayor que esa fracción, y más cuanto más al principio se esté.
//!
//! El segundo es que una **línea de crédito no tiene cuotas contadas**. Al no
//! tenerlas, el cálculo se saltaba y el pasivo se quedaba clavado en el monto
//! desembolsado el primer día, sin que ningún pago lo moviera nunca.
//!
//! La respuesta a los dos es la misma: el pasivo deja de deducirse y pasa a
//! ser un **saldo que se lleva**. Cada cuota lo reduce por su parte de capital
//! —no por su importe entero— y un estado de cuenta puede declararlo, que es
//! la única fuente que cuadra al centavo con la del acreedor.

use super::dinero::Dinero;
use super::errores::ErrorDominio;

/// Naturaleza del financiamiento.
///
/// La distinción que importa no es el destino del dinero sino **si el capital
/// se libera al pagarlo**: una línea revolvente devuelve cupo disponible, un
/// préstamo amortizable no.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoPrestamo {
    Consumo,
    Hipotecario,
    Vehiculo,
    /// Línea de crédito revolvente. Al abonarla, el importe amortizado vuelve
    /// a quedar disponible.
    Flexible,
}

impl TipoPrestamo {
    pub fn desde_codigo(codigo: &str) -> Option<TipoPrestamo> {
        match codigo {
            "consumo" => Some(TipoPrestamo::Consumo),
            "hipotecario" => Some(TipoPrestamo::Hipotecario),
            "vehiculo" => Some(TipoPrestamo::Vehiculo),
            "flexible" => Some(TipoPrestamo::Flexible),
            _ => None,
        }
    }

    pub fn codigo(&self) -> &'static str {
        match self {
            TipoPrestamo::Consumo => "consumo",
            TipoPrestamo::Hipotecario => "hipotecario",
            TipoPrestamo::Vehiculo => "vehiculo",
            TipoPrestamo::Flexible => "flexible",
        }
    }

    /// Si el capital amortizado vuelve a estar disponible para disponer.
    pub fn es_revolvente(&self) -> bool {
        matches!(self, TipoPrestamo::Flexible)
    }
}

/// En qué se reparte una cuota.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesgloseCuota {
    /// Interés del período, que no reduce la deuda.
    pub interes: Dinero,
    /// Capital amortizado. Es lo único que mueve el saldo.
    pub capital: Dinero,
}

/// Reparte una cuota entre el interés del período y el capital que amortiza.
///
/// `tasa_anual` se expresa como porcentaje nominal —`18.5` para un 18,5 %—
/// porque es como lo publica el acreedor y como lo guarda la tabla. El interés
/// del mes es la doceava parte, redondeada a centavos en `Dinero::porcentaje`,
/// que es el único punto del dominio donde se pierde precisión.
///
/// **El capital puede salir negativo**, y no se recorta. Ocurre cuando la
/// cuota no alcanza a cubrir el interés: la deuda crece en lugar de bajar.
/// Es una situación real y grave, y ocultarla tras un cero la volvería
/// invisible justo cuando más importa verla.
///
/// **También puede exceder el saldo**, en la última cuota. Tampoco se recorta,
/// por la misma razón por la que el balance de una tarjeta admite saldo a
/// favor: pagar de más es un hecho que ocurre, y descartarlo pierde dinero sin
/// dejar registro.
pub fn desglosar_cuota(
    saldo: Dinero,
    tasa_anual: f64,
    cuota: Dinero,
) -> Result<DesgloseCuota, ErrorDominio> {
    if !tasa_anual.is_finite() || tasa_anual < 0.0 {
        return Err(ErrorDominio::MontoInvalido { valor: tasa_anual });
    }

    // Un saldo a favor no devenga interés a cargo del deudor.
    let interes = if saldo.es_negativo() || saldo.es_cero() {
        Dinero::cero(saldo.divisa())
    } else {
        saldo.porcentaje(tasa_anual / 100.0 / 12.0)?
    };

    let capital = cuota.restar(&interes)?;
    Ok(DesgloseCuota { interes, capital })
}

/// Cupo que queda por disponer en una línea revolvente.
///
/// Es lo que hace visible el efecto de pagar: a medida que el saldo baja, el
/// disponible sube. Puede ser negativo si el saldo superó el límite, cosa que
/// ocurre cuando se capitalizan intereses o comisiones.
pub fn disponible(limite: Dinero, saldo: Dinero) -> Result<Dinero, ErrorDominio> {
    limite.restar(&saldo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dominio::dinero::Divisa;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }

    // --- Tipo ---

    #[test]
    fn los_codigos_coinciden_con_el_check_de_la_tabla() {
        for codigo in ["consumo", "hipotecario", "vehiculo", "flexible"] {
            let tipo = TipoPrestamo::desde_codigo(codigo).expect(codigo);
            assert_eq!(tipo.codigo(), codigo);
        }
        assert_eq!(TipoPrestamo::desde_codigo("leasing"), None);
    }

    #[test]
    fn solo_la_linea_flexible_es_revolvente() {
        assert!(TipoPrestamo::Flexible.es_revolvente());
        assert!(!TipoPrestamo::Consumo.es_revolvente());
        assert!(!TipoPrestamo::Hipotecario.es_revolvente());
        assert!(!TipoPrestamo::Vehiculo.es_revolvente());
    }

    // --- Desglose de la cuota ---

    #[test]
    fn la_cuota_se_reparte_entre_interes_y_capital() {
        // 100 000 al 12 % anual: 1 % mensual = 1 000 de interés.
        let d = desglosar_cuota(dop(100_000.0), 12.0, dop(5_000.0)).unwrap();
        assert_eq!(d.interes, dop(1_000.0));
        assert_eq!(d.capital, dop(4_000.0));
    }

    #[test]
    fn interes_y_capital_siempre_suman_la_cuota() {
        // La propiedad que impide que el reparto pierda o invente centavos.
        for (saldo, tasa, cuota) in [
            (100_000.0, 12.0, 5_000.0),
            (1_234.56, 18.5, 300.0),
            (99_999.99, 7.25, 1_500.0),
            (0.01, 24.0, 0.05),
        ] {
            let d = desglosar_cuota(dop(saldo), tasa, dop(cuota)).unwrap();
            assert_eq!(
                d.interes.sumar(&d.capital).unwrap(),
                dop(cuota),
                "saldo {saldo}, tasa {tasa}"
            );
        }
    }

    #[test]
    fn el_interes_se_redondea_a_centavos() {
        // 1 234.56 al 18.5 % anual → 1.5416…% mensual = 19.0328 → 19.03.
        let d = desglosar_cuota(dop(1_234.56), 18.5, dop(300.0)).unwrap();
        assert_eq!(d.interes, dop(19.03));
        assert_eq!(d.capital, dop(280.97));
    }

    #[test]
    fn una_cuota_que_no_cubre_el_interes_amortiza_capital_negativo() {
        // La deuda crece. No se recorta en cero: ocultarlo haría invisible
        // justo el caso que hay que ver.
        let d = desglosar_cuota(dop(100_000.0), 12.0, dop(500.0)).unwrap();
        assert_eq!(d.interes, dop(1_000.0));
        assert_eq!(d.capital, dop(-500.0));
        assert!(d.capital.es_negativo());
    }

    #[test]
    fn una_cuota_mayor_que_la_deuda_amortiza_de_mas() {
        // Misma decisión que el saldo a favor de las tarjetas: pagar de más es
        // un hecho, y descartarlo perdería dinero sin registro.
        let d = desglosar_cuota(dop(100.0), 12.0, dop(500.0)).unwrap();
        assert_eq!(d.interes, dop(1.0));
        assert_eq!(d.capital, dop(499.0));
        assert!(dop(100.0).restar(&d.capital).unwrap().es_negativo());
    }

    #[test]
    fn un_saldo_liquidado_no_devenga_interes() {
        let d = desglosar_cuota(dop(0.0), 12.0, dop(500.0)).unwrap();
        assert_eq!(d.interes, dop(0.0));
        assert_eq!(d.capital, dop(500.0));
    }

    #[test]
    fn un_saldo_a_favor_no_devenga_interes_a_cargo_del_deudor() {
        let d = desglosar_cuota(dop(-250.0), 12.0, dop(500.0)).unwrap();
        assert_eq!(d.interes, dop(0.0), "no se cobra interés sobre lo que no se debe");
        assert_eq!(d.capital, dop(500.0));
    }

    #[test]
    fn una_tasa_cero_deja_toda_la_cuota_como_capital() {
        let d = desglosar_cuota(dop(100_000.0), 0.0, dop(5_000.0)).unwrap();
        assert_eq!(d.interes, dop(0.0));
        assert_eq!(d.capital, dop(5_000.0));
    }

    #[test]
    fn una_tasa_invalida_es_error_y_no_un_desglose_silencioso() {
        assert!(desglosar_cuota(dop(100.0), -1.0, dop(50.0)).is_err());
        assert!(desglosar_cuota(dop(100.0), f64::NAN, dop(50.0)).is_err());
        assert!(desglosar_cuota(dop(100.0), f64::INFINITY, dop(50.0)).is_err());
    }

    #[test]
    fn no_se_puede_desglosar_una_cuota_en_otra_divisa() {
        let saldo = dop(100_000.0);
        let cuota = Dinero::nuevo(500.0, Divisa::Usd).unwrap();
        assert!(desglosar_cuota(saldo, 12.0, cuota).is_err());
    }

    // --- Disponible ---

    #[test]
    fn el_disponible_sube_a_medida_que_el_saldo_baja() {
        let limite = dop(200_000.0);
        assert_eq!(disponible(limite, dop(150_000.0)).unwrap(), dop(50_000.0));
        assert_eq!(disponible(limite, dop(120_000.0)).unwrap(), dop(80_000.0));
        assert_eq!(disponible(limite, dop(0.0)).unwrap(), limite);
    }

    #[test]
    fn un_saldo_por_encima_del_limite_deja_el_disponible_en_negativo() {
        // Ocurre al capitalizarse intereses o comisiones. Se muestra tal cual.
        assert_eq!(disponible(dop(100_000.0), dop(105_000.0)).unwrap(), dop(-5_000.0));
    }
}
