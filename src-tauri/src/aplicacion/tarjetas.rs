//! Casos de uso de las tarjetas (parte 1): listarlas con su cupo y sus recordatorios, darlas de alta y corregir sus
//! límites.
//!
//! Lo fijan las pruebas de caracterización `r1`–`r7`. **Hallazgos documentados, sin corregir** (el cambio se consulta):
//! el alta no recorta ni valida entidad y nombre ni que los límites sean positivos (`r2`; los días de corte y de pago
//! los valida el esquema), y corregir los límites de una tarjeta que no existe no dice nada (`r4`).

use super::ErrorAplicacion;
use crate::dominio::dinero::Dinero;
use crate::dominio::tarjeta::{aviso_de_corte, aviso_de_pago, cupo_para_mostrar, PoliticaLiquidacion};
use crate::dominio::dinero::Divisa;
use crate::puertos::repositorios::*;

pub struct DatosTarjetaNueva {
    pub entidad: String,
    pub nombre_tarjeta: String,
    pub limite_pesos: Dinero,
    pub limite_dolares: Dinero,
    pub sobregiro_pesos: Dinero,
    pub sobregiro_dolares: Dinero,
    pub balance_pesos: Dinero,
    pub balance_dolares: Dinero,
    pub balance_corte_pesos: Dinero,
    pub balance_corte_dolares: Dinero,
    pub fecha_corte: i32,
    pub fecha_limite_pago: i32,
}

pub struct DatosLimites {
    pub id: i64,
    pub limite_pesos: Dinero,
    pub limite_dolares: Dinero,
    pub sobregiro_pesos: Dinero,
    pub sobregiro_dolares: Dinero,
    pub balance_corte_pesos: Dinero,
    pub balance_corte_dolares: Dinero,
    pub limite_ajustado_pesos: Option<Dinero>,
    pub limite_ajustado_dolares: Option<Dinero>,
    /// El código tal como llegó; uno ausente o desconocido es «origen».
    pub politica_liquidacion: Option<String>,
}

/// Una tarjeta con lo que el listado le añade: el cupo por divisa, la política normalizada y los recordatorios.
#[derive(Debug, Clone, PartialEq)]
pub struct TarjetaConAvisos {
    pub leida: TarjetaLeida,
    pub politica_liquidacion: String,
    pub limite_efectivo_pesos: f64,
    pub limite_efectivo_dolares: f64,
    pub disponible_pesos: f64,
    pub disponible_dolares: f64,
    pub alerta_corte: bool,
    pub alerta_pago: bool,
    pub dias_corte_msg: String,
    pub dias_pago_msg: String,
}

/// Las tarjetas en el orden en que se crearon, con su cupo y sus recordatorios a fecha `dia_actual` (día del mes).
pub fn listar_tarjetas(
    dia_actual: i32,
    almacen: &impl CatalogoDeTarjetas,
) -> Result<Vec<TarjetaConAvisos>, ErrorAplicacion> {
    Ok(almacen
        .tarjetas()?
        .into_iter()
        .map(|t| {
            let (efectivo_dop, disponible_dop) = cupo_para_mostrar(
                Divisa::Dop,
                t.limite_pesos,
                t.limite_ajustado_pesos,
                t.limite_sobregiro_pesos,
                t.balance_pesos,
            );
            let (efectivo_usd, disponible_usd) = cupo_para_mostrar(
                Divisa::Usd,
                t.limite_dolares,
                t.limite_ajustado_dolares,
                t.limite_sobregiro_dolares,
                t.balance_dolares,
            );
            let (alerta_corte, dias_corte_msg) = aviso_de_corte(t.fecha_corte, dia_actual);
            let (alerta_pago, dias_pago_msg) = aviso_de_pago(t.fecha_limite_pago, dia_actual);
            let politica = PoliticaLiquidacion::desde_codigo(t.politica_liquidacion.as_deref()).codigo().to_string();
            TarjetaConAvisos {
                leida: t,
                politica_liquidacion: politica,
                limite_efectivo_pesos: efectivo_dop,
                limite_efectivo_dolares: efectivo_usd,
                disponible_pesos: disponible_dop,
                disponible_dolares: disponible_usd,
                alerta_corte,
                alerta_pago,
                dias_corte_msg,
                dias_pago_msg,
            }
        })
        .collect())
}

