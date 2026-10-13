//! Casos de uso de las suscripciones: darlas de alta y editarlas, listarlas con su aviso, cobrarlas solas o confirmar
//! sus períodos pendientes.
//!
//! Lo fijan las pruebas de caracterización `s1`–`s25` y `su1`–`su4`. El cargo es un gasto con tarjeta (pasa por
//! `cobrar_suscripcion`, que hereda la política de la tarjeta); aquí se decide **cuándo** y se **avanza la fecha** del
//! próximo cobro desde el vencimiento saldado, no desde hoy, para que abrir la aplicación tarde no corra el calendario.

use super::cobrar_suscripcion::{cobrar_suscripcion, DatosCobro};
use super::ErrorAplicacion;
use crate::dominio::correccion::motivo_de_correccion;
use crate::dominio::dinero::{Dinero, Divisa};
use crate::dominio::errores::ErrorDominio;
use crate::dominio::suscripcion::{
    condiciones_de_suscripcion, fecha_de_correccion, regla_de_un_registro, Suscripcion as ReglaDeSuscripcion,
};
use crate::puertos::repositorios::*;
use chrono::NaiveDate;

/// Las condiciones de una suscripción nueva o editada. La fecha del próximo cobro ya viene validada
/// (`dominio::suscripcion::proximo_cobro_declarado`).
pub struct DatosSuscripcion {
    pub plataforma: String,
    pub monto: Dinero,
    pub tarjeta_id: i64,
    pub frecuencia: String,
    pub dia_facturacion: i32,
    pub fecha_proximo_cobro: Option<String>,
}

fn a_guardar(datos: DatosSuscripcion) -> Result<SuscripcionAGuardar, ErrorAplicacion> {
    condiciones_de_suscripcion(datos.monto, &datos.frecuencia, datos.dia_facturacion)?;
    Ok(SuscripcionAGuardar {
        plataforma: datos.plataforma,
        monto: datos.monto.unidades(),
        tarjeta_id: datos.tarjeta_id,
        frecuencia: datos.frecuencia,
        dia_facturacion: datos.dia_facturacion,
        divisa: datos.monto.divisa().codigo().to_string(),
        fecha_proximo_cobro: datos.fecha_proximo_cobro,
    })
}

pub fn crear_suscripcion(
    datos: DatosSuscripcion,
    almacen: &mut (impl AlmacenSuscripciones + RepositorioTarjetas),
) -> Result<i64, ErrorAplicacion> {
    let guardar = a_guardar(datos)?;
    super::cierre_de_tarjetas::exigir_tarjeta_activa(almacen, guardar.tarjeta_id)?;
    Ok(almacen.insertar_suscripcion(&guardar)?)
}

/// Edita una suscripción **conservando `fecha_ultimo_pago`**: la única alternativa era borrarla y volver a crearla, lo
/// que reinicia el marcador y hace que el siguiente procesamiento cobre otra vez el mismo mes. Los cargos ya realizados
/// son gastos independientes y no se tocan.
pub fn editar_suscripcion(
    id: i64,
    datos: DatosSuscripcion,
    almacen: &mut (impl AlmacenSuscripciones + RepositorioTarjetas),
) -> Result<(), ErrorAplicacion> {
    let guardar = a_guardar(datos)?;
    super::cierre_de_tarjetas::exigir_tarjeta_activa(almacen, guardar.tarjeta_id)?;
    if !almacen.editar_suscripcion(id, &guardar)? {
        return Err(ErrorDominio::SuscripcionNoEncontradaParaEditar.into());
    }
    Ok(())
}

/// Pone a mano la fecha del próximo cobro: la salida cuando una suscripción se queda sin fecha y por tanto parada.
pub fn corregir_proximo_cobro(id: i64, fecha: &str, almacen: &mut impl AlmacenSuscripciones) -> Result<(), ErrorAplicacion> {
    let fecha = fecha_de_correccion(fecha)?;
    if !almacen.fijar_proximo_cobro(id, &fecha)? {
        return Err(ErrorDominio::SuscripcionNoEncontradaParaCorregir.into());
    }
    Ok(())
}

