//! Caso de uso: guardar el capital declarado, y sumarlo.
//!
//! El capital vive como un único documento JSON y hasta ahora se guardaba tal
//! cual llegaba: un monto negativo, con fracción de céntimo o ilegible, una
//! fecha que no existe, una tasa del 850 %. Era la vía de dinero menos
//! vigilada del proyecto, la única que no pasaba por `Dinero`.
//!
//! ## Qué se exige, y a qué
//!
//! **Solo lo que entra o cambia.** El documento se guarda entero en cada
//! acción, así que exigir todo cada vez bloquearía al titular por una entrada
//! antigua que no tocó —y como no hay edición, la única salida sería borrarla—.
//! Lo que ya estaba guardado y llega idéntico se conserva; lo nuevo y lo
//! modificado pasa por las reglas de `dominio::capital`.
//!
//! ## Formato en disco: no cambia
//!
//! Los importes se guardan como **números**, exactos al céntimo, igual que
//! siempre. Es deliberado: la aplicación instalada lee el mismo archivo, y un
//! cambio de forma la rompería. Lo que sí se admite al entrar es un importe
//! como **texto**, tal como se escribió, para que el céntimo lo decidan los
//! dígitos y no una conversión a coma flotante; se normaliza a número al
//! guardar.
//!
//! Un importe que llega ya como número y trae fracción de céntimo se
//! **rechaza**: no se puede saber qué quiso escribir quien lo tecleó, y
//! redondear en silencio es justo lo que esta vía hacía sin que nadie lo viera.

use crate::dominio::capital::{
    self, divisa_del_capital, ErrorCapital, Motivo, CAMPOS_CALCULADOS,
};
use crate::dominio::dinero::Dinero;
use serde_json::{Number, Value};

/// Claves del documento que se calculan al leer y no deben persistirse.
const CAMPOS_CALCULADOS_DEL_DOCUMENTO: &[&str] = &["totales"];

/// Retira todo lo que `obtener_capital` calcula al leer.
///
/// Existe porque las acciones de la interfaz leen el capital, mutan una parte
/// y lo guardan entero: sin esto, lo calculado se grababa como declarado.
pub fn retirar_campos_calculados(data: &mut Value) {
    if let Some(raiz) = data.as_object_mut() {
        for campo in CAMPOS_CALCULADOS_DEL_DOCUMENTO {
            raiz.remove(*campo);
        }
    }
    for coleccion in ["certificados", "bolsa"] {
        if let Some(entradas) = data.get_mut(coleccion).and_then(|v| v.as_array_mut()) {
            for entrada in entradas {
                if let Some(objeto) = entrada.as_object_mut() {
                    for campo in CAMPOS_CALCULADOS {
                        objeto.remove(*campo);
                    }
                }
            }
        }
    }
}

fn error(entrada: impl Into<String>, motivo: impl Into<String>) -> ErrorCapital {
    ErrorCapital { entrada: entrada.into(), motivo: motivo.into() }
}

/// Un importe tal como llega: número exacto al céntimo, o texto.
fn leer_monto(valor: Option<&Value>, campo: &str) -> Result<Dinero, Motivo> {
    let dinero = match valor {
        None | Some(Value::Null) => return Err(format!("falta {campo}.")),
        Some(Value::Number(n)) => {
            let x = n.as_f64().ok_or_else(|| format!("{campo} no es un número válido."))?;
            if !x.is_finite() || (x * 100.0 - (x * 100.0).round()).abs() > 1e-6 {
                return Err(format!(
                    "{campo} trae fracción de céntimo. Escríbelo con dos decimales como máximo."
                ));
            }
            Dinero::nuevo(x, divisa_del_capital()).map_err(|e| e.to_string())?
        }
        Some(Value::String(s)) => {
            Dinero::desde_texto(s, divisa_del_capital()).map_err(|e| e.to_string())?
        }
        Some(_) => return Err(format!("{campo} no es un importe.")),
    };
    capital::validar_monto(dinero, campo)
}

