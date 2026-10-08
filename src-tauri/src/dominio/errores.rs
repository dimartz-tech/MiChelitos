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
    /// Una suscripción cobra un importe mayor que cero: uno de cero o negativo
    /// no cobra, abonaría a la tarjeta cada período.
    SuscripcionSinImporte,
    /// Un abono a la tarjeta es mayor que cero: uno de cero no abona nada y uno
    /// negativo **subiría** la deuda.
    AbonoSinImporte,
    /// Catálogos (categorías y clientes). Los textos son los de siempre: la interfaz los muestra tal cual.
    FacturaDuplicada,
    FechaDeCobroNoEntendida { fecha: String },
    FechaDeCorreccionNoEntendida { fecha: String },
    SuscripcionNoEncontrada,
    SuscripcionNoEncontradaParaEditar,
    SuscripcionNoEncontradaParaCorregir,
    FrecuenciaNoReconocida,
    SinPeriodosPendientes,
    /// Un gasto que creó otra operación no se borra solo; el texto dice qué operación revertir.
    GastoDerivado { motivo: String },
    PorcentajeDeCargoRequerido,
    ImporteDeCargoFijoRequerido,
    CargoContradictorio,
    TipoDeCargoDesconocido { tipo: String },
    FechaNoEntendida { fecha: String },
    IngresoNoPendiente { id: i64 },
    CuotasTotalesRequeridas,
    CuotasPendientesRequeridas,
    CuotasPendientesExcedenTotales,
    DiaDePagoInvalido,
    LimiteSoloEnLineaRevolvente,
    FinanciamientoNoEncontrado { id: i64 },
    TarjetaNoEncontrada { id: i64 },
    FacturaNoEncontrada { id: i64 },
    /// Una factura cobrada en una cuenta que ya no existe no se puede corregir: el ajuste no tendría adónde ir.
    CuentaDeDepositoInexistente { cuenta: String },
    MotivoInsuficiente { motivo: String },
    FacturaNoPendiente { id: i64 },
    CuentaSinNombre,
    ComisionNegativa,
    CuentaNoEncontrada { id: i64 },
    CategoriaSinNombre,
    CategoriaExistente,
    CategoriaDelSistema,
    CategoriaConGastos,
    ClienteSinDatos,
    ClienteDuplicado,
    ClienteConFacturas,
    /// El día de facturación tiene que ser un día del mes.
    DiaDeFacturacionInvalido { dia: i32 },
    /// La frecuencia no es ninguna de las que admite el esquema.
    FrecuenciaDesconocida { codigo: String },
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
            ErrorDominio::SuscripcionSinImporte => write!(
                f,
                "Una suscripción debe cobrar un importe mayor que cero: con cero o menos no cobraría, abonaría a la tarjeta cada período."
            ),
            ErrorDominio::FacturaNoEncontrada { id } => write!(f, "No se encontró la factura {}.", id),
            ErrorDominio::CuentaDeDepositoInexistente { cuenta } => write!(
                f,
                "La factura se cobró en «{}», que ya no existe. Corrige o recrea esa cuenta antes de modificar la factura.",
                cuenta
            ),
            ErrorDominio::MotivoInsuficiente { motivo } => write!(
                f,
                "Explica la corrección en al menos {} caracteres. Dentro de seis meses, «{}» no dirá qué pasó.",
                super::correccion::MINIMO_DEL_MOTIVO,
                motivo
            ),
            ErrorDominio::CuotasTotalesRequeridas => write!(f, "Las cuotas totales son requeridas."),
            ErrorDominio::CuotasPendientesRequeridas => write!(f, "Las cuotas pendientes son requeridas."),
            ErrorDominio::CuotasPendientesExcedenTotales => write!(
                f,
                "Error: El número de cuotas pendientes no puede ser mayor al número total de cuotas del préstamo."
            ),
            ErrorDominio::DiaDePagoInvalido => write!(f, "El día de pago debe ser un día válido del mes (1-31)."),
            ErrorDominio::LimiteSoloEnLineaRevolvente => write!(f, "Solo una línea revolvente tiene límite de crédito."),
            ErrorDominio::FinanciamientoNoEncontrado { id } => write!(f, "No se encontró el financiamiento {}.", id),
            ErrorDominio::TarjetaNoEncontrada { id } => write!(f, "No se encontró la tarjeta {}.", id),
            ErrorDominio::IngresoNoPendiente { id } => {
                write!(f, "No se encontró un ingreso {} pendiente de cobro.", id)
            }
            ErrorDominio::PorcentajeDeCargoRequerido => write!(f, "Indica el porcentaje del cargo."),
            ErrorDominio::ImporteDeCargoFijoRequerido => write!(f, "Indica el importe del cargo fijo."),
            ErrorDominio::CargoContradictorio => {
                write!(f, "El tipo de cargo y los valores que se indican se contradicen.")
            }
            ErrorDominio::TipoDeCargoDesconocido { tipo } => write!(
                f,
                "Tipo de cargo «{}» desconocido. Los admitidos son porcentaje, fijo y exonerado.",
                tipo
            ),
            ErrorDominio::FechaNoEntendida { fecha } => {
                write!(f, "La fecha «{}» no se entiende. Se espera dd/mm/aaaa.", fecha)
            }
            ErrorDominio::GastoDerivado { motivo } => write!(f, "{}", motivo),
            ErrorDominio::FechaDeCobroNoEntendida { fecha } => write!(
                f,
                "La fecha del próximo cobro «{}» no se entiende. Se espera dd/mm/aaaa.",
                fecha
            ),
            ErrorDominio::FechaDeCorreccionNoEntendida { fecha } => {
                write!(f, "«{}» no se entiende como fecha. Se espera dd/mm/aaaa.", fecha)
            }
            ErrorDominio::SuscripcionNoEncontrada => write!(f, "No se encontró la suscripción."),
            ErrorDominio::SuscripcionNoEncontradaParaEditar => {
                write!(f, "No se encontró la suscripción que se intenta editar.")
            }
            ErrorDominio::SuscripcionNoEncontradaParaCorregir => {
                write!(f, "No se encontró la suscripción que se intenta corregir.")
            }
            ErrorDominio::FrecuenciaNoReconocida => {
                write!(f, "La suscripción tiene una frecuencia que no se reconoce.")
            }
            ErrorDominio::SinPeriodosPendientes => {
                write!(f, "Esta suscripción no tiene períodos pendientes de confirmar.")
            }
            ErrorDominio::FacturaDuplicada => write!(f, "El número de factura ya está registrado."),
            ErrorDominio::FacturaNoPendiente { id } => {
                write!(f, "No se encontró una factura {} pendiente de cobro.", id)
            }
            ErrorDominio::CuentaSinNombre => write!(f, "El nombre de la cuenta no puede estar vacío."),
            ErrorDominio::ComisionNegativa => write!(f, "La comisión por pago de impuestos no puede ser negativa."),
            ErrorDominio::CuentaNoEncontrada { id } => write!(f, "No se encontró la cuenta {}.", id),
            ErrorDominio::CategoriaSinNombre => write!(f, "El nombre de la categoría no puede estar vacío."),
            ErrorDominio::CategoriaExistente => write!(f, "La categoría ya existe."),
            ErrorDominio::CategoriaDelSistema => write!(f, "No se puede eliminar la categoría de sistema 'Otros'."),
            ErrorDominio::CategoriaConGastos => write!(
                f,
                "No se puede eliminar la categoría porque tiene gastos registrados asociados."
            ),
            ErrorDominio::ClienteSinDatos => write!(f, "RNC y nombre no pueden estar vacíos."),
            ErrorDominio::ClienteDuplicado => write!(f, "Ya existe un cliente con este RNC."),
            ErrorDominio::ClienteConFacturas => write!(
                f,
                "No se puede eliminar el cliente porque tiene facturas registradas."
            ),
            ErrorDominio::AbonoSinImporte => write!(
                f,
                "Un abono debe ser mayor que cero: con cero no abona nada y con un importe negativo subiría la deuda."
            ),
            ErrorDominio::DiaDeFacturacionInvalido { dia } => write!(
                f,
                "El día de facturación debe estar entre 1 y 31; se recibió {}.",
                dia
            ),
            ErrorDominio::FrecuenciaDesconocida { codigo } => write!(
                f,
                "Frecuencia «{}» desconocida. Las admitidas son mensual y anual.",
                codigo
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
