//! Implementación en memoria de los puertos de persistencia, para pruebas.
//!
//! Permite ejercitar los casos de uso sin SQLite y sin tocar disco: las
//! pruebas pasan de decenas de milisegundos a microsegundos, y dejan de
//! depender del mutex global que hoy serializa las de caracterización.
//!
//! Incluye un interruptor de fallo por repositorio para poder comprobar que
//! una operación a medias no deja saldos movidos.

#![cfg(test)]

use super::repositorios::*;
use crate::dominio::conversion::EstadoConversion;
use crate::dominio::dinero::{Dinero, Divisa};
use crate::dominio::tarjeta::PoliticaLiquidacion;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct CuentaEnMemoria {
    pub nombre: String,
    pub saldo: Dinero,
}

#[derive(Default)]
pub struct AlmacenEnMemoria {
    pub categorias: HashMap<i64, String>,
    pub cuentas: HashMap<i64, CuentaEnMemoria>,
    /// Una deuda por tarjeta Y divisa, igual que las columnas separadas
    /// balance_pesos y balance_dolares del esquema real. Con una sola deuda
    /// por tarjeta el doble no podría representar el traslado entre divisas
    /// que ocurre al liquidar.
    pub deudas: HashMap<(i64, Divisa), Dinero>,
    pub tarjetas: std::collections::HashSet<i64>,
    pub politicas: HashMap<i64, PoliticaLiquidacion>,
    pub gastos: HashMap<i64, GastoGuardado>,
    siguiente_id: i64,
    /// Cuando está activo, `insertar` falla. Sirve para provocar un fallo
    /// después de que los saldos ya se hayan movido.
    pub falla_al_insertar: bool,
}

impl AlmacenEnMemoria {
    pub fn nuevo() -> Self {
        AlmacenEnMemoria { siguiente_id: 1, ..Default::default() }
    }

    pub fn con_categoria(mut self, id: i64, nombre: &str) -> Self {
        self.categorias.insert(id, nombre.to_string());
        self
    }

    pub fn con_cuenta(mut self, id: i64, nombre: &str, saldo: Dinero) -> Self {
        self.cuentas.insert(id, CuentaEnMemoria { nombre: nombre.to_string(), saldo });
        self
    }

    pub fn con_tarjeta(mut self, id: i64, deuda: Dinero) -> Self {
        self.tarjetas.insert(id);
        self.deudas.insert((id, deuda.divisa()), deuda);
        self
    }

    pub fn con_politica(mut self, id: i64, politica: PoliticaLiquidacion) -> Self {
        self.politicas.insert(id, politica);
        self
    }

    pub fn saldo_de(&self, cuenta_id: i64) -> Dinero {
        self.cuentas.get(&cuenta_id).map(|c| c.saldo).expect("cuenta de prueba inexistente")
    }

    pub fn saldo_de_caja(&self, nombre: &str) -> Option<Dinero> {
        self.cuentas.values().find(|c| c.nombre == nombre).map(|c| c.saldo)
    }

    /// Deuda en moneda local, que es la que consultan la mayoría de pruebas.
    pub fn deuda_de(&self, tarjeta_id: i64) -> Dinero {
        self.deuda_en(tarjeta_id, Divisa::Dop)
    }

    pub fn deuda_en(&self, tarjeta_id: i64, divisa: Divisa) -> Dinero {
        *self.deudas.get(&(tarjeta_id, divisa)).unwrap_or(&Dinero::cero(divisa))
    }

    pub fn total_gastos(&self) -> usize {
        self.gastos.len()
    }
}

impl RepositorioCategorias for AlmacenEnMemoria {
    fn nombre(&self, categoria_id: i64) -> Result<Option<String>, ErrorAlmacen> {
        Ok(self.categorias.get(&categoria_id).cloned())
    }
}

