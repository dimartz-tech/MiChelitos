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
//! * Una **anual** se recobra al cambiar el año aunque no haya pasado uno.
//! * Varios períodos vencidos generan **un solo cargo**.
//! * Una marca de cobro **ilegible** obliga a cobrar.
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
}

impl Suscripcion {
    /// Si hoy toca cargar.
    ///
    /// Reproduce la decisión actual sin alterarla. Lo que cambia es que ahora
    /// **se puede leer**, y cada rama dice qué deja fuera.
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
            MarcaDeCobro::Ilegible => true,

            MarcaDeCobro::Ninguna => dia_alcanzado,

            // **Divergencia: varios períodos vencidos generan un cargo.** La
            // marca es una fecha, no un contador, y al cobrar se pone «hoy»:
            // los meses intermedios desaparecen.
            MarcaDeCobro::En { anio, mes } => match self.frecuencia {
                Frecuencia::Mensual => {
                    let mes_nuevo =
                        hoy.year() > *anio || (hoy.year() == *anio && hoy.month() > *mes);
                    mes_nuevo && dia_alcanzado
                }
                // **Divergencia: la anual no mira el mes.** Cobrada en julio,
                // se recobra el 5 de enero siguiente. Bastaría comparar meses
                // absolutos, y por eso el arreglo es tentador; sigue siendo
                // una decisión sobre dinero real.
                Frecuencia::Anual => hoy.year() > *anio && dia_alcanzado,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn en(anio: i32, mes: u32, dia: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(anio, mes, dia).expect("fecha válida")
    }

    fn mensual(dia: u32, ultimo: MarcaDeCobro) -> Suscripcion {
        Suscripcion { frecuencia: Frecuencia::Mensual, dia_de_facturacion: dia, ultimo_cobro: ultimo }
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
    fn una_anual_se_recobra_al_cambiar_el_ano_sin_haber_pasado_uno() {
        // Divergencia fijada: seis meses después del cobro anterior.
        let s = Suscripcion {
            frecuencia: Frecuencia::Anual,
            dia_de_facturacion: 5,
            ultimo_cobro: MarcaDeCobro::En { anio: 2025, mes: 7 },
        };
        assert!(!s.corresponde_cobrar(en(2025, 12, 31)), "en el mismo año no");
        assert!(s.corresponde_cobrar(en(2026, 1, 5)), "y en enero sí, seis meses antes");
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
