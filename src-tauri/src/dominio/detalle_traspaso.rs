//! La foto completa de un traspaso que se borra: lo que el caso de corrección conserva de él.
//!
//! Borrar un traspaso destruye su asiento. El caso de corrección anota un resumen (descripción, importe, divisa), pero
//! un traspaso entre divisas, con tasa y cargo, no cabe en tres campos: el importe de origen solo no basta para saber qué
//! se movió ni hacia dónde. Este tipo guarda **todo** lo necesario para reconstruir el traspaso a mano.
//!
//! **Sin coma flotante.** Los importes viajan como texto exacto (`1187.53`) y la tasa también (`58.75`); la divisa va al
//! lado de cada importe. El tipo se serializa a JSON y se guarda en `correcciones.detalle`; `version` permite cambiar la
//! forma sin romper los casos ya anotados.

use super::dinero::{Dinero, TasaCambio, ESCALA_TASA_CAMBIO};
use serde::{Deserialize, Serialize};

/// Versión de la forma del JSON. Súbela si cambia una clave.
pub const VERSION_DEL_DETALLE: u32 = 1;

/// Un importe con su divisa: `{"monto": "5875.37", "divisa": "DOP"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImporteConDivisa {
    pub monto: String,
    pub divisa: String,
}

/// Un extremo del traspaso: la cuenta (id y nombre en el momento del borrado) y lo que le pasó.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtremoDelDetalle {
    pub cuenta_id: i64,
    pub cuenta_nombre: String,
    pub importe: ImporteConDivisa,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetalleDeTraspaso {
    pub version: u32,
    pub traspaso_id: i64,
    /// La fecha original del traspaso (dd/mm/aaaa), no la del borrado.
    pub fecha: String,
    /// `None` si el traspaso no tenía descripción.
    pub descripcion: Option<String>,
    pub origen: ExtremoDelDetalle,
    pub destino: ExtremoDelDetalle,
    /// DOP por USD implícita; `None` si no cruza divisas.
    pub tasa: Option<String>,
    /// En la divisa de origen; sale aparte del monto.
    pub cargo: ImporteConDivisa,
}

/// Lo que hace falta para armar el detalle; lo aporta el caso de uso.
pub struct DatosDelTraspasoBorrado<'a> {
    pub id: i64,
    pub fecha: &'a str,
    pub descripcion: &'a str,
    pub origen_id: i64,
    pub origen_nombre: &'a str,
    pub destino_id: i64,
    pub destino_nombre: &'a str,
    pub monto_origen: Dinero,
    pub monto_destino: Dinero,
    pub cargo: Dinero,
}

impl DetalleDeTraspaso {
    pub fn construir(d: &DatosDelTraspasoBorrado<'_>) -> DetalleDeTraspaso {
        DetalleDeTraspaso {
            version: VERSION_DEL_DETALLE,
            traspaso_id: d.id,
            fecha: d.fecha.to_string(),
            descripcion: if d.descripcion.is_empty() { None } else { Some(d.descripcion.to_string()) },
            origen: ExtremoDelDetalle {
                cuenta_id: d.origen_id,
                cuenta_nombre: d.origen_nombre.to_string(),
                importe: con_divisa(d.monto_origen),
            },
            destino: ExtremoDelDetalle {
                cuenta_id: d.destino_id,
                cuenta_nombre: d.destino_nombre.to_string(),
                importe: con_divisa(d.monto_destino),
            },
            tasa: tasa_implicita(d.monto_origen, d.monto_destino),
            cargo: con_divisa(d.cargo),
        }
    }

    /// El JSON que se guarda. Solo contiene cadenas, enteros y nulos: serializarlo no puede fallar.
    pub fn a_json(&self) -> String {
        serde_json::to_string(self).expect("el detalle solo contiene cadenas y enteros")
    }

    #[cfg(test)]
    pub fn desde_json(texto: &str) -> Result<DetalleDeTraspaso, String> {
        serde_json::from_str(texto).map_err(|e| e.to_string())
    }
}

