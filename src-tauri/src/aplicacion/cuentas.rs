//! Casos de uso de cuentas: darlas de alta y corregir sus datos declarativos.
//!
//! El saldo **no** se corrige aquí: para eso están las transferencias y los movimientos, que dejan rastro.
//! El orden de las comprobaciones es el de los comandos que sustituyen (primero el nombre, después la
//! entidad y la comisión); lo fijan las pruebas de caracterización `m1`–`m8`.

use super::ErrorAplicacion;
use crate::dominio::cuenta;
use crate::dominio::dinero::Dinero;
use crate::dominio::errores::ErrorDominio;
use crate::puertos::repositorios::*;

pub struct DatosCuentaNueva {
    pub nombre: String,
    /// El texto de la divisa tal como llegó; el esquema acepta solo DOP y USD.
    pub divisa: String,
    pub saldo_inicial: Dinero,
    pub entidad: Option<String>,
    pub comision_pago_impuestos: Option<Dinero>,
}

pub struct DatosCuentaCorregida {
    pub id: i64,
    pub nombre: String,
    pub entidad: Option<String>,
    pub comision_pago_impuestos: Option<Dinero>,
}

pub fn crear_cuenta(datos: DatosCuentaNueva, almacen: &mut impl CatalogoDeCuentas) -> Result<i64, ErrorAplicacion> {
    let nombre = cuenta::nombre_de_cuenta(&datos.nombre)?;
    let entidad = cuenta::entidad_declarada(datos.entidad);
    let comision = cuenta::comision_declarada(datos.comision_pago_impuestos)?;
    Ok(almacen.insertar_cuenta(&CuentaNueva {
        nombre,
        divisa: datos.divisa,
        saldo_inicial: datos.saldo_inicial.unidades(),
        entidad,
        comision_pago_impuestos: comision.map(|c| c.unidades()),
    })?)
}