fn como_numero(d: Dinero) -> Value {
    Value::Number(Number::from_f64(d.unidades()).expect("un importe finito"))
}

fn leer_texto(valor: Option<&Value>, campo: &str) -> Result<String, Motivo> {
    match valor {
        Some(Value::String(s)) => capital::validar_texto(s, campo),
        _ => Err(format!("{campo} es obligatorio.")),
    }
}

fn leer_tasa(valor: Option<&Value>) -> Result<f64, Motivo> {
    let x = match valor {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
    .ok_or_else(|| "falta la tasa.".to_string())?;
    capital::validar_tasa(x)
}

/// Certificados e inversiones de bolsa comparten forma: quién, cuánto, a qué
/// tasa y cuándo vence. Solo cambia cómo se llama el emisor.
fn normalizar_instrumento(entrada: &mut Value, campo_emisor: &str) -> Result<(), Motivo> {
    let objeto = entrada.as_object_mut().ok_or_else(|| "no es un objeto.".to_string())?;

    let nombre = leer_texto(objeto.get(campo_emisor), &format!("el {campo_emisor}"))?;
    let monto = leer_monto(objeto.get("monto"), "el monto")?;
    let tasa = leer_tasa(objeto.get("tasa"))?;
    let vencimiento = match objeto.get("vencimiento") {
        Some(Value::String(s)) => capital::validar_fecha(s)?,
        _ => return Err("falta la fecha de vencimiento.".to_string()),
    };

    objeto.insert(campo_emisor.to_string(), Value::String(nombre));
    objeto.insert("monto".to_string(), como_numero(monto));
    objeto.insert(
        "tasa".to_string(),
        Value::Number(Number::from_f64(tasa).expect("una tasa finita")),
    );
    objeto.insert(
        "vencimiento".to_string(),
        Value::String(vencimiento.format("%d/%m/%Y").to_string()),
    );
    if let Some(Value::String(pago)) = objeto.get("tipo_pago") {
        let pago = pago.trim().to_string();
        objeto.insert("tipo_pago".to_string(), Value::String(pago));
    }
    Ok(())
}

fn normalizar_propiedad(entrada: &mut Value) -> Result<(), Motivo> {
    let objeto = entrada.as_object_mut().ok_or_else(|| "no es un objeto.".to_string())?;

    let id = leer_texto(objeto.get("id"), "el identificador")?;
    let nombre = leer_texto(objeto.get("nombre"), "el nombre")?;
    let valor = leer_monto(objeto.get("valor_estimado"), "el valor estimado")?;

    objeto.insert("id".to_string(), Value::String(id));
    objeto.insert("nombre".to_string(), Value::String(nombre));
    objeto.insert("valor_estimado".to_string(), como_numero(valor));
    if let Some(Value::String(subtipo)) = objeto.get("subtipo") {
        let subtipo = subtipo.trim().to_string();
        objeto.insert("subtipo".to_string(), Value::String(subtipo));
    }
    Ok(())
}

/// Igualdad de documentos que no distingue `500` de `500.0`.
///
/// Lo guardado y lo que vuelve de la interfaz pasan por dos serializaciones y
/// un mismo importe puede llegar como entero o como decimal.
fn equivalentes(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| equivalentes(p, q))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| equivalentes(v, w)))
        }
        _ => a == b,
    }
}

fn ya_estaba_guardada(entrada: &Value, previas: &[&Value]) -> bool {
    previas.iter().any(|p| equivalentes(entrada, p))
}

fn nombre_para_el_mensaje(entrada: &Value, campo: &str) -> String {
    entrada
        .get(campo)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!(" ({s})"))
        .unwrap_or_default()
}

