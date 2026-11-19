//! Casos de uso: pagar un préstamo **desde una cuenta** (o desde la caja de efectivo), preverlo y revertirlo.
//!
//! Antes había dos acciones que no se hablaban: «Abonar cuota» bajaba el saldo del préstamo sin sacar el dinero de
//! ninguna cuenta, y un gasto de la categoría del préstamo sacaba el dinero sin bajar el saldo. Aquí las dos cosas
//! ocurren **en la misma transacción**: el gasto (con la regla de siempre de `registrar_gasto`, retención incluida) y el
//! movimiento del préstamo, que guarda qué gasto lo pagó.
//!
//! * El importe es **libre** (la cuota fija es solo la sugerencia) y la fecha la dice el titular.
//! * Una **cuota** reparte el pago en interés y capital: el interés del recibo si se declara, si no la estimación de
//!   siempre. Consume una cuota del contador.
//! * Un **abono a capital** va íntegro a la deuda y no consume cuota.
//! * **Revertir** solo el último movimiento del préstamo (los posteriores parten de su saldo): deja un caso de
//!   corrección, devuelve el saldo y la cuota, y devuelve el dinero a la cuenta revirtiendo el gasto.
//!
//! Todo en pesos: los financiamientos se llevan en moneda local. La transacción la abre y confirma quien llama.

use super::registrar_gasto::{registrar_gasto, DatosGasto};
use super::revertir_gasto::revertir_gasto;
use super::ErrorAplicacion;
use crate::dominio::correccion::motivo_de_correccion;
use crate::dominio::dinero::Dinero;
use crate::dominio::errores::ErrorDominio;
use crate::dominio::gasto::MetodoPago;
use crate::dominio::prestamo::{desglosar_pago, saldo_previo, TipoPago};
use crate::dominio::tarjeta::MONEDA_LOCAL;
use crate::puertos::repositorios::*;

/// De dónde sale el dinero del pago.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrigenDelPago {
    /// Una cuenta: se registra como transferencia y devenga los cargos de siempre.
    Cuenta(i64),
    /// La caja de efectivo en pesos.
    Efectivo,
}

