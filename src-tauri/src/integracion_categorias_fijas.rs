//! Pruebas de integración de la marca fija/recurrente por categoría (`marcar_categoria_fija`) y de su efecto en el
//! resumen de gastos (`obtener_resumen_gastos`: `totales_fijo` y `totales_variable`).
//!
//! Ejercitan los **comandos reales** de punta a punta sobre una base SQLite temporal con el esquema real (el mismo
//! entorno aislado de `caracterizacion`). Los datos se siembran con los comandos reales (los de `integracion_resumen`:
//! transferencias, consumos en dólares liquidados, un abono con su comisión y un avance con su cargo). Todos los datos
//! son inventados, con céntimos poco redondos.
//!
//! Se reconocen por el prefijo `integracion_`: `cargo test integracion_`.

use crate::caracterizacion::entorno_para_pruebas_externas;
use crate::comandos::catalogos::{crear_categoria, marcar_categoria_fija, obtener_categorias};
use crate::comandos::resumen::{resumen_en, ResumenGastos};
use crate::integracion_resumen::{categoria_por_nombre, centavos, conexion, fila, hoy, huella_de_la_base, sembrar, verificar_forma};
use crate::{db_sql, migraciones};
use rusqlite::Connection;

const TIPOS_IPC: &str = include_str!("../../src/js/tipos-ipc.d.ts");

fn septiembre() -> ResumenGastos {
    resumen_en(hoy(2026, 10, 28), 2026, 9, 3, 60.0, vec![]).unwrap()
}

fn marcar(nombre: &str, fija: bool) {
    let c = marcar_categoria_fija(categoria_por_nombre(nombre), fija).unwrap();
    assert_eq!((c.nombre.as_str(), c.fija), (nombre, fija));
}

/// Verifica la identidad central: lo fijo más lo variable es el total, al centavo, tanto en el mes como en el anterior.
fn suman_el_total(r: &ResumenGastos) {
    assert_eq!(
        centavos(&r.totales_fijo.importe) + centavos(&r.totales_variable.importe),
        centavos(&r.totales.importe),
        "fijo + variable = total del mes"
    );
    assert_eq!(
        centavos(&r.totales_fijo.anterior) + centavos(&r.totales_variable.anterior),
        centavos(&r.totales.anterior),
        "fijo + variable = total del mes anterior"
    );
}

#[test]
fn integracion_marcar_con_el_comando_real_reparte_fijo_y_variable_y_suman_el_total() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    // Ninguna categoría viene marcada: todo es variable.
    let r = septiembre();
    assert_eq!(r.totales_fijo.importe, "0.00");
    assert_eq!(r.totales_variable.importe, r.totales.importe);
    assert_eq!((r.totales_fijo.porcentaje_milesimas, r.totales_variable.porcentaje_milesimas), (0, 1000));
    assert!(r.filas.iter().all(|f| !f.fija));

    // «Alimentación» y «Otros» (donde caen los consumos en dólares y también los cargos derivados) quedan marcadas.
    marcar("Alimentación", true);
    marcar("Otros", true);
    let r = septiembre();

    assert!(fila(&r, "Alimentación").fija && fila(&r, "Otros").fija);
    assert!(!fila(&r, "Transporte").fija);
    let derivados = r.filas.iter().find(|f| f.derivado).expect("fila de cargos derivados");
    assert!(!derivados.fija, "los cargos derivados cuentan como variables aunque su categoría sea fija");

    // Fijo: Alimentación 1223.35 + Otros 3931.07 (USD liquidado y pendiente convertido). Variable: Transporte 3183.64
    // + cargos derivados 78.44.
    assert_eq!(r.totales_fijo.importe, "5154.42");
    assert_eq!(r.totales_variable.importe, "3262.08");
    assert_eq!(r.totales.importe, "8416.50");
    // 5154.42 / 8416.50 = 612.4 milésimas -> 612; 3262.08 / 8416.50 = 387.6 -> 388 (suman 1000).
    assert_eq!((r.totales_fijo.porcentaje_milesimas, r.totales_variable.porcentaje_milesimas), (612, 388));
    // Agosto: solo Alimentación (517.41) es fija; el resto es variable.
    assert_eq!(r.totales_fijo.anterior, fila(&r, "Alimentación").medidas.anterior);
    assert!(r.totales_fijo.variacion_anterior_pb.is_some());
    suman_el_total(&r);

    // Cada importe es la suma de las filas de su segmento.
    let suma = |fija: bool| -> i64 { r.filas.iter().filter(|f| f.fija == fija).map(|f| centavos(&f.medidas.importe)).sum() };
    assert_eq!(centavos(&r.totales_fijo.importe), suma(true));
    assert_eq!(centavos(&r.totales_variable.importe), suma(false));

    // Los meses cerrados y el parcial también suman exacto.
    for (h, mes) in [(hoy(2026, 10, 28), 8), (hoy(2026, 9, 15), 9)] {
        suman_el_total(&resumen_en(h, 2026, mes, 3, 60.0, vec![]).unwrap());
    }
    // Y con una exclusión, sigue sumando el total que queda.
    let excluida = categoria_por_nombre("Transporte");
    let r = resumen_en(hoy(2026, 10, 28), 2026, 9, 3, 60.0, vec![excluida, -1]).unwrap();
    suman_el_total(&r);
    assert_eq!(r.totales_variable.importe, "0.00", "sin Transporte ni derivados no queda nada variable");
}

