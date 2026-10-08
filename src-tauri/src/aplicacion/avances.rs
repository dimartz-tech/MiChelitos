//! Casos de uso de los avances de efectivo que rodean al registro y a la reversión (que ya viven en
//! `registrar_avance_de_efectivo` y `revertir_avance_de_efectivo`): el registro con la categoría del cargo resuelta, el
//! historial y la reversión con su caso de corrección.
//!
//! Lo fijan las pruebas de caracterización `av1`–`av5` y `c108`–`c127`. Lo que llega de la interfaz (el tipo de cargo, la
//! fecha, la nota) ya viene validado por `dominio::avance`.

use super::registrar_avance_de_efectivo::{registrar_avance_de_efectivo, AvanceRegistrado, DatosAvance};
use super::revertir_avance_de_efectivo::{revertir_avance_de_efectivo, AvanceRevertido};
use super::ErrorAplicacion;
use crate::dominio::correccion::motivo_de_correccion;
use crate::puertos::repositorios::*;

/// Registra un avance. El cargo (si lo hay) se asienta como gasto de la categoría de sistema «Otros».
pub fn registrar_avance(
    datos: DatosAvance,
    almacen: &mut (impl AlmacenAvances + CategoriaDeSistema),
) -> Result<AvanceRegistrado, ErrorAplicacion> {
    let categoria = almacen.categoria_de_sistema()?;
    registrar_avance_de_efectivo(datos, categoria, almacen)
}

/// Los avances de una tarjeta, del más nuevo al más viejo.
pub fn listar_avances(tarjeta_id: i64, almacen: &impl ConsultaDeAvances) -> Result<Vec<AvanceLeido>, ErrorAplicacion> {
    Ok(almacen.avances_de_tarjeta(tarjeta_id)?)
}

/// Lo que dejó una reversión: lo deshecho y el número del caso de corrección.
#[derive(Debug, Clone, PartialEq)]
pub struct ReversionDeAvance {
    pub revertido: AvanceRevertido,
    pub caso: String,
}