pub fn eliminar_suscripcion(id: i64, almacen: &mut impl AlmacenSuscripciones) -> Result<(), ErrorAplicacion> {
    Ok(almacen.eliminar_suscripcion(id)?)
}

/// Una suscripción con lo que el listado le añade: el aviso, lo que la frena y los períodos por confirmar.
#[derive(Debug, Clone, PartialEq)]
pub struct SuscripcionConAvisos {
    pub leida: SuscripcionLeida,
    pub avisa: bool,
    pub impedimento: Option<String>,
    pub pendientes: Vec<String>,
}

/// Las suscripciones por nombre, con el aviso resuelto para ese `hoy`. Es una regla, y las reglas no viven en el HTML.
pub fn listar_suscripciones(
    hoy: NaiveDate,
    almacen: &impl AlmacenSuscripciones,
) -> Result<Vec<SuscripcionConAvisos>, ErrorAplicacion> {
    Ok(almacen
        .suscripciones_con_tarjeta()?
        .into_iter()
        .map(|s| {
            let regla = regla_de_un_registro(&s.frecuencia, s.fecha_proximo_cobro.as_deref(), s.dia_facturacion);
            let avisa = regla.as_ref().is_some_and(|r| r.avisa(hoy));
            let impedimento = regla.as_ref().and_then(|r| r.impedimento()).map(|i| i.explicacion().to_string());
            let pendientes = regla
                .as_ref()
                .map(|r| r.pendientes_de_confirmar(hoy).iter().map(|f| f.format("%d/%m/%Y").to_string()).collect())
                .unwrap_or_default();
            SuscripcionConAvisos { leida: s, avisa, impedimento, pendientes }
        })
        .collect())
}

/// Dónde va el gasto de una suscripción: «Suscripciones» y, si no está, «Otros». El titular puede renombrarlas o
/// borrarlas desde la propia aplicación; si ninguna existe se **crea** «Suscripciones» en el momento, en lugar de caer
/// en el identificador 1 literal, que hoy puede ser cualquier otra categoría.
pub fn categoria_de_suscripciones(almacen: &mut impl AlmacenSuscripciones) -> Result<i64, ErrorAplicacion> {
    for nombre in ["suscripciones", "otros"] {
        if let Some(id) = almacen.categoria_por_nombre(nombre)? {
            return Ok(id);
        }
    }
    Ok(almacen.crear_categoria("Suscripciones")?)
}

/// Una suscripción que **parece** tocar cobrarse hoy: su identificador y su nombre (para poder decir cuál falló).
#[derive(Debug, Clone, PartialEq)]
pub struct CandidataACobro {
    pub id: i64,
    pub plataforma: String,
}

/// Las suscripciones que **parecen** tocar cobrarse hoy: **solo si hay exactamente un período vencido**. Con varios, la
/// aplicación no sabe si el proveedor los cobró ni si la suscripción siguió activa: se ofrecen para confirmar
/// (`confirmar_pendiente`) en vez de fabricarse.
///
/// Es una lista de **candidatas** de solo lectura, y nada más: puede quedar vieja antes de que se asiente el primer
/// cargo. Quien cobra es `cobrar_si_vence`, que vuelve a leer la suscripción dentro de la transacción de su propio cargo
/// y decide con lo que hay en ese momento.
pub fn cobros_automaticos(
    hoy: NaiveDate,
    almacen: &impl AlmacenSuscripciones,
) -> Result<Vec<CandidataACobro>, ErrorAplicacion> {
    Ok(almacen
        .suscripciones_registradas()?
        .into_iter()
        .filter(|s| {
            regla_de_un_registro(&s.frecuencia, s.fecha_proximo_cobro.as_deref(), s.dia_facturacion)
                .is_some_and(|r| r.cobro_automatico(hoy).is_some())
        })
        .map(|s| CandidataACobro { id: s.id, plataforma: s.plataforma })
        .collect())
}

