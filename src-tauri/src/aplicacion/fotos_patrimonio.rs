//! Casos de uso: las fotos mensuales del patrimonio.
//!
//! * `tomar_foto`: arma la foto de hoy con lo que hay en la base y en el capital, la guarda (reemplazando la del mismo mes
//!   si no está sellada) y sella los meses anteriores. Tomarla dos veces en el mes deja una sola.
//! * `asegurar_foto_del_mes`: la que llama la interfaz al arrancar. Si ya existe la del mes **no escribe nada**.
//! * `listar_fotos`: lectura, de la más antigua a la más reciente.
//!
//! ## Qué entra en la foto
//!
//! * Cuentas y caja de efectivo (saldo en su divisa). Una cuenta **archivada** con saldo cero no genera partida; con saldo
//!   sí: el dinero sigue ahí.
//! * Tarjetas: la deuda en pesos y, si hay, la de dólares. Una tarjeta **inactiva** con saldo cero no genera partida.
//! * Financiamientos con saldo distinto de cero.
//! * Capital (certificados, bolsa, inmobiliario, vehículos, maquinaria), una partida por entrada.
//!
//! ## Lo que no se inventa
//!
//! Si `capital.json` no se puede leer, el capital cuenta **cero** y la foto queda `con_aviso`. Lo mismo si una entrada
//! suya tiene un importe ilegible. Nunca se reutiliza el capital de un mes anterior.
//!
//! ## `componer` (PR 1) todavía no está aquí
//!
//! Las partidas se arman en este módulo (`partidas_de`). Cuando exista `dominio::patrimonio::componer`, sustituirá a esa
//! función como productora de las partidas; `Foto::construir` no cambia.

use super::ErrorAplicacion;
use crate::dominio::capital::divisa_del_capital;
use crate::dominio::dinero::{Dinero, TasaCambio};
use crate::dominio::foto_patrimonio::{Calidad, Foto, Origen, Rubro};
use crate::puertos::repositorios::*;
use chrono::{Datelike, NaiveDate};
use serde_json::Value;

/// Una foto con sus partidas, tal como se lee.
#[derive(Debug, Clone, PartialEq)]
pub struct FotoCompleta {
    pub foto: FotoLeida,
    pub partidas: Vec<PartidaLeida>,
}

/// Qué pasó al asegurar la foto del mes.
#[derive(Debug, Clone, PartialEq)]
pub struct Asegurada {
    /// `true` si hubo que tomarla; `false` si ya existía y no se escribió nada.
    pub creada: bool,
    /// La foto recién tomada (`None` si ya existía).
    pub foto: Option<FotoCompleta>,
}

/// Un importe del capital tal como esté guardado: `None` si no se entiende (no se inventa).
fn importe_del_capital(valor: Option<&Value>) -> Option<Dinero> {
    match valor {
        Some(Value::Number(n)) => Dinero::nuevo(n.as_f64()?, divisa_del_capital()).ok(),
        Some(Value::String(s)) => Dinero::desde_texto(s, divisa_del_capital()).ok(),
        _ => None,
    }
}

type Cruda = (Rubro, String, Dinero);

/// Las partidas del capital; `false` en el segundo valor si alguna entrada tenía un importe ilegible.
fn partidas_del_capital(capital: &Value) -> (Vec<Cruda>, bool) {
    let mut partidas = Vec::new();
    let mut completa = true;
    let mut tomar = |rubro: Rubro, entradas: Option<&Value>, campo_nombre: &str, campo_importe: &str| {
        for entrada in entradas.and_then(|v| v.as_array()).into_iter().flatten() {
            let nombre = entrada.get(campo_nombre).and_then(|v| v.as_str()).unwrap_or("").to_string();
            match importe_del_capital(entrada.get(campo_importe)) {
                Some(importe) => partidas.push((rubro, nombre, importe)),
                None => completa = false,
            }
        }
    };
    tomar(Rubro::Certificado, capital.get("certificados"), "banco", "monto");
    tomar(Rubro::Bolsa, capital.get("bolsa"), "emisor", "monto");
    let propiedades = capital.get("propiedades");
    tomar(Rubro::Inmobiliario, propiedades.and_then(|p| p.get("inmobiliario")), "nombre", "valor_estimado");
    tomar(Rubro::Vehiculo, propiedades.and_then(|p| p.get("vehiculos")), "nombre", "valor_estimado");
    tomar(Rubro::Maquinaria, propiedades.and_then(|p| p.get("maquinaria")), "nombre", "valor_estimado");
    (partidas, completa)
}

