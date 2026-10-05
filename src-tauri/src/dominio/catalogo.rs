//! Reglas de los catálogos: categorías de gasto y clientes. Puras: sin SQL, sin Tauri.
//!
//! Salen de los comandos `crear_categoria`, `eliminar_categoria`, `crear_cliente` y
//! `eliminar_cliente` (A-03 de la auditoría del 2026-10-02) sin cambiar ninguna regla ni
//! ningún mensaje; las pruebas de caracterización `k1`–`k7` lo fijan.

use super::errores::ErrorDominio;

/// La categoría de sistema: sobre ella descansan los cargos y comisiones automáticos.
pub const CATEGORIA_DEL_SISTEMA: &str = "Otros";

/// El nombre de una categoría nueva, recortado. Uno vacío no es un nombre.
pub fn nombre_de_categoria(entrada: &str) -> Result<String, ErrorDominio> {
    let limpio = entrada.trim();
    if limpio.is_empty() {
        return Err(ErrorDominio::CategoriaSinNombre);
    }
    Ok(limpio.to_string())
}

/// Una categoría se borra si no es la de sistema y ningún gasto la usa.
///
/// El orden importa: la de sistema se nombra primero aunque además tuviera gastos.
pub fn puede_eliminarse_categoria(nombre: &str, gastos_asociados: i64) -> Result<(), ErrorDominio> {
    if nombre == CATEGORIA_DEL_SISTEMA {
        return Err(ErrorDominio::CategoriaDelSistema);
    }
    if gastos_asociados > 0 {
        return Err(ErrorDominio::CategoriaConGastos);
    }
    Ok(())
}

/// RNC y nombre de un cliente nuevo, recortados; ninguno puede quedar vacío.
pub fn datos_de_cliente(rnc: &str, nombre: &str) -> Result<(String, String), ErrorDominio> {
    let (rnc, nombre) = (rnc.trim(), nombre.trim());
    if rnc.is_empty() || nombre.is_empty() {
        return Err(ErrorDominio::ClienteSinDatos);
    }
    Ok((rnc.to_string(), nombre.to_string()))
}

/// Un cliente con facturas no se borra: perderían a quién se facturó.
pub fn puede_eliminarse_cliente(facturas_asociadas: i64) -> Result<(), ErrorDominio> {
    if facturas_asociadas > 0 {
        return Err(ErrorDominio::ClienteConFacturas);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_nombre_de_categoria_se_recorta_y_no_puede_ser_vacio() {
        assert_eq!(nombre_de_categoria("  Mascotas ").unwrap(), "Mascotas");
        assert_eq!(nombre_de_categoria("   "), Err(ErrorDominio::CategoriaSinNombre));
        assert_eq!(nombre_de_categoria(""), Err(ErrorDominio::CategoriaSinNombre));
    }

    #[test]
    fn la_categoria_de_sistema_nunca_se_elimina_ni_con_gastos_ni_sin_ellos() {
        assert_eq!(puede_eliminarse_categoria("Otros", 0), Err(ErrorDominio::CategoriaDelSistema));
        assert_eq!(puede_eliminarse_categoria("Otros", 5), Err(ErrorDominio::CategoriaDelSistema));
        // La comparación es exacta: «otros» no es la de sistema.
        assert_eq!(puede_eliminarse_categoria("otros", 0), Ok(()));
    }

    #[test]
    fn una_categoria_con_gastos_no_se_elimina() {
        assert_eq!(puede_eliminarse_categoria("Comida", 1), Err(ErrorDominio::CategoriaConGastos));
        assert_eq!(puede_eliminarse_categoria("Comida", 0), Ok(()));
    }

    #[test]
    fn un_cliente_exige_rnc_y_nombre_y_se_recorta() {
        assert_eq!(datos_de_cliente(" 123 ", " Acme ").unwrap(), ("123".into(), "Acme".into()));
        for (r, n) in [("", "x"), ("x", ""), ("  ", "  ")] {
            assert_eq!(datos_de_cliente(r, n), Err(ErrorDominio::ClienteSinDatos));
        }
    }

    #[test]
    fn un_cliente_con_facturas_no_se_elimina() {
        assert_eq!(puede_eliminarse_cliente(1), Err(ErrorDominio::ClienteConFacturas));
        assert_eq!(puede_eliminarse_cliente(0), Ok(()));
    }
}
