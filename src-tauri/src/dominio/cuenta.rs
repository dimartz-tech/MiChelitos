//! Cuentas de ahorro y transferencias entre ellas.
//!
//! Una transferencia es la única operación del sistema con **tres efectos que
//! deben ocurrir juntos**: sale dinero de una cuenta, entra en otra y queda un
//! asiento. Hasta ahora vivía como tres sentencias SQL seguidas dentro de un
//! comando, sin nada que impidiera construir una que no significara nada.
//!
//! Este módulo la convierte en un tipo que **no se puede construir mal**. Dos
//! de los hallazgos de la Fase 3 dejan de ser comprobaciones que alguien debe
//! acordarse de escribir y pasan a ser condiciones para que el valor exista:
//!
//! * **H11** — origen y destino iguales: rechazado al construir.
//! * **H12** — importe acreditado en una divisa que no es la de la cuenta que
//!   lo recibe: irrepresentable, porque cada importe se valida contra la
//!   divisa de su extremo. Es el mismo camino por el que se cerró H2.
//!
//! Lo que este módulo **no** decide es qué hacer cuando los importes no
//! cuadran en una transferencia de la misma divisa (**H14**). Lo expone para
//! que se vea, y la decisión queda para el caso de uso.

use super::dinero::{Dinero, Divisa, TasaCambio};
use super::errores::ErrorDominio;

/// El extremo de una transferencia: qué cuenta y en qué divisa opera.
///
/// La divisa viaja con el identificador porque sin ella no se puede validar
/// nada: es lo que permite rechazar un importe que no corresponde al extremo
/// donde se aplica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtremoCuenta {
    pub id: i64,
    pub divisa: Divisa,
}

impl ExtremoCuenta {
    pub fn nuevo(id: i64, divisa: Divisa) -> ExtremoCuenta {
        ExtremoCuenta { id, divisa }
    }
}

/// Un traspaso de fondos entre dos cuentas, con su cargo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transferencia {
    origen: ExtremoCuenta,
    destino: ExtremoCuenta,
    monto_origen: Dinero,
    monto_destino: Dinero,
    cargo: Dinero,
}

impl Transferencia {
    /// Construye la transferencia, o falla si no significaría nada.
    ///
    /// Las condiciones no son validaciones defensivas sino la definición de lo
    /// que es una transferencia: dos cuentas distintas, un importe positivo
    /// saliendo, otro positivo entrando, y cada uno expresado en la divisa del
    /// extremo donde se aplica. El cargo lo paga el origen, de modo que va en
    /// la divisa del origen.
    pub fn nueva(
        origen: ExtremoCuenta,
        destino: ExtremoCuenta,
        monto_origen: Dinero,
        monto_destino: Dinero,
        cargo: Dinero,
    ) -> Result<Transferencia, ErrorDominio> {
        if origen.id == destino.id {
            return Err(ErrorDominio::TransferenciaALaMismaCuenta { cuenta_id: origen.id });
        }

        exigir_divisa(origen.divisa, monto_origen)?;
        exigir_divisa(destino.divisa, monto_destino)?;
        // El cargo lo cobra el banco del origen y sale de esa misma cuenta.
        exigir_divisa(origen.divisa, cargo)?;

        if monto_origen.es_cero() || monto_origen.es_negativo() {
            return Err(ErrorDominio::MontoInvalido { valor: monto_origen.unidades() });
        }
        if monto_destino.es_cero() || monto_destino.es_negativo() {
            return Err(ErrorDominio::MontoInvalido { valor: monto_destino.unidades() });
        }
        if cargo.es_negativo() {
            return Err(ErrorDominio::MontoInvalido { valor: cargo.unidades() });
        }

        // H14, resuelto: sin cambio de divisa, lo que sale y lo que entra
        // tienen que coincidir. La diferencia no puede ser otra cosa que una
        // comisión —y para eso está el cargo, que sale aparte— o un error de
        // tecleo. Admitirla hacía aparecer o desaparecer dinero entre las dos
        // cuentas sin que nada lo dijera.
        if origen.divisa == destino.divisa && monto_origen != monto_destino {
            return Err(ErrorDominio::ImportesNoCuadran {
                sale: monto_origen.unidades(),
                entra: monto_destino.unidades(),
                divisa: origen.divisa,
            });
        }

        Ok(Transferencia { origen, destino, monto_origen, monto_destino, cargo })
    }

    pub fn origen(&self) -> ExtremoCuenta {
        self.origen
    }

    pub fn destino(&self) -> ExtremoCuenta {
        self.destino
    }

    pub fn monto_origen(&self) -> Dinero {
        self.monto_origen
    }

    pub fn monto_destino(&self) -> Dinero {
        self.monto_destino
    }

    pub fn cargo(&self) -> Dinero {
        self.cargo
    }

