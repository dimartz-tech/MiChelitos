//! La foto mensual del patrimonio: lo que se tenía y se debía un mes dado.
//!
//! Este módulo es **solo la foto y su aritmética**; de dónde salen las partidas lo decide el caso de uso. La función
//! `dominio::patrimonio::componer` (otro PR) se enchufará después como productora de las partidas: aquí `Foto::construir`
//! recibe partidas ya calculadas y no depende de ella.
//!
//! ## Reglas
//!
//! * Todo en **centavos enteros** (`Dinero`). Cada partida lleva su importe en la divisa de origen y el convertido a pesos
//!   con la tasa **de la foto**; la foto no usa ninguna otra tasa.
//! * `patrimonio = activos − pasivos`, por construcción (la base lo exige además con un `CHECK`).
//! * Las tarjetas y los préstamos son **pasivos**: su partida lleva la deuda (negativa si es saldo a favor, que resta del
//!   pasivo en vez de inventar un activo). Todo lo demás es activo.
//! * `líquido` = cuentas + efectivo, en pesos: lo que se puede gastar sin vender nada.

use super::dinero::{Dinero, Divisa, TasaCambio};
use super::errores::ErrorDominio;
use chrono::{Datelike, NaiveDate};

/// Tasa USD→DOP que se sugiere cuando el titular no indica otra. Es un valor a ajustar por él, no un dato de mercado.
pub const TASA_USD_DOP_SUGERIDA: f64 = 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rubro {
    Cuenta,
    Efectivo,
    Certificado,
    Bolsa,
    Inmobiliario,
    Vehiculo,
    Maquinaria,
    Tarjeta,
    Prestamo,
}