/// Lo que hace falta para prever un pago.
#[derive(Debug, Clone, PartialEq)]
pub struct PagoAPrever {
    pub prestamo_id: i64,
    pub tipo: TipoPago,
    pub monto: Dinero,
    /// El interés que dice el recibo; `None` para estimarlo.
    pub interes: Option<Dinero>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DatosPagoPrestamo {
    pub pago: PagoAPrever,
    /// `dd/mm/aaaa`.
    pub fecha: String,
    pub origen: OrigenDelPago,
    /// La categoría del gasto que saca el dinero.
    pub categoria_id: i64,
}

/// Cómo quedaría el préstamo con el pago.
#[derive(Debug, Clone, PartialEq)]
pub struct PagoPrevisto {
    pub interes: Dinero,
    pub capital: Dinero,
    /// `true` si el interés es la estimación (no se declaró el del recibo).
    pub interes_estimado: bool,
    pub saldo_previo: Dinero,
    pub saldo_resultante: Dinero,
    /// `None` si el financiamiento no cuenta cuotas.
    pub cuotas_pendientes_despues: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PagoRegistrado {
    pub prevision: PagoPrevisto,
    pub gasto_id: i64,
}

/// Prevé el pago sin escribir nada. Aplica las mismas reglas que el registro, así que lo que se ve es lo que se asienta.
pub fn prever_pago(pago: &PagoAPrever, prestamos: &impl AlmacenPrestamos) -> Result<PagoPrevisto, ErrorAplicacion> {
    let estado = prestamos
        .estado_de_prestamo(pago.prestamo_id)?
        .ok_or(ErrorDominio::FinanciamientoNoEncontrado { id: pago.prestamo_id })?;
    let saldo = Dinero::nuevo(estado.saldo, MONEDA_LOCAL)?;
    let desglose = desglosar_pago(saldo, estado.tasa_anual, pago.tipo, pago.monto, pago.interes)?;
    let cuotas_pendientes_despues = match (pago.tipo.consume_cuota(), estado.cuotas_pendientes) {
        (true, Some(0)) => return Err(ErrorDominio::SinCuotasPendientes.into()),
        (true, Some(n)) => Some(n - 1),
        (false, n) => n,
        (true, None) => None,
    };
    Ok(PagoPrevisto {
        interes: desglose.interes,
        capital: desglose.capital,
        interes_estimado: pago.interes.is_none() && pago.tipo == TipoPago::Cuota,
        saldo_previo: saldo,
        saldo_resultante: saldo.restar(&desglose.capital)?,
        cuotas_pendientes_despues,
    })
}

/// Registra el pago: saca el dinero (un gasto) y lo asienta en el préstamo, en ese orden y en la misma transacción.
pub fn registrar_pago_prestamo(
    datos: DatosPagoPrestamo,
    prestamos: &mut impl AlmacenPrestamos,
    gastos: &mut impl AlmacenGastos,
) -> Result<PagoRegistrado, ErrorAplicacion> {
    // Todo lo que se puede rechazar se comprueba antes de mover dinero.
    let fecha = fecha_del_pago(&datos.fecha)?;
    let prevision = prever_pago(&datos.pago, prestamos)?;
    let institucion = prestamos
        .estado_de_prestamo(datos.pago.prestamo_id)?
        .map(|e| e.institucion)
        .unwrap_or_default();

    // La caja de efectivo no es una cuenta bancaria: pagar «desde ella» es pagar en efectivo, sin la retención de una
    // transferencia, aunque llegue elegida como cuenta.
    let origen = match datos.origen {
        OrigenDelPago::Cuenta(id) if gastos.caja(MONEDA_LOCAL).ok() == Some(id) => OrigenDelPago::Efectivo,
        otro => otro,
    };
    let (metodo, cuenta) = match origen {
        OrigenDelPago::Cuenta(id) => (MetodoPago::Transferencia, Some(id)),
        OrigenDelPago::Efectivo => (MetodoPago::Efectivo, None),
    };
    let concepto = match datos.pago.tipo {
        TipoPago::Cuota => "Cuota de préstamo",
        TipoPago::AbonoCapital => "Abono a capital de préstamo",
    };
    let gasto_id = registrar_gasto(
        DatosGasto {
            fecha: fecha.clone(),
            monto: datos.pago.monto,
            descripcion: format!("{} · {}", concepto, institucion.trim()),
            categoria_id: datos.categoria_id,
            metodo: Some(metodo),
            metodo_texto: metodo_texto(metodo).to_string(),
            es_lbtr: false,
            tarjeta_id: None,
            cuenta_ahorro_id: cuenta,
            tasa_cambio: None,
        },
        gastos,
    )?;

    prestamos.asentar_movimiento(&MovimientoNuevo {
        prestamo_id: datos.pago.prestamo_id,
        fecha,
        tipo: datos.pago.tipo.codigo().to_string(),
        monto: datos.pago.monto,
        interes: prevision.interes,
        capital: prevision.capital,
        saldo_resultante: prevision.saldo_resultante,
        gasto_id: Some(gasto_id),
    })?;
    if datos.pago.tipo.consume_cuota() {
        prestamos.descontar_cuota(datos.pago.prestamo_id)?;
    }
    Ok(PagoRegistrado { prevision, gasto_id })
}

/// Revierte el último pago de un préstamo y devuelve el número del caso de corrección.
///
/// Orden: comprobar (existe, salió de una cuenta, es el último), anotar el caso, devolver el saldo y la cuota, y al
/// final devolver el dinero a la cuenta. Si algo falla, quien abrió la transacción la deshace entera.
pub fn revertir_pago_prestamo(
    movimiento_id: i64,
    motivo: &str,
    prestamos: &mut impl AlmacenPrestamos,
    gastos: &mut (impl AlmacenGastos + RegistroDeCorrecciones),
) -> Result<String, ErrorAplicacion> {
    let m = prestamos
        .movimiento(movimiento_id)?
        .ok_or(ErrorDominio::MovimientoDePrestamoNoEncontrado { id: movimiento_id })?;
    let gasto_id = m.gasto_id.ok_or(ErrorDominio::MovimientoSinPago)?;
    if prestamos.ultimo_movimiento(m.prestamo_id)? != Some(m.id) {
        return Err(ErrorDominio::NoEsElUltimoMovimiento.into());
    }
    let motivo = motivo_de_correccion(motivo)?;
    let institucion = prestamos
        .estado_de_prestamo(m.prestamo_id)?
        .map(|e| e.institucion)
        .unwrap_or_default();
    let concepto = if m.tipo == TipoPago::AbonoCapital.codigo() { "Abono a capital" } else { "Cuota" };

    let caso = gastos.anotar_caso(&CasoAAnotar {
        tipo: "pago_prestamo".to_string(),
        referencia_id: m.id,
        descripcion: format!("{} de préstamo · {} · {}", concepto, institucion.trim(), m.fecha),
        importe: Some(m.monto),
        divisa: Some(MONEDA_LOCAL.codigo().to_string()),
        motivo,
        detalle: Some(
            serde_json::json!({
                "prestamo_id": m.prestamo_id,
                "tipo": m.tipo,
                "fecha": m.fecha,
                "monto": m.monto,
                "interes": m.interes,
                "capital": m.capital,
                "saldo_resultante": m.saldo_resultante,
                "gasto_id": gasto_id,
            })
            .to_string(),
        ),
    })?;

    let previo = saldo_previo(
        Dinero::nuevo(m.saldo_resultante, MONEDA_LOCAL)?,
        Dinero::nuevo(m.capital, MONEDA_LOCAL)?,
    )?;
    prestamos.retirar_movimiento(m.id, m.prestamo_id, previo)?;
    if m.tipo == TipoPago::Cuota.codigo() {
        prestamos.reponer_cuota(m.prestamo_id)?;
    }
    revertir_gasto(gasto_id, gastos)?;
    Ok(caso)
}

fn fecha_del_pago(texto: &str) -> Result<String, ErrorDominio> {
    let fecha = texto.trim();
    if chrono::NaiveDate::parse_from_str(fecha, "%d/%m/%Y").is_err() {
        return Err(ErrorDominio::FechaNoEntendida { fecha: fecha.to_string() });
    }
    Ok(fecha.to_string())
}

fn metodo_texto(metodo: MetodoPago) -> &'static str {
    match metodo {
        MetodoPago::Efectivo => "efectivo",
        MetodoPago::Tarjeta => "tarjeta",
        MetodoPago::Transferencia => "transferencia",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aplicacion::prestamos::{crear_prestamo, declarar_saldo, pagar_cuota, DatosPrestamoNuevo};
    use crate::dominio::dinero::Divisa;
    use crate::puertos::dobles::{AlmacenEnMemoria, PrestamosEnMemoria};

    const MOTIVO: &str = "Se registró con la cuenta equivocada";

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }

