//! Casos de uso de los financiamientos (préstamos y líneas de crédito): darlos de alta, corregir sus condiciones,
//! asentar cuotas y declaraciones de saldo, listarlos y darlos de baja.
//!
//! El orden de las comprobaciones es el de los comandos que sustituyen y lo fijan las pruebas de caracterización
//! `q1`–`q9` y `c34`–`c44`. La transacción la abre y confirma quien llama.
//!
//! El saldo **se lleva**, no se deduce: cada cuota lo reduce por su parte de capital (nunca por su importe entero) y
//! un estado de cuenta puede declararlo; la diferencia queda asentada como un movimiento propio, no corregida en
//! silencio.

use super::ErrorAplicacion;
use crate::dominio::dinero::Dinero;
use crate::dominio::errores::ErrorDominio;
use crate::dominio::prestamo::{
    cuotas_de_un_financiamiento, desglosar_cuota, dia_de_pago, disponible, limite_permitido, recordatorio_de_pago,
    TipoPrestamo,
};
use crate::dominio::tarjeta::MONEDA_LOCAL;
use crate::puertos::repositorios::*;

pub struct DatosPrestamoNuevo {
    pub tipo_prestamo: String,
    pub monto_prestamo: Dinero,
    pub institucion_financiera: String,
    pub tasa_actual: f64,
    pub cuotas_totales: Option<i32>,
    pub cuotas_pendientes: Option<i32>,
    pub monto_cuota: Dinero,
    pub dia_pago: i32,
    /// Capital pendiente hoy; si no se indica se asume el monto íntegro (un financiamiento recién desembolsado).
    pub saldo_actual: Option<Dinero>,
    /// Solo en líneas revolventes: el cupo aprobado.
    pub limite_credito: Option<Dinero>,
}

pub struct DatosPrestamoCorregido {
    pub id: i64,
    pub tasa_actual: f64,
    pub monto_cuota: Dinero,
    pub dia_pago: i32,
    pub limite_credito: Option<Dinero>,
    pub tarjeta_id: Option<i64>,
}

/// Un financiamiento con lo que el listado le añade: el saldo, el cupo y el recordatorio de pago.
#[derive(Debug, Clone, PartialEq)]
pub struct PrestamoConAvisos {
    pub leido: PrestamoLeido,
    /// El saldo llevado; en un registro antiguo sin saldo, el monto.
    pub saldo_actual: f64,
    /// El día de pago que manda: el vencimiento de la tarjeta si la facilidad cuelga de una.
    pub dia_pago: i32,
    pub es_revolvente: bool,
    /// Cupo por disponer. `None` si no es revolvente, falta el límite o los datos están corruptos.
    pub disponible: Option<f64>,
    pub alerta_pago: bool,
    pub dias_pago_msg: String,
}

pub fn crear_prestamo(datos: DatosPrestamoNuevo, almacen: &mut impl AlmacenPrestamos) -> Result<i64, ErrorAplicacion> {
    // 1. Las cuotas (una línea flexible no cuenta cuotas); 2. el día; 3. el límite.
    let (totales, pendientes) = cuotas_de_un_financiamiento(&datos.tipo_prestamo, datos.cuotas_totales, datos.cuotas_pendientes)?;
    let dia = dia_de_pago(datos.dia_pago)?;
    limite_permitido(&datos.tipo_prestamo, datos.limite_credito.is_some())?;

    let monto = datos.monto_prestamo.unidades();
    Ok(almacen.insertar_prestamo(&PrestamoNuevo {
        tipo_prestamo: datos.tipo_prestamo,
        monto_prestamo: monto,
        institucion_financiera: datos.institucion_financiera,
        tasa_actual: datos.tasa_actual,
        cuotas_totales: totales,
        cuotas_pendientes: pendientes,
        monto_cuota: datos.monto_cuota.unidades(),
        dia_pago: dia,
        saldo_actual: datos.saldo_actual.map(|s| s.unidades()).unwrap_or(monto),
        limite_credito: datos.limite_credito.map(|l| l.unidades()),
    })?)
}

