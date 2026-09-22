//! ¿Corresponde cobrar hoy una suscripción?
//!
//! Es la única regla del módulo, y hasta ahora vivía suelta dentro de
//! `procesar_suscripciones`, mezclada con el SQL que carga la tarjeta y
//! registra el gasto. Separarla permite ejercitarla sin base de datos y, sobre
//! todo, **mirarla**: una vez escrita como regla, sus seis defectos se leen
//! solos.
//!
//! ## Lo que este módulo reproduce, y no corrige
//!
//! Se extrae **conservando la conducta actual**, incluidas sus divergencias,
//! que están fijadas por las pruebas `s10`–`s14` y `s17` de caracterización.
//! Corregirlas cambia importes que el proveedor ya cobró de verdad, y eso es
//! una decisión del titular, no del código:
//!
//! * Una mensual del **día 31** se cobra 7 de 12 meses; la del 30, once.
//! * Varios períodos vencidos generan **un solo cargo**.
//! * Una marca de cobro **ilegible** obliga a cobrar, en las mensuales.
//!
//! ## Lo que sí cambió: la anual
//!
//! Una anual se recobraba al cambiar el año aunque no hubiera pasado uno
//! —cobrada en julio, volvía a cobrar el 5 de enero—. La causa era que
//! **intentaba deducir el vencimiento y le faltaba el mes**.
//!
//! Ahora la fecha de renovación se anota, y la decisión la lee en vez de
//! deducirla. Eso cierra de paso los otros dos agujeros en las anuales: una
//! fecha es una fecha, así que ni el día 31 ni una marca ilegible la
//! desvían.
//!
//! Cada una lleva su comentario donde se decide.

use chrono::{Datelike, NaiveDate};

/// Cada cuánto toca el cargo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frecuencia {
    Mensual,
    Anual,
}

impl Frecuencia {
    /// Interpreta el texto almacenado.
    ///
    /// La columna tiene un `CHECK` que solo admite `mensual` y `anual`, de
    /// modo que un valor distinto no debería existir. Devuelve `None` en vez
    /// de elegir uno: quien llama decide, y hoy decide no cobrar, que es lo
    /// que hacía la cadena de `if` original al no coincidir con ninguna.
    pub fn desde_codigo(codigo: &str) -> Option<Frecuencia> {
        match codigo {
            "mensual" => Some(Frecuencia::Mensual),
            "anual" => Some(Frecuencia::Anual),
            _ => None,
        }
    }
}

/// La marca que hace idempotente el cobro: cuándo se cobró por última vez.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarcaDeCobro {
    /// Nunca se ha cobrado.
    Ninguna,
    /// Se cobró en ese mes de ese año.
    ///
    /// **El día no se guarda a propósito**: la regla actual no lo mira, y
    /// conservarlo aquí sugeriría una precisión que la decisión no tiene.
    En { anio: i32, mes: u32 },
    /// Hay una marca, y no se entiende.
    ///
    /// No es lo mismo que no haberla: la regla actual las trata de forma
    /// opuesta, y por eso son dos casos y no uno.
    Ilegible,
}

impl MarcaDeCobro {
    /// Lee la marca tal como está almacenada, en `dd/mm/aaaa`.
    pub fn desde_texto(texto: Option<&str>) -> MarcaDeCobro {
        let Some(texto) = texto else {
            return MarcaDeCobro::Ninguna;
        };

        let partes: Vec<&str> = texto.split('/').collect();
        if partes.len() != 3 {
            return MarcaDeCobro::Ilegible;
        }

        // El día se descarta, igual que hacía el código original con su
        // `_p_dia`. Que las tres partes se parseen y solo se usen dos es la
        // forma en que el defecto de los períodos vencidos se hace visible.
        match (
            partes[0].parse::<i32>(),
            partes[1].parse::<u32>(),
            partes[2].parse::<i32>(),
        ) {
            (Ok(_), Ok(mes), Ok(anio)) => MarcaDeCobro::En { anio, mes },
            _ => MarcaDeCobro::Ilegible,
        }
    }
}

