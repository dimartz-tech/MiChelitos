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
    /// Tarifa fija por pago de impuestos. `None` = ninguna pactada.
    pub comision_impuestos: Option<Dinero>,
}

/// Una factura en el doble: lo que guarda la fila.
#[derive(Debug, Clone)]
pub struct FacturaEnMemoria {
    pub id: i64,
    pub numero_factura: String,
    pub cliente_id: i64,
    pub fecha_emision: String,
    pub monto_total: f64,
    pub porcentaje_retencion: f64,
    pub monto_retenido: f64,
    pub estatus: String,
    pub cuenta_ahorro_id: Option<i64>,
    pub institucion_deposito: Option<String>,
    pub fecha_pago: Option<String>,
    pub monto_recibido: Option<f64>,
}

/// Un ingreso informal en el doble: lo que guarda la fila.
#[derive(Debug, Clone)]
pub struct InformalEnMemoria {
    pub id: i64,
    pub fecha: String,
    pub descripcion: String,
    pub monto: f64,
    pub estatus: String,
    pub cuenta_ahorro_id: Option<i64>,
    pub institucion_deposito: Option<String>,
    pub fecha_pago: Option<String>,
    pub monto_recibido: Option<f64>,
}

/// Una suscripción en el doble: la fila (con la fecha del último pago y la tarjeta como identificador).
#[derive(Debug, Clone)]
pub struct SuscripcionEnMemoria {
    pub datos: SuscripcionRegistrada,
    pub fecha_ultimo_pago: Option<String>,
}

#[derive(Default)]
pub struct AlmacenEnMemoria {
    pub suscripciones: Vec<SuscripcionEnMemoria>,
    /// La descripción de cada gasto y el motivo por el que no se borra solo (el doble de `GastoGuardado` no los guarda).
    pub descripciones_de_gasto: HashMap<i64, String>,
    pub motivos_de_no_borrar: HashMap<i64, String>,
    /// La fecha de cada avance (el doble de `AvanceGuardado` no la guarda).
    pub fechas_de_avance: HashMap<i64, String>,
    /// La fecha de cada abono (el doble de `PagoGuardado` no la guarda).
    pub fechas_de_pago: HashMap<i64, String>,
    pub informales: Vec<InformalEnMemoria>,
    pub facturas: Vec<FacturaEnMemoria>,
    pub casos: Vec<CasoAAnotar>,
    pub clientes_de_facturas: Vec<ClienteGuardado>,
    pub categorias: HashMap<i64, String>,
    pub cuentas: HashMap<i64, CuentaEnMemoria>,
    /// Una deuda por tarjeta Y divisa, igual que las columnas separadas
    /// balance_pesos y balance_dolares del esquema real. Con una sola deuda
    /// por tarjeta el doble no podría representar el traslado entre divisas
    /// que ocurre al liquidar.
    pub deudas: HashMap<(i64, Divisa), Dinero>,
    /// Qué cuenta hace de caja de efectivo en cada divisa. En el esquema real
    /// es la columna `es_caja_efectivo`; aquí basta el índice.
    pub cajas: HashMap<Divisa, i64>,
    pub tarjetas: std::collections::HashSet<i64>,
    pub politicas: HashMap<i64, PoliticaLiquidacion>,
    pub gastos: HashMap<i64, GastoGuardado>,
    siguiente_id: i64,
    /// Cuando está activo, `insertar` falla. Sirve para provocar un fallo
    /// después de que los saldos ya se hayan movido.
    pub bonificaciones: HashMap<i64, BonificacionGuardada>,
    pub transferencias: HashMap<i64, TransferenciaGuardada>,
    pub pagos: HashMap<i64, PagoGuardado>,
    pub avances: HashMap<i64, AvanceGuardado>,
    /// Cuentas en las que se cobró una factura. El doble no modela ingresos;
    /// basta saber a qué cuenta apuntan para ejercitar la guarda de borrado.
    pub ingresos_en_cuenta: Vec<i64>,
    pub informales_en_cuenta: Vec<i64>,
    pub falla_al_insertar: bool,
    /// Cuando está activo, `ajustar_deuda` falla. Permite comprobar que un
    /// fallo a mitad de operación no deja saldos alterados.
    pub falla_al_ajustar_deuda: bool,
}

impl AlmacenEnMemoria {
    pub fn nuevo() -> Self {
        AlmacenEnMemoria { siguiente_id: 1, ..Default::default() }
    }

    /// Siembra un abono ya registrado, para poder revertirlo. Devuelve su id.
    ///
    /// No mueve saldos: el abono ya ocurrió, y la prueba coloca por separado
    /// el estado que dejó. Así se distingue lo que la reversión deshace de lo
    /// que la siembra dio por hecho.
    pub fn con_pago(
        &mut self,
        tarjeta_id: i64,
        monto: Dinero,
        cuenta_ahorro_id: Option<i64>,
        tasa_cambio: Option<f64>,
        gasto_comision_id: Option<i64>,
    ) -> i64 {
        // Sin conversión, lo debitado es el propio importe; la comisión se
        // siembra al 0.20 %, que es lo que el registro habría guardado.
        let debitado = cuenta_ahorro_id.map(|_| monto);
        let comision = debitado.map(|d| {
            d.porcentaje(crate::dominio::cargos::TASA_RETENCION).expect("comisión sembrada")
        });
        self.con_pago_detallado(
            tarjeta_id, monto, cuenta_ahorro_id, tasa_cambio, debitado, comision,
            gasto_comision_id,
        )
    }

    /// Siembra un abono diciendo exactamente qué debitó y qué cobró.
    ///
    /// Permite construir el caso que motivó el tramo 2: un abono cuyo importe
    /// guardado **no** coincide con lo que hoy se recalcularía.
    #[allow(clippy::too_many_arguments)]
    pub fn con_pago_detallado(
        &mut self,
        tarjeta_id: i64,
        monto: Dinero,
        cuenta_ahorro_id: Option<i64>,
        tasa_cambio: Option<f64>,
        monto_debitado: Option<Dinero>,
        comision: Option<Dinero>,
        gasto_comision_id: Option<i64>,
    ) -> i64 {
        let id = self.siguiente_id;
        self.siguiente_id += 1;
        self.pagos.insert(
            id,
            PagoGuardado {
                id,
                tarjeta_id,
                monto,
                cuenta_ahorro_id,
                tasa_cambio,
                monto_debitado,
                comision,
                gasto_comision_id,
            },
        );
        id
    }