#[test]
fn integracion_con_todas_las_categorias_marcadas_solo_los_cargos_derivados_quedan_como_variables() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    for c in obtener_categorias().unwrap() {
        marcar_categoria_fija(c.id, true).unwrap();
    }
    let r = septiembre();
    // La comisión del abono (4.22) y el cargo del avance (74.22) son lo único variable, sea cual sea su categoría.
    assert_eq!(r.totales_variable.importe, "78.44");
    assert_eq!(r.totales_fijo.importe, "8338.06");
    assert!(r.filas.iter().filter(|f| f.derivado).all(|f| !f.fija));
    assert!(r.filas.iter().filter(|f| !f.derivado).all(|f| f.fija));
    suman_el_total(&r);
}

#[test]
fn integracion_marcar_la_categoria_equivocada_no_mueve_las_demas_y_desmarcar_lo_deshace() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    marcar("Transporte", true);
    let r = septiembre();
    assert_eq!(r.totales_fijo.importe, "3183.64");
    assert!(fila(&r, "Transporte").fija);
    assert!(!fila(&r, "Alimentación").fija && !fila(&r, "Otros").fija);
    let marcadas: Vec<String> = obtener_categorias().unwrap().into_iter().filter(|c| c.fija).map(|c| c.nombre).collect();
    assert_eq!(marcadas, vec!["Transporte".to_string()]);

    marcar("Transporte", false);
    let r = septiembre();
    assert_eq!(r.totales_fijo.importe, "0.00");
    assert!(obtener_categorias().unwrap().iter().all(|c| !c.fija));
}

#[test]
fn integracion_marcar_no_altera_ningun_gasto_saldo_ni_tabla_salvo_la_marca() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    let sin_categorias = |h: String| -> String { h.lines().filter(|l| !l.starts_with("categorias:")).collect::<Vec<_>>().join("\n") };
    let antes = huella_de_la_base();
    let total_antes = septiembre().totales.importe;

    marcar("Alimentación", true);
    marcar("Otros", true);
    marcar("Alimentación", false);

    assert_eq!(sin_categorias(huella_de_la_base()), sin_categorias(antes), "gastos, cuentas, tarjetas y abonos intactos");
    assert_eq!(septiembre().totales.importe, total_antes, "el total del mes no depende de la marca");
    // Y en la tabla de categorías cambia solo la columna `fija`.
    let c = conexion();
    let fijas: i64 = c.query_row("SELECT COUNT(*) FROM categorias WHERE fija = 1;", [], |r| r.get(0)).unwrap();
    assert_eq!(fijas, 1);
}

