//! Casos de uso de los abonos a tarjeta que rodean al registro y a la reversión (que ya viven en
//! `registrar_pago_tarjeta` y `revertir_pago_tarjeta`): el registro con la categoría de la comisión resuelta, el
//! historial y la reversión con su caso de corrección.
//!
//! Lo fijan las pruebas de caracterización `ab1`–`ab5` y `c17`–`c20`, `c63`–`c67`.

use super::registrar_pago_tarjeta::{registrar_pago_tarjeta, DatosPago};
use super::revertir_pago_tarjeta::{revertir_pago_tarjeta, AbonoRevertido};
use super::ErrorAplicacion;
use crate::dominio::correccion::motivo_de_correccion;
use crate::puertos::repositorios::*;

/// Registra un abono. La comisión (si lo paga una cuenta) se asienta como gasto de la categoría de sistema «Otros».
pub fn registrar_abono(
    datos: DatosPago,
    almacen: &mut (impl AlmacenAbonos + CategoriaDeSistema + BusquedaDeCuentas),
) -> Result<(), ErrorAplicacion> {
    let categoria = almacen.categoria_de_sistema()?;
    registrar_pago_tarjeta(datos, categoria, almacen)?;
    Ok(())
}

/// Los abonos de una tarjeta, del más nuevo al más viejo.
pub fn listar_abonos(tarjeta_id: i64, almacen: &impl ConsultaDeAbonos) -> Result<Vec<AbonoLeido>, ErrorAplicacion> {
    Ok(almacen.abonos_de_tarjeta(tarjeta_id)?)
}

/// Lo que dejó una reversión: lo deshecho y el número del caso de corrección.
#[derive(Debug, Clone, PartialEq)]
pub struct ReversionConCaso {
    pub revertido: AbonoRevertido,
    pub caso: String,
}

