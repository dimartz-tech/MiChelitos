//! Caso de uso: pedir un avance de efectivo con una tarjeta.
//!
//! Mueve tres cosas a la vez: la deuda de la tarjeta sube por el importe y
//! su cargo, la cuenta de ahorro recibe el importe sin el cargo, y el cargo
//! queda como gasto para que cuente en lo que cuesta financiarse.
//!
//! Como en el resto de casos de uso, **primero se resuelve todo lo que puede
//! fallar por una regla y solo después se toca un saldo**.
//!
//! ## Decisiones
//!
//! * **El cargo lo paga la tarjeta, no la cuenta.** La cuenta recibe el
//!   importe íntegro; el cargo se suma a la deuda. Es como lo asienta el
//!   emisor.
//! * **Misma divisa.** Un avance se acredita en la divisa en que se carga.
//!   Una cuenta en otra divisa se rechaza en vez de convertir: convertir sería
//!   inventar una tasa que nadie ha declarado.
//! * **El cargo es un gasto con la tarjeta**, que una sola operación sube a
//!   la deuda junto con el importe. Borrarlo por separado descuadraría la
//!   tarjeta, y por eso el comando lo impide y obliga a revertir el avance
//!   entero.

use super::ErrorAplicacion;
use crate::dominio::avance::{Avance, CargoDeAvance};
use crate::dominio::conversion::EstadoConversion;
use crate::dominio::dinero::Dinero;
use crate::dominio::errores::ErrorDominio;
use crate::puertos::repositorios::*;

#[derive(Debug, Clone)]
pub struct DatosAvance {
    pub tarjeta_id: i64,
    pub cuenta_ahorro_id: i64,
    pub fecha: String,
    pub monto: Dinero,
    pub cargo: CargoDeAvance,
    /// Por qué se exoneró, o cualquier aclaración. Libre.
    pub nota: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AvanceRegistrado {
    pub id: i64,
    pub cargo: Dinero,
    /// Lo que subió la deuda de la tarjeta: importe más cargo.
    pub a_la_tarjeta: Dinero,
}

pub fn registrar_avance_de_efectivo(
    datos: DatosAvance,
    categoria_cargo_id: i64,
    almacen: &mut impl AlmacenAvances,
) -> Result<AvanceRegistrado, ErrorAplicacion> {
    // 1. Todo lo que puede rechazarse, antes de mover nada.
    let divisa_cuenta = almacen.divisa(datos.cuenta_ahorro_id)?;
    if divisa_cuenta != datos.monto.divisa() {
        return Err(ErrorDominio::AvanceEnOtraDivisa {
            cuenta: divisa_cuenta,
            avance: datos.monto.divisa(),
        }
        .into());
    }
    let avance = Avance::calcular(datos.monto, datos.cargo)?;
    let a_la_tarjeta = avance.a_la_tarjeta()?;

    // 2. La deuda sube por el importe y el cargo.
    almacen.ajustar_deuda(datos.tarjeta_id, a_la_tarjeta)?;

    // 3. La cuenta recibe el importe, sin el cargo.
    almacen.ajustar_saldo(datos.cuenta_ahorro_id, avance.monto)?;

    // 4. El cargo, si lo hubo, es un gasto de la tarjeta.
    let gasto_cargo_id = if avance.cargo.es_cero() {
        None
    } else {
        Some(almacen.insertar(&GastoAPersistir {
            fecha: datos.fecha.clone(),
            monto: avance.cargo,
            descripcion: "Cargo por avance de efectivo".to_string(),
            categoria_id: categoria_cargo_id,
            metodo_pago: "tarjeta".to_string(),
            cargos: Dinero::cero(avance.cargo.divisa()),
            tarjeta_id: Some(datos.tarjeta_id),
            cuenta_ahorro_id: None,
            estado_conversion: EstadoConversion::NoAplica,
        })?)
    };

    // 5. El avance se guarda entero, ya enlazado a todo lo que movió.
    let id = almacen.insertar_avance(&AvanceAPersistir {
        tarjeta_id: datos.tarjeta_id,
        cuenta_ahorro_id: datos.cuenta_ahorro_id,
        fecha: datos.fecha,
        monto: avance.monto,
        tipo_cargo: datos.cargo.codigo(),
        tasa: datos.cargo.tasa(),
        cargo: avance.cargo,
        gasto_cargo_id,
        nota: datos.nota,
    })?;

    Ok(AvanceRegistrado { id, cargo: avance.cargo, a_la_tarjeta })
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
            .con_cuenta(10, "Cuenta Ahorros DOP", dop(1_000.0))
            .con_cuenta(11, "Cuenta Ahorros USD", usd(100.0))
            .con_tarjeta(20, dop(5_000.0))
    }

