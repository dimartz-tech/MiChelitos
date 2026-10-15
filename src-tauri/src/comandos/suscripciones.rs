//! Comandos de suscripciones y de su cobro automático.

use serde::{Deserialize, Serialize};

use super::ayudantes::{con_almacen, lee_almacen};
use crate::aplicacion;
use crate::dominio;
use crate::dominio::dinero::Divisa;
use crate::ipc;
use crate::puertos::reloj::Reloj;

#[derive(Serialize, Deserialize, Debug)]
pub struct Suscripcion {
    pub id: i64,
    pub plataforma: String,
    pub monto: f64,
    pub tarjeta_id: i64,
    pub frecuencia: String,
    pub dia_facturacion: i32,
    pub fecha_ultimo_pago: Option<String>,
    pub divisa: String,
    pub entidad: String,
    pub nombre_tarjeta: String,
    /// Cuándo vence el próximo cobro. `None` significa que no se sabe, y
    /// entonces no se cobra.
    pub fecha_proximo_cobro: Option<String>,
    /// Los vencimientos que hay que confirmar a mano, si son dos o más.
    ///
    /// Viajan con la suscripción y no en una consulta aparte: un período que
    /// se perdió tiene que verse justo donde se mira la suscripción.
    pub pendientes: Vec<String>,
    /// Si el cargo cae dentro de los próximos siete días.
    pub avisa: bool,
    /// Por qué esta suscripción no llegará a cobrarse, si es el caso.
    ///
    /// Va en el mismo viaje que los datos y no en una consulta aparte: una
    /// suscripción parada tiene que verse justo donde se la mira.
    pub impedimento: Option<String>,
}

impl Suscripcion {
    /// Lo que la vista recibe, expuesto para que las pruebas lean **el mismo
    /// campo** en vez de recalcularlo por su cuenta.
    #[cfg(test)]
    pub fn impedimento_para_pruebas(&self) -> Option<&str> {
        self.impedimento.as_deref()
    }
    #[cfg(test)]
    pub fn pendientes_para_pruebas(&self) -> Vec<String> {
        self.pendientes.clone()
    }
    #[cfg(test)]
    pub fn avisa_para_pruebas(&self) -> bool {
        self.avisa
    }
    #[cfg(test)]
    pub fn id_para_pruebas(&self) -> i64 {
        self.id
    }
    #[cfg(test)]
    pub fn plataforma_para_pruebas(&self) -> &str {
        &self.plataforma
    }
}

// --- COMANDOS: SUSCRIPCIONES ---
#[tauri::command]
pub fn obtener_suscripciones() -> Result<Vec<Suscripcion>, String> {
    suscripciones_con_aviso(&crate::adaptadores::reloj_sistema::RelojSistema)
}

/// Las suscripciones, con el aviso ya resuelto para ese «hoy».
pub fn suscripciones_con_aviso(reloj: &dyn Reloj) -> Result<Vec<Suscripcion>, String> {
    let hoy = reloj.hoy();
    lee_almacen(|a| {
        Ok(aplicacion::suscripciones::listar_suscripciones(hoy, a)?
            .into_iter()
            .map(|s| Suscripcion {
                id: s.leida.id,
                plataforma: s.leida.plataforma,
                monto: s.leida.monto,
                tarjeta_id: s.leida.tarjeta_id,
                frecuencia: s.leida.frecuencia,
                dia_facturacion: s.leida.dia_facturacion,
                fecha_ultimo_pago: s.leida.fecha_ultimo_pago,
                divisa: s.leida.divisa,
                entidad: s.leida.entidad,
                nombre_tarjeta: s.leida.nombre_tarjeta,
                fecha_proximo_cobro: s.leida.fecha_proximo_cobro,
                pendientes: s.pendientes,
                avisa: s.avisa,
                impedimento: s.impedimento,
            })
            .collect())
    })
}

/// Las condiciones de una suscripción tal como llegan. El orden en que se rechazan es el de siempre: la fecha, la
/// divisa y, ya en el caso de uso, el importe, la frecuencia y el día.
fn datos_de_suscripcion(
    plataforma: String,
    monto: ipc::ImporteDecimal,
    tarjeta_id: i64,
    frecuencia: String,
    dia_facturacion: i32,
    divisa: String,
    fecha_proximo_cobro: Option<String>,
) -> Result<aplicacion::suscripciones::DatosSuscripcion, String> {
    let fecha_proximo_cobro = dominio::suscripcion::proximo_cobro_declarado(fecha_proximo_cobro)?;
    let monto = monto.con_divisa(Divisa::desde_codigo(&divisa)?);
    Ok(aplicacion::suscripciones::DatosSuscripcion { plataforma, monto, tarjeta_id, frecuencia, dia_facturacion, fecha_proximo_cobro })
}

#[tauri::command]
pub fn crear_suscripcion(plataforma: String, monto: ipc::ImporteDecimal, tarjeta_id: i64, frecuencia: String, dia_facturacion: i32, divisa: String, fecha_proximo_cobro: Option<String>) -> Result<i64, String> {
    let datos = datos_de_suscripcion(plataforma, monto, tarjeta_id, frecuencia, dia_facturacion, divisa, fecha_proximo_cobro)?;
    con_almacen(|a| Ok(aplicacion::suscripciones::crear_suscripcion(datos, a)?))
}

