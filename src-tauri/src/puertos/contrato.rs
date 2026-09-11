//! Suite de contrato que toda implementación de los puertos debe satisfacer.
//!
//! La ejecutan tanto el doble en memoria como el adaptador SQLite. Si ambos la
//! pasan, son intercambiables allí donde se espere un `AlmacenGastos`, que es
//! la verificación del principio de sustitución de Liskov que el plan pedía.
//!
//! Sin esto, el riesgo real es que el doble se comporte mejor que la base de
//! datos y las pruebas de los casos de uso den una falsa sensación de
//! seguridad.

#![cfg(test)]

use super::repositorios::*;
use crate::dominio::conversion::EstadoConversion;
use crate::dominio::dinero::{Dinero, Divisa};

/// Identificadores que la implementación debe haber sembrado antes.
pub struct Semilla {
    pub categoria_id: i64,
    pub categoria_nombre: String,
    pub cuenta_id: i64,
    pub cuenta_saldo: Dinero,
    pub caja_id: i64,
    pub caja_saldo: Dinero,
    pub tarjeta_id: i64,
    pub tarjeta_deuda: Dinero,
}

fn dop(u: f64) -> Dinero {
    Dinero::nuevo(u, Divisa::Dop).unwrap()
}

fn gasto_de(monto: Dinero, metodo: &str, semilla: &Semilla) -> GastoAPersistir {
    GastoAPersistir {
        fecha: "09/09/2026".into(),
        monto,
        descripcion: "Compra de contrato".into(),
        categoria_id: semilla.categoria_id,
        metodo_pago: metodo.into(),
        cargos: dop(2.5),
        tarjeta_id: None,
        cuenta_ahorro_id: Some(semilla.cuenta_id),
        estado_conversion: EstadoConversion::NoAplica,
    }
}

/// Ejecuta el contrato completo. Cada aserción lleva la etiqueta de la
/// implementación para que un fallo diga cuál de las dos se desvió.
pub fn verificar<A: AlmacenGastos>(a: &mut A, s: &Semilla, quien: &str) {
    categorias(a, s, quien);
    ida_y_vuelta_del_gasto(a, s, quien);
    eliminacion(a, s, quien);
    saldos_de_cuenta(a, s, quien);
    caja_por_papel(a, s, quien);
    deuda_de_tarjeta(a, s, quien);
    saldo_a_favor(a, s, quien);
    divisas_incompatibles(a, s, quien);
}

fn categorias<A: AlmacenGastos>(a: &mut A, s: &Semilla, quien: &str) {
    assert_eq!(
        a.nombre(s.categoria_id).unwrap(),
        Some(s.categoria_nombre.clone()),
        "[{}] la categoría sembrada debe devolverse",
        quien
    );
    assert_eq!(
        a.nombre(999_999).unwrap(),
        None,
        "[{}] una categoría inexistente devuelve None, no error",
        quien
    );
}

fn ida_y_vuelta_del_gasto<A: AlmacenGastos>(a: &mut A, s: &Semilla, quien: &str) {
    let g = gasto_de(dop(1234.56), "transferencia", s);
    let id = a.insertar(&g).unwrap();
    let leido = a.obtener(id).unwrap();

    assert_eq!(leido.monto, dop(1234.56), "[{}] el monto vuelve igual", quien);
    assert_eq!(leido.cargos, dop(2.5), "[{}] los cargos vuelven igual", quien);
    assert_eq!(leido.metodo_pago, "transferencia", "[{}] el método vuelve igual", quien);
    assert_eq!(
        leido.cuenta_ahorro_id,
        Some(s.cuenta_id),
        "[{}] la cuenta vuelve igual",
        quien
    );

    assert!(
        matches!(
            a.obtener(999_999).unwrap_err(),
            ErrorAlmacen::NoEncontrado { entidad: "gasto", .. }
        ),
        "[{}] un gasto inexistente es NoEncontrado",
        quien
    );
}

fn eliminacion<A: AlmacenGastos>(a: &mut A, s: &Semilla, quien: &str) {
    let id = a.insertar(&gasto_de(dop(10.0), "efectivo", s)).unwrap();
    a.eliminar(id).unwrap();
    assert!(a.eliminar(id).is_err(), "[{}] eliminar dos veces es error", quien);
    assert!(a.obtener(id).is_err(), "[{}] tras eliminar no se recupera", quien);
}

