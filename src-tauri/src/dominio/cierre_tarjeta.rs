//! A qué tarjeta conviene trasladar cada suscripción cuando se desactiva una tarjeta.
//!
//! **Es una recomendación explicable, no una decisión**: el usuario ve el motivo, puede elegir cualquier destino válido y
//! nada se ejecuta por haber sido sugerido. Todo se compara en centavos exactos y **por divisa**: nunca se suman pesos y
//! dólares ni se supone una tasa.
//!
//! Orden de preferencia entre destinos que tienen cupo para el cargo (se descartan los que no):
//! 1. el que tiene margen **sin depender del sobregiro**;
//! 2. para un cargo en dólares, el que lo **conserva en dólares** (política `origen`) frente al que lo traduce a pesos,
//!    cuya conversión queda pendiente y se avisa;
//! 3. el que da **más días entre el próximo cargo y su siguiente corte** (estimación del ciclo, no una promesa de
//!    financiación sin intereses);
//! 4. el de **mayor margen remanente** tras asignarle esta suscripción y las anteriores;
//! 5. el de menor identificador, para que la sugerencia sea estable.
//!
//! La proyección resta **todas** las suscripciones ya asignadas a cada destino, para no recomendar varias por separado
//! sobre el mismo cupo.

use super::dinero::{Dinero, Divisa};
use super::tarjeta::{LimitesDivisa, PoliticaLiquidacion};
use chrono::{Datelike, NaiveDate};
use std::collections::HashMap;

/// El cupo de una tarjeta en una divisa: sus límites y lo que debe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CupoDeDivisa {
    pub limites: LimitesDivisa,
    pub balance: Dinero,
}

impl CupoDeDivisa {
    /// Lo que queda por consumir contando el sobregiro, en centavos (puede ser negativo).
    pub fn disponible(&self) -> i64 {
        self.limites.disponible(self.balance).map(|d| d.centavos()).unwrap_or(i64::MIN)
    }

