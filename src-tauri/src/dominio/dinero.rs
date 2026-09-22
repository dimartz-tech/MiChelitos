//! Representación de importes monetarios.
//!
//! `Dinero` guarda **centavos como entero**, no unidades como coma flotante.
//! Ambas divisas del sistema (DOP y USD) tienen exactamente dos decimales, de
//! modo que un `i64` de centavos representa cualquier importe de forma exacta
//! y la aritmética de sumas y restas no acumula error.
//!
//! La consecuencia de diseño que más importa: un porcentaje sobre un importe
//! **no puede** producir una fracción de centavo silenciosa. Todo cálculo que
//! divida o aplique una tasa pasa por un único punto —[`Dinero::porcentaje`]—
//! donde el redondeo es explícito y verificable.
//!
//! La conversión a `f64` existe solo para la frontera con SQLite, cuyas
//! columnas siguen siendo `REAL` hasta la fase de migración del esquema.

use super::errores::ErrorDominio;
use serde::{Deserialize, Serialize};

/// Centavos en una unidad monetaria.
const CENTAVOS_POR_UNIDAD: f64 = 100.0;

/// Mayor entero que un `f64` representa de forma exacta (2^53 − 1). Más allá
/// de este valor la conversión desde coma flotante deja de ser fiable, así que
/// se rechaza en lugar de saturar en silencio.
const MAX_CENTAVOS_EXACTOS: f64 = 9_007_199_254_740_991.0;

/// Desviación máxima admitida por **representar una tasa**: 0.01 centavos.
///
/// Acota una de las dos fuentes de desviación del sistema, y conviene no
/// confundirlas:
///
/// * **El redondeo final al céntimo** no es un error sino una decisión. El
///   0.20 % de 89.6790 son 1 793.58 centavos: 0.58 de centavo no existe, y hay
///   que cobrar 1 793 o 1 794. Ese residuo llega a medio centavo **por
///   definición** y ningún límite puede reducirlo; lo que sí puede exigirse es
///   que la decisión sea explícita y siempre la misma, que es lo que hace
///   `dividir_redondeando`.
///
/// * **La representación de la tasa** sí es evitable. Una tasa guardada con
///   escala finita desplaza el resultado, y ese desplazamiento **sí** puede
///   acotarse eligiendo la escala. Es lo que esta constante gobierna.
pub const TOLERANCIA_REPRESENTACION_CENTAVOS: f64 = 0.01;

/// Importe con el que se mide si una escala honra la tolerancia.
///
/// Diez mil unidades: el orden de magnitud de una operación corriente aquí.
/// La desviación por representación crece con el importe, de modo que la
/// tolerancia solo tiene sentido referida a una magnitud declarada.
pub const IMPORTE_DE_REFERENCIA_CENTAVOS: i64 = 1_000_000;

/// Escala de los porcentajes: mil-millonésimas de la fracción.
///
/// **La escala la fija la tolerancia, no la comodidad.** Seis decimales
/// parecían de sobra para expresar un 0.20 %, y lo son para *escribirlo*; pero
/// sobre el importe de referencia una tasa cuantizada a millonésimas desvía
/// hasta 0.5 centavos, cincuenta veces la tolerancia. A mil-millonésimas el
/// desvío cae a 0.0005 centavos.
///
/// Coincide con la escala de las tasas de cambio, y no por casualidad: ambas
/// producen importes que se guardan, y la tolerancia que se les exige es la
/// misma.
pub const ESCALA_TASA: i64 = 1_000_000_000;

/// Escala de las tasas de cambio: mil-millonésimas.
///
/// **Necesita más precisión que un porcentaje, y no por capricho.** Una tasa y
/// su recíproca viven en órdenes de magnitud distintos: 60 pesos por dólar es
/// 0.0166… dólares por peso, un decimal periódico. A escala de millonésimas el
/// error de esa recíproca es de 3·10⁻⁷, suficiente para desviar un importe
/// grande; a mil-millonésimas baja a 3·10⁻¹⁰.
///
/// Que siga sin ser exacta es inevitable —el periódico no cabe en ninguna
/// escala finita— y es la razón de que los **importes** sean lo autoritativo y
/// la tasa un dato acompañante: una conversión guarda sus centavos de origen y
/// de destino, y jamás se recalcula el destino a partir de la tasa.
pub const ESCALA_TASA_CAMBIO: i64 = 1_000_000_000;

/// División entera redondeando **mitad alejándose de cero**.
///
/// Es el **único lugar del sistema donde se decide un céntimo**. Antes esa
/// decisión la tomaba `f64::round()`, que aplica esa misma regla pero sobre
/// el valor binario, no sobre el decimal que el usuario escribió: `1.005` se
/// guarda como 1.00499999…, de modo que redondeaba hacia abajo, mientras que
/// `2.675` —cuyo error se cancela al multiplicar por 100— redondeaba hacia
/// arriba. Dos importes de la misma forma, en direcciones opuestas, y no por
/// la regla sino por un accidente de representación.
///
/// En aritmética entera la regla se cumple siempre. El denominador debe ser
/// positivo; con un denominador impar no existe el caso de mitad exacta, de
/// modo que truncar `d / 2` es correcto.
pub fn dividir_redondeando(numerador: i128, denominador: i128) -> i128 {
    debug_assert!(denominador > 0, "el denominador debe ser positivo");
    let mitad = denominador / 2;
    if numerador >= 0 {
        (numerador + mitad) / denominador
    } else {
        (numerador - mitad) / denominador
    }
}

