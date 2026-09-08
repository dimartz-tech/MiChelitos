use chrono::NaiveDate;

/// Obtener "hoy" es entrada/salida, no una regla. Aislarlo tras un puerto hace
/// deterministas las pruebas de ciclos de corte y suscripciones.
pub trait Reloj {
    fn hoy(&self) -> NaiveDate;
}

#[cfg(test)]
pub struct RelojFijo(pub NaiveDate);

#[cfg(test)]
impl RelojFijo {
    pub fn en(anio: i32, mes: u32, dia: u32) -> RelojFijo {
        RelojFijo(NaiveDate::from_ymd_opt(anio, mes, dia).expect("fecha de prueba válida"))
    }
}

#[cfg(test)]
impl Reloj for RelojFijo {
    fn hoy(&self) -> NaiveDate {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn el_reloj_fijo_devuelve_siempre_la_misma_fecha() {
        // 2028 sí es bisiesto; 2026 no lo es.
        let reloj = RelojFijo::en(2028, 2, 29);
        assert_eq!(reloj.hoy(), reloj.hoy());
        assert_eq!(reloj.hoy().day(), 29);
        assert_eq!(reloj.hoy().month(), 2);
    }

    #[test]
    fn una_fecha_inexistente_no_se_construye_en_silencio() {
        assert!(NaiveDate::from_ymd_opt(2026, 2, 29).is_none());
    }

    #[test]
    fn permite_situar_las_pruebas_en_fechas_de_borde() {
        // Un día de corte 31 en un mes de 30 es exactamente el caso que hoy
        // no puede probarse porque el código lee la fecha del sistema.
        let fin_de_mes_corto = RelojFijo::en(2026, 4, 30);
        assert_eq!(fin_de_mes_corto.hoy().day(), 30);
    }
}
