//! Recomendaciones para el historial del patrimonio: dominio puro, sin SQL ni Tauri. **Solo lectura.**
//!
//! Son **reglas descriptivas y deterministas**, no asesoría financiera ni de inversión: cada recomendación enseña el dato
//! que la dispara (cifras y periodo) y la regla escrita en palabras, con sus umbrales. Con los mismos datos y la misma
//! fecha siempre sale lo mismo, en el mismo orden. Si no hay datos para una regla, se dice (`sin_datos`) en vez de
//! inventar una cifra.
//!
//! ## Principios
//!
//! * **Dinero**: centavos enteros (`Dinero`) y `i128` para los productos cruzados; ningún `f64` toca un importe. Los
//!   porcentajes son milésimas enteras (`dividir_redondeando`, mitad lejos de cero).
//! * **Solo pesos.** Los gastos en dólares, las deudas de tarjeta en dólares y las cuentas en dólares **no se
//!   convierten** (no hay tasa que valga para un análisis): se excluyen de las reglas y se informan aparte
//!   (`usd_excluido`). La excepción son las **fotos del patrimonio**, que ya guardan su propia tasa y su total en pesos.
//! * **Mes de referencia del gasto** = el último mes completo (el anterior a «hoy»): un mes a medias distorsiona pesos y
//!   razones. Los **presupuestos** se evalúan en el mes en curso, que es cuando sirven.
//! * **Orden determinista**: atención antes que informativa; dentro de cada una, por regla (el orden del enumerado
//!   `Regla`), luego por magnitud (mayor primero) y por último por identificador. Se muestran a lo sumo
//!   `MAXIMO_POR_SEVERIDAD` de cada severidad; el resto se cuenta en `omitidas`.
//!
//! ## Reglas y umbrales
//!
//! 1. **`tendencia_gasto`** (atención): una categoría **no fija** con tendencia «sube» según la regla A+D de
//!    `resumen_gastos` (promedio de los 3 últimos meses completos más de un 10 % sobre el de los 3 anteriores y con
//!    una diferencia de al menos DOP 500.00; se necesitan 4 meses completos). Los cargos derivados no cuentan.
//! 2. **`presupuesto`**: una categoría con presupuesto en el mes en curso; «cerca» (desde el 80 %) es informativa y
//!    «excedido» (desde el 100 %) es atención. Las cifras son las de `presupuesto::estado_del_mes`.
//! 3. **`cargos_derivados`**: comisiones de abonos y cargos de avances de efectivo (gastos que crea otra operación) que
//!    pesan desde el 5 % del gasto en pesos del mes de referencia (informativa) o desde el 10 % (atención), siempre con
//!    al menos DOP 200.00 de cargos.
//! 4. **`deuda_y_ahorro`**: hay deuda de tarjetas en pesos (al menos DOP 100.00) y a la vez saldo en cuentas de ahorro en
//!    pesos (la caja de efectivo y las cuentas archivadas no cuentan). Es atención si el ahorro alcanza para cubrir la
//!    deuda; informativa si no. **No estima costos**: la aplicación no guarda la tasa de las tarjetas ni el
//!    rendimiento de las cuentas, y no se inventan tasas.
//! 5. **`variacion_patrimonio`**: cambio del patrimonio entre las dos fotos mensuales más recientes y qué componente lo
//!    explica (activos o pasivos: el de mayor cambio absoluto; si empatan, activos). Baja = atención; sube =
//!    informativa. Si alguna de las dos fotos no es de calidad completa, la severidad es informativa y se dice. Si las
//!    fotos usan tasas USD distintas, se muestran las dos: parte del cambio puede ser cambiaria.
//! 6. **`fijo_variable`** (informativa): el gasto en pesos del mes de referencia se reparte en fijo (categorías que el
//!    **titular** marcó como fijas) y variable (el resto, con los cargos derivados); se avisa del segmento que pesa desde
//!    el 60 %, si el gasto del mes es de al menos DOP 500.00. Sin ninguna categoría fija marcada, la regla no tiene datos.

use super::dinero::{dividir_redondeando, Dinero, Divisa, TasaCambio};
use super::errores::ErrorDominio;
use super::patrimonio::{tasa_estimada, CuentaDePatrimonio, TarjetaDePatrimonio};
use super::presupuesto::{estado_del_mes, EstadoPresupuesto, Presupuesto};
use super::resumen_gastos::{
    promedios_reciente_y_previo, resumen_por_mes, DireccionTendencia, FilaAgregadoGasto, OpcionesResumen, ResumenMensual,
    MINIMO_ABSOLUTO_TENDENCIA_CENTAVOS,
};
use chrono::{Datelike, NaiveDate};
use std::cmp::Reverse;
use std::collections::BTreeSet;

/// Cuántas recomendaciones de cada severidad se muestran como máximo.
pub const MAXIMO_POR_SEVERIDAD: usize = 5;
/// Cargos derivados: desde qué peso (milésimas del gasto en pesos del mes) son informativos.
pub const CARGOS_INFO_MILESIMAS: i128 = 50;
/// Cargos derivados: desde qué peso son de atención.
pub const CARGOS_ATENCION_MILESIMAS: i128 = 100;
/// Cargos derivados: importe mínimo del mes, en centavos (DOP 200.00).
pub const CARGOS_MINIMO_CENTAVOS: i64 = 20_000;
/// Deuda de tarjetas mínima, en centavos, para que la regla de deuda y ahorro se active (DOP 100.00).
pub const DEUDA_MINIMA_CENTAVOS: i64 = 10_000;
/// Desde qué peso (milésimas) un segmento fijo o variable «predomina».
pub const PREDOMINA_MILESIMAS: i128 = 600;
/// Gasto mínimo del mes, en centavos, para evaluar la razón fijo/variable (DOP 500.00).
pub const FIJO_VARIABLE_MINIMO_CENTAVOS: i64 = 50_000;

/// Cada regla, en el orden que desempata la presentación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Regla {
    Presupuesto,
    TendenciaGasto,
    VariacionPatrimonio,
    DeudaYAhorro,
    CargosDerivados,
    FijoVariable,
}

impl Regla {
    pub fn codigo(self) -> &'static str {
        match self {
            Regla::Presupuesto => "presupuesto",
            Regla::TendenciaGasto => "tendencia_gasto",
            Regla::VariacionPatrimonio => "variacion_patrimonio",
            Regla::DeudaYAhorro => "deuda_y_ahorro",
            Regla::CargosDerivados => "cargos_derivados",
            Regla::FijoVariable => "fijo_variable",
        }
    }
}

/// Atención va antes que informativa en el orden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severidad {
    Atencion,
    Info,
}

impl Severidad {
    pub fn codigo(self) -> &'static str {
        match self {
            Severidad::Atencion => "atencion",
            Severidad::Info => "info",
        }
    }
}

/// La pestaña donde se ve el dato. Solo rutas que existen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destino {
    Gastos,
    Tarjetas,
    Prestamos,
    Resumen,
    Cuentas,
    Evolucion,
}

impl Destino {
    pub fn ruta(self) -> &'static str {
        match self {
            Destino::Gastos => "gastos",
            Destino::Tarjetas => "tarjetas",
            Destino::Prestamos => "prestamos",
            Destino::Resumen => "resumen",
            Destino::Cuentas => "cuentas",
            Destino::Evolucion => "evolucion",
        }
    }
}