/// Las partidas de una situación y del capital, y la calidad que les corresponde.
///
/// `capital` es `Err(motivo)` cuando el documento existe pero no se pudo leer.
pub fn partidas_de(situacion: &SituacionParaFoto, capital: &Result<Value, String>) -> (Vec<Cruda>, Calidad) {
    let mut partidas: Vec<Cruda> = Vec::new();
    for c in &situacion.cuentas {
        if c.archivada && c.saldo.es_cero() {
            continue;
        }
        let rubro = if c.es_caja_efectivo { Rubro::Efectivo } else { Rubro::Cuenta };
        partidas.push((rubro, c.nombre.clone(), c.saldo));
    }
    for t in &situacion.tarjetas {
        if !t.activa && t.deuda_pesos.es_cero() && t.deuda_dolares.es_cero() {
            continue;
        }
        if t.activa || !t.deuda_pesos.es_cero() {
            partidas.push((Rubro::Tarjeta, t.nombre.clone(), t.deuda_pesos));
        }
        if !t.deuda_dolares.es_cero() {
            partidas.push((Rubro::Tarjeta, t.nombre.clone(), t.deuda_dolares));
        }
    }
    for p in &situacion.prestamos {
        if !p.saldo.es_cero() {
            partidas.push((Rubro::Prestamo, p.nombre.clone(), p.saldo));
        }
    }
    let calidad = match capital {
        Ok(doc) => {
            let (del_capital, completa) = partidas_del_capital(doc);
            partidas.extend(del_capital);
            if completa {
                Calidad::Completa
            } else {
                Calidad::ConAviso
            }
        }
        Err(_) => Calidad::ConAviso,
    };
    (partidas, calidad)
}

fn leer_completa<R: RepositorioDeFotos>(repo: &R, anio: i32, mes: u32) -> Result<FotoCompleta, ErrorAplicacion> {
    let foto = repo
        .fotos()?
        .into_iter()
        .find(|f| f.anio == anio && f.mes == mes)
        .ok_or(ErrorAlmacen::Fallo("la foto recién guardada no aparece".into()))?;
    let partidas = repo.partidas_de(foto.id)?;
    Ok(FotoCompleta { foto, partidas })
}

/// Toma la foto de `hoy`. Si la del mes existe y no está sellada, la rehace; si está sellada, falla sin tocar nada.
pub fn tomar_foto<R: RepositorioDeFotos + ConsultaDePatrimonio>(
    repo: &mut R,
    capital: &Result<Value, String>,
    hoy: NaiveDate,
    origen: Origen,
    tasa_usd_dop: TasaCambio,
) -> Result<FotoCompleta, ErrorAplicacion> {
    let situacion = repo.situacion_para_foto()?;
    let (crudas, calidad) = partidas_de(&situacion, capital);
    let foto = Foto::construir(hoy, origen, tasa_usd_dop, calidad, crudas)?;
    repo.guardar_foto(&foto)?;
    // Un mes cerrado no vuelve a cambiar: al existir la de este mes, las anteriores se sellan.
    repo.sellar_anteriores(hoy.year(), hoy.month())?;
    leer_completa(repo, foto.anio, foto.mes)
}

/// Garantiza que exista la foto del mes de `hoy`. Si ya existe, **no escribe nada**.
pub fn asegurar_foto_del_mes<R: RepositorioDeFotos + ConsultaDePatrimonio>(
    repo: &mut R,
    capital: &Result<Value, String>,
    hoy: NaiveDate,
    tasa_usd_dop: TasaCambio,
) -> Result<Asegurada, ErrorAplicacion> {
    if repo.existe_foto(hoy.year(), hoy.month())? {
        return Ok(Asegurada { creada: false, foto: None });
    }
    let foto = tomar_foto(repo, capital, hoy, Origen::Real, tasa_usd_dop)?;
    Ok(Asegurada { creada: true, foto: Some(foto) })
}

pub fn listar_fotos<R: RepositorioDeFotos>(repo: &R) -> Result<Vec<FotoCompleta>, ErrorAplicacion> {
    repo.fotos()?
        .into_iter()
        .map(|foto| {
            let partidas = repo.partidas_de(foto.id)?;
            Ok(FotoCompleta { foto, partidas })
        })
        .collect()
}

