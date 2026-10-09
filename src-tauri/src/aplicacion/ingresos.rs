//! Casos de uso de las facturas (ingresos formales): emitirlas y cobrarlas.
//!
//! El orden de las comprobaciones es el de los comandos que sustituyen y lo fijan las pruebas de
//! caracterización `n1`–`n7` y `c70`–`c78b`. La transacción la abre y confirma quien llama.
//!
//! Emitir exige número, RNC y nombre no vacíos y una retención de 0 a 100 (antes solo lo hacía el formulario; ver `n4`).

use super::ErrorAplicacion;
use crate::dominio::correccion::motivo_de_correccion;
use crate::dominio::dinero::{Dinero, Porcentaje};
use crate::dominio::errores::ErrorDominio;
use crate::dominio::ingreso::{corregir, retencion, Cobro, Deposito};
use crate::dominio::tarjeta::MONEDA_LOCAL;
use crate::puertos::repositorios::*;

/// Comprueba que la cuenta existe y que puede recibir ese importe (H17, H19): su divisa es la del importe y no es
/// negativo. Es lo que comparten el cobro de una factura y el de un ingreso informal.
pub(super) fn resolver_deposito(
    almacen: &impl RepositorioCuentas,
    cuenta_id: i64,
    importe: Dinero,
) -> Result<Deposito, ErrorAplicacion> {
    let divisa_cuenta = match almacen.divisa(cuenta_id) {
        Ok(d) => d,
        Err(ErrorAlmacen::NoEncontrado { .. }) => return Err(ErrorDominio::CuentaNoEncontrada { id: cuenta_id }.into()),
        Err(otro) => return Err(otro.into()),
    };
    Ok(Deposito::nuevo(cuenta_id, divisa_cuenta, importe)?)
}

/// Reexpresa un importe en la divisa **de la cuenta** a la que va a sumarse o restarse.
///
/// Los cobros guardan el depósito por el nombre de la cuenta y los ajustes se aplican sobre lo que esa cuenta tenga
/// (una caja de dólares recibe dólares). Antes el ajuste era un número sin divisa; el saldo exacto exige que coincida,
/// y la de la cuenta es la que manda.
pub(super) fn en_la_divisa_de_la_cuenta(
    almacen: &impl RepositorioCuentas,
    cuenta_id: i64,
    importe: Dinero,
) -> Result<Dinero, ErrorAplicacion> {
    Ok(Dinero::nuevo(importe.unidades(), almacen.divisa(cuenta_id)?)?)
}

pub struct DatosFactura {
    pub numero_factura: String,
    pub rnc_cliente: String,
    pub nombre_cliente: String,
    pub fecha_emision: String,
    /// Siempre en moneda local: `ingresos` no tiene columna de divisa.
    pub monto_total: Dinero,
    pub porcentaje_retencion: f64,
}

/// Emite una factura. El cliente se busca por RNC y, si no está, se crea con el nombre dado; si ya existe
/// conserva el suyo. Un número repetido (sin distinguir mayúsculas) se rechaza **antes** de crear ningún cliente.
pub fn crear_ingreso(datos: DatosFactura, almacen: &mut impl AlmacenIngresos) -> Result<i64, ErrorAplicacion> {
    for (valor, campo) in [
        (&datos.numero_factura, "el número de factura"),
        (&datos.rnc_cliente, "el RNC del cliente"),
        (&datos.nombre_cliente, "el nombre del cliente"),
    ] {
        if valor.trim().is_empty() {
            return Err(ErrorDominio::DatoObligatorioVacio { campo }.into());
        }
    }
    if !(0.0..=100.0).contains(&datos.porcentaje_retencion) {
        return Err(ErrorDominio::RetencionFueraDeRango.into());
    }
    if almacen.factura_existe(&datos.numero_factura)? {
        return Err(ErrorDominio::FacturaDuplicada.into());
    }

    let cliente_id = match almacen.cliente_por_rnc(&datos.rnc_cliente)? {
        Some(id) => id,
        None => almacen.registrar_cliente(&datos.rnc_cliente, &datos.nombre_cliente)?,
    };

    // H16: la retención se decide al céntimo, con el mismo núcleo que el resto del sistema.
    let retenido = retencion(datos.monto_total, Porcentaje::desde_porcentaje(datos.porcentaje_retencion)?)?;

    Ok(almacen.insertar_factura(&FacturaNueva {
        numero_factura: datos.numero_factura,
        cliente_id,
        fecha_emision: datos.fecha_emision,
        monto_total: datos.monto_total.unidades(),
        porcentaje_retencion: datos.porcentaje_retencion,
        monto_retenido: retenido.unidades(),
    })?)
}