/// Valida y normaliza el capital que llega, contra el que ya estaba guardado.
pub fn preparar_para_guardar(mut entrante: Value, guardado: &Value) -> Result<Value, ErrorCapital> {
    retirar_campos_calculados(&mut entrante);
    let mut previo = guardado.clone();
    retirar_campos_calculados(&mut previo);

    if !entrante.is_object() {
        return Err(error("Capital", "el documento no es un objeto."));
    }

    for (coleccion, etiqueta, campo_emisor) in [
        ("certificados", "Certificado", "banco"),
        ("bolsa", "Inversión de bolsa", "emisor"),
    ] {
        let previas: Vec<&Value> = previo
            .get(coleccion)
            .and_then(|v| v.as_array())
            .map(|a| a.iter().collect())
            .unwrap_or_default();
        let Some(valor) = entrante.get_mut(coleccion) else { continue };
        let entradas = valor
            .as_array_mut()
            .ok_or_else(|| error(etiqueta, "la colección no es una lista."))?;
        for (i, entrada) in entradas.iter_mut().enumerate() {
            if ya_estaba_guardada(entrada, &previas) {
                continue;
            }
            let quien = format!("{etiqueta} {}{}", i + 1, nombre_para_el_mensaje(entrada, campo_emisor));
            normalizar_instrumento(entrada, campo_emisor).map_err(|m| error(quien, m))?;
        }
    }

    // Propiedades: varios grupos, un solo espacio de identificadores.
    let previas_propiedades: Vec<&Value> = previo
        .get("propiedades")
        .and_then(|v| v.as_object())
        .map(|o| o.values().filter_map(|g| g.as_array()).flatten().collect())
        .unwrap_or_default();

    if let Some(propiedades) = entrante.get_mut("propiedades") {
        let grupos = propiedades
            .as_object_mut()
            .ok_or_else(|| error("Propiedades", "no es un objeto."))?;
        for (grupo, valor) in grupos.iter_mut() {
            let entradas = valor
                .as_array_mut()
                .ok_or_else(|| error(format!("Propiedades ({grupo})"), "no es una lista."))?;
            for (i, entrada) in entradas.iter_mut().enumerate() {
                if ya_estaba_guardada(entrada, &previas_propiedades) {
                    continue;
                }
                let quien = format!("Bien {} de {grupo}{}", i + 1, nombre_para_el_mensaje(entrada, "nombre"));
                normalizar_propiedad(entrada).map_err(|m| error(quien, m))?;
            }
        }

        // Un identificador repetido haría que borrar un bien borrara dos.
        let mut ids: Vec<&str> = Vec::new();
        for valor in grupos.values() {
            for entrada in valor.as_array().into_iter().flatten() {
                if let Some(id) = entrada.get("id").and_then(|v| v.as_str()) {
                    ids.push(id);
                }
            }
        }
        for (i, id) in ids.iter().enumerate() {
            if ids[..i].contains(id) {
                return Err(error(
                    format!("Bien con identificador «{id}»"),
                    "el identificador está repetido: borrar uno borraría los dos.",
                ));
            }
        }
    }

    Ok(entrante)
}

/// Totales del capital, sumados en centavos enteros.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Totales {
    pub certificados: Dinero,
    pub bolsa: Dinero,
    pub inmobiliario: Dinero,
    pub vehiculos: Dinero,
    pub maquinaria: Dinero,
}

impl Totales {
    pub fn patrimonio(&self) -> Dinero {
        [self.certificados, self.bolsa, self.inmobiliario, self.vehiculos, self.maquinaria]
            .into_iter()
            .fold(Dinero::cero(divisa_del_capital()), |acc, d| acc.sumar(&d).unwrap_or(acc))
    }
}

/// Un importe ya guardado, leído sin exigir nada: lo ilegible cuenta como cero.
///
/// Sumar no es validar. Un dato antiguo mal formado no debe impedir ver el
/// total de lo demás, y lo que no se entiende no se inventa.
fn importe_tolerante(valor: Option<&Value>) -> Dinero {
    let cero = Dinero::cero(divisa_del_capital());
    match valor {
        Some(Value::Number(n)) => n
            .as_f64()
            .and_then(|x| Dinero::nuevo(x, divisa_del_capital()).ok())
            .unwrap_or(cero),
        Some(Value::String(s)) => Dinero::desde_texto(s, divisa_del_capital()).unwrap_or(cero),
        _ => cero,
    }
}