#[test]
fn integracion_una_categoria_inexistente_es_un_error_legible_y_no_escribe() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    let antes = huella_de_la_base();
    let e = marcar_categoria_fija(987_654, true).unwrap_err();
    assert!(e.contains("No se encontró la categoría") && e.contains("987654"), "{e}");
    assert_eq!(huella_de_la_base(), antes);
}

#[test]
fn integracion_una_categoria_nueva_nace_variable_y_el_catalogo_trae_la_marca() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    let nueva = crear_categoria("Gimnasio inventado".into()).unwrap();
    assert!(!nueva.fija);
    marcar_categoria_fija(nueva.id, true).unwrap();
    let catalogo = obtener_categorias().unwrap();
    assert!(catalogo.iter().find(|c| c.id == nueva.id).unwrap().fija);
    assert_eq!(catalogo.iter().filter(|c| c.fija).count(), 1);
}

#[test]
fn integracion_una_categoria_fija_sin_gasto_en_el_mes_aporta_cero() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    let nueva = crear_categoria("Seguro inventado".into()).unwrap();
    marcar_categoria_fija(nueva.id, true).unwrap();
    let r = septiembre();
    assert!(r.filas.iter().all(|f| f.categoria_nombre != "Seguro inventado"), "sin gasto no hay fila");
    assert_eq!(r.totales_fijo.importe, "0.00");
    suman_el_total(&r);
}

#[test]
fn integracion_el_dto_y_el_catalogo_tienen_la_forma_de_tipos_ipc() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    marcar("Alimentación", true);
    for (h, mes) in [(hoy(2026, 10, 28), 9), (hoy(2026, 9, 15), 9)] {
        let json = serde_json::to_value(resumen_en(h, 2026, mes, 3, 60.0, vec![]).unwrap()).unwrap();
        verificar_forma(TIPOS_IPC, "ResumenGastos", &json, "ResumenGastos");
        for segmento in ["totales_fijo", "totales_variable"] {
            let s = &json[segmento];
            assert!(s["importe"].is_string() && s["anterior"].is_string(), "los importes viajan como texto exacto");
            assert!(s["porcentaje_milesimas"].is_i64());
        }
    }
    let catalogo = serde_json::to_value(obtener_categorias().unwrap()).unwrap();
    for (i, c) in catalogo.as_array().unwrap().iter().enumerate() {
        verificar_forma(TIPOS_IPC, "Categoria", c, &format!("Categoria[{i}]"));
    }
    let marcada = serde_json::to_value(marcar_categoria_fija(categoria_por_nombre("Transporte"), true).unwrap()).unwrap();
    verificar_forma(TIPOS_IPC, "Categoria", &marcada, "Categoria(marcar)");
    let linea = TIPOS_IPC.lines().find(|l| l.contains("marcar_categoria_fija:")).expect("comando declarado");
    assert!(linea.contains("id: number") && linea.contains("fija: boolean") && linea.contains("ret: Categoria"), "{linea}");
    let fila_dts = TIPOS_IPC.lines().find(|l| l.starts_with("export type FilaResumenGasto")).unwrap();
    assert!(fila_dts.contains("fija: boolean"));
}