/// Lo que hace falta saber para decidir un cobro.
#[derive(Debug, Clone)]
pub struct Suscripcion {
    pub frecuencia: Frecuencia,
    /// Día del mes en que factura el proveedor, de 1 a 31.
    pub dia_de_facturacion: u32,
    pub ultimo_cobro: MarcaDeCobro,
    /// **Solo las anuales.** La fecha exacta en que el proveedor renueva.
    ///
    /// Es un dato anotado, no una deducción. La regla anterior intentaba
    /// deducir el vencimiento de una anual a partir del año del último cobro,
    /// y no podía: le faltaba el mes. De ahí salía el cargo seis meses antes
    /// de tiempo.
    ///
    /// `None` en una anual significa que **no se sabe cuándo renueva**, y
    /// entonces no se cobra. Entre un cargo de más y uno de menos, el de
    /// menos es el que se puede corregir mirando el estado de cuenta.
    pub renovacion: Option<NaiveDate>,
}

impl Suscripcion {
    /// Si hoy toca cargar.
    ///
    /// Las anuales leen su fecha de renovación. Las mensuales conservan la
    /// conducta anterior, divergencias incluidas, y cada rama dice cuál.
    pub fn corresponde_cobrar(&self, hoy: NaiveDate) -> bool {
        // **Divergencia: el día 31 en un mes de 30.** `hoy.day()` nunca llega
        // a 31 en abril, junio, septiembre o noviembre —ni a 30 en febrero—,
        // de modo que esos meses se saltan enteros. Lo natural sería tomar el
        // último día del mes, pero eso cambia el importe anual.
        let dia_alcanzado = hoy.day() >= self.dia_de_facturacion;

        match &self.ultimo_cobro {
            // **Divergencia: la marca ilegible obliga a cobrar.** Invierte la
            // propiedad que sostiene todo el mecanismo. Lo prudente sería lo
            // contrario —no cobrar y avisar—, porque un cargo de más es más
            // difícil de deshacer que uno de menos.
            MarcaDeCobro::Ilegible => match self.frecuencia {
                // La anual tiene una fecha propia: una marca ilegible ya no
                // la arrastra a cobrar. El agujero queda abierto solo donde
                // sigue siendo el único dato, que es en las mensuales.
                Frecuencia::Anual => self.vence_la_renovacion(hoy),
                Frecuencia::Mensual => true,
            },

            MarcaDeCobro::Ninguna => match self.frecuencia {
                Frecuencia::Anual => self.vence_la_renovacion(hoy),
                Frecuencia::Mensual => dia_alcanzado,
            },

            // **Divergencia: varios períodos vencidos generan un cargo.** La
            // marca es una fecha, no un contador, y al cobrar se pone «hoy»:
            // los meses intermedios desaparecen.
            MarcaDeCobro::En { anio, mes } => match self.frecuencia {
                Frecuencia::Mensual => {
                    let mes_nuevo =
                        hoy.year() > *anio || (hoy.year() == *anio && hoy.month() > *mes);
                    mes_nuevo && dia_alcanzado
                }
                // La anual ya no deduce: **lee la fecha anotada**. Ver
                // `renovacion` y `s12b`.
                Frecuencia::Anual => self.vence_la_renovacion(hoy),
            },
        }
    }

    /// Si la renovación anotada ya venció. Sin fecha, no vence nada.
    fn vence_la_renovacion(&self, hoy: NaiveDate) -> bool {
        match self.renovacion {
            Some(fecha) => hoy >= fecha,
            None => false,
        }
    }

    /// Cuándo toca el próximo cargo, si se puede decir.
    ///
    /// **Solo las anuales lo saben.** Una mensual tendría que predecirlo, y
    /// esa predicción arrastraría hoy el defecto del día 31; anunciar una
    /// fecha que el sistema luego no respeta es peor que no anunciar nada.
    pub fn proximo_cobro(&self) -> Option<NaiveDate> {
        match self.frecuencia {
            Frecuencia::Anual => self.renovacion,
            Frecuencia::Mensual => None,
        }
    }

    /// Si toca avisar: el cargo cae dentro de los próximos siete días.
    ///
    /// Incluye el mismo día del cargo y excluye lo ya vencido, que no es un
    /// aviso sino un cobro pendiente.
    pub fn avisa(&self, hoy: NaiveDate) -> bool {
        match self.proximo_cobro() {
            Some(fecha) => {
                let faltan = (fecha - hoy).num_days();
                (0..=DIAS_DE_AVISO).contains(&faltan)
            }
            None => false,
        }
    }

    /// La renovación del año siguiente, para después de cobrar.
    ///
    /// El 29 de febrero se traslada al 28: `NaiveDate` no admite un 29 en año
    /// común, y adelantar al 1 de marzo movería el cargo de mes.
    pub fn renovacion_siguiente(&self) -> Option<NaiveDate> {
        let fecha = self.renovacion?;
        let anio = fecha.year() + 1;
        NaiveDate::from_ymd_opt(anio, fecha.month(), fecha.day())
            .or_else(|| NaiveDate::from_ymd_opt(anio, fecha.month(), 28))
    }
}