/// La tasa USD→DOP de una petición: la indicada, o la sugerida (`TASA_USD_DOP_SUGERIDA`).
pub fn tasa_de_peticion(texto: Option<&str>) -> Result<TasaCambio, ErrorAplicacion> {
    use crate::dominio::errores::ErrorDominio;
    match texto.map(str::trim).filter(|t| !t.is_empty()) {
        None => Ok(TasaCambio::nueva(crate::dominio::foto_patrimonio::TASA_USD_DOP_SUGERIDA)?),
        Some(t) => {
            let valor: f64 = t.replace(',', ".").parse().map_err(|_| ErrorDominio::ImporteIlegible { texto: t.to_string() })?;
            Ok(TasaCambio::nueva(valor)?)
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::dominio::dinero::Divisa;
    use crate::puertos::dobles::FotosEnMemoria;
    use serde_json::json;

    fn d(c: i64) -> Dinero {
        Dinero::de_centavos(c, Divisa::Dop)
    }
    fn u(c: i64) -> Dinero {
        Dinero::de_centavos(c, Divisa::Usd)
    }
    fn dia(a: i32, m: u32, di: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(a, m, di).unwrap()
    }
    fn tasa() -> TasaCambio {
        TasaCambio::nueva(58.4137).unwrap()
    }
    fn situacion() -> SituacionParaFoto {
        SituacionParaFoto {
            cuentas: vec![
                CuentaParaFoto { nombre: "Ahorros".into(), saldo: d(123_456_78), es_caja_efectivo: false, archivada: false },
                CuentaParaFoto { nombre: "Caja".into(), saldo: d(9_876_54), es_caja_efectivo: true, archivada: false },
                CuentaParaFoto { nombre: "Dólares".into(), saldo: u(1_234_56), es_caja_efectivo: false, archivada: false },
            ],
            tarjetas: vec![TarjetaParaFoto { nombre: "Visa Oro".into(), activa: true, deuda_pesos: d(45_678_91), deuda_dolares: u(0) }],
            prestamos: vec![PrestamoParaFoto { nombre: "Banco (consumo)".into(), saldo: d(300_111_23) }],
        }
    }
    fn capital() -> Result<Value, String> {
        Ok(json!({
            "certificados": [{"banco": "Banco Uno", "monto": 500_000.37}],
            "bolsa": [{"emisor": "Bolsa X", "monto": "20000.19"}],
            "propiedades": {"inmobiliario": [{"nombre": "Casa", "valor_estimado": 4_100_000.91}], "vehiculos": [], "maquinaria": []}
        }))
    }

    #[test]
    fn la_foto_junta_la_base_y_el_capital() {
        let mut repo = FotosEnMemoria::con_situacion(situacion());
        let r = tomar_foto(&mut repo, &capital(), dia(2026, 3, 5), Origen::Manual, tasa()).unwrap();
        assert_eq!(r.partidas.len(), 3 + 1 + 1 + 3);
        assert_eq!(r.foto.calidad, "completa");
        assert_eq!(r.foto.patrimonio.centavos(), r.foto.activos.centavos() - r.foto.pasivos.centavos());
        let dolares = r.partidas.iter().find(|p| p.referencia == "Dólares").unwrap();
        assert_eq!(dolares.importe_dop, u(1_234_56).convertir(Divisa::Dop, tasa()).unwrap());
    }

    #[test]
    fn una_cuenta_archivada_en_cero_no_genera_partida_y_con_saldo_si() {
        let mut s = situacion();
        s.cuentas.push(CuentaParaFoto { nombre: "Vieja".into(), saldo: d(0), es_caja_efectivo: false, archivada: true });
        s.cuentas.push(CuentaParaFoto { nombre: "Olvidada".into(), saldo: d(77_01), es_caja_efectivo: false, archivada: true });
        let (partidas, _) = partidas_de(&s, &capital());
        assert!(!partidas.iter().any(|p| p.1 == "Vieja"));
        assert!(partidas.iter().any(|p| p.1 == "Olvidada" && p.2 == d(77_01)));
    }

    #[test]
    fn una_tarjeta_inactiva_en_cero_no_genera_partida_y_con_deuda_si() {
        let mut s = situacion();
        s.tarjetas.push(TarjetaParaFoto { nombre: "Cerrada".into(), activa: false, deuda_pesos: d(0), deuda_dolares: u(0) });
        s.tarjetas.push(TarjetaParaFoto { nombre: "Cerrada con deuda".into(), activa: false, deuda_pesos: d(0), deuda_dolares: u(12_34) });
        let (partidas, _) = partidas_de(&s, &capital());
        assert!(!partidas.iter().any(|p| p.1 == "Cerrada"));
        assert!(partidas.iter().any(|p| p.1 == "Cerrada con deuda" && p.2 == u(12_34)));
    }

    #[test]
    fn un_capital_ilegible_cuenta_cero_y_la_foto_queda_con_aviso() {
        let mut repo = FotosEnMemoria::con_situacion(situacion());
        let r = tomar_foto(&mut repo, &Err("dañado".into()), dia(2026, 3, 5), Origen::Real, tasa()).unwrap();
        assert_eq!(r.foto.calidad, "con_aviso");
        assert!(!r.partidas.iter().any(|p| p.rubro == "certificado" || p.rubro == "inmobiliario"));
    }

    #[test]
    fn un_importe_ilegible_en_el_capital_deja_la_foto_con_aviso_sin_inventarlo() {
        let cap = Ok(json!({"certificados": [{"banco": "A", "monto": "abc"}, {"banco": "B", "monto": 100.5}]}));
        let (partidas, calidad) = partidas_de(&SituacionParaFoto::default(), &cap);
        assert_eq!(calidad, Calidad::ConAviso);
        assert_eq!(partidas.len(), 1);
        assert_eq!(partidas[0].1, "B");
    }

    #[test]
    fn tomarla_dos_veces_en_el_mes_deja_una_sola() {
        let mut repo = FotosEnMemoria::con_situacion(situacion());
        tomar_foto(&mut repo, &capital(), dia(2026, 3, 5), Origen::Real, tasa()).unwrap();
        tomar_foto(&mut repo, &capital(), dia(2026, 3, 20), Origen::Manual, tasa()).unwrap();
        let todas = listar_fotos(&repo).unwrap();
        assert_eq!(todas.len(), 1);
        assert_eq!(todas[0].foto.origen, "manual");
    }

    #[test]
    fn asegurar_toma_la_del_mes_una_vez_y_despues_no_escribe() {
        let mut repo = FotosEnMemoria::con_situacion(situacion());
        let a = asegurar_foto_del_mes(&mut repo, &capital(), dia(2026, 3, 5), tasa()).unwrap();
        assert!(a.creada && a.foto.is_some());
        assert_eq!(repo.escrituras, 1);
        let b = asegurar_foto_del_mes(&mut repo, &capital(), dia(2026, 3, 28), tasa()).unwrap();
        assert!(!b.creada && b.foto.is_none());
        assert_eq!(repo.escrituras, 1, "la segunda llamada no escribió");
    }

    #[test]
    fn al_llegar_el_mes_siguiente_el_anterior_queda_sellado_e_inmutable() {
        let mut repo = FotosEnMemoria::con_situacion(situacion());
        tomar_foto(&mut repo, &capital(), dia(2026, 3, 5), Origen::Real, tasa()).unwrap();
        tomar_foto(&mut repo, &capital(), dia(2026, 4, 2), Origen::Real, tasa()).unwrap();
        let todas = listar_fotos(&repo).unwrap();
        assert_eq!((todas[0].foto.mes, todas[0].foto.sellada), (3, true));
        assert_eq!((todas[1].foto.mes, todas[1].foto.sellada), (4, false));
        // Volver a tomar la de marzo no la cambia.
        let e = tomar_foto(&mut repo, &capital(), dia(2026, 3, 30), Origen::Manual, tasa()).unwrap_err();
        assert_eq!(e, ErrorAplicacion::Almacen(ErrorAlmacen::FotoSellada { anio: 2026, mes: 3 }));
    }

    #[test]
    fn si_guardar_falla_el_error_se_propaga() {
        let mut repo = FotosEnMemoria::con_situacion(situacion());
        repo.fallar_al_guardar = true;
        assert!(tomar_foto(&mut repo, &capital(), dia(2026, 3, 5), Origen::Real, tasa()).is_err());
    }

    #[test]
    fn la_tasa_se_toma_de_la_peticion_o_de_la_sugerida() {
        assert_eq!(tasa_de_peticion(None).unwrap(), TasaCambio::nueva(60.0).unwrap());
        assert_eq!(tasa_de_peticion(Some("  ")).unwrap(), TasaCambio::nueva(60.0).unwrap());
        assert_eq!(tasa_de_peticion(Some("58,75")).unwrap(), TasaCambio::nueva(58.75).unwrap());
        assert!(tasa_de_peticion(Some("abc")).is_err());
        assert!(tasa_de_peticion(Some("0")).is_err());
    }
}
