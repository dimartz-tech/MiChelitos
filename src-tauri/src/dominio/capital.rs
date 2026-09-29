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

use chrono::NaiveDate;

/// Cuántos días de antelación cuentan como «vence pronto».
pub const DIAS_DE_AVISO: i64 = 10;

/// Nombres de los campos calculados. Ninguno de los dos debería sobrevivir a
/// una escritura: son la vista, no el dato.
pub const CAMPOS_CALCULADOS: &[&str] = &["alerta_vencimiento", "dias_restantes", "alerta_msg"];

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

    #[test]
    fn vencido_y_vence_pronto_son_excluyentes() {
        for (v, h) in [(en(2026, 1, 1), en(2026, 6, 1)), (en(2026, 6, 5), en(2026, 6, 1)),
                       (en(2026, 6, 1), en(2026, 6, 1)), (en(2026, 7, 1), en(2026, 6, 1))] {
            let a = calcular(v, h);
            assert!(!(a.vencido && a.vence_pronto), "vencimiento {v} hoy {h}");
        }
    }
}
