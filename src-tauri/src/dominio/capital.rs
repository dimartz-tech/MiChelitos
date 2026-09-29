//! Alerta de vencimiento sobre certificados de depósito e inversiones de
//! renta fija.
//!
//! Es la única regla del capital declarado, y hasta ahora vivía duplicada
//! —una copia para certificados, otra para bolsa— dentro del comando que lee
//! la colección, mezclada con el `serde_json::Value` que la persiste.
//!
//! ## El defecto que la extracción destapó
//!
//! El comando de lectura **mutaba el mismo `Value` que después se guarda**:
//! calculaba `dias_restantes` y `alerta_vencimiento` sobre cada entrada y los
//! escribía en el propio objeto, para devolverlo a la vista. Pero cada acción
//! de escritura del capital —añadir o quitar un certificado, una inversión,
//! un bien— hace exactamente `leer → mutar una parte → guardar el todo`, así
//! que esos campos calculados **se guardaban en el archivo** como si fueran
//! datos declarados por el titular.
//!
//! No es hipotético: la base real ya tenía `alerta_vencimiento` y
//! `dias_restantes` grabados en disco para una inversión de bolsa, calculados
//! el día en que se guardó por última vez y nunca más. Un valor que se
//! recalcula en cada lectura no puede vivir también en la escritura, o dejan
//! de ser el mismo dato.
//!
//! La corrección tiene dos partes: esta función calcula la alerta **sin
//! tocar nada**, y el comando de guardado retira estos campos antes de
//! persistir, sea cual sea su origen.

use super::dinero::{Dinero, Divisa};
use chrono::NaiveDate;
use std::fmt;

/// Cuántos días de antelación cuentan como «vence pronto».
pub const DIAS_DE_AVISO: i64 = 10;

/// Nombres de los campos calculados. Ninguno de los dos debería sobrevivir a
/// una escritura: son la vista, no el dato.
pub const CAMPOS_CALCULADOS: &[&str] = &["alerta_vencimiento", "dias_restantes", "alerta_msg"];

/// Tasa anual máxima que se acepta, en porcentaje.
///
/// No es un límite de mercado: es una red contra el tecleo. `850` por `8.5`
/// entraría como una tasa del 850 % y nada lo cuestionaría. Cualquier tasa
/// real cabe muy por debajo.
pub const TASA_MAXIMA: f64 = 100.0;

/// Qué falló al declarar una entrada del capital, y en cuál.
///
/// Lleva **qué entrada** —«Certificado 2 (Banco X)»— además de qué campo, porque
/// el capital se guarda entero y un mensaje que solo dice «monto inválido» deja
/// al titular buscando entre todas.
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorCapital {
    pub entrada: String,
    pub motivo: String,
}

impl fmt::Display for ErrorCapital {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.entrada, self.motivo)
    }
}

impl std::error::Error for ErrorCapital {}

/// Una regla del capital, antes de saber a qué entrada pertenece.
pub type Motivo = String;

/// Un importe declarado: positivo y en pesos.
///
/// El capital no lleva divisa —la interfaz rotula «DOP» y no hay otra—, de
/// modo que todo importe se lee en pesos. **Cero no vale**: un certificado o
/// un bien sin valor no es un activo, es un campo sin rellenar.
pub fn validar_monto(monto: Dinero, campo: &str) -> Result<Dinero, Motivo> {
    if monto.es_negativo() {
        return Err(format!("{campo} no puede ser negativo."));
    }
    if monto.es_cero() {
        return Err(format!("{campo} debe ser mayor que cero."));
    }
    Ok(monto)
}

/// Los importes del capital son siempre en pesos.
pub fn divisa_del_capital() -> Divisa {
    Divisa::Dop
}

