//! Reglas de los catálogos: categorías de gasto y clientes. Puras: sin SQL, sin Tauri.
//!
//! Salen de los comandos `crear_categoria`, `eliminar_categoria`, `crear_cliente` y
//! `eliminar_cliente` (A-03 de la auditoría del 2026-10-02) sin cambiar ninguna regla ni
//! ningún mensaje; las pruebas de caracterización `k1`–`k7` lo fijan.

use super::errores::ErrorDominio;

/// La categoría de sistema: sobre ella descansan los cargos y comisiones automáticos.
pub const CATEGORIA_DEL_SISTEMA: &str = "Otros";

/// El máximo de caracteres del nombre de una categoría **renombrada**. `crear_categoria` no tiene tope (hallazgo
/// documentado en `aplicacion::catalogos`); renombrar sí, para que el selector y la lista no se rompan.
pub const LONGITUD_MAXIMA_DE_CATEGORIA: usize = 60;

/// Nombres que la interfaz usa para otra cosa: ninguna categoría puede llamarse así. «Cargos derivados» es la fila
/// sintética del Resumen (id -1) y «Sin categoría» la etiqueta de un gasto sin categoría en la lista de gastos.
pub const NOMBRES_RESERVADOS: &[&str] = &[super::resumen_gastos::NOMBRE_CARGOS_DERIVADOS, "Sin categoría"];

/// Una categoría que el código busca **por nombre**, y para qué.
#[derive(Debug, PartialEq, Eq)]
pub struct CategoriaUsadaPorElSistema {
    pub nombre: &'static str,
    pub motivo: &'static str,
}

/// Las categorías del sistema: ni se renombran ni se fusionan **como origen** (sí pueden ser destino).
///
/// * «Otros»: `adaptadores/sqlite/abonos.rs` (`CategoriaDeSistema`) la busca por nombre para la comisión de cada
///   abono a tarjeta y para el cargo de un avance de efectivo, y `aplicacion/suscripciones.rs` la usa de respaldo.
/// * «Suscripciones»: `aplicacion/suscripciones.rs` + `adaptadores/sqlite/suscripciones.rs` la buscan por nombre
///   (sin distinguir mayúsculas) para asentar los cargos recurrentes automáticos.
/// * «Impuestos»: `dominio/cargos.rs` (`es_pago_de_impuestos`) reconoce por este nombre los pagos de impuestos:
///   exime de la retención los pagos a la TSS y la DGII y les aplica la comisión fija de la cuenta.
pub const CATEGORIAS_DEL_SISTEMA: &[CategoriaUsadaPorElSistema] = &[
    CategoriaUsadaPorElSistema {
        nombre: CATEGORIA_DEL_SISTEMA,
        motivo: "asentar las comisiones de los abonos a tarjeta y el cargo de los avances de efectivo",
    },
    CategoriaUsadaPorElSistema {
        nombre: "Suscripciones",
        motivo: "asentar los cargos recurrentes automáticos de las suscripciones",
    },
    CategoriaUsadaPorElSistema {
        nombre: "Impuestos",
        motivo: "reconocer los pagos de impuestos (exención de la retención de la TSS y la DGII y comisión fija de la cuenta)",
    },
];

/// El nombre sin mayúsculas, sin tildes (`áéíóú`, `ü` y sus graves y circunflejas) y con los blancos reducidos a uno,
/// para comparar. La `ñ` **no** se pliega: «Año» y «Ano» son palabras distintas.
pub fn plegar(nombre: &str) -> String {
    let sin_marcas: String = nombre
        .chars()
        .filter(|c| !matches!(*c, '\u{0300}' | '\u{0301}' | '\u{0302}' | '\u{0308}'))
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            otro => otro,
        })
        .collect();
    sin_marcas.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Para qué usa el sistema esta categoría, si la busca por nombre (comparado con `plegar`: es más estricto que cada
/// búsqueda, y por eso nunca deja pasar una que el sistema sí encuentre).
pub fn uso_del_sistema(nombre: &str) -> Option<&'static str> {
    let buscado = plegar(nombre);
    CATEGORIAS_DEL_SISTEMA.iter().find(|c| plegar(c.nombre) == buscado).map(|c| c.motivo)
}

fn rechazar_si_es_del_sistema(nombre: &str, accion: &'static str) -> Result<(), ErrorDominio> {
    match uso_del_sistema(nombre) {
        Some(motivo) => Err(ErrorDominio::CategoriaUsadaPorElSistema { nombre: nombre.to_string(), accion, motivo }),
        None => Ok(()),
    }
}