    /// Un gasto sin más contexto que su importe, para lo que solo necesita
    /// comprobar que se borra.
    pub fn con_gasto_suelto(&mut self, monto: Dinero) -> i64 {
        let id = self.siguiente_id;
        self.siguiente_id += 1;
        self.gastos.insert(
            id,
            GastoGuardado {
                id,
                monto,
                metodo_pago: "transferencia".to_string(),
                cargos: Dinero::cero(monto.divisa()),
                tarjeta_id: None,
                cuenta_ahorro_id: None,
                estado_conversion: EstadoConversion::NoAplica,
            },
        );
        id
    }

    pub fn con_categoria(mut self, id: i64, nombre: &str) -> Self {
        self.categorias.insert(id, nombre.to_string());
        self
    }

    pub fn con_cuenta(mut self, id: i64, nombre: &str, saldo: Dinero) -> Self {
        self.cuentas.insert(
            id,
            CuentaEnMemoria { nombre: nombre.to_string(), saldo, comision_impuestos: None },
        );
        self
    }

    /// Cuenta cuya entidad cobra una tarifa fija por pagar impuestos.
    pub fn con_comision_de_impuestos(mut self, id: i64, tarifa: Dinero) -> Self {
        if let Some(c) = self.cuentas.get_mut(&id) {
            c.comision_impuestos = Some(tarifa);
        }
        self
    }

    /// Cuenta que además hace de caja de efectivo de su divisa.
    pub fn con_caja(mut self, id: i64, saldo: Dinero) -> Self {
        let divisa = saldo.divisa();
        self.cuentas.insert(
            id,
            CuentaEnMemoria {
                nombre: format!("Efectivo {}", divisa.codigo()),
                saldo,
                comision_impuestos: None,
            },
        );
        self.cajas.insert(divisa, id);
        self
    }

    pub fn con_tarjeta(mut self, id: i64, deuda: Dinero) -> Self {
        self.tarjetas.insert(id);
        self.deudas.insert((id, deuda.divisa()), deuda);
        self
    }

    /// Una factura cobrada en esa cuenta.
    pub fn con_factura_cobrada_en(mut self, cuenta_id: i64) -> Self {
        self.ingresos_en_cuenta.push(cuenta_id);
        self
    }

