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
    /// Diferencia entre el neto nuevo y el anterior.
    ///
    /// Es lo que hay que sumar a la cuenta que cobró y al importe recibido.
    /// Positiva si la corrección aumenta lo que corresponde cobrar.
    pub ajuste: Dinero,
}

/// Calcula una corrección a partir del neto que la factura tenía antes.
///
/// El ajuste se aplica **al importe recibido**, no se sustituye por el neto
/// nuevo. Conserva así cualquier diferencia deliberada entre lo facturado y
/// lo que de verdad entró —un cobro parcial, por ejemplo—: corregir el total
/// mueve ambas cifras a la vez y la relación entre ellas sobrevive.
pub fn corregir(
    monto: Dinero,
    tasa: Porcentaje,
    neto_anterior: Dinero,
) -> Result<CorreccionDeFactura, ErrorDominio> {
    let retencion = retencion(monto, tasa)?;
    let neto = monto.restar(&retencion)?;
    let ajuste = neto.restar(&neto_anterior)?;
    Ok(CorreccionDeFactura { retencion, neto, ajuste })
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
    fn corregir_al_alza_devuelve_lo_que_falta_por_acreditar() {
        // La factura decía 10 000 y eran 12 000. El neto sube de 8 500 a
        // 10 200, así que faltan 1 700 por entrar en la cuenta.
        let c = corregir(dop(12_000.0), quince_por_ciento(), dop(8_500.0)).unwrap();

        assert_eq!(c.retencion, dop(1_800.0));
        assert_eq!(c.neto, dop(10_200.0));
        assert_eq!(c.ajuste, dop(1_700.0));
    }

    #[test]
    fn corregir_a_la_baja_devuelve_un_ajuste_negativo() {
        let c = corregir(dop(8_000.0), quince_por_ciento(), dop(8_500.0)).unwrap();

        assert_eq!(c.neto, dop(6_800.0));
        assert_eq!(c.ajuste, dop(-1_700.0), "hay que retirar de la cuenta");
    }

    #[test]
    fn corregir_sin_cambiar_nada_no_mueve_ningun_saldo() {
        let c = corregir(dop(10_000.0), quince_por_ciento(), dop(8_500.0)).unwrap();

        assert_eq!(c.ajuste, dop(0.0), "corregir la fecha no toca la cuenta");
    }

    #[test]
    fn el_ajuste_conserva_la_diferencia_entre_lo_facturado_y_lo_recibido() {
        // Si se cobró de menos —8 000 sobre un neto de 8 500—, corregir el
        // total mueve ambas cifras por igual y los 500 de diferencia siguen
        // ahí. Sustituir el recibido por el neto nuevo los borraría.
        let c = corregir(dop(12_000.0), quince_por_ciento(), dop(8_500.0)).unwrap();
        let recibido_anterior = dop(8_000.0);

        let recibido_nuevo = recibido_anterior.sumar(&c.ajuste).unwrap();

        assert_eq!(recibido_nuevo, dop(9_700.0));
        assert_eq!(
            c.neto.restar(&recibido_nuevo).unwrap(),
            dop(500.0),
            "la diferencia sobrevive a la corrección"
        );
    }
}
