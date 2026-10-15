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
mod comandos;

#[cfg(test)]
mod caracterizacion;
#[cfg(test)]
mod contrato_rusqlite;
#[cfg(test)]
mod sumas_en_centavos;
#[cfg(test)]
mod gherkin;
#[cfg(test)]
mod caso_de_uso_gherkin;

// --- PUNTO DE ENTRADA PRINCIPAL ---
fn main() {
    // Inicializar bases de datos antes del arranque
    db_sql::inicializar_db().expect("Error al inicializar la base de datos SQLite");
    
    // Iniciar app de escritorio Tauri
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            comandos::catalogos::obtener_categorias,
            comandos::catalogos::crear_categoria,
            comandos::catalogos::eliminar_categoria,
            comandos::gastos::obtener_gastos,
            comandos::gastos::crear_gasto,
            comandos::ingresos::obtener_ingresos,
            comandos::ingresos::crear_ingreso,
            comandos::ingresos::actualizar_ingreso,
            comandos::ingresos::marcar_ingreso_pagado,
            comandos::informales::obtener_ingresos_informales,
            comandos::informales::crear_ingreso_informal,
            comandos::informales::marcar_informal_pagado,
            comandos::tarjetas::obtener_tarjetas,
            comandos::tarjetas::crear_tarjeta,
            comandos::tarjetas::actualizar_limites_tarjeta,
            comandos::abonos::registrar_pago_tarjeta,
            comandos::suscripciones::obtener_suscripciones,
            comandos::suscripciones::crear_suscripcion,
            comandos::suscripciones::actualizar_suscripcion,
            comandos::suscripciones::eliminar_suscripcion,
            comandos::suscripciones::corregir_proximo_cobro,
            comandos::suscripciones::asentar_periodo_pendiente,
            comandos::suscripciones::descartar_periodo_pendiente,
            comandos::suscripciones::procesar_suscripciones,
            comandos::capital::obtener_capital,
            comandos::capital::guardar_capital,
            comandos::prestamos::obtener_prestamos,
            comandos::prestamos::crear_prestamo,
            comandos::prestamos::pagar_cuota_prestamo,
            comandos::prestamos::actualizar_prestamo,
            comandos::prestamos::declarar_saldo_prestamo,
            comandos::prestamos::obtener_movimientos_prestamo,
            comandos::prestamos::eliminar_prestamo,
            comandos::catalogos::obtener_clientes,
            comandos::catalogos::crear_cliente,
            comandos::catalogos::eliminar_cliente,
            comandos::respaldos::crear_respaldo,
            comandos::respaldos::listar_respaldos,
            comandos::respaldos::restaurar_respaldo,
            comandos::cuentas::obtener_cuentas,
            comandos::abonos::revertir_abono_tarjeta,
            comandos::avances::simular_avance_efectivo,
            comandos::avances::registrar_avance_efectivo,
            comandos::avances::obtener_avances_tarjeta,
            comandos::avances::revertir_avance_efectivo,
            comandos::abonos::obtener_abonos_tarjeta,
            comandos::cuentas::crear_cuenta,
            comandos::cuentas::actualizar_cuenta,
            comandos::cuentas::archivar_cuenta,
            comandos::tarjetas::obtener_tarjetas_inactivas,
            comandos::cierre_de_tarjetas::previsualizar_cierre_tarjeta,
            comandos::cierre_de_tarjetas::cerrar_tarjeta,
            comandos::cierre_de_tarjetas::reactivar_tarjeta,
            comandos::cuentas::reactivar_cuenta,
            comandos::correcciones::obtener_correcciones,
            comandos::cuentas::obtener_cuentas_archivadas,
            comandos::cuentas::transferir_entre_cuentas,
            comandos::cuentas::obtener_transacciones_cuentas,
            comandos::informales::crear_cobro_efectivo_informal,
            comandos::gastos::eliminar_gasto,
            comandos::gastos::liquidar_consumo_pendiente,
            comandos::bonificaciones::obtener_bonificaciones,
            comandos::bonificaciones::crear_bonificacion,
            comandos::bonificaciones::eliminar_bonificacion,
            comandos::cuentas::eliminar_transaccion_cuenta,
            comandos::informales::eliminar_ingreso_informal,
            comandos::ingresos::eliminar_ingreso
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}