/// Convierte los dígitos de un importe decimal en centavos exactos.
///
/// **No usa coma flotante en ningún punto.** Separa la cadena por el punto
/// decimal, lee cada mitad como entero y las compone. Ese es todo el truco, y
/// es lo que hace que el número guardado sea el número escrito.
///
/// Admite lo que un formulario produce: signo opcional, espacios alrededor,
/// separadores de millar, y con o sin parte decimal. Rechaza lo que no es un
/// importe, nombrando el motivo en vez de devolver cero.
pub fn centavos_desde_texto(texto: &str) -> Result<i64, ErrorDominio> {
    let limpio: String = texto.chars().filter(|c| !c.is_whitespace() && *c != ',').collect();
    if limpio.is_empty() {
        return Err(ErrorDominio::ImporteIlegible { texto: texto.to_string() });
    }

    let (negativo, cuerpo) = match limpio.strip_prefix('-') {
        Some(resto) => (true, resto),
        None => (false, limpio.strip_prefix('+').unwrap_or(&limpio)),
    };

    let (entera, decimal) = match cuerpo.split_once('.') {
        Some((e, d)) => (e, d),
        None => (cuerpo, ""),
    };

    // Una parte entera vacía es legítima en «.50»; ambas vacías, no.
    if entera.is_empty() && decimal.is_empty() {
        return Err(ErrorDominio::ImporteIlegible { texto: texto.to_string() });
    }
    if !entera.chars().all(|c| c.is_ascii_digit()) || !decimal.chars().all(|c| c.is_ascii_digit()) {
        return Err(ErrorDominio::ImporteIlegible { texto: texto.to_string() });
    }

    let unidades: i128 = if entera.is_empty() {
        0
    } else {
        entera.parse().map_err(|_| ErrorDominio::ImporteIlegible { texto: texto.to_string() })?
    };

    // Los decimales se completan o se recortan a centavos. Al recortar se
    // decide con la misma regla que el resto del sistema, usando el primer
    // dígito sobrante para saber hacia dónde.
    let centavos_decimales: i128 = match decimal.len() {
        0 => 0,
        1 => decimal.parse::<i128>().unwrap() * 10,
        2 => decimal.parse::<i128>().unwrap(),
        _ => {
            let dos: i128 = decimal[..2].parse().unwrap();
            let resto = &decimal[2..];
            let sube = resto.as_bytes()[0] >= b'5';
            dos + i128::from(sube)
        }
    };

    let total = unidades
        .checked_mul(CENTAVOS_POR_UNIDAD as i128)
        .and_then(|u| u.checked_add(centavos_decimales))
        .ok_or(ErrorDominio::ImporteIlegible { texto: texto.to_string() })?;

    let total = if negativo { -total } else { total };

    if total.abs() > MAX_CENTAVOS_EXACTOS as i128 {
        return Err(ErrorDominio::MontoInvalido { valor: total as f64 });
    }
    Ok(total as i64)
}

/// Una proporción — una retención, un cashback, un interés— como entero.
///
/// Guarda **millonésimas de la fracción**, no del porcentaje: 0.20 % es la
/// fracción 0.002, que son 2 000 millonésimas. Se representa así y no como
/// `f64` porque de ella sale un importe persistente, y un porcentaje que no
/// se puede escribir exactamente en binario contamina cada céntimo que
/// produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Porcentaje {
    millonesimas: i64,
}

impl Porcentaje {
    /// Puntos básicos: 20 pb = 0.20 % = fracción 0.002.
    ///
    /// Un punto básico es una diezmilésima, de modo que a escala de
    /// mil-millonésimas son 100 000 unidades. Se construye con una
    /// multiplicación entera: la conversión es **exacta**, sin residuo de
    /// representación, que es la razón de declarar así las tasas del sistema.
    pub const fn puntos_basicos(pb: i64) -> Porcentaje {
        Porcentaje { millonesimas: pb * (ESCALA_TASA / 10_000) }
    }

    /// Desde una fracción decimal (0.002 para el 0.20 %).
    ///
    /// Es una frontera con el mundo de coma flotante —una tasa leída de la
    /// base o tecleada— y por eso redondea **una sola vez**, aquí.
    pub fn desde_fraccion(fraccion: f64) -> Result<Porcentaje, ErrorDominio> {
        if !fraccion.is_finite() {
            return Err(ErrorDominio::MontoInvalido { valor: fraccion });
        }
        let m = (fraccion * ESCALA_TASA as f64).round();
        if m.abs() > i64::MAX as f64 {
            return Err(ErrorDominio::MontoInvalido { valor: fraccion });
        }
        Ok(Porcentaje { millonesimas: m as i64 })
    }

    /// Desde un porcentaje tal como se escribe (15.0 para el 15 %).
    pub fn desde_porcentaje(porcentaje: f64) -> Result<Porcentaje, ErrorDominio> {
        Porcentaje::desde_fraccion(porcentaje / 100.0)
    }

    pub fn millonesimas(&self) -> i64 {
        self.millonesimas
    }

