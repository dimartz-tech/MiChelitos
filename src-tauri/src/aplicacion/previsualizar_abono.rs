//! Caso de uso: la vista previa de un abono, **sin registrarlo**.
//!
//! Responde, antes de confirmar, a la pregunta que más abonos revertidos habría evitado: «¿qué va a pasar si pulso
//! Abonar?». Muestra de qué cuenta saldría el dinero (incluida la caja de efectivo por defecto cuando no se eligió
//! ninguna), cuánto se debitaría con su comisión, cómo quedarían la cuenta y la tarjeta, y en qué casos conviene
//! detenerse.
//!
//! ## Una sola fórmula
//!
//! Nada del cálculo se repite aquí: la cuenta de cobro sale de `registrar_pago_tarjeta::cuenta_de_cobro` y los importes
//! de `dominio::abono::calcular_abono`, las mismas funciones que usa el registro. La prueba de propiedad de este módulo
//! compara la vista previa con el estado real tras registrar el abono sobre una copia del mismo estado.
//!
//! ## Solo lectura
//!
//! Recibe el almacén por referencia compartida (`&`): el compilador impide cualquier escritura. Los comandos lo
//! invocan en una transacción diferida (`lee_*`).

use super::registrar_pago_tarjeta::cuenta_de_cobro;
use super::ErrorAplicacion;
use crate::dominio::abono::{calcular_abono, Advertencia, CalculoDeAbono, Gravedad, TipoDeAbono};
use crate::dominio::dinero::{Dinero, Divisa, TasaCambio};
use crate::dominio::errores::ErrorDominio;
use crate::puertos::repositorios::*;

/// Lo que el formulario tiene escrito en este momento.
#[derive(Debug, Clone)]
pub struct DatosDePrevia {
    pub tarjeta_id: i64,
    /// El monto tal como está en el campo (vacío o ilegible es una advertencia, no un error).
    pub monto: String,
    pub divisa: Divisa,
    pub cuenta_ahorro_id: Option<i64>,
    /// `None` es «sin tasa» (la interfaz manda cero).
    pub tasa_cambio: Option<f64>,
    pub tipo: TipoDeAbono,
}

