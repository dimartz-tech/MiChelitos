// Núcleo de dominio: reglas puras, sin SQL, sin Tauri y sin acceso al reloj real.
// Se permite código sin usar mientras los comandos siguen sin migrarse (Fase 1).
#![allow(dead_code)]

pub mod dinero;
pub mod errores;
