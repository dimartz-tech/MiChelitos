// Núcleo de dominio: reglas puras, sin SQL, sin Tauri y sin acceso al reloj real.
// Se permite código sin usar mientras los comandos siguen sin migrarse (Fase 1).
#![allow(dead_code)]

pub mod bonificacion;
pub mod capital;
pub mod cargos;
pub mod conversion;
pub mod cuenta;
pub mod dinero;
pub mod errores;
pub mod gasto;
pub mod ingreso;
pub mod prestamo;
pub mod suscripcion;
pub mod tarjeta;