    /// La fracción en coma flotante, solo para mostrar o comparar.
    pub fn fraccion(&self) -> f64 {
        self.millonesimas as f64 / ESCALA_TASA as f64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Divisa {
    #[serde(rename = "DOP")]
    Dop,
    #[serde(rename = "USD")]
    Usd,
}

impl Divisa {
    pub fn codigo(&self) -> &'static str {
        match self {
            Divisa::Dop => "DOP",
            Divisa::Usd => "USD",
        }
    }

    pub fn desde_codigo(codigo: &str) -> Result<Divisa, ErrorDominio> {
        match codigo.trim().to_uppercase().as_str() {
            "DOP" => Ok(Divisa::Dop),
            "USD" => Ok(Divisa::Usd),
            _ => Err(ErrorDominio::DivisaDesconocida { codigo: codigo.to_string() }),
        }
    }
}

/// Tasa de cambio en pesos dominicanos por un dólar (DOP por 1 USD), que es la
/// convención con la que el usuario la introduce en la interfaz.
#[derive(Debug, Clone, Copy, PartialEq)]
/// Unidades de moneda local por unidad de divisa extranjera.
///
/// Guarda micro-unidades enteras por el mismo motivo que `Porcentaje`: de la
/// tasa sale un importe que se persiste. `nueva` y `valor` son las fronteras
/// con el mundo de coma flotante —la base y la interfaz— y redondean una sola
/// vez al cruzarlas.
pub struct TasaCambio(i64);

impl TasaCambio {
    pub fn nueva(valor: f64) -> Result<TasaCambio, ErrorDominio> {
        if !valor.is_finite() {
            return Err(ErrorDominio::TasaDeCambioInvalida { valor });
        }
        if valor <= 0.0 {
            return Err(ErrorDominio::TasaDeCambioRequerida);
        }
        let micro = (valor * ESCALA_TASA_CAMBIO as f64).round();
        if micro.abs() > i64::MAX as f64 {
            return Err(ErrorDominio::TasaDeCambioInvalida { valor });
        }
        let micro = micro as i64;
        if micro <= 0 {
            // Una tasa positiva pero tan pequeña que se redondea a cero no
            // convertiría nada: es tan inservible como una tasa de cero.
            return Err(ErrorDominio::TasaDeCambioRequerida);
        }
        Ok(TasaCambio(micro))
    }

    pub fn valor(&self) -> f64 {
        self.0 as f64 / ESCALA_TASA_CAMBIO as f64
    }

    pub fn micro_unidades(&self) -> i64 {
        self.0
    }
}

// Sin PartialOrd ni Ord a propósito: ordenar importes de divisas distintas no
// significa nada. Cuando haga falta comparar, será con un método que exija la
// misma divisa, igual que sumar y restar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dinero {
    centavos: i64,
    divisa: Divisa,
}

impl Dinero {
    /// Construye a partir de unidades (pesos o dólares), redondeando a centavos
    /// con la regla comercial: la mitad se aleja del cero.
    ///
    /// Es el constructor de la frontera: los importes que llegan de la interfaz
    /// y de las columnas `REAL` de SQLite entran por aquí.
    /// Construye un importe a partir de los **dígitos que se escribieron**.
    ///
    /// Es la entrada que evita el viaje por coma flotante. `Dinero::nuevo`
    /// recibe un `f64` que **ya no vale** lo que el usuario tecleó: `1234.56`
    /// no existe en binario, y al multiplicarlo por 100 y redondear se decide
    /// un céntimo sobre un número que nunca fue el escrito.
    ///
    /// Los tramos 1 y 2 no eliminaron esa conversión: la **centralizaron**
    /// aquí. Esta función la retira, leyendo la parte entera y la decimal como
    /// enteros y componiéndolas. `"1234.56"` da 123 456 centavos exactos, sin
    /// que ningún `f64` intervenga.
    ///
    /// Acepta más de dos decimales y los decide con la regla del sistema
    /// —mitad alejándose de cero—, porque un importe tecleado con tres
    /// decimales es un error del usuario, no del programa, y rechazarlo sería
    /// un estorbo donde basta decidir.
    pub fn desde_texto(texto: &str, divisa: Divisa) -> Result<Dinero, ErrorDominio> {
        Ok(Dinero { centavos: centavos_desde_texto(texto)?, divisa })
    }

    pub fn nuevo(unidades: f64, divisa: Divisa) -> Result<Dinero, ErrorDominio> {
        if !unidades.is_finite() {
            return Err(ErrorDominio::MontoInvalido { valor: unidades });
        }
        let centavos = (unidades * CENTAVOS_POR_UNIDAD).round();
        if centavos.abs() > MAX_CENTAVOS_EXACTOS {
            return Err(ErrorDominio::MontoInvalido { valor: unidades });
        }
        Ok(Dinero { centavos: centavos as i64, divisa })
    }

    /// Construye a partir de centavos exactos, sin redondeo ni pérdida.
    pub fn de_centavos(centavos: i64, divisa: Divisa) -> Dinero {
        Dinero { centavos, divisa }
    }

    pub fn cero(divisa: Divisa) -> Dinero {
        Dinero { centavos: 0, divisa }
    }

    pub fn centavos(&self) -> i64 {
        self.centavos
    }

    /// Importe en unidades. Solo para la frontera con SQLite y la interfaz.
    pub fn unidades(&self) -> f64 {
        self.centavos as f64 / CENTAVOS_POR_UNIDAD
    }

    pub fn divisa(&self) -> Divisa {
        self.divisa
    }

    pub fn es_cero(&self) -> bool {
        self.centavos == 0
    }

    pub fn es_negativo(&self) -> bool {
        self.centavos < 0
    }

    /// El mismo importe con el signo invertido.
    ///
    /// Existe para expresar un movimiento inverso —revertir un cargo es
    /// aplicar su negado— sin que quien lo hace tenga que construir el
    /// importe de nuevo y arriesgarse a redondearlo dos veces.
    pub fn negado(&self) -> Dinero {
        Dinero { centavos: -self.centavos, divisa: self.divisa }
    }

    pub fn sumar(&self, otro: &Dinero) -> Result<Dinero, ErrorDominio> {
        self.exigir_misma_divisa(otro)?;
        Ok(Dinero { centavos: self.centavos + otro.centavos, divisa: self.divisa })
    }

    pub fn restar(&self, otro: &Dinero) -> Result<Dinero, ErrorDominio> {
        self.exigir_misma_divisa(otro)?;
        Ok(Dinero { centavos: self.centavos - otro.centavos, divisa: self.divisa })
    }