impl RepositorioGastos for AlmacenEnMemoria {
    fn insertar(&mut self, gasto: &GastoAPersistir) -> Result<i64, ErrorAlmacen> {
        if self.falla_al_insertar {
            return Err(ErrorAlmacen::Fallo("fallo provocado en la prueba".into()));
        }
        let id = self.siguiente_id;
        self.siguiente_id += 1;
        self.gastos.insert(
            id,
            GastoGuardado {
                id,
                monto: gasto.monto,
                metodo_pago: gasto.metodo_pago.clone(),
                cargos: gasto.cargos,
                tarjeta_id: gasto.tarjeta_id,
                cuenta_ahorro_id: gasto.cuenta_ahorro_id,
                estado_conversion: gasto.estado_conversion,
            },
        );
        Ok(id)
    }

    fn obtener(&self, gasto_id: i64) -> Result<GastoGuardado, ErrorAlmacen> {
        self.gastos
            .get(&gasto_id)
            .cloned()
            .ok_or(ErrorAlmacen::NoEncontrado { entidad: "gasto", id: gasto_id })
    }

    fn liquidar(
        &mut self,
        gasto_id: i64,
        estado: EstadoConversion,
    ) -> Result<(), ErrorAlmacen> {
        let g = self
            .gastos
            .get_mut(&gasto_id)
            .ok_or(ErrorAlmacen::NoEncontrado { entidad: "gasto", id: gasto_id })?;
        g.estado_conversion = estado;
        Ok(())
    }

    fn eliminar(&mut self, gasto_id: i64) -> Result<(), ErrorAlmacen> {
        self.gastos
            .remove(&gasto_id)
            .map(|_| ())
            .ok_or(ErrorAlmacen::NoEncontrado { entidad: "gasto", id: gasto_id })
    }
}

impl RepositorioTarjetas for AlmacenEnMemoria {
    fn ajustar_deuda(&mut self, tarjeta_id: i64, delta: Dinero) -> Result<(), ErrorAlmacen> {
        if !self.tarjetas.contains(&tarjeta_id) {
            return Err(ErrorAlmacen::NoEncontrado { entidad: "tarjeta", id: tarjeta_id });
        }
        let clave = (tarjeta_id, delta.divisa());
        let actual = self.deudas.entry(clave).or_insert_with(|| Dinero::cero(delta.divisa()));
        *actual = actual.sumar(&delta).map_err(|e| ErrorAlmacen::Fallo(e.to_string()))?;
        Ok(())
    }

    fn politica(&self, tarjeta_id: i64) -> Result<PoliticaLiquidacion, ErrorAlmacen> {
        if !self.tarjetas.contains(&tarjeta_id) {
            return Err(ErrorAlmacen::NoEncontrado { entidad: "tarjeta", id: tarjeta_id });
        }
        Ok(*self
            .politicas
            .get(&tarjeta_id)
            .unwrap_or(&PoliticaLiquidacion::EnDivisaDeOrigen))
    }

    fn reducir_deuda_con_recorte(
        &mut self,
        tarjeta_id: i64,
        monto: Dinero,
    ) -> Result<(), ErrorAlmacen> {
        if !self.tarjetas.contains(&tarjeta_id) {
            return Err(ErrorAlmacen::NoEncontrado { entidad: "tarjeta", id: tarjeta_id });
        }
        let clave = (tarjeta_id, monto.divisa());
        let actual = self.deudas.entry(clave).or_insert_with(|| Dinero::cero(monto.divisa()));
        let restado = actual.restar(&monto).map_err(|e| ErrorAlmacen::Fallo(e.to_string()))?;
        // Réplica exacta del MAX(0.0, ...) de SQL: la diferencia se pierde.
        *actual = if restado.es_negativo() { Dinero::cero(restado.divisa()) } else { restado };
        Ok(())
    }
}

impl RepositorioCuentas for AlmacenEnMemoria {
    fn ajustar_saldo(&mut self, cuenta_id: i64, delta: Dinero) -> Result<(), ErrorAlmacen> {
        let cuenta = self
            .cuentas
            .get_mut(&cuenta_id)
            .ok_or(ErrorAlmacen::NoEncontrado { entidad: "cuenta", id: cuenta_id })?;
        cuenta.saldo =
            cuenta.saldo.sumar(&delta).map_err(|e| ErrorAlmacen::Fallo(e.to_string()))?;
        Ok(())
    }

