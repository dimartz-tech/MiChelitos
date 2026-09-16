//! Intérprete de Gherkin en español, escrito a mano.
//!
//! El plan (§6.4) contemplaba la caja `cucumber` como dependencia de
//! desarrollo. Se descarta: el proyecto lleva cero dependencias nuevas desde
//! la Fase 0, y el criterio de **peso contenido y auditable** pesa más que la
//! comodidad de un runner completo. Lo que hace falta aquí son cuatro
//! escenarios sobre dobles en memoria, y eso cabe en un archivo que cualquiera
//! puede leer de cabo a rabo.
//!
//! Lo que el intérprete soporta, porque es lo que el archivo usa:
//!
//! * `Característica:`, `Antecedentes:` y `Escenario:`.
//! * Pasos con `Dado`, `Cuando`, `Entonces` e `Y`.
//! * **Continuaciones**: una línea que no empieza por palabra clave se une al
//!   paso anterior. Es lo que permite partir un paso largo en dos renglones
//!   sin que deje de ser un solo paso.
//! * Comentarios `#` y líneas en blanco, que se ignoran.
//!
//! Lo que **no** soporta, a propósito: esquemas de escenario, tablas de datos
//! y etiquetas. Añadirlos sin un caso que los pida sería construir un
//! framework en vez de una prueba.

/// Un paso ya normalizado: sin palabra clave y en una sola línea.
#[derive(Debug, Clone, PartialEq)]
pub struct Paso {
    pub texto: String,
    /// Número de línea en el archivo, para que un fallo sea localizable.
    pub linea: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Escenario {
    pub nombre: String,
    /// Los pasos de `Antecedentes` van delante, ya fusionados.
    pub pasos: Vec<Paso>,
}

const PALABRAS_CLAVE: [&str; 4] = ["Dado ", "Cuando ", "Entonces ", "Y "];

/// Parte un archivo `.feature` en escenarios ejecutables.
///
/// Los `Antecedentes` se copian al principio de cada escenario en lugar de
/// guardarse aparte: así ejecutar un escenario es recorrer una única lista, y
/// no hay forma de olvidarse de aplicarlos.
pub fn analizar(contenido: &str) -> Vec<Escenario> {
    let mut antecedentes: Vec<Paso> = Vec::new();
    let mut escenarios: Vec<Escenario> = Vec::new();
    let mut en_antecedentes = false;

    for (indice, linea_cruda) in contenido.lines().enumerate() {
        let linea = linea_cruda.trim();
        if linea.is_empty() || linea.starts_with('#') {
            continue;
        }

        if let Some(_) = linea.strip_prefix("Característica:") {
            continue;
        }
        if linea.starts_with("Antecedentes:") {
            en_antecedentes = true;
            continue;
        }
        if let Some(nombre) = linea.strip_prefix("Escenario:") {
            en_antecedentes = false;
            escenarios.push(Escenario {
                nombre: nombre.trim().to_string(),
                pasos: antecedentes.clone(),
            });
            continue;
        }

        let destino = if en_antecedentes {
            &mut antecedentes
        } else {
            match escenarios.last_mut() {
                Some(e) => &mut e.pasos,
                // Las tres líneas de «Como / Quiero / Para» caen aquí: son
                // narrativa, no pasos, y no hay escenario donde ponerlas.
                None => continue,
            }
        };

        match quitar_palabra_clave(linea) {
            Some(texto) => destino.push(Paso { texto, linea: indice + 1 }),
            // Continuación del paso anterior.
            None => match destino.last_mut() {
                Some(anterior) => {
                    anterior.texto.push(' ');
                    anterior.texto.push_str(linea);
                }
                None => continue,
            },
        }
    }

    escenarios
}

fn quitar_palabra_clave(linea: &str) -> Option<String> {
    PALABRAS_CLAVE
        .iter()
        .find_map(|k| linea.strip_prefix(k))
        .map(|resto| resto.trim().to_string())
}

/// Extrae los fragmentos entre comillas de un paso, en orden.
pub fn entrecomillados(texto: &str) -> Vec<String> {
    let mut salida = Vec::new();
    let mut dentro = false;
    let mut actual = String::new();
    for c in texto.chars() {
        if c == '"' {
            if dentro {
                salida.push(std::mem::take(&mut actual));
            }
            dentro = !dentro;
        } else if dentro {
            actual.push(c);
        }
    }
    salida
}

/// Extrae los números de un paso, en orden.
///
/// Solo reconoce lo que el archivo usa: dígitos con un punto decimal
/// opcional. Se apoya en que los importes van siempre fuera de las comillas,
/// donde están los códigos de divisa y los nombres.
pub fn numeros(texto: &str) -> Vec<f64> {
    let mut salida = Vec::new();
    let mut actual = String::new();
    let mut dentro_de_comillas = false;

    for c in texto.chars() {
        if c == '"' {
            dentro_de_comillas = !dentro_de_comillas;
            continue;
        }
        if dentro_de_comillas {
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && !actual.is_empty()) {
            actual.push(c);
        } else if !actual.is_empty() {
            if let Ok(n) = actual.parse() {
                salida.push(n);
            }
            actual.clear();
        }
    }
    if !actual.is_empty() {
        if let Ok(n) = actual.parse() {
            salida.push(n);
        }
    }
    salida
}

#[cfg(test)]
mod tests {
    use super::*;