/// Edita una suscripción **conservando `fecha_ultimo_pago`** (ver el caso de uso).
#[tauri::command]
pub fn actualizar_suscripcion(
    id: i64,
    plataforma: String,
    monto: ipc::ImporteDecimal,
    tarjeta_id: i64,
    frecuencia: String,
    dia_facturacion: i32,
    divisa: String,
    fecha_proximo_cobro: Option<String>,
) -> Result<(), String> {
    let datos = datos_de_suscripcion(plataforma, monto, tarjeta_id, frecuencia, dia_facturacion, divisa, fecha_proximo_cobro)?;
    con_almacen(|a| Ok(aplicacion::suscripciones::editar_suscripcion(id, datos, a)?))
}

/// Pone a mano la fecha del próximo cobro: la salida cuando una suscripción se queda sin fecha y por tanto parada.
#[tauri::command]
pub fn corregir_proximo_cobro(id: i64, fecha: String) -> Result<(), String> {
    con_almacen(|a| Ok(aplicacion::suscripciones::corregir_proximo_cobro(id, &fecha, a)?))
}

#[tauri::command]
pub fn eliminar_suscripcion(id: i64) -> Result<(), String> {
    con_almacen(|a| Ok(aplicacion::suscripciones::eliminar_suscripcion(id, a)?))
}

/// Lo que dejó el cobro automático: los cargos asentados y los que **fallaron**, ya redactados. Se separan para que la
/// interfaz muestre los segundos como errores y no como cargos.
#[derive(Serialize, Deserialize, Debug)]
pub struct CobrosAutomaticos {
    pub cargos: Vec<String>,
    pub fallos: Vec<String>,
}

#[tauri::command]
pub fn procesar_suscripciones() -> Result<CobrosAutomaticos, String> {
    procesar_suscripciones_con(&crate::adaptadores::reloj_sistema::RelojSistema)
}

/// El cobro automático, con el «hoy» que le den (para que las pruebas puedan fijar la fecha). La categoría se resuelve
/// una vez, antes del bucle, y cada cobro se asienta en su propia transacción: **uno que falle no arrastra a los demás**
/// (se deshace entero, su puntero no avanza y se reintenta al siguiente arranque) y se informa aparte.
pub fn procesar_suscripciones_con(reloj: &dyn crate::puertos::reloj::Reloj) -> Result<CobrosAutomaticos, String> {
    let hoy = reloj.hoy();
    let categoria = con_almacen(|a| Ok(aplicacion::suscripciones::categoria_de_suscripciones(a)?))?;
    // Solo candidatas: cada cargo vuelve a leer su suscripción dentro de su propia transacción.
    let candidatas = con_almacen(|a| Ok(aplicacion::suscripciones::cobros_automaticos(hoy, a)?))?;

    let resultado = aplicacion::suscripciones::procesar_cobros(candidatas, |id| {
        con_almacen(|a| Ok(aplicacion::suscripciones::cobrar_si_vence(id, hoy, categoria, a)?))
    });
    Ok(CobrosAutomaticos {
        cargos: resultado
            .cargos
            .iter()
            .map(|c| {
                format!(
                    "Cargo automático realizado para {} ({} {:.2}) con fecha {}",
                    c.plataforma,
                    c.divisa,
                    c.monto,
                    c.vencimiento.format("%d/%m/%Y")
                )
            })
            .collect(),
        fallos: resultado
            .fallos
            .iter()
            .map(|f| format!("No se pudo cobrar «{}»: {}. Se reintentará al abrir la aplicación.", f.plataforma, f.motivo))
            .collect(),
    })
}

/// Asienta el período pendiente más antiguo, tras confirmarlo el titular.
#[tauri::command]
pub fn asentar_periodo_pendiente(id: i64) -> Result<String, String> {
    confirmar_pendiente(id, &crate::adaptadores::reloj_sistema::RelojSistema, None)
}

/// Da por no cobrado el período pendiente más antiguo y pasa al siguiente. **Exige un motivo escrito**.
#[tauri::command]
pub fn descartar_periodo_pendiente(id: i64, motivo: String) -> Result<String, String> {
    confirmar_pendiente(id, &crate::adaptadores::reloj_sistema::RelojSistema, Some(motivo))
}

pub fn confirmar_pendiente(
    id: i64,
    reloj: &dyn crate::puertos::reloj::Reloj,
    motivo_de_descarte: Option<String>,
) -> Result<String, String> {
    use aplicacion::suscripciones::Confirmado;
    let hoy = reloj.hoy();
    let categoria = con_almacen(|a| Ok(aplicacion::suscripciones::categoria_de_suscripciones(a)?))?;
    let resultado = con_almacen(|a| {
        Ok(aplicacion::suscripciones::confirmar_pendiente(id, hoy, motivo_de_descarte.as_deref(), categoria, a)?)
    })?;
    Ok(match resultado {
        Confirmado::Asentado { plataforma, vencimiento } => {
            format!("Asentado el cargo de {} con fecha {}.", plataforma, vencimiento.format("%d/%m/%Y"))
        }
        Confirmado::Descartado { vencimiento, caso } => {
            format!("Período del {} descartado. Caso {}.", vencimiento.format("%d/%m/%Y"), caso)
        }
    })
}
