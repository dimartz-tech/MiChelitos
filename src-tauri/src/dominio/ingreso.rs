//! Facturas emitidas y lo que de ellas se retiene.
//!
//! La retención de una factura es el mismo tipo de cálculo que la retención
//! de una transferencia: una proporción sobre un importe, que produce dinero
//! que se guarda. Vivía aparte y con otra regla de redondeo —a unidades, no a
//! céntimos—, que es lo que documentó **H16**.
//!
//! Aquí pasa a usar el mismo núcleo que todo lo demás. No por uniformidad
//! estética: porque una regla que existe dos veces se corrige una vez y sigue
//! mal en la otra, que es exactamente lo que había pasado con **H8**.

use super::dinero::{Dinero, Porcentaje};
use super::errores::ErrorDominio;

/// Lo que se retiene de una factura.
///
/// **Resolución de H16.** Antes se calculaba con `(monto × tasa).round()`,
/// que redondea a unidades enteras: el 15 % de 1 234.56 son 185.184, que
/// deberían ser 185.18 y quedaban en 185.00. Se perdían céntimos en cada
/// factura, siempre en la misma dirección.
pub fn retencion(monto: Dinero, tasa: Porcentaje) -> Result<Dinero, ErrorDominio> {
    monto.porcentaje(tasa)
}

/// Lo que queda por cobrar una vez retenido: el neto de la factura.
///
/// Se obtiene **restando**, no calculando el complemento con otro porcentaje.
/// Así la retención y el neto suman siempre el total exacto, sin que un
/// segundo redondeo los descuadre por un céntimo.
pub fn neto(monto: Dinero, tasa: Porcentaje) -> Result<Dinero, ErrorDominio> {
    monto.restar(&retencion(monto, tasa)?)
}

/// Qué se da por cobrado tras corregir una factura.
///
/// La regla es **el cobro completo**: si el titular corrige el monto de una
/// factura, lo normal es que el importe nuevo sea el que se cobró entero. Lo
/// parcial existe para las excepciones, y por eso hay que declararlo: una
/// diferencia entre lo facturado y lo que entró debe ser una afirmación, no
/// un residuo que sobrevive sin que nadie lo mire.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cobro {
    /// Entró el neto entero.
    Completo,
    /// Solo entró una parte, y se dice cuál.
    Parcial(Dinero),
}

/// Cómo cambia una factura al corregirla.
///
/// Corregir una factura **ya cobrada** no puede limitarse a reescribir sus
/// cifras: el dinero ya entró en una cuenta. Esta estructura separa lo que la
/// factura pasa a decir de lo que hay que mover para que la cuenta siga
/// cuadrando.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CorreccionDeFactura {
    pub retencion: Dinero,
    pub neto: Dinero,
    /// Lo que la factura pasa a declarar como cobrado.
    pub recibido: Dinero,
    /// Diferencia entre lo que pasa a estar cobrado y lo que lo estaba.
    ///
    /// Es lo que hay que sumar a la cuenta de depósito. Positiva si la
    /// corrección aumenta lo cobrado, negativa si lo reduce.
    pub ajuste: Dinero,
}