/// Revierte un abono. Orden (el de siempre): el abono, el motivo, el caso de corrección y, **al final**, la reversión:
/// si algo falla antes no queda un caso huérfano, y si la reversión falla quien abrió la transacción la deshace.
pub fn revertir_abono(
    id: i64,
    motivo: &str,
    almacen: &mut (impl AlmacenAbonos + ConsultaDeAbonos + RegistroDeCorrecciones),
) -> Result<ReversionConCaso, ErrorAplicacion> {
    let abono = almacen
        .resumen_de_abono(id)?
        .ok_or(ErrorAlmacen::NoEncontrado { entidad: "abono", id })?;
    let motivo = motivo_de_correccion(motivo)?;
    let caso = almacen.anotar_caso(&CasoAAnotar {
        tipo: "abono".to_string(),
        referencia_id: id,
        descripcion: format!("Abono del {}", abono.fecha_pago),
        importe: Some(abono.monto_pagado),
        divisa: Some(abono.divisa),
        motivo,
    })?;
    let revertido = revertir_pago_tarjeta(id, almacen)?;
    Ok(ReversionConCaso { revertido, caso })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dominio::dinero::{Dinero, Divisa, TasaCambio};
    use crate::dominio::errores::ErrorDominio;
    use crate::puertos::dobles::AlmacenEnMemoria;

    const MOTIVO: &str = "Corrección de prueba del sistema";

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }
    fn usd(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Usd).unwrap()
    }

    fn almacen() -> AlmacenEnMemoria {
        AlmacenEnMemoria::nuevo()
            .con_categoria(7, "Otros")
            .con_cuenta(10, "Cuenta Abonos", dop(50_000.0))
            .con_cuenta(11, "Efectivo DOP", dop(5_000.0))
            .con_cuenta(12, "Efectivo USD", usd(1_000.0))
            .con_tarjeta(20, dop(10_000.0))
    }

    fn pago(monto: Dinero, cuenta: Option<i64>, tasa: Option<f64>) -> DatosPago {
        DatosPago {
            tarjeta_id: 20,
            fecha: "03/10/2026".into(),
            monto,
            cuenta_ahorro_id: cuenta,
            tasa_cambio: tasa.map(|t| TasaCambio::nueva(t).unwrap()),
        }
    }

    #[test]
    fn registrar_asienta_la_comision_en_la_categoria_otros() {
        let mut a = almacen();
        registrar_abono(pago(dop(5_400.0), Some(10), None), &mut a).unwrap();
        assert_eq!(a.gastos.len(), 1);
        assert_eq!(a.gastos.values().next().unwrap().monto, dop(10.8), "el gasto de la comisión del 0.20 %");
        assert_eq!(a.deuda_de(20), dop(4_600.0));
    }

    #[test]
    fn sin_la_categoria_de_sistema_no_se_registra_nada() {
        let mut a = AlmacenEnMemoria::nuevo().con_cuenta(10, "Cuenta", dop(50_000.0)).con_tarjeta(20, dop(10_000.0));
        assert!(registrar_abono(pago(dop(100.0), Some(10), None), &mut a).is_err());
        assert_eq!(a.deuda_de(20), dop(10_000.0));
        assert!(a.pagos.is_empty());
    }

    #[test]
    fn el_historial_va_del_mas_nuevo_al_mas_viejo_y_una_tarjeta_sin_abonos_da_lista_vacia() {
        let mut a = almacen();
        a.ajustar_deuda(20, usd(500.0)).unwrap();
        registrar_abono(pago(usd(100.0), Some(10), Some(60.0)), &mut a).unwrap();
        registrar_abono(pago(usd(200.0), None, None), &mut a).unwrap();
        let abonos = listar_abonos(20, &a).unwrap();
        assert_eq!(abonos.len(), 2);
        assert_eq!((abonos[0].monto_pagado, abonos[0].cuenta_ahorro_id, abonos[0].cuenta_nombre.as_deref()), (200.0, Some(12), Some("Efectivo USD")));
        assert_eq!((abonos[1].monto_pagado, abonos[1].cuenta_nombre.as_deref(), abonos[1].tasa_cambio), (100.0, Some("Cuenta Abonos"), Some(60.0)));
        assert!(listar_abonos(99, &a).unwrap().is_empty());
    }

    #[test]
    fn revertir_repone_la_deuda_devuelve_el_dinero_y_deja_un_caso() {
        let mut a = almacen();
        registrar_abono(pago(dop(5_400.0), Some(10), None), &mut a).unwrap();
        let id = *a.pagos.keys().next().unwrap();
        a.fechas_de_pago.insert(id, "03/10/2026".into());
        let r = revertir_abono(id, MOTIVO, &mut a).unwrap();
        assert_eq!(r.caso, "CASO-0001");
        assert_eq!(r.revertido.deuda_restituida, dop(5_400.0));
        assert_eq!(r.revertido.devuelto_a_la_cuenta, Some(dop(5_410.8)));
        assert_eq!((a.deuda_de(20), a.saldo(10).unwrap()), (dop(10_000.0), dop(50_000.0)));
        let c = &a.casos[0];
        assert_eq!((c.tipo.as_str(), c.descripcion.as_str(), c.importe, c.divisa.as_deref()), ("abono", "Abono del 03/10/2026", Some(5_400.0), Some("DOP")));
    }

    #[test]
    fn revertir_un_abono_sin_cuenta_devuelve_el_efectivo_a_la_caja() {
        let mut a = almacen();
        registrar_abono(pago(dop(700.0), None, None), &mut a).unwrap();
        assert_eq!(a.saldo(11).unwrap(), dop(4_300.0), "salió de la caja");
        let id = *a.pagos.keys().next().unwrap();
        let r = revertir_abono(id, MOTIVO, &mut a).unwrap();
        assert_eq!((r.revertido.deuda_restituida, r.revertido.devuelto_a_la_cuenta), (dop(700.0), Some(dop(700.0))));
        assert_eq!(a.saldo(11).unwrap(), dop(5_000.0), "y vuelve a la caja");
    }

    #[test]
    fn revertir_inexistente_o_con_motivo_corto_no_abre_caso_ni_toca_nada() {
        let mut a = almacen();
        let e = revertir_abono(404, MOTIVO, &mut a).unwrap_err();
        assert_eq!(e.to_string(), "No se encontró abono con identificador 404.");
        registrar_abono(pago(dop(700.0), None, None), &mut a).unwrap();
        let id = *a.pagos.keys().next().unwrap();
        let e = revertir_abono(id, "corto", &mut a).unwrap_err();
        assert!(matches!(e, ErrorAplicacion::Dominio(ErrorDominio::MotivoInsuficiente { .. })));
        assert!(a.casos.is_empty());
        assert_eq!(a.deuda_de(20), dop(9_300.0));
        assert_eq!(a.pagos.len(), 1);
    }
}
