//! Casos de uso: transferir entre cuentas y revertir una transferencia.
//!
//! La transferencia es la operación con más efectos simultáneos del sistema:
//! debita, acredita y asienta. Aquí se orquestan sobre los puertos; la
//! atomicidad la garantiza quien abre la transacción, no este código.
//!
//! **Qué cambia respecto de los comandos que sustituye.** **H11** —origen y
//! destino iguales— se cierra del todo: el dominio no admite el valor que lo
//! producía, así que no se comprueba aquí, dejó de ser representable.
//!
//! **H12 se cierra solo a medias, y conviene ser preciso.** Ningún camino del
//! código puede ya construir una transferencia cuyo importe lleve una divisa
//! distinta a la de su cuenta. Pero eso es el error del *programador*. El del
//! *usuario* sigue abierto: el formulario pide dos números sueltos y la divisa
//! se deduce de la cuenta elegida, de modo que no hay ninguna declaración que
//! contradecir. Quien teclea pensando en pesos y elige una cuenta en dólares
//! acredita dólares. Esa mitad se mitiga en la interfaz, no aquí.
//!
//! **Qué se conserva.** El recorte en cero al revertir el destino (**H10**) y
//! la guarda de borrado que ignora las transferencias (**H13**) siguen
//! exactamente como estaban, a la espera de decisión. El descuadre de importes
//! dentro de una misma divisa (**H14**) también se admite; el caso de uso lo
//! deja pasar igual que el comando actual.

use super::ErrorAplicacion;
use crate::dominio::cuenta::{ExtremoCuenta, Transferencia};
use crate::dominio::dinero::Dinero;
use crate::puertos::repositorios::*;

pub struct DatosTransferencia {
    pub fecha: String,
    pub origen_id: i64,
    pub destino_id: i64,
    pub monto_origen: Dinero,
    pub monto_destino: Dinero,
    pub cargo: Dinero,
    pub descripcion: String,
}

/// Mueve fondos entre dos cuentas y deja el asiento.
pub fn transferir(
    datos: DatosTransferencia,
    almacen: &mut impl AlmacenTransferencias,
) -> Result<i64, ErrorAplicacion> {
    // Las divisas se leen del almacén, no se reciben: son un hecho de cada
    // cuenta, y dejar que quien llama las declare reabriría H12 por otra vía.
    let origen = ExtremoCuenta::nuevo(datos.origen_id, almacen.divisa(datos.origen_id)?);
    let destino = ExtremoCuenta::nuevo(datos.destino_id, almacen.divisa(datos.destino_id)?);

    let transferencia = Transferencia::nueva(
        origen,
        destino,
        datos.monto_origen,
        datos.monto_destino,
        datos.cargo,
    )?;

    almacen.ajustar_saldo(origen.id, transferencia.debito_al_origen()?)?;
    almacen.ajustar_saldo(destino.id, transferencia.credito_al_destino())?;

    almacen.insertar_transferencia(TransferenciaAPersistir {
        fecha: datos.fecha,
        origen_id: origen.id,
        destino_id: destino.id,
        monto_origen: transferencia.monto_origen(),
        monto_destino: transferencia.monto_destino(),
        tasa: transferencia.tasa()?.map(|t| t.valor()),
        cargo: transferencia.cargo(),
        descripcion: datos.descripcion,
    })
    .map_err(Into::into)
}

/// Deshace una transferencia y elimina su asiento.
///
/// **Es la inversa exacta de `transferir`** (resolución de **H10**): las dos
/// cuentas vuelven al estado que tenían, porque revertir significa *«esta
/// transferencia nunca ocurrió»*. No es devolver el dinero —eso sería otra
/// transferencia, un hecho nuevo—, sino retirar un registro mal hecho.
///
/// Antes el destino se descontaba con recorte en cero mientras el origen se
/// restituía entero, de modo que el total repartido entre ambas cuentas
/// **subía**: el patrimonio quedaba inflado.
///
/// Si el destino ya gastó lo recibido, el saldo queda negativo, y eso informa
/// en lugar de estorbar: dice que hay movimientos sin registrar en esa cuenta.
/// Ocultarlo tras un cero borraba justamente esa señal.
pub fn revertir_transferencia(
    id: i64,
    almacen: &mut impl AlmacenTransferencias,
) -> Result<(), ErrorAplicacion> {
    let t = almacen.obtener_transferencia(id)?;

    let devolucion = t.monto_origen.sumar(&t.cargo)?;
    almacen.ajustar_saldo(t.origen_id, devolucion)?;
    almacen.ajustar_saldo(t.destino_id, t.monto_destino.negado())?;

    almacen.eliminar_transferencia(id)?;
    Ok(())
}