fn sumar_entradas(entradas: Option<&Value>, campo: &str) -> Dinero {
    let cero = Dinero::cero(divisa_del_capital());
    entradas
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .fold(cero, |acc, e| acc.sumar(&importe_tolerante(e.get(campo))).unwrap_or(acc))
}

pub fn totales_de(capital: &Value) -> Totales {
    let propiedades = capital.get("propiedades");
    let grupo = |nombre: &str| sumar_entradas(propiedades.and_then(|p| p.get(nombre)), "valor_estimado");
    Totales {
        certificados: sumar_entradas(capital.get("certificados"), "monto"),
        bolsa: sumar_entradas(capital.get("bolsa"), "monto"),
        inmobiliario: grupo("inmobiliario"),
        vehiculos: grupo("vehiculos"),
        maquinaria: grupo("maquinaria"),
    }
}

/// Los totales como los espera la vista: números en pesos.
pub fn totales_como_json(t: &Totales) -> Value {
    serde_json::json!({
        "certificados": t.certificados.unidades(),
        "bolsa": t.bolsa.unidades(),
        "inmobiliario": t.inmobiliario.unidades(),
        "vehiculos": t.vehiculos.unidades(),
        "maquinaria": t.maquinaria.unidades(),
        "patrimonio": t.patrimonio().unidades(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn vacio() -> Value {
        json!({"propiedades": {"inmobiliario": [], "vehiculos": [], "maquinaria": []}, "certificados": [], "bolsa": []})
    }

    fn con_certificado(c: Value) -> Value {
        let mut d = vacio();
        d["certificados"] = json!([c]);
        d
    }

    fn cert(monto: Value) -> Value {
        json!({"banco": "Banco Ejemplo", "monto": monto, "tasa": 8.5, "vencimiento": "31/12/2030", "tipo_pago": "A cuenta"})
    }

    // --- Importes ---

    #[test]
    fn un_certificado_valido_se_guarda_con_el_importe_como_numero_exacto() {
        let salida = preparar_para_guardar(con_certificado(cert(json!(1234.56))), &vacio()).unwrap();
        assert_eq!(salida["certificados"][0]["monto"], json!(1234.56));
    }

    #[test]
    fn un_importe_como_texto_se_normaliza_a_numero_y_lo_deciden_los_digitos() {
        // El formato en disco no cambia: sigue siendo un número.
        let salida = preparar_para_guardar(con_certificado(cert(json!("1234.56"))), &vacio()).unwrap();
        assert_eq!(salida["certificados"][0]["monto"], json!(1234.56));
        // Y el tercer decimal lo decide la regla del sistema, sin binario.
        let salida = preparar_para_guardar(con_certificado(cert(json!("1.005"))), &vacio()).unwrap();
        assert_eq!(salida["certificados"][0]["monto"], json!(1.01));
    }

    #[test]
    fn un_numero_con_fraccion_de_centimo_se_rechaza_en_vez_de_redondearse() {
        let e = preparar_para_guardar(con_certificado(cert(json!(500.005))), &vacio()).unwrap_err();
        assert!(e.motivo.contains("fracción de céntimo"), "{e}");
    }

    #[test]
    fn cero_negativo_e_ilegible_se_rechazan() {
        for malo in [json!(0), json!(-500.0), json!("abc"), json!(null), json!(true)] {
            assert!(preparar_para_guardar(con_certificado(cert(malo.clone())), &vacio()).is_err(), "aceptó {malo}");
        }
    }

    // --- Otros campos ---

    #[test]
    fn la_tasa_la_fecha_y_el_emisor_se_exigen() {
        let mut c = cert(json!(1000.5)); c["tasa"] = json!(-1.0);
        assert!(preparar_para_guardar(con_certificado(c), &vacio()).is_err(), "tasa negativa");
        let mut c = cert(json!(1000.5)); c["tasa"] = json!(850.0);
        assert!(preparar_para_guardar(con_certificado(c), &vacio()).is_err(), "tasa absurda");
        let mut c = cert(json!(1000.5)); c["vencimiento"] = json!("31/02/2030");
        assert!(preparar_para_guardar(con_certificado(c), &vacio()).is_err(), "fecha inexistente");
        let mut c = cert(json!(1000.5)); c["vencimiento"] = json!("2030-12-31");
        assert!(preparar_para_guardar(con_certificado(c), &vacio()).is_err(), "fecha en otro formato");
        let mut c = cert(json!(1000.5)); c["banco"] = json!("   ");
        assert!(preparar_para_guardar(con_certificado(c), &vacio()).is_err(), "banco vacío");
    }

    #[test]
    fn el_error_dice_que_entrada_y_que_campo() {
        let mut d = vacio();
        d["certificados"] = json!([cert(json!(1000.5)), cert(json!(-3.0))]);
        let e = preparar_para_guardar(d, &vacio()).unwrap_err();
        assert!(e.entrada.contains("Certificado 2"), "{e}");
        assert!(e.entrada.contains("Banco Ejemplo"), "{e}");
        assert!(e.motivo.contains("negativo"), "{e}");
    }

    #[test]
    fn la_bolsa_se_valida_igual_con_su_propio_nombre_de_emisor() {
        let mut d = vacio();
        d["bolsa"] = json!([{"emisor": "Emisor Ejemplo", "monto": "-5", "tasa": 6.0, "vencimiento": "01/01/2030"}]);
        let e = preparar_para_guardar(d, &vacio()).unwrap_err();
        assert!(e.entrada.contains("Inversión de bolsa 1"), "{e}");
    }

    #[test]
    fn una_propiedad_exige_valor_nombre_e_identificador() {
        let mut d = vacio();
        d["propiedades"]["inmobiliario"] = json!([{"id": "1", "nombre": "Casa", "subtipo": "residencial", "valor_estimado": "250000.00"}]);
        let ok = preparar_para_guardar(d, &vacio()).unwrap();
        assert_eq!(ok["propiedades"]["inmobiliario"][0]["valor_estimado"], json!(250000.0));

        for malo in [json!({"id": "1", "nombre": "Casa", "valor_estimado": 0}),
                     json!({"id": "1", "nombre": "", "valor_estimado": 10}),
                     json!({"nombre": "Casa", "valor_estimado": 10})] {
            let mut d = vacio();
            d["propiedades"]["vehiculos"] = json!([malo.clone()]);
            assert!(preparar_para_guardar(d, &vacio()).is_err(), "aceptó {malo}");
        }
    }

    #[test]
    fn un_identificador_repetido_se_rechaza_porque_borrar_uno_borraria_los_dos() {
        let mut d = vacio();
        d["propiedades"]["inmobiliario"] = json!([{"id": "7", "nombre": "A", "valor_estimado": 10}]);
        d["propiedades"]["vehiculos"] = json!([{"id": "7", "nombre": "B", "valor_estimado": 20}]);
        let e = preparar_para_guardar(d, &vacio()).unwrap_err();
        assert!(e.motivo.contains("repetido"), "{e}");
    }

    // --- Lo antiguo no bloquea ---

    #[test]
    fn lo_ya_guardado_no_se_vuelve_a_exigir_pero_lo_nuevo_si() {
        // Una entrada antigua mal formada no debe impedir borrar otra: como no
        // hay edición, la única salida sería borrarla, y borrar cualquier otra
        // cosa antes quedaría bloqueado.
        let antigua = json!({"banco": "Antiguo", "monto": -500.005, "tasa": -1.0, "vencimiento": "no es una fecha"});
        let mut guardado = vacio();
        guardado["certificados"] = json!([antigua.clone(), cert(json!(2000.0))]);

        // Se borra la buena; la antigua vuelve idéntica.
        let mut entrante = vacio();
        entrante["certificados"] = json!([antigua.clone()]);
        let salida = preparar_para_guardar(entrante, &guardado).unwrap();
        assert_eq!(salida["certificados"][0], antigua, "se conserva tal cual");

        // Pero añadir algo nuevo y malo sigue rechazándose.
        let mut entrante = vacio();
        entrante["certificados"] = json!([antigua, cert(json!(-1.0))]);
        assert!(preparar_para_guardar(entrante, &guardado).is_err());
    }

    #[test]
    fn modificar_una_entrada_antigua_la_devuelve_a_las_reglas() {
        let antigua = json!({"banco": "Antiguo", "monto": -500.0, "tasa": 5.0, "vencimiento": "01/01/2030"});
        let mut guardado = vacio();
        guardado["certificados"] = json!([antigua.clone()]);
        let mut cambiada = antigua; cambiada["tasa"] = json!(6.0);
        let mut entrante = vacio();
        entrante["certificados"] = json!([cambiada]);
        assert!(preparar_para_guardar(entrante, &guardado).is_err(), "ya no es idéntica: se exige");
    }

    #[test]
    fn un_entero_y_un_decimal_del_mismo_importe_son_la_misma_entrada() {
        // 500 y 500.0 no son iguales para serde_json, y sí para el titular.
        let mut guardado = vacio();
        guardado["certificados"] = json!([{"banco": "Antiguo", "monto": -500.0, "tasa": 5.0, "vencimiento": "01/01/2030"}]);
        let mut entrante = vacio();
        entrante["certificados"] = json!([{"banco": "Antiguo", "monto": -500, "tasa": 5, "vencimiento": "01/01/2030"}]);
        assert!(preparar_para_guardar(entrante, &guardado).is_ok());
    }

    // --- Lo calculado no se guarda ---

    #[test]
    fn los_campos_calculados_y_los_totales_no_sobreviven() {
        let mut d = con_certificado(cert(json!(1000.5)));
        d["certificados"][0]["alerta_vencimiento"] = json!(true);
        d["certificados"][0]["dias_restantes"] = json!(3);
        d["totales"] = json!({"patrimonio": 1.0});
        let salida = preparar_para_guardar(d, &vacio()).unwrap();
        assert!(salida["certificados"][0].get("alerta_vencimiento").is_none());
        assert!(salida["certificados"][0].get("dias_restantes").is_none());
        assert!(salida.get("totales").is_none());
    }

    // --- Totales ---

    #[test]
    fn los_totales_se_suman_en_centavos_sin_arrastrar_ruido() {
        // 0.1 + 0.2 no es 0.3 en coma flotante; en centavos sí.
        let mut d = vacio();
        d["certificados"] = json!([{"banco": "A", "monto": 0.1}, {"banco": "B", "monto": 0.2}]);
        d["propiedades"]["inmobiliario"] = json!([{"id": "1", "valor_estimado": 100000.10}, {"id": "2", "valor_estimado": 200000.20}]);
        let t = totales_de(&d);
        assert_eq!(t.certificados, Dinero::nuevo(0.3, divisa_del_capital()).unwrap());
        assert_eq!(t.inmobiliario, Dinero::nuevo(300000.30, divisa_del_capital()).unwrap());
        assert_eq!(t.patrimonio().centavos(), 30_000_030 + 30);
    }

    #[test]
    fn un_dato_ilegible_cuenta_cero_y_no_impide_sumar_lo_demas() {
        let mut d = vacio();
        d["bolsa"] = json!([{"emisor": "A", "monto": "basura"}, {"emisor": "B", "monto": 500.0}, {"emisor": "C"}]);
        assert_eq!(totales_de(&d).bolsa, Dinero::nuevo(500.0, divisa_del_capital()).unwrap());
    }

    #[test]
    fn un_capital_vacio_o_sin_colecciones_suma_cero() {
        assert_eq!(totales_de(&json!({})).patrimonio().centavos(), 0);
        assert_eq!(totales_de(&vacio()).patrimonio().centavos(), 0);
    }
}
