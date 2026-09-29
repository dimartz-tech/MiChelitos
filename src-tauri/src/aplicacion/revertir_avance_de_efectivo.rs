//! Caso de uso: deshacer un avance de efectivo.
//!
//! Es el inverso exacto de `registrar_avance_de_efectivo`, en el mismo orden:
//! la deuda de la tarjeta baja por el importe **y** el cargo, la cuenta
//! devuelve el importe, el gasto del cargo desaparece y el registro se borra.
//!
//! Se repone **lo guardado**, no lo que hoy se recalcularía con el
//! porcentaje: si la regla o el redondeo cambian, recalcular dejaría un
//! residuo silencioso. Es la lección de la reversión de abonos.
//!
//! **Sin recorte en ningún extremo.** Si el titular ya gastó el dinero del
//! avance, devolverlo deja la cuenta en negativo, y ese es el estado
//! verdadero: la misma resolución de H5 y H10.

use super::ErrorAplicacion;
use crate::dominio::dinero::Dinero;
use crate::puertos::repositorios::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AvanceRevertido {
    /// Lo que baja la deuda de la tarjeta: importe más cargo.
    pub deuda_restituida: Dinero,
    /// Lo que sale de la cuenta: el importe, sin el cargo.
    pub devuelto_por_la_cuenta: Dinero,
}

pub fn revertir_avance_de_efectivo(
    avance_id: i64,
    almacen: &mut impl AlmacenAvances,
) -> Result<AvanceRevertido, ErrorAplicacion> {
    let avance = almacen.obtener_avance(avance_id)?;
    let deuda = avance.monto.sumar(&avance.cargo)?;

    // 1. La deuda baja por lo que el avance la subió.
    almacen.ajustar_deuda(avance.tarjeta_id, deuda.negado())?;

    // 2. La cuenta devuelve el importe. El cargo nunca llegó a ella.
    almacen.ajustar_saldo(avance.cuenta_ahorro_id, avance.monto.negado())?;

    // 3. El gasto del cargo, antes que el avance, para no dejar un gasto
    //    huérfano si algo fallara a medias.
    if let Some(gasto_id) = avance.gasto_cargo_id {
        almacen.eliminar(gasto_id)?;
    }

    // 4. Y el propio avance.
    almacen.eliminar_avance(avance_id)?;

    Ok(AvanceRevertido { deuda_restituida: deuda, devuelto_por_la_cuenta: avance.monto })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aplicacion::registrar_avance_de_efectivo::{
        registrar_avance_de_efectivo, DatosAvance,
    };
    use crate::dominio::avance::CargoDeAvance;
    use crate::dominio::dinero::Divisa;
    use crate::puertos::dobles::AlmacenEnMemoria;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }

    fn almacen() -> AlmacenEnMemoria {
        AlmacenEnMemoria::nuevo()
            .con_categoria(1, "Otros")
            .con_cuenta(10, "Cuenta Ahorros DOP", dop(1_000.0))
            .con_tarjeta(20, dop(5_000.0))
    }

    fn registrar(a: &mut AlmacenEnMemoria, cargo: CargoDeAvance) -> i64 {
        registrar_avance_de_efectivo(
            DatosAvance {
                tarjeta_id: 20,
                cuenta_ahorro_id: 10,
                fecha: "01/10/2026".into(),
                monto: dop(10_000.0),
                cargo,
                nota: None,
            },
            1,
            a,
        )
        .unwrap()
        .id
    }

    #[test]
    fn registrar_y_revertir_dejan_tarjeta_cuenta_y_gastos_como_estaban() {
        for cargo in [
            CargoDeAvance::porcentual(6.25).unwrap(),
            CargoDeAvance::fijo(dop(300.0)).unwrap(),
            CargoDeAvance::Exonerado,
        ] {
            let mut a = almacen();
            let id = registrar(&mut a, cargo);
            revertir_avance_de_efectivo(id, &mut a).unwrap();

            assert_eq!(a.deuda_de(20), dop(5_000.0), "deuda con {:?}", cargo.codigo());
            assert_eq!(a.saldo_de(10), dop(1_000.0), "cuenta con {:?}", cargo.codigo());
            assert!(a.gastos.is_empty(), "gasto huérfano con {:?}", cargo.codigo());
            assert!(a.avances.is_empty());
        }
    }

    #[test]
    fn el_resumen_dice_lo_que_deshizo() {
        let mut a = almacen();
        let id = registrar(&mut a, CargoDeAvance::porcentual(8.0).unwrap());
        let r = revertir_avance_de_efectivo(id, &mut a).unwrap();
        assert_eq!(r.deuda_restituida, dop(10_800.0));
        assert_eq!(r.devuelto_por_la_cuenta, dop(10_000.0));
    }

    #[test]
    fn si_la_cuenta_ya_gasto_el_dinero_queda_en_negativo_sin_recorte() {
        let mut a = almacen();
        let id = registrar(&mut a, CargoDeAvance::Exonerado);
        // El titular gasta 9 000 de los 11 000 que hay.
        a.cuentas.get_mut(&10).unwrap().saldo = dop(2_000.0);

        revertir_avance_de_efectivo(id, &mut a).unwrap();

        assert_eq!(a.saldo_de(10), dop(-8_000.0), "el estado verdadero, sin recorte a cero");
    }

    #[test]
    fn se_devuelve_lo_guardado_y_no_lo_que_se_recalcularia() {
        // Un avance cuyo cargo guardado no coincide con lo que hoy saldría:
        // la reversión debe usar el guardado.
        let mut a = almacen();
        let id = registrar(&mut a, CargoDeAvance::porcentual(6.25).unwrap());
        a.avances.get_mut(&id).unwrap().cargo = dop(999.99);

        let r = revertir_avance_de_efectivo(id, &mut a).unwrap();

        assert_eq!(r.deuda_restituida, dop(10_999.99));
    }

    #[test]
    fn revertir_un_avance_inexistente_es_error_y_no_mueve_nada() {
        let mut a = almacen();
        assert!(revertir_avance_de_efectivo(999, &mut a).is_err());
        assert_eq!(a.deuda_de(20), dop(5_000.0));
    }

    #[test]
    fn revertir_dos_veces_falla_la_segunda_sin_duplicar_la_devolucion() {
        let mut a = almacen();
        let id = registrar(&mut a, CargoDeAvance::porcentual(8.0).unwrap());
        revertir_avance_de_efectivo(id, &mut a).unwrap();
        assert!(revertir_avance_de_efectivo(id, &mut a).is_err());
        assert_eq!(a.deuda_de(20), dop(5_000.0));
        assert_eq!(a.saldo_de(10), dop(1_000.0));
    }
}