/// El valor de una cifra de respaldo; cada variante viaja con su forma exacta.
#[derive(Debug, Clone, PartialEq)]
pub enum ValorCifra {
    /// Pesos (o la divisa que traiga), texto exacto en el borde.
    Importe(Dinero),
    /// Una razón en milésimas (1000 = 100 %), con signo.
    Milesimas(i64),
    Tasa(TasaCambio),
    Texto(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cifra {
    pub etiqueta: String,
    pub valor: ValorCifra,
}

fn cifra(etiqueta: &str, valor: ValorCifra) -> Cifra {
    Cifra { etiqueta: etiqueta.to_string(), valor }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Recomendacion {
    /// Estable: la misma situación da el mismo identificador (`regla:clave`).
    pub id: String,
    pub regla: Regla,
    pub severidad: Severidad,
    pub titulo: String,
    /// Qué dicen los datos, en palabras.
    pub explicacion: String,
    /// La regla que la disparó, con sus umbrales, en palabras.
    pub regla_texto: String,
    pub cifras: Vec<Cifra>,
    pub destino: Destino,
    /// Para ordenar dentro de una regla (mayor primero). No viaja.
    pub magnitud: i64,
}

/// Una regla que no pudo evaluarse, y por qué. No es un error: es la honestidad de «faltan datos».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReglaSinDatos {
    pub regla: Regla,
    pub motivo: String,
}

/// Dólares que se dejaron fuera de las reglas (sin convertir).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsdExcluido {
    /// `gastos`, `tarjetas` o `cuentas`.
    pub origen: &'static str,
    pub importe: Dinero,
}

/// Una foto mensual tal como la necesitan las reglas (el dominio no depende de los puertos).
#[derive(Debug, Clone, PartialEq)]
pub struct FotoResumen {
    pub anio: i32,
    pub mes: u32,
    pub activos: Dinero,
    pub pasivos: Dinero,
    pub patrimonio: Dinero,
    pub tasa_usd_dop: TasaCambio,
    /// `completa` o `con_aviso`.
    pub calidad: String,
}

pub struct Entrada<'a> {
    pub hoy: NaiveDate,
    pub fotos: &'a [FotoResumen],
    pub gastos: &'a [FilaAgregadoGasto],
    pub presupuestos: &'a [Presupuesto],
    pub cuentas: &'a [CuentaDePatrimonio],
    pub tarjetas: &'a [TarjetaDePatrimonio],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Recomendaciones {
    /// Las visibles, ya en orden y recortadas a `MAXIMO_POR_SEVERIDAD` por severidad.
    pub visibles: Vec<Recomendacion>,
    /// Cuántas más hubo y no se muestran.
    pub omitidas: usize,
    pub sin_datos: Vec<ReglaSinDatos>,
    /// Cuántas reglas tuvieron datos para evaluarse (se hayan activado o no).
    pub reglas_evaluadas: usize,
    pub usd_excluido: Vec<UsdExcluido>,
    /// El último mes completo, que es del que se leen las reglas de gasto.
    pub mes_gastos: (i32, u32),
    /// El mes en curso, que es del que se leen los presupuestos.
    pub mes_presupuestos: (i32, u32),
}

fn dop(c: i64) -> Dinero {
    Dinero::de_centavos(c, Divisa::Dop)
}

fn pesos_texto(c: i64) -> String {
    format!("{}.{:02}", c / 100, c % 100)
}

fn periodo(a: i32, m: u32) -> String {
    format!("{m:02}/{a}")
}

fn clave_de_periodo(a: i32, m: u32) -> String {
    format!("{a}-{m:02}")
}

/// El mes anterior a `hoy`.
pub fn mes_anterior(hoy: NaiveDate) -> (i32, u32) {
    if hoy.month() == 1 {
        (hoy.year() - 1, 12)
    } else {
        (hoy.year(), hoy.month() - 1)
    }
}

fn es_usd(f: &FilaAgregadoGasto) -> bool {
    f.divisa.trim().eq_ignore_ascii_case("USD")
}

#[derive(Default)]
struct Acumulador {
    recomendaciones: Vec<Recomendacion>,
    sin_datos: Vec<ReglaSinDatos>,
    evaluadas: BTreeSet<Regla>,
}

impl Acumulador {
    fn sin_datos(&mut self, regla: Regla, motivo: &str) {
        self.sin_datos.push(ReglaSinDatos { regla, motivo: motivo.to_string() });
    }
}

/// Evalúa todas las reglas con los datos recibidos. Falla solo si el mes de referencia no es válido (no ocurre con una
/// fecha real).
pub fn evaluar(e: &Entrada) -> Result<Recomendaciones, ErrorDominio> {
    let (ref_anio, ref_mes) = mes_anterior(e.hoy);
    let mut acc = Acumulador::default();

    // Las reglas de gasto leen solo pesos: los dólares se quitan antes y se informan aparte.
    let filas_dop: Vec<FilaAgregadoGasto> = e.gastos.iter().filter(|f| !es_usd(f)).cloned().collect();
    let resumen = resumen_por_mes(&filas_dop, (ref_anio, ref_mes), e.hoy, 0, tasa_estimada(), &OpcionesResumen::default())?;

    regla_presupuestos(e, &mut acc)?;
    regla_tendencia(&resumen, (ref_anio, ref_mes), &mut acc);
    regla_variacion_patrimonio(e.fotos, &mut acc);
    regla_deuda_y_ahorro(e, &mut acc);
    regla_cargos_derivados(&resumen, (ref_anio, ref_mes), &mut acc);
    regla_fijo_variable(&resumen, (ref_anio, ref_mes), &mut acc);

    // Orden determinista y recorte por severidad.
    let mut todas = acc.recomendaciones;
    todas.sort_by(|a, b| {
        a.severidad
            .cmp(&b.severidad)
            .then_with(|| a.regla.cmp(&b.regla))
            .then_with(|| Reverse(a.magnitud).cmp(&Reverse(b.magnitud)))
            .then_with(|| a.id.cmp(&b.id))
    });
    let total = todas.len();
    let (mut atencion, mut info) = (0usize, 0usize);
    let visibles: Vec<Recomendacion> = todas
        .into_iter()
        .filter(|r| {
            let contador = if r.severidad == Severidad::Atencion { &mut atencion } else { &mut info };
            *contador += 1;
            *contador <= MAXIMO_POR_SEVERIDAD
        })
        .collect();
    let omitidas = total - visibles.len();

    let mut sin_datos = acc.sin_datos;
    sin_datos.sort_by(|a, b| a.regla.cmp(&b.regla));

    Ok(Recomendaciones {
        visibles,
        omitidas,
        sin_datos,
        reglas_evaluadas: acc.evaluadas.len(),
        usd_excluido: usd_excluido(e, (ref_anio, ref_mes)),
        mes_gastos: (ref_anio, ref_mes),
        mes_presupuestos: (e.hoy.year(), e.hoy.month()),
    })
}

fn usd_excluido(e: &Entrada, (anio, mes): (i32, u32)) -> Vec<UsdExcluido> {
    let mut salida = Vec::new();
    let gastos: i64 = e.gastos.iter().filter(|f| es_usd(f) && f.anio == anio && f.mes == mes).map(|f| f.centavos).sum();
    if gastos > 0 {
        salida.push(UsdExcluido { origen: "gastos", importe: Dinero::de_centavos(gastos, Divisa::Usd) });
    }
    let tarjetas: i64 = e.tarjetas.iter().map(|t| t.balance_usd.centavos()).filter(|c| *c > 0).sum();
    if tarjetas > 0 {
        salida.push(UsdExcluido { origen: "tarjetas", importe: Dinero::de_centavos(tarjetas, Divisa::Usd) });
    }
    let cuentas: i64 = e
        .cuentas
        .iter()
        .filter(|c| !c.archivada && c.saldo.divisa() == Divisa::Usd)
        .map(|c| c.saldo.centavos())
        .filter(|c| *c > 0)
        .sum();
    if cuentas > 0 {
        salida.push(UsdExcluido { origen: "cuentas", importe: Dinero::de_centavos(cuentas, Divisa::Usd) });
    }
    salida
}

// --- 2. Presupuestos (mes en curso) ---

