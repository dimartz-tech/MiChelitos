// Prevención de ventana de consola en Windows en producción
#![cfg_attr(
  all(not(debug_assertions), target_os = "windows"),
  windows_subsystem = "windows"
)]

mod db_sql;
mod migraciones;
mod respaldo;
mod correcciones;
mod ipc;
mod db_nosql;

mod adaptadores;
mod aplicacion;
mod dominio;
mod puertos;

#[cfg(test)]
mod caracterizacion;
#[cfg(test)]
mod gherkin;
#[cfg(test)]
mod caso_de_uso_gherkin;

use serde::{Serialize, Deserialize};
use serde_json::Value;
use chrono::{NaiveDate, Local, Datelike};

use dominio::dinero::{Dinero, Divisa, TasaCambio};
use dominio::gasto::MetodoPago;
use adaptadores::sqlite::catalogos::CatalogosSqlite;
use adaptadores::sqlite::cuentas::CuentasSqlite;
use adaptadores::sqlite::prestamos::PrestamosSqlite;
use adaptadores::sqlite::tarjetas::TarjetasSqlite;
use puertos::repositorios::{AlmacenInformales, AlmacenIngresos, AlmacenPrestamos, CatalogoDeCuentas};
use adaptadores::sqlite::gastos::AlmacenSqlite;
use puertos::repositorios::AlmacenCatalogos;
use aplicacion::registrar_gasto::{registrar_gasto, DatosGasto};
use aplicacion::revertir_gasto::revertir_gasto;
use aplicacion::registrar_pago_tarjeta::DatosPago;
use aplicacion::registrar_avance_de_efectivo::DatosAvance;
use aplicacion::cobrar_suscripcion::{cobrar_suscripcion, DatosCobro};
use aplicacion::transferir::{revertir_transferencia, transferir, DatosTransferencia};
use puertos::repositorios::RepositorioCuentas;
use dominio::tarjeta::MONEDA_LOCAL;
use aplicacion::liquidar_gasto::liquidar_gasto;
use aplicacion::registrar_bonificacion::{registrar_bonificacion, revertir_bonificacion, DatosBonificacion};
use dominio::bonificacion::Bonificacion;
use dominio::suscripcion::{Frecuencia, Suscripcion as SuscripcionDominio};
use puertos::reloj::Reloj;

