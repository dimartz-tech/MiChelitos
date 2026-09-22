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
//! * Varios períodos vencidos generan **un solo cargo**.
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
    /// Se cobró ese día.
    ///
    /// Guarda la fecha entera aunque la decisión mensual solo mire el mes.
    /// **No es precisión de adorno: es la única forma de saber que la fecha
    /// existe.** Antes se parseaban las tres partes y se usaban dos, y así
    /// un `31/02/2026` pasaba como febrero y un `13/13/2026` como mes 13.
    En(NaiveDate),
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

        // **La fecha tiene que existir**, no solo estar compuesta de
        // dígitos. `from_ymd_opt` es quien lo decide: un `31/02/2026` tiene
        // la forma correcta y no es un día, y el `CHECK` del esquema —que
        // comprueba forma— lo deja pasar. Aquí es donde se detiene.
        match (
            partes[0].parse::<u32>(),
            partes[1].parse::<u32>(),
            partes[2].parse::<i32>(),
        ) {
            (Ok(dia), Ok(mes), Ok(anio)) => match NaiveDate::from_ymd_opt(anio, mes, dia) {
                Some(fecha) => MarcaDeCobro::En(fecha),
                None => MarcaDeCobro::Ilegible,
            },
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
        let dia_alcanzado = hoy.day() >= self.dia_de_facturacion_en(hoy);

        match &self.ultimo_cobro {
            // Una marca ilegible **no autoriza a cobrar**. Antes sí: era la
            // rama que invertía la propiedad que sostiene el mecanismo.
            //
            // No se puede saber cuándo se cobró por última vez, de modo que
            // la elección es entre arriesgar un cargo de más y uno de menos.
            // El de menos se corrige mirando el estado de cuenta; el de más
            // hay que deshacerlo. Es el mismo criterio que la anual sin fecha
            // de renovación.
            //
            // **A cambio, no puede quedarse callado.** Una suscripción que
            // deja de cobrarse en silencio es peor que una que cobra de más,
            // y por eso existe `impedimento`.
            MarcaDeCobro::Ilegible => match self.frecuencia {
                Frecuencia::Anual => self.vence_la_renovacion(hoy),
                Frecuencia::Mensual => false,
            },

            MarcaDeCobro::Ninguna => match self.frecuencia {
                Frecuencia::Anual => self.vence_la_renovacion(hoy),
                Frecuencia::Mensual => dia_alcanzado,
            },

            // **Divergencia: varios períodos vencidos generan un cargo.** La
            // marca es una fecha, no un contador, y al cobrar se pone «hoy»:
            // los meses intermedios desaparecen.
            MarcaDeCobro::En(cobrado) => match self.frecuencia {
                Frecuencia::Mensual => {
                    let mes_nuevo = hoy.year() > cobrado.year()
                        || (hoy.year() == cobrado.year() && hoy.month() > cobrado.month());
                    mes_nuevo && dia_alcanzado
                }
                // La anual ya no deduce: **lee la fecha anotada**. Ver
                // `renovacion` y `s12b`.
                Frecuencia::Anual => self.vence_la_renovacion(hoy),
            },
        }
    }

    /// Por qué esta suscripción no va a cobrarse nunca, si es el caso.
    ///
    /// Existe porque las dos decisiones prudentes del módulo —no cobrar sin
    /// fecha de renovación, no cobrar con marca ilegible— **dejan a la
    /// suscripción parada**, y una suscripción parada en silencio es peor que
    /// una que cobra de más: la de más se ve en el estado de cuenta, la
    /// parada no se ve en ninguna parte.
    ///
    /// Devuelve `None` cuando no hay impedimento, lo que **no** significa que
    /// hoy toque cobrar: eso lo dice `corresponde_cobrar`.
    pub fn impedimento(&self) -> Option<Impedimento> {
        match self.frecuencia {
            // La mensual decide con la marca: si no se entiende, no hay con
            // qué decidir.
            Frecuencia::Mensual => match self.ultimo_cobro {
                MarcaDeCobro::Ilegible => Some(Impedimento::MarcaDeCobroIlegible),
                _ => None,
            },
            // La anual decide con su fecha de renovación y **no mira la
            // marca**, de modo que una marca ilegible no la impide cobrar.
            // Señalarla aquí sería decirle al titular que algo está parado
            // cuando no lo está, y un aviso que miente se aprende a ignorar.
            Frecuencia::Anual => match self.renovacion {
                None => Some(Impedimento::AnualSinRenovacion),
                Some(_) => None,
            },
        }
    }

    /// El día en que factura, **recortado a los días que tiene ese mes**.
    ///
    /// Antes se comparaba contra el día crudo, y `hoy.day()` nunca llega a 31
    /// en un mes de 30 ni a 30 en febrero: esos meses pasaban sin cargo. Una
    /// mensual del día 31 se cobraba 7 de 12 veces al año.
    ///
    /// **La fecha sale del estado de cuenta, no de lo que parezca lógico.**
    /// Un cargo del día 29 se generó el 28 de febrero, el último día del mes.
    /// Que se liquidara el 1 de marzo es otra cosa: esta aplicación asienta
    /// el consumo contra la tarjeta, y el consumo lleva la fecha en que el
    /// emisor lo generó. La liquidación entra por el ciclo de pago de la
    /// tarjeta, que se lleva aparte.
    fn dia_de_facturacion_en(&self, hoy: NaiveDate) -> u32 {
        self.dia_de_facturacion.min(dias_del_mes(hoy.year(), hoy.month()))
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
    /// **Solo las anuales lo dicen**, y ahora por una razón distinta de la
    /// original. Cuando esto se escribió, una mensual no podía anunciar su
    /// próximo cobro porque el día 31 se saltaba meses y la fecha anunciada
    /// no se habría cumplido. Ese defecto está cerrado: el día se recorta al
    /// último del mes, así que la fecha **ya sería predecible**.
    ///
    /// Se mantiene anual porque es lo que el titular pidió del aviso, no
    /// porque siga siendo imposible. Extenderlo a las mensuales es ahora un
    /// cambio pequeño y deliberado, y conviene que se note la diferencia.
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
    /// común, y adelantar al 1 de marzo movería el cargo de mes. Es la misma
    /// decisión que toma `dia_de_facturacion_en` para las mensuales.
    pub fn renovacion_siguiente(&self) -> Option<NaiveDate> {
        let fecha = self.renovacion?;
        let anio = fecha.year() + 1;
        NaiveDate::from_ymd_opt(anio, fecha.month(), fecha.day())
            .or_else(|| NaiveDate::from_ymd_opt(anio, fecha.month(), 28))
    }
}

