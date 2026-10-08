//! ¿Qué períodos de una suscripción están vencidos y sin saldar?
//!
//! ## Lo que cambió, y por qué era la raíz
//!
//! La pregunta de antes era **«¿estamos en un mes posterior al del último
//! cobro y ya llegó el día de facturación?»**, y la marca de idempotencia
//! guardaba *cuándo se ejecutó* el cargo, no *qué período saldó*. Confundir
//! esas dos cosas era todo el defecto:
//!
//! Cada período tenía una **ventana** para ser cobrado —desde su día de
//! facturación hasta el fin de mes—, y si la aplicación no se abría dentro de
//! ella, al llegar el mes siguiente la misma marca pasaba a leerse como «ya
//! atendido». El período desaparecía sin dejar rastro. La ventana dependía
//! del día: 20 días para una del día 9, **un solo día** para una del 30. Le
//! costó a la base real un cargo de Netflix de agosto de 2026.
//!
//! Ahora la suscripción guarda **la fecha de su próximo cobro**. Una fecha
//! que ya pasó sigue pasada: no hay ventana que perder.
//!
//! ## Las dos piezas, que son hechos distintos
//!
//! * `proximo_cobro` — **qué período toca**. Es un puntero que avanza al
//!   saldar, y es lo que el titular ve y puede corregir.
//! * `dia_ancla` — **qué día del mes factura el proveedor**. Hace falta
//!   además de la fecha porque el recorte a fin de mes no puede persistirse:
//!   si una del 30 se cobra el 28 de febrero y el siguiente se calculara
//!   desde ese 28, quedaría anclada al 28 para siempre. El ancla es lo que
//!   hace que vuelva al 30 en marzo.
//!
//! No son dos versiones del mismo dato: uno dice *cuándo toca el siguiente*,
//! el otro *en qué día del mes cae*.
//!
//! ## Qué se cobra solo y qué se pregunta
//!
//! **Un período vencido se cobra solo. Dos o más, no.** El umbral no es
//! arbitrario: con uno no hay ambigüedad —la suscripción existe, acabas de
//! abrir la aplicación y le toca—. Con varios, la aplicación no sabe si el
//! proveedor los cobró, ni si la suscripción siguió activa durante la
//! ausencia, y **fabricar cargos que quizá no ocurrieron es peor que
//! señalarlos**. Se ofrecen para confirmar uno a uno contra el estado de
//! cuenta, que en este sistema es la fuente que manda.

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
    /// de elegir uno: quien llama decide, y hoy decide no cobrar.
    pub fn desde_codigo(codigo: &str) -> Option<Frecuencia> {
        match codigo {
            "mensual" => Some(Frecuencia::Mensual),
            "anual" => Some(Frecuencia::Anual),
            _ => None,
        }
    }
}

/// Cuántos días antes se avisa de un cargo.
pub const DIAS_DE_AVISO: i64 = 7;

/// Cuántos períodos vencidos se listan como mucho.
///
/// Una suscripción abandonada durante años produciría una lista inmanejable,
/// y el titular no va a conciliar cincuenta cargos uno a uno. Doce es un año:
/// más atrás, la respuesta razonable no es confirmarlos sino descartarlos.
pub const MAXIMO_DE_PENDIENTES: usize = 12;

/// Lo que impide que una suscripción llegue a cobrarse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Impedimento {
    /// No se sabe cuándo toca el siguiente cobro.
    SinProximoCobro,
}

impl Impedimento {
    /// Qué decirle a quien tiene que arreglarlo.
    ///
    /// El mensaje vive aquí y no en la vista: si cambia la regla, el texto
    /// que la explica tiene que cambiar con ella.
    pub fn explicacion(&self) -> &'static str {
        match self {
            Impedimento::SinProximoCobro => {
                "Falta la fecha del próximo cobro. Sin ella no se cobrará."
            }
        }
    }
}

/// Lo que hace falta saber para decidir los cobros de una suscripción.
#[derive(Debug, Clone)]
pub struct Suscripcion {
    pub frecuencia: Frecuencia,
    /// Cuándo vence el próximo cobro sin saldar.
    ///
    /// `None` significa que no se sabe, y entonces no se cobra. Entre un
    /// cargo de más y uno de menos, el de menos se corrige mirando el estado
    /// de cuenta.
    pub proximo_cobro: Option<NaiveDate>,
    /// El día del mes en que factura el proveedor. Ver la cabecera.
    pub dia_ancla: u32,
}