/// Cuántos días antes se avisa de un cargo.
pub const DIAS_DE_AVISO: i64 = 7;

#[cfg(test)]
mod tests {
    use super::*;

    fn en(anio: i32, mes: u32, dia: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(anio, mes, dia).expect("fecha válida")
    }

    fn mensual(dia: u32, ultimo: MarcaDeCobro) -> Suscripcion {
        Suscripcion {
            frecuencia: Frecuencia::Mensual,
            dia_de_facturacion: dia,
            ultimo_cobro: ultimo,
            renovacion: None,
        }
    }

    fn anual(renovacion: Option<NaiveDate>, ultimo: MarcaDeCobro) -> Suscripcion {
        Suscripcion {
            frecuencia: Frecuencia::Anual,
            dia_de_facturacion: 1,
            ultimo_cobro: ultimo,
            renovacion,
        }
    }

    #[test]
    fn una_suscripcion_nunca_cobrada_espera_a_su_dia() {
        let s = mensual(15, MarcaDeCobro::Ninguna);
        assert!(!s.corresponde_cobrar(en(2026, 3, 14)), "el día 14 no toca");
        assert!(s.corresponde_cobrar(en(2026, 3, 15)), "el día 15 sí");
        assert!(s.corresponde_cobrar(en(2026, 3, 16)), "y después también");
    }

    #[test]
    fn cobrada_este_mes_no_vuelve_a_cobrarse() {
        let s = mensual(15, MarcaDeCobro::En { anio: 2026, mes: 3 });
        assert!(!s.corresponde_cobrar(en(2026, 3, 20)));
        assert!(s.corresponde_cobrar(en(2026, 4, 15)), "al mes siguiente sí");
    }

    #[test]
    fn el_dia_31_no_llega_en_los_meses_de_treinta() {
        // Divergencia fijada: en abril no hay día 31, de modo que el mes pasa
        // sin cargo. Se afirma para que corregirla sea un cambio deliberado.
        let s = mensual(31, MarcaDeCobro::En { anio: 2026, mes: 3 });
        for dia in 1..=30u32 {
            assert!(!s.corresponde_cobrar(en(2026, 4, dia)), "cobró el {dia} de abril");
        }
        assert!(s.corresponde_cobrar(en(2026, 5, 31)), "hasta mayo no vuelve a cobrar");
    }

    #[test]
    fn una_anual_espera_a_la_fecha_que_tiene_anotada() {
        // **CAMBIO DE CONDUCTA.** Antes se recobraba al cambiar el año: la
        // cobrada el 05/07/2025 volvía a cobrar el 05/01/2026, seis meses
        // antes. Ahora espera a su fecha.
        let s = anual(Some(en(2026, 7, 5)), MarcaDeCobro::En { anio: 2025, mes: 7 });

        assert!(!s.corresponde_cobrar(en(2026, 1, 5)), "enero ya no dispara nada");
        assert!(!s.corresponde_cobrar(en(2026, 7, 4)), "ni la víspera");
        assert!(s.corresponde_cobrar(en(2026, 7, 5)), "el día anotado sí");
        assert!(s.corresponde_cobrar(en(2026, 8, 1)), "y después sigue pendiente");
    }

    #[test]
    fn una_anual_sin_fecha_anotada_no_se_cobra() {
        // Entre un cargo de más y uno de menos, el de menos es el que se
        // corrige mirando el estado de cuenta.
        let s = anual(None, MarcaDeCobro::En { anio: 2020, mes: 1 });
        assert!(!s.corresponde_cobrar(en(2026, 12, 31)));
    }

    #[test]
    fn en_una_anual_una_marca_ilegible_ya_no_fuerza_el_cobro() {
        // El defecto que invertía la idempotencia queda cerrado en las
        // anuales, porque ya no dependen de la marca.
        let s = anual(Some(en(2026, 7, 5)), MarcaDeCobro::Ilegible);
        assert!(!s.corresponde_cobrar(en(2026, 3, 1)), "sin fecha vencida no cobra");
        assert!(s.corresponde_cobrar(en(2026, 7, 5)), "y en su fecha sí");

        // En las mensuales sigue abierto: ahí la marca es el único dato.
        assert!(mensual(15, MarcaDeCobro::Ilegible).corresponde_cobrar(en(2026, 3, 1)));
    }

