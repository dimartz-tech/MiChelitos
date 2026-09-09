//! Caso de uso: registrar un gasto.
//!
//! Reproduce el orden de `crear_gasto`: primero calcula los cargos, después
//! mueve el saldo y por último inserta. Esa secuencia importa — si la
//! inserción falla, el saldo ya se movió, y es la transacción del adaptador
//! la que lo deshace.

use super::ErrorAplicacion;
use crate::dominio::cargos::cargos_de_transferencia;
use crate::dominio::dinero::Dinero;
use crate::dominio::gasto::{afectacion_de_gasto, nombre_caja, AfectacionSaldo, MetodoPago};
use crate::puertos::repositorios::*;

#[derive(Debug, Clone)]
pub struct DatosGasto {
    pub fecha: String,
    pub monto: Dinero,
    pub descripcion: String,
    pub categoria_id: i64,
    /// `None` cuando el texto recibido no corresponde a ningún método
    /// conocido, que hoy se traduce en no mover ningún saldo.
    pub metodo: Option<MetodoPago>,
    pub metodo_texto: String,
    pub es_lbtr: bool,
    pub tarjeta_id: Option<i64>,
    pub cuenta_ahorro_id: Option<i64>,
}

pub fn registrar_gasto(
    datos: DatosGasto,
    almacen: &mut impl AlmacenGastos,
) -> Result<i64, ErrorAplicacion> {
    let divisa = datos.monto.divisa();

    let cargos = if datos.metodo.is_some_and(|m| m.devenga_cargos()) {
        let categoria = almacen.nombre(datos.categoria_id)?.unwrap_or_default();
        cargos_de_transferencia(datos.monto, &categoria, &datos.descripcion, datos.es_lbtr)?
            .total()?
    } else {
        Dinero::cero(divisa)
    };

    match afectacion_de_gasto(datos.metodo, divisa, datos.tarjeta_id, datos.cuenta_ahorro_id) {
        AfectacionSaldo::Ninguna => {}
        AfectacionSaldo::DeudaTarjeta { tarjeta_id } => {
            almacen.ajustar_deuda(tarjeta_id, datos.monto)?;
        }
        AfectacionSaldo::DebitoCuenta { cuenta_id } => {
            let total = datos.monto.sumar(&cargos)?;
            almacen.ajustar_saldo(cuenta_id, negativo(total)?)?;
        }
        AfectacionSaldo::DebitoCaja { divisa } => {
            almacen.ajustar_saldo_de_caja(nombre_caja(divisa), negativo(datos.monto)?)?;
        }
    }

    let id = almacen.insertar(&GastoAPersistir {
        fecha: datos.fecha,
        monto: datos.monto,
        descripcion: datos.descripcion,
        categoria_id: datos.categoria_id,
        metodo_pago: datos.metodo_texto,
        cargos,
        tarjeta_id: datos.tarjeta_id,
        cuenta_ahorro_id: datos.cuenta_ahorro_id,
    })?;

    Ok(id)
}