    fn dominio(e: ErrorAplicacion) -> ErrorDominio {
        match e {
            ErrorAplicacion::Dominio(d) => d,
            otro => panic!("se esperaba un error de dominio y fue {otro:?}"),
        }
    }

    /// Un préstamo de vehículo de 87 654.32 al 13.5 % con 40 cuotas pendientes y otro flexible sin contador.
    fn montar() -> (PrestamosEnMemoria, AlmacenEnMemoria, i64, i64) {
        let mut p = PrestamosEnMemoria::nuevo();
        let datos = |tipo: &str, cuotas: Option<(i32, i32)>| DatosPrestamoNuevo {
            tipo_prestamo: tipo.into(),
            monto_prestamo: dop(120_000.0),
            institucion_financiera: " Banco Ejemplo ".into(),
            tasa_actual: 13.5,
            cuotas_totales: cuotas.map(|c| c.0),
            cuotas_pendientes: cuotas.map(|c| c.1),
            monto_cuota: dop(3_210.47),
            dia_pago: 15,
            saldo_actual: Some(dop(87_654.32)),
            limite_credito: None,
        };
        let auto = crear_prestamo(datos("vehiculo", Some((48, 40))), &mut p).unwrap();
        let linea = crear_prestamo(datos("flexible", None), &mut p).unwrap();
        let g = AlmacenEnMemoria::nuevo()
            .con_categoria(7, "Prestamo")
            .con_cuenta(10, "Cuenta Ahorros DOP", dop(50_000.0))
            .con_caja(11, dop(9_000.0));
        (p, g, auto, linea)
    }

    fn pago(prestamo_id: i64, tipo: TipoPago, monto: f64, interes: Option<f64>) -> DatosPagoPrestamo {
        DatosPagoPrestamo {
            pago: PagoAPrever { prestamo_id, tipo, monto: dop(monto), interes: interes.map(dop) },
            fecha: "15/11/2026".into(),
            origen: OrigenDelPago::Cuenta(10),
            categoria_id: 7,
        }
    }