impl Suscripcion {
    /// Todos los vencimientos que ya pasaron y siguen sin saldar.
    ///
    /// El primero es `proximo_cobro`; los siguientes se derivan avanzando un
    /// período cada vez. Se corta en `MAXIMO_DE_PENDIENTES`.
    pub fn periodos_vencidos(&self, hoy: NaiveDate) -> Vec<NaiveDate> {
        let mut vencidos = Vec::new();
        let mut cursor = match self.proximo_cobro {
            Some(fecha) => fecha,
            None => return vencidos,
        };

        while cursor <= hoy && vencidos.len() < MAXIMO_DE_PENDIENTES {
            vencidos.push(cursor);
            match self.siguiente_vencimiento(cursor) {
                Some(siguiente) => cursor = siguiente,
                None => break,
            }
        }
        vencidos
    }

    /// El período que se cobra sin preguntar, si lo hay.
    ///
    /// **Solo cuando hay exactamente uno vencido.** Ver la cabecera: con
    /// varios, la aplicación no puede saber si ocurrieron, y fabricarlos
    /// sería inventar movimientos de dinero.
    pub fn cobro_automatico(&self, hoy: NaiveDate) -> Option<NaiveDate> {
        let vencidos = self.periodos_vencidos(hoy);
        match vencidos.len() {
            1 => Some(vencidos[0]),
            _ => None,
        }
    }

    /// Los períodos que hay que confirmar a mano, si los hay.
    ///
    /// Vacío cuando hay cero o uno: con uno, se cobra solo y no hay nada que
    /// preguntar.
    pub fn pendientes_de_confirmar(&self, hoy: NaiveDate) -> Vec<NaiveDate> {
        let vencidos = self.periodos_vencidos(hoy);
        if vencidos.len() >= 2 {
            vencidos
        } else {
            Vec::new()
        }
    }

    /// El vencimiento que sigue a uno dado.
    ///
    /// **Se calcula desde el ancla, no desde la fecha recibida.** Si una del
    /// día 30 se cobró el 28 de febrero, el siguiente es el 30 de marzo y no
    /// el 28: de otro modo un mes corto anclaría la suscripción a su día para
    /// siempre.
    pub fn siguiente_vencimiento(&self, desde: NaiveDate) -> Option<NaiveDate> {
        let (anio, mes) = match self.frecuencia {
            Frecuencia::Mensual => {
                if desde.month() == 12 {
                    (desde.year() + 1, 1)
                } else {
                    (desde.year(), desde.month() + 1)
                }
            }
            Frecuencia::Anual => (desde.year() + 1, desde.month()),
        };
        let dia = self.dia_ancla.clamp(1, 31).min(dias_del_mes(anio, mes));
        NaiveDate::from_ymd_opt(anio, mes, dia)
    }

    /// Por qué esta suscripción no va a cobrarse nunca, si es el caso.
    ///
    /// Una suscripción parada **en silencio** es peor que una que cobra de
    /// más: el cargo indebido aparece en el estado de cuenta, la parada no
    /// aparece en ninguna parte.
    pub fn impedimento(&self) -> Option<Impedimento> {
        match self.proximo_cobro {
            None => Some(Impedimento::SinProximoCobro),
            Some(_) => None,
        }
    }

    /// Si toca avisar: el cargo cae dentro de los próximos siete días.
    ///
    /// **Todas, mensuales y anuales** (decisión del titular, 2026-10-02: antes
    /// solo las anuales, por temor al ruido). Sirve para dar tiempo a cambiar
    /// la tarjeta que cobra —por ejemplo a una que bonifique compras por
    /// internet— antes de que caiga el cargo.
    pub fn avisa(&self, hoy: NaiveDate) -> bool {
        match self.proximo_cobro {
            Some(fecha) => (0..=DIAS_DE_AVISO).contains(&(fecha - hoy).num_days()),
            None => false,
        }
    }
}

pub fn dias_del_mes(anio: i32, mes: u32) -> u32 {
    match mes {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if anio % 4 == 0 && (anio % 100 != 0 || anio % 400 == 0) => 29,
        _ => 28,
    }
}

// --- Reglas de alta y consulta (A-03: salen de `main.rs`) ----------------------------------------------

use super::dinero::Dinero;
use super::errores::ErrorDominio;

/// `dd/mm/aaaa` — el formato de la aplicación.
pub fn fecha_desde_texto(texto: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(texto, "%d/%m/%Y").ok()
}