/// Una tasa anual en porcentaje: de 0 a `TASA_MAXIMA`.
pub fn validar_tasa(tasa: f64) -> Result<f64, Motivo> {
    if !tasa.is_finite() {
        return Err("la tasa no es un número válido.".to_string());
    }
    if tasa < 0.0 {
        return Err("la tasa no puede ser negativa.".to_string());
    }
    if tasa > TASA_MAXIMA {
        return Err(format!(
            "una tasa del {tasa}% queda fuera de lo razonable (el máximo es {TASA_MAXIMA}%). ¿Quisiste escribir {}?",
            tasa / 100.0
        ));
    }
    Ok(tasa)
}

/// Una fecha `dd/mm/aaaa` que **existe**.
///
/// Se exige la forma exacta y después que sea un día real: `31/02/2026` tiene
/// la forma y no es una fecha, y `1/2/2026` es una fecha con otra forma. La
/// aplicación guarda las fechas en un solo formato, y una entrada en otro
/// deja la alerta de vencimiento sin calcular en silencio.
pub fn validar_fecha(texto: &str) -> Result<NaiveDate, Motivo> {
    let texto = texto.trim();
    let bytes = texto.as_bytes();
    let forma = bytes.len() == 10
        && bytes[2] == b'/'
        && bytes[5] == b'/'
        && bytes.iter().enumerate().all(|(i, b)| i == 2 || i == 5 || b.is_ascii_digit());
    if !forma {
        return Err(format!("«{texto}» no tiene la forma dd/mm/aaaa."));
    }
    NaiveDate::parse_from_str(texto, "%d/%m/%Y")
        .map_err(|_| format!("«{texto}» no es una fecha que exista."))
}

