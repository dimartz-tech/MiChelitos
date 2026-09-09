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