    // El intérprete también se prueba: si analiza mal, los escenarios podrían
    // pasar por no ejecutar nada, que es la peor forma de tener la suite en
    // verde.

    #[test]
    fn los_antecedentes_se_anteponen_a_cada_escenario() {
        let f = "Característica: X\n  Antecedentes:\n    Dado que A\n\n  Escenario: uno\n    Cuando B\n\n  Escenario: dos\n    Cuando C\n";
        let e = analizar(f);

        assert_eq!(e.len(), 2);
        assert_eq!(e[0].pasos.iter().map(|p| p.texto.as_str()).collect::<Vec<_>>(), ["que A", "B"]);
        assert_eq!(e[1].pasos.iter().map(|p| p.texto.as_str()).collect::<Vec<_>>(), ["que A", "C"]);
    }

    #[test]
    fn una_linea_sin_palabra_clave_continua_el_paso_anterior() {
        let f = "Escenario: uno\n  Cuando registro un abono\n    Desde la cuenta \"X\"\n";
        let e = analizar(f);

        assert_eq!(e[0].pasos.len(), 1, "son un solo paso, no dos");
        assert_eq!(e[0].pasos[0].texto, "registro un abono Desde la cuenta \"X\"");
    }

    #[test]
    fn la_narrativa_y_los_comentarios_no_son_pasos() {
        let f = "# language: es\nCaracterística: X\n  Como titular\n  Quiero abonar\n  Para saldar\n\n  Escenario: uno\n    Cuando B\n";
        let e = analizar(f);

        assert_eq!(e.len(), 1);
        assert_eq!(e[0].pasos.len(), 1, "solo el Cuando");
    }

    #[test]
    fn los_entrecomillados_salen_en_orden() {
        assert_eq!(
            entrecomillados(r#"abono de 500.00 "USD" a la tarjeta "Tarjeta Ejemplo""#),
            ["USD", "Tarjeta Ejemplo"]
        );
    }

    #[test]
    fn los_numeros_ignoran_lo_que_hay_entre_comillas() {
        // Si un nombre llevara dígitos, colarlos como importe daría una
        // prueba que pasa por casualidad.
        assert_eq!(numeros(r#"balance 500000.00 de "Cuenta 24 Horas""#), [500000.00]);
        assert_eq!(numeros("abono de 500.00 con tasa de 60.00"), [500.00, 60.00]);
    }

    #[test]
    fn un_escenario_sin_antecedentes_no_falla() {
        assert!(analizar("Escenario: solo\n  Cuando B\n")[0].pasos.len() == 1);
    }
}
