//! Casos de uso: transferir entre cuentas y anular un traspaso con un asiento de compensación.
//!
//! **MODELO C (prototipo, no fusionar):** anular en vez de borrar. Ver `anular_transferencia`.
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
//! **Qué se conserva.** La inversa exacta al anular (**H10**) y
//! la guarda de borrado que ignora las transferencias (**H13**) siguen
//! exactamente como estaban, a la espera de decisión. El descuadre de importes
//! dentro de una misma divisa (**H14**) también se admite; el caso de uso lo
//! deja pasar igual que el comando actual.

use super::ErrorAplicacion;
use crate::dominio::cuenta::{ExtremoCuenta, Transferencia};
use crate::dominio::dinero::{Dinero, TasaCambio};
use crate::dominio::errores::ErrorDominio;
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
    super::cuentas::exigir_activa(almacen, datos.origen_id)?;
    super::cuentas::exigir_activa(almacen, datos.destino_id)?;
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
        compensa_a: None,
    })
    .map_err(Into::into)
}

/// **Anula** un traspaso con un asiento de compensación (modelo «C»: anular en vez de borrar).
///
/// El original **sobrevive** marcado `anulada`; nace un asiento contrario con estado `compensacion` y `compensa_a`
/// apuntando al original. Los saldos se ajustan con la **inversa exacta** de `transferir` (resolución de **H10**,
/// intacta): tras anular, las dos cuentas quedan como antes del traspaso, y si el destino ya gastó lo recibido queda en
/// negativo (informa de movimientos sin registrar; ocultarlo tras un cero borraría esa señal).
///
/// El asiento contrario invierte los extremos: su origen es el destino del original (sale `monto_destino`) y su
/// destino es el origen del original (entra `monto_origen`). **El cargo** conserva su importe y, por la naturaleza
/// del asiento, se **reembolsa** a la cuenta destino de la compensación (la que pagó el cargo): entra además de
/// `monto_origen`. Como el cargo vive en la divisa de esa cuenta, no pasa por `Transferencia::nueva`, que lo cobraría
/// al origen de la compensación.
///
/// Orden: **primero** que el traspaso exista —un id inexistente se nombra antes que un motivo flojo—, después el motivo,
/// que esté `vigente` (ni anulado ya, ni una compensación) y que las dos cuentas estén activas (como antes: una cuenta
/// archivada se reactiva primero). Luego se inserta la compensación, se anota el caso (divisa **de origen**; la
/// descripción cita «original #a, compensación #b»), se mueven los saldos y se marca el original. Todo en la
/// transacción de quien llama: si algo falla no queda nada.
///
/// `hoy` (dd/mm/aaaa) es la fecha de la compensación; llega de fuera para que el caso de uso no lea el reloj.
pub fn anular_transferencia(
    id: i64,
    motivo: &str,
    hoy: &str,
    almacen: &mut (impl AlmacenTransferencias + RegistroDeCorrecciones),
) -> Result<String, ErrorAplicacion> {
    let t = almacen.obtener_transferencia(id)?;
    let motivo = crate::dominio::correccion::motivo_de_correccion(motivo)?;
    match t.estado {
        EstadoTraspaso::Vigente => {}
        EstadoTraspaso::Anulada => return Err(ErrorDominio::TraspasoYaAnulado { id }.into()),
        EstadoTraspaso::Compensacion => return Err(ErrorDominio::CompensacionNoSeAnula { id }.into()),
    }
    super::cuentas::exigir_activa(almacen, t.origen_id)?;
    super::cuentas::exigir_activa(almacen, t.destino_id)?;

    let descripcion = if t.descripcion.is_empty() { "Traspaso entre cuentas".to_string() } else { t.descripcion.clone() };

    // La tasa del contrario es la inversa de la del original; sin cambio de divisa no significa nada (`None`).
    let tasa = if t.monto_origen.divisa() != t.monto_destino.divisa() {
        Some(TasaCambio::nueva(t.monto_origen.unidades() / t.monto_destino.unidades())?.valor())
    } else {
        None
    };
    let compensacion_id = almacen.insertar_transferencia(TransferenciaAPersistir {
        fecha: hoy.to_string(),
        origen_id: t.destino_id,
        destino_id: t.origen_id,
        monto_origen: t.monto_destino,
        monto_destino: t.monto_origen,
        tasa,
        cargo: t.cargo,
        descripcion: format!("Compensación de #{id}: {descripcion}"),
        compensa_a: Some(id),
    })?;

    let caso = almacen.anotar_caso(&CasoAAnotar {
        tipo: "traspaso".to_string(),
        referencia_id: id,
        descripcion: format!("{descripcion} (original #{id}, compensación #{compensacion_id})"),
        importe: Some(t.monto_origen.unidades()),
        divisa: Some(t.monto_origen.divisa().codigo().to_string()),
        motivo,
    })?;

    // La inversa exacta de `transferir`: el origen recupera el monto y el cargo; el destino devuelve lo recibido.
    almacen.ajustar_saldo(t.origen_id, t.monto_origen.sumar(&t.cargo)?)?;
    almacen.ajustar_saldo(t.destino_id, t.monto_destino.negado())?;

    if !almacen.marcar_anulada(id)? {
        return Err(ErrorDominio::TraspasoYaAnulado { id }.into());
    }
    Ok(caso)
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

    const MOTIVO: &str = "Corrección de prueba del sistema";

    fn con_traspaso(a: &mut AlmacenEnMemoria, descripcion: &str) -> i64 {
        let mut d = datos(dop(1_000.0), dop(1_000.0), dop(0.0));
        d.descripcion = descripcion.into();
        transferir(d, a).unwrap()
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

    // --- Anulación con asiento de compensación (modelo C) ---

    const HOY: &str = "03/11/2026";

    fn anular(id: i64, a: &mut AlmacenEnMemoria) -> Result<String, ErrorAplicacion> {
        anular_transferencia(id, MOTIVO, HOY, a)
    }

    fn compensacion_de(a: &AlmacenEnMemoria, id: i64) -> TransferenciaGuardada {
        a.transferencias.values().find(|t| t.compensa_a == Some(id)).cloned().expect("hay compensación")
    }

    #[test]
    fn anular_un_traspaso_anota_el_caso_deja_el_original_y_crea_la_compensacion() {
        let mut a = almacen();
        let id = con_traspaso(&mut a, "Pago del alquiler");

        let caso = anular(id, &mut a).unwrap();

        assert_eq!(caso, "CASO-0001");
        assert_eq!(a.casos.len(), 1);
        let c = &a.casos[0];
        let comp = compensacion_de(&a, id);
        assert_eq!((c.tipo.as_str(), c.referencia_id), ("traspaso", id));
        assert_eq!(c.descripcion, format!("Pago del alquiler (original #{id}, compensación #{})", comp.id));
        assert_eq!(c.importe, Some(1_000.0));
        assert_eq!(c.divisa.as_deref(), Some("DOP"));
        assert_eq!(c.motivo, MOTIVO);
        assert_eq!(a.saldo(10).unwrap(), dop(50_000.0));
        assert_eq!(a.saldo(11).unwrap(), dop(10_000.0));
        let original = a.obtener_transferencia(id).unwrap();
        assert_eq!((original.estado, original.compensa_a), (EstadoTraspaso::Anulada, None), "el original sobrevive");
        assert_eq!(comp.estado, EstadoTraspaso::Compensacion);
        assert_eq!((comp.origen_id, comp.destino_id), (11, 10), "extremos invertidos");
        assert_eq!((comp.monto_origen, comp.monto_destino), (dop(1_000.0), dop(1_000.0)));
        assert_eq!(comp.fecha, HOY);
        assert_eq!(comp.descripcion, format!("Compensación de #{id}: Pago del alquiler"));
    }

    #[test]
    fn anular_usd_a_usd_anota_el_caso_en_dolares() {
        // Invierte el antiguo hallazgo: el caso decía siempre «DOP».
        let mut a = AlmacenEnMemoria::nuevo().con_cuenta(12, "A USD", usd(500.0)).con_cuenta(13, "B USD", usd(0.0));
        let id = transferir(
            DatosTransferencia {
                fecha: "13/10/2026".into(),
                origen_id: 12,
                destino_id: 13,
                monto_origen: usd(200.0),
                monto_destino: usd(200.0),
                cargo: usd(0.0),
                descripcion: "usd".into(),
            },
            &mut a,
        )
        .unwrap();

        anular(id, &mut a).unwrap();

        assert_eq!(a.casos[0].divisa.as_deref(), Some("USD"));
        assert_eq!(a.casos[0].importe, Some(200.0));
        assert_eq!((a.saldo_de(12), a.saldo_de(13)), (usd(500.0), usd(0.0)));
    }

    #[test]
    fn anular_dop_a_dop_con_cargo_devuelve_monto_y_cargo_al_origen() {
        let mut a = almacen();
        let id = transferir(datos(dop(8_000.0), dop(8_000.0), dop(100.0)), &mut a).unwrap();
        assert_eq!(a.saldo_de(10), dop(41_900.0));

        anular(id, &mut a).unwrap();

        assert_eq!(a.saldo_de(10), dop(50_000.0), "restitución exacta con cargo");
        assert_eq!(a.saldo_de(11), dop(10_000.0));
        assert_eq!(compensacion_de(&a, id).cargo, dop(100.0), "el cargo reembolsado queda a la vista");
    }

    #[test]
    fn anular_un_cruce_dop_a_usd_usa_la_divisa_de_origen_y_los_importes_invertidos() {
        let mut a = almacen();
        let mut d = datos(dop(6_000.0), usd(100.0), dop(50.0));
        d.destino_id = 12;
        let id = transferir(d, &mut a).unwrap();
        assert_eq!((a.saldo_de(10), a.saldo_de(12)), (dop(43_950.0), usd(100.0)));

        anular(id, &mut a).unwrap();

        assert_eq!((a.saldo_de(10), a.saldo_de(12)), (dop(50_000.0), usd(0.0)));
        assert_eq!(a.casos[0].divisa.as_deref(), Some("DOP"), "la divisa del ORIGEN, no la del destino");
        assert_eq!(a.casos[0].importe, Some(6_000.0));
        let comp = compensacion_de(&a, id);
        assert_eq!((comp.origen_id, comp.destino_id), (12, 10));
        assert_eq!((comp.monto_origen, comp.monto_destino, comp.cargo), (usd(100.0), dop(6_000.0), dop(50.0)));
    }

    #[test]
    fn anular_una_vez_no_deja_anular_otra() {
        let mut a = almacen();
        let id = con_traspaso(&mut a, "x");
        anular(id, &mut a).unwrap();
        let antes = (a.saldo_de(10), a.saldo_de(11), a.transferencias.len(), a.casos.len());

        let e = anular(id, &mut a).unwrap_err();

        assert_eq!(e.to_string(), format!("El traspaso {id} ya está anulado: su asiento de compensación ya existe."));
        assert_eq!((a.saldo_de(10), a.saldo_de(11), a.transferencias.len(), a.casos.len()), antes, "sin efectos");
    }

    #[test]
    fn una_compensacion_no_se_anula() {
        let mut a = almacen();
        let id = con_traspaso(&mut a, "x");
        anular(id, &mut a).unwrap();
        let comp = compensacion_de(&a, id).id;
        let antes = (a.saldo_de(10), a.saldo_de(11), a.transferencias.len(), a.casos.len());

        let e = anular(comp, &mut a).unwrap_err();

        assert!(e.to_string().contains("es la compensación de un traspaso anulado"), "{e}");
        assert_eq!((a.saldo_de(10), a.saldo_de(11), a.transferencias.len(), a.casos.len()), antes);
    }

    #[test]
    fn un_traspaso_inexistente_se_nombra_antes_que_un_motivo_invalido() {
        let mut a = almacen();

        let e = anular_transferencia(99, "corto", HOY, &mut a).unwrap_err();

        assert_eq!(e.to_string(), "No se encontró traspaso con identificador 99.");
        assert!(a.casos.is_empty());
    }

    #[test]
    fn un_motivo_invalido_no_anota_caso_ni_crea_compensacion_ni_toca_los_saldos() {
        let mut a = almacen();
        let id = con_traspaso(&mut a, "x");

        assert!(anular_transferencia(id, "corto", HOY, &mut a).is_err());

        assert!(a.casos.is_empty(), "sin caso huérfano");
        assert_eq!(a.transferencias.len(), 1, "ninguna compensación");
        assert_eq!(a.obtener_transferencia(id).unwrap().estado, EstadoTraspaso::Vigente);
        assert_eq!(a.saldo(11).unwrap(), dop(11_000.0));
    }

    #[test]
    fn un_traspaso_sin_descripcion_se_anota_con_el_texto_generico() {
        let mut a = almacen();
        let id = con_traspaso(&mut a, "");

        anular(id, &mut a).unwrap();

        assert!(a.casos[0].descripcion.starts_with("Traspaso entre cuentas (original #"), "{}", a.casos[0].descripcion);
        assert!(compensacion_de(&a, id).descripcion.ends_with(": Traspaso entre cuentas"));
    }

    #[test]
    fn una_cuenta_archivada_rechaza_la_anulacion_con_el_mismo_mensaje_de_siempre() {
        let mut a = almacen();
        let id = con_traspaso(&mut a, "ida");
        a.ajustar_saldo(11, dop(-11_000.0)).unwrap(); // el destino lo gastó todo: saldo cero
        assert!(a.archivar_cuenta(11).unwrap());
        let antes = (a.saldo_de(10), a.transferencias.len(), a.casos.len());

        let e = anular(id, &mut a).unwrap_err();

        assert_eq!(e.to_string(), "La cuenta «Cuenta Corriente DOP» está archivada: reactívala antes de usarla en una operación.");
        assert_eq!((a.saldo_de(10), a.transferencias.len(), a.casos.len()), antes, "sin efectos");
        assert_eq!(a.obtener_transferencia(id).unwrap().estado, EstadoTraspaso::Vigente);
    }

    #[test]
    fn un_fallo_al_insertar_la_compensacion_no_marca_el_original_ni_mueve_saldos() {
        let mut a = almacen();
        let id = con_traspaso(&mut a, "x");
        a.falla_al_insertar = true;

        assert!(anular(id, &mut a).is_err());

        assert_eq!(a.obtener_transferencia(id).unwrap().estado, EstadoTraspaso::Vigente);
        assert_eq!((a.saldo_de(10), a.saldo_de(11)), (dop(49_000.0), dop(11_000.0)), "los de después del traspaso");
        assert!(a.casos.is_empty());
    }

    #[test]
    fn h10_si_el_destino_gasto_lo_recibido_el_saldo_queda_negativo() {
        // **H10 resuelto**, conservado: la inversa exacta, sin recorte en cero. El total repartido no cambia.
        let mut a = almacen();
        let id = transferir(datos(dop(8_000.0), dop(8_000.0), dop(0.0)), &mut a).unwrap();
        a.ajustar_saldo(11, dop(-15_000.0)).unwrap(); // el destino queda en 3 000
        assert_eq!(a.saldo_de(11), dop(3_000.0));
        let total_antes = a.saldo_de(10).sumar(&a.saldo_de(11)).unwrap();

        anular(id, &mut a).unwrap();

        assert_eq!(a.saldo_de(10), dop(50_000.0), "el origen recupera todo");
        assert_eq!(a.saldo_de(11), dop(-5_000.0), "y el destino devuelve todo");
        let total_despues = a.saldo_de(10).sumar(&a.saldo_de(11)).unwrap();
        assert_eq!(total_despues, total_antes, "anular no crea ni destruye dinero");
    }

    #[test]
    fn invariante_los_saldos_tras_anular_son_los_de_antes_del_traspaso() {
        // Recorre combinaciones de divisa y cargo: tras transferir y anular, todo vuelve a su punto de partida.
        let casos = [
            (11, dop(1_234.56), dop(1_234.56), dop(0.0)),
            (11, dop(1_234.56), dop(1_234.56), dop(7.77)),
            (12, dop(6_000.0), usd(100.0), dop(0.0)),
            (12, dop(6_000.0), usd(97.31), dop(35.0)),
        ];
        for (destino, mo, md, cargo) in casos {
            let mut a = almacen();
            let antes: Vec<_> = [10, 11, 12].iter().map(|c| a.saldo_de(*c)).collect();
            let mut d = datos(mo, md, cargo);
            d.destino_id = destino;
            let id = transferir(d, &mut a).unwrap();

            anular(id, &mut a).unwrap();

            let despues: Vec<_> = [10, 11, 12].iter().map(|c| a.saldo_de(*c)).collect();
            assert_eq!(despues, antes, "destino {destino}, cargo {cargo:?}");
        }
    }
}