fn negativo(importe: Dinero) -> Result<Dinero, ErrorAplicacion> {
    Ok(Dinero::cero(importe.divisa()).restar(&importe)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dominio::dinero::Divisa;
    use crate::puertos::dobles::AlmacenEnMemoria;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }

    fn almacen() -> AlmacenEnMemoria {
        AlmacenEnMemoria::nuevo()
            .con_categoria(1, "Alimentación")
            .con_categoria(2, "Impuestos")
            .con_cuenta(10, "Cuenta Ahorros DOP", dop(100000.0))
            .con_cuenta(11, "Efectivo DOP", dop(5000.0))
            .con_tarjeta(20, dop(500.0))
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
        }
    }

    // --- Transferencias ---

    #[test]
    fn una_transferencia_debita_el_monto_mas_la_retencion() {
        let mut a = almacen();
        let mut d = datos(10000.0, MetodoPago::Transferencia);
        d.cuenta_ahorro_id = Some(10);

        let id = registrar_gasto(d, &mut a).unwrap();

        assert_eq!(a.saldo_de(10), dop(89980.0));
        assert_eq!(a.obtener(id).unwrap().cargos, dop(20.0));
    }

    #[test]
    fn el_pago_de_tss_no_retiene_y_debita_solo_el_monto() {
        let mut a = almacen();
        let mut d = datos(10000.0, MetodoPago::Transferencia);
        d.categoria_id = 2;
        d.descripcion = "Pago TSS".into();
        d.cuenta_ahorro_id = Some(10);

        registrar_gasto(d, &mut a).unwrap();

        assert_eq!(a.saldo_de(10), dop(90000.0));
    }

    #[test]
    fn el_lbtr_anade_su_comision_aunque_el_pago_este_exento() {
        let mut a = almacen();
        let mut d = datos(10000.0, MetodoPago::Transferencia);
        d.categoria_id = 2;
        d.descripcion = "Pago TSS".into();
        d.es_lbtr = true;
        d.cuenta_ahorro_id = Some(10);

        registrar_gasto(d, &mut a).unwrap();

        assert_eq!(a.saldo_de(10), dop(89900.0));
    }

    #[test]
    fn una_categoria_inexistente_no_exime_y_retiene() {
        let mut a = almacen();
        let mut d = datos(10000.0, MetodoPago::Transferencia);
        d.categoria_id = 999;
        d.descripcion = "Pago TSS".into();
        d.cuenta_ahorro_id = Some(10);

        registrar_gasto(d, &mut a).unwrap();

        assert_eq!(a.saldo_de(10), dop(89980.0), "sin categoría no hay exención");
    }

    #[test]
    fn una_transferencia_sin_cuenta_calcula_la_retencion_pero_no_debita() {
        let mut a = almacen();
        let d = datos(10000.0, MetodoPago::Transferencia);

        let id = registrar_gasto(d, &mut a).unwrap();

        assert_eq!(a.obtener(id).unwrap().cargos, dop(20.0));
        assert_eq!(a.saldo_de(10), dop(100000.0));
    }

    // --- Tarjeta y efectivo ---

    #[test]
    fn un_gasto_con_tarjeta_incrementa_la_deuda_y_no_devenga_cargos() {
        let mut a = almacen();
        let mut d = datos(900.0, MetodoPago::Tarjeta);
        d.tarjeta_id = Some(20);

        let id = registrar_gasto(d, &mut a).unwrap();

        assert_eq!(a.deuda_de(20), dop(1400.0));
        assert_eq!(a.obtener(id).unwrap().cargos, dop(0.0));
    }

    #[test]
    fn h4_una_tarjeta_sin_identificador_registra_el_gasto_sin_mover_deuda() {
        let mut a = almacen();
        let d = datos(900.0, MetodoPago::Tarjeta);

        registrar_gasto(d, &mut a).unwrap();

        assert_eq!(a.deuda_de(20), dop(500.0));
        assert_eq!(a.total_gastos(), 1);
    }

    #[test]
    fn un_gasto_en_efectivo_descuenta_de_la_caja_de_su_divisa() {
        let mut a = almacen();
        let d = datos(1200.0, MetodoPago::Efectivo);

        registrar_gasto(d, &mut a).unwrap();

        assert_eq!(a.saldo_de_caja("Efectivo DOP").unwrap(), dop(3800.0));
    }

    #[test]
    fn h3_si_la_caja_no_existe_el_gasto_se_registra_sin_mover_saldo() {
        let mut a = AlmacenEnMemoria::nuevo()
            .con_categoria(1, "Alimentación")
            .con_cuenta(11, "Caja Chica DOP", dop(5000.0));
        let d = datos(1200.0, MetodoPago::Efectivo);

        registrar_gasto(d, &mut a).unwrap();

        assert_eq!(a.saldo_de(11), dop(5000.0));
        assert_eq!(a.total_gastos(), 1);
    }

    #[test]
    fn un_metodo_no_reconocido_no_mueve_ningun_saldo() {
        let mut a = almacen();
        let mut d = datos(1000.0, MetodoPago::Efectivo);
        d.metodo = None;
        d.metodo_texto = "cheque".into();

        registrar_gasto(d, &mut a).unwrap();

        assert_eq!(a.saldo_de_caja("Efectivo DOP").unwrap(), dop(5000.0));
        assert_eq!(a.saldo_de(10), dop(100000.0));
    }

    // --- Fallos ---

    #[test]
    fn si_la_insercion_falla_el_caso_de_uso_propaga_el_error() {
        // El saldo queda movido a propósito: deshacerlo es competencia de la
        // transacción del adaptador, no del caso de uso. Es lo que fija C14.
        let mut a = almacen();
        a.falla_al_insertar = true;
        let mut d = datos(10000.0, MetodoPago::Transferencia);
        d.cuenta_ahorro_id = Some(10);

        assert!(registrar_gasto(d, &mut a).is_err());
        assert_eq!(a.total_gastos(), 0);
    }

    #[test]
    fn debitar_una_cuenta_inexistente_falla_antes_de_insertar() {
        let mut a = almacen();
        let mut d = datos(10000.0, MetodoPago::Transferencia);
        d.cuenta_ahorro_id = Some(999);

        assert!(registrar_gasto(d, &mut a).is_err());
        assert_eq!(a.total_gastos(), 0, "no se insertó nada");
    }

    #[test]
    fn debitar_una_cuenta_de_otra_divisa_es_error() {
        let mut a = AlmacenEnMemoria::nuevo()
            .con_categoria(1, "Alimentación")
            .con_cuenta(10, "Cuenta USD", Dinero::nuevo(5000.0, Divisa::Usd).unwrap());
        let mut d = datos(1000.0, MetodoPago::Transferencia);
        d.cuenta_ahorro_id = Some(10);

        // H2: hoy el SQL no compara divisas y resta igual. Con el tipo Dinero
        // la operación deja de ser representable.
        assert!(registrar_gasto(d, &mut a).is_err());
    }
}