/// Cobra una factura pendiente en una cuenta. El importe viaja en moneda local y la cuenta debe estar en esa
/// misma divisa (H19). Orden: la cuenta, el depósito, la factura y, solo al final, el saldo.
pub fn marcar_ingreso_pagado(
    id: i64,
    cuenta_id: i64,
    fecha: &str,
    importe: Dinero,
    almacen: &mut (impl AlmacenIngresos + RepositorioCuentas + BusquedaDeCuentas),
) -> Result<(), ErrorAplicacion> {
    let deposito = resolver_deposito(almacen, cuenta_id, importe)?;

    let nombre = almacen.nombre_de_cuenta(cuenta_id)?;
    // La fila guarda exactamente el mismo importe que se acredita a la cuenta.
    if !almacen.marcar_cobrada(id, cuenta_id, &nombre, fecha, deposito.importe().unidades())? {
        return Err(ErrorDominio::FacturaNoPendiente { id }.into());
    }

    almacen.ajustar_saldo(deposito.cuenta_id(), deposito.importe())?;
    Ok(())
}

pub struct DatosCorreccionDeFactura {
    pub id: i64,
    pub numero_factura: String,
    pub cliente_id: i64,
    pub fecha_emision: String,
    pub monto_total: Dinero,
    pub porcentaje_retencion: f64,
    /// Lo cobrado de verdad cuando no entró el neto entero. `None` es la regla: se da por cobrado el neto completo.
    pub cobro_parcial: Option<Dinero>,
    /// Obligatorio **solo cuando la corrección mueve dinero**: exigir explicación donde no hay riesgo enseña a
    /// escribirla sin pensar.
    pub motivo: Option<String>,
}

/// Cómo terminó una corrección; el comando lo convierte en el texto que ve el titular.
#[derive(Debug, Clone, PartialEq)]
pub enum ResultadoDeCorreccion {
    /// Sin cobrar, o cobrada pero sin diferencia que mover: no se abre caso.
    Corregida,
    /// Se movió dinero: se ajustó esa cuenta y quedó ese caso.
    Ajustada { cuenta: String, ajuste: Dinero, caso: String },
    /// Cobrada y con ajuste, pero sin cuenta de depósito registrada: queda el caso y no se mueve ningún saldo.
    SinCuentaDeDeposito { caso: String },
}

