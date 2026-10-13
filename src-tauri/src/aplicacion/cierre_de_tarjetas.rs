//! Cierre y reapertura de tarjetas: desactivar una tarjeta **sin borrarla**, trasladando antes sus suscripciones.
//!
//! Las reglas (SPEC-RC-TAR): se desactiva con sus dos balances exactamente en cero —deuda y saldo a favor son bloqueos
//! distintos—; cada suscripción vinculada se traslada a una tarjeta destino activa y distinta, elegida por el usuario
//! (la aplicación solo sugiere, con motivo); y todo ocurre en **una** transacción: o se traslada todo y se desactiva, o
//! no cambia nada. Se conservan el identificador, las fechas y la frecuencia de cada suscripción, y los gastos ya
//! asentados siguen vinculados a la tarjeta original. Al reactivar, ninguna suscripción vuelve sola.

use super::ErrorAplicacion;
use crate::dominio::cierre_tarjeta::{recomendar, DestinoCandidato, Sugerencia, SuscripcionAMover};
use crate::dominio::dinero::{Dinero, Divisa};
use crate::dominio::errores::ErrorDominio;
use crate::dominio::suscripcion::fecha_desde_texto;
use crate::puertos::repositorios::*;
use std::collections::{HashMap, HashSet};

/// Una tarjeta ofrecida como destino, con su cupo en cada divisa.
#[derive(Debug, Clone, PartialEq)]
pub struct DestinoDeVista {
    pub id: i64,
    pub nombre: String,
    pub conserva_dolares: bool,
    pub disponible_dop: Dinero,
    pub disponible_usd: Dinero,
}

/// Una suscripción que hay que trasladar, con lo que se sugiere para ella.
#[derive(Debug, Clone, PartialEq)]
pub struct LineaDeTraslado {
    pub suscripcion: SuscripcionRegistrada,
    pub sugerencia: Sugerencia,
}

