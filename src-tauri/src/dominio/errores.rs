use super::dinero::Divisa;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorDominio {
    DivisasIncompatibles { esperada: Divisa, recibida: Divisa },
    TasaDeCambioRequerida,
    TasaDeCambioInvalida { valor: f64 },
    MontoInvalido { valor: f64 },
    FondosInsuficientes { disponible: f64, requerido: f64, divisa: Divisa },
    DivisaDesconocida { codigo: String },
    ConceptoVacio,
    /// El método de pago exige una referencia que no se indicó.
    ReferenciaFaltante { metodo: &'static str, referencia: &'static str },
    /// Origen y destino de una transferencia son la misma cuenta.
    TransferenciaALaMismaCuenta { cuenta_id: i64 },
    /// Sale un importe y entra otro distinto, sin cambio de divisa que lo
    /// explique.
    ImportesNoCuadran { sale: f64, entra: f64, divisa: Divisa },
    /// Se declaró un cobro parcial mayor que el neto de la factura.
    CobroParcialExcedeElNeto { parcial: f64, neto: f64 },
    /// El texto recibido no es un importe.
    ImporteIlegible { texto: String },
    /// Un avance de efectivo tiene que mover un importe positivo.
    AvanceNoPositivo,
    /// El porcentaje de un avance cae fuera de la banda que cobran las
    /// entidades. Casi siempre es un tecleo: 0.8 por 8, o 80 por 8.
    CargoDeAvanceFueraDeRango { porcentaje: f64, minimo: f64, maximo: f64 },
    /// Un cargo fijo de cero es una exoneración y debe declararse como tal.
    CargoFijoNoPositivo,
    /// El avance se acredita en la divisa en que se carga a la tarjeta.
    AvanceEnOtraDivisa { cuenta: Divisa, avance: Divisa },
}

impl fmt::Display for ErrorDominio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorDominio::DivisasIncompatibles { esperada, recibida } => write!(
                f,
                "No se pueden combinar montos en {} y {}: indique una tasa de cambio para convertirlos.",
                esperada.codigo(),
                recibida.codigo()
            ),
            ErrorDominio::ImporteIlegible { texto } => {
                write!(f, "«{}» no es un importe válido.", texto)
            }
            ErrorDominio::AvanceNoPositivo => {
                write!(f, "Un avance de efectivo debe ser de un importe mayor que cero.")
            }
            ErrorDominio::CargoDeAvanceFueraDeRango { porcentaje, minimo, maximo } => write!(
                f,
                "Un cargo del {}% queda fuera de lo que cobran las entidades por un avance, entre {}% y {}%. Si es un cargo fijo o una exoneración, indícalo así.",
                porcentaje, minimo, maximo
            ),
            ErrorDominio::CargoFijoNoPositivo => write!(
                f,
                "Un cargo fijo debe ser mayor que cero. Si el avance no paga cargo, márcalo como exonerado."
            ),
            ErrorDominio::AvanceEnOtraDivisa { cuenta, avance } => write!(
                f,
                "La cuenta es en {} y el avance en {}: un avance se acredita en la misma divisa en que se carga a la tarjeta.",
                cuenta.codigo(),
                avance.codigo()
            ),
            ErrorDominio::CobroParcialExcedeElNeto { parcial, neto } => write!(
                f,
                "Un cobro parcial de {:.2} no puede superar el neto de la factura, que es {:.2}.",
                parcial, neto
            ),
            ErrorDominio::ImportesNoCuadran { sale, entra, divisa } => write!(
                f,
                "Salen {:.2} {} y entran {:.2}: sin cambio de divisa los importes deben coincidir. Si la diferencia es una comisión, indíquela en el cargo.",
                sale,
                divisa.codigo(),
                entra
            ),
            ErrorDominio::TransferenciaALaMismaCuenta { cuenta_id } => write!(
                f,
                "Una transferencia necesita dos cuentas distintas; se indicó la {} en ambos extremos.",
                cuenta_id
            ),
            ErrorDominio::ReferenciaFaltante { metodo, referencia } => write!(
                f,
                "Un gasto pagado con {} debe indicar {}.",
                metodo, referencia
            ),
            ErrorDominio::TasaDeCambioRequerida => write!(
                f,
                "La operación cruza dos divisas distintas y requiere una tasa de cambio."
            ),
            ErrorDominio::TasaDeCambioInvalida { valor } => write!(
                f,
                "La tasa de cambio debe ser mayor que cero; se recibió {}.",
                valor
            ),
            ErrorDominio::MontoInvalido { valor } => {
                write!(f, "El monto {} no es un número válido.", valor)
            }
            ErrorDominio::FondosInsuficientes { disponible, requerido, divisa } => write!(
                f,
                "Fondos insuficientes: hay {:.2} {} disponibles y la operación requiere {:.2} {}.",
                disponible,
                divisa.codigo(),
                requerido,
                divisa.codigo()
            ),
            ErrorDominio::DivisaDesconocida { codigo } => write!(
                f,
                "Divisa no reconocida: '{}'. Las divisas admitidas son DOP y USD.",
                codigo
            ),
            ErrorDominio::ConceptoVacio => write!(
                f,
                "Indique el concepto de la bonificación: es lo que distingue un cashback de una promoción o una recompensa."
            ),
        }
    }
}

impl std::error::Error for ErrorDominio {}

// Permite que los comandos Tauri existentes, que devuelven Result<_, String>,
// adopten errores de dominio sin cambiar su firma durante la migración.
impl From<ErrorDominio> for String {
    fn from(e: ErrorDominio) -> String {
        e.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_mensaje_de_fondos_insuficientes_indica_ambas_cifras() {
        let e = ErrorDominio::FondosInsuficientes {
            disponible: 12000.0,
            requerido: 15000.0,
            divisa: Divisa::Dop,
        };
        let msg = e.to_string();
        assert!(msg.contains("12000.00"), "falta el disponible: {}", msg);
        assert!(msg.contains("15000.00"), "falta el requerido: {}", msg);
        assert!(msg.contains("DOP"));
    }

    #[test]
    fn el_error_se_convierte_a_string_para_los_comandos_tauri() {
        let e = ErrorDominio::TasaDeCambioRequerida;
        let como_string: String = e.clone().into();
        assert_eq!(como_string, e.to_string());
        assert!(!como_string.is_empty());
    }

    #[test]
    fn divisas_incompatibles_nombra_las_dos_divisas() {
        let msg = ErrorDominio::DivisasIncompatibles {
            esperada: Divisa::Dop,
            recibida: Divisa::Usd,
        }
        .to_string();
        assert!(msg.contains("DOP") && msg.contains("USD"));
    }
}
