//! El patrimonio, calculado en un solo lugar y en centavos enteros.
//!
//! Antes no existía una fórmula única: el «Patrimonio Total» del Dashboard era solo el capital; el «Patrimonio Neto»
//! de Resumen se componía en la interfaz, en `f64`, restando tarjetas (con los dólares a la tasa fija de 60) y
//! préstamos; y las cuentas de ahorro y el efectivo no entraban en ninguna. Esta función pura es esa composición.
//!
//! Da **dos** cifras, durante una versión, para que el titular las compare:
//!
//! * `neto_actual` = capital − tarjetas − préstamos: exactamente la fórmula vigente de Resumen.
//! * `neto_con_liquidez` = `neto_actual` + cuentas de ahorro + efectivo (decisión del titular, D1).
//!
//! ## Qué suma y qué no
//!
//! * Cuentas archivadas y tarjetas inactivas **suman si tienen saldo**; con saldo cero no aportan nada. Archivar no
//!   hace desaparecer el dinero que la cuenta guarda.
//! * Las cuentas en dólares y los dólares de las tarjetas se convierten con la tasa que se pasa por parámetro, que es
//!   una **estimación** (la constante de la interfaz), no una cotización. El importe original en dólares se conserva
//!   aparte en cada partida.
//! * El capital llega ya sumado (`Totales::patrimonio`, en `aplicacion::guardar_capital`): no se duplica su suma.
//!
//! ## Redondeo
//!
//! Cada partida en dólares se convierte y redondea **por separado** al centavo, y el total es la suma de lo
//! redondeado (así el desglose cuadra con el total). La interfaz anterior sumaba sin redondear y mostraba el total
//! redondeado una vez. Con la tasa vigente (60, entera) ambos coinciden siempre; con una tasa de centésimas de
//! centavo pueden diferir en un centavo (ver la prueba `hallazgo_redondeo_por_partida`).

use super::dinero::{Dinero, Divisa, TasaCambio};
use super::errores::ErrorDominio;

/// La tasa estimada con la que se expresan los dólares en pesos. **La misma** que `TASA_USD_A_DOP` de
/// `src/js/ui/servicios.ts` (una prueba lo vigila): dos copias de una tasa se desincronizan.
pub const TASA_ESTIMADA_USD_A_DOP: f64 = 60.0;

pub fn tasa_estimada() -> TasaCambio {
    TasaCambio::nueva(TASA_ESTIMADA_USD_A_DOP).expect("la tasa estimada es positiva")
}