/// Lo que ve el usuario antes de confirmar. Solo lectura: la validación autoritativa se repite al confirmar.
#[derive(Debug, Clone, PartialEq)]
pub struct VistaPreviaDeCierre {
    pub tarjeta_id: i64,
    pub nombre: String,
    pub activa: bool,
    pub balance_dop: Dinero,
    pub balance_usd: Dinero,
    /// Un mensaje por balance distinto de cero (deuda o saldo a favor). Con alguno, no se puede confirmar.
    pub bloqueos: Vec<String>,
    pub lineas: Vec<LineaDeTraslado>,
    pub destinos: Vec<DestinoDeVista>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResumenDeCierre {
    pub nombre: String,
    pub trasladadas: usize,
}

fn nombre_de(t: &TarjetaParaCierre) -> String {
    format!("{} {}", t.entidad, t.nombre_tarjeta)
}

fn bloqueo_de_balance(t: &TarjetaParaCierre, balance: Dinero) -> Option<ErrorDominio> {
    if balance.es_cero() {
        return None;
    }
    Some(ErrorDominio::TarjetaConBalance {
        nombre: nombre_de(t),
        divisa: balance.divisa().codigo(),
        importe: format!("{:.2}", balance.unidades().abs()),
        a_favor: balance.es_negativo(),
    })
}

fn bloqueos(t: &TarjetaParaCierre) -> Vec<ErrorDominio> {
    [t.cupo_dop.balance, t.cupo_usd.balance].into_iter().filter_map(|b| bloqueo_de_balance(t, b)).collect()
}

fn registrada_a_mover(s: &SuscripcionRegistrada) -> Result<SuscripcionAMover, ErrorAplicacion> {
    let divisa = Divisa::desde_codigo(&s.divisa)?;
    Ok(SuscripcionAMover {
        id: s.id,
        monto: Dinero::nuevo(s.monto, divisa)?,
        proximo_cobro: s.fecha_proximo_cobro.as_deref().and_then(fecha_desde_texto),
    })
}

/// Qué se le muestra al usuario al elegir «Desactivar tarjeta».
pub fn previsualizar_cierre(
    tarjeta_id: i64,
    almacen: &(impl TarjetasParaCierre + AlmacenSuscripciones),
) -> Result<VistaPreviaDeCierre, ErrorAplicacion> {
    let tarjetas = almacen.tarjetas_para_cierre()?;
    let origen = tarjetas
        .iter()
        .find(|t| t.id == tarjeta_id)
        .ok_or(ErrorAlmacen::NoEncontrado { entidad: "tarjeta", id: tarjeta_id })?;

    let vinculadas: Vec<SuscripcionRegistrada> =
        almacen.suscripciones_registradas()?.into_iter().filter(|s| s.tarjeta_id == tarjeta_id).collect();
    let a_mover = vinculadas.iter().map(registrada_a_mover).collect::<Result<Vec<_>, _>>()?;

    let candidatos: Vec<&TarjetaParaCierre> = tarjetas.iter().filter(|t| t.activa && t.id != tarjeta_id).collect();
    let sugerencias = recomendar(
        &a_mover,
        &candidatos
            .iter()
            .map(|t| DestinoCandidato {
                id: t.id,
                dop: t.cupo_dop,
                usd: t.cupo_usd,
                politica: t.politica,
                dia_corte: t.fecha_corte,
            })
            .collect::<Vec<_>>(),
    );

    Ok(VistaPreviaDeCierre {
        tarjeta_id,
        nombre: nombre_de(origen),
        activa: origen.activa,
        balance_dop: origen.cupo_dop.balance,
        balance_usd: origen.cupo_usd.balance,
        bloqueos: bloqueos(origen).iter().map(|e| e.to_string()).collect(),
        lineas: vinculadas.into_iter().zip(sugerencias).map(|(suscripcion, sugerencia)| LineaDeTraslado { suscripcion, sugerencia }).collect(),
        destinos: candidatos
            .iter()
            .map(|t| DestinoDeVista {
                id: t.id,
                nombre: nombre_de(t),
                conserva_dolares: t.politica == crate::dominio::tarjeta::PoliticaLiquidacion::EnDivisaDeOrigen,
                disponible_dop: Dinero::de_centavos(t.cupo_dop.disponible(), Divisa::Dop),
                disponible_usd: Dinero::de_centavos(t.cupo_usd.disponible(), Divisa::Usd),
            })
            .collect(),
    })
}

/// Desactiva la tarjeta trasladando sus suscripciones. **Todo o nada**: quien llama abre una transacción de escritura y
/// la deshace si esto devuelve un error. `hoy` es la fecha que queda en la bitácora de traslados (`dd/mm/aaaa`).
///
/// Orden de las comprobaciones: la tarjeta existe y está activa → balances en cero (primero pesos, luego dólares) →
/// la selección cubre **exactamente** las suscripciones que hay ahora → cada destino existe, está activo y es otra
/// tarjeta → el cupo de cada destino alcanza para todo lo que se le asigna, por divisa → se traslada y se desactiva.
pub fn cerrar_tarjeta(
    tarjeta_id: i64,
    traslados: &[(i64, i64)],
    hoy: &str,
    almacen: &mut (impl TarjetasParaCierre + AlmacenSuscripciones),
) -> Result<ResumenDeCierre, ErrorAplicacion> {
    let tarjetas = almacen.tarjetas_para_cierre()?;
    let origen = tarjetas
        .iter()
        .find(|t| t.id == tarjeta_id)
        .ok_or(ErrorAlmacen::NoEncontrado { entidad: "tarjeta", id: tarjeta_id })?;
    if !origen.activa {
        return Err(ErrorDominio::TarjetaYaInactiva { nombre: nombre_de(origen) }.into());
    }
    if let Some(e) = bloqueos(origen).into_iter().next() {
        return Err(e.into());
    }

    let vinculadas: Vec<SuscripcionRegistrada> =
        almacen.suscripciones_registradas()?.into_iter().filter(|s| s.tarjeta_id == tarjeta_id).collect();
    let elegidas: HashMap<i64, i64> = traslados.iter().copied().collect();
    let ids_vinculadas: HashSet<i64> = vinculadas.iter().map(|s| s.id).collect();
    let faltan: Vec<String> =
        vinculadas.iter().filter(|s| !elegidas.contains_key(&s.id)).map(|s| s.plataforma.clone()).collect();
    let sobran = elegidas.keys().filter(|id| !ids_vinculadas.contains(id)).count();
    if !faltan.is_empty() || sobran > 0 {
        return Err(ErrorDominio::TrasladosNoCoinciden { faltan, sobran }.into());
    }

    let mut asignado: HashMap<(i64, Divisa), i64> = HashMap::new();
    for s in &vinculadas {
        let destino_id = elegidas[&s.id];
        let invalido = |motivo: &'static str| -> ErrorAplicacion {
            ErrorDominio::DestinoDeTrasladoInvalido { plataforma: s.plataforma.clone(), motivo }.into()
        };
        if destino_id == tarjeta_id {
            return Err(invalido("es la misma tarjeta que se cierra"));
        }
        let destino = tarjetas.iter().find(|t| t.id == destino_id).ok_or_else(|| invalido("no existe"))?;
        if !destino.activa {
            return Err(invalido("está desactivada"));
        }
        let divisa = Divisa::desde_codigo(&s.divisa)?;
        *asignado.entry((destino_id, divisa)).or_insert(0) += Dinero::nuevo(s.monto, divisa)?.centavos();
    }