    /// Aplica una tasa proporcional redondeando a centavos.
    ///
    /// **Único punto del dominio donde un cálculo puede perder precisión.**
    /// Concentrarlo aquí es lo que impide que una retención o un descuento
    /// arrastren fracciones de centavo hasta los saldos.
    pub fn porcentaje(&self, tasa: Porcentaje) -> Result<Dinero, ErrorDominio> {
        let centavos = dividir_redondeando(
            self.centavos as i128 * tasa.millonesimas() as i128,
            ESCALA_TASA as i128,
        );
        exigir_centavos_representables(centavos, self.divisa)
    }

    /// Convierte a la divisa destino redondeando a centavos. Si ya está en esa
    /// divisa devuelve el mismo importe y la tasa se ignora.
    pub fn convertir(&self, destino: Divisa, tasa: TasaCambio) -> Result<Dinero, ErrorDominio> {
        if self.divisa == destino {
            return Ok(*self);
        }
        let micro = tasa.micro_unidades() as i128;
        let centavos = match (self.divisa, destino) {
            (Divisa::Usd, Divisa::Dop) => {
                dividir_redondeando(self.centavos as i128 * micro, ESCALA_TASA_CAMBIO as i128)
            }
            (Divisa::Dop, Divisa::Usd) => {
                dividir_redondeando(self.centavos as i128 * ESCALA_TASA_CAMBIO as i128, micro)
            }
            _ => unreachable!("la igualdad de divisas ya se descartó arriba"),
        };

        exigir_centavos_representables(centavos, destino)
    }

    fn exigir_misma_divisa(&self, otro: &Dinero) -> Result<(), ErrorDominio> {
        if self.divisa != otro.divisa {
            return Err(ErrorDominio::DivisasIncompatibles {
                esperada: self.divisa,
                recibida: otro.divisa,
            });
        }
        Ok(())
    }
}