    fn ajustar_saldo_de_caja(
        &mut self,
        nombre: &str,
        delta: Dinero,
    ) -> Result<(), ErrorAlmacen> {
        // Réplica de la conducta vigente (H3): si la caja no existe, la
        // operación no falla y ningún saldo se mueve.
        if let Some(c) = self.cuentas.values_mut().find(|c| c.nombre == nombre) {
            c.saldo = c.saldo.sumar(&delta).map_err(|e| ErrorAlmacen::Fallo(e.to_string()))?;
        }
        Ok(())
    }

    fn saldo(&self, cuenta_id: i64) -> Result<Dinero, ErrorAlmacen> {
        self.cuentas
            .get(&cuenta_id)
            .map(|c| c.saldo)
            .ok_or(ErrorAlmacen::NoEncontrado { entidad: "cuenta", id: cuenta_id })
    }

    fn divisa(&self, cuenta_id: i64) -> Result<Divisa, ErrorAlmacen> {
        self.saldo(cuenta_id).map(|s| s.divisa())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }

    fn almacen() -> AlmacenEnMemoria {
        AlmacenEnMemoria::nuevo()
            .con_categoria(1, "Alimentación")
            .con_categoria(2, "Impuestos")
            .con_cuenta(10, "Cuenta Ahorros DOP", dop(100000.0))
            .con_cuenta(11, "Efectivo DOP", dop(5000.0))
            .con_tarjeta(20, dop(500.0))
    }

    // --- Contrato: categorías ---

    #[test]
    fn devuelve_el_nombre_de_una_categoria_existente() {
        assert_eq!(almacen().nombre(1).unwrap(), Some("Alimentación".to_string()));
    }

    #[test]
    fn una_categoria_inexistente_devuelve_none_y_no_error() {
        assert_eq!(almacen().nombre(999).unwrap(), None);
    }

    // --- Contrato: gastos ---

    #[test]
    fn lo_guardado_se_recupera_igual() {
        let mut a = almacen();
        let g = GastoAPersistir {
            fecha: "09/09/2026".into(),
            monto: dop(1000.0),
            descripcion: "Compra".into(),
            categoria_id: 1,
            metodo_pago: "transferencia".into(),
            cargos: dop(2.0),
            tarjeta_id: None,
            cuenta_ahorro_id: Some(10),
            estado_conversion: EstadoConversion::NoAplica,
        };
        let id = a.insertar(&g).unwrap();
        let leido = a.obtener(id).unwrap();

        assert_eq!(leido.monto, dop(1000.0));
        assert_eq!(leido.cargos, dop(2.0));
        assert_eq!(leido.cuenta_ahorro_id, Some(10));
        assert_eq!(leido.metodo_pago, "transferencia");
    }

    #[test]
    fn obtener_un_gasto_inexistente_es_error() {
        assert_eq!(
            almacen().obtener(404).unwrap_err(),
            ErrorAlmacen::NoEncontrado { entidad: "gasto", id: 404 }
        );
    }

    #[test]
    fn eliminar_retira_el_gasto_del_almacen() {
        let mut a = almacen();
        let g = GastoAPersistir {
            fecha: "09/09/2026".into(),
            monto: dop(1000.0),
            descripcion: "Compra".into(),
            categoria_id: 1,
            metodo_pago: "efectivo".into(),
            cargos: dop(0.0),
            tarjeta_id: None,
            cuenta_ahorro_id: None,
            estado_conversion: EstadoConversion::NoAplica,
        };
        let id = a.insertar(&g).unwrap();
        assert_eq!(a.total_gastos(), 1);
        a.eliminar(id).unwrap();
        assert_eq!(a.total_gastos(), 0);
        assert!(a.eliminar(id).is_err(), "eliminar dos veces es error");
    }

    // --- Contrato: saldos ---