    // Cupo agregado por divisa: la suma de lo asignado a cada destino, no cada suscripción por separado.
    let mut por_destino: Vec<_> = asignado.iter().collect();
    por_destino.sort_by_key(|((id, divisa), _)| (*id, divisa.codigo()));
    for ((destino_id, divisa), total) in por_destino {
        let destino = tarjetas.iter().find(|t| t.id == *destino_id).expect("validado arriba");
        let cupo = match divisa {
            Divisa::Dop => destino.cupo_dop,
            Divisa::Usd => destino.cupo_usd,
        };
        if *total > cupo.disponible() {
            return Err(ErrorDominio::DestinoSinCupo {
                destino: nombre_de(destino),
                divisa: divisa.codigo(),
                falta: format!("{:.2}", Dinero::de_centavos(*total - cupo.disponible(), *divisa).unidades()),
            }
            .into());
        }
    }

    for s in &vinculadas {
        if !almacen.trasladar_suscripcion(s.id, tarjeta_id, elegidas[&s.id], hoy)? {
            return Err(ErrorDominio::TrasladoPerdido { plataforma: s.plataforma.clone() }.into());
        }
    }
    if !almacen.desactivar_tarjeta(tarjeta_id)? {
        // Algo cambió entre la lectura y la escritura: se dice la causa verdadera y el llamador deshace todo.
        let ahora = almacen.tarjetas_para_cierre()?.into_iter().find(|t| t.id == tarjeta_id);
        return Err(match ahora {
            Some(t) if !t.activa => ErrorDominio::TarjetaYaInactiva { nombre: nombre_de(&t) }.into(),
            Some(t) => bloqueos(&t).into_iter().next().map(Into::into).unwrap_or_else(|| {
                ErrorAplicacion::Almacen(ErrorAlmacen::Fallo("no se pudo desactivar la tarjeta".into()))
            }),
            None => ErrorAlmacen::NoEncontrado { entidad: "tarjeta", id: tarjeta_id }.into(),
        });
    }
    Ok(ResumenDeCierre { nombre: nombre_de(origen), trasladadas: vinculadas.len() })
}

/// Reactiva una tarjeta desactivada. **No** recupera sus suscripciones: seguirán donde se trasladaron hasta que el
/// usuario las reasigne a mano.
pub fn reactivar_tarjeta(tarjeta_id: i64, almacen: &mut impl TarjetasParaCierre) -> Result<(), ErrorAplicacion> {
    let tarjetas = almacen.tarjetas_para_cierre()?;
    let t = tarjetas
        .iter()
        .find(|t| t.id == tarjeta_id)
        .ok_or(ErrorAlmacen::NoEncontrado { entidad: "tarjeta", id: tarjeta_id })?;
    if !almacen.reactivar_tarjeta(tarjeta_id)? {
        return Err(ErrorDominio::TarjetaNoInactiva { nombre: nombre_de(t) }.into());
    }
    Ok(())
}

