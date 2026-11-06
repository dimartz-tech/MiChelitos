//! Comandos de los presupuestos mensuales por categoría.
//!
//! `fijar_presupuesto` y `quitar_presupuesto` **escriben** (transacción `IMMEDIATE`); `obtener_presupuestos_mes` solo lee.
//! Los importes viajan como texto exacto (`1234.56`), nunca como número. Las reglas (solo cuentan los gastos en DOP, los
//! dólares se informan aparte sin convertir, estados «cerca» desde el 80 % y «excedido» desde el 100 %) están en
//! `dominio::presupuesto`.

use serde::Serialize;

use super::ayudantes::{con_catalogos, lee_almacen};
use crate::aplicacion;
use crate::dominio::dinero::Divisa;
use crate::dominio::presupuesto::{EstadoDeCategoria, EstadoDelMes};
use crate::ipc;

/// El presupuesto de una categoría recién fijado.
#[derive(Serialize, Debug)]
pub struct PresupuestoFijado {
    pub categoria_id: i64,
    pub categoria_nombre: String,
    /// La marca «fija / recurrente» de la categoría (no cambia al fijar el presupuesto).
    pub fija: bool,
    /// Pesos, texto exacto.
    pub monto: String,
}

/// Un presupuesto frente a lo gastado en el mes. Importes en pesos (texto exacto) salvo `usd_aparte`.
#[derive(Serialize, Debug)]
pub struct FilaPresupuesto {
    pub categoria_id: i64,
    pub categoria_nombre: String,
    pub fija: bool,
    pub presupuesto: String,
    /// Solo gastos en DOP de la categoría en el mes.
    pub gastado: String,
    /// Presupuesto menos gastado; negativo si se excedió.
    pub restante: String,
    /// Gastado sobre presupuesto en milésimas, truncado (1000 = 100 %); puede pasar de 1000.
    pub porcentaje_milesimas: i64,
    /// `ok`, `cerca` (desde el 80 %) o `excedido` (desde el 100 %).
    pub estado: String,
    /// Los dólares de la categoría en el mes, **sin convertir** y fuera de `gastado`.
    pub usd_aparte: String,
}

#[derive(Serialize, Debug)]
pub struct EstadoPresupuestos {
    pub anio: i32,
    pub mes: u32,
    /// Excedidos primero, luego cerca, luego ok.
    pub filas: Vec<FilaPresupuesto>,
    pub total_presupuesto: String,
    /// Lo gastado en las categorías que tienen presupuesto (no el total del mes).
    pub total_gastado: String,
    pub total_restante: String,
}

fn a_fila(e: &EstadoDeCategoria) -> FilaPresupuesto {
    FilaPresupuesto {
        categoria_id: e.categoria_id,
        categoria_nombre: e.categoria_nombre.clone(),
        fija: e.fija,
        presupuesto: ipc::importe_a_texto(e.presupuesto),
        gastado: ipc::importe_a_texto(e.gastado),
        restante: ipc::importe_a_texto(e.restante),
        porcentaje_milesimas: e.porcentaje_milesimas,
        estado: e.estado.como_texto().to_string(),
        usd_aparte: ipc::importe_a_texto(e.usd_aparte),
    }
}

fn a_dto(r: &EstadoDelMes) -> EstadoPresupuestos {
    EstadoPresupuestos {
        anio: r.anio,
        mes: r.mes,
        filas: r.filas.iter().map(a_fila).collect(),
        total_presupuesto: ipc::importe_a_texto(r.total_presupuesto),
        total_gastado: ipc::importe_a_texto(r.total_gastado),
        total_restante: ipc::importe_a_texto(r.total_restante),
    }
}

/// Fija (crea o reemplaza) el presupuesto mensual, en pesos, de una categoría. **Escritura.** Un monto que no es mayor
/// que cero, que pasa de DOP 999,999,999.99 o de una categoría que no existe es un error legible y no escribe nada.
#[tauri::command]
pub fn fijar_presupuesto(categoria_id: i64, monto: ipc::ImporteDecimal) -> Result<PresupuestoFijado, String> {
    let centavos = monto.con_divisa(Divisa::Dop).centavos();
    con_catalogos(|a| {
        let p = aplicacion::presupuestos::fijar_presupuesto(categoria_id, centavos, a)?;
        Ok(PresupuestoFijado {
            categoria_id: p.categoria_id,
            categoria_nombre: p.categoria_nombre,
            fija: p.fija,
            monto: ipc::importe_a_texto(crate::dominio::dinero::Dinero::de_centavos(p.centavos, Divisa::Dop)),
        })
    })
}

/// Quita el presupuesto de una categoría. **Escritura.** Devuelve si lo había (quitar uno inexistente no falla).
#[tauri::command]
pub fn quitar_presupuesto(categoria_id: i64) -> Result<bool, String> {
    con_catalogos(|a| Ok(aplicacion::presupuestos::quitar_presupuesto(categoria_id, a)?))
}

/// El estado de los presupuestos en el mes (`anio`, `mes`): por categoría con presupuesto, lo gastado en pesos, lo que
/// resta, el porcentaje y el estado. Los gastos en dólares no se convierten ni cuentan: salen aparte. **Solo lectura.**
#[tauri::command]
pub fn obtener_presupuestos_mes(anio: i32, mes: u32) -> Result<EstadoPresupuestos, String> {
    lee_almacen(|a| Ok(a_dto(&aplicacion::presupuestos::estado_de_presupuestos(anio, mes, a, a)?)))
}
