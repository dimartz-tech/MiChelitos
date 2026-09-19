//! Caso de uso: deshacer un abono a tarjeta.
//!
//! Un abono mueve hasta cuatro cosas a la vez: reduce la deuda de la tarjeta,
//! debita la cuenta que lo paga, anota la comisión del 0.20 % como gasto y
//! deja el propio registro del abono. Hasta ahora no había forma de deshacerlo
//! porque el registro no guardaba tres de esas cuatro: ni de qué cuenta salió,
//! ni a qué tasa se convirtió, ni qué gasto recogió la comisión.
//!
//! Revertir es aplicar el inverso de cada efecto, en el mismo orden en que se
//! aplicaron. **No hay recorte en ninguno de los dos extremos**: si devolver la
//! deuda deja la tarjeta por encima de su límite, o si restituir el dinero a la
//! cuenta la deja donde estaba antes, eso es el estado verdadero. Es la misma
//! resolución que cerró H5 y H10.
//!
//! ## Se devuelve lo guardado, no lo recalculado
//!
//! La primera versión de este caso de uso **recalculaba** lo que había salido
//! de la cuenta, y lo argumentaba: el cálculo es el mismo que al registrar, de
//! modo que no hacía falta guardarlo. Era el argumento equivocado.
//!
//! Deshacer una operación es reponer **lo que ocurrió**, no lo que hoy
//! creemos que debió ocurrir. Si la regla de redondeo cambia —como acaba de
//! cambiar—, o si un importe se corrigió a mano, recalcular devuelve una cifra
//! distinta de la que salió y la reversión deja un residuo silencioso.
//!
//! No es hipotético: al revertir un abono real cuya comisión se había
//! guardado sin redondear, hubo que partir del valor **almacenado** porque
//! recalcular habría devuelto otro. La regla que este módulo defendía no
//! sirvió en el único caso en que se puso a prueba.

use super::ErrorAplicacion;
use crate::dominio::dinero::Dinero;
use crate::puertos::repositorios::*;

/// Lo que la reversión deshizo, para poder contarlo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbonoRevertido {
    /// Importe devuelto a la deuda de la tarjeta.
    pub deuda_restituida: Dinero,
    /// Lo que vuelve a la cuenta: el importe debitado más su comisión.
    /// `None` cuando el abono se registró sin cuenta.
    pub devuelto_a_la_cuenta: Option<Dinero>,
}