/// Lo que de verdad se cobró, leído de la suscripción **en el momento del cargo**.
#[derive(Debug, Clone, PartialEq)]
pub struct CargoAsentado {
    pub plataforma: String,
    pub divisa: String,
    pub monto: f64,
    pub vencimiento: NaiveDate,
}

/// Cobra la suscripción `id` si **sigue** venciendo hoy. Relee todo dentro de la transacción de quien llama: una
/// suscripción borrada, editada (otra tarjeta, otro importe), ya cobrada por otra vía o con varios períodos acumulados
/// desde que se hizo la lista de candidatas no carga la tarjeta antigua ni cobra dos veces. `None` si ya no toca.
pub fn cobrar_si_vence(
    id: i64,
    hoy: NaiveDate,
    categoria: i64,
    almacen: &mut (impl AlmacenGastos + AlmacenSuscripciones),
) -> Result<Option<CargoAsentado>, ErrorAplicacion> {
    let Some(sub) = almacen.suscripciones_registradas()?.into_iter().find(|s| s.id == id) else { return Ok(None) };
    let Some(regla) = regla_de_un_registro(&sub.frecuencia, sub.fecha_proximo_cobro.as_deref(), sub.dia_facturacion) else {
        return Ok(None);
    };
    let Some(vencimiento) = regla.cobro_automatico(hoy) else { return Ok(None) };
    asentar_cargo(&sub, &regla, vencimiento, categoria, almacen)?;
    Ok(Some(CargoAsentado { plataforma: sub.plataforma, divisa: sub.divisa, monto: sub.monto, vencimiento }))
}

/// Un cobro automático que no pudo asentarse, con la causa.
#[derive(Debug, Clone, PartialEq)]
pub struct FalloDeCobro {
    pub plataforma: String,
    pub motivo: String,
}

/// Lo que dejó el cobro automático de una pasada: los cargos asentados y los que fallaron.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CobrosAutomaticos {
    pub cargos: Vec<CargoAsentado>,
    pub fallos: Vec<FalloDeCobro>,
}

/// Recorre las candidatas y cobra cada una con `cobrar` (que abre **su propia transacción**). **Un fallo no corta el
/// recorrido**: ese cargo se deshace entero (su transacción no se confirma, y su puntero no avanza, de modo que se
/// reintenta al siguiente arranque) y las demás suscripciones se cobran igual. Los fallos se devuelven con su causa para
/// que la interfaz los muestre como errores y no como cargos.
pub fn procesar_cobros<E: std::fmt::Display>(
    candidatas: Vec<CandidataACobro>,
    mut cobrar: impl FnMut(i64) -> Result<Option<CargoAsentado>, E>,
) -> CobrosAutomaticos {
    let mut resultado = CobrosAutomaticos::default();
    for candidata in candidatas {
        match cobrar(candidata.id) {
            Ok(Some(cargo)) => resultado.cargos.push(cargo),
            Ok(None) => {}
            Err(e) => resultado.fallos.push(FalloDeCobro { plataforma: candidata.plataforma, motivo: e.to_string() }),
        }
    }
    resultado
}

/// Mueve la fecha del próximo cobro al período siguiente, **desde el vencimiento saldado** y no desde hoy.
fn avanzar_puntero(
    almacen: &mut impl AlmacenSuscripciones,
    id: i64,
    regla: &ReglaDeSuscripcion,
    saldado: NaiveDate,
    marca_de_cobro: Option<&str>,
) -> Result<(), ErrorAplicacion> {
    let siguiente = regla.siguiente_vencimiento(saldado).map(|f| f.format("%d/%m/%Y").to_string());
    Ok(almacen.mover_puntero(id, siguiente.as_deref(), marca_de_cobro)?)
}

