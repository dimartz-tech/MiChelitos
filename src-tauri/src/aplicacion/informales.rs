//! Casos de uso de los ingresos informales (los que no llevan factura): crearlos, cobrarlos y eliminarlos.
//!
//! El orden de las comprobaciones es el de los comandos que sustituyen y lo fijan las pruebas de caracterización
//! `p1`–`p8` y `c81`–`c84b`. La transacción la abre y confirma quien llama.
//!
//! Crear un informal exige un monto positivo y fecha y descripción no vacías (`p2`). **Hallazgo documentado, sin
//! corregir** (el cambio se consulta): el cobro en efectivo localiza la caja por su nombre y, si no existe, registra el
//! ingreso como cobrado sin mover ningún saldo (`p6`, el mismo hueco que H3 cerró para los gastos).

use super::ErrorAplicacion;
use super::ingresos::{en_la_divisa_de_la_cuenta, resolver_deposito};
use crate::dominio::correccion::motivo_de_correccion;
use crate::dominio::cuenta::nombre_de_caja_de_cobro;
use crate::dominio::dinero::Dinero;
use crate::dominio::errores::ErrorDominio;
use crate::dominio::tarjeta::MONEDA_LOCAL;
use crate::puertos::repositorios::*;

/// Un ingreso informal lleva fecha, descripción y un monto mayor que cero; uno de cero o negativo no es un ingreso.
fn validar_informal(fecha: &str, descripcion: &str, monto: Dinero) -> Result<(), ErrorAplicacion> {
    if fecha.trim().is_empty() {
        return Err(ErrorDominio::DatoObligatorioVacio { campo: "la fecha del ingreso" }.into());
    }
    if descripcion.trim().is_empty() {
        return Err(ErrorDominio::DatoObligatorioVacio { campo: "la descripción del ingreso" }.into());
    }
    if monto.es_cero() || monto.es_negativo() {
        return Err(ErrorDominio::IngresoSinImporte.into());
    }
    Ok(())
}

/// Registra un ingreso pendiente de cobro. Nace «pendiente».
pub fn crear_ingreso_informal(
    fecha: &str,
    descripcion: &str,
    monto: Dinero,
    almacen: &mut impl AlmacenInformales,
) -> Result<i64, ErrorAplicacion> {
    validar_informal(fecha, descripcion, monto)?;
    Ok(almacen.insertar_informal(fecha, descripcion, monto.unidades())?)
}

/// Registra un cobro que entró directo en efectivo. El importe se usa **dos veces** (el ingreso y la caja): una sola
/// conversión garantiza que reciben exactamente lo mismo. Si la caja no existe, el ingreso se registra y no se mueve
/// ningún saldo (comportamiento actual; ver `p6`).
pub fn crear_cobro_efectivo_informal(
    fecha: &str,
    descripcion: &str,
    monto: Dinero,
    almacen: &mut (impl AlmacenInformales + RepositorioCuentas + BusquedaDeCuentas),
) -> Result<i64, ErrorAplicacion> {
    validar_informal(fecha, descripcion, monto)?;
    let caja = nombre_de_caja_de_cobro(monto.divisa());
    let id = almacen.insertar_cobro_en_efectivo(fecha, descripcion, monto.unidades(), caja)?;
    if let Some(cuenta_id) = almacen.cuenta_por_nombre(caja)? {
        let monto = en_la_divisa_de_la_cuenta(almacen, cuenta_id, monto)?;
        almacen.ajustar_saldo(cuenta_id, monto)?;
    }
    Ok(id)
}

/// Cobra un ingreso pendiente en una cuenta. Orden: la cuenta, el depósito, el ingreso y, solo al final, el saldo.
pub fn marcar_informal_pagado(
    id: i64,
    cuenta_id: i64,
    fecha: &str,
    importe: Dinero,
    almacen: &mut (impl AlmacenInformales + RepositorioCuentas + BusquedaDeCuentas),
) -> Result<(), ErrorAplicacion> {
    let deposito = resolver_deposito(almacen, cuenta_id, importe)?;
    let nombre = almacen.nombre_de_cuenta(cuenta_id)?;
    // La fila guarda exactamente el mismo importe que se acredita a la cuenta.
    if !almacen.marcar_informal_cobrado(id, cuenta_id, &nombre, fecha, deposito.importe().unidades())? {
        return Err(ErrorDominio::IngresoNoPendiente { id }.into());
    }
    almacen.ajustar_saldo(deposito.cuenta_id(), deposito.importe())?;
    Ok(())
}