    fn prestamo(p: &PrestamosEnMemoria, id: i64) -> &PrestamoLeido {
        p.prestamos.iter().find(|x| x.id == id).unwrap()
    }

    #[test]
    fn una_cuota_saca_el_dinero_con_su_retencion_y_amortiza_solo_el_capital() {
        let (mut p, mut g, auto, _) = montar();
        let r = registrar_pago_prestamo(pago(auto, TipoPago::Cuota, 3_210.47, None), &mut p, &mut g).unwrap();

        // Interés estimado: 87 654.32 × 13.5 % / 12 = 986.11; capital 2 224.36.
        assert_eq!((r.prevision.interes, r.prevision.capital, r.prevision.interes_estimado), (dop(986.11), dop(2_224.36), true));
        assert_eq!((prestamo(&p, auto).saldo_actual, prestamo(&p, auto).cuotas_pendientes), (Some(85_429.96), Some(39)));
        // La cuenta pierde la cuota más la retención de una transferencia (0.20 % = 6.42), como cualquier gasto.
        assert_eq!(g.saldo_de(10), dop(46_783.11), "50 000 − 3 210.47 − 6.42");
        assert_eq!(g.total_gastos(), 1);

        let m = &p.movimientos(auto).unwrap()[0];
        assert_eq!((m.tipo.as_str(), m.fecha.as_str(), m.monto, m.gasto_id), ("cuota", "15/11/2026", 3_210.47, Some(r.gasto_id)));
        assert_eq!((m.interes, m.capital, m.saldo_resultante), (986.11, 2_224.36, 85_429.96));
    }

    #[test]
    fn el_gasto_lleva_la_categoria_y_describe_el_pago() {
        let (mut p, mut g, auto, _) = montar();
        let r = registrar_pago_prestamo(pago(auto, TipoPago::AbonoCapital, 5_000.03, None), &mut p, &mut g).unwrap();
        let gasto = g.resumen_de_gasto(r.gasto_id).unwrap().unwrap();
        assert_eq!(gasto.descripcion, "Abono a capital de préstamo · Banco Ejemplo");
        assert_eq!((gasto.monto, gasto.divisa.as_str()), (5_000.03, "DOP"));
    }

    #[test]
    fn el_interes_del_recibo_manda_sobre_la_estimacion() {
        let (mut p, mut g, auto, _) = montar();
        let r = registrar_pago_prestamo(pago(auto, TipoPago::Cuota, 3_210.47, Some(1_002.03)), &mut p, &mut g).unwrap();
        assert_eq!((r.prevision.interes, r.prevision.capital, r.prevision.interes_estimado), (dop(1_002.03), dop(2_208.44), false));
        assert_eq!(prestamo(&p, auto).saldo_actual, Some(85_445.88));
    }

    #[test]
    fn un_abono_a_capital_va_integro_y_no_consume_cuota() {
        let (mut p, mut g, auto, _) = montar();
        let r = registrar_pago_prestamo(pago(auto, TipoPago::AbonoCapital, 5_000.03, None), &mut p, &mut g).unwrap();
        assert_eq!((r.prevision.interes, r.prevision.capital, r.prevision.interes_estimado), (dop(0.0), dop(5_000.03), false));
        assert_eq!((prestamo(&p, auto).saldo_actual, prestamo(&p, auto).cuotas_pendientes), (Some(82_654.29), Some(40)));
        assert_eq!(p.movimientos(auto).unwrap()[0].tipo, "abono_capital");
    }

    #[test]
    fn en_efectivo_sale_de_la_caja_sin_retencion() {
        let (mut p, mut g, _, linea) = montar();
        let mut d = pago(linea, TipoPago::Cuota, 1_500.0, None);
        d.origen = OrigenDelPago::Efectivo;
        registrar_pago_prestamo(d, &mut p, &mut g).unwrap();
        assert_eq!(g.saldo_de(11), dop(7_500.0));
        assert_eq!(g.saldo_de(10), dop(50_000.0), "la cuenta no se toca");
        assert_eq!(prestamo(&p, linea).cuotas_pendientes, None, "la línea no cuenta cuotas");
    }