    /// Lo que queda por consumir **sin** el sobregiro, en centavos (puede ser negativo).
    pub fn margen_sin_sobregiro(&self) -> i64 {
        self.limites.efectivo().centavos() - self.balance.centavos()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DestinoCandidato {
    pub id: i64,
    pub dop: CupoDeDivisa,
    pub usd: CupoDeDivisa,
    pub politica: PoliticaLiquidacion,
    /// Día del mes del corte (1 a 31; en un mes corto cae en su último día).
    pub dia_corte: i32,
}

impl DestinoCandidato {
    pub fn cupo(&self, divisa: Divisa) -> &CupoDeDivisa {
        match divisa {
            Divisa::Dop => &self.dop,
            Divisa::Usd => &self.usd,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SuscripcionAMover {
    pub id: i64,
    pub monto: Dinero,
    pub proximo_cobro: Option<NaiveDate>,
}

/// Lo que se le propone al usuario para una suscripción.
#[derive(Debug, Clone, PartialEq)]
pub struct Sugerencia {
    pub suscripcion_id: i64,
    /// `None` si ningún destino tiene cupo, o si no hay datos para recomendar con fundamento.
    pub destino_id: Option<i64>,
    /// Todos los destinos con cupo, del mejor al peor; el usuario puede elegir cualquiera de ellos.
    pub elegibles: Vec<i64>,
    pub motivo: String,
}

/// El corte que sigue a una fecha: el día `dia_corte` de ese mes si todavía no pasó, o el del mes siguiente. Un día 29,
/// 30 o 31 se limita al último día real del mes (también en febrero bisiesto).
pub fn siguiente_corte(desde: NaiveDate, dia_corte: i32) -> NaiveDate {
    let en_mes = |anio: i32, mes: u32| -> NaiveDate {
        let ultimo = super::suscripcion::dias_del_mes(anio, mes);
        let dia = (dia_corte.clamp(1, 31) as u32).min(ultimo);
        NaiveDate::from_ymd_opt(anio, mes, dia).expect("día válido por construcción")
    };
    let este = en_mes(desde.year(), desde.month());
    if este >= desde {
        return este;
    }
    let (anio, mes) = if desde.month() == 12 { (desde.year() + 1, 1) } else { (desde.year(), desde.month() + 1) };
    en_mes(anio, mes)
}

/// Días entre el cargo y el corte que lo recoge: más días es más tiempo hasta que el cargo entra en un estado de cuenta.
pub fn dias_hasta_el_corte(cargo: NaiveDate, dia_corte: i32) -> i64 {
    (siguiente_corte(cargo, dia_corte) - cargo).num_days()
}

fn importe(c: i64, divisa: Divisa) -> String {
    format!("{} {:.2}", divisa.codigo(), Dinero::de_centavos(c, divisa).unidades())
}

/// Propone un destino por suscripción. Las suscripciones se reparten de mayor a menor importe (a igualdad, por
/// identificador) para que las grandes encuentren sitio antes de que las pequeñas lo ocupen.
pub fn recomendar(suscripciones: &[SuscripcionAMover], candidatos: &[DestinoCandidato]) -> Vec<Sugerencia> {
    let mut orden: Vec<&SuscripcionAMover> = suscripciones.iter().collect();
    orden.sort_by(|a, b| b.monto.centavos().cmp(&a.monto.centavos()).then(a.id.cmp(&b.id)));

    let mut asignado: HashMap<(i64, Divisa), i64> = HashMap::new();
    let mut sugerencias: HashMap<i64, Sugerencia> = HashMap::new();

    for s in orden {
        let divisa = s.monto.divisa();
        let cargo = s.monto.centavos();

        // (candidato, margen_sin_sobregiro_restante, margen_total_restante, dias, conserva_divisa)
        let mut elegibles: Vec<(&DestinoCandidato, i64, i64, Option<i64>)> = candidatos
            .iter()
            .filter_map(|c| {
                let ya = *asignado.get(&(c.id, divisa)).unwrap_or(&0);
                let total = c.cupo(divisa).disponible() - ya - cargo;
                if total < 0 {
                    return None;
                }
                let sin_sobregiro = c.cupo(divisa).margen_sin_sobregiro() - ya - cargo;
                let dias = s.proximo_cobro.map(|f| dias_hasta_el_corte(f, c.dia_corte));
                Some((c, sin_sobregiro, total, dias))
            })
            .collect();

        let conserva = |c: &DestinoCandidato| divisa == Divisa::Dop || c.politica == PoliticaLiquidacion::EnDivisaDeOrigen;
        elegibles.sort_by(|a, b| {
            (b.1 >= 0)
                .cmp(&(a.1 >= 0))
                .then(conserva(b.0).cmp(&conserva(a.0)))
                .then(b.3.unwrap_or(0).cmp(&a.3.unwrap_or(0)))
                .then(b.2.cmp(&a.2))
                .then(a.0.id.cmp(&b.0.id))
        });

        let ids: Vec<i64> = elegibles.iter().map(|e| e.0.id).collect();
        let sugerencia = match elegibles.first() {
            None => Sugerencia {
                suscripcion_id: s.id,
                destino_id: None,
                elegibles: Vec::new(),
                motivo: format!("Ninguna tarjeta activa tiene cupo para {} (contando lo ya asignado).", importe(cargo, divisa)),
            },
            Some((c, sin_sobregiro, total, dias)) => {
                *asignado.entry((c.id, divisa)).or_insert(0) += cargo;
                let mut partes = Vec::new();
                if *sin_sobregiro >= 0 {
                    partes.push(format!("le quedan {} libres sin usar sobregiro", importe(*sin_sobregiro, divisa)));
                } else {
                    partes.push(format!("cabe, pero usando sobregiro ({} de margen total)", importe(*total, divisa)));
                }
                match dias {
                    Some(d) => partes.push(format!("el cargo cae {d} días antes de su siguiente corte")),
                    None => partes.push("sin fecha de próximo cargo no se compara el ciclo".to_string()),
                }
                if divisa == Divisa::Usd {
                    partes.push(if c.politica == PoliticaLiquidacion::EnDivisaDeOrigen {
                        "conserva el cargo en dólares".to_string()
                    } else {
                        "AVISO: traduce a pesos y la conversión queda pendiente".to_string()
                    });
                }
                let mut motivo = partes.join("; ");
                if let Some(primera) = motivo.get(0..1) {
                    motivo = format!("{}{}", primera.to_uppercase(), &motivo[1..]);
                }
                Sugerencia { suscripcion_id: s.id, destino_id: Some(c.id), elegibles: ids, motivo: format!("{motivo}.") }
            }
        };
        sugerencias.insert(s.id, sugerencia);
    }

    suscripciones.iter().filter_map(|s| sugerencias.remove(&s.id)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dop(c: i64) -> Dinero {
        Dinero::de_centavos(c, Divisa::Dop)
    }
    fn usd(c: i64) -> Dinero {
        Dinero::de_centavos(c, Divisa::Usd)
    }
    fn cupo(divisa: Divisa, aprobado: i64, sobregiro: i64, balance: i64) -> CupoDeDivisa {
        let d = |c| Dinero::de_centavos(c, divisa);
        CupoDeDivisa { limites: LimitesDivisa::nuevos(d(aprobado), None, d(sobregiro)).unwrap(), balance: d(balance) }
    }
    fn destino(id: i64, dop_libre: i64, politica: PoliticaLiquidacion, dia_corte: i32) -> DestinoCandidato {
        DestinoCandidato {
            id,
            dop: cupo(Divisa::Dop, dop_libre, 0, 0),
            usd: cupo(Divisa::Usd, 100_000, 0, 0),
            politica,
            dia_corte,
        }
    }
    fn fecha(a: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(a, m, d).unwrap()
    }
    fn sus(id: i64, monto: Dinero, cargo: Option<NaiveDate>) -> SuscripcionAMover {
        SuscripcionAMover { id, monto, proximo_cobro: cargo }
    }
    const ORIGEN: PoliticaLiquidacion = PoliticaLiquidacion::EnDivisaDeOrigen;

    #[test]
    fn el_corte_de_un_mes_corto_se_limita_a_su_ultimo_dia_real() {
        assert_eq!(siguiente_corte(fecha(2026, 2, 10), 31), fecha(2026, 2, 28));
        assert_eq!(siguiente_corte(fecha(2028, 2, 10), 30), fecha(2028, 2, 29), "febrero bisiesto");
        assert_eq!(siguiente_corte(fecha(2026, 4, 20), 31), fecha(2026, 4, 30));
        assert_eq!(siguiente_corte(fecha(2026, 3, 20), 15), fecha(2026, 4, 15), "si ya pasó, el del mes siguiente");
        assert_eq!(siguiente_corte(fecha(2026, 12, 20), 15), fecha(2027, 1, 15), "cruza el año");
        assert_eq!(siguiente_corte(fecha(2026, 3, 15), 15), fecha(2026, 3, 15), "el mismo día cuenta");
    }

    #[test]
    fn dos_suscripciones_que_compiten_por_un_cupo_no_se_recomiendan_sobre_el_mismo_margen() {
        // TAR-04: A tiene 1 000.00 libres, B 5 000.00; dos cargos de 800.00. Sin proyectar, ambos irían a A.
        let a = destino(1, 100_000, ORIGEN, 15);
        let b = destino(2, 500_000, ORIGEN, 15);
        let s = vec![sus(10, dop(80_000), None), sus(11, dop(80_000), None)];
        let r = recomendar(&s, &[a, b]);
        let por = |id| r.iter().find(|x| x.suscripcion_id == id).unwrap().destino_id;
        assert_eq!(por(10), Some(2), "la mayor margen remanente se queda la primera");
        assert_eq!(por(11), Some(2), "B aún tiene margen para las dos (5 000 - 1 600) y A no puede con dos");
        let tres = vec![sus(10, dop(80_000), None), sus(11, dop(80_000), None), sus(12, dop(80_000), None)];
        let r = recomendar(&tres, &[destino(1, 100_000, ORIGEN, 15), destino(2, 200_000, ORIGEN, 15)]);
        let a_b = r.iter().filter(|x| x.destino_id == Some(2)).count();
        let a_a = r.iter().filter(|x| x.destino_id == Some(1)).count();
        assert_eq!((a_a, a_b), (1, 2), "no se promete el mismo cupo dos veces");
    }

    #[test]
    fn sin_cupo_en_ningun_destino_no_hay_recomendacion_y_se_dice() {
        let r = recomendar(&[sus(1, dop(90_000), None)], &[destino(2, 50_000, ORIGEN, 15)]);
        assert_eq!(r[0].destino_id, None);
        assert!(r[0].elegibles.is_empty());
        assert!(r[0].motivo.contains("Ninguna tarjeta activa tiene cupo"), "{}", r[0].motivo);
    }

    #[test]
    fn se_prefiere_el_margen_que_no_depende_del_sobregiro() {
        // A cabe solo usando sobregiro; B cabe sin él aunque tenga menos margen total.
        let con_sobregiro = DestinoCandidato { dop: cupo(Divisa::Dop, 10_000, 200_000, 0), ..destino(1, 0, ORIGEN, 15) };
        let sin = destino(2, 120_000, ORIGEN, 15);
        let r = recomendar(&[sus(1, dop(100_000), None)], &[con_sobregiro, sin]);
        assert_eq!(r[0].destino_id, Some(2));
        assert_eq!(r[0].elegibles, vec![2, 1], "A sigue siendo elegible, pero peor");
    }

    #[test]
    fn un_cargo_en_dolares_prefiere_conservarlo_y_avisa_si_el_destino_lo_traduce() {
        let traduce = destino(1, 500_000, PoliticaLiquidacion::TraduceAMonedaLocal, 15);
        let conserva = destino(2, 500_000, ORIGEN, 15);
        let r = recomendar(&[sus(1, usd(1_500), None)], &[traduce.clone(), conserva]);
        assert_eq!(r[0].destino_id, Some(2));
        assert!(r[0].motivo.contains("conserva el cargo en dólares"), "{}", r[0].motivo);
        let sola = recomendar(&[sus(1, usd(1_500), None)], &[traduce]);
        assert_eq!(sola[0].destino_id, Some(1), "si es la única con cupo, se ofrece, con aviso");
        assert!(sola[0].motivo.contains("AVISO: traduce a pesos"), "{}", sola[0].motivo);
    }

    #[test]
    fn entre_comparables_gana_el_que_deja_mas_dias_hasta_su_corte_y_luego_el_id() {
        let corta = destino(1, 500_000, ORIGEN, 20); // el cargo del día 18 queda a 2 días de su corte
        let larga = destino(2, 500_000, ORIGEN, 17); // y a 30 días del suyo
        let r = recomendar(&[sus(1, dop(1_000), Some(fecha(2026, 3, 18)))], &[corta, larga]);
        assert_eq!(r[0].destino_id, Some(2));
        assert!(r[0].motivo.contains("30 días"), "{}", r[0].motivo);

        let a = destino(7, 500_000, ORIGEN, 15);
        let b = destino(3, 500_000, ORIGEN, 15);
        let empate = recomendar(&[sus(1, dop(1_000), Some(fecha(2026, 3, 18)))], &[a, b]);
        assert_eq!(empate[0].destino_id, Some(3), "a igualdad total, el menor identificador");
    }

    #[test]
    fn sin_fecha_de_cargo_se_ofrecen_destinos_pero_el_motivo_no_finge_haber_comparado_el_ciclo() {
        let r = recomendar(&[sus(1, dop(1_000), None)], &[destino(1, 500_000, ORIGEN, 15)]);
        assert_eq!(r[0].destino_id, Some(1));
        assert!(r[0].motivo.contains("sin fecha de próximo cargo no se compara el ciclo"), "{}", r[0].motivo);
    }

    #[test]
    fn las_sugerencias_salen_en_el_orden_de_las_suscripciones_recibidas() {
        let s = vec![sus(5, dop(100), None), sus(2, dop(900), None), sus(9, dop(500), None)];
        let r = recomendar(&s, &[destino(1, 500_000, ORIGEN, 15)]);
        assert_eq!(r.iter().map(|x| x.suscripcion_id).collect::<Vec<_>>(), vec![5, 2, 9]);
    }
}