pub fn revertir_pago_tarjeta(
    pago_id: i64,
    almacen: &mut impl AlmacenAbonos,
) -> Result<AbonoRevertido, ErrorAplicacion> {
    let pago = almacen.obtener_pago(pago_id)?;

    // 1. La deuda vuelve a subir. Un abono la redujo; deshacerlo la repone.
    almacen.ajustar_deuda(pago.tarjeta_id, pago.monto)?;

    // 2. El dinero vuelve a la cuenta, si alguna lo pagó. El importe sale de
    //    lo que el abono guardó, no de volver a calcularlo.
    let devuelto = match (pago.cuenta_ahorro_id, pago.monto_debitado) {
        (Some(cuenta_id), Some(debitado)) => {
            let comision = pago.comision.unwrap_or_else(|| Dinero::cero(debitado.divisa()));
            let total = debitado.sumar(&comision)?;
            almacen.ajustar_saldo(cuenta_id, total)?;
            Some(total)
        }
        // Un abono con cuenta pero sin débito guardado no puede revertirse a
        // ciegas: adivinar el importe es justo lo que este módulo dejó de
        // hacer. Se dice qué falta en lugar de devolver una cifra inventada.
        (Some(_), None) => {
            return Err(ErrorAplicacion::Almacen(ErrorAlmacen::Fallo(format!(
                "el abono {} no guarda cuánto debitó de la cuenta; no puede revertirse sin inventar el importe",
                pago_id
            ))))
        }
        (None, _) => None,
    };

    // 3. La comisión deja de existir como gasto. Se borra antes que el abono
    //    para que, si algo fallara, no quede un gasto huérfano apuntando a un
    //    abono que ya no está.
    if let Some(gasto_id) = pago.gasto_comision_id {
        almacen.eliminar(gasto_id)?;
    }

    // 4. Y por último el abono.
    almacen.eliminar_pago(pago_id)?;

    Ok(AbonoRevertido { deuda_restituida: pago.monto, devuelto_a_la_cuenta: devuelto })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dominio::dinero::Divisa;
    use crate::puertos::dobles::AlmacenEnMemoria;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }

    fn usd(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Usd).unwrap()
    }

    fn almacen() -> AlmacenEnMemoria {
        AlmacenEnMemoria::nuevo()
            .con_categoria(1, "Otros")
            .con_cuenta(10, "Cuenta Ahorros DOP", dop(100_000.0))
            .con_tarjeta(20, dop(5_000.0))
    }

    #[test]
    fn revertir_un_abono_en_pesos_repone_la_deuda_y_devuelve_el_dinero() {
        let mut a = almacen();
        // Se abonaron 3 000: la deuda bajó a 2 000 y de la cuenta salieron
        // 3 000 más 6 de comisión.
        a.ajustar_deuda(20, dop(-3_000.0)).unwrap();
        a.ajustar_saldo(10, dop(-3_006.0)).unwrap();
        let pago = a.con_pago(20, dop(3_000.0), Some(10), None, None);

        let r = revertir_pago_tarjeta(pago, &mut a).unwrap();

        assert_eq!(a.deuda_de(20), dop(5_000.0), "la deuda vuelve donde estaba");
        assert_eq!(a.saldo_de(10), dop(100_000.0), "y el dinero a la cuenta");
        assert_eq!(r.deuda_restituida, dop(3_000.0));
        assert_eq!(r.devuelto_a_la_cuenta, Some(dop(3_006.0)));
    }

    #[test]
    fn registrar_y_revertir_un_abono_deja_todo_como_estaba() {
        // La propiedad que define una reversión, comprobada de punta a punta.
        let mut a = almacen();
        let deuda = a.deuda_de(20);
        let saldo = a.saldo_de(10);

        a.ajustar_deuda(20, dop(-3_000.0)).unwrap();
        a.ajustar_saldo(10, dop(-3_006.0)).unwrap();
        let pago = a.con_pago(20, dop(3_000.0), Some(10), None, None);
        revertir_pago_tarjeta(pago, &mut a).unwrap();

        assert_eq!(a.deuda_de(20), deuda);
        assert_eq!(a.saldo_de(10), saldo);
    }

    #[test]
    fn un_abono_en_divisa_devuelve_a_la_cuenta_los_pesos_que_salieron() {
        let mut a = almacen();
        // 100 USD a tasa 60 son 6 000 pesos, más 12 de comisión. El abono
        // guarda el débito **en la divisa de la cuenta**, que es lo que esa
        // cuenta tiene que recibir de vuelta.
        a.ajustar_deuda(20, usd(-100.0)).unwrap();
        a.ajustar_saldo(10, dop(-6_012.0)).unwrap();
        let pago = a.con_pago_detallado(
            20, usd(100.0), Some(10), Some(60.0), Some(dop(6_000.0)), Some(dop(12.0)), None,
        );

        let r = revertir_pago_tarjeta(pago, &mut a).unwrap();

        assert_eq!(a.deuda_en(20, Divisa::Usd), usd(0.0), "la deuda vuelve en dólares");
        assert_eq!(a.saldo_de(10), dop(100_000.0), "y a la cuenta vuelven pesos");
        assert_eq!(r.devuelto_a_la_cuenta, Some(dop(6_012.0)));
    }

    #[test]
    fn se_devuelve_lo_que_salio_aunque_hoy_se_calcularia_distinto() {
        // **La prueba que justifica el tramo 2.** El abono guarda un débito
        // que no coincide con lo que se obtendría recalculándolo: 100 USD a
        // tasa 59.9 darían 5 990.00, pero de la cuenta salieron 5 989.50.
        //
        // Ocurre de verdad cuando la regla de redondeo cambia o cuando un
        // importe se corrigió a mano. Recalcular devolvería 5 990.00 y dejaría
        // medio peso de residuo que nadie vería.
        let mut a = almacen();
        a.ajustar_deuda(20, usd(-100.0)).unwrap();
        a.ajustar_saldo(10, dop(-6_001.48)).unwrap();
        let pago = a.con_pago_detallado(
            20, usd(100.0), Some(10), Some(59.9),
            Some(dop(5_989.50)), Some(dop(11.98)), None,
        );

        let r = revertir_pago_tarjeta(pago, &mut a).unwrap();

        assert_eq!(r.devuelto_a_la_cuenta, Some(dop(6_001.48)), "lo guardado, no lo recalculado");
        assert_eq!(a.saldo_de(10), dop(100_000.0), "la cuenta vuelve exacta");
    }

    #[test]
    fn un_abono_con_cuenta_pero_sin_debito_guardado_se_niega_a_adivinar() {
        // Antes este caso se resolvía recalculando. Ahora se dice qué falta,
        // porque devolver una cifra inventada a una cuenta real es peor que
        // no devolver nada.
        let mut a = almacen();
        a.ajustar_deuda(20, dop(-3_000.0)).unwrap();
        let pago = a.con_pago_detallado(20, dop(3_000.0), Some(10), None, None, None, None);

        let r = revertir_pago_tarjeta(pago, &mut a);

        assert!(r.is_err(), "no se revierte a ciegas");
        assert!(
            format!("{}", r.unwrap_err()).contains("no guarda cuánto debitó"),
            "el error dice qué falta"
        );
    }

    #[test]
    fn un_abono_sin_cuenta_solo_repone_la_deuda() {
        let mut a = almacen();
        a.ajustar_deuda(20, dop(-3_000.0)).unwrap();
        let pago = a.con_pago(20, dop(3_000.0), None, None, None);

        let r = revertir_pago_tarjeta(pago, &mut a).unwrap();

        assert_eq!(a.deuda_de(20), dop(5_000.0));
        assert_eq!(a.saldo_de(10), dop(100_000.0), "ninguna cuenta se toca");
        assert_eq!(r.devuelto_a_la_cuenta, None);
    }

    #[test]
    fn revertir_borra_tambien_el_gasto_de_la_comision() {
        let mut a = almacen();
        let comision = a.con_gasto_suelto(dop(6.0));
        assert_eq!(a.total_gastos(), 1);
        let pago = a.con_pago(20, dop(3_000.0), Some(10), None, Some(comision));

        revertir_pago_tarjeta(pago, &mut a).unwrap();

        assert_eq!(a.total_gastos(), 0, "la comisión deja de existir");
    }

    #[test]
    fn revertir_un_abono_que_excedia_la_deuda_deja_la_tarjeta_donde_estaba() {
        // El caso de H5 visto desde el otro lado: si el abono dejó saldo a
        // favor, deshacerlo devuelve exactamente ese saldo a favor a cero.
        let mut a = AlmacenEnMemoria::nuevo()
            .con_categoria(1, "Otros")
            .con_cuenta(10, "Cuenta Ahorros DOP", dop(100_000.0))
            .con_tarjeta(20, dop(0.0));
        a.ajustar_deuda(20, dop(-500.0)).unwrap();
        assert_eq!(a.deuda_de(20), dop(-500.0), "quedó saldo a favor");
        let pago = a.con_pago(20, dop(500.0), None, None, None);

        revertir_pago_tarjeta(pago, &mut a).unwrap();

        assert_eq!(a.deuda_de(20), dop(0.0), "sin recorte en ningún extremo");
    }

    #[test]
    fn revertir_un_abono_inexistente_es_error() {
        let mut a = almacen();
        assert!(revertir_pago_tarjeta(404, &mut a).is_err());
    }

    #[test]
    fn revertir_dos_veces_falla_la_segunda_y_no_duplica_la_devolucion() {
        let mut a = almacen();
        a.ajustar_deuda(20, dop(-3_000.0)).unwrap();
        a.ajustar_saldo(10, dop(-3_006.0)).unwrap();
        let pago = a.con_pago(20, dop(3_000.0), Some(10), None, None);

        revertir_pago_tarjeta(pago, &mut a).unwrap();
        assert!(revertir_pago_tarjeta(pago, &mut a).is_err());

        assert_eq!(a.saldo_de(10), dop(100_000.0), "sin doble devolución");
        assert_eq!(a.deuda_de(20), dop(5_000.0));
    }

    #[test]
    fn una_tasa_de_uno_es_la_ausencia_de_conversion() {
        // Así llegan los abonos históricos en pesos tras la migración 5.
        let mut a = almacen();
        a.ajustar_saldo(10, dop(-3_006.0)).unwrap();
        let pago = a.con_pago(20, dop(3_000.0), Some(10), Some(1.0), None);

        let r = revertir_pago_tarjeta(pago, &mut a).unwrap();

        assert_eq!(r.devuelto_a_la_cuenta, Some(dop(3_006.0)), "no se multiplica");
    }
}