/// Exige que la tarjeta esté activa antes de una operación **nueva** con ella (consumo, abono, avance, bonificación o
/// alta de una suscripción). Es la comprobación temprana que da el mensaje; la barrera de integridad está en
/// `ajustar_deuda`.
pub fn exigir_tarjeta_activa(almacen: &impl RepositorioTarjetas, tarjeta_id: i64) -> Result<(), ErrorAplicacion> {
    let estado = match almacen.estado_de_tarjeta(tarjeta_id) {
        Ok(e) => e,
        // Una tarjeta que no existe la dice, con su propio mensaje, quien la busca a continuación.
        Err(ErrorAlmacen::NoEncontrado { .. }) => return Ok(()),
        Err(otro) => return Err(otro.into()),
    };
    if !estado.activa {
        return Err(ErrorAplicacion::Almacen(ErrorAlmacen::TarjetaInactiva { nombre: estado.nombre }));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aplicacion::suscripciones::{crear_suscripcion, DatosSuscripcion};
    use crate::puertos::dobles::AlmacenEnMemoria;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }
    fn usd(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Usd).unwrap()
    }

    /// Tarjeta 1 se cierra; 2 y 3 son destinos con cupo de 1 000 DOP y 500 USD.
    fn almacen() -> AlmacenEnMemoria {
        AlmacenEnMemoria::nuevo()
            .con_tarjeta(1, dop(0.0))
            .con_tarjeta(2, dop(0.0))
            .con_tarjeta(3, dop(0.0))
            .con_cupo(2, Divisa::Dop, 1_000.0, 0.0)
            .con_cupo(3, Divisa::Dop, 1_000.0, 0.0)
            .con_cupo(2, Divisa::Usd, 500.0, 0.0)
            .con_cupo(3, Divisa::Usd, 500.0, 0.0)
    }

    fn sus(a: &mut AlmacenEnMemoria, nombre: &str, monto: Dinero, tarjeta: i64) -> i64 {
        crear_suscripcion(
            DatosSuscripcion {
                plataforma: nombre.into(),
                monto,
                tarjeta_id: tarjeta,
                frecuencia: "mensual".into(),
                dia_facturacion: 15,
                fecha_proximo_cobro: Some("15/03/2026".into()),
            },
            a,
        )
        .unwrap()
    }

    fn dominio(e: ErrorAplicacion) -> ErrorDominio {
        match e {
            ErrorAplicacion::Dominio(d) => d,
            otro => panic!("se esperaba un error de dominio y fue {otro:?}"),
        }
    }

    #[test]
    fn se_desactiva_sin_suscripciones_y_conserva_la_tarjeta() {
        let mut a = almacen();
        let r = cerrar_tarjeta(1, &[], "14/10/2026", &mut a).unwrap();
        assert_eq!((r.nombre.as_str(), r.trasladadas), ("Banco Tarjeta 1", 0));
        assert!(a.inactivas.contains(&1) && a.tarjetas.contains(&1));
    }

    #[test]
    fn deuda_y_saldo_a_favor_son_bloqueos_distintos_con_su_importe_y_divisa() {
        let mut a = almacen().con_tarjeta(4, dop(12.5)).con_tarjeta(5, usd(-3.0));
        let con_suscripcion = sus(&mut a, "Se queda", dop(10.0), 4);
        let e = dominio(cerrar_tarjeta(4, &[], "14/10/2026", &mut a).unwrap_err());
        assert!(matches!(e, ErrorDominio::TarjetaConBalance { divisa: "DOP", a_favor: false, .. }), "{e:?}");
        assert!(e.to_string().contains("debe DOP 12.50"));
        let e = dominio(cerrar_tarjeta(5, &[], "14/10/2026", &mut a).unwrap_err());
        assert!(e.to_string().contains("tiene USD 3.00 a favor del titular"), "{e}");
        let v = previsualizar_cierre(5, &a).unwrap();
        assert_eq!(v.bloqueos.len(), 1, "un mensaje por balance distinto de cero");
        assert!(a.inactivas.is_empty());
        assert_eq!(a.suscripciones[0].datos.tarjeta_id, 4, "ni siquiera se tocó la suscripción {con_suscripcion}");
    }

    #[test]
    fn el_cupo_se_comprueba_sumando_lo_asignado_a_cada_destino_por_divisa() {
        let mut a = almacen();
        let s1 = sus(&mut a, "A", dop(600.0), 1);
        let s2 = sus(&mut a, "B", dop(600.0), 1);
        let s3 = sus(&mut a, "Nube", usd(400.0), 1);
        // Cada una cabe sola en la 2 (cupo 1 000), pero las dos juntas no.
        let e = dominio(cerrar_tarjeta(1, &[(s1, 2), (s2, 2), (s3, 2)], "14/10/2026", &mut a).unwrap_err());
        assert!(matches!(e, ErrorDominio::DestinoSinCupo { divisa: "DOP", .. }), "{e:?}");
        assert!(e.to_string().contains("faltan DOP 200.00"), "{e}");
        // Repartidas, sí; y los dólares solo cuentan contra el cupo en dólares.
        assert!(cerrar_tarjeta(1, &[(s1, 2), (s2, 3), (s3, 2)], "14/10/2026", &mut a).is_ok());
        assert_eq!(a.suscripciones.iter().map(|s| s.datos.tarjeta_id).collect::<Vec<_>>(), vec![2, 3, 2]);
    }

    #[test]
    fn la_seleccion_debe_cubrir_exactamente_las_suscripciones_de_la_tarjeta() {
        let mut a = almacen();
        let s1 = sus(&mut a, "A", dop(10.0), 1);
        let s2 = sus(&mut a, "B", dop(10.0), 1);
        let otra = sus(&mut a, "De otra", dop(10.0), 2);
        let e = dominio(cerrar_tarjeta(1, &[(s1, 2)], "14/10/2026", &mut a).unwrap_err());
        assert!(e.to_string().contains("Falta elegir una tarjeta destino para: B"), "{e}");
        let e = dominio(cerrar_tarjeta(1, &[(s1, 2), (s2, 2), (otra, 3)], "14/10/2026", &mut a).unwrap_err());
        assert!(matches!(e, ErrorDominio::TrasladosNoCoinciden { sobran: 1, .. }), "{e:?}");
        assert!(a.inactivas.is_empty() && a.suscripciones[0].datos.tarjeta_id == 1, "nada cambió");
    }

    #[test]
    fn un_destino_desactivado_igual_a_la_tarjeta_o_inexistente_se_rechaza_con_su_motivo() {
        let mut a = almacen().con_tarjeta_inactiva(3);
        let s = sus(&mut a, "A", dop(10.0), 1);
        for (destino, motivo) in [(1, "es la misma tarjeta"), (3, "está desactivada"), (99, "no existe")] {
            let e = cerrar_tarjeta(1, &[(s, destino)], "14/10/2026", &mut a).unwrap_err().to_string();
            assert!(e.contains(motivo), "{destino}: {e}");
        }
        assert!(a.suscripciones[0].datos.tarjeta_id == 1 && !a.inactivas.contains(&1));
    }

    #[test]
    fn reactivar_solo_cambia_el_estado_y_una_activa_lo_dice() {
        let mut a = almacen();
        let s = sus(&mut a, "A", dop(10.0), 1);
        cerrar_tarjeta(1, &[(s, 2)], "14/10/2026", &mut a).unwrap();
        reactivar_tarjeta(1, &mut a).unwrap();
        assert!(!a.inactivas.contains(&1));
        assert_eq!(a.suscripciones[0].datos.tarjeta_id, 2, "la suscripción no vuelve sola");
        let e = reactivar_tarjeta(1, &mut a).unwrap_err().to_string();
        assert_eq!(e, "La tarjeta «Banco Tarjeta 1» no está desactivada.");
    }

    #[test]
    fn la_vista_previa_ofrece_solo_destinos_activos_y_distintos_del_origen() {
        let mut a = almacen().con_tarjeta_inactiva(3);
        sus(&mut a, "A", dop(10.0), 1);
        let v = previsualizar_cierre(1, &a).unwrap();
        assert_eq!(v.destinos.iter().map(|d| d.id).collect::<Vec<_>>(), vec![2]);
        assert_eq!(v.lineas.len(), 1);
        assert_eq!(v.lineas[0].sugerencia.destino_id, Some(2));
        assert_eq!(v.destinos[0].disponible_dop, dop(1_000.0));
    }

    #[test]
    fn exigir_tarjeta_activa_rechaza_la_desactivada_y_deja_pasar_la_inexistente() {
        let a = almacen().con_tarjeta_inactiva(3);
        assert!(exigir_tarjeta_activa(&a, 2).is_ok());
        assert_eq!(
            exigir_tarjeta_activa(&a, 3).unwrap_err().to_string(),
            "La tarjeta «Banco Tarjeta 3» está desactivada: reactívala antes de usarla en una operación."
        );
        assert!(exigir_tarjeta_activa(&a, 99).is_ok(), "la dice quien la busca después");
    }
}
