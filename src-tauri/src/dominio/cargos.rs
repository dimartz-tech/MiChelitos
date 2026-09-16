//! Cargos que acompañan a una transferencia bancaria.
//!
//! Son dos conceptos distintos que hasta ahora vivían fusionados en el campo
//! `costo_adicional`:
//!
//! * **Retención de transferencia** — impuesto sobre el monto transferido. La
//!   Tesorería de la Seguridad Social goza de exención tributaria.
//! * **Comisión de servicio** — precio que cobra el banco por cursar la
//!   operación. Hoy son dos: el carril LBTR de liquidación en tiempo real, y
//!   el servicio de pago de impuestos que algunas entidades tarifan con un
//!   precio fijo en lugar de un porcentaje.
//!
//! Una exención tributaria libera del impuesto, no del precio de un servicio:
//! por eso un pago a la administración tributaria no retiene y sí paga la
//! comisión que corresponda.
//!
//! **La comisión fija por pago de impuestos no está escrita aquí.** La declara
//! cada cuenta, porque es una tarifa de su entidad y varía entre bancos y con
//! el tiempo. El dominio expresa *que existe esa clase de tarifa y cuándo se
//! aplica*; cuánto cobra cada banco es un dato del titular.

use super::dinero::{Dinero, Divisa, Porcentaje};
use super::errores::ErrorDominio;

/// Retención impositiva sobre transferencias: 20 puntos básicos, 0.20 %.
///
/// Se declara en puntos básicos y no como `0.002` porque de ella sale un
/// importe que se guarda: `0.002` no existe exactamente en coma flotante, y
/// cada céntimo producido a partir de ese valor heredaba su error.
pub const TASA_RETENCION: Porcentaje = Porcentaje::puntos_basicos(20);

/// Comisión de servicio del carril LBTR, denominada en pesos.
pub const COMISION_LBTR: f64 = 100.00;

/// Nombre de la categoría que agrupa los pagos a la administración.
const CATEGORIA_IMPUESTOS: &str = "impuestos";

/// Organismos recaudadores cuyos pagos están exentos de la retención.
///
/// Son entidades públicas, no datos del titular, y por eso pueden nombrarse
/// aquí. La exención es tributaria: alcanza al impuesto sobre la transferencia
/// y no al precio que cobre el banco por cursarla.
const MARCAS_EXENCION: [&str; 2] = ["TSS", "DGII"];

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

/// Si la operación es un pago a la administración.
///
/// Es la condición de la que dependen las dos reglas de este módulo que tratan
/// los impuestos de forma distinta al resto: la exención de la retención y la
/// comisión fija del servicio de pago.
pub fn es_pago_de_impuestos(categoria: &str) -> bool {
    categoria.to_lowercase() == CATEGORIA_IMPUESTOS
}

/// Determina si una transferencia está exenta de la retención impositiva.
///
/// Exige que se cumplan **ambas** condiciones: la categoría del gasto y la
/// mención del organismo en su descripción. Ninguna basta por separado, porque
/// la categoría sola incluiría pagos a terceros que sí retienen.
pub fn esta_exenta_de_retencion(categoria: &str, descripcion: &str) -> bool {
    if !es_pago_de_impuestos(categoria) {
        return false;
    }
    let descripcion = descripcion.to_uppercase();
    MARCAS_EXENCION.iter().any(|marca| descripcion.contains(marca))
}