fn regla_presupuestos(e: &Entrada, acc: &mut Acumulador) -> Result<(), ErrorDominio> {
    if e.presupuestos.is_empty() {
        acc.sin_datos(Regla::Presupuesto, "No hay presupuestos fijados; se fijan en Gastos.");
        return Ok(());
    }
    acc.evaluadas.insert(Regla::Presupuesto);
    let estado = estado_del_mes(e.hoy.year(), e.hoy.month(), e.gastos, e.presupuestos)?;
    let mes = periodo(estado.anio, estado.mes);
    for f in estado.filas.iter().filter(|f| f.estado != EstadoPresupuesto::Ok) {
        let excedido = f.estado == EstadoPresupuesto::Excedido;
        let mut cifras = vec![
            cifra("Mes (en curso)", ValorCifra::Texto(mes.clone())),
            cifra("Gastado en pesos", ValorCifra::Importe(f.gastado)),
            cifra("Presupuesto", ValorCifra::Importe(f.presupuesto)),
            cifra(if excedido { "Exceso" } else { "Restante" }, ValorCifra::Importe(if excedido { f.restante.negado() } else { f.restante })),
            cifra("Avance", ValorCifra::Milesimas(f.porcentaje_milesimas)),
        ];
        if f.usd_aparte.centavos() != 0 {
            cifras.push(cifra("Dólares aparte (no cuentan)", ValorCifra::Importe(f.usd_aparte)));
        }
        acc.recomendaciones.push(Recomendacion {
            id: format!("presupuesto_{}:{}", if excedido { "excedido" } else { "cerca" }, f.categoria_id),
            regla: Regla::Presupuesto,
            severidad: if excedido { Severidad::Atencion } else { Severidad::Info },
            titulo: if excedido {
                format!("{}: presupuesto del mes excedido", f.categoria_nombre)
            } else {
                format!("{}: cerca de su presupuesto del mes", f.categoria_nombre)
            },
            explicacion: format!(
                "En {mes} (mes en curso, hasta hoy) los gastos en pesos de «{}» van al {} del presupuesto que fijaste.",
                f.categoria_nombre,
                porcentaje_texto(f.porcentaje_milesimas)
            ),
            regla_texto: "Se compara lo gastado en pesos en la categoría durante el mes en curso con su presupuesto mensual: «cerca» desde el 80 % y «excedido» desde el 100 %. Los dólares no se convierten ni cuentan.".to_string(),
            cifras,
            destino: Destino::Gastos,
            magnitud: f.porcentaje_milesimas,
        });
    }
    Ok(())
}

/// Milésimas como `81.5 %`, por división entera.
fn porcentaje_texto(m: i64) -> String {
    let signo = if m < 0 { "-" } else { "" };
    let a = m.unsigned_abs();
    format!("{signo}{}.{} %", a / 10, a % 10)
}

// --- 1. Tendencia del gasto variable ---

fn regla_tendencia(resumen: &ResumenMensual, (ref_anio, ref_mes): (i32, u32), acc: &mut Acumulador) {
    let candidatas: Vec<_> = resumen.filas.iter().filter(|f| !f.derivado && !f.fija).collect();
    let evaluables = candidatas.iter().filter(|f| f.tendencia.direccion != DireccionTendencia::DatosInsuficientes).count();
    if evaluables == 0 {
        acc.sin_datos(
            Regla::TendenciaGasto,
            "Se necesitan al menos 4 meses completos de gasto en pesos en una categoría variable para ver su tendencia.",
        );
        return;
    }
    acc.evaluadas.insert(Regla::TendenciaGasto);
    for f in candidatas.iter().filter(|f| f.tendencia.direccion == DireccionTendencia::Sube) {
        let Some((reciente, previo)) = promedios_reciente_y_previo(&f.tendencia.meses) else { continue };
        let diferencia = reciente.centavos() - previo.centavos();
        let cambio = dividir_redondeando(diferencia as i128 * 1000, previo.centavos() as i128) as i64;
        acc.recomendaciones.push(Recomendacion {
            id: format!("tendencia_gasto:{}", f.categoria_id),
            regla: Regla::TendenciaGasto,
            severidad: Severidad::Atencion,
            titulo: format!("El gasto variable en «{}» viene subiendo", f.categoria_nombre),
            explicacion: format!(
                "Hasta {} (último mes completo), el promedio mensual de «{}» en los 3 últimos meses completos es mayor que el de los 3 meses anteriores.",
                periodo(ref_anio, ref_mes),
                f.categoria_nombre
            ),
            regla_texto: format!(
                "Una categoría no fija «sube» si el promedio de sus 3 últimos meses completos supera en más del 10 % al de los 3 anteriores y la diferencia es de al menos DOP {}; hacen falta 4 meses completos. Solo cuentan gastos en pesos.",
                pesos_texto(MINIMO_ABSOLUTO_TENDENCIA_CENTAVOS as i64)
            ),
            cifras: vec![
                cifra("Último mes completo", ValorCifra::Texto(periodo(ref_anio, ref_mes))),
                cifra("Promedio de los 3 últimos meses", ValorCifra::Importe(reciente)),
                cifra("Promedio de los 3 meses anteriores", ValorCifra::Importe(previo)),
                cifra("Diferencia", ValorCifra::Importe(dop(diferencia))),
                cifra("Cambio", ValorCifra::Milesimas(cambio)),
            ],
            destino: Destino::Gastos,
            magnitud: diferencia,
        });
    }
}

// --- 5. Variación del patrimonio ---

fn regla_variacion_patrimonio(fotos: &[FotoResumen], acc: &mut Acumulador) {
    if fotos.len() < 2 {
        acc.sin_datos(Regla::VariacionPatrimonio, "Se necesitan al menos 2 fotos mensuales del patrimonio para ver su variación.");
        return;
    }
    acc.evaluadas.insert(Regla::VariacionPatrimonio);
    let mut orden: Vec<&FotoResumen> = fotos.iter().collect();
    orden.sort_by_key(|f| (f.anio, f.mes));
    let (anterior, actual) = (orden[orden.len() - 2], orden[orden.len() - 1]);
    let cambio = actual.patrimonio.centavos() - anterior.patrimonio.centavos();
    if cambio == 0 {
        return;
    }
    let d_activos = actual.activos.centavos() - anterior.activos.centavos();
    let d_pasivos = actual.pasivos.centavos() - anterior.pasivos.centavos();
    let por_activos = d_activos.unsigned_abs() >= d_pasivos.unsigned_abs();
    let completas = actual.calidad == "completa" && anterior.calidad == "completa";
    let sube = cambio > 0;
    let (p_ant, p_act) = (periodo(anterior.anio, anterior.mes), periodo(actual.anio, actual.mes));

    let mut cifras = vec![
        cifra("Foto anterior", ValorCifra::Texto(p_ant.clone())),
        cifra("Foto reciente", ValorCifra::Texto(p_act.clone())),
        cifra("Patrimonio anterior", ValorCifra::Importe(anterior.patrimonio)),
        cifra("Patrimonio reciente", ValorCifra::Importe(actual.patrimonio)),
        cifra("Cambio del patrimonio", ValorCifra::Importe(dop(cambio))),
        cifra("Cambio en activos", ValorCifra::Importe(dop(d_activos))),
        cifra("Cambio en pasivos", ValorCifra::Importe(dop(d_pasivos))),
        cifra("Calidad de las fotos", ValorCifra::Texto(format!("{} / {}", anterior.calidad, actual.calidad))),
    ];
    if anterior.tasa_usd_dop != actual.tasa_usd_dop {
        cifras.push(cifra("Tasa USD de la foto anterior", ValorCifra::Tasa(anterior.tasa_usd_dop)));
        cifras.push(cifra("Tasa USD de la foto reciente", ValorCifra::Tasa(actual.tasa_usd_dop)));
    }

    let componente = if por_activos { "los activos" } else { "los pasivos (lo que debes)" };
    let mut explicacion = format!(
        "Entre la foto de {p_ant} y la de {p_act} el patrimonio {} y el mayor cambio está en {componente}.",
        if sube { "subió" } else { "bajó" }
    );
    if !completas {
        explicacion.push_str(" Alguna de las dos fotos tiene calidad «con aviso» (algo no se pudo leer y contó cero): la cifra puede estar incompleta.");
    }
    if anterior.tasa_usd_dop != actual.tasa_usd_dop {
        explicacion.push_str(" Las dos fotos usan tasas USD distintas, así que parte del cambio puede ser cambiario.");
    }
    acc.recomendaciones.push(Recomendacion {
        id: format!("variacion_patrimonio:{}", clave_de_periodo(actual.anio, actual.mes)),
        regla: Regla::VariacionPatrimonio,
        severidad: if !sube && completas { Severidad::Atencion } else { Severidad::Info },
        titulo: format!(
            "El patrimonio {} entre {p_ant} y {p_act}: lo explican {}",
            if sube { "subió" } else { "bajó" },
            if por_activos { "los activos" } else { "los pasivos" }
        ),
        explicacion,
        regla_texto: "Se comparan las dos fotos mensuales más recientes. El componente que explica el cambio es el que más varió en valor absoluto entre activos y pasivos (si empatan, los activos). Baja con fotos completas = atención; el resto, informativa.".to_string(),
        cifras,
        destino: if por_activos { Destino::Resumen } else { Destino::Prestamos },
        magnitud: cambio.abs(),
    });
}