    /// Lo que realmente sale de la cuenta de origen: el importe y el cargo.
    ///
    /// Se devuelve como el débito a aplicar —negativo— para que quien lo use
    /// no tenga que acordarse del signo. Que el cargo lo pague el origen es
    /// una decisión de negocio, y vive aquí en vez de repartida entre el
    /// comando que transfiere y el que revierte, donde estaba duplicada.
    pub fn debito_al_origen(&self) -> Result<Dinero, ErrorDominio> {
        Ok(self.monto_origen.sumar(&self.cargo)?.negado())
    }

    /// Lo que entra en la cuenta de destino. Nunca incluye el cargo.
    pub fn credito_al_destino(&self) -> Dinero {
        self.monto_destino
    }

    pub fn cruza_divisas(&self) -> bool {
        self.origen.divisa != self.destino.divisa
    }

    /// Tasa implícita en los dos importes.
    ///
    /// Se deduce dividiendo porque es lo que el sistema puede saber: quien
    /// transfiere conoce cuánto salió y cuánto llegó, no a qué tasa lo
    /// convirtió el banco. En una transferencia sin cambio de divisa la tasa
    /// no significa nada y por eso es `None` en vez de un 1 engañoso.
    pub fn tasa(&self) -> Result<Option<TasaCambio>, ErrorDominio> {
        if !self.cruza_divisas() {
            return Ok(None);
        }
        let cociente = self.monto_destino.unidades() / self.monto_origen.unidades();
        TasaCambio::nueva(cociente).map(Some)
    }

}