    #[test]
    fn ajustar_saldo_admite_deltas_en_ambos_sentidos() {
        let mut a = almacen();
        a.ajustar_saldo(10, dop(-10020.0)).unwrap();
        assert_eq!(a.saldo_de(10), dop(89980.0));
        a.ajustar_saldo(10, dop(10020.0)).unwrap();
        assert_eq!(a.saldo_de(10), dop(100000.0), "restituye el importe exacto");
    }

    #[test]
    fn ajustar_una_cuenta_inexistente_es_error() {
        assert!(almacen().ajustar_saldo(999, dop(-1.0)).is_err());
    }

    #[test]
    fn ajustar_deuda_de_tarjeta_suma_y_resta() {
        let mut a = almacen();
        a.ajustar_deuda(20, dop(150.0)).unwrap();
        assert_eq!(a.deuda_de(20), dop(650.0));
        a.ajustar_deuda(20, dop(-150.0)).unwrap();
        assert_eq!(a.deuda_de(20), dop(500.0));
    }

    #[test]
    fn h3_ajustar_una_caja_inexistente_no_falla_ni_mueve_nada() {
        let mut a = almacen();
        let antes = a.saldo_de(11);
        a.ajustar_saldo_de_caja("Caja Que No Existe", dop(-1200.0)).unwrap();
        assert_eq!(a.saldo_de(11), antes, "ninguna caja se movió");
    }

    #[test]
    fn ajustar_la_caja_por_nombre_localiza_la_cuenta_correcta() {
        let mut a = almacen();
        a.ajustar_saldo_de_caja("Efectivo DOP", dop(-1200.0)).unwrap();
        assert_eq!(a.saldo_de_caja("Efectivo DOP").unwrap(), dop(3800.0));
        assert_eq!(a.saldo_de(10), dop(100000.0), "la otra cuenta no se toca");
    }

    #[test]
    fn mezclar_divisas_al_ajustar_un_saldo_es_error() {
        let mut a = almacen();
        let usd = Dinero::nuevo(50.0, Divisa::Usd).unwrap();
        assert!(a.ajustar_saldo(10, usd).is_err(), "la cuenta es en pesos");
    }

    // --- Interruptor de fallo ---

    #[test]
    fn el_interruptor_hace_fallar_la_insercion() {
        let mut a = almacen();
        a.falla_al_insertar = true;
        let g = GastoAPersistir {
            fecha: "09/09/2026".into(),
            monto: dop(1000.0),
            descripcion: "Compra".into(),
            categoria_id: 1,
            metodo_pago: "efectivo".into(),
            cargos: dop(0.0),
            tarjeta_id: None,
            cuenta_ahorro_id: None,
            estado_conversion: EstadoConversion::NoAplica,
        };
        assert!(a.insertar(&g).is_err());
        assert_eq!(a.total_gastos(), 0);
    }
}

#[cfg(test)]
mod contrato_del_doble {
    use super::*;
    use crate::puertos::contrato::{verificar, Semilla};

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }

    /// El doble se somete a la MISMA suite que el adaptador SQLite. Si ambos
    /// la pasan, son intercambiables allí donde se espere un AlmacenGastos.
    #[test]
    fn el_doble_en_memoria_satisface_el_contrato() {
        let mut a = AlmacenEnMemoria::nuevo()
            .con_categoria(1, "Alimentación")
            .con_cuenta(10, "Cuenta Contrato", dop(100000.0))
            .con_cuenta(11, "Efectivo DOP", dop(0.0))
            .con_tarjeta(20, dop(500.0));

        let semilla = Semilla {
            categoria_id: 1,
            categoria_nombre: "Alimentación".into(),
            cuenta_id: 10,
            cuenta_saldo: dop(100000.0),
            caja_nombre: "Efectivo DOP".into(),
            caja_saldo: dop(0.0),
            tarjeta_id: 20,
            tarjeta_deuda: dop(500.0),
        };

        verificar(&mut a, &semilla, "en memoria");
    }
}