// --- ESTRUCTURAS DTO (DATA TRANSFER OBJECTS) ---
#[derive(Serialize, Deserialize, Debug)]
pub struct Cliente {
    id: i64,
    rnc: String,
    nombre: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Ingreso {
    id: i64,
    numero_factura: String,
    cliente_id: i64,
    cliente_nombre: String,
    cliente_rnc: String,
    fecha_emision: String,
    estatus: String,
    monto_total: f64,
    porcentaje_retencion: f64,
    monto_retenido: f64,
    institucion_deposito: Option<String>,
    fecha_pago: Option<String>,
    monto_recibido: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IngresoInformal {
    id: i64,
    fecha: String,
    descripcion: String,
    monto: f64,
    estatus: String,
    institucion_deposito: Option<String>,
    fecha_pago: Option<String>,
    monto_recibido: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Gasto {
    id: i64,
    fecha: String,
    monto: f64,
    divisa: String,
    descripcion: String,
    categoria_id: i64,
    categoria_nombre: String,
    metodo_pago: String,
    costo_adicional: f64,
    tarjeta_id: Option<i64>,
    cuenta_ahorro_id: Option<i64>,
    // Conversión de divisa: NULL cuando no aplica.
    estado_conversion: Option<String>,
    monto_liquidado: Option<f64>,
    tasa_conversion: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BonificacionDto {
    id: i64,
    fecha: String,
    tarjeta_id: i64,
    entidad: String,
    nombre_tarjeta: String,
    monto: f64,
    divisa: String,
    concepto: String,
    gasto_id: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Categoria {
    id: i64,
    nombre: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Tarjeta {
    id: i64,
    entidad: String,
    nombre_tarjeta: String,
    limite_pesos: f64,
    limite_dolares: f64,
    limite_ajustado_pesos: Option<f64>,
    limite_ajustado_dolares: Option<f64>,
    limite_sobregiro_pesos: f64,
    limite_sobregiro_dolares: f64,
    balance_pesos: f64,
    balance_dolares: f64,
    balance_corte_pesos: f64,
    balance_corte_dolares: f64,
    fecha_corte: i32,
    fecha_limite_pago: i32,
    politica_liquidacion: String,
    // Enriquecidos
    limite_efectivo_pesos: f64,
    limite_efectivo_dolares: f64,
    disponible_pesos: f64,
    disponible_dolares: f64,
    alerta_corte: bool,
    alerta_pago: bool,
    dias_corte_msg: String,
    dias_pago_msg: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Suscripcion {
    id: i64,
    plataforma: String,
    monto: f64,
    tarjeta_id: i64,
    frecuencia: String,
    dia_facturacion: i32,
    fecha_ultimo_pago: Option<String>,
    divisa: String,
    entidad: String,
    nombre_tarjeta: String,
    /// Cuándo vence el próximo cobro. `None` significa que no se sabe, y
    /// entonces no se cobra.
    fecha_proximo_cobro: Option<String>,
    /// Los vencimientos que hay que confirmar a mano, si son dos o más.
    ///
    /// Viajan con la suscripción y no en una consulta aparte: un período que
    /// se perdió tiene que verse justo donde se mira la suscripción.
    pendientes: Vec<String>,
    /// Si el cargo cae dentro de los próximos siete días.
    avisa: bool,
    /// Por qué esta suscripción no llegará a cobrarse, si es el caso.
    ///
    /// Va en el mismo viaje que los datos y no en una consulta aparte: una
    /// suscripción parada tiene que verse justo donde se la mira.
    impedimento: Option<String>,
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

#[derive(Serialize, Deserialize, Debug)]
pub struct CuentaAhorro {
    id: i64,
    nombre: String,
    divisa: String,
    balance_actual: f64,
    /// Entidad con la que se mantiene la cuenta. La declara el titular.
    entidad: Option<String>,
    /// Tarifa fija por el servicio de pago de impuestos, si la hay pactada.
    comision_pago_impuestos: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TransaccionCuenta {
    id: i64,
    fecha: String,
    cuenta_origen_id: i64,
    cuenta_origen_nombre: String,
    cuenta_destino_id: i64,
    cuenta_destino_nombre: String,
    monto_origen: f64,
    monto_destino: f64,
    tasa_cambio: f64,
    cargo: f64,
    descripcion: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Prestamo {
    id: i64,
    tipo_prestamo: String,
    monto_prestamo: f64,
    institucion_financiera: String,
    tasa_actual: f64,
    cuotas_totales: Option<i32>,
    cuotas_pendientes: Option<i32>,
    monto_cuota: f64,
    dia_pago: i32,
    /// Capital que se debe hoy. Es el pasivo real, no el monto desembolsado.
    saldo_actual: f64,
    /// Solo en líneas revolventes, y solo si se ha declarado.
    limite_credito: Option<f64>,
    /// Tarjeta que cobra este financiamiento, si es una facilidad suya.
    tarjeta_id: Option<i64>,
    // Enriquecidos
    /// Nombre de la tarjeta vinculada, para no consultarla desde la vista.
    tarjeta_nombre: Option<String>,
    /// Día de corte. Es el de la tarjeta cuando la facilidad cuelga de una:
    /// no se guarda aparte porque mantener dos copias sincronizadas a mano es
    /// justo la clase de dato que se desincroniza.
    dia_corte: Option<i32>,
    /// Cupo por disponer. `None` cuando no es revolvente o falta el límite.
    disponible: Option<f64>,
    es_revolvente: bool,
    alerta_pago: bool,
    dias_pago_msg: String,
}

// --- COMANDOS: CATEGORÍAS ---
/// Corre `f` sobre el almacén de catálogos dentro de una transacción y la confirma si salió bien.
///
/// Es todo lo que los comandos de categorías y clientes tienen de infraestructura; las reglas están en
/// `dominio::catalogo` y `aplicacion::catalogos`.
fn con_catalogos<T>(
    f: impl FnOnce(&mut CatalogosSqlite) -> Result<T, String>,
) -> Result<T, String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let resultado = {
        let mut almacen = CatalogosSqlite::nuevo(&tx);
        f(&mut almacen)?
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(resultado)
}

#[tauri::command]
fn obtener_categorias() -> Result<Vec<Categoria>, String> {
    con_catalogos(|a| {
        Ok(a.categorias()?.into_iter().map(|c| Categoria { id: c.id, nombre: c.nombre }).collect())
    })
}

#[tauri::command]
fn crear_categoria(nombre: String) -> Result<Categoria, String> {
    con_catalogos(|a| {
        let c = aplicacion::catalogos::crear_categoria(&nombre, a)?;
        Ok(Categoria { id: c.id, nombre: c.nombre })
    })
}

#[tauri::command]
fn eliminar_categoria(id: i64) -> Result<(), String> {
    con_catalogos(|a| Ok(aplicacion::catalogos::eliminar_categoria(id, a)?))
}

// --- COMANDOS: GASTOS ---
#[tauri::command]
fn obtener_gastos() -> Result<Vec<Gasto>, String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT g.id, g.fecha, g.monto, g.divisa, g.descripcion, g.categoria_id, c.nombre, g.metodo_pago, g.costo_adicional, g.tarjeta_id, g.cuenta_ahorro_id, g.estado_conversion, g.monto_liquidado, g.tasa_conversion
         FROM gastos g
         JOIN categorias c ON g.categoria_id = c.id
         ORDER BY g.id DESC;"
    ).map_err(|e| e.to_string())?;

    let rows = stmt.query_map([], |row| {
        Ok(Gasto {
            id: row.get(0)?,
            fecha: row.get(1)?,
            monto: row.get(2)?,
            divisa: row.get(3)?,
            descripcion: row.get(4)?,
            categoria_id: row.get(5)?,
            categoria_nombre: row.get(6)?,
            metodo_pago: row.get(7)?,
            costo_adicional: row.get(8)?,
            tarjeta_id: row.get(9)?,
            cuenta_ahorro_id: row.get(10)?,
            estado_conversion: row.get(11)?,
            monto_liquidado: row.get(12)?,
            tasa_conversion: row.get(13)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r.map_err(|e| e.to_string())?);
    }
    Ok(list)
}

#[derive(Deserialize)]
struct GastoInput {
    fecha: String,
    monto: ipc::ImporteDecimal,
    divisa: String,
    descripcion: String,
    categoria_id: i64,
    metodo_pago: String,
    es_lbtr: bool,
    tarjeta_id: Option<i64>,
    cuenta_ahorro_id: Option<i64>,
    #[serde(default)]
    tasa_cambio: Option<f64>,
}

#[tauri::command]
fn crear_gasto(input: GastoInput) -> Result<i64, String> {
    // El comando queda reducido a traducción: interpreta la entrada, abre la
    // transacción, delega en el caso de uso y confirma. Ninguna regla vive ya
    // aquí.
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;

    // Se conserva la interpretación vigente de la divisa: la columna
    // gastos.divisa no tiene CHECK y el código solo distingue "USD".
    let divisa = if input.divisa == "USD" { Divisa::Usd } else { Divisa::Dop };
    let metodo = MetodoPago::desde_codigo(&input.metodo_pago);

    let datos = DatosGasto {
        fecha: input.fecha,
        // El importe llega como se escribió; se casa con la divisa declarada del gasto.
        monto: input.monto.con_divisa(divisa),
        descripcion: input.descripcion,
        categoria_id: input.categoria_id,
        metodo,
        metodo_texto: input.metodo_pago,
        es_lbtr: input.es_lbtr,
        tarjeta_id: input.tarjeta_id,
        cuenta_ahorro_id: input.cuenta_ahorro_id,
        // Una tasa de cero o ausente significa "sin conversión"; el caso de
        // uso la exigirá solo si las divisas realmente difieren.
        tasa_cambio: match input.tasa_cambio {
            Some(v) if v > 0.0 => Some(TasaCambio::nueva(v)?),
            _ => None,
        },
    };

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let id = {
        let mut almacen = AlmacenSqlite::nuevo(&tx);
        registrar_gasto(datos, &mut almacen)?
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(id)
}

// --- COMANDOS: INGRESOS FORMALES ---
/// Corre `f` sobre el almacén de gastos/cuentas/facturas dentro de una transacción y la confirma si salió bien.
fn con_almacen<T>(f: impl FnOnce(&mut AlmacenSqlite) -> Result<T, String>) -> Result<T, String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let resultado = {
        let mut almacen = AlmacenSqlite::nuevo(&tx);
        f(&mut almacen)?
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(resultado)
}

#[tauri::command]
fn obtener_ingresos() -> Result<Vec<Ingreso>, String> {
    con_almacen(|a| {
        Ok(AlmacenIngresos::ingresos(a)?
            .into_iter()
            .map(|i| Ingreso {
                id: i.id,
                numero_factura: i.numero_factura,
                cliente_id: i.cliente_id,
                cliente_nombre: i.cliente_nombre,
                cliente_rnc: i.cliente_rnc,
                fecha_emision: i.fecha_emision,
                estatus: i.estatus,
                monto_total: i.monto_total,
                porcentaje_retencion: i.porcentaje_retencion,
                monto_retenido: i.monto_retenido,
                institucion_deposito: i.institucion_deposito,
                fecha_pago: i.fecha_pago,
                monto_recibido: i.monto_recibido,
            })
            .collect())
    })
}

#[derive(Deserialize)]
struct IngresoInput {
    numero_factura: String,
    rnc_cliente: String,
    nombre_cliente: String,
    fecha_emision: String,
    monto_total: ipc::ImporteDecimal,
    porcentaje_retencion: f64,
}

#[tauri::command]
fn crear_ingreso(input: IngresoInput) -> Result<i64, String> {
    con_almacen(|a| {
        // El total llega como se escribió; el céntimo lo deciden esos dígitos. Una factura es en moneda local.
        Ok(aplicacion::ingresos::crear_ingreso(
            aplicacion::ingresos::DatosFactura {
                numero_factura: input.numero_factura,
                rnc_cliente: input.rnc_cliente,
                nombre_cliente: input.nombre_cliente,
                fecha_emision: input.fecha_emision,
                monto_total: input.monto_total.con_divisa(MONEDA_LOCAL),
                porcentaje_retencion: input.porcentaje_retencion,
            },
            a,
        )?)
    })
}

#[tauri::command]
fn marcar_ingreso_pagado(
    id: i64,
    cuenta_ahorro_id: i64,
    fecha: String,
    monto_recibido: ipc::ImporteDecimal,
) -> Result<(), String> {
    con_almacen(|a| {
        // Una factura se emite en moneda local, de modo que su cobro también.
        Ok(aplicacion::ingresos::marcar_ingreso_pagado(
            id,
            cuenta_ahorro_id,
            &fecha,
            monto_recibido.con_divisa(MONEDA_LOCAL),
            a,
        )?)
    })
}

// --- COMANDOS: INGRESOS INFORMALES ---
#[tauri::command]
fn obtener_ingresos_informales() -> Result<Vec<IngresoInformal>, String> {
    con_almacen(|a| {
        Ok(AlmacenInformales::informales(a)?
            .into_iter()
            .map(|i| IngresoInformal {
                id: i.id,
                fecha: i.fecha,
                descripcion: i.descripcion,
                monto: i.monto,
                estatus: i.estatus,
                institucion_deposito: i.institucion_deposito,
                fecha_pago: i.fecha_pago,
                monto_recibido: i.monto_recibido,
            })
            .collect())
    })
}

#[tauri::command]
fn crear_ingreso_informal(fecha: String, descripcion: String, monto: ipc::ImporteDecimal) -> Result<i64, String> {
    // El importe llega como se escribió y el núcleo decide el céntimo con esos dígitos (`1.005` sube a 1.01).
    con_almacen(|a| {
        Ok(aplicacion::informales::crear_ingreso_informal(&fecha, &descripcion, monto.con_divisa(MONEDA_LOCAL), a)?)
    })
}

#[tauri::command]
fn marcar_informal_pagado(
    id: i64,
    cuenta_ahorro_id: i64,
    fecha: String,
    monto_recibido: ipc::ImporteDecimal,
) -> Result<(), String> {
    con_almacen(|a| {
        // Un ingreso informal es en moneda local, de modo que su cobro también.
        Ok(aplicacion::informales::marcar_informal_pagado(
            id,
            cuenta_ahorro_id,
            &fecha,
            monto_recibido.con_divisa(MONEDA_LOCAL),
            a,
        )?)
    })
}

// --- COMANDOS: TARJETAS ---
/// Como `con_catalogos`, para las tarjetas.
fn con_tarjetas<T>(f: impl FnOnce(&mut TarjetasSqlite) -> Result<T, String>) -> Result<T, String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let resultado = {
        let mut almacen = TarjetasSqlite::nuevo(&tx);
        f(&mut almacen)?
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(resultado)
}

#[tauri::command]
fn obtener_tarjetas() -> Result<Vec<Tarjeta>, String> {
    let dia_actual = Local::now().day() as i32;
    con_tarjetas(|a| {
        Ok(aplicacion::tarjetas::listar_tarjetas(dia_actual, a)?
            .into_iter()
            .map(|t| Tarjeta {
                id: t.leida.id,
                entidad: t.leida.entidad,
                nombre_tarjeta: t.leida.nombre_tarjeta,
                limite_pesos: t.leida.limite_pesos,
                limite_dolares: t.leida.limite_dolares,
                limite_ajustado_pesos: t.leida.limite_ajustado_pesos,
                limite_ajustado_dolares: t.leida.limite_ajustado_dolares,
                limite_sobregiro_pesos: t.leida.limite_sobregiro_pesos,
                limite_sobregiro_dolares: t.leida.limite_sobregiro_dolares,
                balance_pesos: t.leida.balance_pesos,
                balance_dolares: t.leida.balance_dolares,
                balance_corte_pesos: t.leida.balance_corte_pesos,
                balance_corte_dolares: t.leida.balance_corte_dolares,
                fecha_corte: t.leida.fecha_corte,
                fecha_limite_pago: t.leida.fecha_limite_pago,
                politica_liquidacion: t.politica_liquidacion,
                limite_efectivo_pesos: t.limite_efectivo_pesos,
                limite_efectivo_dolares: t.limite_efectivo_dolares,
                disponible_pesos: t.disponible_pesos,
                disponible_dolares: t.disponible_dolares,
                alerta_corte: t.alerta_corte,
                alerta_pago: t.alerta_pago,
                dias_corte_msg: t.dias_corte_msg,
                dias_pago_msg: t.dias_pago_msg,
            })
            .collect())
    })
}

#[tauri::command]
fn crear_tarjeta(
    entidad: String,
    nombre: String,
    limite_pesos: ipc::ImporteDecimal,
    limite_dolares: ipc::ImporteDecimal,
    sobregiro_pesos: ipc::ImporteDecimal,
    sobregiro_dolares: ipc::ImporteDecimal,
    balance_pesos: ipc::ImporteDecimal,
    balance_dolares: ipc::ImporteDecimal,
    balance_corte_pesos: ipc::ImporteDecimal,
    balance_corte_dolares: ipc::ImporteDecimal,
    corte: i32,
    pago: i32
) -> Result<i64, String> {
    con_tarjetas(|a| {
        // Cada importe llega como se escribió: el céntimo lo deciden esos dígitos, no un número ya redondeado.
        Ok(aplicacion::tarjetas::crear_tarjeta(
            aplicacion::tarjetas::DatosTarjetaNueva {
                entidad,
                nombre_tarjeta: nombre,
                limite_pesos: limite_pesos.con_divisa(Divisa::Dop),
                limite_dolares: limite_dolares.con_divisa(Divisa::Usd),
                sobregiro_pesos: sobregiro_pesos.con_divisa(Divisa::Dop),
                sobregiro_dolares: sobregiro_dolares.con_divisa(Divisa::Usd),
                balance_pesos: balance_pesos.con_divisa(Divisa::Dop),
                balance_dolares: balance_dolares.con_divisa(Divisa::Usd),
                balance_corte_pesos: balance_corte_pesos.con_divisa(Divisa::Dop),
                balance_corte_dolares: balance_corte_dolares.con_divisa(Divisa::Usd),
                fecha_corte: corte,
                fecha_limite_pago: pago,
            },
            a,
        )?)
    })
}

#[tauri::command]
fn actualizar_limites_tarjeta(
    id: i64,
    limite_pesos: ipc::ImporteDecimal,
    limite_dolares: ipc::ImporteDecimal,
    sobregiro_pesos: ipc::ImporteDecimal,
    sobregiro_dolares: ipc::ImporteDecimal,
    balance_corte_pesos: ipc::ImporteDecimal,
    balance_corte_dolares: ipc::ImporteDecimal,
    // `None` es «sin ajuste»; cero es un tope deliberado.
    limite_ajustado_pesos: Option<ipc::ImporteDecimal>,
    limite_ajustado_dolares: Option<ipc::ImporteDecimal>,
    politica_liquidacion: Option<String>
) -> Result<(), String> {
    con_tarjetas(|a| {
        Ok(aplicacion::tarjetas::actualizar_limites(
            aplicacion::tarjetas::DatosLimites {
                id,
                limite_pesos: limite_pesos.con_divisa(Divisa::Dop),
                limite_dolares: limite_dolares.con_divisa(Divisa::Usd),
                sobregiro_pesos: sobregiro_pesos.con_divisa(Divisa::Dop),
                sobregiro_dolares: sobregiro_dolares.con_divisa(Divisa::Usd),
                balance_corte_pesos: balance_corte_pesos.con_divisa(Divisa::Dop),
                balance_corte_dolares: balance_corte_dolares.con_divisa(Divisa::Usd),
                limite_ajustado_pesos: limite_ajustado_pesos.map(|i| i.con_divisa(Divisa::Dop)),
                limite_ajustado_dolares: limite_ajustado_dolares.map(|i| i.con_divisa(Divisa::Usd)),
                politica_liquidacion,
            },
            a,
        )?)
    })
}

#[tauri::command]
fn registrar_pago_tarjeta(
    id: i64,
    fecha: String,
    monto: ipc::ImporteDecimal,
    divisa: String,
    cuenta_ahorro_id: Option<i64>,
    tasa_cambio: f64
) -> Result<(), String> {
    con_almacen(|a| {
        Ok(aplicacion::abonos::registrar_abono(
            DatosPago {
                tarjeta_id: id,
                fecha,
                // El céntimo lo deciden los dígitos escritos; la divisa es la declarada por el abono.
                monto: monto.con_divisa(Divisa::desde_codigo(&divisa)?),
                cuenta_ahorro_id,
                // Una tasa de cero es como la interfaz dice «no aplica».
                tasa_cambio: if tasa_cambio > 0.0 {
                    Some(TasaCambio::nueva(tasa_cambio)?)
                } else {
                    None
                },
            },
            a,
        )?)
    })
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AbonoTarjeta {
    id: i64,
    fecha_pago: String,
    monto_pagado: f64,
    divisa: String,
    cuenta_ahorro_id: Option<i64>,
    cuenta_nombre: Option<String>,
    tasa_cambio: Option<f64>,
}

/// Abonos registrados a una tarjeta, del más reciente al más antiguo.
#[tauri::command]
fn obtener_abonos_tarjeta(tarjeta_id: i64) -> Result<Vec<AbonoTarjeta>, String> {
    con_almacen(|a| {
        Ok(aplicacion::abonos::listar_abonos(tarjeta_id, a)?
            .into_iter()
            .map(|p| AbonoTarjeta {
                id: p.id,
                fecha_pago: p.fecha_pago,
                monto_pagado: p.monto_pagado,
                divisa: p.divisa,
                cuenta_ahorro_id: p.cuenta_ahorro_id,
                cuenta_nombre: p.cuenta_nombre,
                tasa_cambio: p.tasa_cambio,
            })
            .collect())
    })
}

/// Deshace un abono a tarjeta.
///
/// Traducción pura: la reversión vive en el caso de uso, igual que la de los
/// gastos. Devuelve lo que se deshizo para que la interfaz pueda decirlo en
/// vez de limitarse a confirmar que algo pasó.
#[tauri::command]
fn revertir_abono_tarjeta(id: i64, motivo: String) -> Result<String, String> {
    con_almacen(|a| {
        let r = aplicacion::abonos::revertir_abono(id, &motivo, a)?;
        let resumen = match r.revertido.devuelto_a_la_cuenta {
            Some(d) => format!(
                "Se repusieron {} {:.2} a la deuda y volvieron {} {:.2} a la cuenta.",
                r.revertido.deuda_restituida.divisa().codigo(),
                r.revertido.deuda_restituida.unidades(),
                d.divisa().codigo(),
                d.unidades()
            ),
            None => format!(
                "Se repusieron {} {:.2} a la deuda. El abono no tenía cuenta asociada.",
                r.revertido.deuda_restituida.divisa().codigo(),
                r.revertido.deuda_restituida.unidades()
            ),
        };
        Ok(format!("{} Caso {}.", resumen, r.caso))
    })
}

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
    monto: f64,
    cargo: f64,
    a_la_tarjeta: f64,
    a_la_cuenta: f64,
}

/// Calcula lo que un avance movería, sin guardar nada.
///
/// Existe para que la interfaz enseñe las cifras **antes** de confirmar sin
/// tener que repetir la regla en JavaScript: una regla en el HTML es una regla
/// sin pruebas, y la cifra que se confirma debe ser exactamente la que se
/// asienta.
#[tauri::command]
fn simular_avance_efectivo(
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
fn registrar_avance_efectivo(
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
    id: i64,
    fecha: String,
    monto: f64,
    divisa: String,
    tipo_cargo: String,
    tasa: Option<f64>,
    cargo: f64,
    cuenta_ahorro_id: i64,
    cuenta_nombre: String,
    nota: Option<String>,
}

/// Avances de una tarjeta, del más reciente al más antiguo.
#[tauri::command]
fn obtener_avances_tarjeta(tarjeta_id: i64) -> Result<Vec<AvanceEfectivo>, String> {
    con_almacen(|a| {
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
fn revertir_avance_efectivo(id: i64, motivo: String) -> Result<String, String> {
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

// --- COMANDOS: SUSCRIPCIONES ---
#[tauri::command]
fn obtener_suscripciones() -> Result<Vec<Suscripcion>, String> {
    suscripciones_con_aviso(&crate::adaptadores::reloj_sistema::RelojSistema)
}

/// Las suscripciones, con el aviso ya resuelto para ese «hoy».
pub fn suscripciones_con_aviso(reloj: &dyn Reloj) -> Result<Vec<Suscripcion>, String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT s.id, s.plataforma, s.monto, s.tarjeta_id, s.frecuencia, s.dia_facturacion, s.fecha_ultimo_pago, s.divisa, t.entidad, t.nombre_tarjeta, s.fecha_proximo_cobro
         FROM suscripciones s
         JOIN tarjetas t ON s.tarjeta_id = t.id
         ORDER BY s.plataforma ASC;"
    ).map_err(|e| e.to_string())?;

    let rows = stmt.query_map([], |row| {
        Ok(Suscripcion {
            id: row.get(0)?,
            plataforma: row.get(1)?,
            monto: row.get(2)?,
            tarjeta_id: row.get(3)?,
            frecuencia: row.get(4)?,
            dia_facturacion: row.get(5)?,
            fecha_ultimo_pago: row.get(6)?,
            divisa: row.get(7)?,
            entidad: row.get(8)?,
            nombre_tarjeta: row.get(9)?,
            fecha_proximo_cobro: row.get(10)?,
            pendientes: Vec::new(), // se calculan abajo, con el reloj y la
            avisa: false,           // regla de dominio
            impedimento: None,
        })
    }).map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r.map_err(|e| e.to_string())?);
    }
    drop(stmt);

    // El aviso se resuelve aquí y no en la vista: es una regla, y las reglas
    // no viven en el HTML. La vista solo pinta el `bool`.
    let hoy = reloj.hoy();
    for s in &mut list {
        let regla = dominio_de(s);
        s.avisa = regla.as_ref().is_some_and(|d| d.avisa(hoy));
        s.impedimento = regla
            .as_ref()
            .and_then(|d| d.impedimento())
            .map(|i| i.explicacion().to_string());
        s.pendientes = regla
            .as_ref()
            .map(|d| {
                d.pendientes_de_confirmar(hoy)
                    .iter()
                    .map(|f| f.format("%d/%m/%Y").to_string())
                    .collect()
            })
            .unwrap_or_default();
    }

    Ok(list)
}

/// La vista de dominio de una fila de `suscripciones`.
///
/// Un solo sitio donde se traduce lo almacenado a la regla, para que el
/// cobro y el aviso no puedan discrepar sobre qué día vence una suscripción.
fn dominio_de(s: &Suscripcion) -> Option<SuscripcionDominio> {
    Some(SuscripcionDominio {
        frecuencia: Frecuencia::desde_codigo(&s.frecuencia)?,
        proximo_cobro: s.fecha_proximo_cobro.as_deref().and_then(fecha_desde_texto),
        dia_ancla: s.dia_facturacion.max(1) as u32,
    })
}

/// `dd/mm/aaaa` — el formato de la aplicación.
fn fecha_desde_texto(texto: &str) -> Option<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(texto, "%d/%m/%Y").ok()
}

#[tauri::command]
fn crear_suscripcion(plataforma: String, monto: ipc::ImporteDecimal, tarjeta_id: i64, frecuencia: String, dia_facturacion: i32, divisa: String, fecha_proximo_cobro: Option<String>) -> Result<i64, String> {
    let proximo = validar_proximo_cobro(fecha_proximo_cobro)?;
    let monto = validar_condiciones_de_suscripcion(monto, &frecuencia, dia_facturacion, &divisa)?;
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO suscripciones (plataforma, monto, tarjeta_id, frecuencia, dia_facturacion, divisa, fecha_proximo_cobro) VALUES (?, ?, ?, ?, ?, ?, ?);",
        (plataforma, monto, tarjeta_id, frecuencia, dia_facturacion, divisa, proximo)
    ).map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

/// Las condiciones de una suscripción: cuánto, en qué divisa, cada cuánto y
/// qué día. Devuelve el importe **en unidades**, ya exacto al céntimo.
///
/// Antes solo el `CHECK` del esquema atajaba algo, con su mensaje crudo, y
/// atajaba poco: un importe **negativo o cero** entraba, y cobrarlo abonaba a
/// la tarjeta cada período; un día de facturación fuera de 1 a 31 también.
/// Se valida aquí para que el titular lea qué falla y no una restricción.
fn validar_condiciones_de_suscripcion(
    monto: ipc::ImporteDecimal,
    frecuencia: &str,
    dia_facturacion: i32,
    divisa: &str,
) -> Result<f64, String> {
    let divisa = Divisa::desde_codigo(divisa)?;
    let importe = monto.con_divisa(divisa);
    if importe.es_cero() || importe.es_negativo() {
        return Err(dominio::errores::ErrorDominio::SuscripcionSinImporte.to_string());
    }
    if Frecuencia::desde_codigo(frecuencia).is_none() {
        return Err(dominio::errores::ErrorDominio::FrecuenciaDesconocida {
            codigo: frecuencia.to_string(),
        }
        .to_string());
    }
    if !(1..=31).contains(&dia_facturacion) {
        return Err(dominio::errores::ErrorDominio::DiaDeFacturacionInvalido { dia: dia_facturacion }
            .to_string());
    }
    Ok(importe.unidades())
}

/// La fecha del próximo cobro tiene que entenderse.
///
/// Se rechaza una fecha ilegible en vez de guardarla: una suscripción con una
/// fecha que no se puede leer no cobra, pero **aparenta estar configurada**, y
/// eso es peor que el hueco visible.
///
/// Ya no distingue por frecuencia: desde que la fecha manda, la mensual la
/// usa igual que la anual.
fn validar_proximo_cobro(fecha: Option<String>) -> Result<Option<String>, String> {
    match fecha.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(texto) => match fecha_desde_texto(texto) {
            Some(_) => Ok(Some(texto.to_string())),
            None => Err(format!(
                "La fecha del próximo cobro «{}» no se entiende. Se espera dd/mm/aaaa.",
                texto
            )),
        },
    }
}

/// Edita una suscripción **conservando `fecha_ultimo_pago`**.
///
/// Esa preservación es el motivo de existir del comando: la única alternativa
/// hasta ahora era borrar y volver a crear, lo que reinicia el marcador de
/// idempotencia y hace que el siguiente procesamiento cobre otra vez el mismo
/// mes. Los cargos ya realizados son gastos independientes y no se tocan: lo
/// que se edita es la configuración de los cobros futuros.
#[tauri::command]
fn actualizar_suscripcion(
    id: i64,
    plataforma: String,
    monto: ipc::ImporteDecimal,
    tarjeta_id: i64,
    frecuencia: String,
    dia_facturacion: i32,
    divisa: String,
    fecha_proximo_cobro: Option<String>,
) -> Result<(), String> {
    let proximo = validar_proximo_cobro(fecha_proximo_cobro)?;
    let monto = validar_condiciones_de_suscripcion(monto, &frecuencia, dia_facturacion, &divisa)?;
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let filas = conn
        .execute(
            "UPDATE suscripciones SET plataforma = ?, monto = ?, tarjeta_id = ?, frecuencia = ?, dia_facturacion = ?, divisa = ?, fecha_proximo_cobro = ? WHERE id = ?;",
            (plataforma, monto, tarjeta_id, frecuencia, dia_facturacion, divisa, proximo, id),
        )
        .map_err(|e| e.to_string())?;
    if filas == 0 {
        return Err("No se encontró la suscripción que se intenta editar.".to_string());
    }
    Ok(())
}

/// Pone a mano la fecha del próximo cobro.
///
/// Es la salida cuando una suscripción se queda sin fecha y por tanto parada.
/// Pide la fecha en lugar de deducirla, por lo mismo que la anual: deducir un
/// vencimiento con datos que no bastan es lo que produjo los defectos de esta
/// fase.
#[tauri::command]
fn corregir_proximo_cobro(id: i64, fecha: String) -> Result<(), String> {
    let fecha = fecha.trim();
    if fecha_desde_texto(fecha).is_none() {
        return Err(format!(
            "«{}» no se entiende como fecha. Se espera dd/mm/aaaa.",
            fecha
        ));
    }

    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let filas = conn
        .execute(
            "UPDATE suscripciones SET fecha_proximo_cobro = ? WHERE id = ?;",
            (fecha, id),
        )
        .map_err(|e| e.to_string())?;
    if filas == 0 {
        return Err("No se encontró la suscripción que se intenta corregir.".to_string());
    }
    Ok(())
}

#[tauri::command]
fn eliminar_suscripcion(id: i64) -> Result<(), String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM suscripciones WHERE id = ?;", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn procesar_suscripciones() -> Result<Vec<String>, String> {
    procesar_suscripciones_con(&crate::adaptadores::reloj_sistema::RelojSistema)
}

/// El cobro automático, con el «hoy» que le den.
///
/// Se separa del comando para que las pruebas puedan fijar la fecha. Hasta
/// ahora leía `Local::now()` por dentro, y eso hacía **imposible escribir la
/// prueba que más falta hace**: que una suscripción *no* se cobre antes de su
/// día. `s1` tiene que usar el día 1 precisamente por eso —es el único que
/// está siempre alcanzado—, de modo que la red cubre el cobro y no cubre la
/// abstención.
///
/// El puerto `Reloj` existe desde la Fase 0 para esto y no lo usaba nadie.
pub fn procesar_suscripciones_con(reloj: &dyn crate::puertos::reloj::Reloj) -> Result<Vec<String>, String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let hoy = reloj.hoy();
    let categoria = categoria_de_suscripciones(&conn)?;

    let mut mensajes = Vec::new();
    for sub in leer_suscripciones(&conn)? {
        let Some(regla) = regla_de(&sub) else { continue };
        // **Solo si hay exactamente un período vencido.** Con varios, la
        // aplicación no sabe si el proveedor los cobró ni si la suscripción
        // siguió activa: se ofrecen para confirmar en vez de fabricarse.
        let Some(vencimiento) = regla.cobro_automatico(hoy) else { continue };

        let tx = conn.transaction().map_err(|e| e.to_string())?;
        asentar_cargo(&tx, &sub, &regla, vencimiento, categoria)?;
        tx.commit().map_err(|e| e.to_string())?;

        mensajes.push(format!(
            "Cargo automático realizado para {} ({} {:.2}) con fecha {}",
            sub.plataforma,
            sub.divisa,
            sub.monto,
            vencimiento.format("%d/%m/%Y")
        ));
    }
    Ok(mensajes)
}

struct SubRecord {
    id: i64,
    plataforma: String,
    monto: f64,
    tarjeta_id: i64,
    frecuencia: String,
    dia_facturacion: i32,
    divisa: String,
    fecha_proximo_cobro: Option<String>,
}

fn leer_suscripciones(conn: &rusqlite::Connection) -> Result<Vec<SubRecord>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, plataforma, monto, tarjeta_id, frecuencia, dia_facturacion,
                    divisa, fecha_proximo_cobro
             FROM suscripciones ORDER BY id;",
        )
        .map_err(|e| e.to_string())?;
    let filas = stmt
        .query_map([], |row| {
            Ok(SubRecord {
                id: row.get(0)?,
                plataforma: row.get(1)?,
                monto: row.get(2)?,
                tarjeta_id: row.get(3)?,
                frecuencia: row.get(4)?,
                dia_facturacion: row.get(5)?,
                divisa: row.get(6)?,
                fecha_proximo_cobro: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut v = Vec::new();
    for f in filas {
        v.push(f.map_err(|e| e.to_string())?);
    }
    Ok(v)
}

/// Una frecuencia que el `CHECK` no admite no debería existir. Si existiera,
/// no cobrar es lo que hacía la cadena de `if` anterior al no coincidir con
/// ninguna rama.
fn regla_de(sub: &SubRecord) -> Option<SuscripcionDominio> {
    Some(SuscripcionDominio {
        frecuencia: Frecuencia::desde_codigo(&sub.frecuencia)?,
        proximo_cobro: sub.fecha_proximo_cobro.as_deref().and_then(fecha_desde_texto),
        dia_ancla: sub.dia_facturacion.max(1) as u32,
    })
}

/// Dónde va el gasto de una suscripción.
///
/// Busca «Suscripciones» y, si no está, «Otros» — ambas nacen en la siembra
/// inicial, pero el titular puede renombrarlas o borrarlas desde la propia
/// aplicación. Antes, si las dos faltaban, el gasto caía en **el
/// identificador 1 literal**, sea cual sea la categoría que lo tenga hoy: un
/// cargo de suscripción podía terminar archivado como alquiler o gasolina sin
/// que nada lo dijera.
///
/// Ahora, si ninguna existe, se **crea** «Suscripciones» en el momento. No es
/// una tercera búsqueda más: es dejar de improvisar con lo que haya en la
/// posición 1 y garantizar en su lugar una categoría que sí describe lo que
/// contiene.
fn categoria_de_suscripciones(conn: &rusqlite::Connection) -> Result<i64, String> {
    for nombre in ["suscripciones", "otros"] {
        if let Ok(id) = conn.query_row(
            "SELECT id FROM categorias WHERE LOWER(nombre) = ?;",
            [nombre],
            |r| r.get(0),
        ) {
            return Ok(id);
        }
    }

    conn.execute(
        "INSERT INTO categorias (nombre) VALUES ('Suscripciones');",
        [],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

/// Asienta un cargo de suscripción **con la fecha de su vencimiento**.
///
/// La fecha es la del período, no la del día en que se ejecuta. Antes se
/// usaba «hoy», y por eso en la base real un cargo de Google One —que factura
/// el día 9— figura asentado el 11: cuadrarlo contra el estado de cuenta era
/// más difícil de lo necesario.
///
/// Es la misma función para el cobro automático y para confirmar un período
/// pendiente, de modo que los dos caminos no puedan divergir.
fn asentar_cargo(
    tx: &rusqlite::Transaction,
    sub: &SubRecord,
    regla: &SuscripcionDominio,
    vencimiento: chrono::NaiveDate,
    categoria: i64,
) -> Result<(), String> {
    let fecha = vencimiento.format("%d/%m/%Y").to_string();

    // El cobro es un consumo con tarjeta: pasa por el mismo caso de uso que
    // cualquier gasto, y hereda su regla de divisa y la política de la tarjeta.
    // Antes se escribía SQL directo, con el importe como `f64`, y se saltaba
    // que un consumo en divisa quede pendiente de liquidar.
    let divisa = Divisa::desde_codigo(&sub.divisa)?;
    let monto = Dinero::nuevo(sub.monto, divisa)?;
    {
        let mut almacen = AlmacenSqlite::nuevo(tx);
        cobrar_suscripcion(
            DatosCobro {
                plataforma: sub.plataforma.clone(),
                monto,
                tarjeta_id: sub.tarjeta_id,
                fecha: fecha.clone(),
                categoria_id: categoria,
            },
            &mut almacen,
        )?;
    }

    avanzar_el_puntero(tx, sub.id, regla, vencimiento, Some(&fecha))
}

/// Mueve `fecha_proximo_cobro` al período siguiente.
///
/// Se calcula **desde el vencimiento saldado**, no desde hoy: si la
/// aplicación se abre tarde, el vencimiento siguiente sigue siendo el del
/// proveedor y no el del descuido.
fn avanzar_el_puntero(
    tx: &rusqlite::Transaction,
    id: i64,
    regla: &SuscripcionDominio,
    saldado: chrono::NaiveDate,
    marca_de_cobro: Option<&str>,
) -> Result<(), String> {
    let siguiente = regla
        .siguiente_vencimiento(saldado)
        .map(|f| f.format("%d/%m/%Y").to_string());

    tx.execute(
        "UPDATE suscripciones SET fecha_proximo_cobro = ? WHERE id = ?;",
        (&siguiente, id),
    )
    .map_err(|e| e.to_string())?;

    if let Some(marca) = marca_de_cobro {
        tx.execute(
            "UPDATE suscripciones SET fecha_ultimo_pago = ? WHERE id = ?;",
            (marca, id),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Asienta el período pendiente más antiguo, tras confirmarlo el titular.
#[tauri::command]
fn asentar_periodo_pendiente(id: i64) -> Result<String, String> {
    confirmar_pendiente(id, &crate::adaptadores::reloj_sistema::RelojSistema, None)
}

/// Da por no cobrado el período pendiente más antiguo y pasa al siguiente.
///
/// **Exige un motivo escrito**, por lo mismo que las correcciones: descartar
/// un período es afirmar que el proveedor no lo cobró, y esa afirmación la
/// hace alguien mirando un estado de cuenta. Si dentro de seis meses la cifra
/// anual no cuadra, esto es lo que dirá por qué.
#[tauri::command]
fn descartar_periodo_pendiente(id: i64, motivo: String) -> Result<String, String> {
    confirmar_pendiente(id, &crate::adaptadores::reloj_sistema::RelojSistema, Some(motivo))
}

pub fn confirmar_pendiente(
    id: i64,
    reloj: &dyn crate::puertos::reloj::Reloj,
    motivo_de_descarte: Option<String>,
) -> Result<String, String> {
    let hoy = reloj.hoy();
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let categoria = categoria_de_suscripciones(&conn)?;

    let sub = leer_suscripciones(&conn)?
        .into_iter()
        .find(|s| s.id == id)
        .ok_or("No se encontró la suscripción.")?;
    let regla = regla_de(&sub).ok_or("La suscripción tiene una frecuencia que no se reconoce.")?;

    // Se actúa sobre el **más antiguo**, y solo si de verdad hay varios: con
    // uno, el cobro automático es quien debe encargarse, y dejar que esta vía
    // lo tocara abriría un segundo camino para el mismo hecho.
    let pendientes = regla.pendientes_de_confirmar(hoy);
    let vencimiento = *pendientes
        .first()
        .ok_or("Esta suscripción no tiene períodos pendientes de confirmar.")?;

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let resumen = match motivo_de_descarte {
        None => {
            asentar_cargo(&tx, &sub, &regla, vencimiento, categoria)?;
            format!(
                "Asentado el cargo de {} con fecha {}.",
                sub.plataforma,
                vencimiento.format("%d/%m/%Y")
            )
        }
        Some(motivo) => {
            let caso = correcciones::registrar(
                &tx,
                correcciones::Correccion {
                    tipo: "período de suscripción",
                    referencia_id: sub.id,
                    descripcion: format!(
                        "{} — período del {}",
                        sub.plataforma,
                        vencimiento.format("%d/%m/%Y")
                    ),
                    importe: Some(sub.monto),
                    divisa: Some(sub.divisa.clone()),
                    motivo: &motivo,
                },
            )?;
            avanzar_el_puntero(&tx, sub.id, &regla, vencimiento, None)?;
            format!(
                "Período del {} descartado. Caso {}.",
                vencimiento.format("%d/%m/%Y"),
                caso
            )
        }
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(resumen)
}

// --- COMANDOS: CAPITAL (NoSQL) ---
#[tauri::command]
fn obtener_capital() -> Result<Value, String> {
    let mut data = db_nosql::leer_coleccion("capital")?;
    let hoy = Local::now().naive_local().date();

    for coleccion in ["certificados", "bolsa"] {
        if let Some(entradas) = data.get_mut(coleccion).and_then(|v| v.as_array_mut()) {
            for entrada in entradas {
                marcar_alerta_de_vencimiento(entrada, hoy);
            }
        }
    }

    // Los totales se suman aquí, en centavos, y no en la vista: una suma de
    // decimales en JavaScript arrastra ruido y es una regla sin pruebas.
    let totales = aplicacion::guardar_capital::totales_de(&data);
    data["totales"] = aplicacion::guardar_capital::totales_como_json(&totales);

    Ok(data)
}

/// Añade al `Value` de una entrada las tres claves de presentación que
/// dependen de la fecha de hoy. **Ninguna sobrevive a un guardado**: ver
/// `retirar_campos_calculados` y `dominio::capital`.
fn marcar_alerta_de_vencimiento(entrada: &mut Value, hoy: NaiveDate) {
    entrada["alerta_vencimiento"] = serde_json::Value::Bool(false);
    let Some(venc_str) = entrada.get("vencimiento").and_then(|v| v.as_str()) else {
        return;
    };
    let Ok(venc_date) = NaiveDate::parse_from_str(venc_str, "%d/%m/%Y") else {
        return;
    };
    let alerta = dominio::capital::calcular(venc_date, hoy);
    entrada["dias_restantes"] = serde_json::Value::Number(alerta.dias_restantes.into());
    entrada["alerta_vencimiento"] = serde_json::Value::Bool(alerta.activa());
    if let Some(mensaje) = alerta.mensaje() {
        entrada["alerta_msg"] = serde_json::Value::String(mensaje);
    }
}

#[tauri::command]
fn guardar_capital(data: Value) -> Result<(), String> {
    // Se exige lo que entra o cambia contra lo que ya estaba guardado; ver
    // `aplicacion::guardar_capital`.
    // Si el archivo existe pero no se puede leer, **no se guarda**: sustituirlo por lo que llegue destruiría lo que hubiera.
    let guardado = db_nosql::leer_coleccion("capital")?;
    let limpio = aplicacion::guardar_capital::preparar_para_guardar(data, &guardado)
        .map_err(|e| e.to_string())?;
    db_nosql::guardar_coleccion("capital", &limpio)
}

// --- COMANDOS: DEUDAS Y FINANCIAMIENTOS ---
/// Como `con_catalogos`, para los financiamientos.
fn con_prestamos<T>(f: impl FnOnce(&mut PrestamosSqlite) -> Result<T, String>) -> Result<T, String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let resultado = {
        let mut almacen = PrestamosSqlite::nuevo(&tx);
        f(&mut almacen)?
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(resultado)
}

/// La fecha de hoy como la guarda la aplicación: `dd/mm/aaaa`.
fn hoy_dd_mm_aaaa() -> String {
    Local::now().format("%d/%m/%Y").to_string()
}

#[tauri::command]
fn obtener_prestamos() -> Result<Vec<Prestamo>, String> {
    let dia_actual = Local::now().day() as i32;
    con_prestamos(|a| {
        Ok(aplicacion::prestamos::listar_prestamos(dia_actual, a)?
            .into_iter()
            .map(|p| Prestamo {
                id: p.leido.id,
                tipo_prestamo: p.leido.tipo_prestamo,
                monto_prestamo: p.leido.monto_prestamo,
                institucion_financiera: p.leido.institucion_financiera,
                tasa_actual: p.leido.tasa_actual,
                cuotas_totales: p.leido.cuotas_totales,
                cuotas_pendientes: p.leido.cuotas_pendientes,
                monto_cuota: p.leido.monto_cuota,
                dia_pago: p.dia_pago,
                saldo_actual: p.saldo_actual,
                limite_credito: p.leido.limite_credito,
                tarjeta_id: p.leido.tarjeta_id,
                tarjeta_nombre: p.leido.tarjeta_nombre,
                dia_corte: p.leido.tarjeta_fecha_corte,
                disponible: p.disponible,
                es_revolvente: p.es_revolvente,
                alerta_pago: p.alerta_pago,
                dias_pago_msg: p.dias_pago_msg,
            })
            .collect())
    })
}

#[derive(Deserialize)]
struct PrestamoInput {
    tipo_prestamo: String,
    monto_prestamo: ipc::ImporteDecimal,
    institucion_financiera: String,
    tasa_actual: f64,
    cuotas_totales: Option<i32>,
    cuotas_pendientes: Option<i32>,
    monto_cuota: ipc::ImporteDecimal,
    dia_pago: i32,
    /// Capital pendiente hoy. Si no se indica se asume el monto íntegro, que
    /// es lo correcto en un financiamiento recién desembolsado.
    saldo_actual: Option<ipc::ImporteDecimal>,
    /// Solo en líneas revolventes: el cupo aprobado.
    limite_credito: Option<ipc::ImporteDecimal>,
}

#[tauri::command]
fn crear_prestamo(input: PrestamoInput) -> Result<i64, String> {
    con_prestamos(|a| {
        // Los importes llegan como se escribieron; cada uno se convierte una vez al guardar. Todo es en moneda local.
        Ok(aplicacion::prestamos::crear_prestamo(
            aplicacion::prestamos::DatosPrestamoNuevo {
                tipo_prestamo: input.tipo_prestamo,
                monto_prestamo: input.monto_prestamo.con_divisa(MONEDA_LOCAL),
                institucion_financiera: input.institucion_financiera,
                tasa_actual: input.tasa_actual,
                cuotas_totales: input.cuotas_totales,
                cuotas_pendientes: input.cuotas_pendientes,
                monto_cuota: input.monto_cuota.con_divisa(MONEDA_LOCAL),
                dia_pago: input.dia_pago,
                saldo_actual: input.saldo_actual.map(|s| s.con_divisa(MONEDA_LOCAL)),
                limite_credito: input.limite_credito.map(|l| l.con_divisa(MONEDA_LOCAL)),
            },
            a,
        )?)
    })
}

#[derive(Deserialize)]
struct ActualizarPrestamoInput {
    id: i64,
    tasa_actual: f64,
    monto_cuota: ipc::ImporteDecimal,
    dia_pago: i32,
    limite_credito: Option<ipc::ImporteDecimal>,
    tarjeta_id: Option<i64>,
}

/// Corrige las condiciones de un financiamiento ya registrado.
///
/// Existe porque los datos que definen un financiamiento **cambian**: la tasa
/// se revisa, la cuota se recalcula y el límite de una línea se amplía. Sin
/// esta vía, corregir cualquiera de ellos obligaba a borrar el registro y
/// crearlo de nuevo, lo que se lleva por delante el libro de movimientos.
///
/// Deliberadamente **no** toca dos campos. `monto_prestamo` es un hecho
/// histórico, el desembolso original, y no se reescribe. `saldo_actual` tiene
/// su propia vía en `declarar_saldo_prestamo`, que deja asiento de la
/// diferencia; permitir editarlo aquí sería una puerta trasera para moverlo
/// sin dejar rastro.
#[tauri::command]
fn actualizar_prestamo(input: ActualizarPrestamoInput) -> Result<(), String> {
    con_prestamos(|a| {
        Ok(aplicacion::prestamos::actualizar_prestamo(
            aplicacion::prestamos::DatosPrestamoCorregido {
                id: input.id,
                tasa_actual: input.tasa_actual,
                monto_cuota: input.monto_cuota.con_divisa(MONEDA_LOCAL),
                dia_pago: input.dia_pago,
                limite_credito: input.limite_credito.map(|l| l.con_divisa(MONEDA_LOCAL)),
                tarjeta_id: input.tarjeta_id,
            },
            a,
        )?)
    })
}

#[tauri::command]
fn pagar_cuota_prestamo(id: i64, fecha: Option<String>) -> Result<(), String> {
    con_prestamos(|a| {
        let fecha = fecha.unwrap_or_else(hoy_dd_mm_aaaa);
        Ok(aplicacion::prestamos::pagar_cuota(id, &fecha, a)?)
    })
}

/// Fija el saldo al que dice el estado de cuenta.
///
/// Es el punto de reconciliación: el saldo que lleva la aplicación se estima cuota a cuota, y ninguna estimación
/// cuadra al centavo con el acreedor. La declaración no corrige la estimación en silencio: queda asentada como un
/// movimiento propio, con la diferencia que introdujo.
#[tauri::command]
fn declarar_saldo_prestamo(id: i64, saldo: ipc::ImporteDecimal, fecha: Option<String>) -> Result<(), String> {
    con_prestamos(|a| {
        let fecha = fecha.unwrap_or_else(hoy_dd_mm_aaaa);
        Ok(aplicacion::prestamos::declarar_saldo(id, saldo.con_divisa(MONEDA_LOCAL), &fecha, a)?)
    })
}

#[derive(Serialize)]
pub struct MovimientoPrestamo {
    id: i64,
    fecha: String,
    tipo: String,
    monto: f64,
    interes: f64,
    capital: f64,
    saldo_resultante: f64,
}

#[tauri::command]
fn obtener_movimientos_prestamo(id: i64) -> Result<Vec<MovimientoPrestamo>, String> {
    con_prestamos(|a| {
        Ok(a.movimientos(id)?
            .into_iter()
            .map(|m| MovimientoPrestamo {
                id: m.id,
                fecha: m.fecha,
                tipo: m.tipo,
                monto: m.monto,
                interes: m.interes,
                capital: m.capital,
                saldo_resultante: m.saldo_resultante,
            })
            .collect())
    })
}

#[tauri::command]
fn eliminar_prestamo(id: i64) -> Result<(), String> {
    con_prestamos(|a| Ok(aplicacion::prestamos::eliminar_prestamo(id, a)?))
}

#[tauri::command]
fn obtener_clientes() -> Result<Vec<Cliente>, String> {
    con_catalogos(|a| {
        Ok(a.clientes()?.into_iter().map(|c| Cliente { id: c.id, rnc: c.rnc, nombre: c.nombre }).collect())
    })
}

#[tauri::command]
fn crear_cliente(rnc: String, nombre: String) -> Result<Cliente, String> {
    con_catalogos(|a| {
        let c = aplicacion::catalogos::crear_cliente(&rnc, &nombre, a)?;
        Ok(Cliente { id: c.id, rnc: c.rnc, nombre: c.nombre })
    })
}

#[tauri::command]
fn eliminar_cliente(id: i64) -> Result<(), String> {
    con_catalogos(|a| Ok(aplicacion::catalogos::eliminar_cliente(id, a)?))
}

/// Toma un respaldo bajo demanda y devuelve dónde quedó.
///
/// Los respaldos automáticos solo se toman al preparar el esquema. Este
/// comando existe para el momento en que se va a hacer algo arriesgado a mano
/// —conciliar varios saldos, cargar un estado— y se quiere una red antes.
#[tauri::command]
fn crear_respaldo() -> Result<String, String> {
    respaldo::respaldar("manual")
        .map(|r| r.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}

/// Nombres de los respaldos disponibles, del más reciente al más antiguo.
#[tauri::command]
fn listar_respaldos() -> Vec<String> {
    respaldo::listar()
}

/// Devuelve la base y el capital al estado de un respaldo. Antes toma uno del
/// estado actual, de modo que la restauración se puede deshacer.
#[tauri::command]
fn restaurar_respaldo(nombre: String) -> Result<serde_json::Value, String> {
    respaldo::restaurar(&nombre)
        .map(|r| {
            serde_json::json!({
                "respaldo_de_seguridad": r.respaldo_de_seguridad.to_string_lossy(),
                "capital_restaurado": r.capital_restaurado,
                "version_del_esquema": r.version_del_esquema,
            })
        })
        .map_err(|e| e.to_string())
}

/// Como `con_catalogos`, para el alta, corrección y consulta de cuentas.
fn con_cuentas<T>(f: impl FnOnce(&mut CuentasSqlite) -> Result<T, String>) -> Result<T, String> {
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let resultado = {
        let mut almacen = CuentasSqlite::nuevo(&tx);
        f(&mut almacen)?
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(resultado)
}

#[tauri::command]
fn obtener_cuentas() -> Result<Vec<CuentaAhorro>, String> {
    con_cuentas(|a| {
        Ok(a.cuentas()?
            .into_iter()
            .map(|c| CuentaAhorro {
                id: c.id,
                nombre: c.nombre,
                divisa: c.divisa,
                balance_actual: c.balance_actual,
                entidad: c.entidad,
                comision_pago_impuestos: c.comision_pago_impuestos,
            })
            .collect())
    })
}

#[tauri::command]
fn crear_cuenta(
    nombre: String,
    divisa: String,
    balance: ipc::ImporteDecimal,
    entidad: Option<String>,
    comision_pago_impuestos: Option<ipc::ImporteDecimal>,
) -> Result<i64, String> {
    con_cuentas(|a| {
        // Los importes llegan como se escribieron: el céntimo lo deciden esos dígitos. La comisión se cobra
        // en moneda local; el saldo inicial se guarda en las unidades que se escribieron.
        Ok(aplicacion::cuentas::crear_cuenta(
            aplicacion::cuentas::DatosCuentaNueva {
                nombre,
                divisa,
                saldo_inicial: balance.con_divisa(Divisa::Dop),
                entidad,
                comision_pago_impuestos: comision_pago_impuestos.map(|c| c.con_divisa(MONEDA_LOCAL)),
            },
            a,
        )?)
    })
}

/// Actualiza los datos declarativos de una cuenta.
///
/// No toca el balance: para eso están las transferencias y los movimientos,
/// que dejan rastro. Aquí solo se corrigen el nombre, la entidad y la tarifa,
/// que son cosas que el titular sabe y el sistema no puede deducir.
#[tauri::command]
fn actualizar_cuenta(
    id: i64,
    nombre: String,
    entidad: Option<String>,
    comision_pago_impuestos: Option<ipc::ImporteDecimal>,
) -> Result<(), String> {
    con_cuentas(|a| {
        Ok(aplicacion::cuentas::actualizar_cuenta(
            aplicacion::cuentas::DatosCuentaCorregida {
                id,
                nombre,
                entidad,
                comision_pago_impuestos: comision_pago_impuestos.map(|c| c.con_divisa(MONEDA_LOCAL)),
            },
            a,
        )?)
    })
}

#[tauri::command]
fn eliminar_cuenta(id: i64) -> Result<(), String> {
    // Traducción pura. La guarda —que hoy solo mira los gastos, H13— vive en
    // el caso de uso.
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    {
        let mut almacen = AlmacenSqlite::nuevo(&tx);
        aplicacion::transferir::eliminar_cuenta(id, &mut almacen)?;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn transferir_entre_cuentas(
    fecha: String,
    origen_id: i64,
    destino_id: i64,
    monto_origen: ipc::ImporteDecimal,
    monto_destino: ipc::ImporteDecimal,
    cargo: ipc::ImporteDecimal,
    descripcion: String
) -> Result<(), String> {
    // Traducción pura: la operación y sus invariantes viven en el dominio y
    // en el caso de uso. Aquí solo se abre la transacción, se convierten los
    // números en importes de la divisa que corresponde a cada cuenta, y se
    // confirma o se deshace.
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    {
        let mut almacen = AlmacenSqlite::nuevo(&tx);
        // Las divisas son un hecho de cada cuenta: se leen, no se declaran.
        let divisa_origen = almacen.divisa(origen_id)?;
        let divisa_destino = almacen.divisa(destino_id)?;

        transferir(
            DatosTransferencia {
                fecha,
                origen_id,
                destino_id,
                // Los tres importes llegan como se escribieron: el céntimo (y con él la tasa que se
                // deduce entre origen y destino) lo deciden esos dígitos. Cada uno se casa con la divisa
                // de SU cuenta, que se lee, no se declara.
                monto_origen: monto_origen.con_divisa(divisa_origen),
                monto_destino: monto_destino.con_divisa(divisa_destino),
                cargo: cargo.con_divisa(divisa_origen),
                descripcion,
            },
            &mut almacen,
        )?;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn obtener_transacciones_cuentas() -> Result<Vec<TransaccionCuenta>, String> {
    con_cuentas(|a| {
        Ok(a.transacciones()?
            .into_iter()
            .map(|t| TransaccionCuenta {
                id: t.id,
                fecha: t.fecha,
                cuenta_origen_id: t.cuenta_origen_id,
                cuenta_origen_nombre: t.cuenta_origen_nombre,
                cuenta_destino_id: t.cuenta_destino_id,
                cuenta_destino_nombre: t.cuenta_destino_nombre,
                monto_origen: t.monto_origen,
                monto_destino: t.monto_destino,
                tasa_cambio: t.tasa_cambio,
                cargo: t.cargo,
                descripcion: t.descripcion,
            })
            .collect())
    })
}

#[tauri::command]
fn actualizar_ingreso(
    id: i64,
    numero_factura: String,
    cliente_id: i64,
    fecha_emision: String,
    monto_total: ipc::ImporteDecimal,
    porcentaje_retencion: f64,
    // Importe cobrado cuando no entró el neto entero. `None` es la regla: se
    // da por cobrado el neto completo.
    cobro_parcial: Option<ipc::ImporteDecimal>,
    // Motivo de la corrección. Obligatorio **solo cuando mueve dinero**: si la
    // factura ya se cobró y el ajuste no es cero, hay un saldo que cambia y
    // eso abre un caso. Corregir una fecha o un número de factura no lo pide,
    // porque exigir explicación donde no hay riesgo enseña a escribirla sin
    // pensar.
    motivo: Option<String>,
) -> Result<String, String> {
    con_almacen(|a| {
        let r = aplicacion::ingresos::actualizar_ingreso(
            aplicacion::ingresos::DatosCorreccionDeFactura {
                id,
                numero_factura,
                cliente_id,
                fecha_emision,
                // El total y el cobro parcial llegan como se escribieron; el céntimo lo deciden esos dígitos.
                monto_total: monto_total.con_divisa(MONEDA_LOCAL),
                porcentaje_retencion,
                cobro_parcial: cobro_parcial.map(|c| c.con_divisa(MONEDA_LOCAL)),
                motivo,
            },
            a,
        )?;
        use aplicacion::ingresos::ResultadoDeCorreccion::*;
        Ok(match r {
            Corregida => "Factura corregida.".to_string(),
            Ajustada { cuenta, ajuste, caso } => format!(
                "Factura corregida. Se ajustó «{}» en DOP {:.2}. Caso {}.",
                cuenta,
                ajuste.unidades(),
                caso
            ),
            SinCuentaDeDeposito { caso } => {
                format!("Factura corregida. No tenía cuenta de depósito que ajustar. Caso {}.", caso)
            }
        })
    })
}

#[tauri::command]
fn crear_cobro_efectivo_informal(fecha: String, descripcion: String, monto: ipc::ImporteDecimal, divisa: String) -> Result<i64, String> {
    con_almacen(|a| {
        // El importe llega como se escribió y se usa **dos veces** (el ingreso y la caja): una sola conversión
        // garantiza que ambos reciben el mismo valor. Solo «USD» va a la caja de dólares; cualquier otra divisa,
        // a la de pesos (así ha sido siempre).
        let divisa = if divisa == "USD" { Divisa::Usd } else { Divisa::Dop };
        Ok(aplicacion::informales::crear_cobro_efectivo_informal(&fecha, &descripcion, monto.con_divisa(divisa), a)?)
    })
}

/// Cierra un consumo pendiente con el importe que el emisor cargó en moneda
/// local. Devuelve la tasa que se dedujo, para poder mostrarla.
#[tauri::command]
fn obtener_bonificaciones() -> Result<Vec<BonificacionDto>, String> {
    con_almacen(|a| {
        Ok(aplicacion::bonificaciones::listar_bonificaciones(a)?
            .into_iter()
            .map(|b| BonificacionDto {
                id: b.id,
                fecha: b.fecha,
                tarjeta_id: b.tarjeta_id,
                entidad: b.entidad,
                nombre_tarjeta: b.nombre_tarjeta,
                monto: b.monto,
                divisa: b.divisa,
                concepto: b.concepto,
                gasto_id: b.gasto_id,
            })
            .collect())
    })
}

/// Registra un crédito del emisor sobre una tarjeta. Reduce su deuda sin
/// alterar el consumo que lo originó.
#[tauri::command]
fn crear_bonificacion(
    fecha: String,
    tarjeta_id: i64,
    monto: ipc::ImporteDecimal,
    divisa: String,
    concepto: String,
    gasto_id: Option<i64>,
) -> Result<i64, String> {
    let divisa = if divisa == "USD" { Divisa::Usd } else { Divisa::Dop };
    // El importe llega como se escribió: el céntimo lo decide el núcleo con esos dígitos, y se casa
    // con la divisa declarada (la de la bonificación, que es del titular y no de una fila).
    let bonificacion = Bonificacion::nueva(monto.con_divisa(divisa), &concepto)?;

    con_almacen(|a| Ok(registrar_bonificacion(DatosBonificacion { fecha, tarjeta_id, bonificacion, gasto_id }, a)?))
}

#[tauri::command]
fn eliminar_bonificacion(id: i64) -> Result<(), String> {
    con_almacen(|a| Ok(revertir_bonificacion(id, a)?))
}

#[tauri::command]
fn liquidar_consumo_pendiente(id: i64, monto_liquidado: ipc::ImporteDecimal) -> Result<f64, String> {
    con_almacen(|a| {
        // El importe llega como se escribió: el céntimo (y con él la tasa que se deduce) sale de esos dígitos.
        let importe = monto_liquidado.con_divisa(MONEDA_LOCAL);
        Ok(liquidar_gasto(id, importe, a)?.tasa().valor())
    })
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CasoCorreccion {
    numero_caso: String,
    fecha: String,
    tipo: String,
    referencia_id: i64,
    descripcion: String,
    importe: Option<f64>,
    divisa: Option<String>,
    motivo: String,
}

/// Los casos de corrección, del más reciente al más antiguo.
///
/// **No hay comando para borrarlos.** Es deliberado: son el rastro de lo que
/// se destruyó, y un rastro que se puede borrar no es un rastro.
#[tauri::command]
fn obtener_correcciones() -> Result<Vec<CasoCorreccion>, String> {
    let conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT numero_caso, fecha, tipo, referencia_id, descripcion, importe, divisa, motivo
             FROM correcciones ORDER BY id DESC;",
        )
        .map_err(|e| e.to_string())?;

    let filas = stmt
        .query_map([], |r| {
            Ok(CasoCorreccion {
                numero_caso: r.get(0)?,
                fecha: r.get(1)?,
                tipo: r.get(2)?,
                referencia_id: r.get(3)?,
                descripcion: r.get(4)?,
                importe: r.get(5)?,
                divisa: r.get(6)?,
                motivo: r.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut lista = Vec::new();
    for f in filas {
        lista.push(f.map_err(|e| e.to_string())?);
    }
    Ok(lista)
}

/// Anota el caso de corrección de un movimiento antes de borrarlo.
///
/// Lee la descripción **antes** del borrado: después no habría de dónde
/// sacarla, y un caso que dice «se borró el gasto 315» sin decir cuál era no
/// sirve para auditar nada.
fn abrir_caso(
    tx: &rusqlite::Transaction,
    tipo: &str,
    id: i64,
    consulta: &str,
    motivo: &str,
) -> Result<String, String> {
    let (descripcion, importe, divisa): (String, Option<f64>, Option<String>) = tx
        .query_row(consulta, [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(|_| format!("No se encontró {} con identificador {}.", tipo, id))?;

    correcciones::registrar(
        tx,
        correcciones::Correccion { tipo, referencia_id: id, descripcion, importe, divisa, motivo },
    )
}

#[tauri::command]
fn eliminar_gasto(id: i64, motivo: String) -> Result<String, String> {
    // Traducción pura, igual que crear_gasto. La reversión vive en el caso de
    // uso y en el puerto, no en esta consulta.
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // Un gasto que creó otra operación —el cargo de un avance, la comisión de
    // un abono— no se borra por separado: se revierte la operación entera. La
    // lista y el porqué viven en `db_sql::GASTOS_DERIVADOS`.
    if let Some(motivo) = db_sql::motivo_de_no_borrar_gasto(&tx, id).map_err(|e| e.to_string())? {
        return Err(motivo.to_string());
    }

    let caso = abrir_caso(&tx, "gasto", id, "SELECT descripcion, monto, divisa FROM gastos WHERE id = ?;", &motivo)?;
    {
        let mut almacen = AlmacenSqlite::nuevo(&tx);
        revertir_gasto(id, &mut almacen)?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(caso)
}

#[tauri::command]
fn eliminar_transaccion_cuenta(id: i64, motivo: String) -> Result<String, String> {
    // Traducción pura. El recorte en cero del destino (H10) vive ahora en el
    // caso de uso y en el puerto, no en esta consulta.
    let mut conn = db_sql::obtener_conexion().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    let caso = abrir_caso(&tx, "traspaso", id, "SELECT COALESCE(descripcion, 'Traspaso entre cuentas'), monto_origen, 'DOP' FROM transacciones_cuentas WHERE id = ?;", &motivo)?;

    {
        let mut almacen = AlmacenSqlite::nuevo(&tx);
        revertir_transferencia(id, &mut almacen)?;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(caso)
}

#[tauri::command]
fn eliminar_ingreso_informal(id: i64, motivo: String) -> Result<String, String> {
    con_almacen(|a| Ok(aplicacion::informales::eliminar_ingreso_informal(id, &motivo, a)?))
}

#[tauri::command]
fn eliminar_ingreso(id: i64, motivo: String) -> Result<String, String> {
    con_almacen(|a| Ok(aplicacion::ingresos::eliminar_ingreso(id, &motivo, a)?))
}

// --- PUNTO DE ENTRADA PRINCIPAL ---
fn main() {
    // Inicializar bases de datos antes del arranque
    db_sql::inicializar_db().expect("Error al inicializar la base de datos SQLite");
    
    // Iniciar app de escritorio Tauri
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            obtener_categorias,
            crear_categoria,
            eliminar_categoria,
            obtener_gastos,
            crear_gasto,
            obtener_ingresos,
            crear_ingreso,
            actualizar_ingreso,
            marcar_ingreso_pagado,
            obtener_ingresos_informales,
            crear_ingreso_informal,
            marcar_informal_pagado,
            obtener_tarjetas,
            crear_tarjeta,
            actualizar_limites_tarjeta,
            registrar_pago_tarjeta,
            obtener_suscripciones,
            crear_suscripcion,
            actualizar_suscripcion,
            eliminar_suscripcion,
            corregir_proximo_cobro,
            asentar_periodo_pendiente,
            descartar_periodo_pendiente,
            procesar_suscripciones,
            obtener_capital,
            guardar_capital,
            obtener_prestamos,
            crear_prestamo,
            pagar_cuota_prestamo,
            actualizar_prestamo,
            declarar_saldo_prestamo,
            obtener_movimientos_prestamo,
            eliminar_prestamo,
            obtener_clientes,
            crear_cliente,
            eliminar_cliente,
            crear_respaldo,
            listar_respaldos,
            restaurar_respaldo,
            obtener_cuentas,
            revertir_abono_tarjeta,
            simular_avance_efectivo,
            registrar_avance_efectivo,
            obtener_avances_tarjeta,
            revertir_avance_efectivo,
            obtener_abonos_tarjeta,
            obtener_correcciones,
            crear_cuenta,
            actualizar_cuenta,
            eliminar_cuenta,
            transferir_entre_cuentas,
            obtener_transacciones_cuentas,
            crear_cobro_efectivo_informal,
            eliminar_gasto,
            liquidar_consumo_pendiente,
            obtener_bonificaciones,
            crear_bonificacion,
            eliminar_bonificacion,
            eliminar_transaccion_cuenta,
            eliminar_ingreso_informal,
            eliminar_ingreso
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}
