//! Método de pago de un gasto y su efecto sobre los saldos.
//!
//! Hasta ahora el método era un `String` comparado con `==` en cuatro puntos
//! repartidos entre `crear_gasto` y `eliminar_gasto`. Añadir un método obligaba
//! a encontrarlos todos; olvidar uno pasaba inadvertido.
//!
//! Aquí el método es un tipo cerrado y su efecto sobre los saldos es un valor
//! explícito. El compilador exige tratar todos los casos, de modo que añadir un
//! método nuevo señala por sí solo cada punto que hay que completar.

use super::dinero::Divisa;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetodoPago {
    Efectivo,
    Tarjeta,
    Transferencia,
}

impl MetodoPago {
    /// Interpreta el texto almacenado en `gastos.metodo_pago`.
    ///
    /// Devuelve `None` ante un valor desconocido en lugar de error, porque hoy
    /// un método no reconocido simplemente no afecta a ningún saldo y es la
    /// restricción `CHECK` de la tabla la que acaba rechazando la inserción.
    pub fn desde_codigo(codigo: &str) -> Option<MetodoPago> {
        match codigo {
            "efectivo" => Some(MetodoPago::Efectivo),
            "tarjeta" => Some(MetodoPago::Tarjeta),
            "transferencia" => Some(MetodoPago::Transferencia),
            _ => None,
        }
    }

    pub fn codigo(&self) -> &'static str {
        match self {
            MetodoPago::Efectivo => "efectivo",
            MetodoPago::Tarjeta => "tarjeta",
            MetodoPago::Transferencia => "transferencia",
        }
    }

    /// Solo las transferencias devengan retención impositiva y comisión.
    pub fn devenga_cargos(&self) -> bool {
        matches!(self, MetodoPago::Transferencia)
    }
}

/// Nombre de la cuenta que representa la caja física de una divisa.
///
/// El vínculo es por nombre y no por identificador, que es la causa de **H3**:
/// si la fila se renombra, el gasto se registra sin mover saldo alguno.
pub fn nombre_caja(divisa: Divisa) -> &'static str {
    match divisa {
        Divisa::Dop => "Efectivo DOP",
        Divisa::Usd => "Efectivo USD",
    }
}

/// Qué saldo mueve un gasto, y cuál.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AfectacionSaldo {
    /// Ningún saldo cambia. Es un resultado real del sistema actual, no un
    /// hueco: ocurre cuando falta el identificador que el método necesita.
    /// Nombrarlo hace visible **H4** en lugar de esconderlo en un `if let`
    /// que no entra.
    Ninguna,
    DeudaTarjeta { tarjeta_id: i64 },
    DebitoCuenta { cuenta_id: i64 },
    DebitoCaja { divisa: Divisa },
}