/// Revierte un avance. Orden (el de siempre): el avance, el motivo, el caso de corrección y, **al final**, la reversión.
pub fn revertir_avance(
    id: i64,
    motivo: &str,
    almacen: &mut (impl AlmacenAvances + ConsultaDeAvances + RegistroDeCorrecciones),
) -> Result<ReversionDeAvance, ErrorAplicacion> {
    let avance = almacen
        .resumen_de_avance(id)?
        .ok_or(ErrorAlmacen::NoEncontrado { entidad: "avance de efectivo", id })?;
    let motivo = motivo_de_correccion(motivo)?;
    let caso = almacen.anotar_caso(&CasoAAnotar {
        tipo: "avance de efectivo".to_string(),
        referencia_id: id,
        descripcion: format!("Avance del {}", avance.fecha),
        importe: Some(avance.monto),
        divisa: Some(avance.divisa),
        motivo,
    })?;
    let revertido = revertir_avance_de_efectivo(id, almacen)?;
    Ok(ReversionDeAvance { revertido, caso })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dominio::avance::CargoDeAvance;
    use crate::dominio::dinero::{Dinero, Divisa};
    use crate::dominio::errores::ErrorDominio;
    use crate::puertos::dobles::AlmacenEnMemoria;

    const MOTIVO: &str = "Corrección de prueba del sistema";

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }

    fn almacen() -> AlmacenEnMemoria {
        AlmacenEnMemoria::nuevo()
            .con_categoria(7, "Otros")
            .con_cuenta(10, "Cuenta Avance", dop(1_000.0))
            .con_tarjeta(20, dop(5_000.0))
    }

    fn datos(monto: f64, cargo: CargoDeAvance) -> DatosAvance {
        DatosAvance { tarjeta_id: 20, cuenta_ahorro_id: 10, fecha: "01/10/2026".into(), monto: dop(monto), cargo, nota: Some("nota".into()) }
    }

    #[test]
    fn registrar_sube_la_deuda_por_importe_y_cargo_y_la_cuenta_recibe_solo_el_importe() {
        let mut a = almacen();
        let r = registrar_avance(datos(800.0, CargoDeAvance::porcentual(6.25).unwrap()), &mut a).unwrap();
        assert_eq!((r.cargo, r.a_la_tarjeta), (dop(50.0), dop(850.0)));
        assert_eq!((a.deuda_de(20), a.saldo(10).unwrap()), (dop(5_850.0), dop(1_800.0)));
        assert_eq!(a.gastos.len(), 1, "el cargo se asienta como gasto");
    }

    #[test]
    fn sin_la_categoria_de_sistema_no_se_registra_nada() {
        let mut a = AlmacenEnMemoria::nuevo().con_cuenta(10, "Cuenta", dop(1_000.0)).con_tarjeta(20, dop(5_000.0));
        assert!(registrar_avance(datos(800.0, CargoDeAvance::Exonerado), &mut a).is_err());
        assert_eq!(a.deuda_de(20), dop(5_000.0));
        assert!(a.avances.is_empty());
    }

    #[test]
    fn el_historial_va_del_mas_nuevo_al_mas_viejo_con_la_cuenta() {
        let mut a = almacen();
        let primero = registrar_avance(datos(300.0, CargoDeAvance::Exonerado), &mut a).unwrap().id;
        let segundo = registrar_avance(datos(400.0, CargoDeAvance::Exonerado), &mut a).unwrap().id;
        a.fechas_de_avance.insert(primero, "01/10/2026".into());
        a.fechas_de_avance.insert(segundo, "02/10/2026".into());
        let lista = listar_avances(20, &a).unwrap();
        assert_eq!(lista.iter().map(|x| x.id).collect::<Vec<_>>(), vec![segundo, primero]);
        assert_eq!((lista[0].cuenta_nombre.as_str(), lista[0].fecha.as_str()), ("Cuenta Avance", "02/10/2026"));
        assert!(listar_avances(99, &a).unwrap().is_empty());
    }

    #[test]
    fn revertir_baja_la_deuda_devuelve_el_importe_y_deja_un_caso() {
        let mut a = almacen();
        let id = registrar_avance(datos(800.0, CargoDeAvance::porcentual(6.25).unwrap()), &mut a).unwrap().id;
        a.fechas_de_avance.insert(id, "01/10/2026".into());
        let r = revertir_avance(id, MOTIVO, &mut a).unwrap();
        assert_eq!(r.caso, "CASO-0001");
        assert_eq!((r.revertido.deuda_restituida, r.revertido.devuelto_por_la_cuenta), (dop(850.0), dop(800.0)));
        assert_eq!((a.deuda_de(20), a.saldo(10).unwrap()), (dop(5_000.0), dop(1_000.0)));
        assert!(a.avances.is_empty() && a.gastos.is_empty(), "el avance y el gasto del cargo desaparecen");
        let c = &a.casos[0];
        assert_eq!((c.tipo.as_str(), c.descripcion.as_str(), c.importe, c.divisa.as_deref()), ("avance de efectivo", "Avance del 01/10/2026", Some(800.0), Some("DOP")));
    }

    #[test]
    fn revertir_inexistente_o_con_motivo_corto_no_abre_caso_ni_toca_nada() {
        let mut a = almacen();
        let e = revertir_avance(404, MOTIVO, &mut a).unwrap_err();
        assert_eq!(e.to_string(), "No se encontró avance de efectivo con identificador 404.");
        let id = registrar_avance(datos(800.0, CargoDeAvance::Exonerado), &mut a).unwrap().id;
        let e = revertir_avance(id, "corto", &mut a).unwrap_err();
        assert!(matches!(e, ErrorAplicacion::Dominio(ErrorDominio::MotivoInsuficiente { .. })));
        assert!(a.casos.is_empty());
        assert_eq!(a.avances.len(), 1);
    }
}