/// Una categoría del sistema no se renombra: el código la dejaría de encontrar.
pub fn puede_renombrarse_categoria(nombre_actual: &str) -> Result<(), ErrorDominio> {
    rechazar_si_es_del_sistema(nombre_actual, "renombrar")
}

/// Una categoría del sistema no se fusiona **en otra** (desaparecería); como destino sí puede ser.
pub fn puede_fusionarse_categoria_como_origen(nombre: &str) -> Result<(), ErrorDominio> {
    rechazar_si_es_del_sistema(nombre, "fusionar en otra")
}

/// El nombre nuevo de una categoría que se renombra: recortado, no vacío, de a lo sumo
/// `LONGITUD_MAXIMA_DE_CATEGORIA` caracteres, no reservado y **único entre las demás** sin distinguir mayúsculas
/// ni tildes. `otros_nombres` son los de todas las categorías menos la que se renombra: por eso puede cambiar su
/// propia capitalización o sus tildes.
pub fn nombre_para_renombrar(entrada: &str, otros_nombres: &[String]) -> Result<String, ErrorDominio> {
    let nombre = nombre_de_categoria(entrada)?;
    if nombre.chars().count() > LONGITUD_MAXIMA_DE_CATEGORIA {
        return Err(ErrorDominio::CategoriaNombreLargo { maximo: LONGITUD_MAXIMA_DE_CATEGORIA });
    }
    let plegado = plegar(&nombre);
    if NOMBRES_RESERVADOS.iter().any(|r| plegar(r) == plegado) {
        return Err(ErrorDominio::CategoriaNombreReservado { nombre });
    }
    if otros_nombres.iter().any(|o| plegar(o) == plegado) {
        return Err(ErrorDominio::CategoriaExistente);
    }
    Ok(nombre)
}

/// La fecha más antigua y la más reciente (`dd/mm/aaaa`) de unos gastos; `None` si no hay ninguna legible.
/// Se comparan como fechas, no como texto: «05/01/2027» va antes que «01/02/2026» solo si se ordena por texto.
pub fn rango_de_fechas(fechas: &[String]) -> Option<(String, String)> {
    let validas: Vec<chrono::NaiveDate> =
        fechas.iter().filter_map(|f| chrono::NaiveDate::parse_from_str(f.trim(), "%d/%m/%Y").ok()).collect();
    let desde = validas.iter().min()?;
    let hasta = validas.iter().max()?;
    Some((desde.format("%d/%m/%Y").to_string(), hasta.format("%d/%m/%Y").to_string()))
}

/// La descripción del caso de auditoría de una fusión: «Fusión de «A» en «B» (N gastos)».
pub fn descripcion_de_fusion(origen: &str, destino: &str, gastos: i64) -> String {
    let unidad = if gastos == 1 { "gasto" } else { "gastos" };
    format!("Fusión de «{origen}» en «{destino}» ({gastos} {unidad})")
}

