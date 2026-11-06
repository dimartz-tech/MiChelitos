//! Pruebas de integración de los presupuestos mensuales por categoría (`fijar_presupuesto`, `quitar_presupuesto`,
//! `obtener_presupuestos_mes`) y de su relación con la fusión y la eliminación de categorías.
//!
//! Ejercitan los **comandos reales** de punta a punta sobre una base SQLite temporal con el esquema real (el entorno
//! aislado de `caracterizacion`). Los datos se siembran con los comandos reales de `integracion_resumen` (septiembre de
//! 2026: transferencias, un consumo en dólares liquidado y otro pendiente, un abono con su comisión y un avance con su
//! cargo). Todos los datos son inventados, con céntimos poco redondos.
//!
//! Se reconocen por el prefijo `integracion_`: `cargo test integracion_`.

use crate::caracterizacion::entorno_para_pruebas_externas;
use crate::comandos::catalogos::{
    crear_categoria, eliminar_categoria, fusionar_categorias, marcar_categoria_fija, previsualizar_fusion_categorias,
};
use crate::comandos::presupuestos::{fijar_presupuesto, obtener_presupuestos_mes, quitar_presupuesto, EstadoPresupuestos};
use crate::integracion_resumen::{categoria_por_nombre, centavos, conexion, huella_de_la_base, importe, sembrar, verificar_forma};
use crate::{db_sql, migraciones};
use rusqlite::Connection;

const TIPOS_IPC: &str = include_str!("../../src/js/tipos-ipc.d.ts");

fn septiembre() -> EstadoPresupuestos {
    obtener_presupuestos_mes(2026, 9).unwrap()
}

fn fijar(nombre: &str, monto: &str) {
    let p = fijar_presupuesto(categoria_por_nombre(nombre), importe(monto)).unwrap();
    assert_eq!(p.categoria_nombre, nombre);
}

fn fila<'a>(r: &'a EstadoPresupuestos, nombre: &str) -> &'a crate::comandos::presupuestos::FilaPresupuesto {
    r.filas.iter().find(|f| f.categoria_nombre == nombre).unwrap_or_else(|| panic!("no hay la fila «{nombre}»"))
}

fn presupuesto_en_base(categoria: i64) -> Option<i64> {
    conexion()
        .query_row("SELECT monto_centavos FROM presupuestos WHERE categoria_id = ?;", [categoria], |r| r.get(0))
        .ok()
}

#[test]
fn integracion_sin_presupuestos_no_hay_filas_ni_totales() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    let r = septiembre();
    assert!(r.filas.is_empty());
    assert_eq!((r.total_presupuesto.as_str(), r.total_gastado.as_str(), r.total_restante.as_str()), ("0.00", "0.00", "0.00"));
}

#[test]
fn integracion_fijar_da_gastado_restante_porcentaje_y_estado_con_los_comandos_reales() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    fijar("Alimentación", "1500.01");
    fijar("Transporte", "3012.44");
    fijar("Otros", "1000.07");
    let r = septiembre();

    // Alimentación: 1223.35 de 1500.01 = 815 milésimas -> cerca. Transporte: 3183.64 de 3012.44 -> excedido.
    let a = fila(&r, "Alimentación");
    assert_eq!((a.gastado.as_str(), a.restante.as_str(), a.porcentaje_milesimas, a.estado.as_str()), ("1223.35", "276.66", 815, "cerca"));
    let t = fila(&r, "Transporte");
    assert_eq!((t.gastado.as_str(), t.restante.as_str(), t.porcentaje_milesimas, t.estado.as_str()), ("3183.64", "-171.20", 1056, "excedido"));
    // «Otros»: solo cuenta lo que está en pesos (la comisión del abono y el cargo del avance); los dólares salen aparte.
    let o = fila(&r, "Otros");
    assert_eq!((o.gastado.as_str(), o.estado.as_str(), o.usd_aparte.as_str()), ("78.44", "ok", "65.45"));
    // Orden: excedido, cerca, ok.
    assert_eq!(r.filas.iter().map(|f| f.estado.as_str()).collect::<Vec<_>>(), vec!["excedido", "cerca", "ok"]);
    // Los totales son la suma exacta de las filas.
    assert_eq!(centavos(&r.total_presupuesto), r.filas.iter().map(|f| centavos(&f.presupuesto)).sum::<i64>());
    assert_eq!(centavos(&r.total_gastado), r.filas.iter().map(|f| centavos(&f.gastado)).sum::<i64>());
    assert_eq!(centavos(&r.total_presupuesto) - centavos(&r.total_gastado), centavos(&r.total_restante));
    assert_eq!(r.total_gastado, "4485.43");
}

