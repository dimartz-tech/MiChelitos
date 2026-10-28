//! El cálculo de un abono a tarjeta, como regla pura.
//!
//! Lo comparten **el registro** (`aplicacion::registrar_pago_tarjeta`) **y la vista previa**
//! (`aplicacion::previsualizar_abono`): lo que se muestra antes de confirmar sale de la misma función que decide lo
//! que se asienta después, de modo que las dos cifras no pueden divergir. Ni SQL ni puertos: entran importes y salen
//! importes.

use super::cargos::TASA_RETENCION;
use super::dinero::{Dinero, Divisa, TasaCambio};
use super::errores::ErrorDominio;

/// Qué importe sale de la cuenta que paga un abono.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalculoDeAbono {
    /// El abono en la divisa de la cuenta (convertido si cruza divisas), sin comisión.
    pub debitado: Dinero,
    /// La comisión del 0,20 % sobre lo debitado. `None` cuando el abono sale de la caja de efectivo.
    pub comision: Option<Dinero>,
    /// Lo que sale de la cuenta en total: `debitado + comision`.
    pub total_debitado: Dinero,
    /// ¿Cruzó divisas? Solo entonces la tasa interviene.
    pub convirtio: bool,
}

/// Calcula lo que un abono de `monto` saca de una cuenta en `divisa_cuenta`.
///
/// La conversión depende de que las divisas **difieran**, no de que llegue una tasa (H15): sin cruce la tasa se
/// ignora; con cruce y sin tasa, `TasaDeCambioRequerida`. La comisión solo existe si `con_comision` (el abono sale de
/// una cuenta elegida, no de la caja de efectivo por defecto).
pub fn calcular_abono(
    monto: Dinero,
    divisa_cuenta: Divisa,
    tasa: Option<TasaCambio>,
    con_comision: bool,
) -> Result<CalculoDeAbono, ErrorDominio> {
    let (debitado, convirtio) = if monto.divisa() == divisa_cuenta {
        (monto, false)
    } else {
        let tasa = tasa.ok_or(ErrorDominio::TasaDeCambioRequerida)?;
        (monto.convertir(divisa_cuenta, tasa)?, true)
    };
    if !con_comision {
        return Ok(CalculoDeAbono { debitado, comision: None, total_debitado: debitado, convirtio });
    }
    let comision = debitado.porcentaje(TASA_RETENCION)?;
    let total_debitado = debitado.sumar(&comision)?;
    Ok(CalculoDeAbono { debitado, comision: Some(comision), total_debitado, convirtio })
}

/// Cómo se eligió el importe del abono en el formulario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoDeAbono {
    /// Monto libre, escrito por el titular.
    Personalizado,
    /// El balance al corte de la tarjeta.
    Corte,
    /// El balance actual de la tarjeta.
    Actual,
}

impl TipoDeAbono {
    /// Los códigos del formulario: `personalizado`, `corte` y `actual`. Cualquier otro se toma como monto libre.
    pub fn desde_codigo(codigo: &str) -> TipoDeAbono {
        match codigo {
            "corte" => TipoDeAbono::Corte,
            "actual" => TipoDeAbono::Actual,
            _ => TipoDeAbono::Personalizado,
        }
    }

    pub fn codigo(&self) -> &'static str {
        match self {
            TipoDeAbono::Personalizado => "personalizado",
            TipoDeAbono::Corte => "corte",
            TipoDeAbono::Actual => "actual",
        }
    }

    pub fn etiqueta(&self) -> &'static str {
        match self {
            TipoDeAbono::Personalizado => "Monto libre",
            TipoDeAbono::Corte => "Balance al corte",
            TipoDeAbono::Actual => "Saldo actual",
        }
    }
}

/// Cuánto pesa una advertencia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gravedad {
    /// El abono no puede registrarse así (el registro lo rechazaría o no hay con qué calcularlo).
    Bloqueo,
    /// Se puede registrar, pero conviene mirarlo antes.
    Aviso,
    /// Un dato que el titular debe tener presente, sin reproche.
    Info,
}

impl Gravedad {
    pub fn codigo(&self) -> &'static str {
        match self {
            Gravedad::Bloqueo => "bloqueo",
            Gravedad::Aviso => "aviso",
            Gravedad::Info => "info",
        }
    }
}

