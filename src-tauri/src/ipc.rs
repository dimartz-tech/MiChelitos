//! La frontera con la interfaz: importes que llegan como se escribieron.
//!
//! ## De dónde viene esta decisión
//!
//! Salió de investigar el tramo 4 de la política de redondeo, y el argumento
//! no estaba en el planteamiento original del tramo: **los tramos 1 y 2 no
//! eliminaron la conversión desde coma flotante, la centralizaron**.
//!
//! `Dinero::nuevo` recibe un `f64` que ya no vale lo que el usuario tecleó
//! —`1.005` se guarda como 1.00499…— y decide el céntimo sobre ese número. Es
//! el mismo defecto que `dividir_redondeando` documenta, viviendo en la puerta
//! de entrada en vez de en la aritmética.
//!
//! ## Por qué el alcance es el que es
//!
//! Antes de aplicarlo se midió, y el resultado acotó el trabajo:
//!
//! * **52 de 53 campos de importe de la interfaz llevan `step="0.01"`**, que
//!   el navegador valida al enviar el formulario. Por ahí no entra un tercer
//!   decimal.
//! * Para importes de dos decimales, el viaje `texto → Number → texto` no
//!   pierde nada: cero divergencias en 28 571 muestras.
//! * **Los importes que entran por `prompt()` no pasan por esa validación.**
//!   Son tres: la comisión por pago de impuestos, el saldo declarado de un
//!   préstamo y el cobro parcial de una factura.
//!
//! De modo que el tipo existe para toda la frontera, pero **se aplica primero
//! donde cambia el resultado**. Migrar los cuarenta y un parámetros restantes
//! es mecánico y no urge: el `step` ya los cubre.
//!
//! Se documenta así, y no como «se hizo todo», porque la diferencia entre una
//! corrección que cambia importes y una que ordena el código debería poder
//! leerse en el código.

use crate::dominio::dinero::{centavos_desde_texto, Dinero, Divisa};
use crate::dominio::errores::ErrorDominio;
use serde::de::{self, Deserializer};
use serde::Deserialize;

/// Un importe tal como llega de la interfaz, ya en centavos.
///
/// **No lleva divisa a propósito.** En este sistema la divisa es un hecho de
/// la fila, no algo que la interfaz declare: `transferir_entre_cuentas` la lee
/// de la cuenta con el comentario «se leen, no se declaran». Un importe que
/// trajera su moneda reabriría esa puerta, así que se casa con su `Divisa` en
/// el cuerpo del comando.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImporteDecimal {
    centavos: i64,
}

impl ImporteDecimal {
    /// Desde los dígitos escritos. Es la misma puerta que usa el IPC, y estar
    /// disponible permite que las pruebas la ejerciten sin pasar por serde.
    pub fn desde_texto(texto: &str) -> Result<ImporteDecimal, ErrorDominio> {
        Ok(ImporteDecimal { centavos: centavos_desde_texto(texto)? })
    }

    pub fn centavos(&self) -> i64 {
        self.centavos
    }

    /// Lo casa con la divisa que le corresponda, que se lee de la fila.
    pub fn con_divisa(&self, divisa: Divisa) -> Dinero {
        Dinero::de_centavos(self.centavos, divisa)
    }

    /// Para los sitios que todavía persisten un `f64`.
    ///
    /// Es una salida, no una entrada: el céntimo ya está decidido y este
    /// número lo representa exactamente.
    pub fn unidades(&self) -> f64 {
        self.con_divisa(Divisa::Dop).unidades()
    }
}

/// Acepta **texto o número**, a propósito.
///
/// El texto es el camino bueno. El número se admite durante la transición
/// para que la interfaz pueda migrar pantalla a pantalla en vez de en un solo
/// cambio de todo o nada; cuando no quede ninguna llamada que mande números,
/// esta rama se retira y la coma flotante deja de entrar por aquí.
impl<'de> Deserialize<'de> for ImporteDecimal {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<ImporteDecimal, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Entrada {
            Texto(String),
            Numero(f64),
        }

        let centavos = match Entrada::deserialize(d)? {
            Entrada::Texto(t) => centavos_desde_texto(&t).map_err(de::Error::custom)?,
            Entrada::Numero(n) => Dinero::nuevo(n, Divisa::Dop)
                .map_err(de::Error::custom)?
                .centavos(),
        };
        Ok(ImporteDecimal { centavos })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leer(json: &str) -> Result<ImporteDecimal, serde_json::Error> {
        serde_json::from_str(json)
    }

    #[test]
    fn el_texto_llega_con_sus_digitos_intactos() {
        assert_eq!(leer(r#""1234.56""#).unwrap().centavos(), 123_456);
        assert_eq!(leer(r#""0.01""#).unwrap().centavos(), 1);
        assert_eq!(leer(r#""-42.75""#).unwrap().centavos(), -4_275);
    }

    #[test]
    fn el_texto_decide_el_centimo_donde_la_coma_flotante_fallaba() {
        // La diferencia que justifica el cambio, en la frontera real.
        assert_eq!(leer(r#""1.005""#).unwrap().centavos(), 101, "por texto sube");
        assert_eq!(leer("1.005").unwrap().centavos(), 100, "por número baja");
    }

    #[test]
    fn un_numero_sigue_admitiendose_durante_la_transicion() {
        assert_eq!(leer("1234.56").unwrap().centavos(), 123_456);
        assert_eq!(leer("100").unwrap().centavos(), 10_000);
    }

    #[test]
    fn un_texto_ilegible_falla_con_un_mensaje_que_lo_dice() {
        let e = leer(r#""abc""#).unwrap_err().to_string();
        assert!(e.contains("no es un importe"), "mensaje obtenido: {e}");
    }

    #[test]
    fn se_casa_con_la_divisa_de_su_fila_y_no_con_una_declarada() {
        let i = leer(r#""100.00""#).unwrap();
        assert_eq!(i.con_divisa(Divisa::Usd).divisa(), Divisa::Usd);
        assert_eq!(i.con_divisa(Divisa::Dop).divisa(), Divisa::Dop);
        assert_eq!(i.con_divisa(Divisa::Usd).centavos(), i.con_divisa(Divisa::Dop).centavos());
    }
}
