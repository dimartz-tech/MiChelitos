//! Caso de uso: registrar un gasto.
//!
//! Reproduce el orden de `crear_gasto`: primero calcula los cargos, después
//! mueve el saldo y por último inserta. Esa secuencia importa — si la
//! inserción falla, el saldo ya se movió, y es la transacción del adaptador
//! la que lo deshace.

use super::ErrorAplicacion;
use crate::dominio::errores::ErrorDominio;
use crate::dominio::cargos::cargos_de_transferencia;
use crate::dominio::conversion::{Conversion, EstadoConversion};
use crate::dominio::tarjeta::MONEDA_LOCAL;
use crate::dominio::dinero::{Dinero, TasaCambio};
use crate::dominio::gasto::{afectacion_de_gasto, exigir_referencias, AfectacionSaldo, MetodoPago};
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
    /// Tasa declarada por el titular cuando el gasto se paga desde una cuenta
    /// de otra divisa. El banco se la aplica al ejecutar, así que no es una
    /// estimación sino un dato de la operación.
    pub tasa_cambio: Option<TasaCambio>,
}

pub fn registrar_gasto(
    datos: DatosGasto,
    almacen: &mut impl AlmacenGastos,
) -> Result<i64, ErrorAplicacion> {
    let divisa = datos.monto.divisa();
    // Se valida antes de tocar nada (H4): un consumo con tarjeta que no dice
    // cuál es un dato incompleto, no un gasto que no afecta a ningún saldo.
    exigir_referencias(datos.metodo, datos.tarjeta_id)?;

    let afectacion =
        afectacion_de_gasto(datos.metodo, divisa, datos.tarjeta_id, datos.cuenta_ahorro_id);

    // Si el gasto se paga desde una cuenta de otra divisa, se convierte con la
    // tasa declarada. La conversión ocurre ANTES de calcular los cargos,
    // porque la retención se aplica sobre el importe que sale de la cuenta.
    let estado_conversion = match afectacion {
        AfectacionSaldo::DebitoCuenta { cuenta_id } => {
            let divisa_cuenta = almacen.divisa(cuenta_id)?;
            if divisa_cuenta == divisa {
                EstadoConversion::NoAplica
            } else {
                let tasa = datos.tasa_cambio.ok_or(ErrorDominio::TasaDeCambioRequerida)?;
                EstadoConversion::Liquidada(Conversion::con_tasa(datos.monto, divisa_cuenta, tasa)?)
            }
        }
        // Un consumo en divisa con una tarjeta que traduce queda PENDIENTE: su
        // importe en moneda local no existe todavía, lo fijará el emisor. No
        // se estima, porque una cifra inventada nunca cuadraría con el estado.
        AfectacionSaldo::DeudaTarjeta { tarjeta_id } => {
            if almacen.politica(tarjeta_id)?.deja_pendiente(divisa, MONEDA_LOCAL) {
                EstadoConversion::Pendiente
            } else {
                EstadoConversion::NoAplica
            }
        }
        _ => EstadoConversion::NoAplica,
    };

    let base_de_cargos = match estado_conversion.conversion() {
        Some(c) => c.destino(),
        None => datos.monto,
    };

    let cargos = if datos.metodo.is_some_and(|m| m.devenga_cargos()) {
        let categoria = almacen.nombre(datos.categoria_id)?.unwrap_or_default();
        cargos_de_transferencia(base_de_cargos, &categoria, &datos.descripcion, datos.es_lbtr)?
            .total()?
    } else {
        Dinero::cero(base_de_cargos.divisa())
    };

    // Qué cuenta queda anotada en el gasto. Para el efectivo es la caja que
    // se acaba de resolver: guardarla convierte el vínculo en una referencia
    // real, que es la otra mitad de H3. Antes el gasto en efectivo no
    // referenciaba nada y la caja parecía no tener dependientes.
    let mut cuenta_anotada = datos.cuenta_ahorro_id;

    match afectacion {
        AfectacionSaldo::Ninguna => {}
        AfectacionSaldo::DeudaTarjeta { tarjeta_id } => {
            almacen.ajustar_deuda(tarjeta_id, datos.monto)?;
        }
        AfectacionSaldo::DebitoCuenta { cuenta_id } => {
            let total = base_de_cargos.sumar(&cargos)?;
            almacen.ajustar_saldo(cuenta_id, negativo(total)?)?;
        }
        AfectacionSaldo::DebitoCaja { divisa } => {
            // La caja se resuelve por su papel y su ausencia es un error,
            // no un Ok silencioso (H3).
            let caja = almacen.caja(divisa)?;
            almacen.ajustar_saldo(caja, negativo(datos.monto)?)?;
            cuenta_anotada = Some(caja);
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
        cuenta_ahorro_id: cuenta_anotada,
        estado_conversion,
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
            .con_caja(11, dop(5000.0))
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
            tasa_cambio: None,
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
    fn una_tarjeta_sin_identificador_se_rechaza_en_vez_de_registrarse() {
        // Antes (H4) el gasto se insertaba y ninguna deuda subía. Quedaba en
        // la lista de gastos sin corresponderse con ningún saldo.
        let mut a = almacen();
        let d = datos(900.0, MetodoPago::Tarjeta);

        let error = registrar_gasto(d, &mut a).unwrap_err();

        assert!(
            error.to_string().contains("tarjeta"),
            "el mensaje dice qué falta: {error}"
        );
        assert_eq!(a.total_gastos(), 0, "no se guardó nada");
        assert_eq!(a.deuda_de(20), dop(500.0));
    }

    #[test]
    fn un_gasto_en_efectivo_descuenta_de_la_caja_de_su_divisa() {
        let mut a = almacen();
        let d = datos(1200.0, MetodoPago::Efectivo);

        registrar_gasto(d, &mut a).unwrap();

        assert_eq!(a.saldo_de_caja("Efectivo DOP").unwrap(), dop(3800.0));
    }

    #[test]
    fn sin_caja_de_efectivo_el_gasto_no_se_registra_y_se_avisa() {
        // Antes (H3) esto devolvía Ok: el gasto quedaba guardado y ningún
        // saldo se movía. Un gasto que no se asienta en ninguna parte no es
        // un registro válido, así que la operación falla entera.
        let mut a = AlmacenEnMemoria::nuevo()
            .con_categoria(1, "Alimentación")
            .con_cuenta(11, "Caja Chica DOP", dop(5000.0));
        let d = datos(1200.0, MetodoPago::Efectivo);

        let error = registrar_gasto(d, &mut a).unwrap_err();

        assert!(
            error.to_string().contains("caja de efectivo"),
            "el mensaje dice qué falta: {error}"
        );
        assert_eq!(a.saldo_de(11), dop(5000.0), "ninguna cuenta se tocó");
        assert_eq!(a.total_gastos(), 0, "tampoco se guardó el gasto");
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

    // --- Conversión con tasa declarada ---

    fn usd(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Usd).unwrap()
    }

    fn tasa(v: f64) -> TasaCambio {
        TasaCambio::nueva(v).unwrap()
    }

    #[test]
    fn un_gasto_en_dolares_desde_cuenta_en_pesos_convierte_y_retiene_sobre_los_pesos() {
        let mut a = almacen();
        let mut d = datos(100.0, MetodoPago::Transferencia);
        d.monto = usd(100.0);
        d.cuenta_ahorro_id = Some(10);
        d.tasa_cambio = Some(tasa(60.0));

        let id = registrar_gasto(d, &mut a).unwrap();

        // 100.00 USD x 60 = 6 000.00 DOP; su 0.20 % = 12.00.
        assert_eq!(a.saldo_de(10), dop(100000.0 - 6012.0), "sale monto convertido + retención");
        let g = a.obtener(id).unwrap();
        assert_eq!(g.monto, usd(100.0), "el gasto conserva su divisa de origen");
        assert_eq!(g.cargos, dop(12.0), "la retención va en la divisa debitada");
        assert_eq!(g.estado_conversion.conversion().unwrap().destino(), dop(6000.0));
        assert_eq!(g.monto_debitado(), dop(6000.0));
    }

    #[test]
    fn cruzar_divisas_sin_declarar_la_tasa_es_error() {
        let mut a = almacen();
        let mut d = datos(100.0, MetodoPago::Transferencia);
        d.monto = usd(100.0);
        d.cuenta_ahorro_id = Some(10);

        let e = registrar_gasto(d, &mut a).unwrap_err();

        assert_eq!(e, ErrorAplicacion::Dominio(ErrorDominio::TasaDeCambioRequerida));
        assert_eq!(a.saldo_de(10), dop(100000.0), "ningún saldo se movió");
        assert_eq!(a.total_gastos(), 0);
    }

    #[test]
    fn en_la_misma_divisa_no_se_exige_tasa_ni_se_registra_conversion() {
        let mut a = almacen();
        let mut d = datos(10000.0, MetodoPago::Transferencia);
        d.cuenta_ahorro_id = Some(10);

        let id = registrar_gasto(d, &mut a).unwrap();

        assert_eq!(a.obtener(id).unwrap().estado_conversion, EstadoConversion::NoAplica);
        assert_eq!(a.saldo_de(10), dop(89980.0));
    }

    #[test]
    fn una_tasa_declarada_de_mas_se_ignora_si_las_divisas_coinciden() {
        let mut a = almacen();
        let mut d = datos(10000.0, MetodoPago::Transferencia);
        d.cuenta_ahorro_id = Some(10);
        d.tasa_cambio = Some(tasa(60.0));

        registrar_gasto(d, &mut a).unwrap();

        assert_eq!(a.saldo_de(10), dop(89980.0), "no se aplica conversión alguna");
    }

    #[test]
    fn el_pago_de_tss_en_divisa_convierte_pero_no_retiene() {
        let mut a = almacen();
        let mut d = datos(100.0, MetodoPago::Transferencia);
        d.monto = usd(100.0);
        d.categoria_id = 2;
        d.descripcion = "Pago TSS".into();
        d.cuenta_ahorro_id = Some(10);
        d.tasa_cambio = Some(tasa(60.0));

        registrar_gasto(d, &mut a).unwrap();

        assert_eq!(a.saldo_de(10), dop(94000.0), "sale solo el convertido, sin retención");
    }

    #[test]
    fn un_gasto_con_tarjeta_en_otra_divisa_no_exige_tasa() {
        // La conversión con tasa declarada solo entra cuando se debita una
        // CUENTA. La deuda de tarjeta se lleva en su propia divisa, con una
        // por moneda igual que las columnas del esquema real.
        let mut a = almacen();
        let mut d = datos(75.0, MetodoPago::Tarjeta);
        d.monto = usd(75.0);
        d.tarjeta_id = Some(20);

        registrar_gasto(d, &mut a).unwrap();

        assert_eq!(a.deuda_en(20, Divisa::Usd), usd(75.0), "sube la deuda en dólares");
        assert_eq!(a.deuda_de(20), dop(500.0), "la deuda en pesos no se toca");
    }
}