fn saldos_de_cuenta<A: AlmacenGastos>(a: &mut A, s: &Semilla, quien: &str) {
    let inicial = a.saldo(s.cuenta_id).unwrap();
    assert_eq!(inicial, s.cuenta_saldo, "[{}] saldo inicial sembrado", quien);

    a.ajustar_saldo(s.cuenta_id, dop(-10020.0)).unwrap();
    assert_eq!(
        a.saldo(s.cuenta_id).unwrap(),
        inicial.restar(&dop(10020.0)).unwrap(),
        "[{}] el débito resta el importe exacto",
        quien
    );

    a.ajustar_saldo(s.cuenta_id, dop(10020.0)).unwrap();
    assert_eq!(
        a.saldo(s.cuenta_id).unwrap(),
        inicial,
        "[{}] devolver el mismo importe restituye el saldo",
        quien
    );

    assert!(
        a.ajustar_saldo(999_999, dop(-1.0)).is_err(),
        "[{}] ajustar una cuenta inexistente es error",
        quien
    );
}

fn caja_por_papel<A: AlmacenGastos>(a: &mut A, s: &Semilla, quien: &str) {
    // Resolución de H3: la caja se localiza por el papel que cumple, no por su
    // nombre, y ambas implementaciones deben devolver la misma fila.
    assert_eq!(
        a.caja(Divisa::Dop).unwrap(),
        s.caja_id,
        "[{quien}] la caja en pesos es la cuenta marcada como tal"
    );

    // Que la ausencia sea un error se comprueba en cada implementación por
    // separado: el esquema real siembra las dos cajas, así que la suite
    // compartida no puede montar el caso sin desmontar la semilla.

    let antes = a.saldo(s.caja_id).unwrap();
    let caja = a.caja(Divisa::Dop).unwrap();
    a.ajustar_saldo(caja, dop(-1200.0)).unwrap();
    assert_eq!(
        a.saldo(s.caja_id).unwrap(),
        antes.restar(&dop(1200.0)).unwrap(),
        "[{quien}] el ajuste llega a la caja"
    );
    a.ajustar_saldo(caja, dop(1200.0)).unwrap();
}

fn deuda_de_tarjeta<A: AlmacenGastos>(a: &mut A, s: &Semilla, quien: &str) {
    a.ajustar_deuda(s.tarjeta_id, dop(150.0)).unwrap();
    a.ajustar_deuda(s.tarjeta_id, dop(-150.0)).unwrap();

    assert!(
        a.ajustar_deuda(999_999, dop(1.0)).is_err(),
        "[{}] ajustar una tarjeta inexistente es error",
        quien
    );
}

fn saldo_a_favor<A: AlmacenGastos>(a: &mut A, s: &Semilla, quien: &str) {
    // Resolución de H5: reducir por encima de la deuda la deja en negativo,
    // no en cero. Ambas implementaciones deben coincidir en esto, porque es
    // donde antes divergían el `MAX(0.0, ...)` de SQL y la aritmética exacta.
    let exceso = s.tarjeta_deuda.sumar(&dop(1000.0)).unwrap();
    a.ajustar_deuda(s.tarjeta_id, exceso.negado()).unwrap();
    assert_eq!(
        a.deuda(s.tarjeta_id, exceso.divisa()).unwrap(),
        dop(-1000.0),
        "[{quien}] el exceso sobre la deuda queda como saldo a favor"
    );
    // Se restituye la deuda sembrada para no arrastrar estado entre bloques.
    a.ajustar_deuda(s.tarjeta_id, exceso).unwrap();
}

fn divisas_incompatibles<A: AlmacenGastos>(a: &mut A, s: &Semilla, quien: &str) {
    let usd = Dinero::nuevo(50.0, Divisa::Usd).unwrap();
    assert!(
        a.ajustar_saldo(s.cuenta_id, usd).is_err(),
        "[{}] no se puede restar dólares de un saldo en pesos",
        quien
    );
}