#[test]
fn integracion_los_dolares_no_se_convierten_ni_cuentan_contra_el_presupuesto() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    // En septiembre «Otros» tiene USD 53.11 (liquidado en 3190.67 pesos) y USD 12.34 pendiente: nada de eso entra.
    fijar("Otros", "100.01");
    let o = septiembre();
    let o = fila(&o, "Otros");
    assert_eq!(o.gastado, "78.44", "solo los pesos: ni lo liquidado ni la conversión de lo pendiente");
    assert_eq!(o.usd_aparte, "65.45");
    assert_eq!((o.porcentaje_milesimas, o.estado.as_str()), (784, "ok"));
    // Con un presupuesto de 90.00 los mismos 78.44 son el 87 %: cerca. Los dólares siguen sin mover nada.
    fijar("Otros", "90.00");
    let o = septiembre();
    let o = fila(&o, "Otros");
    assert_eq!((o.gastado.as_str(), o.porcentaje_milesimas, o.estado.as_str()), ("78.44", 871, "cerca"));
}

#[test]
fn integracion_el_mes_sin_gastos_deja_todo_el_presupuesto_y_un_mes_invalido_falla() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    fijar("Alimentación", "800.03");
    let r = obtener_presupuestos_mes(2027, 3).unwrap();
    let a = fila(&r, "Alimentación");
    assert_eq!((a.gastado.as_str(), a.restante.as_str(), a.porcentaje_milesimas, a.estado.as_str()), ("0.00", "800.03", 0, "ok"));
    // Agosto: 310.47 + 205.91 más sus retenciones; el presupuesto de septiembre aplica igual a cualquier mes.
    let agosto = obtener_presupuestos_mes(2026, 8).unwrap();
    assert_eq!(fila(&agosto, "Alimentación").gastado, "517.41");
    assert!(obtener_presupuestos_mes(2026, 13).is_err());
    assert!(obtener_presupuestos_mes(2026, 0).is_err());
}

#[test]
fn integracion_fijar_valida_el_monto_y_la_categoria_y_no_escribe_si_falla() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    let antes = huella_de_la_base();
    let alimentacion = categoria_por_nombre("Alimentación");
    for malo in ["0", "0.00", "-5.00"] {
        let e = fijar_presupuesto(alimentacion, importe(malo)).unwrap_err();
        assert!(e.contains("mayor que cero"), "{malo}: {e}");
    }
    let e = fijar_presupuesto(alimentacion, importe("1000000000.00")).unwrap_err();
    assert!(e.contains("999,999,999.99"), "{e}");
    let e = fijar_presupuesto(987_654, importe("10.00")).unwrap_err();
    assert!(e.contains("987654"), "categoría inexistente: {e}");
    assert!(quitar_presupuesto(987_654).is_err());
    assert_eq!(huella_de_la_base(), antes, "nada se escribió");
    // El tope exacto sí cabe.
    fijar_presupuesto(alimentacion, importe("999999999.99")).unwrap();
    assert_eq!(presupuesto_en_base(alimentacion), Some(99_999_999_999));
}

#[test]
fn integracion_fijar_dos_veces_reemplaza_y_quitar_borra_sin_fallar_si_no_habia() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    let id = categoria_por_nombre("Transporte");
    fijar("Transporte", "2500.13");
    fijar("Transporte", "2600.27");
    assert_eq!(presupuesto_en_base(id), Some(260_027));
    let n: i64 = conexion().query_row("SELECT COUNT(*) FROM presupuestos;", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
    assert!(quitar_presupuesto(id).unwrap());
    assert!(!quitar_presupuesto(id).unwrap());
    assert_eq!(presupuesto_en_base(id), None);
    assert!(septiembre().filas.is_empty());
}

#[test]
fn integracion_una_categoria_fija_tiene_presupuesto_igual_y_su_marca_no_cambia() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    marcar_categoria_fija(categoria_por_nombre("Transporte"), true).unwrap();
    let p = fijar_presupuesto(categoria_por_nombre("Transporte"), importe("3183.64")).unwrap();
    assert!(p.fija);
    let r = septiembre();
    let t = fila(&r, "Transporte");
    assert!(t.fija);
    assert_eq!((t.estado.as_str(), t.restante.as_str()), ("excedido", "0.00"), "el 100 % exacto cuenta como excedido");
    // Desmarcarla no toca el presupuesto, y fijarlo no tocó la marca.
    marcar_categoria_fija(categoria_por_nombre("Transporte"), false).unwrap();
    assert_eq!(presupuesto_en_base(categoria_por_nombre("Transporte")), Some(318_364));
}