/// La cuenta de la que saldría el dinero.
#[derive(Debug, Clone, PartialEq)]
pub struct CuentaDeCobro {
    pub id: i64,
    pub nombre: String,
    pub divisa: Divisa,
    pub saldo_actual: Dinero,
    /// `true` si no se eligió cuenta y es la caja de efectivo de la divisa del abono.
    pub por_defecto: bool,
    pub archivada: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VistaPreviaDeAbono {
    pub tarjeta_id: i64,
    pub tarjeta_nombre: String,
    pub tarjeta_activa: bool,
    pub divisa: Divisa,
    /// `None` si el monto no es válido.
    pub monto: Option<Dinero>,
    pub tipo: TipoDeAbono,
    pub cuenta: Option<CuentaDeCobro>,
    pub con_comision: bool,
    /// `None` mientras falte el monto, la cuenta o la tasa.
    pub calculo: Option<CalculoDeAbono>,
    /// La tasa usada, solo si el abono cruza divisas.
    pub tasa: Option<TasaCambio>,
    pub saldo_resultante: Option<Dinero>,
    pub balance_actual_dop: Dinero,
    pub balance_actual_usd: Dinero,
    pub balance_resultante_dop: Dinero,
    pub balance_resultante_usd: Dinero,
    /// Primero los bloqueos, luego los avisos y al final los datos.
    pub advertencias: Vec<Advertencia>,
    /// `false` si alguna advertencia bloquea o falta con qué calcular: el registro lo rechazaría.
    pub se_puede_confirmar: bool,
}

pub fn previsualizar_abono(
    datos: DatosDePrevia,
    almacen: &(impl ConsultaParaAbono + BusquedaDeCuentas + RepositorioCuentas),
) -> Result<VistaPreviaDeAbono, ErrorAplicacion> {
    let tarjeta = almacen.situacion_de_tarjeta(datos.tarjeta_id)?;
    let mut advertencias: Vec<Advertencia> = Vec::new();
    if !tarjeta.activa {
        advertencias.push(Advertencia::TarjetaInactiva { nombre: tarjeta.nombre.clone() });
    }

    // El monto: el registro exige un importe positivo (`AbonoSinImporte`).
    let monto = match Dinero::desde_texto(datos.monto.trim(), datos.divisa) {
        Ok(m) if m.centavos() > 0 => Some(m),
        _ => {
            advertencias.push(Advertencia::MontoNoValido);
            None
        }
    };

    // La cuenta efectiva: la misma resolución que el registro.
    let por_defecto = datos.cuenta_ahorro_id.is_none();
    let mut con_comision = false;
    let mut cuenta: Option<CuentaDeCobro> = None;
    match cuenta_de_cobro(datos.divisa, datos.cuenta_ahorro_id, almacen) {
        Ok((id, comisiona)) => {
            con_comision = comisiona;
            match almacen.estado_de_cuenta(id) {
                Ok(estado) => {
                    let divisa = almacen.divisa(id)?;
                    if estado.archivada {
                        advertencias.push(Advertencia::CuentaArchivada { nombre: estado.nombre.clone() });
                    }
                    if por_defecto {
                        advertencias.push(Advertencia::EfectivoPorDefecto { caja: estado.nombre.clone() });
                    }
                    if divisa != datos.divisa {
                        advertencias.push(Advertencia::CruzaDivisas { divisa_abono: datos.divisa, divisa_cuenta: divisa });
                    }
                    cuenta = Some(CuentaDeCobro {
                        id,
                        nombre: estado.nombre,
                        divisa,
                        saldo_actual: estado.saldo,
                        por_defecto,
                        archivada: estado.archivada,
                    });
                }
                Err(ErrorAlmacen::NoEncontrado { .. }) => advertencias.push(Advertencia::CuentaNoEncontrada),
                Err(otro) => return Err(otro.into()),
            }
        }
        Err(ErrorAplicacion::Dominio(ErrorDominio::CajaDeAbonoNoEncontrada { nombre })) => {
            advertencias.push(Advertencia::CajaNoEncontrada { nombre });
        }
        Err(otro) => return Err(otro),
    }

    // La tasa, tal como la entiende el registro: positiva o ausente.
    let tasa = match datos.tasa_cambio {
        None => None,
        Some(v) => match TasaCambio::nueva(v) {
            Ok(t) => Some(t),
            Err(_) => {
                advertencias.push(Advertencia::TasaNoValida);
                None
            }
        },
    };

    // El cálculo de dinero es el del registro.
    let mut calculo = None;
    if let (Some(m), Some(c)) = (monto, cuenta.as_ref()) {
        match calcular_abono(m, c.divisa, tasa, con_comision) {
            Ok(r) => calculo = Some(r),
            Err(ErrorDominio::TasaDeCambioRequerida) => {
                if datos.tasa_cambio.is_none() {
                    advertencias.push(Advertencia::FaltaTasa);
                }
            }
            Err(otro) => advertencias.push(Advertencia::NoSePuedeCalcular { motivo: otro.to_string() }),
        }
    }

    // Saldo resultante de la cuenta.
    let mut saldo_resultante = None;
    if let (Some(r), Some(c)) = (calculo.as_ref(), cuenta.as_ref()) {
        let resultante = c.saldo_actual.restar(&r.total_debitado).map_err(ErrorAplicacion::from)?;
        if resultante.es_negativo() {
            advertencias.push(Advertencia::CuentaQuedariaEnNegativo { saldo_resultante: resultante });
        }
        saldo_resultante = Some(resultante);
    }

    // Balance resultante de la tarjeta: el abono solo mueve la divisa en que se hace.
    let balance_actual = |divisa: Divisa| match divisa {
        Divisa::Dop => tarjeta.balance_dop,
        Divisa::Usd => tarjeta.balance_usd,
    };
    let (mut resultante_dop, mut resultante_usd) = (tarjeta.balance_dop, tarjeta.balance_usd);
    if let Some(m) = monto {
        let antes = balance_actual(datos.divisa);
        let despues = antes.restar(&m).map_err(ErrorAplicacion::from)?;
        match datos.divisa {
            Divisa::Dop => resultante_dop = despues,
            Divisa::Usd => resultante_usd = despues,
        }
        if despues.es_negativo() {
            advertencias.push(Advertencia::SuperaElBalance { balance: antes, a_favor: despues.negado() });
        }
        if datos.tipo != TipoDeAbono::Personalizado {
            let referencia = match (datos.tipo, datos.divisa) {
                (TipoDeAbono::Corte, Divisa::Dop) => tarjeta.corte_dop,
                (TipoDeAbono::Corte, Divisa::Usd) => tarjeta.corte_usd,
                _ => antes,
            };
            if referencia != m {
                advertencias.push(Advertencia::MontoNoCoincideConElTipo { tipo: datos.tipo, referencia });
            }
        }
    }

    advertencias.sort_by_key(|a| match a.gravedad() {
        Gravedad::Bloqueo => 0,
        Gravedad::Aviso => 1,
        Gravedad::Info => 2,
    });
    let se_puede_confirmar = calculo.is_some() && !advertencias.iter().any(|a| a.gravedad() == Gravedad::Bloqueo);
    let tasa = calculo.filter(|c| c.convirtio).and(tasa);

    Ok(VistaPreviaDeAbono {
        tarjeta_id: datos.tarjeta_id,
        tarjeta_nombre: tarjeta.nombre.clone(),
        tarjeta_activa: tarjeta.activa,
        divisa: datos.divisa,
        monto,
        tipo: datos.tipo,
        cuenta,
        con_comision,
        calculo,
        tasa,
        saldo_resultante,
        balance_actual_dop: tarjeta.balance_dop,
        balance_actual_usd: tarjeta.balance_usd,
        balance_resultante_dop: resultante_dop,
        balance_resultante_usd: resultante_usd,
        advertencias,
        se_puede_confirmar,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aplicacion::registrar_pago_tarjeta::{registrar_pago_tarjeta, DatosPago};
    use crate::puertos::dobles::AlmacenEnMemoria;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }
    fn usd(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Usd).unwrap()
    }

    /// Estado inventado: cuentas en pesos y dólares, cajas de efectivo y una tarjeta con deuda en las dos divisas.
    fn almacen() -> AlmacenEnMemoria {
        AlmacenEnMemoria::nuevo()
            .con_categoria(1, "Otros")
            .con_cuenta(10, "Cuenta Pesos", dop(5_000.0))
            .con_cuenta(11, "Cuenta Dólares", usd(300.0))
            .con_caja(12, dop(1_000.0))
            .con_caja(13, usd(50.0))
            .con_tarjeta(20, dop(2_750.0))
            .con_tarjeta(20, usd(120.0))
            .con_corte(20, dop(2_314.0))
            .con_corte(20, usd(100.0))
            .con_tarjeta(21, dop(0.0))
    }

    fn previa(tarjeta: i64, monto: &str, divisa: Divisa, cuenta: Option<i64>, tasa: Option<f64>, tipo: TipoDeAbono) -> DatosDePrevia {
        DatosDePrevia { tarjeta_id: tarjeta, monto: monto.into(), divisa, cuenta_ahorro_id: cuenta, tasa_cambio: tasa, tipo }
    }

    fn ver(a: &AlmacenEnMemoria, d: DatosDePrevia) -> VistaPreviaDeAbono {
        previsualizar_abono(d, a).unwrap()
    }

    /// Una huella del estado entero que la vista previa no debe tocar.
    fn huella(a: &AlmacenEnMemoria) -> String {
        let mut cuentas: Vec<_> = a.cuentas.iter().map(|(id, c)| (*id, c.saldo.centavos(), c.archivada)).collect();
        cuentas.sort();
        let mut deudas: Vec<_> = a.deudas.iter().map(|((id, d), v)| (*id, d.codigo(), v.centavos())).collect();
        deudas.sort();
        let mut gastos: Vec<_> = a.gastos.values().map(|g| (g.id, g.monto.centavos())).collect();
        gastos.sort();
        let mut pagos: Vec<_> = a.pagos.keys().copied().collect();
        pagos.sort();
        format!("{cuentas:?}|{deudas:?}|{gastos:?}|{pagos:?}")
    }

    /// Los casos de la propiedad: pesos y dólares, con y sin cuenta, con y sin comisión, saldo a favor y cuenta insuficiente.
    fn casos() -> Vec<(&'static str, DatosDePrevia)> {
        use TipoDeAbono::*;
        vec![
            ("pesos con cuenta (comisión)", previa(20, "1187.50", Divisa::Dop, Some(10), None, Personalizado)),
            ("pesos sin cuenta (caja, sin comisión)", previa(20, "1187.50", Divisa::Dop, None, None, Personalizado)),
            ("dólares con cuenta en dólares", previa(20, "50.00", Divisa::Usd, Some(11), None, Personalizado)),
            ("dólares sin cuenta (caja USD)", previa(20, "50.00", Divisa::Usd, None, None, Personalizado)),
            ("dólares desde cuenta en pesos con tasa", previa(20, "100.00", Divisa::Usd, Some(10), Some(58.75), Corte)),
            ("pesos desde cuenta en dólares con tasa", previa(20, "1187.50", Divisa::Dop, Some(11), Some(58.75), Personalizado)),
            ("saldo a favor (supera el balance)", previa(20, "3177.25", Divisa::Dop, Some(10), None, Personalizado)),
            ("tarjeta sin deuda: todo es saldo a favor", previa(21, "10.00", Divisa::Usd, Some(11), None, Personalizado)),
            ("cuenta insuficiente (queda en negativo)", previa(20, "6200.00", Divisa::Dop, Some(10), None, Actual)),
            ("caja insuficiente", previa(20, "1400.00", Divisa::Dop, None, None, Personalizado)),
            ("un centavo con cuenta", previa(20, "0.01", Divisa::Dop, Some(10), None, Personalizado)),
            ("importe con comisión fraccionaria", previa(20, "33.33", Divisa::Dop, Some(10), None, Personalizado)),
            ("conversión con redondeo", previa(20, "7.77", Divisa::Usd, Some(10), Some(59.123456), Personalizado)),
            ("tasa ignorada sin cruce de divisas", previa(20, "100.00", Divisa::Dop, Some(10), Some(60.0), Personalizado)),
        ]
    }

    fn registrar(a: &mut AlmacenEnMemoria, d: &DatosDePrevia) -> crate::aplicacion::registrar_pago_tarjeta::PagoRegistrado {
        let monto = Dinero::desde_texto(&d.monto, d.divisa).unwrap();
        registrar_pago_tarjeta(
            DatosPago {
                tarjeta_id: d.tarjeta_id,
                fecha: "01/01/2027".into(),
                monto,
                cuenta_ahorro_id: d.cuenta_ahorro_id,
                tasa_cambio: d.tasa_cambio.map(|t| TasaCambio::nueva(t).unwrap()),
            },
            1,
            a,
        )
        .unwrap()
    }

    #[test]
    fn la_vista_previa_coincide_con_el_estado_real_tras_registrar_el_abono() {
        for (caso, datos) in casos() {
            let base = almacen();
            let vista = ver(&base, datos.clone());
            assert!(vista.se_puede_confirmar, "{caso}: debería poder confirmarse ({:?})", vista.advertencias);

            // El mismo estado, copiado, con el abono registrado de verdad.
            let mut real = base.clone();
            let pago = registrar(&mut real, &datos);

            let cuenta = vista.cuenta.as_ref().unwrap();
            let calculo = vista.calculo.unwrap();
            assert_eq!(real.saldo_de(cuenta.id), vista.saldo_resultante.unwrap(), "{caso}: saldo de la cuenta");
            assert_eq!(real.deuda_en(datos.tarjeta_id, Divisa::Dop), vista.balance_resultante_dop, "{caso}: balance en pesos");
            assert_eq!(real.deuda_en(datos.tarjeta_id, Divisa::Usd), vista.balance_resultante_usd, "{caso}: balance en dólares");
            assert_eq!(pago.comision, calculo.comision, "{caso}: comisión");
            assert_eq!(pago.debitado, Some(calculo.debitado), "{caso}: debitado");
            let delta = base.saldo_de(cuenta.id).restar(&real.saldo_de(cuenta.id)).unwrap();
            assert_eq!(delta, calculo.total_debitado, "{caso}: total debitado");
            // La comisión asentada como gasto es la que se mostró.
            let asentada: i64 = real.gastos.values().map(|g| g.monto.centavos()).sum();
            assert_eq!(asentada, calculo.comision.map_or(0, |c| c.centavos()), "{caso}: gasto de comisión");
            // Las demás cuentas no se movieron.
            for id in [10, 11, 12, 13] {
                if id != cuenta.id {
                    assert_eq!(real.saldo_de(id), base.saldo_de(id), "{caso}: la cuenta {id} no debía moverse");
                }
            }
            // Y el mismo origen que el registro: el abono guardado apunta a la cuenta mostrada.
            assert_eq!(real.pagos.values().next().unwrap().cuenta_ahorro_id, Some(cuenta.id), "{caso}: cuenta del abono");
        }
    }

    #[test]
    fn la_vista_previa_no_modifica_nada() {
        let mut escenarios: Vec<AlmacenEnMemoria> = vec![almacen()];
        let mut archivada = almacen();
        archivada.cuentas.get_mut(&10).unwrap().archivada = true;
        escenarios.push(archivada);
        escenarios.push(almacen().con_tarjeta_inactiva(20));
        for base in escenarios {
            for (caso, datos) in casos() {
                let antes = huella(&base);
                let _ = ver(&base, datos);
                assert_eq!(huella(&base), antes, "{caso}: la vista previa tocó el estado");
            }
            for monto in ["", "abc", "0", "-5.00"] {
                let antes = huella(&base);
                let _ = ver(&base, previa(20, monto, Divisa::Dop, Some(10), None, TipoDeAbono::Personalizado));
                assert_eq!(huella(&base), antes);
            }
        }
    }

    #[test]
    fn sin_cuenta_dice_efectivo_por_defecto_en_la_divisa_del_abono() {
        let a = almacen();
        let pesos = ver(&a, previa(20, "100.00", Divisa::Dop, None, None, TipoDeAbono::Personalizado));
        let cuenta = pesos.cuenta.as_ref().unwrap();
        assert_eq!((cuenta.nombre.as_str(), cuenta.por_defecto, cuenta.id), ("Efectivo DOP", true, 12));
        assert!(!pesos.con_comision && pesos.calculo.unwrap().comision.is_none());
        assert!(pesos.advertencias.iter().any(|a| a.codigo() == "efectivo_por_defecto"));
        let dolares = ver(&a, previa(20, "10.00", Divisa::Usd, None, None, TipoDeAbono::Personalizado));
        assert_eq!(dolares.cuenta.unwrap().nombre, "Efectivo USD");
    }

    #[test]
    fn con_cuenta_elegida_hay_comision_y_no_hay_aviso_de_efectivo() {
        let a = almacen();
        let v = ver(&a, previa(20, "1187.50", Divisa::Dop, Some(10), None, TipoDeAbono::Personalizado));
        let c = v.calculo.unwrap();
        assert_eq!(c.comision, Some(dop(2.38)));
        assert_eq!(c.total_debitado, dop(1_189.88));
        assert_eq!(v.saldo_resultante, Some(dop(3_810.12)));
        assert!(v.con_comision);
        assert!(v.advertencias.is_empty(), "{:?}", v.advertencias);
    }

    #[test]
    fn muestra_la_tasa_y_el_equivalente_cuando_cruza_divisas() {
        let a = almacen();
        let v = ver(&a, previa(20, "100.00", Divisa::Usd, Some(10), Some(58.75), TipoDeAbono::Personalizado));
        let c = v.calculo.unwrap();
        assert_eq!(c.debitado, dop(5_875.0));
        assert_eq!(v.tasa.map(|t| t.valor()), Some(58.75));
        assert!(v.advertencias.iter().any(|a| a.codigo() == "cruza_divisas"));
        // Sin cruce la tasa no se muestra.
        let v = ver(&a, previa(20, "100.00", Divisa::Dop, Some(10), Some(58.75), TipoDeAbono::Personalizado));
        assert!(v.tasa.is_none());
    }

    #[test]
    fn cruzar_divisas_sin_tasa_bloquea_con_un_texto_que_la_pide() {
        let a = almacen();
        let v = ver(&a, previa(20, "100.00", Divisa::Usd, Some(10), None, TipoDeAbono::Personalizado));
        assert!(!v.se_puede_confirmar && v.calculo.is_none());
        assert!(v.advertencias.iter().any(|a| a.codigo() == "falta_tasa" && a.texto().contains("tasa")));
        // El balance resultante de la tarjeta sí se conoce: no depende de la cuenta.
        assert_eq!(v.balance_resultante_usd, usd(20.0));
    }

    #[test]
    fn avisa_cuando_la_cuenta_quedaria_en_negativo() {
        let a = almacen();
        let v = ver(&a, previa(20, "4995.00", Divisa::Dop, Some(10), None, TipoDeAbono::Personalizado));
        assert_eq!(v.saldo_resultante, Some(dop(-4.99)), "4 995.00 y 9.99 de comisión sobre un saldo de 5 000");
        let adv = v.advertencias.iter().find(|a| a.codigo() == "cuenta_en_negativo").expect("falta el aviso");
        assert!(adv.texto().contains("-4.99"), "{}", adv.texto());
        assert!(v.se_puede_confirmar, "el registro lo permite (H10): es un aviso, no un bloqueo");
        // Justo sin pasarse, no avisa.
        let v = ver(&a, previa(20, "4990.00", Divisa::Dop, Some(10), None, TipoDeAbono::Personalizado));
        assert_eq!(v.saldo_resultante.unwrap().centavos(), 5_000_00 - 4_990_00 - 998);
        assert!(!v.advertencias.iter().any(|a| a.codigo() == "cuenta_en_negativo"));
    }

    #[test]
    fn avisa_cuando_el_abono_supera_el_balance_y_cuanto_queda_a_favor() {
        let a = almacen();
        let v = ver(&a, previa(20, "3177.25", Divisa::Dop, Some(10), None, TipoDeAbono::Personalizado));
        assert_eq!(v.balance_resultante_dop, dop(-427.25));
        assert_eq!(v.balance_resultante_usd, usd(120.0), "la otra divisa no se mueve");
        let adv = v.advertencias.iter().find(|a| a.codigo() == "supera_el_balance").expect("falta el aviso");
        assert!(adv.texto().contains("427.25"), "{}", adv.texto());
        // Abonar exactamente el balance no es saldo a favor.
        let v = ver(&a, previa(20, "2750.00", Divisa::Dop, Some(10), None, TipoDeAbono::Actual));
        assert_eq!(v.balance_resultante_dop, dop(0.0));
        assert!(v.advertencias.is_empty(), "{:?}", v.advertencias);
    }

    #[test]
    fn bloquea_cuenta_archivada_tarjeta_inactiva_y_monto_no_valido() {
        let mut a = almacen();
        a.cuentas.get_mut(&10).unwrap().archivada = true;
        let v = ver(&a, previa(20, "100.00", Divisa::Dop, Some(10), None, TipoDeAbono::Personalizado));
        assert!(!v.se_puede_confirmar);
        assert_eq!(v.advertencias[0].codigo(), "cuenta_archivada", "los bloqueos van primero");
        assert!(v.cuenta.unwrap().archivada);

        let a = almacen().con_tarjeta_inactiva(20);
        let v = ver(&a, previa(20, "100.00", Divisa::Dop, Some(10), None, TipoDeAbono::Personalizado));
        assert!(!v.se_puede_confirmar && !v.tarjeta_activa);
        assert!(v.advertencias.iter().any(|a| a.codigo() == "tarjeta_inactiva"));

        for monto in ["", "   ", "abc", "0", "0.00", "-5.00"] {
            let v = ver(&almacen(), previa(20, monto, Divisa::Dop, Some(10), None, TipoDeAbono::Personalizado));
            assert!(!v.se_puede_confirmar && v.monto.is_none(), "monto {monto:?}");
            assert!(v.advertencias.iter().any(|a| a.codigo() == "monto_no_valido"), "monto {monto:?}");
            assert_eq!(v.balance_resultante_dop, v.balance_actual_dop, "sin monto válido nada cambia");
        }
    }

    #[test]
    fn una_cuenta_o_una_caja_inexistentes_se_advierten_sin_fallar() {
        let a = almacen();
        let v = ver(&a, previa(20, "10.00", Divisa::Dop, Some(999), None, TipoDeAbono::Personalizado));
        assert!(v.advertencias.iter().any(|a| a.codigo() == "cuenta_no_encontrada") && !v.se_puede_confirmar);
        let sin_cajas = AlmacenEnMemoria::nuevo().con_tarjeta(20, dop(100.0));
        let v = ver(&sin_cajas, previa(20, "10.00", Divisa::Dop, None, None, TipoDeAbono::Personalizado));
        assert!(v.advertencias.iter().any(|a| a.codigo() == "caja_no_encontrada" && a.texto().contains("Efectivo DOP")));
        assert!(!v.se_puede_confirmar);
    }

    #[test]
    fn el_tipo_que_no_coincide_con_el_monto_se_advierte() {
        let a = almacen();
        let v = ver(&a, previa(20, "2314.00", Divisa::Dop, Some(10), None, TipoDeAbono::Corte));
        assert!(!v.advertencias.iter().any(|a| a.codigo() == "monto_no_coincide_con_el_tipo"));
        let v = ver(&a, previa(20, "2313.99", Divisa::Dop, Some(10), None, TipoDeAbono::Corte));
        assert!(v.advertencias.iter().any(|a| a.codigo() == "monto_no_coincide_con_el_tipo"));
        let v = ver(&a, previa(20, "100.00", Divisa::Dop, Some(10), None, TipoDeAbono::Actual));
        assert!(v.advertencias.iter().any(|a| a.codigo() == "monto_no_coincide_con_el_tipo"));
        let v = ver(&a, previa(20, "100.00", Divisa::Dop, Some(10), None, TipoDeAbono::Personalizado));
        assert!(!v.advertencias.iter().any(|a| a.codigo() == "monto_no_coincide_con_el_tipo"));
    }

    #[test]
    fn una_tarjeta_inexistente_es_un_error_del_almacen() {
        let r = previsualizar_abono(previa(99, "1.00", Divisa::Dop, Some(10), None, TipoDeAbono::Personalizado), &almacen());
        assert!(matches!(r, Err(ErrorAplicacion::Almacen(ErrorAlmacen::NoEncontrado { .. }))));
    }

    #[test]
    fn la_vista_previa_y_el_registro_rechazan_lo_mismo() {
        // Todo lo que la vista previa declara no confirmable, el registro lo rechaza; y a la inversa.
        let mut base = almacen();
        base.cuentas.get_mut(&11).unwrap().archivada = true;
        let candidatos = [
            previa(20, "10.00", Divisa::Dop, Some(11), Some(58.0), TipoDeAbono::Personalizado),
            previa(20, "10.00", Divisa::Usd, Some(10), None, TipoDeAbono::Personalizado),
            previa(20, "10.00", Divisa::Usd, Some(10), Some(58.0), TipoDeAbono::Personalizado),
            previa(20, "0.00", Divisa::Dop, Some(10), None, TipoDeAbono::Personalizado),
            previa(20, "10.00", Divisa::Dop, Some(999), None, TipoDeAbono::Personalizado),
            previa(20, "10.00", Divisa::Dop, None, None, TipoDeAbono::Personalizado),
        ];
        for d in candidatos {
            let vista = ver(&base, d.clone());
            let mut real = base.clone();
            let monto = Dinero::desde_texto(&d.monto, d.divisa).unwrap();
            let r = registrar_pago_tarjeta(
                DatosPago {
                    tarjeta_id: d.tarjeta_id,
                    fecha: "01/01/2027".into(),
                    monto,
                    cuenta_ahorro_id: d.cuenta_ahorro_id,
                    tasa_cambio: d.tasa_cambio.map(|t| TasaCambio::nueva(t).unwrap()),
                },
                1,
                &mut real,
            );
            assert_eq!(vista.se_puede_confirmar, r.is_ok(), "{d:?}: previa {:?} / registro {r:?}", vista.advertencias);
        }
    }
}
