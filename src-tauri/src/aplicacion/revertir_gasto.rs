//! Caso de uso: revertir un gasto ya registrado.
//!
//! Deshace el efecto sobre los saldos y elimina el registro, que es la
//! conducta vigente de `eliminar_gasto`. Conserva dos asimetrías conocidas:
//!
//! * **H5** — la deuda de tarjeta se recorta en cero, cosa que no ocurre al
//!   registrar, de modo que crear y revertir no son inversas exactas.
//! * El borrado destruye el rastro. Sustituirlo por asientos de compensación
//!   es la Fase 8 y depende de decidir H5.

use super::ErrorAplicacion;
use crate::dominio::gasto::{afectacion_de_gasto, nombre_caja, AfectacionSaldo, MetodoPago};
use crate::puertos::repositorios::*;

pub fn revertir_gasto(
    gasto_id: i64,
    almacen: &mut impl AlmacenGastos,
) -> Result<(), ErrorAplicacion> {
    let gasto = almacen.obtener(gasto_id)?;
    let divisa = gasto.monto.divisa();
    let metodo = MetodoPago::desde_codigo(&gasto.metodo_pago);

    match afectacion_de_gasto(metodo, divisa, gasto.tarjeta_id, gasto.cuenta_ahorro_id) {
        AfectacionSaldo::Ninguna => {}
        AfectacionSaldo::DeudaTarjeta { tarjeta_id } => {
            almacen.reducir_deuda_con_recorte(tarjeta_id, gasto.monto)?;
        }
        AfectacionSaldo::DebitoCuenta { cuenta_id } => {
            // Se devuelve lo que realmente salió: con conversión, el importe
            // en la divisa de la cuenta; sin ella, el monto del gasto.
            let total = gasto.monto_debitado().sumar(&gasto.cargos)?;
            almacen.ajustar_saldo(cuenta_id, total)?;
        }
        AfectacionSaldo::DebitoCaja { divisa } => {
            almacen.ajustar_saldo_de_caja(nombre_caja(divisa), gasto.monto)?;
        }
    }

    almacen.eliminar(gasto_id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::registrar_gasto::{registrar_gasto, DatosGasto};
    use super::*;
    use crate::dominio::dinero::{Dinero, Divisa};
    use crate::puertos::dobles::AlmacenEnMemoria;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }

    fn almacen() -> AlmacenEnMemoria {
        AlmacenEnMemoria::nuevo()
            .con_categoria(1, "Alimentación")
            .con_cuenta(10, "Cuenta Ahorros DOP", dop(100000.0))
            .con_cuenta(11, "Efectivo DOP", dop(5000.0))
            .con_tarjeta(20, dop(100.0))
    }

    fn datos(monto: f64, metodo: MetodoPago) -> DatosGasto {
        DatosGasto {
            fecha: "09/09/2026".into(),
            monto: dop(monto),
            descripcion: "Compra".into(),
            categoria_id: 1,
            metodo: Some(metodo),
            metodo_texto: metodo.codigo().into(),
            es_lbtr: false,
            tarjeta_id: None,
            cuenta_ahorro_id: None,
            tasa_cambio: None,
        }
    }

    #[test]
    fn revertir_una_transferencia_restituye_el_saldo_exacto() {
        let mut a = almacen();
        let mut d = datos(10000.0, MetodoPago::Transferencia);
        d.cuenta_ahorro_id = Some(10);
        let id = registrar_gasto(d, &mut a).unwrap();
        assert_eq!(a.saldo_de(10), dop(89980.0));

        revertir_gasto(id, &mut a).unwrap();

        assert_eq!(a.saldo_de(10), dop(100000.0), "monto y retención devueltos");
        assert_eq!(a.total_gastos(), 0);
    }

    #[test]
    fn revertir_un_gasto_en_efectivo_devuelve_el_importe_a_la_caja() {
        let mut a = almacen();
        let id = registrar_gasto(datos(1200.0, MetodoPago::Efectivo), &mut a).unwrap();
        assert_eq!(a.saldo_de_caja("Efectivo DOP").unwrap(), dop(3800.0));

        revertir_gasto(id, &mut a).unwrap();

        assert_eq!(a.saldo_de_caja("Efectivo DOP").unwrap(), dop(5000.0));
    }

    #[test]
    fn revertir_un_gasto_de_tarjeta_reduce_la_deuda() {
        let mut a = almacen();
        let mut d = datos(50.0, MetodoPago::Tarjeta);
        d.tarjeta_id = Some(20);
        let id = registrar_gasto(d, &mut a).unwrap();
        assert_eq!(a.deuda_de(20), dop(150.0));

        revertir_gasto(id, &mut a).unwrap();

        assert_eq!(a.deuda_de(20), dop(100.0));
    }

    #[test]
    fn h5_si_medio_un_abono_la_reversion_recorta_en_cero_y_pierde_la_diferencia() {
        let mut a = almacen();
        let mut d = datos(150.0, MetodoPago::Tarjeta);
        d.tarjeta_id = Some(20);
        let id = registrar_gasto(d, &mut a).unwrap();
        assert_eq!(a.deuda_de(20), dop(250.0));

        // El titular abona 200 antes de advertir el error de registro.
        a.ajustar_deuda(20, dop(-200.0)).unwrap();
        assert_eq!(a.deuda_de(20), dop(50.0));

        revertir_gasto(id, &mut a).unwrap();

        // Correspondería 50 - 150 = -100. El recorte lo deja en cero y esos
        // 100 desaparecen sin registro.
        assert_eq!(a.deuda_de(20), dop(0.0));
    }

    #[test]
    fn registrar_y_revertir_es_inverso_salvo_cuando_el_recorte_interviene() {
        let mut a = almacen();
        let mut d = datos(50.0, MetodoPago::Tarjeta);
        d.tarjeta_id = Some(20);
        let inicial = a.deuda_de(20);

        let id = registrar_gasto(d, &mut a).unwrap();
        revertir_gasto(id, &mut a).unwrap();

        assert_eq!(a.deuda_de(20), inicial);
    }

    #[test]
    fn revertir_un_gasto_inexistente_es_error() {
        let mut a = almacen();
        assert!(revertir_gasto(404, &mut a).is_err());
    }

    #[test]
    fn revertir_dos_veces_falla_la_segunda() {
        let mut a = almacen();
        let id = registrar_gasto(datos(1200.0, MetodoPago::Efectivo), &mut a).unwrap();
        revertir_gasto(id, &mut a).unwrap();

        assert!(revertir_gasto(id, &mut a).is_err());
        assert_eq!(a.saldo_de_caja("Efectivo DOP").unwrap(), dop(5000.0), "sin doble abono");
    }

    #[test]
    fn revertir_un_gasto_sin_afectacion_solo_lo_elimina() {
        let mut a = almacen();
        // Tarjeta sin identificador: no movió deuda al registrarse (H4).
        let id = registrar_gasto(datos(900.0, MetodoPago::Tarjeta), &mut a).unwrap();

        revertir_gasto(id, &mut a).unwrap();

        assert_eq!(a.deuda_de(20), dop(100.0));
        assert_eq!(a.total_gastos(), 0);
    }

    #[test]
    fn revertir_un_gasto_convertido_devuelve_lo_que_realmente_salio() {
        use crate::dominio::dinero::TasaCambio;

        let mut a = almacen();
        let mut d = datos(100.0, MetodoPago::Transferencia);
        d.monto = Dinero::nuevo(100.0, Divisa::Usd).unwrap();
        d.cuenta_ahorro_id = Some(10);
        d.tasa_cambio = Some(TasaCambio::nueva(60.0).unwrap());

        let id = registrar_gasto(d, &mut a).unwrap();
        assert_eq!(a.saldo_de(10), dop(93988.0), "6 000 convertidos + 12 de retención");

        revertir_gasto(id, &mut a).unwrap();

        // Se devuelven pesos, no dólares: es lo que salió de la cuenta.
        assert_eq!(a.saldo_de(10), dop(100000.0), "restitución exacta");
        assert_eq!(a.total_gastos(), 0);
    }
}
