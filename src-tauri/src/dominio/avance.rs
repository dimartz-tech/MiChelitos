//! Avance de efectivo: dinero que una tarjeta de crédito pone en una cuenta.
//!
//! La tarjeta se **debita** por el importe y por el cargo del avance, y la
//! cuenta de ahorro se **acredita** por el importe, sin el cargo. La deuda
//! sube más de lo que llega a la cuenta, y esa diferencia es el coste.
//!
//! ## El cargo tiene tres formas
//!
//! * **Porcentual** — entre el 6 % y el 10 % del importe. Es lo habitual.
//! * **Fijo** — un importe determinado, en ocasiones.
//! * **Exonerado** — bajo ciertas condiciones la entidad no cobra.
//!
//! Son tres variantes de un tipo y no un porcentaje con casos especiales:
//! un cargo fijo no tiene tasa, y una exoneración no es «un cargo del cero
//! por ciento» sino la declaración de que no hubo cargo. Distinguirlos
//! importa al leer el historial.
//!
//! ## La banda del 6 % al 10 %
//!
//! Se exige porque casi todo porcentaje fuera de ella es un tecleo —`0.8` por
//! `8`, o `80` por `8`— y el importe resultante entraría en la deuda de la
//! tarjeta sin que nada lo cuestionara. Si una entidad cambiara su tarifa,
//! las dos constantes son el único sitio que se toca.

use super::dinero::{Dinero, Porcentaje};
use super::errores::ErrorDominio;

/// Cargo porcentual mínimo: 6 %.
pub const CARGO_MINIMO: Porcentaje = Porcentaje::puntos_basicos(600);
/// Cargo porcentual máximo: 10 %.
pub const CARGO_MAXIMO: Porcentaje = Porcentaje::puntos_basicos(1_000);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CargoDeAvance {
    /// Porcentaje del importe, dentro de la banda.
    Porcentual(Porcentaje),
    /// Importe fijo, positivo, en la divisa del avance.
    Fijo(Dinero),
    /// La entidad no cobró.
    Exonerado,
}

impl CargoDeAvance {
    /// Desde un porcentaje tal como se escribe: `8.5` para el 8,5 %.
    pub fn porcentual(porcentaje: f64) -> Result<CargoDeAvance, ErrorDominio> {
        let tasa = Porcentaje::desde_porcentaje(porcentaje)?;
        if tasa < CARGO_MINIMO || tasa > CARGO_MAXIMO {
            return Err(ErrorDominio::CargoDeAvanceFueraDeRango {
                porcentaje,
                minimo: CARGO_MINIMO.fraccion() * 100.0,
                maximo: CARGO_MAXIMO.fraccion() * 100.0,
            });
        }
        Ok(CargoDeAvance::Porcentual(tasa))
    }

    pub fn fijo(importe: Dinero) -> Result<CargoDeAvance, ErrorDominio> {
        if importe.es_cero() || importe.es_negativo() {
            return Err(ErrorDominio::CargoFijoNoPositivo);
        }
        Ok(CargoDeAvance::Fijo(importe))
    }

    /// El código que se guarda: `porcentaje`, `fijo` o `exonerado`.
    pub fn codigo(&self) -> &'static str {
        match self {
            CargoDeAvance::Porcentual(_) => "porcentaje",
            CargoDeAvance::Fijo(_) => "fijo",
            CargoDeAvance::Exonerado => "exonerado",
        }
    }

    /// El porcentaje tal como se escribe, solo en la forma porcentual.
    pub fn tasa(&self) -> Option<f64> {
        match self {
            CargoDeAvance::Porcentual(p) => Some(p.fraccion() * 100.0),
            _ => None,
        }
    }
}

/// Lo que un avance mueve, ya calculado.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Avance {
    pub monto: Dinero,
    pub cargo: Dinero,
}