/// Un texto obligatorio: sin espacios sobrantes y no vacío.
pub fn validar_texto(texto: &str, campo: &str) -> Result<String, Motivo> {
    let limpio = texto.trim();
    if limpio.is_empty() {
        return Err(format!("{campo} es obligatorio."));
    }
    Ok(limpio.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlertaVencimiento {
    pub dias_restantes: i64,
    pub vencido: bool,
    pub vence_pronto: bool,
}

impl AlertaVencimiento {
    /// Si corresponde mostrar alerta: vencido o dentro de la ventana de aviso.
    ///
    /// Reproduce la condición original tal cual: `vencido` y `vence_pronto`
    /// son mutuamente excluyentes por construcción de `calcular`, así que
    /// esto es solo la unión de los dos, no una tercera regla.
    pub fn activa(&self) -> bool {
        self.vencido || self.vence_pronto
    }

    /// El texto que acompañaba a la alerta.
    pub fn mensaje(&self) -> Option<String> {
        if self.vencido {
            Some("¡Vencido!".to_string())
        } else if self.vence_pronto {
            Some(format!("¡Vence en {} días!", self.dias_restantes))
        } else {
            None
        }
    }
}

/// Calcula la alerta para una fecha de vencimiento conocida.
///
/// Reproduce la conducta actual sin alterarla: `dias_restantes` puede ser
/// negativo, y lo negativo no se recorta, porque es lo que distingue un
/// vencimiento pasado de uno próximo.
pub fn calcular(vencimiento: NaiveDate, hoy: NaiveDate) -> AlertaVencimiento {
    let dias_restantes = vencimiento.signed_duration_since(hoy).num_days();
    AlertaVencimiento {
        dias_restantes,
        vencido: dias_restantes < 0,
        vence_pronto: (0..=DIAS_DE_AVISO).contains(&dias_restantes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn en(anio: i32, mes: u32, dia: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(anio, mes, dia).expect("fecha válida")
    }

    #[test]
    fn lejos_del_vencimiento_no_hay_alerta() {
        let a = calcular(en(2026, 12, 31), en(2026, 1, 1));
        assert_eq!(a.dias_restantes, 364);
        assert!(!a.activa());
        assert_eq!(a.mensaje(), None);
    }

    #[test]
    fn dentro_de_diez_dias_avisa_con_la_cuenta_regresiva() {
        let a = calcular(en(2026, 3, 20), en(2026, 3, 10));
        assert_eq!(a.dias_restantes, 10);
        assert!(a.vence_pronto && !a.vencido);
        assert!(a.activa());
        assert_eq!(a.mensaje(), Some("¡Vence en 10 días!".to_string()));
    }

    #[test]
    fn once_dias_antes_todavia_no_avisa() {
        let a = calcular(en(2026, 3, 21), en(2026, 3, 10));
        assert_eq!(a.dias_restantes, 11);
        assert!(!a.activa());
    }

    #[test]
    fn el_mismo_dia_del_vencimiento_avisa() {
        let a = calcular(en(2026, 3, 10), en(2026, 3, 10));
        assert_eq!(a.dias_restantes, 0);
        assert!(a.vence_pronto && !a.vencido);
    }

    #[test]
    fn un_vencimiento_pasado_se_marca_vencido_y_no_recorta_los_dias() {
        let a = calcular(en(2026, 3, 1), en(2026, 3, 10));
        assert_eq!(a.dias_restantes, -9, "negativo, sin recortar: es lo que lo distingue");
        assert!(a.vencido && !a.vence_pronto);
        assert!(a.activa());
        assert_eq!(a.mensaje(), Some("¡Vencido!".to_string()));
    }

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }

    #[test]
    fn un_monto_positivo_se_acepta_y_cero_o_negativo_no() {
        assert_eq!(validar_monto(dop(1_500.0), "el monto"), Ok(dop(1_500.0)));
        assert!(validar_monto(dop(0.01), "el monto").is_ok(), "un centavo es un importe");
        assert!(validar_monto(dop(0.0), "el monto").unwrap_err().contains("mayor que cero"));
        assert!(validar_monto(dop(-5.0), "el monto").unwrap_err().contains("negativo"));
    }

    #[test]
    fn la_tasa_va_de_cero_al_maximo() {
        for ok in [0.0, 6.25, 8.5, 100.0] {
            assert_eq!(validar_tasa(ok), Ok(ok));
        }
        assert!(validar_tasa(-1.0).unwrap_err().contains("negativa"));
        assert!(validar_tasa(f64::NAN).is_err());
        assert!(validar_tasa(f64::INFINITY).is_err());
    }

    #[test]
    fn una_tasa_absurda_sugiere_lo_que_probablemente_se_quiso_escribir() {
        let e = validar_tasa(850.0).unwrap_err();
        assert!(e.contains("8.5"), "debe sugerir 8.5: {e}");
    }

    #[test]
    fn la_fecha_exige_la_forma_y_que_exista() {
        assert_eq!(validar_fecha("29/02/2028"), Ok(en(2028, 2, 29)));
        assert_eq!(validar_fecha("  05/07/2027 "), Ok(en(2027, 7, 5)), "sin espacios sobrantes");
        for mala in ["1/2/2026", "2026-02-01", "31/02/2026", "29/02/2027", "ayer", "", "01/13/2026", "01-02-2026"] {
            assert!(validar_fecha(mala).is_err(), "aceptó «{mala}»");
        }
    }

    #[test]
    fn un_texto_obligatorio_no_puede_estar_vacio() {
        assert_eq!(validar_texto("  Banco Ejemplo ", "el banco"), Ok("Banco Ejemplo".to_string()));
        assert!(validar_texto("   ", "el banco").unwrap_err().contains("obligatorio"));
    }

    #[test]
    fn vencido_y_vence_pronto_son_excluyentes() {
        for (v, h) in [(en(2026, 1, 1), en(2026, 6, 1)), (en(2026, 6, 5), en(2026, 6, 1)),
                       (en(2026, 6, 1), en(2026, 6, 1)), (en(2026, 7, 1), en(2026, 6, 1))] {
            let a = calcular(v, h);
            assert!(!(a.vencido && a.vence_pronto), "vencimiento {v} hoy {h}");
        }
    }
}