    #[test]
    fn la_caja_elegida_como_cuenta_se_paga_en_efectivo_sin_retencion() {
        let (mut p, mut g, auto, _) = montar();
        let mut d = pago(auto, TipoPago::Cuota, 1_500.0, None);
        d.origen = OrigenDelPago::Cuenta(11);
        let r = registrar_pago_prestamo(d, &mut p, &mut g).unwrap();
        assert_eq!(g.saldo_de(11), dop(7_500.0), "sale el importe justo, sin el 0.20 %");
        let gasto = g.gastos.get(&r.gasto_id).unwrap();
        assert_eq!(gasto.metodo_pago, "efectivo");
    }

    #[test]
    fn lo_que_se_rechaza_no_mueve_dinero_ni_deja_movimiento() {
        let casos: Vec<(DatosPagoPrestamo, ErrorDominio)> = {
            let (_, _, auto, _) = montar();
            let mut sin_fecha = pago(auto, TipoPago::Cuota, 100.0, None);
            sin_fecha.fecha = "31/02/2026".into();
            vec![
                (pago(auto, TipoPago::Cuota, 0.0, None), ErrorDominio::PagoDePrestamoSinImporte),
                (pago(auto, TipoPago::Cuota, 100.0, Some(100.01)), ErrorDominio::InteresFueraDelPago),
                (pago(auto, TipoPago::AbonoCapital, 100.0, Some(1.0)), ErrorDominio::InteresEnAbonoACapital),
                (pago(404, TipoPago::Cuota, 100.0, None), ErrorDominio::FinanciamientoNoEncontrado { id: 404 }),
                (sin_fecha, ErrorDominio::FechaNoEntendida { fecha: "31/02/2026".into() }),
            ]
        };
        for (datos, esperado) in casos {
            let (mut p, mut g, auto, _) = montar();
            let e = registrar_pago_prestamo(datos, &mut p, &mut g).unwrap_err();
            assert_eq!(dominio(e), esperado);
            assert_eq!((g.saldo_de(10), g.total_gastos()), (dop(50_000.0), 0));
            assert!(p.libro.is_empty());
            assert_eq!(prestamo(&p, auto).saldo_actual, Some(87_654.32));
        }
    }

    #[test]
    fn sin_cuotas_pendientes_la_cuota_se_rechaza_y_el_abono_a_capital_no() {
        let (mut p, mut g, auto, _) = montar();
        p.prestamos.iter_mut().find(|x| x.id == auto).unwrap().cuotas_pendientes = Some(0);
        let e = registrar_pago_prestamo(pago(auto, TipoPago::Cuota, 100.0, None), &mut p, &mut g).unwrap_err();
        assert_eq!(dominio(e), ErrorDominio::SinCuotasPendientes);
        assert_eq!(g.total_gastos(), 0);
        registrar_pago_prestamo(pago(auto, TipoPago::AbonoCapital, 100.0, None), &mut p, &mut g).unwrap();
    }

    #[test]
    fn una_cuenta_que_no_existe_no_deja_movimiento() {
        let (mut p, mut g, auto, _) = montar();
        let mut d = pago(auto, TipoPago::Cuota, 100.0, None);
        d.origen = OrigenDelPago::Cuenta(99);
        assert!(registrar_pago_prestamo(d, &mut p, &mut g).is_err());
        assert!(p.libro.is_empty());
        assert_eq!(prestamo(&p, auto).saldo_actual, Some(87_654.32));
    }

    #[test]
    fn prever_dice_lo_mismo_que_registrar_y_no_escribe() {
        let (mut p, mut g, auto, _) = montar();
        let d = pago(auto, TipoPago::Cuota, 3_210.47, None);
        let previsto = prever_pago(&d.pago, &p).unwrap();
        assert!(p.libro.is_empty() && g.total_gastos() == 0);
        assert_eq!(previsto.saldo_previo, dop(87_654.32));
        assert_eq!(previsto.cuotas_pendientes_despues, Some(39));
        assert_eq!(registrar_pago_prestamo(d, &mut p, &mut g).unwrap().prevision, previsto);
    }