/// Corrige una factura. Orden (el de siempre): la factura, el cálculo, la fila y, solo si hay dinero que mover,
/// el caso (que valida el motivo), lo recibido y el saldo de la cuenta de depósito.
pub fn actualizar_ingreso(
    datos: DatosCorreccionDeFactura,
    almacen: &mut (impl AlmacenIngresos + RepositorioCuentas + BusquedaDeCuentas + RegistroDeCorrecciones),
) -> Result<ResultadoDeCorreccion, ErrorAplicacion> {
    // Lo que la factura decía antes: hace falta para saber cuánto mover, no solo qué escribir.
    let antes = almacen
        .estado_de_factura(datos.id)?
        .ok_or(ErrorDominio::FacturaNoEncontrada { id: datos.id })?;
    // Una factura sin cobrar parte de cero: corregirla y darla por cobrada sería un ajuste por el neto entero.
    let recibido_anterior = Dinero::nuevo(antes.monto_recibido.unwrap_or(0.0), MONEDA_LOCAL)?;

    let cobro = match datos.cobro_parcial {
        Some(parte) => Cobro::Parcial(parte),
        None => Cobro::Completo,
    };
    let correccion = corregir(
        datos.monto_total,
        Porcentaje::desde_porcentaje(datos.porcentaje_retencion)?,
        recibido_anterior,
        cobro,
    )?;

    almacen.corregir_factura(&FacturaCorregida {
        id: datos.id,
        numero_factura: datos.numero_factura.clone(),
        cliente_id: datos.cliente_id,
        fecha_emision: datos.fecha_emision,
        monto_total: datos.monto_total.unidades(),
        porcentaje_retencion: datos.porcentaje_retencion,
        monto_retenido: correccion.retencion.unidades(),
    })?;

    // Una factura sin cobrar no tiene dinero que reajustar: basta con reescribir sus cifras.
    if antes.estatus != "pagada" || correccion.ajuste.es_cero() {
        return Ok(ResultadoDeCorreccion::Corregida);
    }

    // Aquí sí se mueve un saldo, de modo que queda constancia: la misma clase de corrección que un borrado.
    let motivo = motivo_de_correccion(datos.motivo.as_deref().unwrap_or(""))?;
    let caso = almacen.anotar_caso(&CasoAAnotar {
        tipo: "corrección de factura".to_string(),
        referencia_id: datos.id,
        descripcion: format!(
            "Factura {}: {:.2} → {:.2}",
            datos.numero_factura,
            antes.monto_total,
            datos.monto_total.unidades()
        ),
        importe: Some(correccion.ajuste.unidades()),
        divisa: Some(MONEDA_LOCAL.codigo().to_string()),
        motivo,
    })?;
    almacen.fijar_recibido(datos.id, correccion.recibido.unidades())?;

    let Some(cuenta) = antes.institucion_deposito.filter(|d| !d.is_empty()) else {
        return Ok(ResultadoDeCorreccion::SinCuentaDeDeposito { caso });
    };
    let Some(cuenta_id) = almacen.cuenta_por_nombre(&cuenta)? else {
        return Err(ErrorDominio::CuentaDeDepositoInexistente { cuenta }.into());
    };
    let ajuste = en_la_divisa_de_la_cuenta(almacen, cuenta_id, correccion.ajuste)?;
    almacen.ajustar_saldo(cuenta_id, ajuste)?;
    Ok(ResultadoDeCorreccion::Ajustada { cuenta, ajuste: correccion.ajuste, caso })
}

