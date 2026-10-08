//! Casos de uso de los gastos que rodean al registro y a la reversión (que ya viven en `registrar_gasto` y
//! `revertir_gasto`): el listado y la eliminación con su caso de corrección.
//!
//! Lo fijan las pruebas de caracterización `gt1`–`gt5`, `c1`–`c14` y `c136`–`c137`.

use super::revertir_gasto::revertir_gasto;
use super::ErrorAplicacion;
use crate::dominio::correccion::motivo_de_correccion;
use crate::dominio::errores::ErrorDominio;
use crate::puertos::repositorios::*;

/// Los gastos, del más nuevo al más viejo.
pub fn listar_gastos(almacen: &impl ConsultaDeGastos) -> Result<Vec<GastoLeido>, ErrorAplicacion> {
    Ok(almacen.gastos()?)
}

/// Elimina un gasto y devuelve el número del caso de corrección.
///
/// Orden (el de siempre): **primero** si lo creó otra operación —el cargo de un avance, la comisión de un abono: ese no
/// se borra solo y su mensaje manda aunque el motivo sea corto—, después el gasto, el motivo, el caso de corrección y,
/// **al final**, la reversión: si algo falla antes no queda un caso huérfano.
pub fn eliminar_gasto(
    id: i64,
    motivo: &str,
    almacen: &mut (impl AlmacenGastos + ConsultaDeGastos + RegistroDeCorrecciones),
) -> Result<String, ErrorAplicacion> {
    if let Some(razon) = almacen.motivo_de_no_borrar(id)? {
        return Err(ErrorDominio::GastoDerivado { motivo: razon }.into());
    }
    let gasto = almacen.resumen_de_gasto(id)?.ok_or(ErrorAlmacen::NoEncontrado { entidad: "gasto", id })?;
    let motivo = motivo_de_correccion(motivo)?;
    let caso = almacen.anotar_caso(&CasoAAnotar {
        tipo: "gasto".to_string(),
        referencia_id: id,
        descripcion: gasto.descripcion,
        importe: Some(gasto.monto),
        divisa: Some(gasto.divisa),
        motivo,
    })?;
    revertir_gasto(id, almacen)?;
    Ok(caso)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aplicacion::registrar_gasto::{registrar_gasto, DatosGasto};
    use crate::dominio::dinero::{Dinero, Divisa};
    use crate::dominio::gasto::MetodoPago;
    use crate::puertos::dobles::AlmacenEnMemoria;

    const MOTIVO: &str = "Corrección de prueba del sistema";

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }

    fn almacen() -> AlmacenEnMemoria {
        AlmacenEnMemoria::nuevo().con_categoria(1, "Otros").con_caja(10, dop(500.0))
    }

    fn gasto_en_efectivo(a: &mut AlmacenEnMemoria, monto: f64) -> i64 {
        registrar_gasto(
            DatosGasto {
                fecha: "05/10/2026".into(),
                monto: dop(monto),
                descripcion: "Almuerzo".into(),
                categoria_id: 1,
                metodo: Some(MetodoPago::Efectivo),
                metodo_texto: "efectivo".into(),
                es_lbtr: false,
                tarjeta_id: None,
                cuenta_ahorro_id: None,
                tasa_cambio: None,
            },
            a,
        )
        .unwrap()
    }

    #[test]
    fn el_listado_va_del_mas_nuevo_al_mas_viejo() {
        let mut a = almacen();
        let primero = gasto_en_efectivo(&mut a, 40.0);
        let segundo = gasto_en_efectivo(&mut a, 10.0);
        let lista = listar_gastos(&a).unwrap();
        assert_eq!(lista.iter().map(|g| g.id).collect::<Vec<_>>(), vec![segundo, primero]);
        assert_eq!((lista[1].monto, lista[1].divisa.as_str(), lista[1].metodo_pago.as_str()), (40.0, "DOP", "efectivo"));
    }

    #[test]
    fn eliminar_devuelve_lo_gastado_deja_un_caso_y_borra_el_gasto() {
        let mut a = almacen();
        let id = gasto_en_efectivo(&mut a, 60.0);
        a.descripciones_de_gasto.insert(id, "Almuerzo".into());
        assert_eq!(a.saldo(10).unwrap(), dop(440.0));
        let caso = eliminar_gasto(id, MOTIVO, &mut a).unwrap();
        assert_eq!(caso, "CASO-0001");
        assert_eq!(a.saldo(10).unwrap(), dop(500.0));
        assert!(a.gastos.is_empty());
        let c = &a.casos[0];
        assert_eq!((c.tipo.as_str(), c.descripcion.as_str(), c.importe, c.divisa.as_deref()), ("gasto", "Almuerzo", Some(60.0), Some("DOP")));
    }

    #[test]
    fn eliminar_inexistente_o_con_motivo_corto_no_abre_caso_ni_toca_nada() {
        let mut a = almacen();
        let e = eliminar_gasto(404, MOTIVO, &mut a).unwrap_err();
        assert_eq!(e.to_string(), "No se encontró gasto con identificador 404.");
        let id = gasto_en_efectivo(&mut a, 60.0);
        let e = eliminar_gasto(id, "corto", &mut a).unwrap_err();
        assert!(matches!(e, ErrorAplicacion::Dominio(ErrorDominio::MotivoInsuficiente { .. })));
        assert!(a.casos.is_empty());
        assert_eq!((a.gastos.len(), a.saldo(10).unwrap()), (1, dop(440.0)));
    }

    #[test]
    fn un_gasto_derivado_se_rechaza_antes_que_el_motivo_y_con_el_texto_de_su_operacion() {
        let mut a = almacen();
        let id = gasto_en_efectivo(&mut a, 60.0);
        a.motivos_de_no_borrar.insert(id, "Este gasto es el cargo de un avance de efectivo.".into());
        let e = eliminar_gasto(id, "corto", &mut a).unwrap_err();
        assert_eq!(e.to_string(), "Este gasto es el cargo de un avance de efectivo.");
        assert!(matches!(e, ErrorAplicacion::Dominio(ErrorDominio::GastoDerivado { .. })));
        assert_eq!((a.gastos.len(), a.casos.len()), (1, 0));
    }
}