    fn datos(monto: Dinero, cargo: CargoDeAvance) -> DatosAvance {
        DatosAvance {
            tarjeta_id: 20,
            cuenta_ahorro_id: 10,
            fecha: "01/10/2026".into(),
            monto,
            cargo,
            nota: None,
        }
    }

    #[test]
    fn la_tarjeta_sube_por_importe_y_cargo_y_la_cuenta_recibe_solo_el_importe() {
        let mut a = almacen();
        let r = registrar_avance_de_efectivo(
            datos(dop(10_000.0), CargoDeAvance::porcentual(6.25).unwrap()), 1, &mut a,
        )
        .unwrap();

        assert_eq!(r.cargo, dop(625.0));
        assert_eq!(r.a_la_tarjeta, dop(10_625.0));
        assert_eq!(a.deuda_de(20), dop(15_625.0), "5 000 previos + 10 000 + 625");
        assert_eq!(a.saldo_de(10), dop(11_000.0), "1 000 + 10 000, sin el cargo");
    }

    #[test]
    fn el_cargo_queda_como_gasto_de_la_tarjeta() {
        let mut a = almacen();
        let r = registrar_avance_de_efectivo(
            datos(dop(10_000.0), CargoDeAvance::porcentual(8.0).unwrap()), 1, &mut a,
        )
        .unwrap();

        let avance = a.obtener_avance(r.id).unwrap();
        let gasto = a.obtener(avance.gasto_cargo_id.expect("hay gasto")).unwrap();
        assert_eq!(gasto.monto, dop(800.0));
        assert_eq!(gasto.tarjeta_id, Some(20));
        assert_eq!(gasto.metodo_pago, "tarjeta");
    }

    #[test]
    fn un_cargo_fijo_se_suma_a_la_deuda_tal_cual() {
        let mut a = almacen();
        let r = registrar_avance_de_efectivo(
            datos(dop(4_000.0), CargoDeAvance::fijo(dop(300.0)).unwrap()), 1, &mut a,
        )
        .unwrap();
        assert_eq!(r.cargo, dop(300.0));
        assert_eq!(a.deuda_de(20), dop(9_300.0));
        assert_eq!(a.saldo_de(10), dop(5_000.0));
    }

    #[test]
    fn un_avance_exonerado_no_genera_gasto_y_solo_sube_el_importe() {
        let mut a = almacen();
        let r = registrar_avance_de_efectivo(
            datos(dop(2_000.0), CargoDeAvance::Exonerado), 1, &mut a,
        )
        .unwrap();

        assert!(r.cargo.es_cero());
        assert_eq!(a.deuda_de(20), dop(7_000.0));
        assert_eq!(a.saldo_de(10), dop(3_000.0));
        assert_eq!(a.obtener_avance(r.id).unwrap().gasto_cargo_id, None, "sin cargo no hay gasto");
        assert!(a.gastos.is_empty());
    }

    #[test]
    fn una_cuenta_en_otra_divisa_se_rechaza_sin_mover_nada() {
        let mut a = almacen();
        let mut d = datos(dop(1_000.0), CargoDeAvance::Exonerado);
        d.cuenta_ahorro_id = 11; // cuenta en dólares
        let r = registrar_avance_de_efectivo(d, 1, &mut a);

        assert!(matches!(
            r,
            Err(ErrorAplicacion::Dominio(ErrorDominio::AvanceEnOtraDivisa { .. }))
        ));
        assert_eq!(a.deuda_de(20), dop(5_000.0), "la deuda no se movió");
        assert_eq!(a.saldo_de(11), usd(100.0));
    }

    #[test]
    fn un_importe_no_positivo_se_rechaza_sin_mover_nada() {
        let mut a = almacen();
        let r = registrar_avance_de_efectivo(
            datos(dop(0.0), CargoDeAvance::Exonerado), 1, &mut a,
        );
        assert!(r.is_err());
        assert_eq!(a.deuda_de(20), dop(5_000.0));
        assert_eq!(a.saldo_de(10), dop(1_000.0));
    }

    #[test]
    fn una_cuenta_inexistente_falla_antes_de_tocar_la_deuda() {
        let mut a = almacen();
        let mut d = datos(dop(1_000.0), CargoDeAvance::Exonerado);
        d.cuenta_ahorro_id = 999;
        assert!(registrar_avance_de_efectivo(d, 1, &mut a).is_err());
        assert_eq!(a.deuda_de(20), dop(5_000.0));
    }

    #[test]
    fn el_avance_guarda_la_forma_del_cargo() {
        let mut a = almacen();
        let r = registrar_avance_de_efectivo(
            datos(dop(1_000.0), CargoDeAvance::porcentual(6.25).unwrap()), 1, &mut a,
        )
        .unwrap();
        let guardado = a.obtener_avance(r.id).unwrap();
        assert_eq!(guardado.cargo, dop(62.5));
        assert_eq!(guardado.monto, dop(1_000.0));
    }
}
