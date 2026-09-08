use crate::puertos::reloj::Reloj;
use chrono::{Local, NaiveDate};

pub struct RelojSistema;

impl Reloj for RelojSistema {
    fn hoy(&self) -> NaiveDate {
        Local::now().date_naive()
    }
}