/// La regla de una suscripción a partir de lo que guarda la fila.
///
/// Un solo sitio donde se traduce lo almacenado a la regla, para que el cobro, el aviso y la confirmación no puedan
/// discrepar sobre qué día vence una suscripción. Una frecuencia que el `CHECK` no admite no debería existir; si
/// existiera, no cobrar es lo correcto (`None`).
pub fn regla_de_un_registro(frecuencia: &str, proximo_cobro: Option<&str>, dia_facturacion: i32) -> Option<Suscripcion> {
    Some(Suscripcion {
        frecuencia: Frecuencia::desde_codigo(frecuencia)?,
        proximo_cobro: proximo_cobro.and_then(fecha_desde_texto),
        dia_ancla: dia_facturacion.max(1) as u32,
    })
}

/// Las condiciones de una suscripción: cuánto, cada cuánto y qué día (la divisa ya vino validada con el importe).
///
/// Antes solo el `CHECK` del esquema atajaba algo, con su mensaje crudo, y atajaba poco: un importe **negativo o cero**
/// entraba, y cobrarlo abonaba a la tarjeta cada período; un día de facturación fuera de 1 a 31 también. Orden: el
/// importe, la frecuencia y el día.
pub fn condiciones_de_suscripcion(importe: Dinero, frecuencia: &str, dia_facturacion: i32) -> Result<(), ErrorDominio> {
    if importe.es_cero() || importe.es_negativo() {
        return Err(ErrorDominio::SuscripcionSinImporte);
    }
    if Frecuencia::desde_codigo(frecuencia).is_none() {
        return Err(ErrorDominio::FrecuenciaDesconocida { codigo: frecuencia.to_string() });
    }
    if !(1..=31).contains(&dia_facturacion) {
        return Err(ErrorDominio::DiaDeFacturacionInvalido { dia: dia_facturacion });
    }
    Ok(())
}

/// La fecha del próximo cobro, si la dan, tiene que entenderse. En blanco es lo mismo que ausente.
///
/// Se rechaza una fecha ilegible en vez de guardarla: una suscripción con una fecha que no se puede leer no cobra, pero
/// **aparenta estar configurada**, y eso es peor que el hueco visible.
pub fn proximo_cobro_declarado(fecha: Option<String>) -> Result<Option<String>, ErrorDominio> {
    match fecha.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(texto) => match fecha_desde_texto(texto) {
            Some(_) => Ok(Some(texto.to_string())),
            None => Err(ErrorDominio::FechaDeCobroNoEntendida { fecha: texto.to_string() }),
        },
    }
}

/// La fecha que se pone a mano para sacar una suscripción de la parada: recortada y legible.
pub fn fecha_de_correccion(texto: &str) -> Result<String, ErrorDominio> {
    let fecha = texto.trim();
    if fecha_desde_texto(fecha).is_none() {
        return Err(ErrorDominio::FechaDeCorreccionNoEntendida { fecha: fecha.to_string() });
    }
    Ok(fecha.to_string())
}