/// Elimina una factura. Abre el caso **antes** de borrar (con la factura todavía a la vista) y, si estaba cobrada,
/// revierte el abono **por el nombre** de la cuenta; si esa cuenta ya no existe borra igual y no mueve ningún saldo
/// (a diferencia de corregir, que se niega: comportamiento actual, fijado por `o6`).
pub fn eliminar_ingreso(
    id: i64,
    motivo: &str,
    almacen: &mut (impl AlmacenIngresos + RepositorioCuentas + BusquedaDeCuentas + RegistroDeCorrecciones),
) -> Result<String, ErrorAplicacion> {
    let factura = almacen
        .estado_de_factura(id)?
        .ok_or(ErrorAlmacen::NoEncontrado { entidad: "factura", id })?;
    let motivo = motivo_de_correccion(motivo)?;
    let caso = almacen.anotar_caso(&CasoAAnotar {
        tipo: "factura".to_string(),
        referencia_id: id,
        descripcion: format!("Factura {}", factura.numero_factura),
        importe: Some(factura.monto_total),
        divisa: Some("DOP".to_string()),
        motivo,
    })?;

    if factura.estatus == "pagada" {
        if let Some(cuenta) = factura.institucion_deposito.filter(|d| !d.is_empty()) {
            // Sin recorte a cero (H20): si lo cobrado ya se gastó, deshacer el cobro deja la cuenta en negativo,
            // que es el estado verdadero: el dinero salió.
            if let Some(cuenta_id) = almacen.cuenta_por_nombre(&cuenta)? {
                let recibido = Dinero::nuevo(factura.monto_recibido.unwrap_or(0.0), MONEDA_LOCAL)?;
                let recibido = en_la_divisa_de_la_cuenta(almacen, cuenta_id, recibido)?;
                almacen.ajustar_saldo(cuenta_id, recibido.negado())?;
            }
        }
    }

    almacen.eliminar_factura(id)?;
    Ok(caso)
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
        AlmacenEnMemoria::nuevo().con_cuenta(10, "Cuenta DOP", dop(100.0)).con_cuenta(11, "Cuenta USD", usd(100.0))
    }

    fn factura(numero: &str, rnc: &str, nombre: &str, total: f64, porcentaje: f64) -> DatosFactura {
        DatosFactura {
            numero_factura: numero.into(),
            rnc_cliente: rnc.into(),
            nombre_cliente: nombre.into(),
            fecha_emision: "05/10/2026".into(),
            monto_total: dop(total),
            porcentaje_retencion: porcentaje,
        }
    }

    fn dominio(e: ErrorAplicacion) -> ErrorDominio {
        match e {
            ErrorAplicacion::Dominio(d) => d,
            otro => panic!("se esperaba un error de dominio y fue {otro:?}"),
        }
    }

    #[test]
    fn emitir_guarda_la_factura_emitida_con_su_retencion_al_centimo() {
        let mut a = almacen();
        let id = crear_ingreso(factura("F-1", "101", "Cliente", 1234.56, 15.0), &mut a).unwrap();
        let f = a.facturas.iter().find(|f| f.id == id).unwrap();
        assert_eq!((f.estatus.as_str(), f.numero_factura.as_str()), ("emitida", "F-1"));
        assert!((f.monto_retenido - 185.18).abs() < 1e-9, "15 % de 1234.56 = 185.184 → 185.18");
        assert_eq!(a.clientes_de_facturas.len(), 1);
    }

    #[test]
    fn el_cliente_se_reutiliza_por_rnc_y_conserva_su_nombre() {
        let mut a = almacen();
        crear_ingreso(factura("F-1", "101", "Original", 10.0, 0.0), &mut a).unwrap();
        crear_ingreso(factura("F-2", "101", "Otro", 10.0, 0.0), &mut a).unwrap();
        assert_eq!(a.clientes_de_facturas.len(), 1);
        assert_eq!(a.clientes_de_facturas[0].nombre, "Original");
        assert_eq!(a.facturas[0].cliente_id, a.facturas[1].cliente_id);
    }

    #[test]
    fn un_numero_repetido_se_rechaza_sin_distinguir_mayusculas_y_antes_de_crear_el_cliente() {
        let mut a = almacen();
        crear_ingreso(factura("F-1", "101", "Uno", 10.0, 0.0), &mut a).unwrap();
        for repetido in ["F-1", "f-1"] {
            let e = crear_ingreso(factura(repetido, "202", "Dos", 10.0, 0.0), &mut a).unwrap_err();
            assert_eq!(dominio(e), ErrorDominio::FacturaDuplicada);
        }
        assert_eq!(a.clientes_de_facturas.len(), 1, "no se creó el cliente de la rechazada");
        assert_eq!(a.facturas.len(), 1);
    }

    #[test]
    fn cobrar_acredita_la_cuenta_y_deja_la_factura_pagada_con_lo_mismo() {
        let mut a = almacen();
        let id = crear_ingreso(factura("F-1", "101", "C", 200.0, 15.0), &mut a).unwrap();
        marcar_ingreso_pagado(id, 10, "06/10/2026", dop(170.5), &mut a).unwrap();
        let f = &a.facturas[0];
        assert_eq!((f.estatus.as_str(), f.institucion_deposito.as_deref(), f.cuenta_ahorro_id), ("pagada", Some("Cuenta DOP"), Some(10)));
        assert_eq!((f.fecha_pago.as_deref(), f.monto_recibido), (Some("06/10/2026"), Some(170.5)));
        assert_eq!(a.saldo(10).unwrap(), dop(270.5));
    }

    #[test]
    fn los_rechazos_del_cobro_no_dejan_nada_a_medias() {
        let mut a = almacen();
        let id = crear_ingreso(factura("F-1", "101", "C", 200.0, 0.0), &mut a).unwrap();

        let e = marcar_ingreso_pagado(id, 99, "x", dop(10.0), &mut a).unwrap_err();
        assert_eq!(dominio(e), ErrorDominio::CuentaNoEncontrada { id: 99 });
        let e = marcar_ingreso_pagado(404, 10, "x", dop(10.0), &mut a).unwrap_err();
        assert_eq!(dominio(e), ErrorDominio::FacturaNoPendiente { id: 404 });
        let e = marcar_ingreso_pagado(id, 11, "x", dop(10.0), &mut a).unwrap_err();
        assert!(matches!(dominio(e), ErrorDominio::DivisasIncompatibles { .. }));
        let e = marcar_ingreso_pagado(id, 10, "x", dop(-10.0), &mut a).unwrap_err();
        assert!(matches!(dominio(e), ErrorDominio::MontoInvalido { .. }));

        assert_eq!(a.facturas[0].estatus, "emitida");
        assert_eq!((a.saldo(10).unwrap(), a.saldo(11).unwrap()), (dop(100.0), usd(100.0)));
    }

    #[test]
    fn cobrar_dos_veces_no_acredita_dos_veces() {
        let mut a = almacen();
        let id = crear_ingreso(factura("F-1", "101", "C", 200.0, 0.0), &mut a).unwrap();
        marcar_ingreso_pagado(id, 10, "x", dop(10.0), &mut a).unwrap();
        let e = marcar_ingreso_pagado(id, 10, "x", dop(10.0), &mut a).unwrap_err();
        assert_eq!(dominio(e), ErrorDominio::FacturaNoPendiente { id });
        assert_eq!(a.saldo(10).unwrap(), dop(110.0));
    }

    #[test]
    fn el_listado_va_de_la_mas_nueva_a_la_mas_vieja_con_los_datos_del_cliente() {
        let mut a = almacen();
        let primera = crear_ingreso(factura("F-1", "101", "Uno", 300.0, 10.0), &mut a).unwrap();
        let segunda = crear_ingreso(factura("F-2", "202", "Dos", 400.0, 0.0), &mut a).unwrap();
        marcar_ingreso_pagado(primera, 10, "07/10/2026", dop(270.0), &mut a).unwrap();
        let lista = a.ingresos().unwrap();
        assert_eq!(lista.iter().map(|i| i.id).collect::<Vec<_>>(), vec![segunda, primera]);
        assert_eq!((lista[1].cliente_nombre.as_str(), lista[1].cliente_rnc.as_str(), lista[1].estatus.as_str()), ("Uno", "101", "pagada"));
        assert_eq!(lista[0].monto_recibido, None);
    }
}