    /// Un ingreso informal cobrado en esa cuenta.
    pub fn con_informal_cobrado_en(mut self, cuenta_id: i64) -> Self {
        self.informales_en_cuenta.push(cuenta_id);
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

    pub fn total_bonificaciones(&self) -> usize {
        self.bonificaciones.len()
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

impl RepositorioPagosTarjeta for AlmacenEnMemoria {
    fn insertar_pago(&mut self, pago: &PagoAPersistir) -> Result<i64, ErrorAlmacen> {
        if !self.tarjetas.contains(&pago.tarjeta_id) {
            return Err(ErrorAlmacen::NoEncontrado { entidad: "tarjeta", id: pago.tarjeta_id });
        }
        let id = self.siguiente_id;
        self.siguiente_id += 1;
        self.pagos.insert(
            id,
            PagoGuardado {
                id,
                tarjeta_id: pago.tarjeta_id,
                monto: pago.monto,
                cuenta_ahorro_id: pago.cuenta_ahorro_id,
                tasa_cambio: pago.tasa_cambio,
                monto_debitado: pago.monto_debitado,
                comision: pago.comision,
                gasto_comision_id: pago.gasto_comision_id,
            },
        );
        Ok(id)
    }

    fn obtener_pago(&self, pago_id: i64) -> Result<PagoGuardado, ErrorAlmacen> {
        self.pagos
            .get(&pago_id)
            .cloned()
            .ok_or(ErrorAlmacen::NoEncontrado { entidad: "abono", id: pago_id })
    }

    fn eliminar_pago(&mut self, pago_id: i64) -> Result<(), ErrorAlmacen> {
        self.pagos
            .remove(&pago_id)
            .map(|_| ())
            .ok_or(ErrorAlmacen::NoEncontrado { entidad: "abono", id: pago_id })
    }
}

impl RepositorioAvances for AlmacenEnMemoria {
    fn insertar_avance(&mut self, a: &AvanceAPersistir) -> Result<i64, ErrorAlmacen> {
        if !self.tarjetas.contains(&a.tarjeta_id) {
            return Err(ErrorAlmacen::NoEncontrado { entidad: "tarjeta", id: a.tarjeta_id });
        }
        if !self.cuentas.contains_key(&a.cuenta_ahorro_id) {
            return Err(ErrorAlmacen::NoEncontrado { entidad: "cuenta", id: a.cuenta_ahorro_id });
        }
        let id = self.siguiente_id;
        self.siguiente_id += 1;
        self.avances.insert(
            id,
            AvanceGuardado {
                id,
                tarjeta_id: a.tarjeta_id,
                cuenta_ahorro_id: a.cuenta_ahorro_id,
                monto: a.monto,
                cargo: a.cargo,
                gasto_cargo_id: a.gasto_cargo_id,
            },
        );
        Ok(id)
    }

    fn obtener_avance(&self, avance_id: i64) -> Result<AvanceGuardado, ErrorAlmacen> {
        self.avances
            .get(&avance_id)
            .cloned()
            .ok_or(ErrorAlmacen::NoEncontrado { entidad: "avance", id: avance_id })
    }

    fn eliminar_avance(&mut self, avance_id: i64) -> Result<(), ErrorAlmacen> {
        self.avances
            .remove(&avance_id)
            .map(|_| ())
            .ok_or(ErrorAlmacen::NoEncontrado { entidad: "avance", id: avance_id })
    }
}

impl RepositorioBonificaciones for AlmacenEnMemoria {
    fn insertar_bonificacion(&mut self, b: &BonificacionAPersistir) -> Result<i64, ErrorAlmacen> {
        if !self.tarjetas.contains(&b.tarjeta_id) {
            return Err(ErrorAlmacen::NoEncontrado { entidad: "tarjeta", id: b.tarjeta_id });
        }
        let id = self.siguiente_id;
        self.siguiente_id += 1;
        self.bonificaciones.insert(
            id,
            BonificacionGuardada {
                id,
                tarjeta_id: b.tarjeta_id,
                bonificacion: b.bonificacion.clone(),
            },
        );
        Ok(id)
    }

    fn obtener_bonificacion(&self, id: i64) -> Result<BonificacionGuardada, ErrorAlmacen> {
        self.bonificaciones
            .get(&id)
            .cloned()
            .ok_or(ErrorAlmacen::NoEncontrado { entidad: "bonificación", id })
    }

    fn eliminar_bonificacion(&mut self, id: i64) -> Result<(), ErrorAlmacen> {
        self.bonificaciones
            .remove(&id)
            .map(|_| ())
            .ok_or(ErrorAlmacen::NoEncontrado { entidad: "bonificación", id })
    }
}

impl RepositorioTarjetas for AlmacenEnMemoria {
    fn ajustar_deuda(&mut self, tarjeta_id: i64, delta: Dinero) -> Result<(), ErrorAlmacen> {
        if self.falla_al_ajustar_deuda {
            return Err(ErrorAlmacen::Fallo("fallo simulado al guardar la tarjeta".into()));
        }
        if !self.tarjetas.contains(&tarjeta_id) {
            return Err(ErrorAlmacen::NoEncontrado { entidad: "tarjeta", id: tarjeta_id });
        }
        let clave = (tarjeta_id, delta.divisa());
        let actual = self.deudas.entry(clave).or_insert_with(|| Dinero::cero(delta.divisa()));
        *actual = actual.sumar(&delta).map_err(|e| ErrorAlmacen::Fallo(e.to_string()))?;
        Ok(())
    }

    fn deuda(&self, tarjeta_id: i64, divisa: Divisa) -> Result<Dinero, ErrorAlmacen> {
        if !self.tarjetas.contains(&tarjeta_id) {
            return Err(ErrorAlmacen::NoEncontrado { entidad: "tarjeta", id: tarjeta_id });
        }
        Ok(self.deuda_en(tarjeta_id, divisa))
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

    fn caja(&self, divisa: Divisa) -> Result<i64, ErrorAlmacen> {
        self.cajas
            .get(&divisa)
            .copied()
            .ok_or(ErrorAlmacen::CajaDeEfectivoAusente { divisa })
    }

    fn es_caja(&self, cuenta_id: i64) -> Result<bool, ErrorAlmacen> {
        if !self.cuentas.contains_key(&cuenta_id) {
            return Err(ErrorAlmacen::NoEncontrado { entidad: "cuenta", id: cuenta_id });
        }
        Ok(self.cajas.values().any(|&id| id == cuenta_id))
    }

    fn referencias_a_la_cuenta(&self, cuenta_id: i64) -> Result<Vec<ReferenciaACuenta>, ErrorAlmacen> {
        if !self.cuentas.contains_key(&cuenta_id) {
            return Err(ErrorAlmacen::NoEncontrado { entidad: "cuenta", id: cuenta_id });
        }
        let contar = |ids: &[i64]| ids.iter().filter(|&&c| c == cuenta_id).count() as i64;
        let cuentas_de_gastos: Vec<i64> =
            self.gastos.values().filter_map(|g| g.cuenta_ahorro_id).collect();
        let origenes: Vec<i64> = self.transferencias.values().map(|t| t.origen_id).collect();
        let destinos: Vec<i64> = self.transferencias.values().map(|t| t.destino_id).collect();
        let pagos: Vec<i64> = self.pagos.values().filter_map(|p| p.cuenta_ahorro_id).collect();
        let avances: Vec<i64> = self.avances.values().map(|a| a.cuenta_ahorro_id).collect();

        // El mismo orden y los mismos nombres que `RELACIONES_CON_CUENTAS`.
        let todas: [(&'static str, &'static str, i64); 7] = [
            ("gastos", "cuenta_ahorro_id", contar(&cuentas_de_gastos)),
            ("transacciones_cuentas", "cuenta_origen_id", contar(&origenes)),
            ("transacciones_cuentas", "cuenta_destino_id", contar(&destinos)),
            ("ingresos", "cuenta_ahorro_id", contar(&self.ingresos_en_cuenta)),
            ("ingresos_informales", "cuenta_ahorro_id", contar(&self.informales_en_cuenta)),
            ("pagos_tarjeta", "cuenta_ahorro_id", contar(&pagos)),
            ("avances_efectivo", "cuenta_ahorro_id", contar(&avances)),
        ];
        Ok(todas
            .into_iter()
            .filter(|(_, _, n)| *n > 0)
            .map(|(tabla, columna, cantidad)| ReferenciaACuenta { tabla, columna, cantidad })
            .collect())
    }

    fn eliminar_cuenta(&mut self, cuenta_id: i64) -> Result<(), ErrorAlmacen> {
        self.cuentas
            .remove(&cuenta_id)
            .map(|_| ())
            .ok_or(ErrorAlmacen::NoEncontrado { entidad: "cuenta", id: cuenta_id })
    }

    fn comision_pago_impuestos(&self, cuenta_id: i64) -> Result<Option<Dinero>, ErrorAlmacen> {
        self.cuentas
            .get(&cuenta_id)
            .map(|c| c.comision_impuestos)
            .ok_or(ErrorAlmacen::NoEncontrado { entidad: "cuenta", id: cuenta_id })
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
            .con_caja(11, dop(5000.0))
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
    fn pedir_una_caja_que_no_existe_es_error() {
        // Antes (H3) esto era un Ok que no movía nada.
        let a = almacen();
        assert!(matches!(
            a.caja(Divisa::Usd),
            Err(ErrorAlmacen::CajaDeEfectivoAusente { divisa: Divisa::Usd })
        ));
    }

    #[test]
    fn la_caja_se_localiza_por_su_papel_y_no_por_su_nombre() {
        let mut a = almacen();
        let caja = a.caja(Divisa::Dop).unwrap();
        assert_eq!(caja, 11);

        a.ajustar_saldo(caja, dop(-1200.0)).unwrap();
        assert_eq!(a.saldo_de(11), dop(3800.0));
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
            .con_caja(11, dop(0.0))
            .con_tarjeta(20, dop(500.0));

        let semilla = Semilla {
            categoria_id: 1,
            categoria_nombre: "Alimentación".into(),
            cuenta_id: 10,
            cuenta_saldo: dop(100000.0),
            caja_id: 11,
            caja_saldo: dop(0.0),
            tarjeta_id: 20,
            tarjeta_deuda: dop(500.0),
        };

        verificar(&mut a, &semilla, "en memoria");
    }
}

impl RepositorioTransferencias for AlmacenEnMemoria {
    fn insertar_transferencia(
        &mut self,
        datos: TransferenciaAPersistir,
    ) -> Result<i64, ErrorAlmacen> {
        if self.falla_al_insertar {
            return Err(ErrorAlmacen::Fallo("fallo simulado al insertar".into()));
        }
        let id = self.siguiente_id;
        self.siguiente_id += 1;
        self.transferencias.insert(
            id,
            TransferenciaGuardada {
                id,
                fecha: datos.fecha,
                origen_id: datos.origen_id,
                destino_id: datos.destino_id,
                monto_origen: datos.monto_origen,
                monto_destino: datos.monto_destino,
                cargo: datos.cargo,
                descripcion: datos.descripcion,
            },
        );
        Ok(id)
    }

    fn obtener_transferencia(&self, id: i64) -> Result<TransferenciaGuardada, ErrorAlmacen> {
        self.transferencias
            .get(&id)
            .cloned()
            .ok_or(ErrorAlmacen::NoEncontrado { entidad: "transferencia", id })
    }

    fn eliminar_transferencia(&mut self, id: i64) -> Result<(), ErrorAlmacen> {
        self.transferencias
            .remove(&id)
            .map(|_| ())
            .ok_or(ErrorAlmacen::NoEncontrado { entidad: "transferencia", id })
    }
}

/// Doble en memoria del puerto de catálogos.
///
/// Guarda lo mismo que las tablas: categorías con su nombre, clientes con RNC y nombre, y, para ejercitar
/// las guardas de borrado, cuántos gastos usan cada categoría y cuántas facturas tiene cada cliente.
#[derive(Default)]
pub struct CatalogosEnMemoria {
    pub categorias: Vec<CategoriaGuardada>,
    pub clientes: Vec<ClienteGuardado>,
    pub gastos_por_categoria: HashMap<i64, i64>,
    pub facturas_por_cliente: HashMap<i64, i64>,
    siguiente_id: i64,
}

impl CatalogosEnMemoria {
    pub fn nuevo() -> Self {
        CatalogosEnMemoria { siguiente_id: 1, ..Default::default() }
    }
    pub fn con_categoria(mut self, nombre: &str) -> Self {
        let id = self.siguiente_id;
        self.siguiente_id += 1;
        self.categorias.push(CategoriaGuardada { id, nombre: nombre.to_string() });
        self
    }
    pub fn id_de_categoria(&self, nombre: &str) -> i64 {
        self.categorias.iter().find(|c| c.nombre == nombre).expect("categoría sembrada").id
    }
}

impl AlmacenCatalogos for CatalogosEnMemoria {
    fn categorias(&self) -> Result<Vec<CategoriaGuardada>, ErrorAlmacen> {
        let mut v = self.categorias.clone();
        v.sort_by(|a, b| a.nombre.cmp(&b.nombre));
        Ok(v)
    }
    fn categoria_existe(&self, nombre: &str) -> Result<bool, ErrorAlmacen> {
        Ok(self.categorias.iter().any(|c| c.nombre.to_lowercase() == nombre.to_lowercase()))
    }
    fn insertar_categoria(&mut self, nombre: &str) -> Result<i64, ErrorAlmacen> {
        let id = self.siguiente_id;
        self.siguiente_id += 1;
        self.categorias.push(CategoriaGuardada { id, nombre: nombre.to_string() });
        Ok(id)
    }
    fn nombre_de_categoria(&self, id: i64) -> Result<String, ErrorAlmacen> {
        self.categorias
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.nombre.clone())
            .ok_or(ErrorAlmacen::NoEncontrado { entidad: "la categoría", id })
    }
    fn gastos_de_categoria(&self, id: i64) -> Result<i64, ErrorAlmacen> {
        Ok(*self.gastos_por_categoria.get(&id).unwrap_or(&0))
    }
    fn eliminar_categoria(&mut self, id: i64) -> Result<(), ErrorAlmacen> {
        self.categorias.retain(|c| c.id != id);
        Ok(())
    }
    fn clientes(&self) -> Result<Vec<ClienteGuardado>, ErrorAlmacen> {
        let mut v = self.clientes.clone();
        v.sort_by(|a, b| a.nombre.cmp(&b.nombre));
        Ok(v)
    }
    fn rnc_registrado(&self, rnc: &str) -> Result<bool, ErrorAlmacen> {
        Ok(self.clientes.iter().any(|c| c.rnc == rnc))
    }
    fn insertar_cliente(&mut self, rnc: &str, nombre: &str) -> Result<i64, ErrorAlmacen> {
        let id = self.siguiente_id;
        self.siguiente_id += 1;
        self.clientes.push(ClienteGuardado { id, rnc: rnc.to_string(), nombre: nombre.to_string() });
        Ok(id)
    }
    fn facturas_de_cliente(&self, id: i64) -> Result<i64, ErrorAlmacen> {
        Ok(*self.facturas_por_cliente.get(&id).unwrap_or(&0))
    }
    fn eliminar_cliente(&mut self, id: i64) -> Result<(), ErrorAlmacen> {
        self.clientes.retain(|c| c.id != id);
        Ok(())
    }
}

/// Doble en memoria del puerto de cuentas: guarda lo mismo que las columnas, y las transacciones ya armadas.
#[derive(Default)]
pub struct CuentasEnMemoria {
    pub cuentas: Vec<CuentaLeida>,
    pub transacciones_guardadas: Vec<TransaccionLeida>,
    siguiente_id: i64,
}

impl CuentasEnMemoria {
    pub fn nuevo() -> Self {
        CuentasEnMemoria { siguiente_id: 1, ..Default::default() }
    }
}

impl CatalogoDeCuentas for CuentasEnMemoria {
    fn cuentas(&self) -> Result<Vec<CuentaLeida>, ErrorAlmacen> {
        let mut v = self.cuentas.clone();
        v.sort_by(|a, b| a.nombre.cmp(&b.nombre));
        Ok(v)
    }
    fn insertar_cuenta(&mut self, c: &CuentaNueva) -> Result<i64, ErrorAlmacen> {
        if self.cuentas.iter().any(|x| x.nombre == c.nombre) {
            return Err(ErrorAlmacen::Fallo("UNIQUE constraint failed: cuentas_ahorro.nombre".into()));
        }
        if c.divisa != "DOP" && c.divisa != "USD" {
            return Err(ErrorAlmacen::Fallo("CHECK constraint failed: divisa IN ('DOP', 'USD')".into()));
        }
        let id = self.siguiente_id;
        self.siguiente_id += 1;
        self.cuentas.push(CuentaLeida {
            id,
            nombre: c.nombre.clone(),
            divisa: c.divisa.clone(),
            balance_actual: c.saldo_inicial,
            entidad: c.entidad.clone(),
            comision_pago_impuestos: c.comision_pago_impuestos,
        });
        Ok(id)
    }
    fn corregir_cuenta(&mut self, c: &CuentaCorregida) -> Result<bool, ErrorAlmacen> {
        match self.cuentas.iter_mut().find(|x| x.id == c.id) {
            None => Ok(false),
            Some(x) => {
                x.nombre = c.nombre.clone();
                x.entidad = c.entidad.clone();
                x.comision_pago_impuestos = c.comision_pago_impuestos;
                Ok(true)
            }
        }
    }
    fn transacciones(&self) -> Result<Vec<TransaccionLeida>, ErrorAlmacen> {
        let mut v = self.transacciones_guardadas.clone();
        v.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(v)
    }
}

impl AlmacenIngresos for AlmacenEnMemoria {
    fn ingresos(&self) -> Result<Vec<IngresoLeido>, ErrorAlmacen> {
        let mut v: Vec<IngresoLeido> = self
            .facturas
            .iter()
            .map(|f| {
                let c = self.clientes_de_facturas.iter().find(|c| c.id == f.cliente_id).expect("cliente de la factura");
                IngresoLeido {
                    id: f.id,
                    numero_factura: f.numero_factura.clone(),
                    cliente_id: f.cliente_id,
                    cliente_nombre: c.nombre.clone(),
                    cliente_rnc: c.rnc.clone(),
                    fecha_emision: f.fecha_emision.clone(),
                    estatus: f.estatus.clone(),
                    monto_total: f.monto_total,
                    porcentaje_retencion: f.porcentaje_retencion,
                    monto_retenido: f.monto_retenido,
                    institucion_deposito: f.institucion_deposito.clone(),
                    fecha_pago: f.fecha_pago.clone(),
                    monto_recibido: f.monto_recibido,
                }
            })
            .collect();
        v.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(v)
    }
    fn factura_existe(&self, numero: &str) -> Result<bool, ErrorAlmacen> {
        Ok(self.facturas.iter().any(|f| f.numero_factura.to_lowercase() == numero.to_lowercase()))
    }
    fn cliente_por_rnc(&self, rnc: &str) -> Result<Option<i64>, ErrorAlmacen> {
        Ok(self.clientes_de_facturas.iter().find(|c| c.rnc == rnc).map(|c| c.id))
    }
    fn registrar_cliente(&mut self, rnc: &str, nombre: &str) -> Result<i64, ErrorAlmacen> {
        let id = self.siguiente_id;
        self.siguiente_id += 1;
        self.clientes_de_facturas.push(ClienteGuardado { id, rnc: rnc.to_string(), nombre: nombre.to_string() });
        Ok(id)
    }
    fn insertar_factura(&mut self, f: &FacturaNueva) -> Result<i64, ErrorAlmacen> {
        let id = self.siguiente_id;
        self.siguiente_id += 1;
        self.facturas.push(FacturaEnMemoria {
            id,
            numero_factura: f.numero_factura.clone(),
            cliente_id: f.cliente_id,
            fecha_emision: f.fecha_emision.clone(),
            monto_total: f.monto_total,
            porcentaje_retencion: f.porcentaje_retencion,
            monto_retenido: f.monto_retenido,
            estatus: "emitida".to_string(),
            cuenta_ahorro_id: None,
            institucion_deposito: None,
            fecha_pago: None,
            monto_recibido: None,
        });
        Ok(id)
    }
    fn estado_de_factura(&self, id: i64) -> Result<Option<EstadoDeFactura>, ErrorAlmacen> {
        Ok(self.facturas.iter().find(|f| f.id == id).map(|f| EstadoDeFactura {
            numero_factura: f.numero_factura.clone(),
            estatus: f.estatus.clone(),
            monto_total: f.monto_total,
            institucion_deposito: f.institucion_deposito.clone(),
            monto_recibido: f.monto_recibido,
        }))
    }
    fn corregir_factura(&mut self, c: &FacturaCorregida) -> Result<(), ErrorAlmacen> {
        if let Some(f) = self.facturas.iter_mut().find(|f| f.id == c.id) {
            f.numero_factura = c.numero_factura.clone();
            f.cliente_id = c.cliente_id;
            f.fecha_emision = c.fecha_emision.clone();
            f.monto_total = c.monto_total;
            f.porcentaje_retencion = c.porcentaje_retencion;
            f.monto_retenido = c.monto_retenido;
        }
        Ok(())
    }
    fn fijar_recibido(&mut self, id: i64, monto: f64) -> Result<(), ErrorAlmacen> {
        if let Some(f) = self.facturas.iter_mut().find(|f| f.id == id) {
            f.monto_recibido = Some(monto);
        }
        Ok(())
    }
    fn eliminar_factura(&mut self, id: i64) -> Result<(), ErrorAlmacen> {
        self.facturas.retain(|f| f.id != id);
        Ok(())
    }
    fn marcar_cobrada(&mut self, id: i64, cuenta_id: i64, nombre: &str, fecha: &str, monto: f64) -> Result<bool, ErrorAlmacen> {
        match self.facturas.iter_mut().find(|f| f.id == id && f.estatus != "pagada") {
            None => Ok(false),
            Some(f) => {
                f.estatus = "pagada".to_string();
                f.cuenta_ahorro_id = Some(cuenta_id);
                f.institucion_deposito = Some(nombre.to_string());
                f.fecha_pago = Some(fecha.to_string());
                f.monto_recibido = Some(monto);
                Ok(true)
            }
        }
    }
}

/// Los casos que el doble ha anotado, con su número correlativo.
impl RegistroDeCorrecciones for AlmacenEnMemoria {
    fn anotar_caso(&mut self, caso: &CasoAAnotar) -> Result<String, ErrorAlmacen> {
        self.casos.push(caso.clone());
        Ok(format!("CASO-{:04}", self.casos.len()))
    }
}

impl BusquedaDeCuentas for AlmacenEnMemoria {
    fn nombre_de_cuenta(&self, cuenta_id: i64) -> Result<String, ErrorAlmacen> {
        self.cuentas
            .get(&cuenta_id)
            .map(|c| c.nombre.clone())
            .ok_or(ErrorAlmacen::NoEncontrado { entidad: "cuenta", id: cuenta_id })
    }
    fn cuenta_por_nombre(&self, nombre: &str) -> Result<Option<i64>, ErrorAlmacen> {
        Ok(self.cuentas.iter().find(|(_, c)| c.nombre == nombre).map(|(id, _)| *id))
    }
}

impl AlmacenInformales for AlmacenEnMemoria {
    fn informales(&self) -> Result<Vec<InformalLeido>, ErrorAlmacen> {
        let mut v: Vec<InformalLeido> = self
            .informales
            .iter()
            .map(|i| InformalLeido {
                id: i.id,
                fecha: i.fecha.clone(),
                descripcion: i.descripcion.clone(),
                monto: i.monto,
                estatus: i.estatus.clone(),
                institucion_deposito: i.institucion_deposito.clone(),
                fecha_pago: i.fecha_pago.clone(),
                monto_recibido: i.monto_recibido,
            })
            .collect();
        v.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(v)
    }
    fn insertar_informal(&mut self, fecha: &str, descripcion: &str, monto: f64) -> Result<i64, ErrorAlmacen> {
        let id = self.siguiente_id;
        self.siguiente_id += 1;
        self.informales.push(InformalEnMemoria {
            id,
            fecha: fecha.into(),
            descripcion: descripcion.into(),
            monto,
            estatus: "pendiente".into(),
            cuenta_ahorro_id: None,
            institucion_deposito: None,
            fecha_pago: None,
            monto_recibido: None,
        });
        Ok(id)
    }
    fn insertar_cobro_en_efectivo(&mut self, fecha: &str, descripcion: &str, monto: f64, caja: &str) -> Result<i64, ErrorAlmacen> {
        let id = self.insertar_informal(fecha, descripcion, monto)?;
        let i = self.informales.iter_mut().find(|i| i.id == id).unwrap();
        i.estatus = "pagado".into();
        i.institucion_deposito = Some(caja.into());
        i.fecha_pago = Some(fecha.into());
        i.monto_recibido = Some(monto);
        Ok(id)
    }
    fn estado_de_informal(&self, id: i64) -> Result<Option<EstadoDeInformal>, ErrorAlmacen> {
        Ok(self.informales.iter().find(|i| i.id == id).map(|i| EstadoDeInformal {
            descripcion: i.descripcion.clone(),
            estatus: i.estatus.clone(),
            monto: i.monto,
            institucion_deposito: i.institucion_deposito.clone(),
            monto_recibido: i.monto_recibido,
        }))
    }
    fn marcar_informal_cobrado(&mut self, id: i64, cuenta_id: i64, nombre: &str, fecha: &str, monto: f64) -> Result<bool, ErrorAlmacen> {
        match self.informales.iter_mut().find(|i| i.id == id && i.estatus != "pagado") {
            None => Ok(false),
            Some(i) => {
                i.estatus = "pagado".into();
                i.cuenta_ahorro_id = Some(cuenta_id);
                i.institucion_deposito = Some(nombre.into());
                i.fecha_pago = Some(fecha.into());
                i.monto_recibido = Some(monto);
                Ok(true)
            }
        }
    }
    fn eliminar_informal(&mut self, id: i64) -> Result<(), ErrorAlmacen> {
        self.informales.retain(|i| i.id != id);
        Ok(())
    }
}

/// Doble en memoria del puerto de financiamientos: guarda las filas y su libro de movimientos.
#[derive(Default)]
pub struct PrestamosEnMemoria {
    pub prestamos: Vec<PrestamoLeido>,
    pub libro: Vec<(i64, MovimientoDePrestamo)>,
    pub tarjetas: Vec<i64>,
    siguiente_id: i64,
    siguiente_movimiento: i64,
}

impl PrestamosEnMemoria {
    pub fn nuevo() -> Self {
        PrestamosEnMemoria { siguiente_id: 1, siguiente_movimiento: 1, ..Default::default() }
    }
    pub fn con_tarjeta(mut self, id: i64) -> Self {
        self.tarjetas.push(id);
        self
    }
}

impl AlmacenPrestamos for PrestamosEnMemoria {
    fn prestamos(&self) -> Result<Vec<PrestamoLeido>, ErrorAlmacen> {
        let mut v = self.prestamos.clone();
        v.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(v)
    }
    fn insertar_prestamo(&mut self, p: &PrestamoNuevo) -> Result<i64, ErrorAlmacen> {
        let id = self.siguiente_id;
        self.siguiente_id += 1;
        self.prestamos.push(PrestamoLeido {
            id,
            tipo_prestamo: p.tipo_prestamo.clone(),
            monto_prestamo: p.monto_prestamo,
            institucion_financiera: p.institucion_financiera.clone(),
            tasa_actual: p.tasa_actual,
            cuotas_totales: p.cuotas_totales,
            cuotas_pendientes: p.cuotas_pendientes,
            monto_cuota: p.monto_cuota,
            dia_pago: p.dia_pago,
            saldo_actual: Some(p.saldo_actual),
            limite_credito: p.limite_credito,
            tarjeta_id: None,
            tarjeta_nombre: None,
            tarjeta_fecha_corte: None,
            tarjeta_fecha_limite_pago: None,
        });
        Ok(id)
    }
    fn tipo_de_prestamo(&self, id: i64) -> Result<Option<String>, ErrorAlmacen> {
        Ok(self.prestamos.iter().find(|p| p.id == id).map(|p| p.tipo_prestamo.clone()))
    }
    fn tarjeta_existe(&self, tarjeta_id: i64) -> Result<bool, ErrorAlmacen> {
        Ok(self.tarjetas.contains(&tarjeta_id))
    }
    fn corregir_prestamo(&mut self, c: &PrestamoCorregido) -> Result<(), ErrorAlmacen> {
        if let Some(p) = self.prestamos.iter_mut().find(|p| p.id == c.id) {
            p.tasa_actual = c.tasa_actual;
            p.monto_cuota = c.monto_cuota;
            p.dia_pago = c.dia_pago;
            p.limite_credito = c.limite_credito;
            p.tarjeta_id = c.tarjeta_id;
        }
        Ok(())
    }
    fn estado_de_prestamo(&self, id: i64) -> Result<Option<EstadoDePrestamo>, ErrorAlmacen> {
        Ok(self.prestamos.iter().find(|p| p.id == id).map(|p| EstadoDePrestamo {
            saldo: p.saldo_actual.unwrap_or(0.0),
            tasa_anual: p.tasa_actual,
            monto_cuota: p.monto_cuota,
        }))
    }
    fn asentar_movimiento(&mut self, m: &MovimientoNuevo) -> Result<(), ErrorAlmacen> {
        let id = self.siguiente_movimiento;
        self.siguiente_movimiento += 1;
        self.libro.push((
            m.prestamo_id,
            MovimientoDePrestamo {
                id,
                fecha: m.fecha.clone(),
                tipo: m.tipo.clone(),
                monto: m.monto.unidades(),
                interes: m.interes.unidades(),
                capital: m.capital.unidades(),
                saldo_resultante: m.saldo_resultante.unidades(),
            },
        ));
        if let Some(p) = self.prestamos.iter_mut().find(|p| p.id == m.prestamo_id) {
            p.saldo_actual = Some(m.saldo_resultante.unidades());
        }
        Ok(())
    }
    fn descontar_cuota(&mut self, id: i64) -> Result<(), ErrorAlmacen> {
        if let Some(p) = self.prestamos.iter_mut().find(|p| p.id == id) {
            if let Some(c) = p.cuotas_pendientes {
                p.cuotas_pendientes = Some((c - 1).max(0));
            }
        }
        Ok(())
    }
    fn movimientos(&self, id: i64) -> Result<Vec<MovimientoDePrestamo>, ErrorAlmacen> {
        let mut v: Vec<MovimientoDePrestamo> = self.libro.iter().filter(|(p, _)| *p == id).map(|(_, m)| m.clone()).collect();
        v.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(v)
    }
    fn eliminar_prestamo(&mut self, id: i64) -> Result<(), ErrorAlmacen> {
        self.prestamos.retain(|p| p.id != id);
        self.libro.retain(|(p, _)| *p != id);
        Ok(())
    }
}

/// Doble en memoria del puerto de tarjetas: guarda las filas.
#[derive(Default)]
pub struct TarjetasEnMemoria {
    pub tarjetas: Vec<TarjetaLeida>,
    siguiente_id: i64,
}

impl TarjetasEnMemoria {
    pub fn nuevo() -> Self {
        TarjetasEnMemoria { siguiente_id: 1, ..Default::default() }
    }
}

impl CatalogoDeTarjetas for TarjetasEnMemoria {
    fn tarjetas(&self) -> Result<Vec<TarjetaLeida>, ErrorAlmacen> {
        Ok(self.tarjetas.clone())
    }
    fn insertar_tarjeta(&mut self, t: &TarjetaNueva) -> Result<i64, ErrorAlmacen> {
        if !(1..=31).contains(&t.fecha_corte) || !(1..=31).contains(&t.fecha_limite_pago) {
            return Err(ErrorAlmacen::Fallo("CHECK constraint failed: fecha BETWEEN 1 AND 31".into()));
        }
        let id = self.siguiente_id;
        self.siguiente_id += 1;
        self.tarjetas.push(TarjetaLeida {
            id,
            entidad: t.entidad.clone(),
            nombre_tarjeta: t.nombre_tarjeta.clone(),
            limite_pesos: t.limite_pesos,
            limite_dolares: t.limite_dolares,
            limite_sobregiro_pesos: t.limite_sobregiro_pesos,
            limite_sobregiro_dolares: t.limite_sobregiro_dolares,
            balance_pesos: t.balance_pesos,
            balance_dolares: t.balance_dolares,
            balance_corte_pesos: t.balance_corte_pesos,
            balance_corte_dolares: t.balance_corte_dolares,
            fecha_corte: t.fecha_corte,
            fecha_limite_pago: t.fecha_limite_pago,
            limite_ajustado_pesos: None,
            limite_ajustado_dolares: None,
            politica_liquidacion: None,
        });
        Ok(id)
    }
    fn actualizar_limites(&mut self, l: &LimitesDeTarjeta) -> Result<(), ErrorAlmacen> {
        if let Some(t) = self.tarjetas.iter_mut().find(|t| t.id == l.id) {
            t.limite_pesos = l.limite_pesos;
            t.limite_dolares = l.limite_dolares;
            t.limite_sobregiro_pesos = l.limite_sobregiro_pesos;
            t.limite_sobregiro_dolares = l.limite_sobregiro_dolares;
            t.balance_corte_pesos = l.balance_corte_pesos;
            t.balance_corte_dolares = l.balance_corte_dolares;
            t.limite_ajustado_pesos = l.limite_ajustado_pesos;
            t.limite_ajustado_dolares = l.limite_ajustado_dolares;
            t.politica_liquidacion = Some(l.politica_liquidacion.clone());
        }
        Ok(())
    }
}

impl ConsultaDeAbonos for AlmacenEnMemoria {
    fn abonos_de_tarjeta(&self, tarjeta_id: i64) -> Result<Vec<AbonoLeido>, ErrorAlmacen> {
        let mut v: Vec<AbonoLeido> = self
            .pagos
            .values()
            .filter(|p| p.tarjeta_id == tarjeta_id)
            .map(|p| AbonoLeido {
                id: p.id,
                fecha_pago: self.fechas_de_pago.get(&p.id).cloned().unwrap_or_default(),
                monto_pagado: p.monto.unidades(),
                divisa: p.monto.divisa().codigo().to_string(),
                cuenta_ahorro_id: p.cuenta_ahorro_id,
                cuenta_nombre: p.cuenta_ahorro_id.and_then(|c| self.cuentas.get(&c)).map(|c| c.nombre.clone()),
                tasa_cambio: p.tasa_cambio,
            })
            .collect();
        v.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(v)
    }
    fn resumen_de_abono(&self, id: i64) -> Result<Option<ResumenDeAbono>, ErrorAlmacen> {
        Ok(self.pagos.get(&id).map(|p| ResumenDeAbono {
            fecha_pago: self.fechas_de_pago.get(&id).cloned().unwrap_or_default(),
            monto_pagado: p.monto.unidades(),
            divisa: p.monto.divisa().codigo().to_string(),
        }))
    }
}

impl CategoriaDeSistema for AlmacenEnMemoria {
    fn categoria_de_sistema(&self) -> Result<i64, ErrorAlmacen> {
        self.categorias
            .iter()
            .find(|(_, n)| n.as_str() == "Otros")
            .map(|(id, _)| *id)
            .ok_or(ErrorAlmacen::Fallo("Query returned no rows".into()))
    }
}

impl ConsultaDeAvances for AlmacenEnMemoria {
    fn avances_de_tarjeta(&self, tarjeta_id: i64) -> Result<Vec<AvanceLeido>, ErrorAlmacen> {
        let mut v: Vec<AvanceLeido> = self
            .avances
            .values()
            .filter(|a| a.tarjeta_id == tarjeta_id)
            .filter_map(|a| {
                let cuenta = self.cuentas.get(&a.cuenta_ahorro_id)?;
                Some(AvanceLeido {
                    id: a.id,
                    fecha: self.fechas_de_avance.get(&a.id).cloned().unwrap_or_default(),
                    monto: a.monto.unidades(),
                    divisa: a.monto.divisa().codigo().to_string(),
                    // El doble de `AvanceGuardado` no guarda el tipo de cargo, la tasa ni la nota: lo cubre SQLite.
                    tipo_cargo: String::new(),
                    tasa: None,
                    cargo: a.cargo.unidades(),
                    cuenta_ahorro_id: a.cuenta_ahorro_id,
                    cuenta_nombre: cuenta.nombre.clone(),
                    nota: None,
                })
            })
            .collect();
        v.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(v)
    }
    fn resumen_de_avance(&self, id: i64) -> Result<Option<ResumenDeAvance>, ErrorAlmacen> {
        Ok(self.avances.get(&id).map(|a| ResumenDeAvance {
            fecha: self.fechas_de_avance.get(&id).cloned().unwrap_or_default(),
            monto: a.monto.unidades(),
            divisa: a.monto.divisa().codigo().to_string(),
        }))
    }
}

impl ConsultaDeBonificaciones for AlmacenEnMemoria {
    fn bonificaciones(&self) -> Result<Vec<BonificacionLeida>, ErrorAlmacen> {
        let mut v: Vec<BonificacionLeida> = self
            .bonificaciones
            .values()
            .map(|b| BonificacionLeida {
                id: b.id,
                // El doble no guarda la fecha ni los datos de la tarjeta: lo cubre SQLite.
                fecha: String::new(),
                tarjeta_id: b.tarjeta_id,
                entidad: String::new(),
                nombre_tarjeta: String::new(),
                monto: b.bonificacion.monto().unidades(),
                divisa: b.bonificacion.monto().divisa().codigo().to_string(),
                concepto: b.bonificacion.concepto().to_string(),
                gasto_id: None,
            })
            .collect();
        v.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(v)
    }
}

impl ConsultaDeGastos for AlmacenEnMemoria {
    fn gastos(&self) -> Result<Vec<GastoLeido>, ErrorAlmacen> {
        let mut v: Vec<GastoLeido> = self
            .gastos
            .values()
            .map(|g| GastoLeido {
                id: g.id,
                // El doble de `GastoGuardado` no guarda fecha, descripción ni categoría: lo cubre SQLite.
                fecha: String::new(),
                monto: g.monto.unidades(),
                divisa: g.monto.divisa().codigo().to_string(),
                descripcion: String::new(),
                categoria_id: 0,
                categoria_nombre: String::new(),
                metodo_pago: g.metodo_pago.clone(),
                costo_adicional: g.cargos.unidades(),
                tarjeta_id: g.tarjeta_id,
                cuenta_ahorro_id: g.cuenta_ahorro_id,
                estado_conversion: g.estado_conversion.codigo().map(str::to_string),
                monto_liquidado: None,
                tasa_conversion: None,
            })
            .collect();
        v.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(v)
    }
    fn resumen_de_gasto(&self, id: i64) -> Result<Option<ResumenDeGasto>, ErrorAlmacen> {
        Ok(self.gastos.get(&id).map(|g| ResumenDeGasto {
            descripcion: self.descripciones_de_gasto.get(&id).cloned().unwrap_or_default(),
            monto: g.monto.unidades(),
            divisa: g.monto.divisa().codigo().to_string(),
        }))
    }
    fn motivo_de_no_borrar(&self, id: i64) -> Result<Option<String>, ErrorAlmacen> {
        Ok(self.motivos_de_no_borrar.get(&id).cloned())
    }
}

impl AlmacenSuscripciones for AlmacenEnMemoria {
    fn suscripciones_con_tarjeta(&self) -> Result<Vec<SuscripcionLeida>, ErrorAlmacen> {
        let mut v: Vec<SuscripcionLeida> = self
            .suscripciones
            .iter()
            .map(|s| SuscripcionLeida {
                id: s.datos.id,
                plataforma: s.datos.plataforma.clone(),
                monto: s.datos.monto,
                tarjeta_id: s.datos.tarjeta_id,
                frecuencia: s.datos.frecuencia.clone(),
                dia_facturacion: s.datos.dia_facturacion,
                fecha_ultimo_pago: s.fecha_ultimo_pago.clone(),
                divisa: s.datos.divisa.clone(),
                // El doble no guarda los datos de la tarjeta: lo cubre SQLite.
                entidad: String::new(),
                nombre_tarjeta: String::new(),
                fecha_proximo_cobro: s.datos.fecha_proximo_cobro.clone(),
            })
            .collect();
        v.sort_by(|a, b| a.plataforma.cmp(&b.plataforma));
        Ok(v)
    }
    fn suscripciones_registradas(&self) -> Result<Vec<SuscripcionRegistrada>, ErrorAlmacen> {
        let mut v: Vec<SuscripcionRegistrada> = self.suscripciones.iter().map(|s| s.datos.clone()).collect();
        v.sort_by_key(|s| s.id);
        Ok(v)
    }
    fn insertar_suscripcion(&mut self, s: &SuscripcionAGuardar) -> Result<i64, ErrorAlmacen> {
        let id = self.siguiente_id;
        self.siguiente_id += 1;
        self.suscripciones.push(SuscripcionEnMemoria {
            datos: SuscripcionRegistrada {
                id,
                plataforma: s.plataforma.clone(),
                monto: s.monto,
                tarjeta_id: s.tarjeta_id,
                frecuencia: s.frecuencia.clone(),
                dia_facturacion: s.dia_facturacion,
                divisa: s.divisa.clone(),
                fecha_proximo_cobro: s.fecha_proximo_cobro.clone(),
            },
            fecha_ultimo_pago: None,
        });
        Ok(id)
    }
    fn editar_suscripcion(&mut self, id: i64, s: &SuscripcionAGuardar) -> Result<bool, ErrorAlmacen> {
        match self.suscripciones.iter_mut().find(|x| x.datos.id == id) {
            None => Ok(false),
            Some(x) => {
                x.datos.plataforma = s.plataforma.clone();
                x.datos.monto = s.monto;
                x.datos.tarjeta_id = s.tarjeta_id;
                x.datos.frecuencia = s.frecuencia.clone();
                x.datos.dia_facturacion = s.dia_facturacion;
                x.datos.divisa = s.divisa.clone();
                x.datos.fecha_proximo_cobro = s.fecha_proximo_cobro.clone();
                Ok(true)
            }
        }
    }
    fn fijar_proximo_cobro(&mut self, id: i64, fecha: &str) -> Result<bool, ErrorAlmacen> {
        match self.suscripciones.iter_mut().find(|x| x.datos.id == id) {
            None => Ok(false),
            Some(x) => {
                x.datos.fecha_proximo_cobro = Some(fecha.to_string());
                Ok(true)
            }
        }
    }
    fn eliminar_suscripcion(&mut self, id: i64) -> Result<(), ErrorAlmacen> {
        self.suscripciones.retain(|x| x.datos.id != id);
        Ok(())
    }
    fn mover_puntero(&mut self, id: i64, siguiente: Option<&str>, marca_de_cobro: Option<&str>) -> Result<(), ErrorAlmacen> {
        if let Some(x) = self.suscripciones.iter_mut().find(|x| x.datos.id == id) {
            x.datos.fecha_proximo_cobro = siguiente.map(str::to_string);
            if let Some(m) = marca_de_cobro {
                x.fecha_ultimo_pago = Some(m.to_string());
            }
        }
        Ok(())
    }
    fn categoria_por_nombre(&self, nombre_en_minusculas: &str) -> Result<Option<i64>, ErrorAlmacen> {
        Ok(self.categorias.iter().find(|(_, n)| n.to_lowercase() == nombre_en_minusculas).map(|(id, _)| *id))
    }
    fn crear_categoria(&mut self, nombre: &str) -> Result<i64, ErrorAlmacen> {
        let id = self.siguiente_id;
        self.siguiente_id += 1;
        self.categorias.insert(id, nombre.to_string());
        Ok(id)
    }
}