impl Rubro {
    pub fn codigo(&self) -> &'static str {
        match self {
            Rubro::Cuenta => "cuenta",
            Rubro::Efectivo => "efectivo",
            Rubro::Certificado => "certificado",
            Rubro::Bolsa => "bolsa",
            Rubro::Inmobiliario => "inmobiliario",
            Rubro::Vehiculo => "vehiculo",
            Rubro::Maquinaria => "maquinaria",
            Rubro::Tarjeta => "tarjeta",
            Rubro::Prestamo => "prestamo",
        }
    }

    pub fn es_pasivo(&self) -> bool {
        matches!(self, Rubro::Tarjeta | Rubro::Prestamo)
    }

    pub fn es_liquido(&self) -> bool {
        matches!(self, Rubro::Cuenta | Rubro::Efectivo)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origen {
    /// La toma el arranque de la aplicación, sola.
    Real,
    /// El titular la pidió.
    Manual,
    /// Se tomó al guardar el capital.
    Capital,
    /// Rehecha a partir de la historia (todavía no existe quien la produzca; el esquema ya la admite).
    #[allow(dead_code)]
    Reconstruida,
}

impl Origen {
    pub fn codigo(&self) -> &'static str {
        match self {
            Origen::Real => "real",
            Origen::Manual => "manual",
            Origen::Capital => "capital",
            Origen::Reconstruida => "reconstruida",
        }
    }

    /// Los orígenes que un comando acepta de la interfaz: ni `real` (lo pone el arranque) ni `reconstruida`.
    pub fn desde_peticion(codigo: &str) -> Option<Origen> {
        match codigo.trim() {
            "manual" => Some(Origen::Manual),
            "capital" => Some(Origen::Capital),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Calidad {
    Completa,
    /// Algo no se pudo leer (el capital, por ejemplo) y se contó cero: la foto lo dice.
    ConAviso,
}

impl Calidad {
    pub fn codigo(&self) -> &'static str {
        match self {
            Calidad::Completa => "completa",
            Calidad::ConAviso => "con_aviso",
        }
    }
}

/// Una línea de la foto, ya convertida.
#[derive(Debug, Clone, PartialEq)]
pub struct Partida {
    pub rubro: Rubro,
    /// Cómo la llama el titular (nombre de la cuenta, banco del certificado…).
    pub referencia: String,
    pub importe: Dinero,
    /// `importe` en pesos con la tasa de la foto.
    pub importe_dop: Dinero,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Foto {
    pub anio: i32,
    pub mes: u32,
    /// `dd/mm/aaaa`, como el resto de los datos de la aplicación.
    pub fecha: String,
    pub origen: Origen,
    pub tasa_usd_dop: TasaCambio,
    pub calidad: Calidad,
    pub partidas: Vec<Partida>,
    pub activos: Dinero,
    pub pasivos: Dinero,
    pub patrimonio: Dinero,
    pub liquido: Dinero,
}

impl Foto {
    /// Arma la foto de `fecha` con partidas ya calculadas `(rubro, referencia, importe en su divisa)`.
    ///
    /// Las partidas con importe cero se conservan: «tiene la cuenta y está en cero» es información. Quien no quiera una
    /// (cuenta archivada sin saldo) no la pasa.
    pub fn construir(
        fecha: NaiveDate,
        origen: Origen,
        tasa_usd_dop: TasaCambio,
        calidad: Calidad,
        crudas: Vec<(Rubro, String, Dinero)>,
    ) -> Result<Foto, ErrorDominio> {
        let cero = Dinero::cero(Divisa::Dop);
        let (mut activos, mut pasivos, mut liquido) = (cero, cero, cero);
        let mut partidas = Vec::with_capacity(crudas.len());
        for (rubro, referencia, importe) in crudas {
            let importe_dop = importe.convertir(Divisa::Dop, tasa_usd_dop)?;
            if rubro.es_pasivo() {
                pasivos = pasivos.sumar(&importe_dop)?;
            } else {
                activos = activos.sumar(&importe_dop)?;
            }
            if rubro.es_liquido() {
                liquido = liquido.sumar(&importe_dop)?;
            }
            let referencia = referencia.trim();
            let referencia = if referencia.is_empty() { "(sin nombre)" } else { referencia };
            partidas.push(Partida { rubro, referencia: referencia.to_string(), importe, importe_dop });
        }
        let patrimonio = activos.restar(&pasivos)?;
        Ok(Foto {
            anio: fecha.year(),
            mes: fecha.month(),
            fecha: fecha.format("%d/%m/%Y").to_string(),
            origen,
            tasa_usd_dop,
            calidad,
            partidas,
            activos,
            pasivos,
            patrimonio,
            liquido,
        })
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn d(c: i64) -> Dinero {
        Dinero::de_centavos(c, Divisa::Dop)
    }
    fn u(c: i64) -> Dinero {
        Dinero::de_centavos(c, Divisa::Usd)
    }
    fn fecha() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 3, 17).unwrap()
    }
    fn tasa() -> TasaCambio {
        TasaCambio::nueva(58.7312).unwrap()
    }

    #[test]
    fn el_patrimonio_es_activos_menos_pasivos_y_el_liquido_solo_cuenta_cuentas_y_efectivo() {
        let f = Foto::construir(
            fecha(),
            Origen::Manual,
            tasa(),
            Calidad::Completa,
            vec![
                (Rubro::Cuenta, "Ahorros".into(), d(1_234_567_13)),
                (Rubro::Efectivo, "Caja".into(), d(4_321_09)),
                (Rubro::Certificado, "Banco Uno".into(), d(5_000_000_77)),
                (Rubro::Tarjeta, "Visa".into(), d(871_234_51)),
                (Rubro::Prestamo, "Carro".into(), d(2_345_678_91)),
            ],
        )
        .unwrap();
        assert_eq!(f.activos.centavos(), 1_234_567_13 + 4_321_09 + 5_000_000_77);
        assert_eq!(f.pasivos.centavos(), 871_234_51 + 2_345_678_91);
        assert_eq!(f.patrimonio.centavos(), f.activos.centavos() - f.pasivos.centavos());
        assert_eq!(f.liquido.centavos(), 1_234_567_13 + 4_321_09);
        assert_eq!((f.anio, f.mes, f.fecha.as_str()), (2026, 3, "17/03/2026"));
    }

    #[test]
    fn un_importe_en_dolares_se_convierte_con_la_tasa_de_la_foto() {
        let f = Foto::construir(fecha(), Origen::Real, tasa(), Calidad::Completa, vec![(Rubro::Cuenta, "Dólares".into(), u(1_234_57))])
            .unwrap();
        let esperado = u(1_234_57).convertir(Divisa::Dop, tasa()).unwrap();
        assert_eq!(f.partidas[0].importe_dop, esperado);
        assert_eq!(f.partidas[0].importe, u(1_234_57), "el importe de origen no se toca");
        assert_eq!(f.activos, esperado);
    }

    #[test]
    fn la_tarjeta_con_saldo_a_favor_resta_del_pasivo() {
        let f = Foto::construir(
            fecha(),
            Origen::Real,
            tasa(),
            Calidad::Completa,
            vec![(Rubro::Tarjeta, "A".into(), d(10_000_03)), (Rubro::Tarjeta, "B".into(), d(-2_500_01))],
        )
        .unwrap();
        assert_eq!(f.pasivos.centavos(), 7_500_02);
        assert_eq!(f.patrimonio.centavos(), -7_500_02);
    }

    #[test]
    fn una_foto_vacia_es_cero_y_una_referencia_en_blanco_recibe_nombre() {
        let f = Foto::construir(fecha(), Origen::Real, tasa(), Calidad::ConAviso, vec![]).unwrap();
        assert!(f.activos.es_cero() && f.pasivos.es_cero() && f.patrimonio.es_cero() && f.liquido.es_cero());
        let g = Foto::construir(fecha(), Origen::Real, tasa(), Calidad::Completa, vec![(Rubro::Bolsa, "   ".into(), d(1))]).unwrap();
        assert_eq!(g.partidas[0].referencia, "(sin nombre)");
    }

    #[test]
    fn solo_manual_y_capital_se_aceptan_de_la_interfaz() {
        assert_eq!(Origen::desde_peticion("manual"), Some(Origen::Manual));
        assert_eq!(Origen::desde_peticion(" capital "), Some(Origen::Capital));
        assert_eq!(Origen::desde_peticion("real"), None);
        assert_eq!(Origen::desde_peticion("reconstruida"), None);
    }
}