/// Elimina una cuenta si nada la referencia.
///
/// Dos guardas, y en este orden. La caja de efectivo no se borra nunca: es el
/// papel del que depende todo gasto en efectivo de su divisa, y su clave
/// foránea está declarada `ON DELETE SET NULL`, de modo que borrarla
/// desvincularía los gastos en silencio en lugar de impedirlo. Se comprueba
/// primero porque explica mejor el rechazo: una caja con gastos daría el
/// mensaje genérico y dejaría creer que basta con borrar los gastos.
///
/// La tercera guarda cierra **H13**: una cuenta que participó en alguna
/// transferencia tampoco se borra. Su clave foránea es `ON DELETE CASCADE`, de
/// modo que borrarla se llevaba por delante esos asientos **y** dejaba a la
/// contraparte con el dinero recibido sin constancia de dónde salió.
pub fn eliminar_cuenta(
    cuenta_id: i64,
    almacen: &mut impl AlmacenTransferencias,
) -> Result<(), ErrorAplicacion> {
    if almacen.es_caja(cuenta_id)? {
        return Err(ErrorAplicacion::Almacen(ErrorAlmacen::Fallo(
            "No se puede eliminar la caja de efectivo: es la cuenta donde se asientan todos los gastos en efectivo de su divisa."
                .into(),
        )));
    }

    if almacen.gastos_que_referencian(cuenta_id)? > 0 {
        return Err(ErrorAplicacion::Almacen(ErrorAlmacen::Fallo(
            "No se puede eliminar la cuenta porque tiene transferencias registradas en gastos."
                .into(),
        )));
    }
    if almacen.transferencias_que_referencian(cuenta_id)? > 0 {
        return Err(ErrorAplicacion::Almacen(ErrorAlmacen::Fallo(
            "No se puede eliminar la cuenta porque participa en transferencias registradas: borrarla se llevaría ese historial."
                .into(),
        )));
    }

    almacen.eliminar_cuenta(cuenta_id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dominio::dinero::{Dinero, Divisa};
    use crate::puertos::dobles::AlmacenEnMemoria;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }

    fn usd(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Usd).unwrap()
    }

    fn almacen() -> AlmacenEnMemoria {
        AlmacenEnMemoria::nuevo()
            .con_cuenta(10, "Cuenta Ahorros DOP", dop(50_000.0))
            .con_cuenta(11, "Cuenta Corriente DOP", dop(10_000.0))
            .con_cuenta(12, "Cuenta Ahorros USD", usd(0.0))
    }

    fn datos(monto_origen: Dinero, monto_destino: Dinero, cargo: Dinero) -> DatosTransferencia {
        DatosTransferencia {
            fecha: "13/09/2026".into(),
            origen_id: 10,
            destino_id: 11,
            monto_origen,
            monto_destino,
            cargo,
            descripcion: "Traspaso".into(),
        }
    }

    #[test]
    fn transferir_mueve_los_dos_saldos_y_el_cargo_lo_paga_el_origen() {
        let mut a = almacen();

        let id = transferir(datos(dop(8_000.0), dop(8_000.0), dop(100.0)), &mut a).unwrap();

        assert_eq!(a.saldo_de(10), dop(41_900.0), "sale el monto y el cargo");
        assert_eq!(a.saldo_de(11), dop(18_000.0), "entra solo el monto");
        assert_eq!(a.obtener_transferencia(id).unwrap().cargo, dop(100.0));
    }

    #[test]
    fn h11_no_se_puede_transferir_una_cuenta_a_si_misma() {
        let mut a = almacen();
        let mut d = datos(dop(8_000.0), dop(8_000.0), dop(0.0));
        d.destino_id = 10;

        assert!(transferir(d, &mut a).is_err());
        assert_eq!(a.saldo_de(10), dop(50_000.0), "y ningún saldo se movió");
    }

    #[test]
    fn h12_no_se_puede_acreditar_pesos_a_una_cuenta_en_dolares() {
        let mut a = almacen();
        let mut d = datos(dop(6_000.0), dop(6_000.0), dop(0.0));
        d.destino_id = 12;

        assert!(transferir(d, &mut a).is_err(), "el dominio lo impide");
        assert_eq!(a.saldo_de(10), dop(50_000.0));
        assert_eq!(a.saldo_de(12), usd(0.0));
    }

    #[test]
    fn una_transferencia_entre_divisas_se_asienta_con_su_tasa() {
        let mut a = almacen();
        let mut d = datos(dop(6_000.0), usd(100.0), dop(0.0));
        d.destino_id = 12;

        let id = transferir(d, &mut a).unwrap();

        assert_eq!(a.saldo_de(10), dop(44_000.0));
        assert_eq!(a.saldo_de(12), usd(100.0));
        assert_eq!(a.obtener_transferencia(id).unwrap().monto_destino, usd(100.0));
    }

    #[test]
    fn revertir_devuelve_el_monto_y_el_cargo_al_origen() {
        let mut a = almacen();
        let id = transferir(datos(dop(8_000.0), dop(8_000.0), dop(100.0)), &mut a).unwrap();

        revertir_transferencia(id, &mut a).unwrap();

        assert_eq!(a.saldo_de(10), dop(50_000.0), "restitución exacta");
        assert_eq!(a.saldo_de(11), dop(10_000.0));
        assert!(a.obtener_transferencia(id).is_err(), "el asiento se fue");
    }

    #[test]
    fn h10_si_el_destino_gasto_lo_recibido_el_saldo_queda_negativo() {
        // **H10 resuelto.** Antes el destino se recortaba en cero mientras el
        // origen se restituía entero, y el total repartido entre las dos
        // cuentas subía: el patrimonio quedaba inflado.
        //
        // Ahora el negativo se muestra, y dice algo cierto: si la
        // transferencia nunca ocurrió, el destino nunca tuvo ese dinero, así
        // que gastarlo significa que faltan movimientos por registrar en esa
        // cuenta. Ocultarlo tras un cero borraba justamente esa señal.
        let mut a = almacen();
        let id = transferir(datos(dop(8_000.0), dop(8_000.0), dop(0.0)), &mut a).unwrap();
        a.ajustar_saldo(11, dop(-15_000.0)).unwrap(); // el destino queda en 3 000
        assert_eq!(a.saldo_de(11), dop(3_000.0));
        let total_antes = a.saldo_de(10).sumar(&a.saldo_de(11)).unwrap();

        revertir_transferencia(id, &mut a).unwrap();

        assert_eq!(a.saldo_de(10), dop(50_000.0), "el origen recupera todo");
        assert_eq!(a.saldo_de(11), dop(-5_000.0), "y el destino devuelve todo");

        let total_despues = a.saldo_de(10).sumar(&a.saldo_de(11)).unwrap();
        assert_eq!(
            total_despues, total_antes,
            "el total no cambia: revertir no crea ni destruye dinero"
        );
    }

    #[test]
    fn transferir_y_revertir_es_inverso_cuando_el_recorte_no_interviene() {
        let mut a = almacen();
        let antes = (a.saldo_de(10), a.saldo_de(11));

        let id = transferir(datos(dop(8_000.0), dop(8_000.0), dop(100.0)), &mut a).unwrap();
        revertir_transferencia(id, &mut a).unwrap();

        assert_eq!((a.saldo_de(10), a.saldo_de(11)), antes);
    }

    #[test]
    fn revertir_dos_veces_falla_la_segunda() {
        let mut a = almacen();
        let id = transferir(datos(dop(8_000.0), dop(8_000.0), dop(0.0)), &mut a).unwrap();
        revertir_transferencia(id, &mut a).unwrap();

        assert!(revertir_transferencia(id, &mut a).is_err());
        assert_eq!(a.saldo_de(10), dop(50_000.0), "sin doble restitución");
    }

    #[test]
    fn revertir_una_transferencia_inexistente_es_error() {
        let mut a = almacen();
        assert!(revertir_transferencia(404, &mut a).is_err());
    }

    #[test]
    fn h14_un_descuadre_dentro_de_la_misma_divisa_se_rechaza() {
        // **H14 resuelto.** Antes salían 8 000 y entraban 7 500, y los 500 de
        // diferencia desaparecían entre las dos cuentas sin que nada lo
        // dijera. La diferencia solo puede ser una comisión —y para eso está
        // el cargo, que sale aparte— o un error de tecleo.
        let mut a = almacen();

        let error = transferir(datos(dop(8_000.0), dop(7_500.0), dop(0.0)), &mut a)
            .unwrap_err()
            .to_string();

        assert!(error.contains("comisión"), "orienta hacia el cargo: {error}");
        assert_eq!(a.saldo_de(10), dop(50_000.0), "ningún saldo se movió");
        assert_eq!(a.saldo_de(11), dop(10_000.0));
    }

    #[test]
    fn la_caja_de_efectivo_no_se_borra_nunca() {
        // Su clave foránea es ON DELETE SET NULL: borrarla desvincularía los
        // gastos en efectivo en silencio en lugar de impedir el borrado.
        let mut a = almacen().con_caja(20, dop(500.0));

        let error = eliminar_cuenta(20, &mut a).unwrap_err().to_string();

        assert!(error.contains("caja de efectivo"), "explica por qué: {error}");
    }

    #[test]
    fn h13_una_cuenta_con_transferencias_no_se_borra() {
        // **H13 resuelto.** Antes se borraba: la clave foránea es
        // ON DELETE CASCADE, así que el historial se iba en silencio y la
        // contraparte conservaba el dinero recibido sin constancia de dónde
        // había salido.
        let mut a = almacen();
        transferir(datos(dop(8_000.0), dop(8_000.0), dop(0.0)), &mut a).unwrap();

        let error = eliminar_cuenta(10, &mut a).unwrap_err().to_string();

        assert!(error.contains("transferencias"), "explica por qué: {error}");
        assert!(error.contains("historial"), "y qué se perdería: {error}");
    }

    #[test]
    fn una_cuenta_sin_nada_que_la_referencie_si_se_borra() {
        // La guarda no puede volverse un candado: una cuenta limpia se borra.
        let mut a = almacen();

        assert!(eliminar_cuenta(11, &mut a).is_ok());
    }
}
