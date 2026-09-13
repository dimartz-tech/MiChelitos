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
/// **No es la inversa exacta de `transferir`** y eso es deliberado en esta
/// fase: al origen se le restituye todo, pero al destino se le descuenta con
/// recorte en cero (**H10**). Si el destino ya gastó parte de lo recibido, la
/// diferencia no se descuenta y el patrimonio queda inflado. Se conserva la
/// conducta vigente; corregirla es una decisión aparte.
pub fn revertir_transferencia(
    id: i64,
    almacen: &mut impl AlmacenTransferencias,
) -> Result<(), ErrorAplicacion> {
    let t = almacen.obtener_transferencia(id)?;

    let devolucion = t.monto_origen.sumar(&t.cargo)?;
    almacen.ajustar_saldo(t.origen_id, devolucion)?;
    almacen.reducir_saldo_con_recorte(t.destino_id, t.monto_destino)?;

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
/// **Conserva H13**: la segunda guarda cuenta los gastos que apuntan a la
/// cuenta, pero no las transferencias. Como la clave foránea de éstas sí es
/// `ON DELETE CASCADE`, borrar una cuenta con transferencias se lleva su
/// historial en silencio. El puerto ya expone
/// `transferencias_que_referencian` para cuando se decida cerrarlo.
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
    fn h10_si_el_destino_gasto_lo_recibido_la_reversion_infla_el_patrimonio() {
        // Conducta vigente. El origen recupera todo y al destino solo se le
        // quita lo que tiene: la diferencia no desaparece, se inventa.
        let mut a = almacen();
        let id = transferir(datos(dop(8_000.0), dop(8_000.0), dop(0.0)), &mut a).unwrap();
        a.ajustar_saldo(11, dop(-15_000.0)).unwrap(); // el destino queda en 3 000
        assert_eq!(a.saldo_de(11), dop(3_000.0));
        let total_antes = a.saldo_de(10).sumar(&a.saldo_de(11)).unwrap();

        revertir_transferencia(id, &mut a).unwrap();

        assert_eq!(a.saldo_de(10), dop(50_000.0), "el origen recupera todo");
        assert_eq!(a.saldo_de(11), dop(0.0), "recorte en cero (H10)");

        let total_despues = a.saldo_de(10).sumar(&a.saldo_de(11)).unwrap();
        assert_eq!(
            total_despues.restar(&total_antes).unwrap(),
            dop(5_000.0),
            "cinco mil aparecidos de la nada"
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
    fn h14_un_descuadre_dentro_de_la_misma_divisa_se_admite() {
        // Conducta vigente: salen 8 000 y entran 7 500. El caso de uso no lo
        // impide; el tipo lo expone para cuando se decida.
        let mut a = almacen();

        transferir(datos(dop(8_000.0), dop(7_500.0), dop(0.0)), &mut a).unwrap();

        assert_eq!(a.saldo_de(10), dop(42_000.0));
        assert_eq!(a.saldo_de(11), dop(17_500.0), "500 desaparecidos");
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
    fn h13_borrar_una_cuenta_no_mira_sus_transferencias() {
        // Conducta vigente: la guarda solo cuenta gastos.
        let mut a = almacen();
        transferir(datos(dop(8_000.0), dop(8_000.0), dop(0.0)), &mut a).unwrap();
        assert_eq!(a.transferencias_que_referencian(10).unwrap(), 1);

        assert!(eliminar_cuenta(10, &mut a).is_ok(), "se borra igual (H13)");
    }
}