    #[test]
    fn revertir_devuelve_el_dinero_el_saldo_y_la_cuota_y_deja_un_caso() {
        let (mut p, mut g, auto, _) = montar();
        registrar_pago_prestamo(pago(auto, TipoPago::Cuota, 3_210.47, None), &mut p, &mut g).unwrap();
        let mov = p.movimientos(auto).unwrap()[0].id;

        let caso = revertir_pago_prestamo(mov, MOTIVO, &mut p, &mut g).unwrap();

        assert!(!caso.is_empty());
        assert_eq!(g.saldo_de(10), dop(50_000.0), "cuota y retención devueltas");
        assert_eq!(g.total_gastos(), 0);
        assert_eq!((prestamo(&p, auto).saldo_actual, prestamo(&p, auto).cuotas_pendientes), (Some(87_654.32), Some(40)));
        assert!(p.libro.is_empty());
        let c = g.casos.last().unwrap();
        assert_eq!((c.tipo.as_str(), c.referencia_id, c.importe, c.motivo.as_str()), ("pago_prestamo", mov, Some(3_210.47), MOTIVO));
        assert_eq!(c.descripcion, "Cuota de préstamo · Banco Ejemplo · 15/11/2026");
        let detalle: serde_json::Value = serde_json::from_str(c.detalle.as_deref().unwrap()).unwrap();
        assert_eq!((detalle["capital"].as_f64(), detalle["saldo_resultante"].as_f64()), (Some(2_224.36), Some(85_429.96)));
    }

    #[test]
    fn revertir_un_abono_a_capital_no_toca_el_contador() {
        let (mut p, mut g, auto, _) = montar();
        registrar_pago_prestamo(pago(auto, TipoPago::AbonoCapital, 5_000.03, None), &mut p, &mut g).unwrap();
        let mov = p.movimientos(auto).unwrap()[0].id;
        revertir_pago_prestamo(mov, MOTIVO, &mut p, &mut g).unwrap();
        assert_eq!((prestamo(&p, auto).saldo_actual, prestamo(&p, auto).cuotas_pendientes), (Some(87_654.32), Some(40)));
    }

    #[test]
    fn solo_se_revierte_el_ultimo_y_solo_si_salio_de_una_cuenta() {
        let (mut p, mut g, auto, linea) = montar();
        registrar_pago_prestamo(pago(auto, TipoPago::Cuota, 3_210.47, None), &mut p, &mut g).unwrap();
        let primero = p.movimientos(auto).unwrap()[0].id;
        // Un pago a otro préstamo no cuenta: el último se mide por préstamo.
        registrar_pago_prestamo(pago(linea, TipoPago::Cuota, 1_000.0, None), &mut p, &mut g).unwrap();
        declarar_saldo(auto, dop(85_000.0), "30/11/2026", &mut p).unwrap();
        let declaracion = p.movimientos(auto).unwrap()[0].id;

        assert_eq!(dominio(revertir_pago_prestamo(primero, MOTIVO, &mut p, &mut g).unwrap_err()), ErrorDominio::NoEsElUltimoMovimiento);
        assert_eq!(dominio(revertir_pago_prestamo(declaracion, MOTIVO, &mut p, &mut g).unwrap_err()), ErrorDominio::MovimientoSinPago);
        assert_eq!(
            dominio(revertir_pago_prestamo(9_999, MOTIVO, &mut p, &mut g).unwrap_err()),
            ErrorDominio::MovimientoDePrestamoNoEncontrado { id: 9_999 }
        );
        // Una cuota del botón antiguo (sin cuenta) tampoco: no hay dinero que devolver.
        let (mut p2, mut g2, auto2, _) = montar();
        pagar_cuota(auto2, "15/10/2026", &mut p2).unwrap();
        let vieja = p2.movimientos(auto2).unwrap()[0].id;
        assert_eq!(dominio(revertir_pago_prestamo(vieja, MOTIVO, &mut p2, &mut g2).unwrap_err()), ErrorDominio::MovimientoSinPago);
        assert!(g.casos.is_empty() && g2.casos.is_empty(), "un rechazo no deja caso");
    }

    #[test]
    fn un_motivo_insuficiente_no_revierte_nada() {
        let (mut p, mut g, auto, _) = montar();
        registrar_pago_prestamo(pago(auto, TipoPago::Cuota, 3_210.47, None), &mut p, &mut g).unwrap();
        let mov = p.movimientos(auto).unwrap()[0].id;
        assert!(matches!(dominio(revertir_pago_prestamo(mov, "error", &mut p, &mut g).unwrap_err()), ErrorDominio::MotivoInsuficiente { .. }));
        assert_eq!((g.total_gastos(), p.libro.len(), g.casos.len()), (1, 1, 0));
        assert_eq!(prestamo(&p, auto).saldo_actual, Some(85_429.96));
    }
}
