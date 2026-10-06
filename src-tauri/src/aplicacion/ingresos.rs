//! Casos de uso de las facturas (ingresos formales): emitirlas y cobrarlas.
//!
//! El orden de las comprobaciones es el de los comandos que sustituyen y lo fijan las pruebas de
//! caracterización `n1`–`n7` y `c70`–`c78b`. La transacción la abre y confirma quien llama.
//!
//! **Hallazgo documentado, sin corregir:** emitir no valida que el porcentaje de retención esté entre 0 y 100 ni
//! que número, RNC y nombre no estén vacíos (lo hace el formulario). Ver `n4`; el cambio se consulta.

use super::ErrorAplicacion;
use crate::dominio::dinero::{Dinero, Porcentaje};
use crate::dominio::errores::ErrorDominio;
use crate::dominio::ingreso::{retencion, Deposito};
use crate::puertos::repositorios::*;

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
    almacen: &mut (impl AlmacenIngresos + RepositorioCuentas),
) -> Result<(), ErrorAplicacion> {
    let divisa_cuenta = match almacen.divisa(cuenta_id) {
        Ok(d) => d,
        Err(ErrorAlmacen::NoEncontrado { .. }) => return Err(ErrorDominio::CuentaNoEncontrada { id: cuenta_id }.into()),
        Err(otro) => return Err(otro.into()),
    };
    let deposito = Deposito::nuevo(cuenta_id, divisa_cuenta, importe)?;

    let nombre = almacen.nombre_de_cuenta(cuenta_id)?;
    // La fila guarda exactamente el mismo importe que se acredita a la cuenta.
    if !almacen.marcar_cobrada(id, cuenta_id, &nombre, fecha, deposito.importe().unidades())? {
        return Err(ErrorDominio::FacturaNoPendiente { id }.into());
    }

    almacen.ajustar_saldo(deposito.cuenta_id(), deposito.importe())?;
    Ok(())
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
