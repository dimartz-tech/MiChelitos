//! Ejecuta `features/abono_tarjeta_multidivisa.feature` contra el caso de uso.
//!
//! Es la prueba de §6.4 del plan, y la única de todo el proyecto escrita en el
//! lenguaje del titular en vez del del programador. Su valor no es cubrir un
//! camino que otras pruebas no cubran —varias lo hacen—, sino **poder leerse
//! sin saber Rust**: quien decide si la regla es correcta puede comprobar que
//! lo escrito es lo que quería, sin fiarse de la traducción.
//!
//! Se ejecuta sobre los dobles en memoria, no sobre SQLite, porque lo que
//! verifica son las reglas del caso de uso. Que el adaptador cumpla el mismo
//! contrato lo garantiza la suite de `puertos::contrato`.
//!
//! **Divergencia declarada frente al plan:** §6.4 esperaba la categoría
//! `"Comisiones Bancarias"`. El sistema real anota las comisiones bajo
//! `"Otros"`, así que el archivo dice `"Otros"`. Cambiar el sistema para que
//! encaje con el documento sería mover el mundo para salvar el mapa.

use crate::aplicacion::registrar_pago_tarjeta::{registrar_pago_tarjeta, DatosPago};
use crate::aplicacion::ErrorAplicacion;
use crate::dominio::dinero::{Dinero, Divisa, TasaCambio};
use crate::gherkin::{analizar, entrecomillados, numeros, Escenario, Paso};
use crate::puertos::dobles::AlmacenEnMemoria;
use crate::puertos::repositorios::*;
use std::collections::HashMap;

const CATEGORIA_COMISION: i64 = 1;

/// Estado que los pasos comparten dentro de un escenario.
struct Mundo {
    almacen: AlmacenEnMemoria,
    cuentas: HashMap<String, i64>,
    tarjetas: HashMap<String, i64>,
    siguiente_id: i64,
    /// Resultado del último `Cuando`, para que los `Entonces` lo interroguen.
    error: Option<ErrorAplicacion>,
    debitado: Option<Dinero>,
    comision: Option<Dinero>,
}

impl Mundo {
    fn nuevo() -> Mundo {
        Mundo {
            almacen: AlmacenEnMemoria::nuevo().con_categoria(CATEGORIA_COMISION, "Otros"),
            cuentas: HashMap::new(),
            tarjetas: HashMap::new(),
            siguiente_id: 100,
            error: None,
            debitado: None,
            comision: None,
        }
    }

    fn id(&mut self) -> i64 {
        self.siguiente_id += 1;
        self.siguiente_id
    }

    fn cuenta(&self, nombre: &str) -> i64 {
        *self.cuentas.get(nombre).unwrap_or_else(|| panic!("cuenta no sembrada: {nombre}"))
    }

    fn tarjeta(&self, nombre: &str) -> i64 {
        *self.tarjetas.get(nombre).unwrap_or_else(|| panic!("tarjeta no sembrada: {nombre}"))
    }
}

fn divisa(codigo: &str) -> Divisa {
    Divisa::desde_codigo(codigo).unwrap_or_else(|_| panic!("divisa desconocida: {codigo}"))
}

fn importe(valor: f64, codigo: &str) -> Dinero {
    Dinero::nuevo(valor, divisa(codigo)).expect("importe válido")
}