/// Calcula una corrección a partir de lo que la factura tenía cobrado.
///
/// **La cuenta sigue siempre a lo recibido**, sea completo o parcial: el
/// ajuste es la diferencia entre lo que pasa a estar cobrado y lo que lo
/// estaba. Esa es la única regla, y evita tener dos caminos que puedan
/// divergir.
pub fn corregir(
    monto: Dinero,
    tasa: Porcentaje,
    recibido_anterior: Dinero,
    cobro: Cobro,
) -> Result<CorreccionDeFactura, ErrorDominio> {
    let retencion = retencion(monto, tasa)?;
    let neto = monto.restar(&retencion)?;

    let recibido = match cobro {
        Cobro::Completo => neto,
        Cobro::Parcial(parte) => {
            if parte.es_negativo() {
                return Err(ErrorDominio::MontoInvalido { valor: parte.unidades() });
            }
            // Un «parcial» mayor que el neto no es parcial. Admitirlo dejaría
            // la factura diciendo que se cobró más de lo que se facturó, sin
            // nada que lo explicara.
            if parte.restar(&neto)?.centavos() > 0 {
                return Err(ErrorDominio::CobroParcialExcedeElNeto {
                    parcial: parte.unidades(),
                    neto: neto.unidades(),
                });
            }
            parte
        }
    };

    let ajuste = recibido.restar(&recibido_anterior)?;
    Ok(CorreccionDeFactura { retencion, neto, recibido, ajuste })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dominio::dinero::Divisa;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }

    fn quince_por_ciento() -> Porcentaje {
        Porcentaje::puntos_basicos(1500)
    }

    #[test]
    fn h16_la_retencion_se_redondea_al_centimo_y_no_a_unidades() {
        // El caso que la caracterización fijaba con el resultado antiguo: el
        // 15 % de 1 234.56 son 185.184, que se decidían en 185.00 y ahora se
        // deciden en 185.18.
        assert_eq!(retencion(dop(1_234.56), quince_por_ciento()).unwrap(), dop(185.18));
    }

    #[test]
    fn la_retencion_y_el_neto_suman_siempre_el_total() {
        // La propiedad que obliga a restar en vez de calcular el complemento.
        // Con dos porcentajes redondeados por separado, esto falla por un
        // céntimo en cuanto el importe no es redondo.
        for u in [1_234.56, 10_000.0, 0.01, 999_999.99, 7.77] {
            let monto = dop(u);
            let r = retencion(monto, quince_por_ciento()).unwrap();
            let n = neto(monto, quince_por_ciento()).unwrap();
            assert_eq!(r.sumar(&n).unwrap(), monto, "no cuadra con {u}");
        }
    }

    #[test]
    fn una_retencion_de_cero_deja_el_neto_igual_al_total() {
        let monto = dop(5_000.0);
        assert_eq!(retencion(monto, Porcentaje::puntos_basicos(0)).unwrap(), dop(0.0));
        assert_eq!(neto(monto, Porcentaje::puntos_basicos(0)).unwrap(), monto);
    }

    // --- Corrección ---

    #[test]
    fn corregir_al_alza_da_por_cobrado_el_neto_nuevo() {
        // La regla: corregir una factura la da por cobrada entera. El neto
        // sube de 8 500 a 10 200, así que faltan 1 700 por entrar.
        let c = corregir(dop(12_000.0), quince_por_ciento(), dop(8_500.0), Cobro::Completo)
            .unwrap();

        assert_eq!(c.retencion, dop(1_800.0));
        assert_eq!(c.neto, dop(10_200.0));
        assert_eq!(c.recibido, dop(10_200.0), "se da por cobrado entero");
        assert_eq!(c.ajuste, dop(1_700.0));
    }

    #[test]
    fn corregir_a_la_baja_devuelve_un_ajuste_negativo() {
        let c = corregir(dop(8_000.0), quince_por_ciento(), dop(8_500.0), Cobro::Completo)
            .unwrap();

        assert_eq!(c.neto, dop(6_800.0));
        assert_eq!(c.ajuste, dop(-1_700.0), "hay que retirar de la cuenta");
    }

    #[test]
    fn corregir_sin_cambiar_nada_no_mueve_ningun_saldo() {
        let c = corregir(dop(10_000.0), quince_por_ciento(), dop(8_500.0), Cobro::Completo)
            .unwrap();

        assert_eq!(c.ajuste, dop(0.0), "corregir la fecha no toca la cuenta");
    }

    #[test]
    fn un_cobro_parcial_declara_lo_que_entro_de_verdad() {
        // La excepción, y hay que afirmarla. El neto es 10 200 pero solo
        // entraron 9 000: la cuenta sube 500 sobre los 8 500 que ya tenía.
        let c = corregir(
            dop(12_000.0), quince_por_ciento(), dop(8_500.0), Cobro::Parcial(dop(9_000.0)),
        )
        .unwrap();

        assert_eq!(c.neto, dop(10_200.0), "lo facturado no cambia");
        assert_eq!(c.recibido, dop(9_000.0), "lo cobrado sí");
        assert_eq!(c.ajuste, dop(500.0));
        assert_eq!(c.neto.restar(&c.recibido).unwrap(), dop(1_200.0), "queda por cobrar");
    }

    #[test]
    fn un_parcial_mayor_que_el_neto_no_es_parcial() {
        // Admitirlo dejaría la factura diciendo que se cobró más de lo
        // facturado, sin nada que lo explicara.
        let r = corregir(
            dop(10_000.0), quince_por_ciento(), dop(0.0), Cobro::Parcial(dop(9_000.0)),
        );

        assert!(matches!(r, Err(ErrorDominio::CobroParcialExcedeElNeto { .. })));
    }

    #[test]
    fn un_parcial_igual_al_neto_es_valido_y_equivale_al_completo() {
        // El borde: cobrar exactamente el neto es un cobro completo dicho de
        // otra forma, y rechazarlo sería un estorbo sin motivo.
        let completo =
            corregir(dop(10_000.0), quince_por_ciento(), dop(0.0), Cobro::Completo).unwrap();
        let parcial = corregir(
            dop(10_000.0), quince_por_ciento(), dop(0.0), Cobro::Parcial(dop(8_500.0)),
        )
        .unwrap();

        assert_eq!(completo, parcial);
    }

    #[test]
    fn un_parcial_negativo_se_rechaza() {
        let r = corregir(
            dop(10_000.0), quince_por_ciento(), dop(0.0), Cobro::Parcial(dop(-1.0)),
        );

        assert!(matches!(r, Err(ErrorDominio::MontoInvalido { .. })));
    }

    #[test]
    fn la_cuenta_sigue_siempre_a_lo_recibido() {
        // La única regla, comprobada en los dos caminos: el ajuste es la
        // diferencia entre lo que pasa a estar cobrado y lo que lo estaba.
        for cobro in [Cobro::Completo, Cobro::Parcial(dop(9_000.0))] {
            let c = corregir(dop(12_000.0), quince_por_ciento(), dop(8_500.0), cobro).unwrap();
            assert_eq!(
                c.recibido.restar(&dop(8_500.0)).unwrap(),
                c.ajuste,
                "el ajuste no sigue a lo recibido con {cobro:?}"
            );
        }
    }
}
