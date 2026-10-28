//! Adaptador SQLite de la lectura del patrimonio. **Solo `SELECT`**: nada de aquí escribe.
//!
//! Lee los importes `REAL` y los pasa a `Dinero` en la frontera (redondeo al centavo, una sola vez).

use super::gastos::AlmacenSqlite;
use crate::dominio::dinero::{Dinero, Divisa};
use crate::dominio::patrimonio::{CuentaDePatrimonio, PrestamoDePatrimonio, TarjetaDePatrimonio};
use crate::puertos::repositorios::*;

fn fallo(e: rusqlite::Error) -> ErrorAlmacen {
    ErrorAlmacen::Fallo(e.to_string())
}

fn dinero(unidades: f64, divisa: Divisa) -> Result<Dinero, ErrorAlmacen> {
    Dinero::nuevo(unidades, divisa).map_err(|e| ErrorAlmacen::Fallo(e.to_string()))
}

impl LecturaDePatrimonio for AlmacenSqlite<'_> {
    fn cuentas_de_patrimonio(&self) -> Result<Vec<CuentaDePatrimonio>, ErrorAlmacen> {
        let mut stmt = self
            .tx
            .prepare("SELECT nombre, divisa, balance_actual, es_caja_efectivo, archivada FROM cuentas_ahorro ORDER BY id;")
            .map_err(fallo)?;
        let filas = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, f64>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?)))
            .map_err(fallo)?;
        let mut salida = Vec::new();
        for fila in filas {
            let (nombre, divisa, saldo, caja, archivada) = fila.map_err(fallo)?;
            // Igual que el resto del sistema: solo se distingue «USD»; cualquier otro texto es pesos.
            let divisa = if divisa == "USD" { Divisa::Usd } else { Divisa::Dop };
            salida.push(CuentaDePatrimonio { nombre, saldo: dinero(saldo, divisa)?, es_efectivo: caja == 1, archivada: archivada == 1 });
        }
        Ok(salida)
    }

    fn tarjetas_de_patrimonio(&self) -> Result<Vec<TarjetaDePatrimonio>, ErrorAlmacen> {
        let mut stmt = self
            .tx
            .prepare("SELECT entidad, nombre_tarjeta, activa, balance_pesos, balance_dolares FROM tarjetas ORDER BY id;")
            .map_err(fallo)?;
        let filas = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, f64>(3)?, r.get::<_, f64>(4)?)))
            .map_err(fallo)?;
        let mut salida = Vec::new();
        for fila in filas {
            let (entidad, nombre, activa, pesos, dolares) = fila.map_err(fallo)?;
            salida.push(TarjetaDePatrimonio {
                nombre: format!("{entidad} {nombre}"),
                activa: activa == 1,
                balance_dop: dinero(pesos, Divisa::Dop)?,
                balance_usd: dinero(dolares, Divisa::Usd)?,
            });
        }
        Ok(salida)
    }

    fn prestamos_de_patrimonio(&self) -> Result<Vec<PrestamoDePatrimonio>, ErrorAlmacen> {
        let mut stmt = self
            .tx
            .prepare("SELECT institucion_financiera, tipo_prestamo, monto_prestamo, saldo_actual FROM prestamos ORDER BY id;")
            .map_err(fallo)?;
        let filas = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, f64>(2)?, r.get::<_, Option<f64>>(3)?)))
            .map_err(fallo)?;
        let mut salida = Vec::new();
        for fila in filas {
            let (institucion, tipo, monto, saldo) = fila.map_err(fallo)?;
            // Un registro antiguo sin saldo llevado cuenta por su monto, como en el listado de préstamos.
            salida.push(PrestamoDePatrimonio { nombre: format!("{institucion} ({tipo})"), saldo: dinero(saldo.unwrap_or(monto), Divisa::Dop)? });
        }
        Ok(salida)
    }
}
