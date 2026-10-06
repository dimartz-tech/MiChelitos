//! La regla del motivo de una corrección: sin explicación no se corrige nada.
//!
//! **Medida de fricción deliberada.** Una corrección que cuesta una frase se piensa dos veces. El umbral no es una cifra
//! mágica sino una barrera contra el motivo de trámite: «error» u «ok» pasan cualquier comprobación de «no vacío» y no
//! explican nada. (Antes vivía en `correcciones.rs`, mezclada con el SQL; A-03.)

use super::errores::ErrorDominio;

/// Longitud mínima de un motivo para que cuente como explicación.
pub const MINIMO_DEL_MOTIVO: usize = 15;

/// El motivo recortado, si explica; si no, el error que dice cuánto hace falta.
pub fn motivo_de_correccion(entrada: &str) -> Result<String, ErrorDominio> {
    let motivo = entrada.trim();
    if motivo.chars().count() < MINIMO_DEL_MOTIVO {
        return Err(ErrorDominio::MotivoInsuficiente { motivo: motivo.to_string() });
    }
    Ok(motivo.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_motivo_de_tramite_se_rechaza_y_el_mensaje_lo_cita_recortado() {
        let e = motivo_de_correccion("  error ").unwrap_err();
        assert_eq!(e, ErrorDominio::MotivoInsuficiente { motivo: "error".into() });
        assert_eq!(
            e.to_string(),
            "Explica la corrección en al menos 15 caracteres. Dentro de seis meses, «error» no dirá qué pasó."
        );
    }

    #[test]
    fn el_umbral_es_de_quince_caracteres_no_de_bytes() {
        assert!(motivo_de_correccion("12345678901234").is_err());
        assert_eq!(motivo_de_correccion(" 123456789012345 ").unwrap(), "123456789012345");
        // 15 caracteres con acentos: más de 15 bytes, pero se cuentan caracteres.
        assert!(motivo_de_correccion("áéíóúñáéíóúñáéí").is_ok());
        assert!(motivo_de_correccion("áéíóúñáéíóúñáé").is_err());
    }
}