    #[test]
    fn el_aviso_se_enciende_una_semana_antes_y_no_despues_de_cobrar() {
        let s = anual(Some(en(2026, 7, 5)), MarcaDeCobro::Ninguna);

        assert!(!s.avisa(en(2026, 6, 27)), "ocho días antes todavía no");
        assert!(s.avisa(en(2026, 6, 28)), "siete días antes sí");
        assert!(s.avisa(en(2026, 7, 4)), "la víspera");
        assert!(s.avisa(en(2026, 7, 5)), "y el mismo día");
        assert!(!s.avisa(en(2026, 7, 6)), "pasada la fecha ya no es aviso, es cobro pendiente");
    }

    #[test]
    fn una_mensual_no_anuncia_una_fecha_que_el_sistema_no_respetaria() {
        // Mientras el día 31 siga saltándose meses, predecir el próximo cobro
        // de una mensual sería anunciar algo que luego no ocurre.
        let s = mensual(30, MarcaDeCobro::En { anio: 2026, mes: 1 });
        assert_eq!(s.proximo_cobro(), None);
        assert!(!s.avisa(en(2026, 2, 25)));
    }

    #[test]
    fn la_renovacion_siguiente_traslada_el_29_de_febrero_al_28() {
        // 2028 es bisiesto; 2029 no.
        let s = anual(Some(en(2028, 2, 29)), MarcaDeCobro::Ninguna);
        assert_eq!(s.renovacion_siguiente(), Some(en(2029, 2, 28)),
                   "adelantar al 1 de marzo movería el cargo de mes");

        let s = anual(Some(en(2026, 7, 5)), MarcaDeCobro::Ninguna);
        assert_eq!(s.renovacion_siguiente(), Some(en(2027, 7, 5)));
    }

    #[test]
    fn una_marca_ilegible_obliga_a_cobrar_aunque_no_toque() {
        // Divergencia fijada, y la que invierte la idempotencia: cobra
        // incluso antes del día de facturación.
        let s = mensual(15, MarcaDeCobro::Ilegible);
        assert!(s.corresponde_cobrar(en(2026, 3, 1)), "cobra el día 1 teniendo el 15");
    }

    #[test]
    fn una_marca_en_otro_formato_es_ilegible_y_no_ausente() {
        // La distinción importa: ausente espera a su día, ilegible no espera.
        assert_eq!(MarcaDeCobro::desde_texto(None), MarcaDeCobro::Ninguna);
        for texto in ["2026-01-10", "", "ayer", "10-01-2026", "x/y/z", "1/2"] {
            assert_eq!(
                MarcaDeCobro::desde_texto(Some(texto)),
                MarcaDeCobro::Ilegible,
                "«{texto}» debería leerse como ilegible"
            );
        }
        assert_eq!(
            MarcaDeCobro::desde_texto(Some("10/01/2026")),
            MarcaDeCobro::En { anio: 2026, mes: 1 }
        );
    }

    #[test]
    fn una_frecuencia_desconocida_no_se_interpreta() {
        assert_eq!(Frecuencia::desde_codigo("mensual"), Some(Frecuencia::Mensual));
        assert_eq!(Frecuencia::desde_codigo("anual"), Some(Frecuencia::Anual));
        assert_eq!(Frecuencia::desde_codigo("semanal"), None);
        assert_eq!(Frecuencia::desde_codigo("Mensual"), None, "el CHECK guarda minúsculas");
    }

    #[test]
    fn un_ano_completo_del_dia_31_deja_cinco_meses_sin_cargo() {
        // La misma medición que `s10` hace contra la base, aquí sin ella.
        let mut cobros = 0;
        let mut marca = MarcaDeCobro::Ninguna;
        for mes in 1..=12u32 {
            let dias = match mes {
                1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                4 | 6 | 9 | 11 => 30,
                _ => 28,
            };
            for dia in 1..=dias {
                let s = Suscripcion {
                    frecuencia: Frecuencia::Mensual,
                    dia_de_facturacion: 31,
                    ultimo_cobro: marca.clone(),
                    renovacion: None,
                };
                if s.corresponde_cobrar(en(2026, mes, dia)) {
                    cobros += 1;
                    marca = MarcaDeCobro::En { anio: 2026, mes };
                }
            }
        }
        assert_eq!(cobros, 7, "siete cargos donde el proveedor hace doce");
    }
}
