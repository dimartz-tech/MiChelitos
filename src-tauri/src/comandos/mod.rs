//! Los comandos de Tauri, un módulo por agregado.
//!
//! Cada comando es traducción pura: interpreta lo que llega de la interfaz, abre la transacción por un ayudante de
//! `ayudantes` y delega en un caso de uso. Las reglas viven en `dominio` y `aplicacion`. `main.rs` solo los registra.
//! El generador de tipos (`herramientas/generar_tipos_ipc.mjs`) lee estos archivos, un solo nivel de profundidad.

pub mod abonos;
pub mod avances;
mod ayudantes;
pub mod bonificaciones;
pub mod capital;
pub mod catalogos;
pub mod cierre_de_tarjetas;
pub mod correcciones;
pub mod cuentas;
pub mod gastos;
pub mod informales;
pub mod ingresos;
pub mod patrimonio;
pub mod prestamos;
pub mod respaldos;
pub mod suscripciones;
pub mod tarjetas;

/// El código fuente de cada módulo de comandos, para las pruebas estructurales que lo recorren (`db_sql`,
/// `migraciones`). Una prueba de este módulo exige que la lista cubra todos los archivos de la carpeta.
#[cfg(test)]
pub(crate) const FUENTES: &[(&str, &str)] = &[
    ("comandos/abonos.rs", include_str!("abonos.rs")),
    ("comandos/avances.rs", include_str!("avances.rs")),
    ("comandos/ayudantes.rs", include_str!("ayudantes.rs")),
    ("comandos/bonificaciones.rs", include_str!("bonificaciones.rs")),
    ("comandos/capital.rs", include_str!("capital.rs")),
    ("comandos/catalogos.rs", include_str!("catalogos.rs")),
    ("comandos/cierre_de_tarjetas.rs", include_str!("cierre_de_tarjetas.rs")),
    ("comandos/correcciones.rs", include_str!("correcciones.rs")),
    ("comandos/cuentas.rs", include_str!("cuentas.rs")),
    ("comandos/gastos.rs", include_str!("gastos.rs")),
    ("comandos/informales.rs", include_str!("informales.rs")),
    ("comandos/ingresos.rs", include_str!("ingresos.rs")),
    ("comandos/patrimonio.rs", include_str!("patrimonio.rs")),
    ("comandos/prestamos.rs", include_str!("prestamos.rs")),
    ("comandos/respaldos.rs", include_str!("respaldos.rs")),
    ("comandos/suscripciones.rs", include_str!("suscripciones.rs")),
    ("comandos/tarjetas.rs", include_str!("tarjetas.rs")),
];

/// `main.rs` y todos los módulos de comandos, como un solo texto.
#[cfg(test)]
pub(crate) fn fuente_de_comandos() -> String {
    let mut todo = String::from(include_str!("../main.rs"));
    for (_, texto) in FUENTES {
        todo.push('\n');
        todo.push_str(texto);
    }
    todo
}

#[cfg(test)]
mod pruebas {
    use super::FUENTES;

    #[test]
    fn la_lista_de_fuentes_cubre_todos_los_modulos_de_comandos() {
        let carpeta = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("comandos");
        let mut en_disco: Vec<String> = std::fs::read_dir(&carpeta)
            .expect("leer src/comandos")
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".rs") && n != "mod.rs")
            .map(|n| format!("comandos/{n}"))
            .collect();
        en_disco.sort();
        let listados: Vec<String> = FUENTES.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(en_disco, listados, "comandos/mod.rs::FUENTES no coincide con los archivos de la carpeta");
        let mod_rs = include_str!("mod.rs");
        for (nombre, _) in FUENTES {
            let modulo = nombre.trim_start_matches("comandos/").trim_end_matches(".rs");
            assert!(mod_rs.contains(&format!("mod {modulo};")), "{modulo} no está declarado en mod.rs");
        }
    }
}