/// Calcula los cargos de una transferencia.
///
/// La retención se redondea **a centavos**, porque el banco la cobra al
/// centavo. La comisión del LBTR se expresa en la divisa del monto sin
/// conversión, replicando el comportamiento vigente; convertirla sería un
/// cambio de conducta y corresponde decidirlo aparte.
///
/// `comision_impuestos` es la tarifa fija que la cuenta de origen declara para
/// el servicio de pago de impuestos. `None` significa que esa cuenta no tiene
/// tarifa pactada, y es distinto de `Some(cero)`: lo primero deja la operación
/// sin comisión de ese concepto, lo segundo declara que el banco la presta
/// gratis. Solo se aplica a pagos de impuestos; en cualquier otra categoría se
/// ignora, porque es el precio de ese servicio y no de la transferencia.
pub fn cargos_de_transferencia(
    monto: Dinero,
    categoria: &str,
    descripcion: &str,
    es_lbtr: bool,
    comision_impuestos: Option<Dinero>,
) -> Result<Cargos, ErrorDominio> {
    let divisa = monto.divisa();

    let retencion = if esta_exenta_de_retencion(categoria, descripcion) {
        Dinero::cero(divisa)
    } else {
        monto.porcentaje(TASA_RETENCION)?
    };

    let mut comision = if es_lbtr {
        Dinero::nuevo(COMISION_LBTR, divisa)?
    } else {
        Dinero::cero(divisa)
    };

    // Las dos comisiones se suman en vez de excluirse: son servicios
    // distintos, y nada impide cursar un pago de impuestos por LBTR.
    if es_pago_de_impuestos(categoria) {
        if let Some(tarifa) = comision_impuestos {
            comision = comision.sumar(&tarifa)?;
        }
    }

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
    fn la_exencion_alcanza_a_los_dos_organismos_recaudadores() {
        assert!(esta_exenta_de_retencion("Impuestos", "Pago TSS"));
        assert!(esta_exenta_de_retencion("Impuestos", "Pago DGII"));
        // Un pago de impuestos a cualquier otro concepto sí retiene.
        assert!(!esta_exenta_de_retencion("Impuestos", "Pago de arbitrios"));
    }

    // --- Comisión fija por el servicio de pago de impuestos ---

    #[test]
    fn la_tarifa_de_impuestos_se_cobra_y_convive_con_la_exencion() {
        // El caso que motivó la regla: el organismo está exento del impuesto
        // sobre la transferencia, y aun así el banco cobra su servicio.
        let c = cargos_de_transferencia(
            dop(10_000.0), "Impuestos", "Pago DGII", false, Some(dop(75.0)),
        )
        .unwrap();

        assert_eq!(c.retencion, dop(0.0), "exento del 0.20 %");
        assert_eq!(c.comision, dop(75.0), "el servicio se paga igual");
        assert_eq!(c.total().unwrap(), dop(75.0));
    }

    #[test]
    fn sin_tarifa_pactada_un_pago_de_impuestos_no_paga_comision() {
        let c = cargos_de_transferencia(
            dop(10_000.0), "Impuestos", "Pago DGII", false, None,
        )
        .unwrap();

        assert_eq!(c.comision, dop(0.0));
    }

    #[test]
    fn una_tarifa_de_cero_declara_que_el_servicio_es_gratis() {
        // Distinto de `None`: aquí el titular afirma que su banco no cobra.
        // Hoy dan el mismo importe; se fija para que la diferencia siga siendo
        // representable si mañana se quiere informar de ella.
        let c = cargos_de_transferencia(
            dop(10_000.0), "Impuestos", "Pago DGII", false, Some(dop(0.0)),
        )
        .unwrap();

        assert_eq!(c.comision, dop(0.0));
    }

    #[test]
    fn la_tarifa_de_impuestos_no_se_aplica_a_otras_categorias() {
        // La cuenta declara la tarifa, pero esto no es un pago de impuestos.
        let c = cargos_de_transferencia(
            dop(10_000.0), "Alimentación", "Compra", false, Some(dop(75.0)),
        )
        .unwrap();

        assert_eq!(c.comision, dop(0.0), "es el precio de otro servicio");
        assert_eq!(c.retencion, dop(20.0), "y sí retiene");
    }

    #[test]
    fn la_tarifa_de_impuestos_se_suma_al_lbtr_porque_son_servicios_distintos() {
        let c = cargos_de_transferencia(
            dop(10_000.0), "Impuestos", "Pago DGII", true, Some(dop(75.0)),
        )
        .unwrap();

        assert_eq!(c.comision, dop(175.0), "carril LBTR más servicio de pago");
    }

    #[test]
    fn la_tarifa_es_fija_y_no_crece_con_el_monto() {
        // Es lo que la distingue de la retención: 10 000 y 500 000 pagan lo
        // mismo. Si algún día un banco la tarifa por tramos, esta prueba es la
        // que obliga a decirlo en vez de dejarlo pasar.
        let pequeno = cargos_de_transferencia(
            dop(10_000.0), "Impuestos", "Pago DGII", false, Some(dop(75.0)),
        )
        .unwrap();
        let grande = cargos_de_transferencia(
            dop(500_000.0), "Impuestos", "Pago DGII", false, Some(dop(75.0)),
        )
        .unwrap();

        assert_eq!(pequeno.total().unwrap(), grande.total().unwrap());
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
        let c = cargos_de_transferencia(dop(10000.0), "Alimentación", "Compra", false, None).unwrap();
        assert_eq!(c.retencion, dop(20.0));
        assert_eq!(c.comision, dop(0.0));
        assert_eq!(c.total().unwrap(), dop(20.0));
    }

    #[test]
    fn la_retencion_conserva_los_centavos_en_vez_de_redondear_a_pesos() {
        // 1250.00 × 0.20 % = 2.50 exactos.
        // Conducta anterior: 3.00, por redondear a unidades enteras.
        let c = cargos_de_transferencia(dop(1250.0), "Alimentación", "Compra", false, None).unwrap();
        assert_eq!(c.retencion, dop(2.50));
        assert_eq!(c.retencion.centavos(), 250);
    }

    #[test]
    fn la_retencion_no_pierde_los_centavos_por_debajo_de_media_unidad() {
        // 1200.00 × 0.20 % = 2.40. Conducta anterior: 2.00.
        let c = cargos_de_transferencia(dop(1200.0), "Alimentación", "Compra", false, None).unwrap();
        assert_eq!(c.retencion, dop(2.40));
    }

    #[test]
    fn un_monto_cero_no_genera_retencion() {
        let c = cargos_de_transferencia(dop(0.0), "Alimentación", "Compra", false, None).unwrap();
        assert_eq!(c.retencion, dop(0.0));
        assert!(c.total().unwrap().es_cero());
    }

    #[test]
    fn un_monto_pequeno_ya_no_pierde_la_retencion() {
        // 100.00 × 0.20 % = 0.20. Conducta anterior: 0.00, el cargo se perdía.
        let c = cargos_de_transferencia(dop(100.0), "Alimentación", "Compra", false, None).unwrap();
        assert_eq!(c.retencion, dop(0.20));
        assert_eq!(c.retencion.centavos(), 20);
    }

    #[test]
    fn la_retencion_de_un_importe_con_centavos_es_exacta() {
        // 12 345.67 × 0.20 % = 24.69134 → 24.69 al centavo.
        // Sin redondeo, ese sobrante fraccionario acababa en los saldos.
        let c = cargos_de_transferencia(dop(12345.67), "Alimentación", "Compra", false, None).unwrap();
        assert_eq!(c.retencion.centavos(), 2469);
    }

    // --- Exención y comisión, juntas ---

    #[test]
    fn el_pago_exento_no_retiene() {
        let c = cargos_de_transferencia(dop(10000.0), "Impuestos", "Pago TSS", false, None).unwrap();
        assert_eq!(c.retencion, dop(0.0));
        assert_eq!(c.total().unwrap(), dop(0.0));
    }

    #[test]
    fn el_lbtr_suma_su_comision_a_la_retencion() {
        let c = cargos_de_transferencia(dop(10000.0), "Alimentación", "Pago", true, None).unwrap();
        assert_eq!(c.retencion, dop(20.0));
        assert_eq!(c.comision, dop(100.0));
        assert_eq!(c.total().unwrap(), dop(120.0));
    }

    #[test]
    fn la_exencion_no_alcanza_a_la_comision_del_lbtr() {
        // La regla del dominio queda legible: exento del impuesto, no del
        // precio del servicio.
        let c = cargos_de_transferencia(dop(10000.0), "Impuestos", "Pago TSS", true, None).unwrap();
        assert_eq!(c.retencion, dop(0.0));
        assert_eq!(c.comision, dop(100.0));
        assert_eq!(c.total().unwrap(), dop(100.0));
    }

    // --- Divisa ---

    #[test]
    fn los_cargos_conservan_la_divisa_del_monto() {
        let c = cargos_de_transferencia(usd(1000.0), "Alimentación", "Compra", true, None).unwrap();
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