pub fn crear_tarjeta(datos: DatosTarjetaNueva, almacen: &mut impl CatalogoDeTarjetas) -> Result<i64, ErrorAplicacion> {
    Ok(almacen.insertar_tarjeta(&TarjetaNueva {
        entidad: datos.entidad,
        nombre_tarjeta: datos.nombre_tarjeta,
        limite_pesos: datos.limite_pesos.unidades(),
        limite_dolares: datos.limite_dolares.unidades(),
        limite_sobregiro_pesos: datos.sobregiro_pesos.unidades(),
        limite_sobregiro_dolares: datos.sobregiro_dolares.unidades(),
        balance_pesos: datos.balance_pesos.unidades(),
        balance_dolares: datos.balance_dolares.unidades(),
        balance_corte_pesos: datos.balance_corte_pesos.unidades(),
        balance_corte_dolares: datos.balance_corte_dolares.unidades(),
        fecha_corte: datos.fecha_corte,
        fecha_limite_pago: datos.fecha_limite_pago,
    })?)
}

pub fn actualizar_limites(datos: DatosLimites, almacen: &mut impl CatalogoDeTarjetas) -> Result<(), ErrorAplicacion> {
    Ok(almacen.actualizar_limites(&LimitesDeTarjeta {
        id: datos.id,
        limite_pesos: datos.limite_pesos.unidades(),
        limite_dolares: datos.limite_dolares.unidades(),
        limite_sobregiro_pesos: datos.sobregiro_pesos.unidades(),
        limite_sobregiro_dolares: datos.sobregiro_dolares.unidades(),
        balance_corte_pesos: datos.balance_corte_pesos.unidades(),
        balance_corte_dolares: datos.balance_corte_dolares.unidades(),
        limite_ajustado_pesos: datos.limite_ajustado_pesos.map(|d| d.unidades()),
        limite_ajustado_dolares: datos.limite_ajustado_dolares.map(|d| d.unidades()),
        politica_liquidacion: PoliticaLiquidacion::desde_codigo(datos.politica_liquidacion.as_deref()).codigo().to_string(),
    })?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::puertos::dobles::TarjetasEnMemoria;

    fn dop(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Dop).unwrap()
    }
    fn usd(u: f64) -> Dinero {
        Dinero::nuevo(u, Divisa::Usd).unwrap()
    }

    fn nueva(nombre: &str, corte: i32, pago: i32) -> DatosTarjetaNueva {
        DatosTarjetaNueva {
            entidad: "Banco".into(),
            nombre_tarjeta: nombre.into(),
            limite_pesos: dop(1000.0),
            limite_dolares: usd(2000.0),
            sobregiro_pesos: dop(300.0),
            sobregiro_dolares: usd(400.0),
            balance_pesos: dop(500.0),
            balance_dolares: usd(600.0),
            balance_corte_pesos: dop(700.0),
            balance_corte_dolares: usd(800.0),
            fecha_corte: corte,
            fecha_limite_pago: pago,
        }
    }

    fn limites(id: i64, ajustado_dop: Option<f64>, politica: Option<&str>) -> DatosLimites {
        DatosLimites {
            id,
            limite_pesos: dop(1000.0),
            limite_dolares: usd(2000.0),
            sobregiro_pesos: dop(300.0),
            sobregiro_dolares: usd(400.0),
            balance_corte_pesos: dop(50.0),
            balance_corte_dolares: usd(60.0),
            limite_ajustado_pesos: ajustado_dop.map(dop),
            limite_ajustado_dolares: None,
            politica_liquidacion: politica.map(|p| p.to_string()),
        }
    }

    #[test]
    fn crear_guarda_cada_importe_en_su_columna_sin_ajuste_ni_politica() {
        let mut a = TarjetasEnMemoria::nuevo();
        let id = crear_tarjeta(nueva("Visa", 15, 5), &mut a).unwrap();
        let t = &a.tarjetas[0];
        assert_eq!(t.id, id);
        assert_eq!((t.limite_pesos, t.limite_dolares, t.limite_sobregiro_pesos, t.limite_sobregiro_dolares), (1000.0, 2000.0, 300.0, 400.0));
        assert_eq!((t.balance_pesos, t.balance_dolares, t.balance_corte_pesos, t.balance_corte_dolares), (500.0, 600.0, 700.0, 800.0));
        assert_eq!((t.limite_ajustado_pesos, t.politica_liquidacion.clone()), (None, None));
    }

    #[test]
    fn un_dia_fuera_del_mes_lo_rechaza_el_almacen_y_no_deja_nada() {
        let mut a = TarjetasEnMemoria::nuevo();
        for (corte, pago) in [(0, 5), (32, 5), (15, 0), (15, 32)] {
            let e = crear_tarjeta(nueva("T", corte, pago), &mut a).unwrap_err();
            assert!(matches!(e, ErrorAplicacion::Almacen(ErrorAlmacen::Fallo(ref d)) if d.contains("CHECK")), "{corte}/{pago}: {e:?}");
        }
        assert!(a.tarjetas.is_empty());
    }

    #[test]
    fn actualizar_limites_guarda_el_ajuste_distingue_cero_de_ninguno_y_normaliza_la_politica() {
        let mut a = TarjetasEnMemoria::nuevo();
        let id = crear_tarjeta(nueva("Visa", 15, 5), &mut a).unwrap();
        actualizar_limites(limites(id, Some(0.0), Some("traduce")), &mut a).unwrap();
        assert_eq!((a.tarjetas[0].limite_ajustado_pesos, a.tarjetas[0].politica_liquidacion.as_deref()), (Some(0.0), Some("traduce")));
        assert_eq!((a.tarjetas[0].balance_corte_pesos, a.tarjetas[0].balance_pesos), (50.0, 500.0), "el corte cambia; el balance no");

        actualizar_limites(limites(id, None, Some("cualquier cosa")), &mut a).unwrap();
        assert_eq!((a.tarjetas[0].limite_ajustado_pesos, a.tarjetas[0].politica_liquidacion.as_deref()), (None, Some("origen")));
        actualizar_limites(limites(id, None, None), &mut a).unwrap();
        assert_eq!(a.tarjetas[0].politica_liquidacion.as_deref(), Some("origen"));
    }

    #[test]
    fn actualizar_los_limites_de_una_tarjeta_inexistente_no_dice_nada() {
        let mut a = TarjetasEnMemoria::nuevo();
        assert!(actualizar_limites(limites(404, None, None), &mut a).is_ok());
    }

    #[test]
    fn el_listado_trae_el_cupo_la_politica_y_los_recordatorios() {
        let mut a = TarjetasEnMemoria::nuevo();
        let id = crear_tarjeta(nueva("Visa", 10, 14), &mut a).unwrap();
        actualizar_limites(limites(id, Some(800.0), Some("traduce")), &mut a).unwrap();
        let t = &listar_tarjetas(10, &a).unwrap()[0];
        assert_eq!(t.politica_liquidacion, "traduce");
        assert_eq!((t.limite_efectivo_pesos, t.limite_efectivo_dolares), (800.0, 2000.0));
        assert_eq!((t.disponible_pesos, t.disponible_dolares), (800.0 + 300.0 - 500.0, 2000.0 + 400.0 - 600.0));
        assert_eq!((t.alerta_corte, t.dias_corte_msg.as_str()), (true, "Hoy es la fecha de corte"));
        assert_eq!((t.alerta_pago, t.dias_pago_msg.as_str()), (false, "Faltan 4 días para pagar"));
    }

    #[test]
    fn el_listado_va_en_orden_de_creacion_y_una_politica_ausente_es_origen() {
        let mut a = TarjetasEnMemoria::nuevo();
        let primera = crear_tarjeta(nueva("Primera", 15, 5), &mut a).unwrap();
        let segunda = crear_tarjeta(nueva("Segunda", 15, 5), &mut a).unwrap();
        let lista = listar_tarjetas(1, &a).unwrap();
        assert_eq!(lista.iter().map(|t| t.leida.id).collect::<Vec<_>>(), vec![primera, segunda]);
        assert_eq!(lista[0].politica_liquidacion, "origen");
    }
}