/// Corrige las condiciones de un financiamiento (tasa, cuota, día, límite, tarjeta). **No** toca el monto original
/// (un hecho histórico) ni el saldo (que tiene su propia vía, `declarar_saldo`, y deja asiento).
pub fn actualizar_prestamo(
    datos: DatosPrestamoCorregido,
    almacen: &mut impl AlmacenPrestamos,
) -> Result<(), ErrorAplicacion> {
    // El día se comprueba antes de buscar el financiamiento; después, el tipo, el límite y la tarjeta.
    let dia = dia_de_pago(datos.dia_pago)?;
    let tipo = almacen
        .tipo_de_prestamo(datos.id)?
        .ok_or(ErrorDominio::FinanciamientoNoEncontrado { id: datos.id })?;
    limite_permitido(&tipo, datos.limite_credito.is_some())?;
    if let Some(tarjeta_id) = datos.tarjeta_id {
        if !almacen.tarjeta_existe(tarjeta_id)? {
            return Err(ErrorDominio::TarjetaNoEncontrada { id: tarjeta_id }.into());
        }
    }
    Ok(almacen.corregir_prestamo(&PrestamoCorregido {
        id: datos.id,
        tasa_actual: datos.tasa_actual,
        monto_cuota: datos.monto_cuota.unidades(),
        dia_pago: dia,
        limite_credito: datos.limite_credito.map(|l| l.unidades()),
        tarjeta_id: datos.tarjeta_id,
    })?)
}

fn estado(id: i64, almacen: &impl AlmacenPrestamos) -> Result<(Dinero, f64, Dinero), ErrorAplicacion> {
    let e = almacen.estado_de_prestamo(id)?.ok_or(ErrorDominio::FinanciamientoNoEncontrado { id })?;
    Ok((Dinero::nuevo(e.saldo, MONEDA_LOCAL)?, e.tasa_anual, Dinero::nuevo(e.monto_cuota, MONEDA_LOCAL)?))
}

/// Asienta una cuota. No reduce la deuda por su importe entero: parte se va en el interés del período. Si la cuota
/// no cubre el interés la deuda crece (no se recorta). Luego baja el contador de cuotas, si el financiamiento lo tiene.
pub fn pagar_cuota(id: i64, fecha: &str, almacen: &mut impl AlmacenPrestamos) -> Result<(), ErrorAplicacion> {
    let (saldo, tasa, cuota) = estado(id, almacen)?;
    let desglose = desglosar_cuota(saldo, tasa, cuota)?;
    let saldo_resultante = saldo.restar(&desglose.capital)?;
    almacen.asentar_movimiento(&MovimientoNuevo {
        prestamo_id: id,
        fecha: fecha.to_string(),
        tipo: "cuota".to_string(),
        monto: cuota,
        interes: desglose.interes,
        capital: desglose.capital,
        saldo_resultante,
    })?;
    almacen.descontar_cuota(id)?;
    Ok(())
}

/// Fija el saldo al que dice el estado de cuenta, dejando asentada la diferencia como un movimiento propio.
pub fn declarar_saldo(
    id: i64,
    declarado: Dinero,
    fecha: &str,
    almacen: &mut impl AlmacenPrestamos,
) -> Result<(), ErrorAplicacion> {
    let (previo, _, _) = estado(id, almacen)?;
    let diferencia = declarado.restar(&previo)?;
    almacen.asentar_movimiento(&MovimientoNuevo {
        prestamo_id: id,
        fecha: fecha.to_string(),
        tipo: "declaracion".to_string(),
        monto: diferencia,
        interes: Dinero::cero(MONEDA_LOCAL),
        capital: diferencia.negado(),
        saldo_resultante: declarado,
    })?;
    Ok(())
}

/// Los financiamientos con su saldo, su cupo y su recordatorio de pago a fecha `dia_actual` (día del mes).
pub fn listar_prestamos(
    dia_actual: i32,
    almacen: &impl AlmacenPrestamos,
) -> Result<Vec<PrestamoConAvisos>, ErrorAplicacion> {
    Ok(almacen
        .prestamos()?
        .into_iter()
        .map(|p| {
            let saldo = p.saldo_actual.unwrap_or(p.monto_prestamo);
            // Una facilidad que cuelga de una tarjeta se paga cuando se paga la tarjeta: su día propio deja de
            // mandar (sería una segunda copia de un dato que ya vive en otro sitio).
            let dia_pago = p.tarjeta_fecha_limite_pago.unwrap_or(p.dia_pago);
            let es_revolvente = TipoPrestamo::desde_codigo(&p.tipo_prestamo).map(|t| t.es_revolvente()).unwrap_or(false);
            // Ante datos corruptos el cupo se degrada a `None` en vez de tumbar la consulta.
            let disponible = match (es_revolvente, p.limite_credito) {
                (true, Some(limite)) => Dinero::nuevo(limite, MONEDA_LOCAL)
                    .and_then(|l| Ok((l, Dinero::nuevo(saldo, MONEDA_LOCAL)?)))
                    .and_then(|(l, s)| disponible(l, s))
                    .map(|d| d.unidades())
                    .ok(),
                _ => None,
            };
            let (alerta_pago, dias_pago_msg) = recordatorio_de_pago(&p.tipo_prestamo, p.cuotas_pendientes, dia_pago, dia_actual);
            PrestamoConAvisos { leido: p, saldo_actual: saldo, dia_pago, es_revolvente, disponible, alerta_pago, dias_pago_msg }
        })
        .collect())
}

