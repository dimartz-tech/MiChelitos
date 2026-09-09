//! Casos de uso de bonificaciones: registrarlas y revertirlas.
//!
//! Una bonificación reduce la deuda de la tarjeta en su propia divisa. No
//! toca el gasto que la originó: si se vincula a uno es solo para poder
//! contrastar después lo abonado con la tabla de beneficios del emisor.

use super::ErrorAplicacion;
use crate::dominio::bonificacion::Bonificacion;
use crate::puertos::repositorios::*;

pub struct DatosBonificacion {
    pub fecha: String,
    pub tarjeta_id: i64,
    pub bonificacion: Bonificacion,
    /// Consumo que la generó, si se conoce. Los estados de cuenta no lo
    /// indican, así que lo habitual es no tenerlo.
    pub gasto_id: Option<i64>,
}

pub fn registrar_bonificacion(
    datos: DatosBonificacion,
    almacen: &mut impl AlmacenBonificaciones,
) -> Result<i64, ErrorAplicacion> {
    // El saldo se mueve antes de insertar, igual que en los gastos: si la
    // inserción falla, la transacción del adaptador deshace ambas cosas.
    almacen.ajustar_deuda(datos.tarjeta_id, datos.bonificacion.efecto_sobre_deuda()?)?;

    let id = almacen.insertar_bonificacion(&BonificacionAPersistir {
        fecha: datos.fecha,
        tarjeta_id: datos.tarjeta_id,
        bonificacion: datos.bonificacion,
        gasto_id: datos.gasto_id,
    })?;
    Ok(id)
}

pub fn revertir_bonificacion(
    id: i64,
    almacen: &mut impl AlmacenBonificaciones,
) -> Result<(), ErrorAplicacion> {
    let guardada = almacen.obtener_bonificacion(id)?;

    // Devolver la deuda que el crédito había reducido. No se recorta en cero:
    // aquí se está restituyendo un importe, no retirando valor.
    almacen.ajustar_deuda(guardada.tarjeta_id, guardada.bonificacion.monto())?;
    almacen.eliminar_bonificacion(id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dominio::dinero::{Dinero, Divisa};
    use crate::puertos::dobles::AlmacenEnMemoria;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }
    fn usd(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Usd).unwrap()
    }

    fn almacen() -> AlmacenEnMemoria {
        AlmacenEnMemoria::nuevo().con_tarjeta(20, dop(10000.0))
    }

    fn datos(monto: Dinero, concepto: &str) -> DatosBonificacion {
        DatosBonificacion {
            fecha: "09/09/2026".into(),
            tarjeta_id: 20,
            bonificacion: Bonificacion::nueva(monto, concepto).unwrap(),
            gasto_id: None,
        }
    }

    #[test]
    fn una_bonificacion_reduce_la_deuda_de_la_tarjeta() {
        let mut a = almacen();
        registrar_bonificacion(datos(dop(36.41), "Cashback compra por internet"), &mut a).unwrap();
        assert_eq!(a.deuda_de(20), dop(9963.59));
        assert_eq!(a.total_bonificaciones(), 1);
    }

    #[test]
    fn un_mismo_consumo_admite_varias_bonificaciones() {
        // 1 % base y 2 % de categoría, en dos líneas del mismo día.
        let mut a = almacen();
        let mut base = datos(dop(50.00), "Recompensa base");
        base.gasto_id = Some(7);
        let mut extra = datos(dop(100.00), "Bonificación de categoría");
        extra.gasto_id = Some(7);

        registrar_bonificacion(base, &mut a).unwrap();
        registrar_bonificacion(extra, &mut a).unwrap();

        assert_eq!(a.total_bonificaciones(), 2, "dos créditos para un solo gasto");
        assert_eq!(a.deuda_de(20), dop(10000.0 - 150.00));
    }

    #[test]
    fn una_bonificacion_en_dolares_solo_toca_la_deuda_en_dolares() {
        let mut a = AlmacenEnMemoria::nuevo()
            .con_tarjeta(20, dop(10000.0))
            .con_tarjeta(21, usd(500.0));
        let mut d = datos(usd(12.50), "Cashback");
        d.tarjeta_id = 21;

        registrar_bonificacion(d, &mut a).unwrap();

        assert_eq!(a.deuda_en(21, Divisa::Usd), usd(487.50));
        assert_eq!(a.deuda_de(20), dop(10000.0), "la otra tarjeta no se toca");
    }

    #[test]
    fn una_bonificacion_puede_dejar_la_deuda_en_negativo() {
        // Un saldo a favor es legítimo: el emisor lo arrastra al período
        // siguiente. No se recorta en cero.
        let mut a = AlmacenEnMemoria::nuevo().con_tarjeta(20, dop(50.0));
        registrar_bonificacion(datos(dop(120.00), "Devolución promocional"), &mut a).unwrap();
        assert_eq!(a.deuda_de(20), dop(-70.0));
    }

    #[test]
    fn no_se_registra_sobre_una_tarjeta_inexistente() {
        let mut a = almacen();
        let mut d = datos(dop(10.0), "Cashback");
        d.tarjeta_id = 999;
        assert!(registrar_bonificacion(d, &mut a).is_err());
        assert_eq!(a.total_bonificaciones(), 0);
    }

    // --- Reversión ---

    #[test]
    fn revertir_restituye_la_deuda_exacta() {
        let mut a = almacen();
        let id = registrar_bonificacion(datos(dop(36.41), "Cashback"), &mut a).unwrap();
        assert_eq!(a.deuda_de(20), dop(9963.59));

        revertir_bonificacion(id, &mut a).unwrap();

        assert_eq!(a.deuda_de(20), dop(10000.0));
        assert_eq!(a.total_bonificaciones(), 0);
    }

    #[test]
    fn revertir_dos_veces_falla_la_segunda() {
        let mut a = almacen();
        let id = registrar_bonificacion(datos(dop(36.41), "Cashback"), &mut a).unwrap();
        revertir_bonificacion(id, &mut a).unwrap();

        assert!(revertir_bonificacion(id, &mut a).is_err());
        assert_eq!(a.deuda_de(20), dop(10000.0), "sin doble restitución");
    }

    #[test]
    fn revertir_una_bonificacion_inexistente_es_error() {
        let mut a = almacen();
        assert!(revertir_bonificacion(404, &mut a).is_err());
    }
}