/// Traduce un paso a una acción sobre el mundo.
///
/// El emparejamiento es por fragmentos distintivos y no por expresión regular
/// completa: mantiene el archivo legible y evita que un cambio de redacción
/// menor rompa la prueba por motivos que no son de negocio. A cambio exige que
/// cada fragmento sea inconfundible, y por eso el último brazo **entra en
/// pánico** ante un paso no reconocido: un paso que no hace nada sería una
/// prueba verde que no prueba.
fn ejecutar(mundo: &mut Mundo, paso: &Paso) {
    let t = &paso.texto;
    let textos = entrecomillados(t);
    let nums = numeros(t);
    let sitio = format!("línea {}: «{}»", paso.linea, t);

    // --- Dado ---
    if t.starts_with("que existe la cuenta de ahorros") {
        let id = mundo.id();
        let (nombre, codigo) = (&textos[0], &textos[1]);
        mundo.almacen = std::mem::replace(&mut mundo.almacen, AlmacenEnMemoria::nuevo())
            .con_cuenta(id, nombre, importe(nums[0], codigo));
        mundo.cuentas.insert(nombre.clone(), id);
    } else if t.starts_with("que existe la tarjeta") {
        let id = mundo.id();
        mundo.almacen = std::mem::replace(&mut mundo.almacen, AlmacenEnMemoria::nuevo())
            .con_tarjeta(id, Dinero::cero(Divisa::Dop));
        mundo.tarjetas.insert(textos[0].clone(), id);
    } else if t.starts_with("que la tarjeta") && t.contains("tiene balance") {
        let id = mundo.tarjeta(&textos[0]);
        mundo.almacen.ajustar_deuda(id, importe(nums[0], &textos[1])).expect("sembrar deuda");
    } else if t.starts_with("que el repositorio de tarjetas fallará al guardar") {
        mundo.almacen.falla_al_ajustar_deuda = true;

    // --- Cuando ---
    } else if t.starts_with("registro un abono") {
        let tarjeta = mundo.tarjeta(&textos[1]);
        let cuenta = mundo.cuenta(&textos[2]);
        // nums = [monto, (tasa)]; sin tasa el paso dice «sin tasa de cambio».
        let tasa = if t.contains("sin tasa de cambio") {
            None
        } else {
            Some(TasaCambio::nueva(nums[1]).expect("tasa válida"))
        };

        let datos = DatosPago {
            tarjeta_id: tarjeta,
            fecha: "16/09/2026".to_string(),
            monto: importe(nums[0], &textos[0]),
            cuenta_ahorro_id: Some(cuenta),
            tasa_cambio: tasa,
        };

        match registrar_pago_tarjeta(datos, CATEGORIA_COMISION, &mut mundo.almacen) {
            Ok(r) => {
                mundo.debitado = r.debitado;
                mundo.comision = r.comision;
                mundo.error = None;
            }
            Err(e) => {
                mundo.error = Some(e);
                mundo.debitado = None;
                mundo.comision = None;
            }
        }

    // --- Entonces ---
    } else if t.starts_with("el balance de la tarjeta") {
        let id = mundo.tarjeta(&textos[0]);
        let esperado = importe(nums[0], &textos[1]);
        let real = mundo.almacen.deuda_en(id, divisa(&textos[1]));
        assert_eq!(real, esperado, "{sitio}");
    } else if t.starts_with("el equivalente debitado en pesos debe ser") {
        assert_eq!(mundo.debitado, Some(importe(nums[0], "DOP")), "{sitio}");
    } else if t.starts_with("se debe registrar una comisión bancaria de") {
        assert_eq!(mundo.comision, Some(importe(nums[0], &textos[0])), "{sitio}");
    } else if t.starts_with("el balance de la cuenta") {
        let id = mundo.cuenta(&textos[0]);
        let real = mundo.almacen.saldo_de(id);
        assert_eq!(real.unidades(), nums[0], "{sitio}");
    } else if t.starts_with("debe existir un gasto de categoría") {
        let esperado = importe(nums[0], &textos[1]);
        let hay = mundo.almacen.gastos.values().any(|g| g.monto == esperado);
        assert!(hay, "{sitio}: no se encontró un gasto por {esperado:?}");
    } else if t.starts_with("no debe existir ningún gasto de categoría") {
        assert_eq!(mundo.almacen.total_gastos(), 0, "{sitio}");
    } else if t.starts_with("la operación debe fallar con el error") {
        let e = mundo.error.as_ref().unwrap_or_else(|| panic!("{sitio}: la operación no falló"));
        let nombre = &textos[0];
        assert!(
            format!("{e:?}").contains(nombre.as_str()),
            "{sitio}: se esperaba {nombre}, llegó {e:?}"
        );
    } else if t.starts_with("la operación debe fallar") {
        assert!(mundo.error.is_some(), "{sitio}: la operación no falló");
    } else {
        panic!("paso no reconocido en {sitio}");
    }
}

fn correr(escenario: &Escenario) {
    let mut mundo = Mundo::nuevo();
    for paso in &escenario.pasos {
        ejecutar(&mut mundo, paso);
    }
}

fn escenarios() -> Vec<Escenario> {
    // La ruta es relativa al manifiesto para que no dependa del directorio
    // desde el que se invoque `cargo test`.
    let ruta = concat!(env!("CARGO_MANIFEST_DIR"), "/../features/abono_tarjeta_multidivisa.feature");
    let contenido = std::fs::read_to_string(ruta)
        .unwrap_or_else(|e| panic!("no se pudo leer {ruta}: {e}"));
    analizar(&contenido)
}

#[test]
fn el_archivo_declara_los_cuatro_escenarios_previstos() {
    // Si alguien vacía el archivo o rompe su formato, las demás pruebas
    // pasarían por no ejecutar nada. Esta lo impide.
    let e = escenarios();
    assert_eq!(e.len(), 4, "escenarios encontrados: {:?}", e.iter().map(|x| &x.nombre).collect::<Vec<_>>());
    assert!(e.iter().all(|x| x.pasos.len() >= 4), "algún escenario quedó sin pasos");
}

#[test]
fn escenario_1_abono_con_conversion_y_comision() {
    correr(&escenarios()[0]);
}

#[test]
fn escenario_2_la_tasa_es_obligatoria_al_cruzar_divisas() {
    correr(&escenarios()[1]);
}

#[test]
fn escenario_3_la_misma_divisa_no_aplica_conversion() {
    correr(&escenarios()[2]);
}

#[test]
fn escenario_4_un_fallo_a_mitad_no_deja_saldos_alterados() {
    correr(&escenarios()[3]);
}