#[test]
fn integracion_respaldo_y_restauracion_conservan_las_marcas() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    marcar("Alimentación", true);
    let esperado = septiembre();
    let nombre = crate::comandos::respaldos::crear_respaldo().unwrap();
    let nombre = std::path::Path::new(&nombre).file_name().unwrap().to_string_lossy().to_string();

    // Después del respaldo cambian las marcas; la restauración debe devolver las del respaldo.
    marcar("Alimentación", false);
    marcar("Transporte", true);
    assert_eq!(septiembre().totales_fijo.importe, "3183.64");

    let r = crate::comandos::respaldos::restaurar_respaldo(nombre).unwrap();
    assert_eq!(r["version_del_esquema"], migraciones::VERSION_OBJETIVO);
    let marcadas: Vec<String> = obtener_categorias().unwrap().into_iter().filter(|c| c.fija).map(|c| c.nombre).collect();
    assert_eq!(marcadas, vec!["Alimentación".to_string()]);
    let despues = septiembre();
    assert_eq!(despues.totales_fijo.importe, esperado.totales_fijo.importe);
    assert_eq!(despues.totales_variable.importe, esperado.totales_variable.importe);
}

#[test]
fn integracion_migrar_una_base_v19_deja_todas_las_categorias_variables_y_el_resumen_intacto() {
    let _g = entorno_para_pruebas_externas();
    let ruta = db_sql::obtener_ruta_db();
    assert!(ruta.starts_with(std::env::temp_dir().to_str().unwrap()), "ABORTADO: apuntaría a la base real ({ruta})");
    let _ = std::fs::remove_file(&ruta);
    for sufijo in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{ruta}{sufijo}"));
    }
    {
        // Una base como la dejaría la versión 19 (la instalada), sembrada con datos inventados.
        let mut c = Connection::open(&ruta).unwrap();
        migraciones::migrar_hasta(&mut c, 19);
        assert_eq!(migraciones::version_de(&c).unwrap(), 19);
        let columnas: i64 = c.query_row("SELECT COUNT(*) FROM pragma_table_info('categorias') WHERE name = 'fija'", [], |r| r.get(0)).unwrap();
        assert_eq!(columnas, 0, "la v19 no tiene la columna");
        c.execute_batch(
            "INSERT INTO categorias (nombre) VALUES ('Alquiler'), ('Suscripciones'), ('Otros'), ('Comida inventada');
             INSERT INTO gastos (fecha, monto, divisa, descripcion, categoria_id, metodo_pago, costo_adicional)
                 VALUES ('05/08/2026', 18750.37, 'DOP', 'Alquiler viejo', 1, 'transferencia', 0.0),
                        ('05/09/2026', 18750.37, 'DOP', 'Alquiler nuevo', 1, 'transferencia', 0.0),
                        ('09/09/2026', 1187.53, 'DOP', 'Comida', 4, 'transferencia', 2.38);",
        )
        .unwrap();
    }
    let antes_de_migrar = {
        let c = Connection::open(&ruta).unwrap();
        c.query_row("SELECT COUNT(*) FROM gastos;", [], |r| r.get::<_, i64>(0)).unwrap()
    };

    db_sql::inicializar_db().expect("la base v19 migra a v20");

    let c = conexion();
    assert_eq!(migraciones::version_de(&c).unwrap(), 20);
    let marcadas: i64 = c.query_row("SELECT COUNT(*) FROM categorias WHERE fija <> 0;", [], |r| r.get(0)).unwrap();
    assert_eq!(marcadas, 0, "la migración no marca ninguna, ni «Alquiler» ni «Suscripciones»");
    let despues: i64 = c.query_row("SELECT COUNT(*) FROM gastos;", [], |r| r.get(0)).unwrap();
    assert_eq!(despues, antes_de_migrar);
    drop(c);

    let r = septiembre();
    assert_eq!(r.totales.importe, "19940.28");
    assert_eq!(r.totales_fijo.importe, "0.00");
    assert_eq!(r.totales_variable.importe, "19940.28");
    assert!(obtener_categorias().unwrap().iter().all(|c| !c.fija));

    // El titular puede ahora marcar «Alquiler» y el reparto cambia sin mover el total.
    marcar("Alquiler", true);
    let r = septiembre();
    assert_eq!((r.totales_fijo.importe.as_str(), r.totales_variable.importe.as_str()), ("18750.37", "1189.91"));
    suman_el_total(&r);
}