impl Avance {
    /// Valida el importe y calcula el cargo en la divisa del avance.
    ///
    /// El cargo porcentual se redondea al céntimo con `Dinero::porcentaje`,
    /// el único punto donde el dominio decide un céntimo.
    pub fn calcular(monto: Dinero, cargo: CargoDeAvance) -> Result<Avance, ErrorDominio> {
        if monto.es_cero() || monto.es_negativo() {
            return Err(ErrorDominio::AvanceNoPositivo);
        }
        let importe_del_cargo = match cargo {
            CargoDeAvance::Porcentual(tasa) => monto.porcentaje(tasa)?,
            CargoDeAvance::Fijo(fijo) => {
                if fijo.divisa() != monto.divisa() {
                    return Err(ErrorDominio::DivisasIncompatibles {
                        esperada: monto.divisa(),
                        recibida: fijo.divisa(),
                    });
                }
                fijo
            }
            CargoDeAvance::Exonerado => Dinero::cero(monto.divisa()),
        };
        Ok(Avance { monto, cargo: importe_del_cargo })
    }

    /// Lo que sube la deuda de la tarjeta: importe más cargo.
    pub fn a_la_tarjeta(&self) -> Result<Dinero, ErrorDominio> {
        self.monto.sumar(&self.cargo)
    }
}

// --- Lo que llega de la interfaz, ya como reglas (A-03: sale de `main.rs`) ----------------------------

/// El cargo a partir de lo que pide la interfaz: el tipo (`porcentaje`, `fijo` o `exonerado`) y los valores que
/// acompañan. Cada tipo exige lo suyo y **solo** lo suyo: un valor de más es una contradicción, no algo que se ignore.
pub fn cargo_desde_la_peticion(
    tipo: &str,
    porcentaje: Option<f64>,
    cargo_fijo: Option<Dinero>,
) -> Result<CargoDeAvance, ErrorDominio> {
    match (tipo, porcentaje, cargo_fijo) {
        ("porcentaje", Some(p), None) => CargoDeAvance::porcentual(p),
        ("porcentaje", None, _) => Err(ErrorDominio::PorcentajeDeCargoRequerido),
        ("fijo", None, Some(f)) => CargoDeAvance::fijo(f),
        ("fijo", _, None) => Err(ErrorDominio::ImporteDeCargoFijoRequerido),
        ("exonerado", None, None) => Ok(CargoDeAvance::Exonerado),
        ("porcentaje" | "fijo" | "exonerado", _, _) => Err(ErrorDominio::CargoContradictorio),
        (otro, _, _) => Err(ErrorDominio::TipoDeCargoDesconocido { tipo: otro.to_string() }),
    }
}

/// La fecha del avance, recortada, si tiene la forma `dd/mm/aaaa` de un día que existe.
pub fn fecha_de_avance(texto: &str) -> Result<String, ErrorDominio> {
    let fecha = texto.trim();
    if chrono::NaiveDate::parse_from_str(fecha, "%d/%m/%Y").is_err() {
        return Err(ErrorDominio::FechaNoEntendida { fecha: fecha.to_string() });
    }
    Ok(fecha.to_string())
}

/// La nota, recortada; en blanco es lo mismo que ausente.
pub fn nota_de_avance(nota: Option<String>) -> Option<String> {
    nota.map(|n| n.trim().to_string()).filter(|n| !n.is_empty())
}

