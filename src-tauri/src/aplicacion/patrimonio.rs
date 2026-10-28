//! Caso de uso: el patrimonio, con sus dos cifras. **Solo lectura.**
//!
//! Junta el capital (ya sumado por `guardar_capital::Totales`, sin duplicar su suma) con lo que el almacén lee de
//! cuentas, tarjetas y préstamos, y delega la composición en `dominio::patrimonio::componer`. El almacén entra por
//! referencia compartida: el compilador impide cualquier escritura.

use super::guardar_capital::Totales;
use super::ErrorAplicacion;
use crate::dominio::dinero::TasaCambio;
use crate::dominio::patrimonio::{componer, EntradasDePatrimonio, Foto};
use crate::puertos::repositorios::LecturaDePatrimonio;

pub fn obtener_patrimonio(
    capital: &Totales,
    tasa: TasaCambio,
    almacen: &impl LecturaDePatrimonio,
) -> Result<Foto, ErrorAplicacion> {
    let entradas = EntradasDePatrimonio {
        capital: capital.patrimonio(),
        cuentas: almacen.cuentas_de_patrimonio()?,
        tarjetas: almacen.tarjetas_de_patrimonio()?,
        prestamos: almacen.prestamos_de_patrimonio()?,
        tasa,
    };
    Ok(componer(&entradas)?)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::dominio::dinero::{Dinero, Divisa};
    use crate::dominio::patrimonio::{tasa_estimada, CuentaDePatrimonio, PrestamoDePatrimonio, TarjetaDePatrimonio};
    use crate::puertos::repositorios::ErrorAlmacen;
    use serde_json::json;

    struct Doble {
        cuentas: Vec<CuentaDePatrimonio>,
        tarjetas: Vec<TarjetaDePatrimonio>,
        prestamos: Vec<PrestamoDePatrimonio>,
    }

    impl LecturaDePatrimonio for Doble {
        fn cuentas_de_patrimonio(&self) -> Result<Vec<CuentaDePatrimonio>, ErrorAlmacen> {
            Ok(self.cuentas.clone())
        }
        fn tarjetas_de_patrimonio(&self) -> Result<Vec<TarjetaDePatrimonio>, ErrorAlmacen> {
            Ok(self.tarjetas.clone())
        }
        fn prestamos_de_patrimonio(&self) -> Result<Vec<PrestamoDePatrimonio>, ErrorAlmacen> {
            Ok(self.prestamos.clone())
        }
    }

    fn dop(c: i64) -> Dinero {
        Dinero::de_centavos(c, Divisa::Dop)
    }

    #[test]
    fn reutiliza_el_total_del_capital_y_compone_las_dos_cifras() {
        let capital = crate::aplicacion::guardar_capital::totales_de(&json!({
            "certificados": [{"monto": 100_000.07}],
            "bolsa": [{"monto": 20_000.13}],
            "propiedades": {"inmobiliario": [{"valor_estimado": 3_000_000.01}], "vehiculos": [], "maquinaria": []}
        }));
        let almacen = Doble {
            cuentas: vec![CuentaDePatrimonio { nombre: "Ahorro".into(), saldo: dop(55_555), es_efectivo: false, archivada: false }],
            tarjetas: vec![TarjetaDePatrimonio {
                nombre: "T".into(),
                activa: true,
                balance_dop: dop(77_777),
                balance_usd: Dinero::de_centavos(1_001, Divisa::Usd),
            }],
            prestamos: vec![PrestamoDePatrimonio { nombre: "P".into(), saldo: dop(1_234_567) }],
        };
        let f = obtener_patrimonio(&capital, tasa_estimada(), &almacen).unwrap();
        assert_eq!(f.capital, capital.patrimonio());
        assert_eq!(f.capital, dop(312_000_021));
        let pasivos = 77_777 + 60_060 + 1_234_567;
        assert_eq!(f.neto_actual, dop(312_000_021 - pasivos));
        assert_eq!(f.neto_con_liquidez, dop(312_000_021 - pasivos + 55_555));
    }
}