/// Resuelve el efecto sobre los saldos a partir del método y los datos
/// disponibles. Es el único punto donde se decide qué saldo se toca.
pub fn afectacion_de_gasto(
    metodo: Option<MetodoPago>,
    divisa: Divisa,
    tarjeta_id: Option<i64>,
    cuenta_id: Option<i64>,
) -> AfectacionSaldo {
    match metodo {
        Some(MetodoPago::Efectivo) => AfectacionSaldo::DebitoCaja { divisa },
        Some(MetodoPago::Tarjeta) => match tarjeta_id {
            Some(tarjeta_id) => AfectacionSaldo::DeudaTarjeta { tarjeta_id },
            None => AfectacionSaldo::Ninguna,
        },
        Some(MetodoPago::Transferencia) => match cuenta_id {
            Some(cuenta_id) => AfectacionSaldo::DebitoCuenta { cuenta_id },
            None => AfectacionSaldo::Ninguna,
        },
        None => AfectacionSaldo::Ninguna,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Interpretación del método ---

    #[test]
    fn los_codigos_coinciden_con_el_check_de_la_tabla() {
        assert_eq!(MetodoPago::desde_codigo("efectivo"), Some(MetodoPago::Efectivo));
        assert_eq!(MetodoPago::desde_codigo("tarjeta"), Some(MetodoPago::Tarjeta));
        assert_eq!(MetodoPago::desde_codigo("transferencia"), Some(MetodoPago::Transferencia));
    }

    #[test]
    fn el_codigo_va_y_vuelve_sin_perderse() {
        for m in [MetodoPago::Efectivo, MetodoPago::Tarjeta, MetodoPago::Transferencia] {
            assert_eq!(MetodoPago::desde_codigo(m.codigo()), Some(m));
        }
    }

    #[test]
    fn un_metodo_desconocido_no_se_interpreta() {
        assert_eq!(MetodoPago::desde_codigo("cheque"), None);
        // La comparación distingue mayúsculas, igual que el código vigente.
        assert_eq!(MetodoPago::desde_codigo("Efectivo"), None);
    }

    #[test]
    fn solo_la_transferencia_devenga_cargos() {
        assert!(MetodoPago::Transferencia.devenga_cargos());
        assert!(!MetodoPago::Tarjeta.devenga_cargos());
        assert!(!MetodoPago::Efectivo.devenga_cargos());
    }

    // --- Caja ---

    #[test]
    fn cada_divisa_tiene_su_caja() {
        assert_eq!(nombre_caja(Divisa::Dop), "Efectivo DOP");
        assert_eq!(nombre_caja(Divisa::Usd), "Efectivo USD");
    }

    // --- Efecto sobre los saldos ---

    #[test]
    fn el_efectivo_debita_la_caja_de_su_divisa() {
        assert_eq!(
            afectacion_de_gasto(Some(MetodoPago::Efectivo), Divisa::Usd, None, None),
            AfectacionSaldo::DebitoCaja { divisa: Divisa::Usd }
        );
    }

    #[test]
    fn la_tarjeta_incrementa_la_deuda_de_la_tarjeta_indicada() {
        assert_eq!(
            afectacion_de_gasto(Some(MetodoPago::Tarjeta), Divisa::Dop, Some(7), None),
            AfectacionSaldo::DeudaTarjeta { tarjeta_id: 7 }
        );
    }

    #[test]
    fn la_transferencia_debita_la_cuenta_indicada() {
        assert_eq!(
            afectacion_de_gasto(Some(MetodoPago::Transferencia), Divisa::Dop, None, Some(3)),
            AfectacionSaldo::DebitoCuenta { cuenta_id: 3 }
        );
    }

    #[test]
    fn h4_una_tarjeta_sin_identificador_no_mueve_ningun_saldo() {
        // Conducta vigente, ahora con nombre propio en lugar de un `if let`
        // que no entra. Queda pendiente de decisión.
        assert_eq!(
            afectacion_de_gasto(Some(MetodoPago::Tarjeta), Divisa::Dop, None, None),
            AfectacionSaldo::Ninguna
        );
    }

    #[test]
    fn una_transferencia_sin_cuenta_tampoco_mueve_saldo() {
        // La retención sí se calcula y se guarda; simplemente no hay cuenta
        // de la que debitarla.
        assert_eq!(
            afectacion_de_gasto(Some(MetodoPago::Transferencia), Divisa::Dop, None, None),
            AfectacionSaldo::Ninguna
        );
    }

    #[test]
    fn un_metodo_no_reconocido_no_mueve_saldo() {
        assert_eq!(
            afectacion_de_gasto(None, Divisa::Dop, Some(7), Some(3)),
            AfectacionSaldo::Ninguna
        );
    }

    #[test]
    fn la_tarjeta_ignora_la_cuenta_y_la_transferencia_ignora_la_tarjeta() {
        // Cada método atiende solo al identificador que le corresponde.
        assert_eq!(
            afectacion_de_gasto(Some(MetodoPago::Tarjeta), Divisa::Dop, Some(7), Some(3)),
            AfectacionSaldo::DeudaTarjeta { tarjeta_id: 7 }
        );
        assert_eq!(
            afectacion_de_gasto(Some(MetodoPago::Transferencia), Divisa::Dop, Some(7), Some(3)),
            AfectacionSaldo::DebitoCuenta { cuenta_id: 3 }
        );
    }
}