fn con_divisa(d: Dinero) -> ImporteConDivisa {
    ImporteConDivisa { monto: centavos_a_texto(d.centavos()), divisa: d.divisa().codigo().to_string() }
}

/// `-1234.56` calculado sobre centavos enteros.
fn centavos_a_texto(centavos: i64) -> String {
    let signo = if centavos < 0 { "-" } else { "" };
    let abs = centavos.unsigned_abs();
    format!("{signo}{}.{:02}", abs / 100, abs % 100)
}

/// La tasa que dejan los dos importes (como la deduce `Transferencia::tasa`), en texto con al menos dos decimales.
/// `None` si no cruza divisas o si no se puede deducir (un monto de origen en cero).
fn tasa_implicita(origen: Dinero, destino: Dinero) -> Option<String> {
    if origen.divisa() == destino.divisa() || origen.centavos() == 0 {
        return None;
    }
    let cociente = destino.unidades() / origen.unidades();
    let t = TasaCambio::nueva(cociente).ok()?;
    let nano = t.micro_unidades();
    let mut decimales = format!("{:09}", nano % ESCALA_TASA_CAMBIO);
    while decimales.len() > 2 && decimales.ends_with('0') {
        decimales.pop();
    }
    Some(format!("{}.{decimales}", nano / ESCALA_TASA_CAMBIO))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dominio::dinero::Divisa;

    fn dop(c: i64) -> Dinero {
        Dinero::de_centavos(c, Divisa::Dop)
    }
    fn usd(c: i64) -> Dinero {
        Dinero::de_centavos(c, Divisa::Usd)
    }

    fn datos<'a>(o: Dinero, d: Dinero, cargo: Dinero) -> DatosDelTraspasoBorrado<'a> {
        DatosDelTraspasoBorrado {
            id: 7,
            fecha: "13/10/2026",
            descripcion: "Cambio de divisas",
            origen_id: 10,
            origen_nombre: "Ahorros DOP",
            destino_id: 12,
            destino_nombre: "Ahorros USD",
            monto_origen: o,
            monto_destino: d,
            cargo,
        }
    }

    #[test]
    fn usd_a_usd_conserva_la_divisa_y_no_lleva_tasa() {
        let d = DetalleDeTraspaso::construir(&datos(usd(20_037), usd(20_037), usd(0)));
        assert_eq!(d.origen.importe, ImporteConDivisa { monto: "200.37".into(), divisa: "USD".into() });
        assert_eq!(d.destino.importe.divisa, "USD");
        assert_eq!(d.tasa, None);
        assert_eq!(d.cargo, ImporteConDivisa { monto: "0.00".into(), divisa: "USD".into() });
    }

    #[test]
    fn dop_a_usd_lleva_los_dos_lados_y_la_tasa_exacta() {
        // 5 875.00 DOP -> 100.00 USD: tasa 58.75 (USD por DOP se invierte: aquí es destino/origen tal como lo deduce
        // el dominio, ver `Transferencia::tasa`).
        let d = DetalleDeTraspaso::construir(&datos(dop(587_500), usd(10_000), dop(0)));
        assert_eq!(d.origen.importe, ImporteConDivisa { monto: "5875.00".into(), divisa: "DOP".into() });
        assert_eq!(d.destino.importe, ImporteConDivisa { monto: "100.00".into(), divisa: "USD".into() });
        assert!(d.tasa.is_some());
    }

    #[test]
    fn la_tasa_sigue_la_misma_convencion_que_el_dominio() {
        let o = dop(600_000);
        let dst = usd(10_137);
        let esperada = TasaCambio::nueva(dst.unidades() / o.unidades()).unwrap().micro_unidades();
        let t = DetalleDeTraspaso::construir(&datos(o, dst, dop(0))).tasa.unwrap();
        let (e, f) = t.split_once('.').unwrap();
        let nano: i64 = e.parse::<i64>().unwrap() * ESCALA_TASA_CAMBIO + format!("{f:0<9}").parse::<i64>().unwrap();
        assert_eq!(nano, esperada, "{t}");
    }

    #[test]
    fn una_tasa_redonda_se_escribe_con_dos_decimales() {
        // 100.00 USD -> 5 875.00 DOP (destino/origen = 58.75)
        let d = DetalleDeTraspaso::construir(&datos(usd(10_000), dop(587_500), usd(0)));
        assert_eq!(d.tasa.as_deref(), Some("58.75"));
    }

    #[test]
    fn el_cargo_se_conserva_en_la_divisa_de_origen() {
        let d = DetalleDeTraspaso::construir(&datos(dop(587_500), usd(10_000), dop(11_873)));
        assert_eq!(d.cargo, ImporteConDivisa { monto: "118.73".into(), divisa: "DOP".into() });
    }

    #[test]
    fn sin_descripcion_es_nulo_y_con_ella_viaja_tal_cual() {
        let mut x = datos(dop(100), dop(100), dop(0));
        x.descripcion = "";
        assert_eq!(DetalleDeTraspaso::construir(&x).descripcion, None);
        x.descripcion = "Pago del alquiler";
        assert_eq!(DetalleDeTraspaso::construir(&x).descripcion.as_deref(), Some("Pago del alquiler"));
    }

    #[test]
    fn un_nombre_hostil_sobrevive_al_viaje_de_json_sin_alterarse() {
        let hostil = "<b>A'\"&</b> \\ \n";
        let mut x = datos(dop(100), dop(100), dop(0));
        x.origen_nombre = hostil;
        x.destino_nombre = hostil;
        x.descripcion = hostil;
        let json = DetalleDeTraspaso::construir(&x).a_json();
        let vuelto = DetalleDeTraspaso::desde_json(&json).unwrap();
        assert_eq!(vuelto.origen.cuenta_nombre, hostil);
        assert_eq!(vuelto.destino.cuenta_nombre, hostil);
        assert_eq!(vuelto.descripcion.as_deref(), Some(hostil));
        // Y es JSON válido para un parser independiente.
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["origen"]["cuenta_nombre"], hostil);
    }

    #[test]
    fn el_json_no_contiene_numeros_decimales() {
        let json = DetalleDeTraspaso::construir(&datos(dop(587_500), usd(10_000), dop(11_873))).a_json();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        fn sin_flotantes(v: &serde_json::Value) {
            match v {
                serde_json::Value::Number(n) => assert!(n.is_i64(), "número no entero: {n}"),
                serde_json::Value::Array(a) => a.iter().for_each(sin_flotantes),
                serde_json::Value::Object(o) => o.values().for_each(sin_flotantes),
                _ => {}
            }
        }
        sin_flotantes(&v);
    }

    #[test]
    fn serializa_con_las_claves_y_el_orden_documentados() {
        let json = DetalleDeTraspaso::construir(&datos(usd(10_000), dop(587_500), usd(0))).a_json();
        assert_eq!(
            json,
            r#"{"version":1,"traspaso_id":7,"fecha":"13/10/2026","descripcion":"Cambio de divisas","origen":{"cuenta_id":10,"cuenta_nombre":"Ahorros DOP","importe":{"monto":"100.00","divisa":"USD"}},"destino":{"cuenta_id":12,"cuenta_nombre":"Ahorros USD","importe":{"monto":"5875.00","divisa":"DOP"}},"tasa":"58.75","cargo":{"monto":"0.00","divisa":"USD"}}"#
        );
    }

    #[test]
    fn ida_y_vuelta_reproduce_el_mismo_detalle() {
        let d = DetalleDeTraspaso::construir(&datos(dop(587_500), usd(10_000), dop(11_873)));
        assert_eq!(DetalleDeTraspaso::desde_json(&d.a_json()).unwrap(), d);
    }

    #[test]
    fn un_importe_negativo_o_pequeno_se_escribe_sin_perder_ceros() {
        assert_eq!(centavos_a_texto(5), "0.05");
        assert_eq!(centavos_a_texto(-5), "-0.05");
        assert_eq!(centavos_a_texto(0), "0.00");
    }
}