// --- 4. Deuda de tarjetas frente al saldo en cuentas de ahorro ---

fn regla_deuda_y_ahorro(e: &Entrada, acc: &mut Acumulador) {
    // Hacen falta las dos mitades de la comparación: alguna tarjeta y alguna cuenta de ahorro (la caja de efectivo y las
    // archivadas no son cuentas de ahorro).
    if e.tarjetas.is_empty() || !e.cuentas.iter().any(|c| !c.es_efectivo && !c.archivada) {
        acc.sin_datos(Regla::DeudaYAhorro, "Se necesita al menos una tarjeta y una cuenta de ahorro para comparar su deuda con el saldo.");
        return;
    }
    acc.evaluadas.insert(Regla::DeudaYAhorro);
    let con_deuda: Vec<&TarjetaDePatrimonio> = e.tarjetas.iter().filter(|t| t.balance_dop.centavos() > 0).collect();
    let deuda: i64 = con_deuda.iter().map(|t| t.balance_dop.centavos()).sum();
    let ahorro: i64 = e
        .cuentas
        .iter()
        .filter(|c| !c.archivada && !c.es_efectivo && c.saldo.divisa() == Divisa::Dop && c.saldo.centavos() > 0)
        .map(|c| c.saldo.centavos())
        .sum();
    if deuda < DEUDA_MINIMA_CENTAVOS || ahorro <= 0 {
        return;
    }
    let cubre = ahorro >= deuda;
    acc.recomendaciones.push(Recomendacion {
        id: "deuda_y_ahorro".to_string(),
        regla: Regla::DeudaYAhorro,
        severidad: if cubre { Severidad::Atencion } else { Severidad::Info },
        titulo: "Hay deuda en tarjetas y a la vez saldo en cuentas de ahorro".to_string(),
        explicacion: format!(
            "{} tarjeta(s) tienen deuda en pesos y las cuentas de ahorro en pesos tienen saldo{}. MiChelitos no guarda la tasa de tus tarjetas ni el rendimiento de tus cuentas, así que no estima el costo de mantener la deuda ni lo que ganan los ahorros: compáralos con tus estados de cuenta.",
            con_deuda.len(),
            if cubre { " suficiente para cubrir esa deuda" } else { " pero menor que esa deuda" }
        ),
        regla_texto: format!(
            "Se activa si la deuda de tarjetas en pesos suma al menos DOP {} y hay saldo en cuentas de ahorro en pesos (no cuentan la caja de efectivo, las cuentas archivadas ni los dólares). Es de atención si el ahorro alcanza para cubrir la deuda. No se usa ninguna tasa que la aplicación no tenga guardada.",
            pesos_texto(DEUDA_MINIMA_CENTAVOS)
        ),
        cifras: vec![
            cifra("Deuda de tarjetas en pesos", ValorCifra::Importe(dop(deuda))),
            cifra("Tarjetas con deuda", ValorCifra::Texto(con_deuda.len().to_string())),
            cifra("Saldo en cuentas de ahorro en pesos", ValorCifra::Importe(dop(ahorro))),
        ],
        destino: Destino::Tarjetas,
        magnitud: deuda,
    });
}

// --- 3. Cargos derivados ---

fn regla_cargos_derivados(resumen: &ResumenMensual, (anio, mes): (i32, u32), acc: &mut Acumulador) {
    let total = resumen.totales.importe.centavos();
    if total <= 0 {
        acc.sin_datos(Regla::CargosDerivados, "No hay gastos en pesos en el último mes completo.");
        return;
    }
    acc.evaluadas.insert(Regla::CargosDerivados);
    let derivados = resumen.filas.iter().filter(|f| f.derivado).map(|f| f.medidas.importe.centavos()).sum::<i64>();
    if derivados < CARGOS_MINIMO_CENTAVOS || (derivados as i128) * 1000 < (total as i128) * CARGOS_INFO_MILESIMAS {
        return;
    }
    let atencion = (derivados as i128) * 1000 >= (total as i128) * CARGOS_ATENCION_MILESIMAS;
    let peso = dividir_redondeando(derivados as i128 * 1000, total as i128) as i64;
    acc.recomendaciones.push(Recomendacion {
        id: format!("cargos_derivados:{}", clave_de_periodo(anio, mes)),
        regla: Regla::CargosDerivados,
        severidad: if atencion { Severidad::Atencion } else { Severidad::Info },
        titulo: "Los cargos generados por otras operaciones pesan en tu gasto".to_string(),
        explicacion: format!(
            "En {} las comisiones de abonos y los cargos de avances de efectivo suman el {} del gasto en pesos del mes.",
            periodo(anio, mes),
            porcentaje_texto(peso)
        ),
        regla_texto: format!(
            "Se activa si esos cargos suman al menos DOP {} y el 5 % del gasto en pesos del último mes completo (informativa) o el 10 % (atención).",
            pesos_texto(CARGOS_MINIMO_CENTAVOS)
        ),
        cifras: vec![
            cifra("Mes", ValorCifra::Texto(periodo(anio, mes))),
            cifra("Cargos derivados", ValorCifra::Importe(dop(derivados))),
            cifra("Gasto total en pesos", ValorCifra::Importe(dop(total))),
            cifra("Peso", ValorCifra::Milesimas(peso)),
        ],
        destino: Destino::Tarjetas,
        magnitud: derivados,
    });
}

// --- 6. Gasto fijo y variable ---