/// Elimina un ingreso informal. Abre el caso **antes** de borrar y, si estaba cobrado, revierte el abono **por el
/// nombre** de la cuenta (sin recorte a cero, H20); si esa cuenta ya no existe borra igual y no mueve ningún saldo.
pub fn eliminar_ingreso_informal(
    id: i64,
    motivo: &str,
    almacen: &mut (impl AlmacenInformales + RepositorioCuentas + BusquedaDeCuentas + RegistroDeCorrecciones),
) -> Result<String, ErrorAplicacion> {
    let ingreso = almacen
        .estado_de_informal(id)?
        .ok_or(ErrorAlmacen::NoEncontrado { entidad: "ingreso informal", id })?;
    let motivo = motivo_de_correccion(motivo)?;
    let caso = almacen.anotar_caso(&CasoAAnotar {
        tipo: "ingreso informal".to_string(),
        referencia_id: id,
        descripcion: ingreso.descripcion.clone(),
        importe: Some(ingreso.monto),
        divisa: Some("DOP".to_string()),
        motivo,
    })?;

    if ingreso.estatus == "pagado" {
        if let Some(cuenta) = ingreso.institucion_deposito.filter(|d| !d.is_empty()) {
            if let Some(cuenta_id) = almacen.cuenta_por_nombre(&cuenta)? {
                // La caja de dólares recibió dólares: se revierte en la divisa de la cuenta.
                let recibido = Dinero::nuevo(ingreso.monto_recibido.unwrap_or(0.0), MONEDA_LOCAL)?;
                let recibido = en_la_divisa_de_la_cuenta(almacen, cuenta_id, recibido)?;
                almacen.ajustar_saldo(cuenta_id, recibido.negado())?;
            }
        }
    }

    almacen.eliminar_informal(id)?;
    Ok(caso)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dominio::dinero::Divisa;
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
            .con_cuenta(10, "Cuenta DOP", dop(100.0))
            .con_cuenta(11, "Cuenta USD", usd(100.0))
            .con_cuenta(20, "Efectivo DOP", dop(10.0))
            .con_cuenta(21, "Efectivo USD", usd(10.0))
    }

    fn dominio(e: ErrorAplicacion) -> ErrorDominio {
        match e {
            ErrorAplicacion::Dominio(d) => d,
            otro => panic!("se esperaba un error de dominio y fue {otro:?}"),
        }
    }

    #[test]
    fn crear_un_informal_lo_deja_pendiente_y_el_listado_va_del_mas_nuevo_al_mas_viejo() {
        let mut a = almacen();
        let primero = crear_ingreso_informal("01/10/2026", "Uno", dop(100.0), &mut a).unwrap();
        let segundo = crear_ingreso_informal("02/10/2026", "Dos", dop(200.0), &mut a).unwrap();
        let lista = a.informales().unwrap();
        assert_eq!(lista.iter().map(|i| i.id).collect::<Vec<_>>(), vec![segundo, primero]);
        assert_eq!((lista[1].estatus.as_str(), lista[1].monto_recibido), ("pendiente", None));
    }

    #[test]
    fn el_cobro_en_efectivo_entra_en_la_caja_de_su_divisa_y_el_ingreso_queda_pagado() {
        let mut a = almacen();
        let id = crear_cobro_efectivo_informal("04/10/2026", "Pesos", dop(40.0), &mut a).unwrap();
        crear_cobro_efectivo_informal("04/10/2026", "Dólares", usd(5.0), &mut a).unwrap();
        let i = a.informales.iter().find(|i| i.id == id).unwrap();
        assert_eq!((i.estatus.as_str(), i.institucion_deposito.as_deref(), i.monto_recibido), ("pagado", Some("Efectivo DOP"), Some(40.0)));
        assert_eq!((a.saldo(20).unwrap(), a.saldo(21).unwrap()), (dop(50.0), usd(15.0)));
    }

    #[test]
    fn sin_caja_de_efectivo_el_ingreso_se_registra_y_no_se_mueve_ningun_saldo() {
        let mut a = AlmacenEnMemoria::nuevo().con_cuenta(10, "Cuenta DOP", dop(100.0));
        let id = crear_cobro_efectivo_informal("04/10/2026", "Sin caja", dop(90.0), &mut a).unwrap();
        assert_eq!(a.informales[0].id, id);
        assert_eq!(a.saldo(10).unwrap(), dop(100.0));
    }

    #[test]
    fn cobrar_acredita_la_cuenta_y_deja_la_fila_pagada_con_lo_mismo() {
        let mut a = almacen();
        let id = crear_ingreso_informal("01/10/2026", "Trabajo", dop(300.0), &mut a).unwrap();
        marcar_informal_pagado(id, 10, "03/10/2026", dop(275.5), &mut a).unwrap();
        let i = &a.informales[0];
        assert_eq!((i.estatus.as_str(), i.institucion_deposito.as_deref(), i.cuenta_ahorro_id), ("pagado", Some("Cuenta DOP"), Some(10)));
        assert_eq!((i.fecha_pago.as_deref(), i.monto_recibido), (Some("03/10/2026"), Some(275.5)));
        assert_eq!(a.saldo(10).unwrap(), dop(375.5));
    }

    #[test]
    fn los_rechazos_del_cobro_no_dejan_nada_a_medias_y_cobrar_dos_veces_no_acredita_dos_veces() {
        let mut a = almacen();
        let id = crear_ingreso_informal("01/10/2026", "Trabajo", dop(300.0), &mut a).unwrap();
        let e = marcar_informal_pagado(id, 99, "x", dop(10.0), &mut a).unwrap_err();
        assert_eq!(dominio(e), ErrorDominio::CuentaNoEncontrada { id: 99 });
        let e = marcar_informal_pagado(404, 10, "x", dop(10.0), &mut a).unwrap_err();
        assert_eq!(dominio(e), ErrorDominio::IngresoNoPendiente { id: 404 });
        let e = marcar_informal_pagado(id, 11, "x", dop(10.0), &mut a).unwrap_err();
        assert!(matches!(dominio(e), ErrorDominio::DivisasIncompatibles { .. }));
        let e = marcar_informal_pagado(id, 10, "x", dop(-10.0), &mut a).unwrap_err();
        assert!(matches!(dominio(e), ErrorDominio::MontoInvalido { .. }));
        assert_eq!((a.informales[0].estatus.as_str(), a.saldo(10).unwrap()), ("pendiente", dop(100.0)));

        marcar_informal_pagado(id, 10, "x", dop(10.0), &mut a).unwrap();
        let e = marcar_informal_pagado(id, 10, "x", dop(10.0), &mut a).unwrap_err();
        assert_eq!(dominio(e), ErrorDominio::IngresoNoPendiente { id });
        assert_eq!(a.saldo(10).unwrap(), dop(110.0));
    }

    #[test]
    fn eliminar_un_cobrado_revierte_el_abono_deja_un_caso_y_lo_borra() {
        let mut a = almacen();
        let id = crear_ingreso_informal("01/10/2026", "Clase suelta", dop(120.0), &mut a).unwrap();
        marcar_informal_pagado(id, 10, "03/10/2026", dop(120.0), &mut a).unwrap();
        let caso = eliminar_ingreso_informal(id, MOTIVO, &mut a).unwrap();
        assert_eq!(caso, "CASO-0001");
        assert_eq!(a.saldo(10).unwrap(), dop(100.0));
        assert!(a.informales.is_empty());
        let c = &a.casos[0];
        assert_eq!((c.tipo.as_str(), c.descripcion.as_str(), c.importe, c.divisa.as_deref()), ("ingreso informal", "Clase suelta", Some(120.0), Some("DOP")));
    }

    #[test]
    fn eliminar_un_cobro_en_efectivo_revierte_la_caja() {
        let mut a = almacen();
        let id = crear_cobro_efectivo_informal("04/10/2026", "En caja", dop(60.0), &mut a).unwrap();
        eliminar_ingreso_informal(id, MOTIVO, &mut a).unwrap();
        assert_eq!(a.saldo(20).unwrap(), dop(10.0));
    }

    #[test]
    fn eliminar_un_cobro_en_efectivo_en_dolares_revierte_la_caja_de_dolares() {
        // Regresión que cazó la comprobación en la app empaquetada: la reversión usaba pesos y el saldo exacto
        // de una caja en dólares la rechazaba.
        let mut a = almacen();
        let id = crear_cobro_efectivo_informal("04/10/2026", "En dólares", usd(5.0), &mut a).unwrap();
        assert_eq!(a.saldo(21).unwrap(), usd(15.0));
        eliminar_ingreso_informal(id, MOTIVO, &mut a).unwrap();
        assert_eq!(a.saldo(21).unwrap(), usd(10.0));
        assert!(a.informales.is_empty());
    }

    #[test]
    fn eliminar_con_la_cuenta_desaparecida_borra_igual_y_no_mueve_saldos() {
        let mut a = almacen();
        let id = crear_ingreso_informal("01/10/2026", "Trabajo", dop(100.0), &mut a).unwrap();
        marcar_informal_pagado(id, 10, "03/10/2026", dop(100.0), &mut a).unwrap();
        a.cuentas.get_mut(&10).unwrap().nombre = "Otro Nombre".into();
        eliminar_ingreso_informal(id, MOTIVO, &mut a).unwrap();
        assert!(a.informales.is_empty());
        assert_eq!(a.saldo(10).unwrap(), dop(200.0));
    }

    #[test]
    fn eliminar_rechaza_inexistente_motivo_corto_y_no_revierte_lo_pendiente() {
        let mut a = almacen();
        let e = eliminar_ingreso_informal(404, MOTIVO, &mut a).unwrap_err();
        assert_eq!(e.to_string(), "No se encontró ingreso informal con identificador 404.");
        let id = crear_ingreso_informal("01/10/2026", "Pendiente", dop(50.0), &mut a).unwrap();
        let e = eliminar_ingreso_informal(id, "corto", &mut a).unwrap_err();
        assert!(matches!(dominio(e), ErrorDominio::MotivoInsuficiente { .. }));
        assert_eq!(a.informales.len(), 1);
        // Datos incoherentes (pendiente pero con cuenta y recibido): manda el estatus, no el depósito.
        a.informales[0].institucion_deposito = Some("Cuenta DOP".into());
        a.informales[0].monto_recibido = Some(500.0);
        eliminar_ingreso_informal(id, MOTIVO, &mut a).unwrap();
        assert_eq!(a.saldo(10).unwrap(), dop(100.0));
    }
}
