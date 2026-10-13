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

/// El nombre de una cuenta archivada sigue ocupado (el esquema lo exige único entre todas): se avisa con un mensaje
/// que dice qué hacer en lugar de dejar pasar la restricción cruda.
fn exigir_nombre_libre(nombre: &str, excluida: Option<i64>, almacen: &impl CatalogoDeCuentas) -> Result<(), ErrorAplicacion> {
    if almacen.cuentas_archivadas()?.iter().any(|c| c.nombre == nombre && Some(c.id) != excluida) {
        return Err(ErrorDominio::NombreDeCuentaArchivada { nombre: nombre.to_string() }.into());
    }
    Ok(())
}

pub fn crear_cuenta(datos: DatosCuentaNueva, almacen: &mut impl CatalogoDeCuentas) -> Result<i64, ErrorAplicacion> {
    let nombre = cuenta::nombre_de_cuenta(&datos.nombre)?;
    exigir_nombre_libre(&nombre, None, almacen)?;
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
    exigir_nombre_libre(&nombre, Some(datos.id), almacen)?;
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

/// Las cuentas archivadas, para administrarlas. Nunca alimentan un selector.
pub fn listar_archivadas(almacen: &impl CatalogoDeCuentas) -> Result<Vec<CuentaLeida>, ErrorAplicacion> {
    Ok(almacen.cuentas_archivadas()?)
}

/// Exige que la cuenta esté activa antes de una operación **nueva** con ella. Es la comprobación temprana que da el
/// mensaje; la barrera de integridad está en `ajustar_saldo`, que tampoco mueve dinero de una archivada.
pub fn exigir_activa(almacen: &impl RepositorioCuentas, cuenta_id: i64) -> Result<(), ErrorAplicacion> {
    let estado = match almacen.estado_de_cuenta(cuenta_id) {
        Ok(e) => e,
        // Una cuenta que no existe la dice, con su propio mensaje, quien la busca a continuación.
        Err(ErrorAlmacen::NoEncontrado { .. }) => return Ok(()),
        Err(otro) => return Err(otro.into()),
    };
    if estado.archivada {
        return Err(ErrorAplicacion::Almacen(ErrorAlmacen::CuentaArchivada { nombre: estado.nombre }));
    }
    Ok(())
}

/// Archiva una cuenta. **No hay `DELETE`**: la cuenta conserva su identificador, su nombre y su historial.
///
/// Exige saldo **exactamente cero** en su propia divisa (comparado en centavos, sin tolerancias) y no es la caja de
/// efectivo. Un saldo a favor se distingue de uno pendiente. La decisión se repite dentro de la sentencia de archivo
/// (`archivar_cuenta` del puerto), de modo que una carrera entre la lectura y la escritura no puede archivar una cuenta
/// con saldo: quien llama abre la transacción de escritura.
pub fn archivar_cuenta(cuenta_id: i64, almacen: &mut impl RepositorioCuentas) -> Result<(), ErrorAplicacion> {
    let estado = almacen.estado_de_cuenta(cuenta_id)?;
    if estado.archivada {
        return Err(ErrorDominio::CuentaYaArchivada { nombre: estado.nombre }.into());
    }
    if estado.es_caja {
        return Err(ErrorDominio::CajaNoSeArchiva.into());
    }
    if !estado.saldo.es_cero() {
        return Err(ErrorDominio::CuentaConSaldo {
            nombre: estado.nombre,
            divisa: estado.saldo.divisa().codigo(),
            importe: format!("{:.2}", estado.saldo.unidades()),
            a_favor: !estado.saldo.es_negativo(),
        }
        .into());
    }
    if !almacen.archivar_cuenta(cuenta_id)? {
        // Cambió algo entre la lectura y la escritura: se vuelve a mirar para dar la causa verdadera.
        let ahora = almacen.estado_de_cuenta(cuenta_id)?;
        return Err(ErrorDominio::CuentaConSaldo {
            nombre: ahora.nombre,
            divisa: ahora.saldo.divisa().codigo(),
            importe: format!("{:.2}", ahora.saldo.unidades()),
            a_favor: !ahora.saldo.es_negativo(),
        }
        .into());
    }
    Ok(())
}

/// Reactiva una cuenta archivada. Conserva su saldo (cero), su identificador y su historial.
pub fn reactivar_cuenta(cuenta_id: i64, almacen: &mut impl RepositorioCuentas) -> Result<(), ErrorAplicacion> {
    let estado = almacen.estado_de_cuenta(cuenta_id)?;
    if !almacen.reactivar_cuenta(cuenta_id)? {
        return Err(ErrorDominio::CuentaNoArchivada { nombre: estado.nombre }.into());
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

    // --- Archivo y reactivación ---

    use crate::puertos::dobles::AlmacenEnMemoria;

    fn usd(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Usd).unwrap()
    }

    fn con_cuentas() -> AlmacenEnMemoria {
        AlmacenEnMemoria::nuevo()
            .con_cuenta(10, "Vacía", dop(0.0))
            .con_cuenta(11, "A favor", dop(0.01))
            .con_cuenta(12, "Pendiente", usd(-0.01))
            .con_caja(13, dop(0.0))
    }

    #[test]
    fn archivar_exige_cero_exacto_distingue_el_signo_y_no_cambia_nada_si_rechaza() {
        let mut a = con_cuentas();
        let e = archivar_cuenta(11, &mut a).unwrap_err().to_string();
        assert!(e.contains("DOP 0.01 a favor"), "{e}");
        let e = archivar_cuenta(12, &mut a).unwrap_err().to_string();
        assert!(e.contains("saldo pendiente de USD -0.01"), "{e}");
        assert!(!a.estado_de_cuenta(11).unwrap().archivada && !a.estado_de_cuenta(12).unwrap().archivada);

        archivar_cuenta(10, &mut a).unwrap();
        assert!(a.estado_de_cuenta(10).unwrap().archivada);
        assert_eq!(archivar_cuenta(10, &mut a).unwrap_err().to_string(), "La cuenta «Vacía» ya está archivada.");
    }

    #[test]
    fn la_caja_no_se_archiva_ni_en_cero() {
        let mut a = con_cuentas();
        let e = archivar_cuenta(13, &mut a).unwrap_err().to_string();
        assert!(e.contains("caja de efectivo"), "{e}");
        assert!(!a.estado_de_cuenta(13).unwrap().archivada);
    }

    #[test]
    fn una_archivada_no_recibe_operaciones_nuevas_pero_se_reactiva_con_su_saldo_intacto() {
        let mut a = con_cuentas();
        archivar_cuenta(10, &mut a).unwrap();
        let e = exigir_activa(&a, 10).unwrap_err().to_string();
        assert_eq!(e, "La cuenta «Vacía» está archivada: reactívala antes de usarla en una operación.");
        assert!(exigir_activa(&a, 11).is_ok());
        assert!(exigir_activa(&a, 999).is_ok(), "una inexistente la dice quien la busca después");
        assert!(a.ajustar_saldo(10, dop(1.0)).is_err());

        reactivar_cuenta(10, &mut a).unwrap();
        assert!(exigir_activa(&a, 10).is_ok());
        assert_eq!(a.saldo(10).unwrap(), dop(0.0));
        assert_eq!(reactivar_cuenta(10, &mut a).unwrap_err().to_string(), "La cuenta «Vacía» no está archivada.");
    }

    #[test]
    fn el_nombre_de_una_archivada_sigue_ocupado_para_crear_y_para_renombrar() {
        let mut c = CuentasEnMemoria::nuevo();
        c.archivadas.push(CuentaLeida {
            id: 90,
            nombre: "Vieja".into(),
            divisa: "DOP".into(),
            balance_actual: 0.0,
            entidad: None,
            comision_pago_impuestos: None,
        });
        let otra = crear_cuenta(nueva("Otra"), &mut c).unwrap();
        let e = crear_cuenta(nueva("Vieja"), &mut c).unwrap_err().to_string();
        assert_eq!(e, "Ya existe una cuenta archivada llamada «Vieja». Reactívala o elige otro nombre.");
        let renombrar = DatosCuentaCorregida { id: otra, nombre: "Vieja".into(), entidad: None, comision_pago_impuestos: None };
        assert!(actualizar_cuenta(renombrar, &mut c).is_err());
        assert_eq!(listar_archivadas(&c).unwrap().len(), 1);
        assert_eq!(c.cuentas().unwrap().len(), 1, "la archivada no figura entre las operativas");
    }
}