#[cfg(test)]
mod tests_de_la_peticion {
    use super::*;
    use crate::dominio::dinero::Divisa;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }

    #[test]
    fn cada_tipo_de_cargo_exige_lo_suyo_y_solo_lo_suyo() {
        assert_eq!(cargo_desde_la_peticion("porcentaje", Some(8.0), None), CargoDeAvance::porcentual(8.0));
        assert_eq!(cargo_desde_la_peticion("fijo", None, Some(dop(25.0))), CargoDeAvance::fijo(dop(25.0)));
        assert_eq!(cargo_desde_la_peticion("exonerado", None, None), Ok(CargoDeAvance::Exonerado));
    }

    #[test]
    fn lo_que_falta_y_lo_que_sobra_dicen_su_causa() {
        assert_eq!(cargo_desde_la_peticion("porcentaje", None, None), Err(ErrorDominio::PorcentajeDeCargoRequerido));
        assert_eq!(cargo_desde_la_peticion("porcentaje", None, Some(dop(5.0))), Err(ErrorDominio::PorcentajeDeCargoRequerido));
        assert_eq!(cargo_desde_la_peticion("fijo", None, None), Err(ErrorDominio::ImporteDeCargoFijoRequerido));
        assert_eq!(cargo_desde_la_peticion("fijo", Some(5.0), None), Err(ErrorDominio::ImporteDeCargoFijoRequerido));
        for (tipo, pct, fijo) in [("porcentaje", Some(5.0), Some(dop(5.0))), ("fijo", Some(5.0), Some(dop(5.0))), ("exonerado", Some(5.0), None), ("exonerado", None, Some(dop(5.0)))] {
            assert_eq!(cargo_desde_la_peticion(tipo, pct, fijo), Err(ErrorDominio::CargoContradictorio), "{tipo}");
        }
        assert_eq!(
            cargo_desde_la_peticion("quincenal", None, None),
            Err(ErrorDominio::TipoDeCargoDesconocido { tipo: "quincenal".into() })
        );
    }

    #[test]
    fn un_porcentaje_fuera_de_la_banda_o_un_fijo_no_positivo_los_rechaza_el_cargo() {
        assert!(matches!(cargo_desde_la_peticion("porcentaje", Some(0.8), None), Err(ErrorDominio::CargoDeAvanceFueraDeRango { .. })));
        assert_eq!(cargo_desde_la_peticion("fijo", None, Some(dop(0.0))), Err(ErrorDominio::CargoFijoNoPositivo));
    }

    #[test]
    fn la_fecha_se_recorta_y_debe_existir_en_dd_mm_aaaa() {
        assert_eq!(fecha_de_avance("  01/10/2026 "), Ok("01/10/2026".into()));
        // «1/10/2026» sí pasa esta comprobación (el analizador acepta el día sin cero); el esquema, que es estricto, la
        // rechaza al guardar. Se deja dicho para que nadie suponga que esta validación es la única.
        assert_eq!(fecha_de_avance("1/10/2026"), Ok("1/10/2026".into()));
        for mala in ["2026-10-01", "31/02/2026", "ayer", ""] {
            assert_eq!(fecha_de_avance(mala), Err(ErrorDominio::FechaNoEntendida { fecha: mala.trim().into() }), "«{mala}»");
        }
    }

    #[test]
    fn una_nota_en_blanco_es_una_nota_ausente() {
        assert_eq!(nota_de_avance(Some("  para el taller ".into())), Some("para el taller".into()));
        assert_eq!(nota_de_avance(Some("   ".into())), None);
        assert_eq!(nota_de_avance(None), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dominio::dinero::Divisa;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }
    fn usd(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Usd).unwrap()
    }

    #[test]
    fn el_cargo_porcentual_se_calcula_sobre_el_importe() {
        let a = Avance::calcular(dop(10_000.0), CargoDeAvance::porcentual(8.0).unwrap()).unwrap();
        assert_eq!(a.cargo, dop(800.0));
        assert_eq!(a.a_la_tarjeta().unwrap(), dop(10_800.0));
    }

    #[test]
    fn un_porcentaje_con_dos_decimales_se_calcula_exacto() {
        // El caso real más reciente del titular: 6.25 %. La escala de los
        // porcentajes lo representa sin residuo, así que el céntimo no
        // depende de cómo se escriba en binario.
        let tasa = CargoDeAvance::porcentual(6.25).unwrap();
        assert_eq!(Avance::calcular(dop(10_000.0), tasa).unwrap().cargo, dop(625.0));
        assert_eq!(Avance::calcular(dop(1_234.56), tasa).unwrap().cargo, dop(77.16));
        assert_eq!(tasa.tasa(), Some(6.25));
    }

    #[test]
    fn los_extremos_de_la_banda_se_admiten() {
        assert!(CargoDeAvance::porcentual(6.0).is_ok());
        assert!(CargoDeAvance::porcentual(10.0).is_ok());
    }

    #[test]
    fn fuera_de_la_banda_se_rechaza_porque_casi_siempre_es_un_tecleo() {
        for p in [0.8, 5.99, 10.01, 80.0, 0.0, -8.0] {
            assert!(
                matches!(
                    CargoDeAvance::porcentual(p),
                    Err(ErrorDominio::CargoDeAvanceFueraDeRango { .. })
                ),
                "aceptó {p}"
            );
        }
    }

    #[test]
    fn un_porcentaje_ilegible_es_error() {
        assert!(CargoDeAvance::porcentual(f64::NAN).is_err());
        assert!(CargoDeAvance::porcentual(f64::INFINITY).is_err());
    }

    #[test]
    fn el_cargo_se_redondea_al_centimo_con_la_regla_del_sistema() {
        // 8.5 % de 1 234.56 = 104.9376 → 104.94.
        let a = Avance::calcular(dop(1_234.56), CargoDeAvance::porcentual(8.5).unwrap()).unwrap();
        assert_eq!(a.cargo, dop(104.94));
        // La mitad se aleja de cero: 7 % de 0.50 = 0.035 → 0.04.
        let a = Avance::calcular(dop(0.50), CargoDeAvance::porcentual(7.0).unwrap()).unwrap();
        assert_eq!(a.cargo, dop(0.04));
    }

    #[test]
    fn un_cargo_fijo_no_depende_del_importe() {
        let fijo = CargoDeAvance::fijo(dop(250.0)).unwrap();
        for monto in [500.0, 10_000.0, 90_000.0] {
            assert_eq!(Avance::calcular(dop(monto), fijo).unwrap().cargo, dop(250.0));
        }
    }

    #[test]
    fn un_cargo_fijo_de_cero_o_negativo_se_rechaza() {
        // Cero es una exoneración: declararlo como fijo esconde la distinción.
        assert_eq!(CargoDeAvance::fijo(dop(0.0)), Err(ErrorDominio::CargoFijoNoPositivo));
        assert_eq!(CargoDeAvance::fijo(dop(-5.0)), Err(ErrorDominio::CargoFijoNoPositivo));
    }

    #[test]
    fn un_cargo_fijo_en_otra_divisa_no_se_suma_sin_convertir() {
        let fijo = CargoDeAvance::fijo(usd(5.0)).unwrap();
        assert!(matches!(
            Avance::calcular(dop(10_000.0), fijo),
            Err(ErrorDominio::DivisasIncompatibles { .. })
        ));
    }

    #[test]
    fn exonerado_no_paga_cargo_y_la_tarjeta_sube_solo_el_importe() {
        let a = Avance::calcular(dop(10_000.0), CargoDeAvance::Exonerado).unwrap();
        assert!(a.cargo.es_cero());
        assert_eq!(a.a_la_tarjeta().unwrap(), dop(10_000.0));
    }

    #[test]
    fn un_avance_sin_importe_positivo_se_rechaza() {
        for monto in [0.0, -100.0] {
            assert_eq!(
                Avance::calcular(dop(monto), CargoDeAvance::Exonerado),
                Err(ErrorDominio::AvanceNoPositivo)
            );
        }
    }

    #[test]
    fn el_codigo_y_la_tasa_describen_cada_forma() {
        let p = CargoDeAvance::porcentual(8.5).unwrap();
        assert_eq!((p.codigo(), p.tasa()), ("porcentaje", Some(8.5)));
        let f = CargoDeAvance::fijo(dop(1.0)).unwrap();
        assert_eq!((f.codigo(), f.tasa()), ("fijo", None));
        assert_eq!((CargoDeAvance::Exonerado.codigo(), CargoDeAvance::Exonerado.tasa()), ("exonerado", None));
    }
}