/// Lo que la vista previa advierte. Cada variante lleva los datos que su texto necesita.
#[derive(Debug, Clone, PartialEq)]
pub enum Advertencia {
    /// El monto está vacío, no es un importe, es cero o es negativo.
    MontoNoValido,
    TarjetaInactiva { nombre: String },
    CuentaNoEncontrada,
    CuentaArchivada { nombre: String },
    CajaNoEncontrada { nombre: String },
    /// El abono cruza divisas y falta la tasa.
    FaltaTasa,
    TasaNoValida,
    /// Cualquier otra regla del dominio que impediría calcular el abono.
    NoSePuedeCalcular { motivo: String },
    /// No se eligió cuenta: el abono sale de la caja de efectivo de su divisa, sin comisión.
    EfectivoPorDefecto { caja: String },
    /// La cuenta quedaría con saldo negativo (el registro lo permite; el titular debe saberlo).
    CuentaQuedariaEnNegativo { saldo_resultante: Dinero },
    /// El abono supera el balance de la tarjeta: la diferencia queda como saldo a favor.
    SuperaElBalance { balance: Dinero, a_favor: Dinero },
    /// El tipo elegido (actual o corte) no coincide con el monto escrito.
    MontoNoCoincideConElTipo { tipo: TipoDeAbono, referencia: Dinero },
    /// El abono se paga con una cuenta en otra divisa que la del abono.
    CruzaDivisas { divisa_abono: Divisa, divisa_cuenta: Divisa },
}

impl Advertencia {
    pub fn codigo(&self) -> &'static str {
        match self {
            Advertencia::MontoNoValido => "monto_no_valido",
            Advertencia::TarjetaInactiva { .. } => "tarjeta_inactiva",
            Advertencia::CuentaNoEncontrada => "cuenta_no_encontrada",
            Advertencia::CuentaArchivada { .. } => "cuenta_archivada",
            Advertencia::CajaNoEncontrada { .. } => "caja_no_encontrada",
            Advertencia::FaltaTasa => "falta_tasa",
            Advertencia::TasaNoValida => "tasa_no_valida",
            Advertencia::NoSePuedeCalcular { .. } => "no_se_puede_calcular",
            Advertencia::EfectivoPorDefecto { .. } => "efectivo_por_defecto",
            Advertencia::CuentaQuedariaEnNegativo { .. } => "cuenta_en_negativo",
            Advertencia::SuperaElBalance { .. } => "supera_el_balance",
            Advertencia::MontoNoCoincideConElTipo { .. } => "monto_no_coincide_con_el_tipo",
            Advertencia::CruzaDivisas { .. } => "cruza_divisas",
        }
    }

    pub fn gravedad(&self) -> Gravedad {
        match self {
            Advertencia::MontoNoValido
            | Advertencia::TarjetaInactiva { .. }
            | Advertencia::CuentaNoEncontrada
            | Advertencia::CuentaArchivada { .. }
            | Advertencia::CajaNoEncontrada { .. }
            | Advertencia::FaltaTasa
            | Advertencia::TasaNoValida
            | Advertencia::NoSePuedeCalcular { .. } => Gravedad::Bloqueo,
            Advertencia::EfectivoPorDefecto { .. }
            | Advertencia::CuentaQuedariaEnNegativo { .. }
            | Advertencia::SuperaElBalance { .. }
            | Advertencia::MontoNoCoincideConElTipo { .. } => Gravedad::Aviso,
            Advertencia::CruzaDivisas { .. } => Gravedad::Info,
        }
    }

    /// El texto que lee el titular.
    pub fn texto(&self) -> String {
        match self {
            Advertencia::MontoNoValido => "El monto del abono debe ser un importe mayor que cero.".to_string(),
            Advertencia::TarjetaInactiva { nombre } => {
                format!("La tarjeta «{nombre}» está desactivada: reactívala antes de abonarle.")
            }
            Advertencia::CuentaNoEncontrada => "La cuenta elegida ya no existe.".to_string(),
            Advertencia::CuentaArchivada { nombre } => {
                format!("La cuenta «{nombre}» está archivada: reactívala o elige otra para pagar.")
            }
            Advertencia::CajaNoEncontrada { nombre } => {
                format!("No hay cuenta elegida y no existe la caja «{nombre}» de donde pagar en efectivo. Elige una cuenta.")
            }
            Advertencia::FaltaTasa => {
                "El abono cruza dos divisas: indica la tasa de cambio (pesos por 1 dólar).".to_string()
            }
            Advertencia::TasaNoValida => "La tasa de cambio no es válida: debe ser un número mayor que cero.".to_string(),
            Advertencia::NoSePuedeCalcular { motivo } => format!("No se puede calcular el abono: {motivo}"),
            Advertencia::EfectivoPorDefecto { caja } => format!(
                "No elegiste cuenta de origen: el abono saldrá de «{caja}» (por defecto) y no lleva comisión. Si lo pagaste con una cuenta, elígela."
            ),
            Advertencia::CuentaQuedariaEnNegativo { saldo_resultante } => format!(
                "La cuenta quedaría en negativo: {} {}.",
                saldo_resultante.divisa().codigo(),
                importe_legible(*saldo_resultante)
            ),
            Advertencia::SuperaElBalance { balance, a_favor } => format!(
                "El abono supera el balance de la tarjeta ({} {}): quedaría un saldo a favor de {} {}.",
                balance.divisa().codigo(),
                importe_legible(*balance),
                a_favor.divisa().codigo(),
                importe_legible(*a_favor)
            ),
            Advertencia::MontoNoCoincideConElTipo { tipo, referencia } => format!(
                "El tipo «{}» corresponde a {} {}, pero el monto escrito es distinto.",
                tipo.etiqueta(),
                referencia.divisa().codigo(),
                importe_legible(*referencia)
            ),
            Advertencia::CruzaDivisas { divisa_abono, divisa_cuenta } => format!(
                "El abono es en {} y la cuenta está en {}: se convierte con la tasa indicada.",
                divisa_abono.codigo(),
                divisa_cuenta.codigo()
            ),
        }
    }
}

