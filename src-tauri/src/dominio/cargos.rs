//! Cargos que acompañan a una transferencia bancaria.
//!
//! Son dos conceptos distintos que hasta ahora vivían fusionados en el campo
//! `costo_adicional`:
//!
//! * **Retención de transferencia** — impuesto sobre el monto transferido. La
//!   Tesorería de la Seguridad Social goza de exención tributaria.
//! * **Comisión de servicio** — precio que cobra el banco por cursar la
//!   operación por el carril LBTR de liquidación en tiempo real. Es opcional
//!   porque no toda transferencia usa esa vía.
//!
//! Una exención tributaria libera del impuesto, no del precio de un servicio:
//! por eso un pago de TSS cursado por LBTR no retiene y sí paga la comisión.

use super::dinero::{Dinero, Divisa};
use super::errores::ErrorDominio;

/// Tasa de la retención impositiva sobre transferencias (0.20 %).
pub const TASA_RETENCION: f64 = 0.002;

/// Comisión de servicio del carril LBTR, denominada en pesos.
pub const COMISION_LBTR: f64 = 100.00;

/// Nombre de la categoría cuyos gastos pueden acogerse a la exención.
const CATEGORIA_EXENTA: &str = "impuestos";

/// Marca que debe aparecer en la descripción para acogerse a la exención.
const MARCA_EXENCION: &str = "TSS";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cargos {
    pub retencion: Dinero,
    pub comision: Dinero,
}

impl Cargos {
    pub fn ninguno(divisa: Divisa) -> Cargos {
        Cargos { retencion: Dinero::cero(divisa), comision: Dinero::cero(divisa) }
    }

    /// Importe conjunto, que es lo que se persiste hoy en `costo_adicional`.
    pub fn total(&self) -> Result<Dinero, ErrorDominio> {
        self.retencion.sumar(&self.comision)
    }
}

/// Determina si una transferencia está exenta de la retención impositiva.
///
/// Exige que se cumplan **ambas** condiciones: la categoría del gasto y la
/// mención de la marca en su descripción. Ninguna basta por separado.
pub fn esta_exenta_de_retencion(categoria: &str, descripcion: &str) -> bool {
    categoria.to_lowercase() == CATEGORIA_EXENTA
        && descripcion.to_uppercase().contains(MARCA_EXENCION)
}