fn regla_fijo_variable(resumen: &ResumenMensual, (anio, mes): (i32, u32), acc: &mut Acumulador) {
    let total = resumen.totales.importe.centavos();
    if total <= 0 {
        acc.sin_datos(Regla::FijoVariable, "No hay gastos en pesos en el último mes completo.");
        return;
    }
    if total < FIJO_VARIABLE_MINIMO_CENTAVOS {
        acc.sin_datos(Regla::FijoVariable, "El gasto en pesos del último mes completo es demasiado pequeño para comparar fijo y variable.");
        return;
    }
    let fijo = resumen.totales_fijo.importe.centavos();
    let variable = resumen.totales_variable.importe.centavos();
    if fijo == 0 {
        acc.sin_datos(
            Regla::FijoVariable,
            "Ninguna categoría con gasto está marcada como fija / recurrente; márcalas en Ajustes para comparar fijo y variable.",
        );
        return;
    }
    acc.evaluadas.insert(Regla::FijoVariable);
    let predomina_fijo = (fijo as i128) * 1000 >= (total as i128) * PREDOMINA_MILESIMAS;
    let predomina_variable = (variable as i128) * 1000 >= (total as i128) * PREDOMINA_MILESIMAS;
    if !predomina_fijo && !predomina_variable {
        return;
    }
    let (clave, titulo, explicacion, segmento) = if predomina_fijo {
        (
            "gasto_fijo_predomina",
            "La mayor parte de tu gasto es fijo / recurrente",
            "Las categorías que marcaste como fijas o recurrentes concentran la mayor parte del gasto en pesos del mes; cambiarlas suele exigir revisar contratos o suscripciones, no el consumo diario.",
            fijo,
        )
    } else {
        (
            "gasto_variable_predomina",
            "La mayor parte de tu gasto es variable",
            "El gasto que no marcaste como fijo (incluidos los cargos derivados) concentra la mayor parte del gasto en pesos del mes: es la parte que cambia de un mes a otro.",
            variable,
        )
    };
    acc.recomendaciones.push(Recomendacion {
        id: format!("{clave}:{}", clave_de_periodo(anio, mes)),
        regla: Regla::FijoVariable,
        severidad: Severidad::Info,
        titulo: titulo.to_string(),
        explicacion: format!("{explicacion} Mes: {}.", periodo(anio, mes)),
        regla_texto: format!(
            "El gasto en pesos del último mes completo (de al menos DOP {}) se reparte entre las categorías que tú marcaste como fijas y el resto; se avisa del segmento que pesa desde el 60 %. La marca es actual, no histórica.",
            pesos_texto(FIJO_VARIABLE_MINIMO_CENTAVOS)
        ),
        cifras: vec![
            cifra("Mes", ValorCifra::Texto(periodo(anio, mes))),
            cifra("Gasto fijo", ValorCifra::Importe(dop(fijo))),
            cifra("Gasto variable", ValorCifra::Importe(dop(variable))),
            cifra("Gasto total en pesos", ValorCifra::Importe(dop(total))),
            cifra("Peso fijo", ValorCifra::Milesimas(resumen.totales_fijo.porcentaje_milesimas)),
            cifra("Peso variable", ValorCifra::Milesimas(resumen.totales_variable.porcentaje_milesimas)),
        ],
        destino: Destino::Gastos,
        magnitud: segmento,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hoy() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 6).unwrap()
    }

    fn fila(anio: i32, mes: u32, cat: i64, nombre: &str, centavos: i64) -> FilaAgregadoGasto {
        FilaAgregadoGasto {
            anio,
            mes,
            dia: 9,
            categoria_id: cat,
            categoria_nombre: nombre.into(),
            divisa: "DOP".into(),
            derivado: false,
            fija: false,
            centavos,
            liquidado_origen_centavos: 0,
            liquidado_dop_centavos: None,
            n: 1,
        }
    }

    /// Meses 2026-05..=2026-10 de una categoría, con los importes dados (del más antiguo al más reciente).
    fn serie(cat: i64, nombre: &str, importes: [i64; 6]) -> Vec<FilaAgregadoGasto> {
        let meses = [(2026, 5), (2026, 6), (2026, 7), (2026, 8), (2026, 9), (2026, 10)];
        meses.iter().zip(importes).map(|(&(a, m), c)| fila(a, m, cat, nombre, c)).collect()
    }

    fn foto(anio: i32, mes: u32, activos: i64, pasivos: i64, calidad: &str, tasa: f64) -> FotoResumen {
        FotoResumen {
            anio,
            mes,
            activos: dop(activos),
            pasivos: dop(pasivos),
            patrimonio: dop(activos - pasivos),
            tasa_usd_dop: TasaCambio::nueva(tasa).unwrap(),
            calidad: calidad.into(),
        }
    }

    fn entrada<'a>(
        gastos: &'a [FilaAgregadoGasto],
        fotos: &'a [FotoResumen],
        presupuestos: &'a [Presupuesto],
        cuentas: &'a [CuentaDePatrimonio],
        tarjetas: &'a [TarjetaDePatrimonio],
    ) -> Entrada<'a> {
        Entrada { hoy: hoy(), fotos, gastos, presupuestos, cuentas, tarjetas }
    }

    fn vacia() -> Recomendaciones {
        evaluar(&entrada(&[], &[], &[], &[], &[])).unwrap()
    }

    fn con_id<'a>(r: &'a Recomendaciones, id: &str) -> Option<&'a Recomendacion> {
        r.visibles.iter().find(|x| x.id == id)
    }

    fn importe_de(r: &Recomendacion, etiqueta: &str) -> i64 {
        match &r.cifras.iter().find(|c| c.etiqueta == etiqueta).unwrap_or_else(|| panic!("falta la cifra «{etiqueta}»")).valor {
            ValorCifra::Importe(d) => d.centavos(),
            otro => panic!("{etiqueta} no es un importe: {otro:?}"),
        }
    }

    // --- Sin datos ---

    #[test]
    fn sin_ningun_dato_no_inventa_nada_y_dice_por_que() {
        let r = vacia();
        assert!(r.visibles.is_empty());
        assert_eq!((r.omitidas, r.reglas_evaluadas), (0, 0));
        let reglas: Vec<Regla> = r.sin_datos.iter().map(|s| s.regla).collect();
        assert_eq!(
            reglas,
            vec![Regla::Presupuesto, Regla::TendenciaGasto, Regla::VariacionPatrimonio, Regla::DeudaYAhorro, Regla::CargosDerivados, Regla::FijoVariable]
        );
        assert_eq!(r.mes_gastos, (2026, 10));
        assert_eq!(r.mes_presupuestos, (2026, 11));
        assert!(r.usd_excluido.is_empty());
    }

    #[test]
    fn en_enero_el_mes_de_referencia_es_diciembre_del_anio_anterior() {
        assert_eq!(mes_anterior(NaiveDate::from_ymd_opt(2027, 1, 15).unwrap()), (2026, 12));
        assert_eq!(mes_anterior(NaiveDate::from_ymd_opt(2026, 3, 1).unwrap()), (2026, 2));
    }

    // --- Tendencia ---

    #[test]
    fn una_categoria_variable_que_sube_se_recomienda_con_sus_cifras() {
        // previos 3 meses: 1000.03 cada uno; recientes: 1600.07 -> +60 % y +600.04.
        let gastos = serie(7, "Salidas inventadas", [100_003, 100_003, 100_003, 160_007, 160_007, 160_007]);
        let r = evaluar(&entrada(&gastos, &[], &[], &[], &[])).unwrap();
        let rec = con_id(&r, "tendencia_gasto:7").expect("debe recomendarse");
        assert_eq!((rec.severidad, rec.regla, rec.destino), (Severidad::Atencion, Regla::TendenciaGasto, Destino::Gastos));
        assert_eq!(importe_de(rec, "Promedio de los 3 últimos meses"), 160_007);
        assert_eq!(importe_de(rec, "Promedio de los 3 meses anteriores"), 100_003);
        assert_eq!(importe_de(rec, "Diferencia"), 60_004);
        assert!(rec.cifras.iter().any(|c| c.etiqueta == "Cambio" && c.valor == ValorCifra::Milesimas(600)));
        assert!(rec.titulo.contains("Salidas inventadas"));
        assert!(rec.regla_texto.contains("10 %") && rec.regla_texto.contains("500.00"));
        assert!(r.reglas_evaluadas >= 1);
    }

    #[test]
    fn una_categoria_fija_o_derivada_nunca_dispara_la_tendencia() {
        let mut gastos = serie(7, "Renta inventada", [100_003, 100_003, 100_003, 160_007, 160_007, 160_007]);
        for f in gastos.iter_mut() {
            f.fija = true;
        }
        let r = evaluar(&entrada(&gastos, &[], &[], &[], &[])).unwrap();
        assert!(con_id(&r, "tendencia_gasto:7").is_none());
        // Un derivado que sube tampoco.
        let mut derivados = serie(8, "Comisiones", [100_003, 100_003, 100_003, 160_007, 160_007, 160_007]);
        for f in derivados.iter_mut() {
            f.derivado = true;
        }
        let r = evaluar(&entrada(&derivados, &[], &[], &[], &[])).unwrap();
        assert!(r.visibles.iter().all(|x| x.regla != Regla::TendenciaGasto));
    }

    #[test]
    fn la_tendencia_respeta_el_minimo_de_500_pesos_y_el_umbral_del_10_por_ciento() {
        // +50 % pero solo 150.00 de diferencia: no cumple el mínimo absoluto.
        let poco = serie(1, "Pequeña", [30_001, 30_001, 30_001, 45_001, 45_001, 45_001]);
        assert!(evaluar(&entrada(&poco, &[], &[], &[], &[])).unwrap().visibles.is_empty());
        // Diferencia grande pero cambio de solo 5 %: tampoco.
        let relativo = serie(2, "Grande estable", [1_000_001, 1_000_001, 1_000_001, 1_050_001, 1_050_001, 1_050_001]);
        assert!(evaluar(&entrada(&relativo, &[], &[], &[], &[])).unwrap().visibles.is_empty());
    }

    #[test]
    fn con_menos_de_4_meses_la_tendencia_no_tiene_datos() {
        let gastos: Vec<_> = serie(7, "Corta", [0, 0, 0, 100_003, 160_007, 160_007]).into_iter().skip(3).collect();
        let r = evaluar(&entrada(&gastos, &[], &[], &[], &[])).unwrap();
        assert!(r.sin_datos.iter().any(|s| s.regla == Regla::TendenciaGasto));
        assert!(r.visibles.is_empty());
    }

    #[test]
    fn los_dolares_no_entran_en_la_tendencia_y_se_informan_aparte() {
        let mut gastos = serie(7, "Salidas inventadas", [100_003, 100_003, 100_003, 100_003, 100_003, 100_003]);
        // Un gasto enorme en dólares en el último mes: si se contara (o convirtiera), la categoría «subiría».
        let mut usd = fila(2026, 10, 7, "Salidas inventadas", 900_077);
        usd.divisa = "USD".into();
        gastos.push(usd);
        let r = evaluar(&entrada(&gastos, &[], &[], &[], &[])).unwrap();
        assert!(con_id(&r, "tendencia_gasto:7").is_none());
        assert_eq!(r.usd_excluido, vec![UsdExcluido { origen: "gastos", importe: Dinero::de_centavos(900_077, Divisa::Usd) }]);
    }

    // --- Presupuestos ---

    fn pres(cat: i64, nombre: &str, centavos: i64) -> Presupuesto {
        Presupuesto { categoria_id: cat, categoria_nombre: nombre.into(), fija: false, centavos }
    }

    #[test]
    fn un_presupuesto_cerca_es_informativo_y_uno_excedido_es_atencion_en_el_mes_en_curso() {
        let gastos = vec![fila(2026, 11, 1, "Comida inv.", 83_001), fila(2026, 11, 2, "Ocio inv.", 52_009), fila(2026, 11, 3, "Libros inv.", 10_001)];
        let presupuestos = vec![pres(1, "Comida inv.", 100_007), pres(2, "Ocio inv.", 50_003), pres(3, "Libros inv.", 100_007)];
        let r = evaluar(&entrada(&gastos, &[], &presupuestos, &[], &[])).unwrap();
        let cerca = con_id(&r, "presupuesto_cerca:1").unwrap();
        let excedido = con_id(&r, "presupuesto_excedido:2").unwrap();
        assert_eq!((cerca.severidad, excedido.severidad), (Severidad::Info, Severidad::Atencion));
        assert!(con_id(&r, "presupuesto_cerca:3").is_none() && con_id(&r, "presupuesto_excedido:3").is_none(), "dentro del presupuesto: nada");
        assert_eq!(importe_de(excedido, "Exceso"), 52_009 - 50_003);
        assert_eq!(importe_de(cerca, "Restante"), 100_007 - 83_001);
        assert!(excedido.cifras.iter().any(|c| c.etiqueta == "Avance" && c.valor == ValorCifra::Milesimas(1040)));
        // Atención antes que informativa.
        assert_eq!(r.visibles[0].id, "presupuesto_excedido:2");
    }

    #[test]
    fn el_presupuesto_no_cuenta_dolares_pero_los_informa() {
        let mut usd = fila(2026, 11, 1, "Comida inv.", 999_999);
        usd.divisa = "USD".into();
        let gastos = vec![fila(2026, 11, 1, "Comida inv.", 85_000), usd];
        let r = evaluar(&entrada(&gastos, &[], &[pres(1, "Comida inv.", 100_000)], &[], &[])).unwrap();
        let rec = con_id(&r, "presupuesto_cerca:1").unwrap();
        assert_eq!(importe_de(rec, "Gastado en pesos"), 85_000);
        assert_eq!(importe_de(rec, "Dólares aparte (no cuentan)"), 999_999);
    }

    #[test]
    fn sin_presupuestos_la_regla_dice_que_no_tiene_datos() {
        let r = evaluar(&entrada(&[fila(2026, 11, 1, "x", 5)], &[], &[], &[], &[])).unwrap();
        assert!(r.sin_datos.iter().any(|s| s.regla == Regla::Presupuesto));
    }

    // --- Cargos derivados ---

    fn gastos_de_octubre(variables: i64, derivados: i64) -> Vec<FilaAgregadoGasto> {
        let mut d = fila(2026, 10, 5, "Comisión", derivados);
        d.derivado = true;
        vec![fila(2026, 10, 1, "Comida inv.", variables), d]
    }

    #[test]
    fn los_cargos_derivados_se_activan_en_exactamente_5_y_10_por_ciento() {
        // Total 400 000: 5 % = 20 000 (límite exacto, e igual al mínimo de 200.00).
        let r = evaluar(&entrada(&gastos_de_octubre(380_000, 20_000), &[], &[], &[], &[])).unwrap();
        let rec = con_id(&r, "cargos_derivados:2026-10").expect("5 % exacto activa");
        assert_eq!(rec.severidad, Severidad::Info);
        // Un centavo menos: no se activa.
        let r = evaluar(&entrada(&gastos_de_octubre(380_001, 19_999), &[], &[], &[], &[])).unwrap();
        assert!(con_id(&r, "cargos_derivados:2026-10").is_none());
        // 10 % exacto: atención.
        let r = evaluar(&entrada(&gastos_de_octubre(360_000, 40_000), &[], &[], &[], &[])).unwrap();
        assert_eq!(con_id(&r, "cargos_derivados:2026-10").unwrap().severidad, Severidad::Atencion);
        // 9.99 %: informativa.
        let r = evaluar(&entrada(&gastos_de_octubre(360_100, 39_900), &[], &[], &[], &[])).unwrap();
        assert_eq!(con_id(&r, "cargos_derivados:2026-10").unwrap().severidad, Severidad::Info);
    }

    #[test]
    fn los_cargos_derivados_exigen_el_minimo_absoluto() {
        // 50 % del gasto, pero solo 150.00 en cargos: no llega a DOP 200.00.
        let r = evaluar(&entrada(&gastos_de_octubre(15_000, 15_000), &[], &[], &[], &[])).unwrap();
        assert!(con_id(&r, "cargos_derivados:2026-10").is_none());
    }

    #[test]
    fn el_peso_de_los_cargos_se_redondea_a_milesimas_sin_cambiar_la_decision() {
        let r = evaluar(&entrada(&gastos_de_octubre(360_000, 40_001), &[], &[], &[], &[])).unwrap();
        let rec = con_id(&r, "cargos_derivados:2026-10").unwrap();
        assert!(rec.cifras.iter().any(|c| c.etiqueta == "Peso" && c.valor == ValorCifra::Milesimas(100)));
        assert_eq!(importe_de(rec, "Gasto total en pesos"), 400_001);
    }

    // --- Fijo y variable ---

    fn con_fija(mut f: FilaAgregadoGasto) -> FilaAgregadoGasto {
        f.fija = true;
        f
    }

    #[test]
    fn el_gasto_fijo_predomina_desde_el_60_por_ciento_exacto() {
        // Total 100 000: fijo 60 000 exactos.
        let gastos = vec![con_fija(fila(2026, 10, 1, "Alquiler inv.", 60_000)), fila(2026, 10, 2, "Comida inv.", 40_000)];
        let r = evaluar(&entrada(&gastos, &[], &[], &[], &[])).unwrap();
        let rec = con_id(&r, "gasto_fijo_predomina:2026-10").expect("60 % exacto");
        assert_eq!((rec.severidad, importe_de(rec, "Gasto fijo"), importe_de(rec, "Gasto variable")), (Severidad::Info, 60_000, 40_000));
        // 59.999 %: no.
        let gastos = vec![con_fija(fila(2026, 10, 1, "Alquiler inv.", 59_999)), fila(2026, 10, 2, "Comida inv.", 40_001)];
        let r = evaluar(&entrada(&gastos, &[], &[], &[], &[])).unwrap();
        assert!(r.visibles.is_empty());
        assert!(r.reglas_evaluadas >= 1);
    }

    #[test]
    fn el_gasto_variable_predomina_solo_si_hay_algo_marcado_como_fijo() {
        let gastos = vec![con_fija(fila(2026, 10, 1, "Alquiler inv.", 20_000)), fila(2026, 10, 2, "Comida inv.", 80_000)];
        let r = evaluar(&entrada(&gastos, &[], &[], &[], &[])).unwrap();
        assert!(con_id(&r, "gasto_variable_predomina:2026-10").is_some());
        // Sin ninguna categoría fija: no hay razón que comparar, y la regla lo dice.
        let gastos = vec![fila(2026, 10, 2, "Comida inv.", 80_000)];
        let r = evaluar(&entrada(&gastos, &[], &[], &[], &[])).unwrap();
        assert!(r.visibles.is_empty());
        let motivo = &r.sin_datos.iter().find(|s| s.regla == Regla::FijoVariable).unwrap().motivo;
        assert!(motivo.contains("fija"), "{motivo}");
    }

    #[test]
    fn un_gasto_total_pequeno_no_evalua_la_razon_fijo_variable() {
        let gastos = vec![con_fija(fila(2026, 10, 1, "Alquiler inv.", 40_000)), fila(2026, 10, 2, "Comida inv.", 9_999)];
        let r = evaluar(&entrada(&gastos, &[], &[], &[], &[])).unwrap();
        assert!(r.sin_datos.iter().any(|s| s.regla == Regla::FijoVariable));
        assert!(r.visibles.is_empty());
    }

    // --- Variación del patrimonio ---

    #[test]
    fn con_menos_de_dos_fotos_no_hay_variacion() {
        let fotos = [foto(2026, 10, 500_001, 100_003, "completa", 60.0)];
        let r = evaluar(&entrada(&[], &fotos, &[], &[], &[])).unwrap();
        assert!(r.sin_datos.iter().any(|s| s.regla == Regla::VariacionPatrimonio));
    }

    #[test]
    fn una_baja_por_pasivos_es_atencion_y_apunta_a_las_deudas() {
        let fotos = [foto(2026, 9, 500_001, 100_003, "completa", 60.0), foto(2026, 10, 505_001, 250_003, "completa", 60.0)];
        let r = evaluar(&entrada(&[], &fotos, &[], &[], &[])).unwrap();
        let rec = con_id(&r, "variacion_patrimonio:2026-10").unwrap();
        assert_eq!((rec.severidad, rec.destino), (Severidad::Atencion, Destino::Prestamos));
        assert_eq!(importe_de(rec, "Cambio del patrimonio"), (505_001 - 250_003) - (500_001 - 100_003));
        assert_eq!(importe_de(rec, "Cambio en activos"), 5_000);
        assert_eq!(importe_de(rec, "Cambio en pasivos"), 150_000);
        assert!(rec.titulo.contains("bajó") && rec.titulo.contains("pasivos"));
        assert!(!rec.cifras.iter().any(|c| c.etiqueta.starts_with("Tasa USD")), "misma tasa: sin cifra de tasa");
    }

    #[test]
    fn una_subida_por_activos_es_informativa_y_empata_a_favor_de_los_activos() {
        let fotos = [foto(2026, 9, 500_001, 100_003, "completa", 60.0), foto(2026, 10, 700_001, 100_003, "completa", 60.0)];
        let r = evaluar(&entrada(&[], &fotos, &[], &[], &[])).unwrap();
        let rec = con_id(&r, "variacion_patrimonio:2026-10").unwrap();
        assert_eq!((rec.severidad, rec.destino), (Severidad::Info, Destino::Resumen));
        // Empate exacto de |Δactivos| y |Δpasivos|: gana activos.
        let fotos = [foto(2026, 9, 500_000, 100_000, "completa", 60.0), foto(2026, 10, 550_000, 50_000, "completa", 60.0)];
        let rec = evaluar(&entrada(&[], &fotos, &[], &[], &[])).unwrap().visibles.remove(0);
        assert!(rec.titulo.contains("activos"), "{}", rec.titulo);
    }

    #[test]
    fn una_foto_con_aviso_baja_la_severidad_y_una_tasa_distinta_se_senala() {
        let fotos = [foto(2026, 9, 500_001, 100_003, "completa", 58.5), foto(2026, 10, 300_001, 100_003, "con_aviso", 60.0)];
        let rec = evaluar(&entrada(&[], &fotos, &[], &[], &[])).unwrap().visibles.remove(0);
        assert_eq!(rec.severidad, Severidad::Info, "baja pero con una foto con aviso");
        assert!(rec.explicacion.contains("con aviso") && rec.explicacion.contains("cambiario"));
        assert!(rec.cifras.iter().any(|c| c.etiqueta == "Tasa USD de la foto anterior"));
        assert!(rec.cifras.iter().any(|c| c.etiqueta == "Calidad de las fotos" && c.valor == ValorCifra::Texto("completa / con_aviso".into())));
    }

    #[test]
    fn sin_cambio_de_patrimonio_no_hay_recomendacion_pero_la_regla_cuenta_como_evaluada() {
        let fotos = [foto(2026, 9, 500_001, 100_003, "completa", 60.0), foto(2026, 10, 500_001, 100_003, "completa", 60.0)];
        let r = evaluar(&entrada(&[], &fotos, &[], &[], &[])).unwrap();
        assert!(r.visibles.is_empty());
        assert_eq!(r.reglas_evaluadas, 1);
    }

    #[test]
    fn las_fotos_desordenadas_se_comparan_por_fecha() {
        let fotos = [foto(2026, 10, 700_001, 100_003, "completa", 60.0), foto(2026, 8, 100_001, 0, "completa", 60.0), foto(2026, 9, 500_001, 100_003, "completa", 60.0)];
        let rec = evaluar(&entrada(&[], &fotos, &[], &[], &[])).unwrap().visibles.remove(0);
        assert_eq!(importe_de(&rec, "Patrimonio anterior"), 500_001 - 100_003);
        assert_eq!(importe_de(&rec, "Patrimonio reciente"), 700_001 - 100_003);
    }

    // --- Deuda y ahorro ---

    fn cuenta(nombre: &str, saldo: i64, divisa: Divisa, efectivo: bool, archivada: bool) -> CuentaDePatrimonio {
        CuentaDePatrimonio { nombre: nombre.into(), saldo: Dinero::de_centavos(saldo, divisa), es_efectivo: efectivo, archivada }
    }

    fn tarjeta(nombre: &str, pesos: i64, dolares: i64) -> TarjetaDePatrimonio {
        TarjetaDePatrimonio {
            nombre: nombre.into(),
            activa: true,
            balance_dop: dop(pesos),
            balance_usd: Dinero::de_centavos(dolares, Divisa::Usd),
        }
    }

    #[test]
    fn deuda_con_ahorro_suficiente_es_atencion_y_no_inventa_costos() {
        let cuentas = [cuenta("Ahorro inv.", 900_001, Divisa::Dop, false, false)];
        let tarjetas = [tarjeta("Banco X Visa", 250_003, 0), tarjeta("Banco Y", 100_007, 0)];
        let r = evaluar(&entrada(&[], &[], &[], &cuentas, &tarjetas)).unwrap();
        let rec = con_id(&r, "deuda_y_ahorro").unwrap();
        assert_eq!((rec.severidad, rec.destino), (Severidad::Atencion, Destino::Tarjetas));
        assert_eq!(importe_de(rec, "Deuda de tarjetas en pesos"), 350_010);
        assert_eq!(importe_de(rec, "Saldo en cuentas de ahorro en pesos"), 900_001);
        assert!(rec.explicacion.contains("no guarda la tasa"), "{}", rec.explicacion);
        // Ninguna cifra es un costo estimado.
        assert!(rec.cifras.iter().all(|c| !c.etiqueta.to_lowercase().contains("costo") && !c.etiqueta.to_lowercase().contains("interés")));
    }

    #[test]
    fn el_ahorro_exactamente_igual_a_la_deuda_ya_la_cubre() {
        let tarjetas = [tarjeta("Banco X Visa", 250_003, 0)];
        let justo = [cuenta("Ahorro inv.", 250_003, Divisa::Dop, false, false)];
        assert_eq!(con_id(&evaluar(&entrada(&[], &[], &[], &justo, &tarjetas)).unwrap(), "deuda_y_ahorro").unwrap().severidad, Severidad::Atencion);
        let un_centavo_menos = [cuenta("Ahorro inv.", 250_002, Divisa::Dop, false, false)];
        assert_eq!(con_id(&evaluar(&entrada(&[], &[], &[], &un_centavo_menos, &tarjetas)).unwrap(), "deuda_y_ahorro").unwrap().severidad, Severidad::Info);
    }

    #[test]
    fn deuda_mayor_que_el_ahorro_es_informativa() {
        let cuentas = [cuenta("Ahorro inv.", 50_001, Divisa::Dop, false, false)];
        let tarjetas = [tarjeta("Banco X Visa", 250_003, 0)];
        let r = evaluar(&entrada(&[], &[], &[], &cuentas, &tarjetas)).unwrap();
        assert_eq!(con_id(&r, "deuda_y_ahorro").unwrap().severidad, Severidad::Info);
    }

    #[test]
    fn la_caja_de_efectivo_las_archivadas_los_dolares_y_los_saldos_a_favor_no_cuentan() {
        let cuentas = [
            cuenta("Caja", 900_001, Divisa::Dop, true, false),
            cuenta("Vieja", 900_001, Divisa::Dop, false, true),
            cuenta("Dólares inv.", 900_001, Divisa::Usd, false, false),
        ];
        let tarjetas = [tarjeta("Banco X Visa", 250_003, 7_777), tarjeta("A favor", -50_000, 0)];
        let r = evaluar(&entrada(&[], &[], &[], &cuentas, &tarjetas)).unwrap();
        assert!(r.visibles.is_empty(), "sin ahorro en pesos no hay regla");
        // Los dólares se informan aparte, sin convertir.
        let origenes: Vec<&str> = r.usd_excluido.iter().map(|u| u.origen).collect();
        assert_eq!(origenes, vec!["tarjetas", "cuentas"]);
        // El saldo a favor no reduce la deuda de otra tarjeta: solo suma deuda positiva.
        let cuentas = [cuenta("Ahorro inv.", 900_001, Divisa::Dop, false, false)];
        let rec = evaluar(&entrada(&[], &[], &[], &cuentas, &tarjetas)).unwrap().visibles.remove(0);
        assert_eq!(importe_de(&rec, "Deuda de tarjetas en pesos"), 250_003);
    }

    #[test]
    fn solo_con_la_caja_de_efectivo_o_sin_tarjetas_la_regla_no_tiene_datos() {
        let caja = [cuenta("Caja", 900_001, Divisa::Dop, true, false)];
        let r = evaluar(&entrada(&[], &[], &[], &caja, &[tarjeta("T", 250_003, 0)])).unwrap();
        assert!(r.sin_datos.iter().any(|s| s.regla == Regla::DeudaYAhorro), "sin cuenta de ahorro");
        let ahorro = [cuenta("Ahorro inv.", 900_001, Divisa::Dop, false, false)];
        let r = evaluar(&entrada(&[], &[], &[], &ahorro, &[])).unwrap();
        assert!(r.sin_datos.iter().any(|s| s.regla == Regla::DeudaYAhorro), "sin tarjetas");
        assert_eq!(r.reglas_evaluadas, 0);
    }

    #[test]
    fn una_deuda_bajo_el_minimo_no_activa_la_regla() {
        let cuentas = [cuenta("Ahorro inv.", 900_001, Divisa::Dop, false, false)];
        let r = evaluar(&entrada(&[], &[], &[], &cuentas, &[tarjeta("T", 9_999, 0)])).unwrap();
        assert!(r.visibles.is_empty());
        let r = evaluar(&entrada(&[], &[], &[], &cuentas, &[tarjeta("T", 10_000, 0)])).unwrap();
        assert!(con_id(&r, "deuda_y_ahorro").is_some(), "el mínimo exacto sí");
    }

    // --- Orden, límites y determinismo ---

    #[test]
    fn se_muestran_a_lo_sumo_cinco_por_severidad_y_el_resto_se_cuenta() {
        let mut gastos = Vec::new();
        let mut presupuestos = Vec::new();
        for i in 1..=8 {
            gastos.push(fila(2026, 11, i, &format!("Cat {i}"), 100_000 + i * 1_001));
            presupuestos.push(pres(i, &format!("Cat {i}"), 50_000));
        }
        let r = evaluar(&entrada(&gastos, &[], &presupuestos, &[], &[])).unwrap();
        assert_eq!(r.visibles.len(), MAXIMO_POR_SEVERIDAD);
        assert_eq!(r.omitidas, 3);
        // Los de mayor avance primero (la magnitud desempata dentro de una regla).
        assert_eq!(r.visibles[0].id, "presupuesto_excedido:8");
        assert!(r.visibles.windows(2).all(|w| w[0].magnitud >= w[1].magnitud));
    }

    #[test]
    fn atencion_va_antes_que_info_y_el_resultado_es_determinista() {
        let gastos = vec![fila(2026, 11, 1, "Comida inv.", 85_000), fila(2026, 11, 2, "Ocio inv.", 70_000)];
        let presupuestos = vec![pres(1, "Comida inv.", 100_000), pres(2, "Ocio inv.", 50_000)];
        let fotos = [foto(2026, 9, 500_001, 100_003, "completa", 60.0), foto(2026, 10, 505_001, 250_003, "completa", 60.0)];
        let entrada_comun = entrada(&gastos, &fotos, &presupuestos, &[], &[]);
        let a = evaluar(&entrada_comun).unwrap();
        let b = evaluar(&entrada_comun).unwrap();
        assert_eq!(a, b);
        let severidades: Vec<Severidad> = a.visibles.iter().map(|r| r.severidad).collect();
        let mut ordenadas = severidades.clone();
        ordenadas.sort();
        assert_eq!(severidades, ordenadas, "atención primero");
        // Dentro de atención, la regla de presupuestos va antes que la del patrimonio.
        assert_eq!(a.visibles[0].regla, Regla::Presupuesto);
        assert_eq!(a.visibles[1].regla, Regla::VariacionPatrimonio);
    }

    #[test]
    fn los_identificadores_son_estables_y_unicos() {
        let gastos = vec![fila(2026, 11, 1, "Comida inv.", 85_000), fila(2026, 11, 2, "Ocio inv.", 70_000)];
        let presupuestos = vec![pres(1, "Comida inv.", 100_000), pres(2, "Ocio inv.", 50_000)];
        let r = evaluar(&entrada(&gastos, &[], &presupuestos, &[], &[])).unwrap();
        let ids: Vec<&str> = r.visibles.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, vec!["presupuesto_excedido:2", "presupuesto_cerca:1"]);
        assert_eq!(ids.iter().collect::<BTreeSet<_>>().len(), ids.len());
    }

    #[test]
    fn las_etiquetas_de_regla_destino_y_severidad_son_las_del_contrato() {
        assert_eq!(Regla::TendenciaGasto.codigo(), "tendencia_gasto");
        assert_eq!(Regla::FijoVariable.codigo(), "fijo_variable");
        assert_eq!((Severidad::Atencion.codigo(), Severidad::Info.codigo()), ("atencion", "info"));
        let rutas = [Destino::Gastos, Destino::Tarjetas, Destino::Prestamos, Destino::Resumen, Destino::Cuentas, Destino::Evolucion].map(Destino::ruta);
        assert_eq!(rutas, ["gastos", "tarjetas", "prestamos", "resumen", "cuentas", "evolucion"]);
    }

    #[test]
    fn el_porcentaje_se_trunca_por_division_entera() {
        assert_eq!(porcentaje_texto(815), "81.5 %");
        assert_eq!(porcentaje_texto(1040), "104.0 %");
        assert_eq!(porcentaje_texto(-25), "-2.5 %");
    }
}