#[cfg(test)]
mod tests_de_correccion {
    use super::*;
    use crate::dominio::dinero::Divisa;
    use crate::puertos::dobles::AlmacenEnMemoria;

    const MOTIVO: &str = "Corrección de prueba del sistema";

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }

    fn almacen_con_factura_cobrada() -> (AlmacenEnMemoria, i64) {
        let mut a = AlmacenEnMemoria::nuevo().con_cuenta(10, "Cuenta DOP", dop(100.0));
        let id = crear_ingreso(
            DatosFactura {
                numero_factura: "F-1".into(),
                rnc_cliente: "101".into(),
                nombre_cliente: "C".into(),
                fecha_emision: "05/10/2026".into(),
                monto_total: dop(2000.0),
                porcentaje_retencion: 15.0,
            },
            &mut a,
        )
        .unwrap();
        marcar_ingreso_pagado(id, 10, "06/10/2026", dop(1700.0), &mut a).unwrap();
        (a, id)
    }

    fn correccion(id: i64, total: f64, motivo: Option<&str>) -> DatosCorreccionDeFactura {
        DatosCorreccionDeFactura {
            id,
            numero_factura: "F-1".into(),
            cliente_id: 1,
            fecha_emision: "05/10/2026".into(),
            monto_total: dop(total),
            porcentaje_retencion: 15.0,
            cobro_parcial: None,
            motivo: motivo.map(|m| m.to_string()),
        }
    }

    fn dominio(e: ErrorAplicacion) -> ErrorDominio {
        match e {
            ErrorAplicacion::Dominio(d) => d,
            otro => panic!("se esperaba un error de dominio y fue {otro:?}"),
        }
    }

    #[test]
    fn corregir_al_alza_acredita_la_diferencia_abre_un_caso_y_fija_lo_recibido() {
        let (mut a, id) = almacen_con_factura_cobrada();
        let r = actualizar_ingreso(correccion(id, 3000.0, Some(MOTIVO)), &mut a).unwrap();
        assert_eq!(r, ResultadoDeCorreccion::Ajustada { cuenta: "Cuenta DOP".into(), ajuste: dop(850.0), caso: "CASO-0001".into() });
        assert_eq!(a.saldo(10).unwrap(), dop(100.0 + 1700.0 + 850.0));
        assert_eq!(a.facturas[0].monto_recibido, Some(2550.0));
        assert!((a.facturas[0].monto_retenido - 450.0).abs() < 1e-9);
        let caso = &a.casos[0];
        assert_eq!((caso.tipo.as_str(), caso.referencia_id), ("corrección de factura", id));
        assert_eq!(caso.descripcion, format!("Factura F-1: {:.2} → {:.2}", 2000.0, 3000.0));
    }

    #[test]
    fn corregir_sin_diferencia_que_mover_no_abre_caso_ni_pide_motivo() {
        let (mut a, id) = almacen_con_factura_cobrada();
        let r = actualizar_ingreso(correccion(id, 2000.0, None), &mut a).unwrap();
        assert_eq!(r, ResultadoDeCorreccion::Corregida);
        assert!(a.casos.is_empty());
        assert_eq!(a.saldo(10).unwrap(), dop(1800.0));
    }

    #[test]
    fn corregir_una_factura_sin_cobrar_solo_reescribe_sus_cifras() {
        let mut a = AlmacenEnMemoria::nuevo();
        let id = crear_ingreso(
            DatosFactura { numero_factura: "F-9".into(), rnc_cliente: "1".into(), nombre_cliente: "C".into(), fecha_emision: "x".into(), monto_total: dop(100.0), porcentaje_retencion: 0.0 },
            &mut a,
        )
        .unwrap();
        let r = actualizar_ingreso(correccion(id, 500.0, None), &mut a).unwrap();
        assert_eq!(r, ResultadoDeCorreccion::Corregida);
        assert_eq!(a.facturas[0].monto_total, 500.0);
        assert!(a.casos.is_empty());
    }

    #[test]
    fn mover_dinero_exige_un_motivo_que_explique_y_dice_cuanto_falta() {
        let (mut a, id) = almacen_con_factura_cobrada();
        for motivo in [None, Some("  ok ")] {
            let e = actualizar_ingreso(correccion(id, 3000.0, motivo), &mut a).unwrap_err();
            assert!(matches!(dominio(e), ErrorDominio::MotivoInsuficiente { .. }));
        }
        assert!(a.casos.is_empty());
    }

    #[test]
    fn corregir_una_factura_inexistente_o_con_cuenta_desaparecida_falla_con_su_causa() {
        let (mut a, id) = almacen_con_factura_cobrada();
        let e = actualizar_ingreso(correccion(404, 10.0, None), &mut a).unwrap_err();
        assert_eq!(dominio(e), ErrorDominio::FacturaNoEncontrada { id: 404 });

        a.cuentas.get_mut(&10).unwrap().nombre = "Otro Nombre".into();
        let e = actualizar_ingreso(correccion(id, 3000.0, Some(MOTIVO)), &mut a).unwrap_err();
        assert_eq!(dominio(e), ErrorDominio::CuentaDeDepositoInexistente { cuenta: "Cuenta DOP".into() });
    }

    #[test]
    fn sin_cuenta_de_deposito_registrada_queda_el_caso_y_no_se_mueve_ningun_saldo() {
        let (mut a, id) = almacen_con_factura_cobrada();
        a.facturas[0].institucion_deposito = None;
        let r = actualizar_ingreso(correccion(id, 3000.0, Some(MOTIVO)), &mut a).unwrap();
        assert_eq!(r, ResultadoDeCorreccion::SinCuentaDeDeposito { caso: "CASO-0001".into() });
        assert_eq!(a.saldo(10).unwrap(), dop(1800.0));
    }

    #[test]
    fn eliminar_una_factura_cobrada_revierte_el_abono_deja_un_caso_y_la_borra() {
        let (mut a, id) = almacen_con_factura_cobrada();
        let caso = eliminar_ingreso(id, MOTIVO, &mut a).unwrap();
        assert_eq!(caso, "CASO-0001");
        assert_eq!(a.saldo(10).unwrap(), dop(100.0));
        assert!(a.facturas.is_empty());
        assert_eq!((a.casos[0].tipo.as_str(), a.casos[0].descripcion.as_str(), a.casos[0].importe), ("factura", "Factura F-1", Some(2000.0)));
    }

    #[test]
    fn eliminar_con_el_saldo_ya_gastado_deja_la_cuenta_en_negativo_sin_recorte() {
        let (mut a, id) = almacen_con_factura_cobrada();
        a.cuentas.get_mut(&10).unwrap().saldo = dop(50.0);
        eliminar_ingreso(id, MOTIVO, &mut a).unwrap();
        assert_eq!(a.saldo(10).unwrap(), dop(-1650.0));
    }

    #[test]
    fn eliminar_rechaza_inexistente_y_motivo_corto_sin_borrar_nada() {
        let (mut a, id) = almacen_con_factura_cobrada();
        let e = eliminar_ingreso(404, MOTIVO, &mut a).unwrap_err();
        assert_eq!(e.to_string(), "No se encontró factura con identificador 404.");
        let e = eliminar_ingreso(id, "corto", &mut a).unwrap_err();
        assert!(matches!(dominio(e), ErrorDominio::MotivoInsuficiente { .. }));
        assert_eq!(a.facturas.len(), 1);
        assert!(a.casos.is_empty());
    }

    #[test]
    fn eliminar_con_la_cuenta_desaparecida_borra_igual_y_no_mueve_saldos() {
        let (mut a, id) = almacen_con_factura_cobrada();
        a.cuentas.get_mut(&10).unwrap().nombre = "Otro Nombre".into();
        eliminar_ingreso(id, MOTIVO, &mut a).unwrap();
        assert!(a.facturas.is_empty());
        assert_eq!(a.saldo(10).unwrap(), dop(1800.0));
    }

    #[test]
    fn solo_se_revierte_lo_cobrado_aunque_una_factura_pendiente_arrastre_un_deposito() {
        // Datos incoherentes (pendiente pero con cuenta y recibido): la guarda de estatus manda, no el depósito.
        let (mut a, id) = almacen_con_factura_cobrada();
        a.facturas[0].estatus = "emitida".into();
        eliminar_ingreso(id, MOTIVO, &mut a).unwrap();
        assert_eq!(a.saldo(10).unwrap(), dop(1800.0), "no se revierte nada: no estaba cobrada");
    }

    #[test]
    fn eliminar_una_factura_sin_cobrar_no_toca_ningun_saldo() {
        let mut a = AlmacenEnMemoria::nuevo().con_cuenta(10, "Cuenta DOP", dop(321.0));
        let id = crear_ingreso(
            DatosFactura { numero_factura: "F-2".into(), rnc_cliente: "2".into(), nombre_cliente: "C".into(), fecha_emision: "x".into(), monto_total: dop(100.0), porcentaje_retencion: 0.0 },
            &mut a,
        )
        .unwrap();
        eliminar_ingreso(id, MOTIVO, &mut a).unwrap();
        assert_eq!(a.saldo(10).unwrap(), dop(321.0));
    }
}