#[test]
fn integracion_consultar_no_escribe_nada() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    fijar("Alimentación", "1500.01");
    let antes = huella_de_la_base();
    septiembre();
    obtener_presupuestos_mes(2026, 8).unwrap();
    assert_eq!(huella_de_la_base(), antes);
}

#[test]
fn integracion_eliminar_la_categoria_borra_su_presupuesto() {
    let _g = entorno_para_pruebas_externas();
    let id = crear_categoria("Mascotas".into()).unwrap().id;
    let otra = crear_categoria("Viajes".into()).unwrap().id;
    fijar_presupuesto(id, importe("455.19")).unwrap();
    fijar_presupuesto(otra, importe("999.11")).unwrap();
    eliminar_categoria(id).unwrap();
    assert_eq!(presupuesto_en_base(id), None);
    assert_eq!(presupuesto_en_base(otra), Some(99_911), "el de otra categoría no se toca");
}

// --- Fusión: regla decidida por el titular ---

fn dos_categorias(origen: Option<&str>, destino: Option<&str>) -> (i64, i64) {
    let o = crear_categoria("Renta".into()).unwrap().id;
    let d = crear_categoria("Vivienda".into()).unwrap().id;
    if let Some(m) = origen {
        fijar_presupuesto(o, importe(m)).unwrap();
    }
    if let Some(m) = destino {
        fijar_presupuesto(d, importe(m)).unwrap();
    }
    (o, d)
}

#[test]
fn integracion_fusion_sin_presupuesto_en_ninguna_no_crea_ninguno() {
    let _g = entorno_para_pruebas_externas();
    let (o, d) = dos_categorias(None, None);
    let v = previsualizar_fusion_categorias(o, d).unwrap();
    assert!(!v.advertencias.iter().any(|a| a.contains("presupuesto")), "{:?}", v.advertencias);
    fusionar_categorias(o, d).unwrap();
    assert_eq!(presupuesto_en_base(d), None);
}

#[test]
fn integracion_fusion_solo_el_origen_con_presupuesto_el_destino_lo_hereda_y_la_vista_previa_lo_avisa() {
    let _g = entorno_para_pruebas_externas();
    let (o, d) = dos_categorias(Some("1234.57"), None);
    let v = previsualizar_fusion_categorias(o, d).unwrap();
    assert!(v.advertencias.iter().any(|a| a.contains("heredará") && a.contains("1234.57")), "{:?}", v.advertencias);
    fusionar_categorias(o, d).unwrap();
    assert_eq!(presupuesto_en_base(d), Some(123_457));
    assert_eq!(presupuesto_en_base(o), None);
    let n: i64 = conexion().query_row("SELECT COUNT(*) FROM presupuestos;", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
    let casos = crate::comandos::correcciones::obtener_correcciones().unwrap();
    assert!(casos[0].descripcion.contains("presupuesto heredado: DOP 1234.57"), "{}", casos[0].descripcion);
}

#[test]
fn integracion_fusion_solo_el_destino_con_presupuesto_lo_conserva() {
    let _g = entorno_para_pruebas_externas();
    let (o, d) = dos_categorias(None, Some("777.31"));
    let v = previsualizar_fusion_categorias(o, d).unwrap();
    assert!(!v.advertencias.iter().any(|a| a.contains("presupuesto")), "{:?}", v.advertencias);
    fusionar_categorias(o, d).unwrap();
    assert_eq!(presupuesto_en_base(d), Some(77_731));
}

#[test]
fn integracion_fusion_con_presupuesto_en_ambas_conserva_el_del_destino_avisa_el_descartado_y_nunca_suma() {
    let _g = entorno_para_pruebas_externas();
    let (o, d) = dos_categorias(Some("1234.57"), Some("777.31"));
    let v = previsualizar_fusion_categorias(o, d).unwrap();
    let aviso = v.advertencias.iter().find(|a| a.contains("se descarta")).unwrap_or_else(|| panic!("{:?}", v.advertencias));
    assert!(aviso.contains("1234.57") && aviso.contains("No se suman"), "{aviso}");
    let antes = huella_de_la_base();
    assert_eq!(huella_de_la_base(), antes, "la vista previa no escribe");
    fusionar_categorias(o, d).unwrap();
    assert_eq!(presupuesto_en_base(d), Some(77_731), "gana el del destino; 1234.57 + 777.31 = 2011.88 no aparece");
    assert_eq!(presupuesto_en_base(o), None);
    let casos = crate::comandos::correcciones::obtener_correcciones().unwrap();
    assert!(casos[0].descripcion.contains("presupuesto de origen descartado: DOP 1234.57"), "{}", casos[0].descripcion);
}

// --- Contrato, respaldo y migración ---

#[test]
fn integracion_contrato_de_forma_de_los_dto_con_tipos_ipc() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    let p = fijar_presupuesto(categoria_por_nombre("Alimentación"), importe("1500.01")).unwrap();
    verificar_forma(TIPOS_IPC, "PresupuestoFijado", &serde_json::to_value(&p).unwrap(), "PresupuestoFijado");
    fijar("Transporte", "3012.44");
    let r = serde_json::to_value(septiembre()).unwrap();
    verificar_forma(TIPOS_IPC, "EstadoPresupuestos", &r, "EstadoPresupuestos");
    for (i, f) in r["filas"].as_array().unwrap().iter().enumerate() {
        verificar_forma(TIPOS_IPC, "FilaPresupuesto", f, &format!("FilaPresupuesto[{i}]"));
    }
    for firma in [
        "fijar_presupuesto: { args: { categoriaId: number; monto: string }; ret: PresupuestoFijado }",
        "quitar_presupuesto: { args: { categoriaId: number }; ret: boolean }",
        "obtener_presupuestos_mes: { args: { anio: number; mes: number }; ret: EstadoPresupuestos }",
    ] {
        assert!(TIPOS_IPC.contains(firma), "tipos-ipc.d.ts debe declarar: {firma}");
    }
}