#[cfg(test)]
mod tests_de_alta {
    use super::*;
    use crate::dominio::dinero::Divisa;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }

    #[test]
    fn las_condiciones_se_comprueban_en_orden_importe_frecuencia_dia() {
        assert_eq!(condiciones_de_suscripcion(dop(0.0), "semanal", 0), Err(ErrorDominio::SuscripcionSinImporte));
        assert_eq!(condiciones_de_suscripcion(dop(-1.0), "mensual", 5), Err(ErrorDominio::SuscripcionSinImporte));
        assert_eq!(
            condiciones_de_suscripcion(dop(5.0), "semanal", 0),
            Err(ErrorDominio::FrecuenciaDesconocida { codigo: "semanal".into() })
        );
        assert_eq!(condiciones_de_suscripcion(dop(5.0), "mensual", 0), Err(ErrorDominio::DiaDeFacturacionInvalido { dia: 0 }));
        assert_eq!(condiciones_de_suscripcion(dop(5.0), "anual", 32), Err(ErrorDominio::DiaDeFacturacionInvalido { dia: 32 }));
        assert_eq!(condiciones_de_suscripcion(dop(5.0), "mensual", 31), Ok(()));
        assert_eq!(condiciones_de_suscripcion(dop(5.0), "anual", 1), Ok(()));
    }

    #[test]
    fn el_proximo_cobro_en_blanco_es_ausente_y_uno_ilegible_se_rechaza_recortado() {
        assert_eq!(proximo_cobro_declarado(None), Ok(None));
        assert_eq!(proximo_cobro_declarado(Some("   ".into())), Ok(None));
        assert_eq!(proximo_cobro_declarado(Some(" 05/03/2026 ".into())), Ok(Some("05/03/2026".into())));
        assert_eq!(
            proximo_cobro_declarado(Some("  ayer ".into())),
            Err(ErrorDominio::FechaDeCobroNoEntendida { fecha: "ayer".into() })
        );
    }

    #[test]
    fn la_fecha_de_correccion_se_recorta_y_debe_entenderse() {
        assert_eq!(fecha_de_correccion(" 15/03/2026 "), Ok("15/03/2026".into()));
        assert_eq!(fecha_de_correccion(" ayer "), Err(ErrorDominio::FechaDeCorreccionNoEntendida { fecha: "ayer".into() }));
        assert_eq!(fecha_de_correccion(""), Err(ErrorDominio::FechaDeCorreccionNoEntendida { fecha: "".into() }));
    }

    #[test]
    fn la_regla_de_un_registro_traduce_lo_guardado_y_una_frecuencia_rara_no_cobra() {
        let r = regla_de_un_registro("mensual", Some("15/03/2026"), 15).unwrap();
        assert_eq!((r.frecuencia, r.dia_ancla), (Frecuencia::Mensual, 15));
        assert!(r.proximo_cobro.is_some());
        assert!(regla_de_un_registro("anual", Some("basura"), 0).unwrap().proximo_cobro.is_none(), "una fecha ilegible es «sin fecha»");
        assert_eq!(regla_de_un_registro("anual", None, 0).unwrap().dia_ancla, 1, "el ancla nunca baja de 1");
        assert!(regla_de_un_registro("semanal", Some("15/03/2026"), 15).is_none());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn en(anio: i32, mes: u32, dia: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(anio, mes, dia).expect("fecha válida")
    }

    fn mensual(proximo: Option<NaiveDate>, ancla: u32) -> Suscripcion {
        Suscripcion { frecuencia: Frecuencia::Mensual, proximo_cobro: proximo, dia_ancla: ancla }
    }

    fn anual(proximo: Option<NaiveDate>, ancla: u32) -> Suscripcion {
        Suscripcion { frecuencia: Frecuencia::Anual, proximo_cobro: proximo, dia_ancla: ancla }
    }

    #[test]
    fn antes_de_su_fecha_no_hay_nada_vencido() {
        let s = mensual(Some(en(2026, 3, 15)), 15);
        assert!(s.periodos_vencidos(en(2026, 3, 14)).is_empty());
        assert_eq!(s.cobro_automatico(en(2026, 3, 14)), None);
    }

    #[test]
    fn un_solo_periodo_vencido_se_cobra_sin_preguntar() {
        let s = mensual(Some(en(2026, 3, 15)), 15);
        assert_eq!(s.cobro_automatico(en(2026, 3, 15)), Some(en(2026, 3, 15)));
        assert_eq!(s.cobro_automatico(en(2026, 4, 1)), Some(en(2026, 3, 15)));
        assert!(s.pendientes_de_confirmar(en(2026, 4, 1)).is_empty());
    }

    #[test]
    fn la_ventana_desaparece_porque_una_fecha_pasada_sigue_pasada() {
        // **El defecto, invertido.** Antes, una del día 30 tenía un solo día
        // para ser vista; si no se abría la aplicación el 30 o el 31, el
        // período se perdía al llegar el mes siguiente.
        let s = mensual(Some(en(2026, 8, 30)), 30);

        // Se abre once días tarde y el período sigue ahí.
        assert_eq!(s.cobro_automatico(en(2026, 9, 10)), Some(en(2026, 8, 30)));
    }

    #[test]
    fn dos_periodos_vencidos_no_se_cobran_solos() {
        // El umbral. Con dos, la aplicación no sabe si el proveedor los cobró
        // ni si la suscripción siguió activa: fabricarlos sería inventar
        // movimientos de dinero.
        let s = mensual(Some(en(2026, 7, 30)), 30);

        let pendientes = s.pendientes_de_confirmar(en(2026, 9, 10));

        assert_eq!(pendientes, vec![en(2026, 7, 30), en(2026, 8, 30)]);
        assert_eq!(s.cobro_automatico(en(2026, 9, 10)), None, "no se cobra ninguno solo");
    }

    #[test]
    fn el_caso_real_de_netflix() {
        // Última marca 30/07/2026 y sin cargo de agosto. Con la regla
        // anterior, el 30 de septiembre habría generado **un** cargo y agosto
        // habría quedado absorbido para siempre.
        let s = mensual(Some(en(2026, 8, 30)), 30);

        let pendientes = s.pendientes_de_confirmar(en(2026, 9, 30));

        assert_eq!(pendientes, vec![en(2026, 8, 30), en(2026, 9, 30)],
                   "agosto y septiembre, los dos a la vista");
    }

    #[test]
    fn el_ancla_devuelve_la_suscripcion_a_su_dia_tras_un_mes_corto() {
        // Si el siguiente se calculara desde el 28 recortado, la suscripción
        // quedaría anclada al 28 para siempre.
        let s = mensual(Some(en(2027, 2, 28)), 30);
        assert_eq!(s.siguiente_vencimiento(en(2027, 2, 28)), Some(en(2027, 3, 30)));
    }

    #[test]
    fn un_mes_corto_recorta_el_vencimiento_a_su_ultimo_dia() {
        let s = mensual(Some(en(2027, 1, 30)), 30);
        assert_eq!(s.siguiente_vencimiento(en(2027, 1, 30)), Some(en(2027, 2, 28)));

        let bisiesto = mensual(Some(en(2028, 1, 30)), 30);
        assert_eq!(bisiesto.siguiente_vencimiento(en(2028, 1, 30)), Some(en(2028, 2, 29)));
    }

    #[test]
    fn diciembre_pasa_a_enero_del_ano_siguiente() {
        let s = mensual(Some(en(2026, 12, 15)), 15);
        assert_eq!(s.siguiente_vencimiento(en(2026, 12, 15)), Some(en(2027, 1, 15)));
    }

    #[test]
    fn una_anual_avanza_un_ano_y_el_29_de_febrero_se_recorta() {
        let s = anual(Some(en(2026, 7, 5)), 5);
        assert_eq!(s.siguiente_vencimiento(en(2026, 7, 5)), Some(en(2027, 7, 5)));

        // 2028 es bisiesto, 2029 no.
        let bisiesta = anual(Some(en(2028, 2, 29)), 29);
        assert_eq!(bisiesta.siguiente_vencimiento(en(2028, 2, 29)), Some(en(2029, 2, 28)));
    }

    #[test]
    fn una_suscripcion_sin_fecha_no_se_cobra_y_lo_dice() {
        let s = mensual(None, 15);
        assert!(s.periodos_vencidos(en(2026, 12, 31)).is_empty());
        assert_eq!(s.cobro_automatico(en(2026, 12, 31)), None);
        assert_eq!(s.impedimento(), Some(Impedimento::SinProximoCobro));
    }

    #[test]
    fn una_suscripcion_al_dia_no_inventa_un_impedimento() {
        let s = mensual(Some(en(2027, 1, 15)), 15);
        assert_eq!(s.impedimento(), None);
        assert!(s.periodos_vencidos(en(2026, 12, 31)).is_empty(), "todavía no vence");
    }

    #[test]
    fn la_lista_de_pendientes_tiene_tope() {
        // Una suscripción abandonada años produciría una lista que nadie va a
        // conciliar. Más atrás de un año, lo razonable es descartar.
        let s = mensual(Some(en(2020, 1, 15)), 15);
        assert_eq!(s.periodos_vencidos(en(2026, 1, 15)).len(), MAXIMO_DE_PENDIENTES);
    }

    #[test]
    fn anuales_y_mensuales_avisan_una_semana_antes() {
        let a = anual(Some(en(2026, 7, 5)), 5);
        assert!(!a.avisa(en(2026, 6, 27)), "ocho días antes todavía no");
        assert!(a.avisa(en(2026, 6, 28)), "siete días antes sí");
        assert!(a.avisa(en(2026, 7, 5)), "y el mismo día");
        assert!(!a.avisa(en(2026, 7, 6)), "pasada la fecha ya no es aviso, es cobro vencido");

        // La mensual avisa igual (desde 1.76.0), con la misma ventana.
        let m = mensual(Some(en(2026, 7, 5)), 5);
        assert!(!m.avisa(en(2026, 6, 27)), "ocho días antes todavía no");
        assert!(m.avisa(en(2026, 7, 1)));
        assert!(m.avisa(en(2026, 7, 5)));
        assert!(!m.avisa(en(2026, 7, 6)));

        // Sin fecha de próximo cobro no hay de qué avisar.
        assert!(!mensual(None, 5).avisa(en(2026, 7, 1)));
    }
}