fn exigir_divisa(esperada: Divisa, importe: Dinero) -> Result<(), ErrorDominio> {
    if importe.divisa() != esperada {
        return Err(ErrorDominio::DivisasIncompatibles {
            esperada,
            recibida: importe.divisa(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }

    fn usd(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Usd).unwrap()
    }

    fn cuenta_dop(id: i64) -> ExtremoCuenta {
        ExtremoCuenta::nuevo(id, Divisa::Dop)
    }

    fn cuenta_usd(id: i64) -> ExtremoCuenta {
        ExtremoCuenta::nuevo(id, Divisa::Usd)
    }

    // --- Construcción ---

    #[test]
    fn una_transferencia_en_la_misma_divisa_se_construye() {
        let t = Transferencia::nueva(
            cuenta_dop(1), cuenta_dop(2), dop(8_000.0), dop(8_000.0), dop(100.0),
        )
        .unwrap();

        assert_eq!(t.monto_origen(), dop(8_000.0));
        assert_eq!(t.credito_al_destino(), dop(8_000.0));
        assert!(!t.cruza_divisas());
    }

    #[test]
    fn h11_no_se_puede_transferir_una_cuenta_a_si_misma() {
        let r = Transferencia::nueva(
            cuenta_dop(7), cuenta_dop(7), dop(8_000.0), dop(8_000.0), dop(0.0),
        );

        assert_eq!(r, Err(ErrorDominio::TransferenciaALaMismaCuenta { cuenta_id: 7 }));
    }

    #[test]
    fn h12_no_se_puede_acreditar_una_divisa_que_no_es_la_del_destino() {
        // El caso que hoy pasa sin aviso: pesos acreditados a una cuenta USD.
        let r = Transferencia::nueva(
            cuenta_dop(1), cuenta_usd(2), dop(6_000.0), dop(6_000.0), dop(0.0),
        );

        assert_eq!(
            r,
            Err(ErrorDominio::DivisasIncompatibles { esperada: Divisa::Usd, recibida: Divisa::Dop })
        );
    }

    #[test]
    fn h12_tampoco_se_puede_debitar_una_divisa_que_no_es_la_del_origen() {
        let r = Transferencia::nueva(
            cuenta_dop(1), cuenta_usd(2), usd(100.0), usd(100.0), dop(0.0),
        );

        assert_eq!(
            r,
            Err(ErrorDominio::DivisasIncompatibles { esperada: Divisa::Dop, recibida: Divisa::Usd })
        );
    }

    #[test]
    fn el_cargo_va_en_la_divisa_del_origen_porque_lo_paga_el_origen() {
        let r = Transferencia::nueva(
            cuenta_dop(1), cuenta_usd(2), dop(6_000.0), usd(100.0), usd(2.0),
        );

        assert_eq!(
            r,
            Err(ErrorDominio::DivisasIncompatibles { esperada: Divisa::Dop, recibida: Divisa::Usd })
        );
    }

    #[test]
    fn una_transferencia_de_cero_o_negativa_no_es_una_transferencia() {
        for (o, d) in [(0.0, 8_000.0), (8_000.0, 0.0), (-100.0, 8_000.0), (8_000.0, -100.0)] {
            assert!(
                Transferencia::nueva(cuenta_dop(1), cuenta_dop(2), dop(o), dop(d), dop(0.0))
                    .is_err(),
                "origen {o}, destino {d}"
            );
        }
    }

    #[test]
    fn un_cargo_negativo_se_rechaza_pero_cero_es_valido() {
        assert!(Transferencia::nueva(
            cuenta_dop(1), cuenta_dop(2), dop(100.0), dop(100.0), dop(-1.0)
        )
        .is_err());

        assert!(Transferencia::nueva(
            cuenta_dop(1), cuenta_dop(2), dop(100.0), dop(100.0), dop(0.0)
        )
        .is_ok(), "una transferencia sin cargo es corriente");
    }

    // --- Efectos ---

    #[test]
    fn el_debito_al_origen_incluye_el_cargo_y_viene_con_signo_negativo() {
        let t = Transferencia::nueva(
            cuenta_dop(1), cuenta_dop(2), dop(8_000.0), dop(8_000.0), dop(100.0),
        )
        .unwrap();

        assert_eq!(t.debito_al_origen().unwrap(), dop(-8_100.0));
        assert_eq!(t.credito_al_destino(), dop(8_000.0), "el destino no paga el cargo");
    }

    #[test]
    fn aplicar_el_debito_y_el_credito_conserva_el_dinero_menos_el_cargo() {
        // La propiedad que hace que una transferencia sea una transferencia:
        // lo que falta en un lado aparece en el otro, salvo lo que se lleva
        // el banco.
        let t = Transferencia::nueva(
            cuenta_dop(1), cuenta_dop(2), dop(8_000.0), dop(8_000.0), dop(100.0),
        )
        .unwrap();

        let movido = t.debito_al_origen().unwrap().sumar(&t.credito_al_destino()).unwrap();
        assert_eq!(movido, t.cargo().negado());
    }

    // --- Tasa ---

    #[test]
    fn la_tasa_se_deduce_de_los_dos_importes_cuando_hay_cambio_de_divisa() {
        let t = Transferencia::nueva(
            cuenta_dop(1), cuenta_usd(2), dop(6_000.0), usd(100.0), dop(0.0),
        )
        .unwrap();

        let tasa = t.tasa().unwrap().expect("cruza divisas");
        // La tolerancia es la de la escala de la tasa, no la de `f64`. 100/6000
        // es un decimal periódico: no cabe exacto en ninguna escala finita, y
        // por eso los importes son lo autoritativo y la tasa un acompañante.
        assert!((tasa.valor() - 100.0 / 6_000.0).abs() < 1e-9);
    }

    #[test]
    fn sin_cambio_de_divisa_no_hay_tasa_en_vez_de_un_uno_enganoso() {
        let t = Transferencia::nueva(
            cuenta_dop(1), cuenta_dop(2), dop(8_000.0), dop(8_000.0), dop(0.0),
        )
        .unwrap();

        assert_eq!(t.tasa().unwrap(), None);
    }

    // --- H14, resuelto: los importes tienen que cuadrar ---

    #[test]
    fn h14_sin_cambio_de_divisa_los_importes_deben_coincidir() {
        // Antes se admitía y 500 desaparecían entre las dos cuentas sin que
        // nada lo dijera. La diferencia solo puede ser una comisión —y para
        // eso está el cargo, que sale aparte— o un error de tecleo.
        let r = Transferencia::nueva(
            cuenta_dop(1), cuenta_dop(2), dop(8_000.0), dop(7_500.0), dop(0.0),
        );

        assert_eq!(
            r,
            Err(ErrorDominio::ImportesNoCuadran {
                sale: 8_000.0,
                entra: 7_500.0,
                divisa: Divisa::Dop
            })
        );
    }

    #[test]
    fn el_mensaje_del_descuadre_dice_donde_va_la_comision() {
        let error = Transferencia::nueva(
            cuenta_dop(1), cuenta_dop(2), dop(8_000.0), dop(7_500.0), dop(0.0),
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("comisión"), "orienta hacia el cargo: {error}");
        assert!(error.contains("cargo"), "y lo nombra: {error}");
    }

    #[test]
    fn el_cargo_no_cuenta_como_descuadre_porque_sale_aparte() {
        // Salen 8 000 y entran 8 000; los 100 del cargo se cobran además, al
        // origen, y no rompen la igualdad.
        assert!(Transferencia::nueva(
            cuenta_dop(1), cuenta_dop(2), dop(8_000.0), dop(8_000.0), dop(100.0)
        )
        .is_ok());
    }

    #[test]
    fn con_cambio_de_divisa_los_importes_no_tienen_por_que_coincidir() {
        // Es lo normal: seis mil pesos se convierten en cien dólares.
        assert!(Transferencia::nueva(
            cuenta_dop(1), cuenta_usd(2), dop(6_000.0), usd(100.0), dop(0.0)
        )
        .is_ok());
    }
}