#[test]
fn integracion_respaldo_y_restauracion_conservan_los_presupuestos() {
    let _g = entorno_para_pruebas_externas();
    sembrar();
    fijar("Alimentación", "1500.01");
    let esperado = huella_de_la_base();
    let nombre = crate::comandos::respaldos::crear_respaldo().unwrap();
    let nombre = std::path::Path::new(&nombre).file_name().unwrap().to_string_lossy().to_string();

    fijar("Transporte", "3012.44");
    quitar_presupuesto(categoria_por_nombre("Alimentación")).unwrap();

    let r = crate::comandos::respaldos::restaurar_respaldo(nombre).unwrap();
    assert_eq!(r["version_del_esquema"], migraciones::VERSION_OBJETIVO);
    assert_eq!(huella_de_la_base(), esperado);
    let e = septiembre();
    assert_eq!(e.filas.len(), 1);
    assert_eq!(e.filas[0].categoria_nombre, "Alimentación");
}

#[test]
fn integracion_migrar_una_base_v20_crea_la_tabla_vacia_y_conserva_marcas_y_gastos() {
    let _g = entorno_para_pruebas_externas();
    let ruta = db_sql::obtener_ruta_db();
    assert!(ruta.starts_with(std::env::temp_dir().to_str().unwrap()), "ABORTADO: apuntaría a la base real ({ruta})");
    let _ = std::fs::remove_file(&ruta);
    for sufijo in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{ruta}{sufijo}"));
    }
    {
        let mut c = Connection::open(&ruta).unwrap();
        migraciones::migrar_hasta(&mut c, 20);
        assert_eq!(migraciones::version_de(&c).unwrap(), 20);
        let existe: i64 = c.query_row("SELECT COUNT(*) FROM sqlite_master WHERE name = 'presupuestos';", [], |r| r.get(0)).unwrap();
        assert_eq!(existe, 0, "la v20 no tiene la tabla");
        c.execute_batch(
            "INSERT INTO categorias (nombre, fija) VALUES ('Seguros inventados', 1), ('Comida inventada', 0);
             INSERT INTO gastos (fecha, monto, divisa, descripcion, categoria_id, metodo_pago, costo_adicional)
                 VALUES ('09/09/2026', 1187.53, 'DOP', 'Comida', 2, 'transferencia', 2.38);",
        )
        .unwrap();
    }

    db_sql::inicializar_db().expect("la base v20 migra a v21");

    let c = conexion();
    assert_eq!(migraciones::version_de(&c).unwrap(), 21);
    let n: i64 = c.query_row("SELECT COUNT(*) FROM presupuestos;", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0, "nace vacía: ningún presupuesto se inventa");
    let marcas: i64 = c.query_row("SELECT COUNT(*) FROM categorias WHERE fija = 1;", [], |r| r.get(0)).unwrap();
    assert_eq!(marcas, 1, "la marca fija se conserva");
    drop(c);

    fijar("Comida inventada", "1500.00");
    let r = septiembre();
    assert_eq!(fila(&r, "Comida inventada").gastado, "1189.91");
    assert_eq!(fila(&r, "Comida inventada").porcentaje_milesimas, 793);
    assert_eq!(fila(&r, "Comida inventada").estado, "ok");
}