/// Asienta un cargo **con la fecha de su vencimiento** (no la del día en que se ejecuta) y avanza el puntero. Es la
/// misma función para el cobro automático y para confirmar un período pendiente: los dos caminos no pueden divergir.
pub fn asentar_cargo(
    suscripcion: &SuscripcionRegistrada,
    regla: &ReglaDeSuscripcion,
    vencimiento: NaiveDate,
    categoria: i64,
    almacen: &mut (impl AlmacenGastos + AlmacenSuscripciones),
) -> Result<(), ErrorAplicacion> {
    let fecha = vencimiento.format("%d/%m/%Y").to_string();
    // El cobro es un consumo con tarjeta: pasa por el mismo caso de uso que cualquier gasto y hereda su regla de divisa
    // y la política de la tarjeta (un consumo en divisa puede quedar pendiente de liquidar).
    let divisa = Divisa::desde_codigo(&suscripcion.divisa)?;
    let monto = Dinero::nuevo(suscripcion.monto, divisa)?;
    cobrar_suscripcion(
        DatosCobro {
            plataforma: suscripcion.plataforma.clone(),
            monto,
            tarjeta_id: suscripcion.tarjeta_id,
            fecha: fecha.clone(),
            categoria_id: categoria,
        },
        almacen,
    )?;
    avanzar_puntero(almacen, suscripcion.id, regla, vencimiento, Some(&fecha))
}

/// Lo que dejó confirmar un período pendiente.
#[derive(Debug, Clone, PartialEq)]
pub enum Confirmado {
    /// Se asentó el cargo de ese período.
    Asentado { plataforma: String, vencimiento: NaiveDate },
    /// Se dio por no cobrado ese período, con ese caso de corrección.
    Descartado { vencimiento: NaiveDate, caso: String },
}