/// Calcula los cargos de una transferencia.
///
/// La retención se redondea **a centavos**, porque el banco la cobra al
/// centavo. La comisión del LBTR se expresa en la divisa del monto sin
/// conversión, replicando el comportamiento vigente; convertirla sería un
/// cambio de conducta y corresponde decidirlo aparte.
pub fn cargos_de_transferencia(
    monto: Dinero,
    categoria: &str,
    descripcion: &str,
    es_lbtr: bool,
) -> Result<Cargos, ErrorDominio> {
    let divisa = monto.divisa();

    let retencion = if esta_exenta_de_retencion(categoria, descripcion) {
        Dinero::cero(divisa)
    } else {
        monto.porcentaje(TASA_RETENCION)?
    };

    let comision = if es_lbtr {
        Dinero::nuevo(COMISION_LBTR, divisa)?
    } else {
        Dinero::cero(divisa)
    };

    Ok(Cargos { retencion, comision })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dop(m: f64) -> Dinero {
        Dinero::nuevo(m, Divisa::Dop).unwrap()
    }
    fn usd(m: f64) -> Dinero {
        Dinero::nuevo(m, Divisa::Usd).unwrap()
    }

    // --- La exención, aislada de todo lo demás ---

    #[test]
    fn la_exencion_exige_categoria_y_marca_a_la_vez() {
        assert!(esta_exenta_de_retencion("Impuestos", "Pago TSS"));
        assert!(!esta_exenta_de_retencion("Alimentación", "Pago TSS"));
        assert!(!esta_exenta_de_retencion("Impuestos", "Pago ITBIS"));
        assert!(!esta_exenta_de_retencion("Alimentación", "Compra"));
    }

    #[test]
    fn la_categoria_se_compara_sin_distinguir_mayusculas() {
        assert!(esta_exenta_de_retencion("IMPUESTOS", "TSS"));
        assert!(esta_exenta_de_retencion("impuestos", "TSS"));
        assert!(esta_exenta_de_retencion("ImPuEsToS", "TSS"));
    }

    #[test]
    fn la_marca_se_reconoce_en_minusculas_y_dentro_de_una_frase() {
        assert!(esta_exenta_de_retencion("Impuestos", "pago tss de agosto"));
        assert!(esta_exenta_de_retencion("Impuestos", "TSS"));
        assert!(esta_exenta_de_retencion("Impuestos", "Liquidación TSS 08/2026"));
    }

    #[test]
    fn la_marca_tambien_coincide_dentro_de_otra_palabra() {
        // Comportamiento vigente: `contains` no exige límites de palabra.
        // Se fija tal cual; cambiarlo sería alterar la conducta actual.
        assert!(esta_exenta_de_retencion("Impuestos", "RETSSA"));
    }

    #[test]
    fn una_categoria_que_solo_empieza_por_impuestos_no_exime() {
        assert!(!esta_exenta_de_retencion("Impuestos municipales", "TSS"));
    }

    // --- Retención ---

    #[test]
    fn la_retencion_ordinaria_es_el_020_por_ciento() {
        let c = cargos_de_transferencia(dop(10000.0), "Alimentación", "Compra", false).unwrap();
        assert_eq!(c.retencion, dop(20.0));
        assert_eq!(c.comision, dop(0.0));
        assert_eq!(c.total().unwrap(), dop(20.0));
    }

    #[test]
    fn la_retencion_conserva_los_centavos_en_vez_de_redondear_a_pesos() {
        // 1250.00 × 0.20 % = 2.50 exactos.
        // Conducta anterior: 3.00, por redondear a unidades enteras.
        let c = cargos_de_transferencia(dop(1250.0), "Alimentación", "Compra", false).unwrap();
        assert_eq!(c.retencion, dop(2.50));
        assert_eq!(c.retencion.centavos(), 250);
    }

    #[test]
    fn la_retencion_no_pierde_los_centavos_por_debajo_de_media_unidad() {
        // 1200.00 × 0.20 % = 2.40. Conducta anterior: 2.00.
        let c = cargos_de_transferencia(dop(1200.0), "Alimentación", "Compra", false).unwrap();
        assert_eq!(c.retencion, dop(2.40));
    }

    #[test]
    fn un_monto_cero_no_genera_retencion() {
        let c = cargos_de_transferencia(dop(0.0), "Alimentación", "Compra", false).unwrap();
        assert_eq!(c.retencion, dop(0.0));
        assert!(c.total().unwrap().es_cero());
    }

    #[test]
    fn un_monto_pequeno_ya_no_pierde_la_retencion() {
        // 100.00 × 0.20 % = 0.20. Conducta anterior: 0.00, el cargo se perdía.
        let c = cargos_de_transferencia(dop(100.0), "Alimentación", "Compra", false).unwrap();
        assert_eq!(c.retencion, dop(0.20));
        assert_eq!(c.retencion.centavos(), 20);
    }

    #[test]
    fn la_retencion_de_un_importe_con_centavos_es_exacta() {
        // 10 423.44 × 0.20 % = 20.84688 → 20.85 al centavo.
        // Sin redondeo, ese sobrante fraccionario acababa en los saldos.
        let c = cargos_de_transferencia(dop(10423.44), "Alimentación", "Compra", false).unwrap();
        assert_eq!(c.retencion.centavos(), 2085);
    }

    // --- Exención y comisión, juntas ---

    #[test]
    fn el_pago_exento_no_retiene() {
        let c = cargos_de_transferencia(dop(10000.0), "Impuestos", "Pago TSS", false).unwrap();
        assert_eq!(c.retencion, dop(0.0));
        assert_eq!(c.total().unwrap(), dop(0.0));
    }

    #[test]
    fn el_lbtr_suma_su_comision_a_la_retencion() {
        let c = cargos_de_transferencia(dop(10000.0), "Alimentación", "Pago", true).unwrap();
        assert_eq!(c.retencion, dop(20.0));
        assert_eq!(c.comision, dop(100.0));
        assert_eq!(c.total().unwrap(), dop(120.0));
    }

    #[test]
    fn la_exencion_no_alcanza_a_la_comision_del_lbtr() {
        // La regla del dominio queda legible: exento del impuesto, no del
        // precio del servicio.
        let c = cargos_de_transferencia(dop(10000.0), "Impuestos", "Pago TSS", true).unwrap();
        assert_eq!(c.retencion, dop(0.0));
        assert_eq!(c.comision, dop(100.0));
        assert_eq!(c.total().unwrap(), dop(100.0));
    }

    // --- Divisa ---

    #[test]
    fn los_cargos_conservan_la_divisa_del_monto() {
        let c = cargos_de_transferencia(usd(1000.0), "Alimentación", "Compra", true).unwrap();
        assert_eq!(c.retencion, usd(2.0));
        assert_eq!(c.comision, usd(100.0), "comportamiento vigente: sin conversión");
        assert_eq!(c.total().unwrap(), usd(102.0));
    }

    #[test]
    fn los_cargos_de_una_operacion_sin_transferencia_son_cero() {
        let c = Cargos::ninguno(Divisa::Dop);
        assert!(c.retencion.es_cero() && c.comision.es_cero());
        assert_eq!(c.total().unwrap(), dop(0.0));
    }
}