/// `1,234.56` (con signo si es negativo), sin la divisa. Exacto: trabaja sobre los centavos.
pub fn importe_legible(d: Dinero) -> String {
    let centavos = d.centavos();
    let signo = if centavos < 0 { "-" } else { "" };
    let abs = centavos.unsigned_abs();
    let enteros = (abs / 100).to_string();
    let mut con_miles = String::new();
    for (i, c) in enteros.chars().enumerate() {
        if i > 0 && (enteros.len() - i) % 3 == 0 {
            con_miles.push(',');
        }
        con_miles.push(c);
    }
    format!("{signo}{con_miles}.{:02}", abs % 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dop(c: i64) -> Dinero {
        Dinero::de_centavos(c, Divisa::Dop)
    }
    fn usd(c: i64) -> Dinero {
        Dinero::de_centavos(c, Divisa::Usd)
    }
    fn tasa(v: f64) -> Option<TasaCambio> {
        Some(TasaCambio::nueva(v).unwrap())
    }

    #[test]
    fn con_cuenta_la_comision_es_el_veinte_por_diez_mil() {
        let c = calcular_abono(dop(1_150_000), Divisa::Dop, None, true).unwrap();
        assert_eq!(c.comision, Some(dop(2_300)), "0,20 % de 11 500.00 es 23.00");
        assert_eq!(c.total_debitado, dop(1_152_300));
        assert!(!c.convirtio);
    }

    #[test]
    fn en_efectivo_no_hay_comision() {
        let c = calcular_abono(dop(1_150_000), Divisa::Dop, None, false).unwrap();
        assert_eq!((c.comision, c.total_debitado), (None, dop(1_150_000)));
    }

    #[test]
    fn cruzar_divisas_convierte_y_comisiona_sobre_lo_convertido() {
        let c = calcular_abono(usd(10_000), Divisa::Dop, tasa(60.0), true).unwrap();
        assert_eq!(c.debitado, dop(600_000));
        assert_eq!(c.comision, Some(dop(1_200)));
        assert!(c.convirtio);
    }

    #[test]
    fn cruzar_divisas_sin_tasa_se_rechaza_y_sin_cruce_la_tasa_se_ignora() {
        assert_eq!(calcular_abono(usd(100), Divisa::Dop, None, true), Err(ErrorDominio::TasaDeCambioRequerida));
        let c = calcular_abono(dop(100), Divisa::Dop, tasa(60.0), true).unwrap();
        assert_eq!(c.debitado, dop(100), "H15: sin cruce de divisas la tasa no interviene");
    }

    #[test]
    fn el_importe_legible_agrupa_miles_y_conserva_el_signo() {
        assert_eq!(importe_legible(dop(123_456_789)), "1,234,567.89");
        assert_eq!(importe_legible(dop(-5)), "-0.05");
        assert_eq!(importe_legible(dop(123_400)), "1,234.00");
        assert_eq!(importe_legible(dop(99_999)), "999.99");
    }

    #[test]
    fn solo_los_bloqueos_impiden_confirmar_y_los_codigos_son_distintos() {
        let todas = [
            Advertencia::MontoNoValido,
            Advertencia::TarjetaInactiva { nombre: "x".into() },
            Advertencia::CuentaNoEncontrada,
            Advertencia::CuentaArchivada { nombre: "x".into() },
            Advertencia::CajaNoEncontrada { nombre: "x".into() },
            Advertencia::FaltaTasa,
            Advertencia::TasaNoValida,
            Advertencia::NoSePuedeCalcular { motivo: "x".into() },
            Advertencia::EfectivoPorDefecto { caja: "x".into() },
            Advertencia::CuentaQuedariaEnNegativo { saldo_resultante: dop(-1) },
            Advertencia::SuperaElBalance { balance: dop(1), a_favor: dop(1) },
            Advertencia::MontoNoCoincideConElTipo { tipo: TipoDeAbono::Actual, referencia: dop(1) },
            Advertencia::CruzaDivisas { divisa_abono: Divisa::Usd, divisa_cuenta: Divisa::Dop },
        ];
        let mut codigos: Vec<_> = todas.iter().map(|a| a.codigo()).collect();
        codigos.sort();
        codigos.dedup();
        assert_eq!(codigos.len(), todas.len());
        assert!(todas.iter().all(|a| !a.texto().is_empty()));
        assert_eq!(todas.iter().filter(|a| a.gravedad() == Gravedad::Bloqueo).count(), 8);
    }
}