/// Comprueba que el resultado sigue cabiendo donde debe.
///
/// El límite es el mayor entero que `f64` representa sin pérdida. Se conserva
/// aunque la aritmética ya sea entera, porque los importes siguen cruzando la
/// frontera con SQLite como `REAL`: pasar de ahí haría que el número guardado
/// dejara de ser el calculado.
fn exigir_centavos_representables(
    centavos: i128,
    divisa: Divisa,
) -> Result<Dinero, ErrorDominio> {
    if centavos.abs() > MAX_CENTAVOS_EXACTOS as i128 {
        return Err(ErrorDominio::MontoInvalido { valor: centavos as f64 });
    }
    Ok(Dinero { centavos: centavos as i64, divisa })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dop(unidades: f64) -> Dinero {
        Dinero::nuevo(unidades, Divisa::Dop).unwrap()
    }
    fn usd(unidades: f64) -> Dinero {
        Dinero::nuevo(unidades, Divisa::Usd).unwrap()
    }

    // --- Divisa ---

    #[test]
    fn el_codigo_de_divisa_coincide_con_el_almacenado_en_sqlite() {
        assert_eq!(Divisa::Dop.codigo(), "DOP");
        assert_eq!(Divisa::Usd.codigo(), "USD");
    }

    #[test]
    fn la_divisa_se_serializa_como_el_texto_que_espera_la_interfaz() {
        assert_eq!(serde_json::to_string(&Divisa::Dop).unwrap(), "\"DOP\"");
        assert_eq!(serde_json::to_string(&Divisa::Usd).unwrap(), "\"USD\"");
    }

    #[test]
    fn el_codigo_se_interpreta_sin_distinguir_mayusculas_ni_espacios() {
        assert_eq!(Divisa::desde_codigo("dop").unwrap(), Divisa::Dop);
        assert_eq!(Divisa::desde_codigo("  USD ").unwrap(), Divisa::Usd);
    }

    #[test]
    fn una_divisa_no_admitida_es_rechazada() {
        let e = Divisa::desde_codigo("EUR").unwrap_err();
        assert_eq!(e, ErrorDominio::DivisaDesconocida { codigo: "EUR".into() });
    }

    // --- Representación en centavos ---

    #[test]
    fn el_importe_se_guarda_como_centavos_enteros() {
        assert_eq!(dop(1234.56).centavos(), 123_456);
        assert_eq!(dop(0.01).centavos(), 1);
        assert_eq!(dop(-50.0).centavos(), -5_000);
    }

    #[test]
    fn las_unidades_devuelven_el_importe_para_la_frontera_con_sqlite() {
        assert_eq!(dop(1234.56).unidades(), 1234.56);
        assert_eq!(Dinero::de_centavos(1, Divisa::Dop).unidades(), 0.01);
    }

    #[test]
    fn una_fraccion_de_centavo_se_redondea_al_construir() {
        // Es la clase de valor que produce una tasa aplicada sin redondear, y
        // que arrastraba decimales sobrantes hasta los saldos.
        assert_eq!(dop(24.69134).centavos(), 2469);
        assert_eq!(dop(102.34567).centavos(), 10235);
        assert_eq!(dop(0.005).centavos(), 1, "la mitad se aleja del cero");
        assert_eq!(dop(-0.005).centavos(), -1);
    }

    #[test]
    fn sumar_muchos_centavos_no_acumula_error() {
        // 0.1 + 0.2 != 0.3 en coma flotante; en centavos es exacto.
        let mut total = Dinero::cero(Divisa::Dop);
        for _ in 0..1000 {
            total = total.sumar(&dop(0.10)).unwrap();
        }
        assert_eq!(total.centavos(), 10_000);
        assert_eq!(total.unidades(), 100.0);
    }

    #[test]
    fn un_monto_no_finito_o_fuera_de_rango_es_rechazado() {
        assert!(Dinero::nuevo(f64::NAN, Divisa::Dop).is_err());
        assert!(Dinero::nuevo(f64::INFINITY, Divisa::Dop).is_err());
        assert!(Dinero::nuevo(1e18, Divisa::Dop).is_err(), "no debe saturar en silencio");
    }

    #[test]
    fn el_cero_conserva_su_divisa() {
        let c = Dinero::cero(Divisa::Usd);
        assert!(c.es_cero());
        assert_eq!(c.divisa(), Divisa::Usd);
    }

    // --- Regla central: no se mezclan divisas ---

    #[test]
    fn sumar_pesos_con_dolares_devuelve_error_en_vez_de_un_total_falso() {
        let e = dop(1000.0).sumar(&usd(50.0)).unwrap_err();
        assert_eq!(
            e,
            ErrorDominio::DivisasIncompatibles { esperada: Divisa::Dop, recibida: Divisa::Usd }
        );
    }

    #[test]
    fn restar_divisas_distintas_tambien_es_error() {
        assert!(usd(50.0).restar(&dop(1000.0)).is_err());
    }

    #[test]
    fn sumar_y_restar_en_la_misma_divisa_opera_con_normalidad() {
        assert_eq!(dop(1000.0).sumar(&dop(250.5)).unwrap(), dop(1250.5));
        assert_eq!(dop(1000.0).restar(&dop(250.5)).unwrap(), dop(749.5));
    }

    #[test]
    fn restar_por_debajo_de_cero_esta_permitido_a_nivel_de_tipo() {
        // El sobregiro de tarjetas es legítimo: el saldo insuficiente lo decide
        // cada caso de uso, no el tipo Dinero.
        let r = dop(100.0).restar(&dop(150.0)).unwrap();
        assert_eq!(r.centavos(), -5_000);
        assert!(r.es_negativo());
    }

    #[test]
    fn el_negado_invierte_el_signo_y_conserva_la_divisa() {
        assert_eq!(dop(150.0).negado(), dop(-150.0));
        assert_eq!(dop(-150.0).negado(), dop(150.0));
        assert_eq!(dop(0.0).negado(), dop(0.0), "cero no tiene signo");

        let usd = Dinero::nuevo(25.0, Divisa::Usd).unwrap();
        assert_eq!(usd.negado().divisa(), Divisa::Usd);
    }

    #[test]
    fn negar_dos_veces_devuelve_el_importe_original() {
        // La propiedad de la que depende que revertir un cargo sea su inverso
        // exacto: aplicar el negado no puede perder ni un centavo.
        for unidades in [0.01, 1234.56, -99.99, 0.0] {
            let d = dop(unidades);
            assert_eq!(d.negado().negado(), d, "con {unidades}");
        }
    }

    #[test]
    fn sumar_el_negado_equivale_a_restar() {
        let a = dop(1000.0);
        let b = dop(250.5);
        assert_eq!(a.sumar(&b.negado()).unwrap(), a.restar(&b).unwrap());
    }

    // --- Porcentaje: el único punto donde se redondea ---

    #[test]
    fn el_porcentaje_redondea_a_centavos_y_no_a_unidades() {
        // 1250.00 × 0.20 % = 2.50 exactos. Antes se redondeaba a 3.00.
        assert_eq!(dop(1250.0).porcentaje(Porcentaje::puntos_basicos(20)).unwrap(), dop(2.50));
        // 1200.00 × 0.20 % = 2.40
        assert_eq!(dop(1200.0).porcentaje(Porcentaje::puntos_basicos(20)).unwrap(), dop(2.40));
        // 100.00 × 0.20 % = 0.20
        assert_eq!(dop(100.0).porcentaje(Porcentaje::puntos_basicos(20)).unwrap(), dop(0.20));
    }

    #[test]
    fn el_porcentaje_de_un_importe_redondo_es_exacto() {
        assert_eq!(dop(10000.0).porcentaje(Porcentaje::puntos_basicos(20)).unwrap(), dop(20.0));
    }

    #[test]
    fn el_porcentaje_redondea_la_mitad_alejandose_del_cero() {
        // 12.50 × 0.20 % = 0.025 unidades = 2.5 centavos → 3 centavos
        assert_eq!(dop(12.50).porcentaje(Porcentaje::puntos_basicos(20)).unwrap().centavos(), 3);
    }

    #[test]
    fn el_porcentaje_conserva_la_divisa() {
        assert_eq!(usd(1000.0).porcentaje(Porcentaje::puntos_basicos(20)).unwrap().divisa(), Divisa::Usd);
    }

    #[test]
    fn una_tasa_no_finita_se_rechaza_al_construirla_y_ya_no_al_usarla() {
        // Antes `porcentaje` recibía un `f64` y tenía que defenderse de NaN
        // en cada llamada. Ahora recibe un `Porcentaje`, que no puede
        // construirse con uno: la comprobación vive en la frontera y el tipo
        // hace irrepresentable el caso aguas abajo.
        assert!(Porcentaje::desde_fraccion(f64::NAN).is_err());
        assert!(Porcentaje::desde_fraccion(f64::INFINITY).is_err());
    }

    // --- El céntimo se decide en un solo sitio ---

    #[test]
    fn el_resultado_no_cambia_respecto_al_calculo_anterior() {
        // Importes sintéticos elegidos por su **forma**, no copiados de
        // ninguna operación: cada uno reproduce una propiedad que el cálculo
        // anterior resolvía de una manera concreta, y comprueba que la
        // aritmética entera llega al mismo céntimo.

        // Conversión seguida de su comisión: el camino más largo del sistema.
        let convertido = usd(2_500.75)
            .convertir(Divisa::Dop, TasaCambio::nueva(60.5).unwrap())
            .unwrap();
        assert_eq!(convertido, dop(151_295.38), "151 295.375 sube por mitad exacta");
        assert_eq!(
            convertido.porcentaje(Porcentaje::puntos_basicos(20)).unwrap(),
            dop(302.59),
            "302.59075 baja"
        );

        // Fracciones de céntimo a cada lado de la mitad.
        assert_eq!(dop(12_345.67).porcentaje(Porcentaje::puntos_basicos(20)).unwrap(), dop(24.69));
        assert_eq!(dop(54_321.09).porcentaje(Porcentaje::puntos_basicos(20)).unwrap(), dop(108.64));
        assert_eq!(dop(10_000.50).porcentaje(Porcentaje::puntos_basicos(20)).unwrap(), dop(20.00));
        assert_eq!(usd(150.00).porcentaje(Porcentaje::puntos_basicos(20)).unwrap(), usd(0.30));
    }

    // --- Los dígitos escritos, sin pasar por binario ---

    #[test]
    fn el_texto_se_lee_como_centavos_exactos() {
        assert_eq!(centavos_desde_texto("1234.56").unwrap(), 123_456);
        assert_eq!(centavos_desde_texto("0.01").unwrap(), 1);
        assert_eq!(centavos_desde_texto("100").unwrap(), 10_000);
        assert_eq!(centavos_desde_texto("100.5").unwrap(), 10_050, "un decimal se completa");
        assert_eq!(centavos_desde_texto(".50").unwrap(), 50, "sin parte entera");
        assert_eq!(centavos_desde_texto("-42.75").unwrap(), -4_275);
    }

    #[test]
    fn el_texto_evita_el_error_que_la_coma_flotante_introducia() {
        // **La razón de que esto exista.** Por `f64`, 1.005 se guarda como
        // 1.00499… y al multiplicar por 100 da 100.4999…, que baja a 1.00. Por
        // texto no hay nada que aproximar: los dígitos son los dígitos.
        assert_eq!(centavos_desde_texto("1.005").unwrap(), 101, "el 5 sube, sin binario");
        assert_eq!(Dinero::nuevo(1.005, Divisa::Dop).unwrap().centavos(), 100, "por f64 baja");

        // Y el caso simétrico, donde la coma flotante acertaba por accidente.
        assert_eq!(centavos_desde_texto("2.675").unwrap(), 268);
        assert_eq!(Dinero::nuevo(2.675, Divisa::Dop).unwrap().centavos(), 268);
    }

    #[test]
    fn un_tercer_decimal_se_decide_con_la_regla_del_sistema() {
        // Mitad alejándose de cero, igual que `dividir_redondeando`.
        assert_eq!(centavos_desde_texto("10.004").unwrap(), 1_000);
        assert_eq!(centavos_desde_texto("10.005").unwrap(), 1_001);
        assert_eq!(centavos_desde_texto("10.009").unwrap(), 1_001);
        assert_eq!(centavos_desde_texto("-10.005").unwrap(), -1_001, "también en negativo");
    }

    #[test]
    fn se_admite_lo_que_un_formulario_produce() {
        assert_eq!(centavos_desde_texto("  1234.56  ").unwrap(), 123_456, "espacios");
        assert_eq!(centavos_desde_texto("1,234.56").unwrap(), 123_456, "separador de millar");
        assert_eq!(centavos_desde_texto("+50.00").unwrap(), 5_000, "signo explícito");
    }

    #[test]
    fn lo_que_no_es_un_importe_se_rechaza_nombrando_el_motivo() {
        // Devolver cero ante un texto ilegible es la forma habitual de que un
        // importe desaparezca sin que nadie se entere.
        for basura in ["", "  ", "abc", "1.2.3", "12a", "-", ".", "1e5"] {
            let r = centavos_desde_texto(basura);
            assert!(r.is_err(), "aceptó «{basura}»");
            assert!(
                format!("{}", r.unwrap_err()).contains("no es un importe"),
                "el error no explica qué pasó con «{basura}»"
            );
        }
    }

    #[test]
    fn el_texto_y_la_coma_flotante_coinciden_salvo_donde_esta_pierde() {
        // Barrido: para importes con dos decimales —lo que un formulario
        // produce normalmente— ambos caminos dan lo mismo. La diferencia
        // aparece solo con un tercer decimal, que es donde `f64` decide sobre
        // un número que no es el escrito.
        let mut divergencias = 0;
        for centavos in (1..200_000).step_by(13) {
            let texto = format!("{}.{:02}", centavos / 100, centavos % 100);
            let por_texto = centavos_desde_texto(&texto).unwrap();
            let por_f64 = Dinero::nuevo(texto.parse::<f64>().unwrap(), Divisa::Dop)
                .unwrap()
                .centavos();
            if por_texto != por_f64 {
                divergencias += 1;
            }
            assert_eq!(por_texto, centavos as i64, "el texto no es exacto en {texto}");
        }
        assert_eq!(divergencias, 0, "con dos decimales ambos caminos coinciden");
    }

    #[test]
    fn desde_texto_construye_un_dinero_con_su_divisa() {
        let d = Dinero::desde_texto("1234.56", Divisa::Usd).unwrap();
        assert_eq!(d.centavos(), 123_456);
        assert_eq!(d.divisa(), Divisa::Usd);
    }

    #[test]
    fn un_importe_desmesurado_se_rechaza_en_vez_de_desbordar() {
        assert!(centavos_desde_texto("999999999999999999999.99").is_err());
    }

    // --- La tolerancia de representación, exigida ---

    /// Desvío en centavos que una escala provoca sobre el importe de
    /// referencia, por no poder representar una tasa exactamente.
    fn desvio_por_representacion(escala: i64) -> f64 {
        // Una tasa cae como mucho a media unidad de escala de su valor real.
        let error_de_tasa = 0.5 / escala as f64;
        IMPORTE_DE_REFERENCIA_CENTAVOS as f64 * error_de_tasa
    }

    #[test]
    fn las_escalas_honran_la_tolerancia_de_representacion() {
        // Esta es la prueba que **fija el límite**. Si alguien bajara una
        // escala por comodidad, aquí se entera antes de que un importe se
        // desvíe.
        for (escala, nombre) in [(ESCALA_TASA, "porcentajes"), (ESCALA_TASA_CAMBIO, "tasas")] {
            let desvio = desvio_por_representacion(escala);
            assert!(
                desvio <= TOLERANCIA_REPRESENTACION_CENTAVOS,
                "la escala de {nombre} desvía {desvio} ¢, por encima de {} ¢",
                TOLERANCIA_REPRESENTACION_CENTAVOS
            );
        }
    }

    #[test]
    fn una_escala_de_millonesimas_no_habria_bastado() {
        // Deja constancia de por qué la escala es la que es. Seis decimales
        // bastan para *escribir* un 0.20 %, pero no para representarlo dentro
        // de la tolerancia sobre un importe corriente.
        assert!(
            desvio_por_representacion(1_000_000) > TOLERANCIA_REPRESENTACION_CENTAVOS,
            "si esto deja de ser cierto, la justificación de la escala cambió"
        );
    }

    #[test]
    fn las_tasas_declaradas_del_sistema_no_tienen_residuo_de_representacion() {
        // Un punto básico cabe exacto en la escala, así que las tasas que el
        // sistema declara —la retención, los cashback— no aportan desviación
        // ninguna: toda la que queda es la del redondeo final.
        for pb in [1_i64, 20, 100, 300, 500, 1500, 10_000] {
            let p = Porcentaje::puntos_basicos(pb);
            let exacta = Porcentaje::desde_fraccion(pb as f64 / 10_000.0).unwrap();
            assert_eq!(p, exacta, "{pb} pb no se representa exacto");
        }
    }

    #[test]
    fn una_tasa_derivada_se_mantiene_dentro_de_la_tolerancia() {
        // El interés mensual de un préstamo es un decimal periódico: no cabe
        // exacto en ninguna escala. Lo que sí se exige es que su desviación
        // sobre el importe de referencia quede bajo la tolerancia.
        let mensual = 30.95 / 100.0 / 12.0; // 0.0257916666…
        let p = Porcentaje::desde_fraccion(mensual).unwrap();

        let referencia = Dinero::de_centavos(IMPORTE_DE_REFERENCIA_CENTAVOS, Divisa::Dop);
        let calculado = referencia.porcentaje(p).unwrap().centavos() as f64;
        let exacto = IMPORTE_DE_REFERENCIA_CENTAVOS as f64 * mensual;

        // Se descuenta el medio centavo del redondeo final, que no es error.
        let desvio = (calculado - exacto).abs() - 0.5;
        assert!(
            desvio <= TOLERANCIA_REPRESENTACION_CENTAVOS,
            "desvío de representación {desvio} ¢ sobre la tolerancia"
        );
    }

    #[test]
    fn el_residuo_del_redondeo_final_llega_a_medio_centavo_y_eso_es_correcto() {
        // La otra cara del límite: no se puede exigir 0.01 ¢ al redondeo
        // final, porque su residuo es medio centavo por definición. Confundir
        // ambas cosas llevaría a rechazar comisiones corrientes.
        let importe = dop(8_967.90);
        let comision = importe.porcentaje(Porcentaje::puntos_basicos(20)).unwrap();

        let exacto = importe.centavos() as f64 * 0.002;
        let residuo = (comision.centavos() as f64 - exacto).abs();

        assert!(residuo > TOLERANCIA_REPRESENTACION_CENTAVOS, "residuo real de una comisión");
        assert!(residuo <= 0.5, "pero nunca más de medio centavo");
    }

    #[test]
    fn la_division_redondea_mitad_alejandose_de_cero() {
        assert_eq!(dividir_redondeando(50, 100), 1, "0.5 sube");
        assert_eq!(dividir_redondeando(49, 100), 0);
        assert_eq!(dividir_redondeando(-50, 100), -1, "-0.5 baja");
        assert_eq!(dividir_redondeando(-49, 100), 0);
        assert_eq!(dividir_redondeando(150, 100), 2, "1.5 sube");
        assert_eq!(dividir_redondeando(-150, 100), -2);
    }

    #[test]
    fn el_redondeo_es_simetrico_en_torno_al_cero() {
        // La propiedad que define «mitad alejándose de cero»: el signo no
        // cambia la magnitud del resultado.
        for n in [1_i128, 49, 50, 51, 99, 100, 101, 12_345] {
            assert_eq!(
                dividir_redondeando(n, 100),
                -dividir_redondeando(-n, 100),
                "asimetría en {n}"
            );
        }
    }

    #[test]
    fn el_caso_que_la_coma_flotante_resolvia_mal() {
        // 1.005 se guarda como 1.00499999…, así que el cálculo antiguo lo
        // bajaba a 1.00 mientras subía 2.675 a 2.68: dos importes de la misma
        // forma, en direcciones opuestas. En enteros ambos suben.
        assert_eq!(dividir_redondeando(1005, 10), 101, "1.005 -> 1.01");
        assert_eq!(dividir_redondeando(2675, 10), 268, "2.675 -> 2.68");
        assert_eq!(dividir_redondeando(145, 10), 15, "0.145 -> 0.15");
    }

    #[test]
    fn el_porcentaje_ya_no_hereda_el_error_de_la_coma_flotante() {
        // 100.01 x 0.20 % = 0.20002, que se decide en 20 céntimos.
        assert_eq!(dop(100.01).porcentaje(Porcentaje::puntos_basicos(20)).unwrap(), dop(0.20));
        // Y el caso simétrico, que debe subir.
        assert_eq!(dop(100.05).porcentaje(Porcentaje::puntos_basicos(50)).unwrap(), dop(0.50));
    }

    #[test]
    fn la_tasa_de_cambio_conserva_su_valor_al_ida_y_vuelta() {
        for v in [59.9, 60.0, 61.25, 0.5, 1.0] {
            let t = TasaCambio::nueva(v).unwrap();
            assert!((t.valor() - v).abs() < 1e-9, "no vuelve {v}");
        }
    }

    #[test]
    fn una_tasa_sobrevive_al_viaje_por_una_columna_real() {
        // **Esta prueba es la que hace innecesario persistir la tasa como
        // entero.** SQLite guarda las tasas en columnas `REAL`, y la duda
        // razonable era si ese viaje pierde micro-unidades.
        //
        // No las pierde: a escala de mil-millonésimas, una tasa del rango
        // plausible necesita unos doce dígitos significativos y `f64` ofrece
        // quince. Hay holgura de sobra.
        //
        // Lo que esta prueba protege es justamente esa holgura: si algún día
        // se subiera `ESCALA_TASA_CAMBIO` hasta agotarla, el viaje empezaría a
        // perder y aquí se sabría antes de que un importe se desviara.
        let mut comprobadas = 0;
        for milesimas in (1..200_000).step_by(37) {
            let valor = milesimas as f64 / 1_000.0;
            let original = TasaCambio::nueva(valor).unwrap();

            // Ida y vuelta por la columna: el `f64` que se guarda y se relee.
            let guardado = original.valor();
            let recuperada = TasaCambio::nueva(guardado).unwrap();

            assert_eq!(
                recuperada.micro_unidades(),
                original.micro_unidades(),
                "la tasa {valor} no sobrevive a la columna REAL"
            );
            comprobadas += 1;
        }
        assert!(comprobadas > 5_000, "el barrido se quedó corto: {comprobadas}");
    }

    #[test]
    fn un_porcentaje_tambien_sobrevive_al_viaje_por_una_columna_real() {
        // Mismo argumento para las tasas de interés, que sí se guardan como
        // `REAL` y llegan desde la base en cada cálculo de cuota.
        for centesimas in (1..10_000).step_by(7) {
            let fraccion = centesimas as f64 / 10_000.0;
            let original = Porcentaje::desde_fraccion(fraccion).unwrap();
            let recuperado = Porcentaje::desde_fraccion(original.fraccion()).unwrap();

            assert_eq!(
                recuperado.millonesimas(),
                original.millonesimas(),
                "el porcentaje {fraccion} no sobrevive"
            );
        }
    }

    #[test]
    fn una_tasa_positiva_pero_despreciable_se_rechaza() {
        // Redondearía a cero micro-unidades y no convertiría nada: es tan
        // inservible como una tasa de cero, y decirlo evita un importe nulo
        // silencioso.
        // El umbral es la escala: con mil-millonésimas, 1e-9 todavía se
        // representa y 1e-10 ya no.
        assert!(TasaCambio::nueva(1e-9).is_ok(), "1e-9 es exactamente una micro-unidad");
        assert_eq!(
            TasaCambio::nueva(1e-10).unwrap_err(),
            ErrorDominio::TasaDeCambioRequerida
        );
    }

    // --- Tasa de cambio ---

    #[test]
    fn una_tasa_de_cero_pide_la_tasa_en_lugar_de_asumir_una() {
        assert_eq!(TasaCambio::nueva(0.0).unwrap_err(), ErrorDominio::TasaDeCambioRequerida);
    }

    #[test]
    fn una_tasa_negativa_o_no_finita_es_invalida() {
        assert_eq!(TasaCambio::nueva(-60.0).unwrap_err(), ErrorDominio::TasaDeCambioRequerida);
        assert!(matches!(
            TasaCambio::nueva(f64::NAN).unwrap_err(),
            ErrorDominio::TasaDeCambioInvalida { .. }
        ));
    }

    // --- Conversión ---

    #[test]
    fn abonar_500_usd_con_tasa_60_debita_30000_pesos() {
        let tasa = TasaCambio::nueva(60.0).unwrap();
        assert_eq!(usd(500.0).convertir(Divisa::Dop, tasa).unwrap(), dop(30000.0));
    }

    #[test]
    fn la_conversion_inversa_divide_por_la_tasa() {
        let tasa = TasaCambio::nueva(60.0).unwrap();
        assert_eq!(dop(30000.0).convertir(Divisa::Usd, tasa).unwrap(), usd(500.0));
    }

    #[test]
    fn convertir_a_la_misma_divisa_no_altera_el_importe() {
        let tasa = TasaCambio::nueva(60.0).unwrap();
        assert_eq!(usd(500.0).convertir(Divisa::Usd, tasa).unwrap(), usd(500.0));
    }

    #[test]
    fn la_conversion_redondea_a_centavos() {
        // 100.00 USD a 58.755 = 5875.50 DOP exactos
        let tasa = TasaCambio::nueva(58.755).unwrap();
        let convertido = usd(100.0).convertir(Divisa::Dop, tasa).unwrap();
        assert_eq!(convertido.centavos(), 587_550);
    }

    #[test]
    fn ida_y_vuelta_recupera_el_importe_original() {
        let tasa = TasaCambio::nueva(58.75).unwrap();
        let vuelta = usd(500.0)
            .convertir(Divisa::Dop, tasa)
            .unwrap()
            .convertir(Divisa::Usd, tasa)
            .unwrap();
        assert_eq!(vuelta, usd(500.0));
    }
}