#[derive(Debug, Clone, PartialEq)]
pub struct CuentaDePatrimonio {
    pub nombre: String,
    pub saldo: Dinero,
    pub es_efectivo: bool,
    pub archivada: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TarjetaDePatrimonio {
    pub nombre: String,
    pub activa: bool,
    pub balance_dop: Dinero,
    pub balance_usd: Dinero,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrestamoDePatrimonio {
    pub nombre: String,
    pub saldo: Dinero,
}

#[derive(Debug, Clone)]
pub struct EntradasDePatrimonio {
    /// El capital ya sumado, en pesos (`Totales::patrimonio`).
    pub capital: Dinero,
    pub cuentas: Vec<CuentaDePatrimonio>,
    pub tarjetas: Vec<TarjetaDePatrimonio>,
    pub prestamos: Vec<PrestamoDePatrimonio>,
    pub tasa: TasaCambio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoDePartida {
    Activo,
    Pasivo,
}

impl TipoDePartida {
    pub fn codigo(&self) -> &'static str {
        match self {
            TipoDePartida::Activo => "activo",
            TipoDePartida::Pasivo => "pasivo",
        }
    }
}

/// Una partida: lo que es, su importe original (en su divisa) y su equivalente en pesos; el tipo dice si suma o resta.
#[derive(Debug, Clone, PartialEq)]
pub struct Partida {
    pub tipo: TipoDePartida,
    /// `capital`, `cuenta`, `efectivo`, `tarjeta` o `prestamo`.
    pub rubro: &'static str,
    pub nombre: String,
    pub original: Dinero,
    pub en_dop: Dinero,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Foto {
    pub tasa: TasaCambio,
    pub capital: Dinero,
    /// Todas las cuentas en pesos (efectivo incluido), archivadas con saldo incluidas.
    pub liquidez_dop: Dinero,
    /// Todas las cuentas en dólares, en dólares.
    pub liquidez_usd: Dinero,
    pub liquidez_usd_en_dop: Dinero,
    /// De la liquidez, lo que es caja de efectivo.
    pub efectivo_dop: Dinero,
    pub efectivo_usd: Dinero,
    pub tarjetas_dop: Dinero,
    pub tarjetas_usd: Dinero,
    pub tarjetas_usd_en_dop: Dinero,
    pub prestamos: Dinero,
    pub total_pasivos: Dinero,
    pub neto_actual: Dinero,
    pub neto_con_liquidez: Dinero,
    pub partidas: Vec<Partida>,
}

fn dop(centavos: i64) -> Dinero {
    Dinero::de_centavos(centavos, Divisa::Dop)
}

fn usd(centavos: i64) -> Dinero {
    Dinero::de_centavos(centavos, Divisa::Usd)
}

/// Compone las dos cifras. Pura: no lee nada y no escribe nada.
pub fn componer(e: &EntradasDePatrimonio) -> Result<Foto, ErrorDominio> {
    let mut partidas: Vec<Partida> = Vec::new();
    let mut punto = |tipo, rubro, nombre: &str, original: Dinero| -> Result<Dinero, ErrorDominio> {
        let en_dop = original.convertir(Divisa::Dop, e.tasa)?;
        partidas.push(Partida { tipo, rubro, nombre: nombre.to_string(), original, en_dop });
        Ok(en_dop)
    };

    let capital = punto(TipoDePartida::Activo, "capital", "Capital declarado", e.capital)?;

    let (mut liquidez_dop, mut liquidez_usd, mut liquidez_usd_en_dop) = (dop(0), usd(0), dop(0));
    let (mut efectivo_dop, mut efectivo_usd) = (dop(0), usd(0));
    for c in &e.cuentas {
        let rubro = if c.es_efectivo { "efectivo" } else { "cuenta" };
        let nombre = if c.archivada { format!("{} (archivada)", c.nombre) } else { c.nombre.clone() };
        let en_dop = punto(TipoDePartida::Activo, rubro, &nombre, c.saldo)?;
        match c.saldo.divisa() {
            Divisa::Dop => {
                liquidez_dop = liquidez_dop.sumar(&en_dop)?;
                if c.es_efectivo {
                    efectivo_dop = efectivo_dop.sumar(&c.saldo)?;
                }
            }
            Divisa::Usd => {
                liquidez_usd = liquidez_usd.sumar(&c.saldo)?;
                liquidez_usd_en_dop = liquidez_usd_en_dop.sumar(&en_dop)?;
                if c.es_efectivo {
                    efectivo_usd = efectivo_usd.sumar(&c.saldo)?;
                }
            }
        }
    }

    let (mut tarjetas_dop, mut tarjetas_usd, mut tarjetas_usd_en_dop) = (dop(0), usd(0), dop(0));
    for t in &e.tarjetas {
        let nombre = if t.activa { t.nombre.clone() } else { format!("{} (inactiva)", t.nombre) };
        if !t.balance_dop.es_cero() {
            punto(TipoDePartida::Pasivo, "tarjeta", &nombre, t.balance_dop)?;
            tarjetas_dop = tarjetas_dop.sumar(&t.balance_dop)?;
        }
        if !t.balance_usd.es_cero() {
            let en_dop = punto(TipoDePartida::Pasivo, "tarjeta", &format!("{nombre} (USD)"), t.balance_usd)?;
            tarjetas_usd = tarjetas_usd.sumar(&t.balance_usd)?;
            tarjetas_usd_en_dop = tarjetas_usd_en_dop.sumar(&en_dop)?;
        }
    }

    let mut prestamos = dop(0);
    for p in &e.prestamos {
        let en_dop = punto(TipoDePartida::Pasivo, "prestamo", &p.nombre, p.saldo)?;
        prestamos = prestamos.sumar(&en_dop)?;
    }

    let total_pasivos = tarjetas_dop.sumar(&tarjetas_usd_en_dop)?.sumar(&prestamos)?;
    let neto_actual = capital.restar(&total_pasivos)?;
    let liquidez = liquidez_dop.sumar(&liquidez_usd_en_dop)?;
    let neto_con_liquidez = neto_actual.sumar(&liquidez)?;

    Ok(Foto {
        tasa: e.tasa,
        capital,
        liquidez_dop,
        liquidez_usd,
        liquidez_usd_en_dop,
        efectivo_dop,
        efectivo_usd,
        tarjetas_dop,
        tarjetas_usd,
        tarjetas_usd_en_dop,
        prestamos,
        total_pasivos,
        neto_actual,
        neto_con_liquidez,
        partidas,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn d(c: i64) -> Dinero {
        dop(c)
    }

    fn cuenta(nombre: &str, saldo: Dinero, efectivo: bool, archivada: bool) -> CuentaDePatrimonio {
        CuentaDePatrimonio { nombre: nombre.into(), saldo, es_efectivo: efectivo, archivada }
    }

    fn tarjeta(nombre: &str, activa: bool, pesos: i64, dolares: i64) -> TarjetaDePatrimonio {
        TarjetaDePatrimonio { nombre: nombre.into(), activa, balance_dop: d(pesos), balance_usd: usd(dolares) }
    }

    fn vacias() -> EntradasDePatrimonio {
        EntradasDePatrimonio { capital: d(0), cuentas: vec![], tarjetas: vec![], prestamos: vec![], tasa: tasa_estimada() }
    }

    #[test]
    fn caracterizacion_la_formula_vigente_resta_tarjetas_con_dolares_a_60_y_prestamos() {
        // capital 1 234 567.89; tarjeta: 10 000.37 DOP y USD 100.19; préstamo 50 000.11
        let e = EntradasDePatrimonio {
            capital: d(123_456_789),
            tarjetas: vec![tarjeta("T1", true, 1_000_037, 10_019)],
            prestamos: vec![PrestamoDePatrimonio { nombre: "P1".into(), saldo: d(5_000_011) }],
            ..vacias()
        };
        let f = componer(&e).unwrap();
        // 100.19 * 60 = 6 011.40
        assert_eq!(f.tarjetas_usd_en_dop, d(601_140));
        assert_eq!(f.total_pasivos, d(1_000_037 + 601_140 + 5_000_011));
        assert_eq!(f.neto_actual, d(123_456_789 - 1_000_037 - 601_140 - 5_000_011));
        // Sin cuentas, la cifra con liquidez es la misma.
        assert_eq!(f.neto_con_liquidez, f.neto_actual);
    }

    #[test]
    fn la_cifra_con_liquidez_suma_cuentas_y_efectivo() {
        let e = EntradasDePatrimonio {
            capital: d(1_000_001),
            cuentas: vec![
                cuenta("Ahorro", d(250_033), false, false),
                cuenta("Efectivo DOP", d(7_777), true, false),
                cuenta("Ahorro USD", usd(12_345), false, false),
                cuenta("Efectivo USD", usd(101), true, false),
            ],
            ..vacias()
        };
        let f = componer(&e).unwrap();
        assert_eq!(f.liquidez_dop, d(257_810));
        assert_eq!(f.liquidez_usd, usd(12_446));
        assert_eq!(f.liquidez_usd_en_dop, d(746_760));
        assert_eq!((f.efectivo_dop, f.efectivo_usd), (d(7_777), usd(101)));
        assert_eq!(f.neto_actual, d(1_000_001));
        assert_eq!(f.neto_con_liquidez, d(1_000_001 + 257_810 + 746_760));
    }

    #[test]
    fn archivadas_e_inactivas_suman_solo_si_tienen_saldo() {
        let e = EntradasDePatrimonio {
            cuentas: vec![cuenta("Vieja", d(0), false, true), cuenta("Vieja con saldo", d(4_321), false, true)],
            tarjetas: vec![tarjeta("Cerrada", false, 0, 0), tarjeta("Cerrada con deuda", false, 1_234, 0)],
            ..vacias()
        };
        let f = componer(&e).unwrap();
        assert_eq!(f.liquidez_dop, d(4_321));
        assert_eq!(f.total_pasivos, d(1_234));
        assert_eq!(f.neto_con_liquidez, d(4_321 - 1_234));
        assert!(f.partidas.iter().any(|p| p.nombre == "Vieja con saldo (archivada)"));
        assert!(f.partidas.iter().any(|p| p.nombre == "Cerrada con deuda (inactiva)"));
        assert!(!f.partidas.iter().any(|p| p.nombre.starts_with("Cerrada (")), "la tarjeta a cero no genera partida");
    }

    #[test]
    fn el_pasivo_resta_y_el_saldo_a_favor_de_una_tarjeta_suma() {
        let e = EntradasDePatrimonio { tarjetas: vec![tarjeta("A", true, -2_500, -100)], ..vacias() };
        let f = componer(&e).unwrap();
        // Saldo a favor: pasivo negativo, el neto sube (igual que la resta vigente).
        assert_eq!(f.total_pasivos, d(-2_500 - 6_000));
        assert_eq!(f.neto_actual, d(8_500));
    }

    #[test]
    fn el_desglose_por_partida_cuadra_con_las_cifras() {
        let e = EntradasDePatrimonio {
            capital: d(9_999_901),
            cuentas: vec![cuenta("C", d(33_333), false, false), cuenta("CU", usd(777), false, false)],
            tarjetas: vec![tarjeta("T", true, 12_345, 4_321)],
            prestamos: vec![PrestamoDePatrimonio { nombre: "P".into(), saldo: d(88_888) }],
            ..vacias()
        };
        let f = componer(&e).unwrap();
        let suma = |t| f.partidas.iter().filter(|p| p.tipo == t).fold(0, |a, p| a + p.en_dop.centavos());
        assert_eq!(suma(TipoDePartida::Activo) - suma(TipoDePartida::Pasivo), f.neto_con_liquidez.centavos());
    }

    #[test]
    fn hallazgo_redondeo_por_partida() {
        // Con una tasa de centésimas de centavo, redondear cada partida y sumar difiere en un centavo de
        // sumar los dólares y redondear una vez (lo que hacía la interfaz en f64). Con la tasa vigente (60) no.
        let tasa = TasaCambio::nueva(58.755).unwrap();
        let e = EntradasDePatrimonio {
            tarjetas: vec![tarjeta("A", true, 0, 3), tarjeta("B", true, 0, 3)],
            tasa,
            ..vacias()
        };
        let f = componer(&e).unwrap();
        let de_una_vez = usd(6).convertir(Divisa::Dop, tasa).unwrap();
        assert_eq!(f.tarjetas_usd_en_dop, d(352));
        assert_eq!(de_una_vez, d(353));
        // Con la tasa estimada vigente no hay diferencia posible: 60 por centavos es entero.
        let e60 = EntradasDePatrimonio { tarjetas: vec![tarjeta("A", true, 0, 3), tarjeta("B", true, 0, 3)], ..vacias() };
        assert_eq!(componer(&e60).unwrap().tarjetas_usd_en_dop, usd(6).convertir(Divisa::Dop, tasa_estimada()).unwrap());
    }

    #[test]
    fn la_tasa_estimada_es_la_de_la_interfaz() {
        let ts = include_str!("../../../src/js/ui/servicios.ts");
        assert!(
            ts.contains(&format!("TASA_USD_A_DOP = {:.1};", TASA_ESTIMADA_USD_A_DOP)),
            "la tasa de Rust y la de servicios.ts deben ser la misma"
        );
    }
}