pub fn eliminar_prestamo(id: i64, almacen: &mut impl AlmacenPrestamos) -> Result<(), ErrorAplicacion> {
    Ok(almacen.eliminar_prestamo(id)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dominio::dinero::Divisa;
    use crate::puertos::dobles::PrestamosEnMemoria;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }

    fn nuevo(tipo: &str, monto: f64, cuotas: Option<(i32, i32)>, cuota: f64, dia: i32) -> DatosPrestamoNuevo {
        DatosPrestamoNuevo {
            tipo_prestamo: tipo.into(),
            monto_prestamo: dop(monto),
            institucion_financiera: "Banco Ejemplo".into(),
            tasa_actual: 12.0,
            cuotas_totales: cuotas.map(|c| c.0),
            cuotas_pendientes: cuotas.map(|c| c.1),
            monto_cuota: dop(cuota),
            dia_pago: dia,
            saldo_actual: None,
            limite_credito: None,
        }
    }

    fn dominio(e: ErrorAplicacion) -> ErrorDominio {
        match e {
            ErrorAplicacion::Dominio(d) => d,
            otro => panic!("se esperaba un error de dominio y fue {otro:?}"),
        }
    }

    #[test]
    fn crear_guarda_las_cifras_y_el_saldo_por_omision_es_el_monto() {
        let mut a = PrestamosEnMemoria::nuevo();
        let id = crear_prestamo(nuevo("vehiculo", 50000.5, Some((60, 58)), 1250.25, 15), &mut a).unwrap();
        let p = &a.prestamos[0];
        assert_eq!((p.id, p.cuotas_totales, p.cuotas_pendientes, p.saldo_actual), (id, Some(60), Some(58), Some(50000.5)));
        let con_saldo = DatosPrestamoNuevo { saldo_actual: Some(dop(40000.0)), ..nuevo("consumo", 50000.0, Some((12, 12)), 100.0, 5) };
        crear_prestamo(con_saldo, &mut a).unwrap();
        assert_eq!(a.prestamos[1].saldo_actual, Some(40000.0));
    }

    #[test]
    fn una_linea_flexible_nace_sin_cuotas_aunque_se_le_den() {
        let mut a = PrestamosEnMemoria::nuevo();
        crear_prestamo(nuevo("flexible", 1000.0, Some((10, 20)), 50.0, 5), &mut a).unwrap();
        assert_eq!((a.prestamos[0].cuotas_totales, a.prestamos[0].cuotas_pendientes), (None, None));
    }

    #[test]
    fn crear_rechaza_en_su_orden_y_no_deja_nada() {
        let mut a = PrestamosEnMemoria::nuevo();
        let e = crear_prestamo(nuevo("vehiculo", 100.0, None, 10.0, 0), &mut a).unwrap_err();
        assert_eq!(dominio(e), ErrorDominio::CuotasTotalesRequeridas, "las cuotas antes que el día");
        let e = crear_prestamo(nuevo("vehiculo", 100.0, Some((12, 12)), 10.0, 32), &mut a).unwrap_err();
        assert_eq!(dominio(e), ErrorDominio::DiaDePagoInvalido);
        let e = crear_prestamo(DatosPrestamoNuevo { limite_credito: Some(dop(5.0)), ..nuevo("vehiculo", 100.0, Some((12, 12)), 10.0, 5) }, &mut a).unwrap_err();
        assert_eq!(dominio(e), ErrorDominio::LimiteSoloEnLineaRevolvente);
        assert!(a.prestamos.is_empty());
    }

    #[test]
    fn actualizar_comprueba_el_dia_antes_de_buscar_y_dice_cual_falta() {
        let mut a = PrestamosEnMemoria::nuevo().con_tarjeta(7);
        let auto = crear_prestamo(nuevo("vehiculo", 100.0, Some((12, 12)), 10.0, 5), &mut a).unwrap();
        let linea = crear_prestamo(DatosPrestamoNuevo { limite_credito: Some(dop(500.0)), ..nuevo("flexible", 100.0, None, 10.0, 5) }, &mut a).unwrap();
        let datos = |id: i64| DatosPrestamoCorregido { id, tasa_actual: 9.0, monto_cuota: dop(20.0), dia_pago: 25, limite_credito: None, tarjeta_id: None };

        let e = actualizar_prestamo(DatosPrestamoCorregido { dia_pago: 0, ..datos(999) }, &mut a).unwrap_err();
        assert_eq!(dominio(e), ErrorDominio::DiaDePagoInvalido, "antes de buscar el financiamiento");
        let e = actualizar_prestamo(datos(999), &mut a).unwrap_err();
        assert_eq!(dominio(e), ErrorDominio::FinanciamientoNoEncontrado { id: 999 });
        let e = actualizar_prestamo(DatosPrestamoCorregido { limite_credito: Some(dop(1.0)), ..datos(auto) }, &mut a).unwrap_err();
        assert_eq!(dominio(e), ErrorDominio::LimiteSoloEnLineaRevolvente);
        let e = actualizar_prestamo(DatosPrestamoCorregido { tarjeta_id: Some(99), ..datos(linea) }, &mut a).unwrap_err();
        assert_eq!(dominio(e), ErrorDominio::TarjetaNoEncontrada { id: 99 });

        actualizar_prestamo(DatosPrestamoCorregido { limite_credito: Some(dop(900.0)), tarjeta_id: Some(7), ..datos(linea) }, &mut a).unwrap();
        let p = a.prestamos.iter().find(|p| p.id == linea).unwrap();
        assert_eq!((p.tasa_actual, p.monto_cuota, p.dia_pago, p.limite_credito, p.tarjeta_id), (9.0, 20.0, 25, Some(900.0), Some(7)));
        assert_eq!(p.monto_prestamo, 100.0, "el monto original no se reescribe");
        assert_eq!(p.saldo_actual, Some(100.0), "ni el saldo");
    }

    #[test]
    fn pagar_una_cuota_amortiza_solo_el_capital_y_la_asienta_con_su_desglose() {
        let mut a = PrestamosEnMemoria::nuevo();
        // 100 000 al 12 %: 1 000 de interés al mes, 4 000 de capital con una cuota de 5 000.
        let id = crear_prestamo(DatosPrestamoNuevo { limite_credito: Some(dop(150_000.0)), ..nuevo("flexible", 100_000.0, None, 5_000.0, 25) }, &mut a).unwrap();
        pagar_cuota(id, "25/09/2026", &mut a).unwrap();
        assert_eq!(a.prestamos[0].saldo_actual, Some(96_000.0));
        let m = &a.movimientos(id).unwrap()[0];
        assert_eq!((m.tipo.as_str(), m.fecha.as_str(), m.monto, m.interes, m.capital, m.saldo_resultante), ("cuota", "25/09/2026", 5_000.0, 1_000.0, 4_000.0, 96_000.0));
        assert_eq!(a.prestamos[0].cuotas_pendientes, None, "la línea no cuenta cuotas");
    }

    #[test]
    fn una_cuota_que_no_cubre_el_interes_hace_crecer_la_deuda() {
        let mut a = PrestamosEnMemoria::nuevo();
        let id = crear_prestamo(nuevo("vehiculo", 100_000.0, Some((100, 99)), 500.0, 25), &mut a).unwrap();
        pagar_cuota(id, "01/10/2026", &mut a).unwrap();
        assert_eq!(a.prestamos[0].saldo_actual, Some(100_500.0));
    }

    #[test]
    fn el_contador_de_cuotas_baja_de_uno_en_uno_sin_pasar_de_cero() {
        let mut a = PrestamosEnMemoria::nuevo();
        let id = crear_prestamo(nuevo("vehiculo", 10_000.0, Some((12, 1)), 1_000.0, 5), &mut a).unwrap();
        pagar_cuota(id, "01/10/2026", &mut a).unwrap();
        pagar_cuota(id, "01/11/2026", &mut a).unwrap();
        assert_eq!(a.prestamos[0].cuotas_pendientes, Some(0));
        assert_eq!(a.movimientos(id).unwrap().len(), 2, "el pago se asienta igual");
    }

    #[test]
    fn declarar_fija_el_saldo_y_deja_la_diferencia_sin_interes() {
        let mut a = PrestamosEnMemoria::nuevo();
        let id = crear_prestamo(nuevo("flexible", 100_000.0, None, 5_000.0, 25), &mut a).unwrap();
        pagar_cuota(id, "25/09/2026", &mut a).unwrap();
        declarar_saldo(id, dop(96_250.75), "30/09/2026", &mut a).unwrap();
        assert_eq!(a.prestamos[0].saldo_actual, Some(96_250.75));
        let m = &a.movimientos(id).unwrap()[0];
        assert_eq!((m.tipo.as_str(), m.monto, m.interes, m.capital, m.saldo_resultante), ("declaracion", 250.75, 0.0, -250.75, 96_250.75));
    }

    #[test]
    fn pagar_o_declarar_sobre_un_financiamiento_inexistente_dice_su_causa() {
        let mut a = PrestamosEnMemoria::nuevo();
        let e = pagar_cuota(404, "x", &mut a).unwrap_err();
        assert_eq!(dominio(e), ErrorDominio::FinanciamientoNoEncontrado { id: 404 });
        let e = declarar_saldo(404, dop(1.0), "x", &mut a).unwrap_err();
        assert_eq!(dominio(e), ErrorDominio::FinanciamientoNoEncontrado { id: 404 });
        assert!(a.libro.is_empty());
    }

    #[test]
    fn el_listado_calcula_cupo_dia_de_pago_de_la_tarjeta_y_recordatorio() {
        let mut a = PrestamosEnMemoria::nuevo().con_tarjeta(7);
        let linea = crear_prestamo(DatosPrestamoNuevo { limite_credito: Some(dop(150_000.0)), ..nuevo("flexible", 100_000.0, None, 5_000.0, 20) }, &mut a).unwrap();
        let auto = crear_prestamo(nuevo("vehiculo", 100_000.0, Some((100, 0)), 5_000.0, 10), &mut a).unwrap();
        a.prestamos[0].tarjeta_id = Some(7);
        a.prestamos[0].tarjeta_fecha_limite_pago = Some(5);

        let lista = listar_prestamos(5, &a).unwrap();
        assert_eq!(lista.iter().map(|p| p.leido.id).collect::<Vec<_>>(), vec![auto, linea], "el más nuevo primero");
        let l = lista.iter().find(|p| p.leido.id == linea).unwrap();
        assert_eq!((l.dia_pago, l.es_revolvente, l.disponible), (5, true, Some(50_000.0)));
        assert_eq!((l.alerta_pago, l.dias_pago_msg.as_str()), (true, "Hoy vence la cuota."));
        let v = lista.iter().find(|p| p.leido.id == auto).unwrap();
        assert_eq!((v.es_revolvente, v.disponible, v.alerta_pago, v.dias_pago_msg.as_str()), (false, None, false, "-"));
    }

    #[test]
    fn un_registro_sin_saldo_llevado_se_lista_con_el_monto() {
        let mut a = PrestamosEnMemoria::nuevo();
        crear_prestamo(nuevo("vehiculo", 777.0, Some((12, 12)), 10.0, 5), &mut a).unwrap();
        a.prestamos[0].saldo_actual = None;
        assert_eq!(listar_prestamos(1, &a).unwrap()[0].saldo_actual, 777.0);
    }

    #[test]
    fn eliminar_se_lleva_el_libro_y_borrar_lo_que_no_esta_no_es_un_error() {
        let mut a = PrestamosEnMemoria::nuevo();
        let id = crear_prestamo(nuevo("flexible", 1_000.0, None, 100.0, 5), &mut a).unwrap();
        pagar_cuota(id, "01/10/2026", &mut a).unwrap();
        eliminar_prestamo(id, &mut a).unwrap();
        assert!(a.prestamos.is_empty() && a.libro.is_empty());
        assert!(eliminar_prestamo(id, &mut a).is_ok());
    }
}