/// Confirma (o descarta, si hay motivo) el período pendiente **más antiguo**, y solo si de verdad hay varios: con uno, el
/// cobro automático es quien debe encargarse. Descartar exige un motivo escrito: es afirmar que el proveedor no lo cobró.
pub fn confirmar_pendiente(
    id: i64,
    hoy: NaiveDate,
    motivo_de_descarte: Option<&str>,
    categoria: i64,
    almacen: &mut (impl AlmacenGastos + AlmacenSuscripciones + RegistroDeCorrecciones),
) -> Result<Confirmado, ErrorAplicacion> {
    let sub = almacen
        .suscripciones_registradas()?
        .into_iter()
        .find(|s| s.id == id)
        .ok_or(ErrorDominio::SuscripcionNoEncontrada)?;
    let regla = regla_de_un_registro(&sub.frecuencia, sub.fecha_proximo_cobro.as_deref(), sub.dia_facturacion)
        .ok_or(ErrorDominio::FrecuenciaNoReconocida)?;
    let vencimiento = *regla.pendientes_de_confirmar(hoy).first().ok_or(ErrorDominio::SinPeriodosPendientes)?;

    match motivo_de_descarte {
        None => {
            asentar_cargo(&sub, &regla, vencimiento, categoria, almacen)?;
            Ok(Confirmado::Asentado { plataforma: sub.plataforma, vencimiento })
        }
        Some(motivo) => {
            let motivo = motivo_de_correccion(motivo)?;
            let caso = almacen.anotar_caso(&CasoAAnotar {
                tipo: "período de suscripción".to_string(),
                referencia_id: sub.id,
                descripcion: format!("{} — período del {}", sub.plataforma, vencimiento.format("%d/%m/%Y")),
                importe: Some(sub.monto),
                divisa: Some(sub.divisa.clone()),
                motivo,
            })?;
            avanzar_puntero(almacen, sub.id, &regla, vencimiento, None)?;
            Ok(Confirmado::Descartado { vencimiento, caso })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::puertos::dobles::AlmacenEnMemoria;

    const MOTIVO: &str = "El proveedor no lo cobró este mes";

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }
    fn fecha(a: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(a, m, d).unwrap()
    }

    fn almacen() -> AlmacenEnMemoria {
        AlmacenEnMemoria::nuevo().con_categoria(7, "Otros").con_tarjeta(20, dop(0.0))
    }

    fn datos(plataforma: &str, frecuencia: &str, dia: i32, proximo: Option<&str>) -> DatosSuscripcion {
        DatosSuscripcion {
            plataforma: plataforma.into(),
            monto: dop(500.0),
            tarjeta_id: 20,
            frecuencia: frecuencia.into(),
            dia_facturacion: dia,
            fecha_proximo_cobro: proximo.map(str::to_string),
        }
    }

    fn dominio(e: ErrorAplicacion) -> ErrorDominio {
        match e {
            ErrorAplicacion::Dominio(d) => d,
            otro => panic!("se esperaba un error de dominio y fue {otro:?}"),
        }
    }

    #[test]
    fn crear_guarda_las_condiciones_y_rechaza_en_su_orden_sin_dejar_nada() {
        let mut a = almacen();
        let id = crear_suscripcion(datos("Netflix", "mensual", 5, Some("05/03/2026")), &mut a).unwrap();
        let s = &a.suscripciones[0].datos;
        assert_eq!((s.id, s.plataforma.as_str(), s.divisa.as_str(), s.monto), (id, "Netflix", "DOP", 500.0));
        let mal = |d: DatosSuscripcion| dominio(crear_suscripcion(d, &mut almacen()).unwrap_err());
        assert_eq!(mal(DatosSuscripcion { monto: dop(0.0), ..datos("X", "semanal", 0, None) }), ErrorDominio::SuscripcionSinImporte);
        assert_eq!(mal(datos("X", "semanal", 0, None)), ErrorDominio::FrecuenciaDesconocida { codigo: "semanal".into() });
        assert_eq!(mal(datos("X", "mensual", 32, None)), ErrorDominio::DiaDeFacturacionInvalido { dia: 32 });
    }

    #[test]
    fn editar_conserva_el_ultimo_pago_y_una_inexistente_dice_su_mensaje() {
        let mut a = almacen();
        let id = crear_suscripcion(datos("Netflix", "mensual", 5, Some("05/03/2026")), &mut a).unwrap();
        a.suscripciones[0].fecha_ultimo_pago = Some("05/02/2026".into());
        editar_suscripcion(id, datos("Netflix Premium", "mensual", 6, Some("06/03/2026")), &mut a).unwrap();
        assert_eq!(a.suscripciones[0].datos.plataforma, "Netflix Premium");
        assert_eq!(a.suscripciones[0].fecha_ultimo_pago.as_deref(), Some("05/02/2026"), "el marcador no se reinicia");
        let e = editar_suscripcion(999, datos("X", "mensual", 5, None), &mut a).unwrap_err();
        assert_eq!(dominio(e), ErrorDominio::SuscripcionNoEncontradaParaEditar);
    }

    #[test]
    fn corregir_el_proximo_cobro_exige_una_fecha_legible_y_una_suscripcion_que_exista() {
        let mut a = almacen();
        let id = crear_suscripcion(datos("Netflix", "mensual", 5, None), &mut a).unwrap();
        corregir_proximo_cobro(id, " 15/03/2026 ", &mut a).unwrap();
        assert_eq!(a.suscripciones[0].datos.fecha_proximo_cobro.as_deref(), Some("15/03/2026"));
        assert_eq!(dominio(corregir_proximo_cobro(id, "ayer", &mut a).unwrap_err()), ErrorDominio::FechaDeCorreccionNoEntendida { fecha: "ayer".into() });
        assert_eq!(dominio(corregir_proximo_cobro(999, "15/03/2026", &mut a).unwrap_err()), ErrorDominio::SuscripcionNoEncontradaParaCorregir);
        eliminar_suscripcion(id, &mut a).unwrap();
        assert!(eliminar_suscripcion(id, &mut a).is_ok(), "borrar una que no está no es un error");
    }

    #[test]
    fn el_listado_va_por_nombre_con_el_aviso_el_impedimento_y_los_pendientes() {
        let mut a = almacen();
        crear_suscripcion(datos("Zeta", "anual", 5, Some("05/07/2026")), &mut a).unwrap();
        crear_suscripcion(datos("Alfa", "mensual", 5, None), &mut a).unwrap();
        crear_suscripcion(datos("Medio", "mensual", 15, Some("15/01/2026")), &mut a).unwrap();
        let lista = listar_suscripciones(fecha(2026, 7, 1), &a).unwrap();
        assert_eq!(lista.iter().map(|s| s.leida.plataforma.as_str()).collect::<Vec<_>>(), vec!["Alfa", "Medio", "Zeta"]);
        assert!(lista[2].avisa, "la anual del día 5 avisa dentro de la semana");
        assert!(lista[0].impedimento.is_some(), "sin fecha no se cobra y se dice");
        assert!(lista[1].pendientes.len() > 1, "varios períodos vencidos quedan por confirmar");
    }

    #[test]
    fn la_categoria_es_suscripciones_luego_otros_y_si_no_hay_ninguna_se_crea() {
        let mut a = AlmacenEnMemoria::nuevo().con_categoria(1, "Comida");
        let creada = categoria_de_suscripciones(&mut a).unwrap();
        assert_eq!(a.categorias.get(&creada).map(String::as_str), Some("Suscripciones"));
        assert_eq!(categoria_de_suscripciones(&mut a).unwrap(), creada, "idempotente");
        let mut b = AlmacenEnMemoria::nuevo().con_categoria(7, "Otros");
        assert_eq!(categoria_de_suscripciones(&mut b).unwrap(), 7);
        let mut c = AlmacenEnMemoria::nuevo().con_categoria(7, "Otros").con_categoria(9, "SUSCRIPCIONES");
        assert_eq!(categoria_de_suscripciones(&mut c).unwrap(), 9, "sin distinguir mayúsculas, y antes que «Otros»");
    }

    #[test]
    fn solo_son_candidatas_las_que_tienen_exactamente_un_periodo_vencido() {
        let mut a = almacen();
        let uno = crear_suscripcion(datos("Uno", "mensual", 15, Some("15/03/2026")), &mut a).unwrap();
        crear_suscripcion(datos("Varios", "mensual", 15, Some("15/01/2026")), &mut a).unwrap();
        crear_suscripcion(datos("Futura", "mensual", 15, Some("15/09/2026")), &mut a).unwrap();
        crear_suscripcion(datos("SinFecha", "mensual", 15, None), &mut a).unwrap();
        assert_eq!(cobros_automaticos(fecha(2026, 3, 20), &a).unwrap(), vec![CandidataACobro { id: uno, plataforma: "Uno".into() }]);
    }

    #[test]
    fn cobrar_si_vence_cobra_con_la_fecha_del_vencimiento_sube_la_deuda_y_avanza_desde_el_vencimiento() {
        let mut a = almacen();
        let id = crear_suscripcion(datos("Uno", "mensual", 15, Some("15/03/2026")), &mut a).unwrap();
        let cargo = cobrar_si_vence(id, fecha(2026, 3, 20), 7, &mut a).unwrap().unwrap();
        assert_eq!(cargo, CargoAsentado { plataforma: "Uno".into(), divisa: "DOP".into(), monto: 500.0, vencimiento: fecha(2026, 3, 15) });
        assert_eq!(a.deuda_de(20), dop(500.0));
        assert_eq!(a.gastos.len(), 1);
        let s = &a.suscripciones[0];
        assert_eq!(s.datos.fecha_proximo_cobro.as_deref(), Some("15/04/2026"), "desde el vencimiento saldado, no desde hoy");
        assert_eq!(s.fecha_ultimo_pago.as_deref(), Some("15/03/2026"));
    }

    #[test]
    fn una_lista_vieja_no_carga_la_tarjeta_antigua_porque_el_cobro_relee_la_suscripcion() {
        let mut a = almacen().con_tarjeta(21, dop(0.0));
        let id = crear_suscripcion(datos("Uno", "mensual", 15, Some("15/03/2026")), &mut a).unwrap();
        let candidatas = cobros_automaticos(fecha(2026, 3, 20), &a).unwrap();
        // Entre la lista y el cargo, el titular traslada la suscripción a otra tarjeta y cambia el importe.
        let mut nueva = datos("Uno", "mensual", 15, Some("15/03/2026"));
        nueva.tarjeta_id = 21;
        nueva.monto = dop(80.0);
        editar_suscripcion(id, nueva, &mut a).unwrap();
        let cargo = cobrar_si_vence(candidatas[0].id, fecha(2026, 3, 20), 7, &mut a).unwrap().unwrap();
        assert_eq!((a.deuda_de(20), a.deuda_de(21)), (dop(0.0), dop(80.0)), "la tarjeta antigua no se toca");
        assert_eq!(cargo.monto, 80.0, "el mensaje cuenta lo que de verdad se cobró");
    }

    #[test]
    fn si_la_suscripcion_ya_no_vence_borrada_cobrada_o_con_varios_periodos_no_se_cobra() {
        let mut a = almacen();
        let borrada = crear_suscripcion(datos("Borrada", "mensual", 15, Some("15/03/2026")), &mut a).unwrap();
        let cobrada = crear_suscripcion(datos("Cobrada", "mensual", 15, Some("15/03/2026")), &mut a).unwrap();
        let acumulada = crear_suscripcion(datos("Acumulada", "mensual", 15, Some("15/03/2026")), &mut a).unwrap();
        let hoy = fecha(2026, 3, 20);
        let candidatas = cobros_automaticos(hoy, &a).unwrap();
        assert_eq!(candidatas.iter().map(|c| c.id).collect::<Vec<_>>(), vec![borrada, cobrada, acumulada]);

        eliminar_suscripcion(borrada, &mut a).unwrap();
        assert!(cobrar_si_vence(cobrada, hoy, 7, &mut a).unwrap().is_some());
        // Dos veces seguidas con la misma lista: el segundo intento ve el puntero ya avanzado.
        assert!(cobrar_si_vence(cobrada, hoy, 7, &mut a).unwrap().is_none(), "no cobra dos veces");
        corregir_proximo_cobro(acumulada, "15/01/2026", &mut a).unwrap();

        assert!(cobrar_si_vence(borrada, hoy, 7, &mut a).unwrap().is_none());
        assert!(cobrar_si_vence(acumulada, hoy, 7, &mut a).unwrap().is_none(), "con varios períodos se ofrece confirmar");
        assert_eq!(a.gastos.len(), 1, "solo el cargo de «Cobrada»");
    }

    fn cargo(plataforma: &str) -> CargoAsentado {
        CargoAsentado { plataforma: plataforma.into(), divisa: "DOP".into(), monto: 10.0, vencimiento: fecha(2026, 3, 15) }
    }

    fn candidata(id: i64, plataforma: &str) -> CandidataACobro {
        CandidataACobro { id, plataforma: plataforma.into() }
    }

    #[test]
    fn un_fallo_no_corta_el_recorrido_y_se_devuelve_con_su_causa() {
        let candidatas = vec![candidata(1, "Primera"), candidata(2, "Rota"), candidata(3, "Tercera")];
        let mut intentos = Vec::new();
        let r = procesar_cobros(candidatas, |id| {
            intentos.push(id);
            match id {
                2 => Err("la tarjeta rechazó el cargo"),
                otro => Ok(Some(cargo(&format!("S{otro}")))),
            }
        });
        assert_eq!(intentos, vec![1, 2, 3], "se intentaron todas, también las posteriores al fallo");
        assert_eq!(r.cargos.iter().map(|c| c.plataforma.as_str()).collect::<Vec<_>>(), vec!["S1", "S3"]);
        assert_eq!(r.fallos, vec![FalloDeCobro { plataforma: "Rota".into(), motivo: "la tarjeta rechazó el cargo".into() }]);
    }

    #[test]
    fn sin_fallos_ni_cargos_no_hay_nada_que_contar_y_una_que_ya_no_vence_no_es_un_fallo() {
        let r = procesar_cobros(vec![candidata(1, "A"), candidata(2, "B")], |_| Ok::<_, String>(None));
        assert_eq!(r, CobrosAutomaticos::default());
        let todas_rotas = procesar_cobros(vec![candidata(1, "A"), candidata(2, "B")], |_| Err::<Option<CargoAsentado>, _>("x"));
        assert_eq!((todas_rotas.cargos.len(), todas_rotas.fallos.len()), (0, 2));
    }

    #[test]
    fn confirmar_asienta_el_mas_antiguo_y_descartar_avanza_sin_cobrar_y_deja_caso() {
        let mut a = almacen();
        let id = crear_suscripcion(datos("Varios", "mensual", 15, Some("15/01/2026")), &mut a).unwrap();
        let hoy = fecha(2026, 3, 20);
        let r = confirmar_pendiente(id, hoy, None, 7, &mut a).unwrap();
        assert_eq!(r, Confirmado::Asentado { plataforma: "Varios".into(), vencimiento: fecha(2026, 1, 15) });
        assert_eq!(a.deuda_de(20), dop(500.0));
        assert_eq!(a.suscripciones[0].datos.fecha_proximo_cobro.as_deref(), Some("15/02/2026"));

        let r = confirmar_pendiente(id, hoy, Some(MOTIVO), 7, &mut a).unwrap();
        assert_eq!(r, Confirmado::Descartado { vencimiento: fecha(2026, 2, 15), caso: "CASO-0001".into() });
        assert_eq!(a.deuda_de(20), dop(500.0), "descartar no cobra");
        assert_eq!(a.suscripciones[0].datos.fecha_proximo_cobro.as_deref(), Some("15/03/2026"));
        assert_eq!(a.casos[0].descripcion, "Varios — período del 15/02/2026");
        assert_eq!(a.casos[0].tipo, "período de suscripción");
    }

    #[test]
    fn confirmar_dice_su_causa_y_un_motivo_corto_no_avanza_nada() {
        let mut a = almacen();
        let hoy = fecha(2026, 3, 20);
        assert_eq!(dominio(confirmar_pendiente(999, hoy, None, 7, &mut a).unwrap_err()), ErrorDominio::SuscripcionNoEncontrada);
        let id = crear_suscripcion(datos("Uno", "mensual", 15, Some("15/03/2026")), &mut a).unwrap();
        assert_eq!(dominio(confirmar_pendiente(id, hoy, None, 7, &mut a).unwrap_err()), ErrorDominio::SinPeriodosPendientes);
        let varios = crear_suscripcion(datos("Varios", "mensual", 15, Some("15/01/2026")), &mut a).unwrap();
        let e = confirmar_pendiente(varios, hoy, Some("corto"), 7, &mut a).unwrap_err();
        assert!(matches!(dominio(e), ErrorDominio::MotivoInsuficiente { .. }));
        assert_eq!(a.suscripciones[1].datos.fecha_proximo_cobro.as_deref(), Some("15/01/2026"));
        assert!(a.casos.is_empty());
    }
}
