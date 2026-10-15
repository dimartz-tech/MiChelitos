//! Comandos de avances de efectivo.

use serde::{Deserialize, Serialize};

use super::ayudantes::{con_almacen, lee_almacen};
use crate::aplicacion;
use crate::aplicacion::registrar_avance_de_efectivo::DatosAvance;
use crate::dominio;
use crate::dominio::dinero::Divisa;
use crate::ipc;

// --- COMANDOS: AVANCES DE EFECTIVO ---

/// Interpreta el cargo tal como llega de la interfaz.
///
/// Lo comparten el registro y la simulación, y por eso está aparte: que lo que
/// se enseña antes de confirmar y lo que se asienta después salgan del mismo
/// código es lo único que garantiza que sean la misma cifra.
///
/// Llega en **tres formas excluyentes** y se rechaza la contradicción —un
/// porcentaje con un cargo fijo, o un valor con una exoneración— en vez de
/// elegir una en silencio: un dato que se ignora sin avisar es el que después
/// nadie sabe por qué no cuadra.

#[derive(Serialize, Deserialize, Debug)]
pub struct SimulacionAvance {
    pub monto: f64,
    pub cargo: f64,
    pub a_la_tarjeta: f64,
    pub a_la_cuenta: f64,
}

/// Calcula lo que un avance movería, sin guardar nada.
///
/// Existe para que la interfaz enseñe las cifras **antes** de confirmar sin
/// tener que repetir la regla en JavaScript: una regla en el HTML es una regla
/// sin pruebas, y la cifra que se confirma debe ser exactamente la que se
/// asienta.
#[tauri::command]
pub fn simular_avance_efectivo(
    monto: ipc::ImporteDecimal,
    divisa: String,
    tipo_cargo: String,
    porcentaje: Option<f64>,
    cargo_fijo: Option<ipc::ImporteDecimal>,
) -> Result<SimulacionAvance, String> {
    let divisa = Divisa::desde_codigo(&divisa)?;
    let cargo = dominio::avance::cargo_desde_la_peticion(&tipo_cargo, porcentaje, cargo_fijo.map(|f| f.con_divisa(divisa)))?;
    let avance = dominio::avance::Avance::calcular(monto.con_divisa(divisa), cargo)?;
    Ok(SimulacionAvance {
        monto: avance.monto.unidades(),
        cargo: avance.cargo.unidades(),
        a_la_tarjeta: avance.a_la_tarjeta()?.unidades(),
        a_la_cuenta: avance.monto.unidades(),
    })
}

/// Registra un avance de efectivo: la tarjeta pone dinero en una cuenta.
///
/// Traducción pura, como el resto de comandos: la regla vive en
/// `dominio::avance` y en el caso de uso. Aquí solo se interpreta lo que
/// llega de la interfaz.
#[tauri::command]
pub fn registrar_avance_efectivo(
    tarjeta_id: i64,
    cuenta_ahorro_id: i64,
    fecha: String,
    monto: ipc::ImporteDecimal,
    divisa: String,
    tipo_cargo: String,
    porcentaje: Option<f64>,
    cargo_fijo: Option<ipc::ImporteDecimal>,
    nota: Option<String>,
) -> Result<String, String> {
    // Todo lo que llega de la interfaz se valida antes de abrir la transacción: la fecha, la divisa, el cargo y la nota.
    let fecha = dominio::avance::fecha_de_avance(&fecha)?;
    let divisa = Divisa::desde_codigo(&divisa)?;
    let cargo = dominio::avance::cargo_desde_la_peticion(&tipo_cargo, porcentaje, cargo_fijo.map(|f| f.con_divisa(divisa)))?;
    let nota = dominio::avance::nota_de_avance(nota);

    let registrado = con_almacen(|a| {
        Ok(aplicacion::avances::registrar_avance(
            DatosAvance { tarjeta_id, cuenta_ahorro_id, fecha, monto: monto.con_divisa(divisa), cargo, nota },
            a,
        )?)
    })?;

    Ok(format!(
        "Avance registrado. La cuenta recibe {} {:.2}; el cargo es {} {:.2} y la deuda de la tarjeta sube {} {:.2}.",
        divisa.codigo(),
        monto.con_divisa(divisa).unidades(),
        divisa.codigo(),
        registrado.cargo.unidades(),
        divisa.codigo(),
        registrado.a_la_tarjeta.unidades()
    ))
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AvanceEfectivo {
    pub id: i64,
    pub fecha: String,
    pub monto: f64,
    pub divisa: String,
    pub tipo_cargo: String,
    pub tasa: Option<f64>,
    pub cargo: f64,
    pub cuenta_ahorro_id: i64,
    pub cuenta_nombre: String,
    pub nota: Option<String>,
}

/// Avances de una tarjeta, del más reciente al más antiguo.
#[tauri::command]
pub fn obtener_avances_tarjeta(tarjeta_id: i64) -> Result<Vec<AvanceEfectivo>, String> {
    lee_almacen(|a| {
        Ok(aplicacion::avances::listar_avances(tarjeta_id, a)?
            .into_iter()
            .map(|v| AvanceEfectivo {
                id: v.id,
                fecha: v.fecha,
                monto: v.monto,
                divisa: v.divisa,
                tipo_cargo: v.tipo_cargo,
                tasa: v.tasa,
                cargo: v.cargo,
                cuenta_ahorro_id: v.cuenta_ahorro_id,
                cuenta_nombre: v.cuenta_nombre,
                nota: v.nota,
            })
            .collect())
    })
}

/// Deshace un avance de efectivo, con caso de auditoría.
#[tauri::command]
pub fn revertir_avance_efectivo(id: i64, motivo: String) -> Result<String, String> {
    con_almacen(|a| {
        let r = aplicacion::avances::revertir_avance(id, &motivo, a)?;
        Ok(format!(
            "La deuda de la tarjeta baja {} {:.2} y la cuenta devuelve {} {:.2}. Caso {}.",
            r.revertido.deuda_restituida.divisa().codigo(),
            r.revertido.deuda_restituida.unidades(),
            r.revertido.devuelto_por_la_cuenta.divisa().codigo(),
            r.revertido.devuelto_por_la_cuenta.unidades(),
            r.caso
        ))
    })
}