pub fn actualizar_cuenta(
    datos: DatosCuentaCorregida,
    almacen: &mut impl CatalogoDeCuentas,
) -> Result<(), ErrorAplicacion> {
    let nombre = cuenta::nombre_de_cuenta(&datos.nombre)?;
    let entidad = cuenta::entidad_declarada(datos.entidad);
    let comision = cuenta::comision_declarada(datos.comision_pago_impuestos)?;
    let corregida = almacen.corregir_cuenta(&CuentaCorregida {
        id: datos.id,
        nombre,
        entidad,
        comision_pago_impuestos: comision.map(|c| c.unidades()),
    })?;
    if !corregida {
        return Err(ErrorDominio::CuentaNoEncontrada { id: datos.id }.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dominio::dinero::Divisa;
    use crate::puertos::dobles::CuentasEnMemoria;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }

    fn nueva(nombre: &str) -> DatosCuentaNueva {
        DatosCuentaNueva {
            nombre: nombre.into(),
            divisa: "DOP".into(),
            saldo_inicial: dop(100.0),
            entidad: None,
            comision_pago_impuestos: None,
        }
    }

    fn dominio(e: ErrorAplicacion) -> ErrorDominio {
        match e {
            ErrorAplicacion::Dominio(d) => d,
            otro => panic!("se esperaba un error de dominio y fue {otro:?}"),
        }
    }

    #[test]
    fn crear_una_cuenta_la_guarda_recortada_con_su_saldo_y_sus_datos() {
        let mut a = CuentasEnMemoria::nuevo();
        let id = crear_cuenta(
            DatosCuentaNueva {
                entidad: Some(" Banco ".into()),
                comision_pago_impuestos: Some(dop(12.0)),
                ..nueva("  Ahorros ")
            },
            &mut a,
        )
        .unwrap();
        let c = &a.cuentas[0];
        assert_eq!((c.id, c.nombre.as_str(), c.entidad.as_deref()), (id, "Ahorros", Some("Banco")));
        assert_eq!((c.balance_actual, c.comision_pago_impuestos), (100.0, Some(12.0)));
    }

    #[test]
    fn el_nombre_se_comprueba_antes_que_la_comision_y_lo_rechazado_no_deja_nada() {
        let mut a = CuentasEnMemoria::nuevo();
        let e = crear_cuenta(DatosCuentaNueva { comision_pago_impuestos: Some(dop(-5.0)), ..nueva("  ") }, &mut a).unwrap_err();
        assert_eq!(dominio(e), ErrorDominio::CuentaSinNombre);
        let e = crear_cuenta(DatosCuentaNueva { comision_pago_impuestos: Some(dop(-5.0)), ..nueva("X") }, &mut a).unwrap_err();
        assert_eq!(dominio(e), ErrorDominio::ComisionNegativa);
        assert!(a.cuentas.is_empty());
    }

    #[test]
    fn una_comision_de_cero_se_guarda_como_cero_y_no_como_ninguna() {
        let mut a = CuentasEnMemoria::nuevo();
        crear_cuenta(DatosCuentaNueva { comision_pago_impuestos: Some(dop(0.0)), ..nueva("Cero") }, &mut a).unwrap();
        crear_cuenta(nueva("Ninguna"), &mut a).unwrap();
        let por = |n: &str| a.cuentas.iter().find(|c| c.nombre == n).unwrap().comision_pago_impuestos;
        assert_eq!(por("Cero"), Some(0.0));
        assert_eq!(por("Ninguna"), None);
    }

    #[test]
    fn un_nombre_repetido_o_una_divisa_ilegal_los_rechaza_el_almacen() {
        let mut a = CuentasEnMemoria::nuevo();
        crear_cuenta(nueva("Repetida"), &mut a).unwrap();
        let e = crear_cuenta(nueva("Repetida"), &mut a).unwrap_err();
        assert!(matches!(e, ErrorAplicacion::Almacen(ErrorAlmacen::Fallo(ref d)) if d.contains("UNIQUE")), "{e:?}");
        let e = crear_cuenta(DatosCuentaNueva { divisa: "EUR".into(), ..nueva("Euro") }, &mut a).unwrap_err();
        assert!(matches!(e, ErrorAplicacion::Almacen(ErrorAlmacen::Fallo(ref d)) if d.contains("CHECK")), "{e:?}");
        assert_eq!(a.cuentas.len(), 1);
    }

    #[test]
    fn corregir_cambia_nombre_entidad_y_comision_sin_tocar_el_saldo() {
        let mut a = CuentasEnMemoria::nuevo();
        let id = crear_cuenta(nueva("Original"), &mut a).unwrap();
        actualizar_cuenta(
            DatosCuentaCorregida { id, nombre: " Nueva ".into(), entidad: Some(" B ".into()), comision_pago_impuestos: Some(dop(5.0)) },
            &mut a,
        )
        .unwrap();
        let c = &a.cuentas[0];
        assert_eq!((c.nombre.as_str(), c.entidad.as_deref(), c.comision_pago_impuestos, c.balance_actual), ("Nueva", Some("B"), Some(5.0), 100.0));

        actualizar_cuenta(DatosCuentaCorregida { id, nombre: "Nueva".into(), entidad: Some("  ".into()), comision_pago_impuestos: None }, &mut a).unwrap();
        assert_eq!((a.cuentas[0].entidad.clone(), a.cuentas[0].comision_pago_impuestos), (None, None));
    }

    #[test]
    fn corregir_rechaza_vacio_negativo_e_inexistente() {
        let mut a = CuentasEnMemoria::nuevo();
        let id = crear_cuenta(nueva("Estable"), &mut a).unwrap();
        let dato = |nombre: &str, comision| DatosCuentaCorregida { id, nombre: nombre.into(), entidad: None, comision_pago_impuestos: comision };
        assert_eq!(dominio(actualizar_cuenta(dato("  ", None), &mut a).unwrap_err()), ErrorDominio::CuentaSinNombre);
        assert_eq!(dominio(actualizar_cuenta(dato("Estable", Some(dop(-1.0))), &mut a).unwrap_err()), ErrorDominio::ComisionNegativa);
        let e = actualizar_cuenta(DatosCuentaCorregida { id: 999, nombre: "N".into(), entidad: None, comision_pago_impuestos: None }, &mut a).unwrap_err();
        assert_eq!(dominio(e), ErrorDominio::CuentaNoEncontrada { id: 999 });
        assert_eq!(a.cuentas[0].nombre, "Estable");
    }
}