fn dias_del_mes(anio: i32, mes: u32) -> u32 {
    match mes {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if anio % 4 == 0 && (anio % 100 != 0 || anio % 400 == 0) => 29,
        _ => 28,
    }
}

/// Lo que impide que una suscripción llegue a cobrarse.
///
/// Es un estado que hay que **resolver**, no un error de programa: en los dos
/// casos falta un dato que solo el titular puede aportar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Impedimento {
    /// La fecha del último cobro no se entiende, así que no se sabe si toca.
    MarcaDeCobroIlegible,
    /// Una anual sin fecha de renovación anotada.
    AnualSinRenovacion,
}

impl Impedimento {
    /// Qué decirle a quien tiene que arreglarlo.
    ///
    /// El mensaje vive aquí y no en la vista por la misma razón que el aviso:
    /// si cambia la regla, el texto que la explica tiene que cambiar con
    /// ella, y para eso han de estar juntos.
    pub fn explicacion(&self) -> &'static str {
        match self {
            Impedimento::MarcaDeCobroIlegible => {
                "La fecha del último cobro no se entiende, así que no se sabe \
                 si toca cobrar. No se cobrará hasta corregirla."
            }
            Impedimento::AnualSinRenovacion => {
                "Falta la fecha de renovación. Una anual no se cobra sin ella."
            }
        }
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
        let s = mensual(15, MarcaDeCobro::En(en(2026, 3, 15)));
        assert!(!s.corresponde_cobrar(en(2026, 3, 20)));
        assert!(s.corresponde_cobrar(en(2026, 4, 15)), "al mes siguiente sí");
    }

    #[test]
    fn el_dia_de_facturacion_se_recorta_a_los_dias_que_tiene_el_mes() {
        // **CAMBIO DE CONDUCTA.** Antes abril pasaba sin cargo por no tener
        // un día 31. Ahora el cargo se genera el 30, que es lo que hace el
        // emisor: un cargo del día 29 se generó el 28 de febrero.
        let s = mensual(31, MarcaDeCobro::En(en(2026, 3, 15)));
        for dia in 1..=29u32 {
            assert!(!s.corresponde_cobrar(en(2026, 4, dia)), "aún no es el último día");
        }
        assert!(s.corresponde_cobrar(en(2026, 4, 30)), "el 30 de abril, que es el último");
    }

    #[test]
    fn en_febrero_el_cargo_del_dia_29_se_genera_el_28() {
        // El caso real, con su fecha: 2026 no es bisiesto.
        let s = mensual(29, MarcaDeCobro::En(en(2026, 1, 15)));
        assert!(!s.corresponde_cobrar(en(2026, 2, 27)));
        assert!(s.corresponde_cobrar(en(2026, 2, 28)), "el último día de febrero");
    }

    #[test]
    fn en_un_febrero_bisiesto_el_ultimo_dia_es_el_29() {
        // 2028 sí es bisiesto. El recorte sigue al mes, no a una cifra fija.
        let s = mensual(30, MarcaDeCobro::En(en(2028, 1, 15)));
        assert!(!s.corresponde_cobrar(en(2028, 2, 28)));
        assert!(s.corresponde_cobrar(en(2028, 2, 29)));
    }

    #[test]
    fn recortar_el_dia_no_adelanta_el_cargo_de_un_mes_largo() {
        // El recorte solo actúa donde el día no existe. En marzo, una del 30
        // sigue esperando al 30.
        let s = mensual(30, MarcaDeCobro::En(en(2026, 2, 15)));
        assert!(!s.corresponde_cobrar(en(2026, 3, 29)));
        assert!(s.corresponde_cobrar(en(2026, 3, 30)));
    }

    #[test]
    fn una_anual_espera_a_la_fecha_que_tiene_anotada() {
        // **CAMBIO DE CONDUCTA.** Antes se recobraba al cambiar el año: la
        // cobrada el 05/07/2025 volvía a cobrar el 05/01/2026, seis meses
        // antes. Ahora espera a su fecha.
        let s = anual(Some(en(2026, 7, 5)), MarcaDeCobro::En(en(2025, 7, 5)));

        assert!(!s.corresponde_cobrar(en(2026, 1, 5)), "enero ya no dispara nada");
        assert!(!s.corresponde_cobrar(en(2026, 7, 4)), "ni la víspera");
        assert!(s.corresponde_cobrar(en(2026, 7, 5)), "el día anotado sí");
        assert!(s.corresponde_cobrar(en(2026, 8, 1)), "y después sigue pendiente");
    }

    #[test]
    fn una_anual_sin_fecha_anotada_no_se_cobra() {
        // Entre un cargo de más y uno de menos, el de menos es el que se
        // corrige mirando el estado de cuenta.
        let s = anual(None, MarcaDeCobro::En(en(2020, 1, 5)));
        assert!(!s.corresponde_cobrar(en(2026, 12, 31)));
    }

    #[test]
    fn una_anual_con_marca_ilegible_sigue_su_fecha_de_renovacion() {
        // La anual no depende de la marca, así que una ilegible no la
        // desvía. Aun así **queda señalada**: la marca sigue siendo un dato
        // roto que el titular debería corregir.
        let s = anual(Some(en(2026, 7, 5)), MarcaDeCobro::Ilegible);
        assert!(!s.corresponde_cobrar(en(2026, 3, 1)), "sin fecha vencida no cobra");
        assert!(s.corresponde_cobrar(en(2026, 7, 5)), "y en su fecha sí");
        // Y **no** se señala como impedida: cobra. Señalarla sería decir que
        // algo está parado cuando no lo está, y un aviso que miente se
        // aprende a ignorar. La marca se reescribe sola al cobrar.
        assert_eq!(s.impedimento(), None);
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
        let s = mensual(30, MarcaDeCobro::En(en(2026, 1, 15)));
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
    fn una_marca_ilegible_no_autoriza_a_cobrar() {
        // **CAMBIO DE CONDUCTA.** Antes cobraba, y en cada arranque: era la
        // rama que invertía la propiedad que da nombre a la fase.
        //
        // No se puede saber cuándo se cobró por última vez. Entre arriesgar
        // un cargo de más y uno de menos, el de menos se corrige mirando el
        // estado de cuenta.
        let s = mensual(15, MarcaDeCobro::Ilegible);
        for dia in 1..=28u32 {
            assert!(!s.corresponde_cobrar(en(2026, 2, dia)), "cobró el día {dia}");
        }
    }

    #[test]
    fn una_suscripcion_parada_dice_por_que() {
        // La contrapartida de no cobrar: quedarse parada **en silencio**
        // sería peor que cobrar de más. Un cargo indebido se ve en el estado
        // de cuenta; una suscripción que dejó de registrarse no se ve en
        // ninguna parte.
        let s = mensual(15, MarcaDeCobro::Ilegible);
        assert_eq!(s.impedimento(), Some(Impedimento::MarcaDeCobroIlegible));

        // Una anual sin fecha está parada aunque su marca se entienda.
        let s = anual(None, MarcaDeCobro::En(en(2026, 1, 15)));
        assert_eq!(s.impedimento(), Some(Impedimento::AnualSinRenovacion));

        let s = anual(None, MarcaDeCobro::Ninguna);
        assert_eq!(s.impedimento(), Some(Impedimento::AnualSinRenovacion));

        // Y una sana no inventa un impedimento.
        assert_eq!(mensual(15, MarcaDeCobro::Ninguna).impedimento(), None);
        assert_eq!(anual(Some(en(2027, 1, 1)), MarcaDeCobro::Ninguna).impedimento(), None);
    }

    #[test]
    fn no_tener_impedimento_no_significa_que_hoy_toque_cobrar() {
        // Son dos preguntas distintas, y confundirlas haría que la vista
        // anunciara un cargo inminente cada vez que la suscripción está sana.
        let s = mensual(15, MarcaDeCobro::En(en(2026, 3, 15)));
        assert_eq!(s.impedimento(), None);
        assert!(!s.corresponde_cobrar(en(2026, 3, 20)), "ya se cobró este mes");
    }

    #[test]
    fn una_marca_en_otro_formato_es_ilegible_y_no_ausente() {
        // La distinción importa: ausente espera a su día, ilegible no espera.
        assert_eq!(MarcaDeCobro::desde_texto(None), MarcaDeCobro::Ninguna);
        // Los dos últimos tienen la forma que el esquema exige y aun así no
        // son fechas: es el hueco que el `CHECK` no cubre.
        for texto in ["2026-01-10", "", "ayer", "10-01-2026", "x/y/z", "1/2",
                      "31/02/2026", "13/13/2026"] {
            assert_eq!(
                MarcaDeCobro::desde_texto(Some(texto)),
                MarcaDeCobro::Ilegible,
                "«{texto}» debería leerse como ilegible"
            );
        }
        assert_eq!(
            MarcaDeCobro::desde_texto(Some("10/01/2026")),
            MarcaDeCobro::En(en(2026, 1, 10))
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
    fn un_ano_completo_del_dia_31_ya_no_deja_ningun_mes_sin_cargo() {
        // La misma medición que `s10` hace contra la base, aquí sin ella.
        // Antes daba siete.
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
                    marca = MarcaDeCobro::En(en(2026, mes, dia));
                }
            }
        }
        assert_eq!(cobros, 12, "doce, los mismos que hace el proveedor");
    }
}