/// El motivo automático del caso de auditoría: la fusión la pide el titular y no pasa por el diálogo de motivo, pero el
/// registro exige una explicación que siga teniendo sentido dentro de seis meses.
pub fn motivo_de_fusion(origen: &str, destino: &str, gastos: i64) -> String {
    format!(
        "Fusión pedida por el titular en Ajustes: la categoría «{origen}» se eliminó y sus {gastos} gastos pasaron a «{destino}»."
    )
}

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

    fn nombres(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn plegar_ignora_mayusculas_tildes_y_blancos_pero_no_la_enie() {
        assert_eq!(plegar("  ALIMENTACIÓN  "), plegar("alimentacion"));
        assert_eq!(plegar("Pingüino"), plegar("pinguino"));
        assert_eq!(plegar("Casa   de  campo"), "casa de campo");
        assert_ne!(plegar("Año"), plegar("Ano"));
        // La misma «é» escrita descompuesta (e + acento combinante) se pliega igual.
        assert_eq!(plegar("Cafe\u{301}"), plegar("café"));
    }

    #[test]
    fn renombrar_recorta_y_acepta_un_nombre_nuevo() {
        assert_eq!(nombre_para_renombrar("  Mascotas ", &nombres(&["Comida"])).unwrap(), "Mascotas");
    }

    #[test]
    fn renombrar_rechaza_vacio_demasiado_largo_y_reservado() {
        assert_eq!(nombre_para_renombrar("   ", &[]), Err(ErrorDominio::CategoriaSinNombre));
        let sesenta = "a".repeat(60);
        assert_eq!(nombre_para_renombrar(&sesenta, &[]).unwrap(), sesenta);
        assert_eq!(
            nombre_para_renombrar(&"a".repeat(61), &[]),
            Err(ErrorDominio::CategoriaNombreLargo { maximo: 60 })
        );
        // Los 60 se cuentan en caracteres, no en bytes: 60 «ñ» caben.
        assert!(nombre_para_renombrar(&"ñ".repeat(60), &[]).is_ok());
        for reservado in ["Cargos derivados", "  cargos DERIVADOS ", "Sin categoría", "SIN CATEGORIA"] {
            assert!(
                matches!(nombre_para_renombrar(reservado, &[]), Err(ErrorDominio::CategoriaNombreReservado { .. })),
                "{reservado}"
            );
        }
    }

    #[test]
    fn renombrar_rechaza_un_duplicado_sin_mirar_mayusculas_ni_tildes() {
        let otras = nombres(&["Alimentación", "Comida"]);
        for repetido in ["alimentacion", "ALIMENTACIÓN", " Alimentacion ", "comida", "COMIDA"] {
            assert_eq!(nombre_para_renombrar(repetido, &otras), Err(ErrorDominio::CategoriaExistente), "{repetido}");
        }
        assert!(nombre_para_renombrar("Alimentos", &otras).is_ok());
    }

    #[test]
    fn las_categorias_del_sistema_se_reconocen_con_cualquier_capitalizacion_y_explican_para_que_sirven() {
        for nombre in ["Otros", "otros", "SUSCRIPCIONES", "Impuestos", "impuestós"] {
            let e = puede_renombrarse_categoria(nombre).unwrap_err();
            assert!(matches!(e, ErrorDominio::CategoriaUsadaPorElSistema { accion: "renombrar", .. }), "{nombre}");
            assert!(e.to_string().contains("el sistema la usa para"), "{e}");
            assert!(puede_fusionarse_categoria_como_origen(nombre).is_err(), "{nombre}");
        }
        assert!(puede_renombrarse_categoria("Comida").is_ok());
        assert!(puede_fusionarse_categoria_como_origen("Comida").is_ok());
    }

    #[test]
    fn cada_categoria_del_sistema_tiene_un_motivo_y_ninguna_esta_reservada_ni_repetida() {
        let mut vistos = Vec::new();
        for c in CATEGORIAS_DEL_SISTEMA {
            assert!(c.motivo.len() > 20, "{} necesita explicarse", c.nombre);
            assert!(!vistos.contains(&plegar(c.nombre)), "repetida: {}", c.nombre);
            vistos.push(plegar(c.nombre));
            assert!(!NOMBRES_RESERVADOS.iter().any(|r| plegar(r) == plegar(c.nombre)));
        }
        assert_eq!(vistos.len(), 3);
    }

    #[test]
    fn el_rango_de_fechas_compara_fechas_no_texto() {
        let f = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            rango_de_fechas(&f(&["05/01/2027", "01/02/2026", "30/12/2026"])),
            Some(("01/02/2026".to_string(), "05/01/2027".to_string()))
        );
        assert_eq!(rango_de_fechas(&f(&["10/03/2027"])), Some(("10/03/2027".into(), "10/03/2027".into())));
        assert_eq!(rango_de_fechas(&f(&[])), None);
        assert_eq!(rango_de_fechas(&f(&["no es fecha"])), None);
    }

    #[test]
    fn la_descripcion_y_el_motivo_de_una_fusion_nombran_las_dos_categorias_y_pasan_el_umbral_del_registro() {
        assert_eq!(descripcion_de_fusion("Renta", "Alquiler", 12), "Fusión de «Renta» en «Alquiler» (12 gastos)");
        assert_eq!(descripcion_de_fusion("Renta", "Alquiler", 1), "Fusión de «Renta» en «Alquiler» (1 gasto)");
        assert_eq!(descripcion_de_fusion("Renta", "Alquiler", 0), "Fusión de «Renta» en «Alquiler» (0 gastos)");
        // Aun con nombres de una letra, el motivo supera los 15 caracteres que exige el registro.
        assert!(crate::dominio::correccion::motivo_de_correccion(&motivo_de_fusion("A", "B", 0)).is_ok());
    }
}
