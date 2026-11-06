//! Casos de uso de los catálogos: crear, eliminar, renombrar, fusionar y marcar categorías, y crear y eliminar clientes.
//!
//! Orquestan las reglas de `dominio::catalogo` sobre el puerto `AlmacenCatalogos`. La transacción la
//! abre y confirma quien llama. El **orden** de las comprobaciones es el de los comandos que sustituyen
//! (se valida primero la entrada, después el duplicado o la guarda de borrado): lo fijan las pruebas de
//! caracterización `k1`–`k7`.
//!
//! **Renombrar y fusionar categorías.** Ninguna de las dos toca el esquema: la fusión reasigna
//! `gastos.categoria_id` (la única clave ajena hacia `categorias`) y elimina la de origen, todo en la transacción
//! que abre quien llama. Las categorías que el código busca **por nombre** (`dominio::catalogo::CATEGORIAS_DEL_SISTEMA`)
//! no se renombran ni se fusionan como origen. La unicidad del nombre se decide aquí, sin mayúsculas ni tildes,
//! porque la restricción `UNIQUE` de SQLite distingue mayúsculas.

use super::ErrorAplicacion;
use crate::dominio::catalogo;
use crate::dominio::errores::ErrorDominio;
use crate::puertos::repositorios::*;

/// Lo que la interfaz muestra antes de confirmar una fusión: nada se ha escrito.
#[derive(Debug, Clone, PartialEq)]
pub struct VistaPreviaFusion {
    pub origen_id: i64,
    pub origen_nombre: String,
    pub destino_id: i64,
    pub destino_nombre: String,
    /// Cuántos gastos se moverían.
    pub gastos: i64,
    /// Fecha más antigua y más reciente (`dd/mm/aaaa`) de esos gastos; `None` si no hay ninguno.
    pub desde: Option<String>,
    pub hasta: Option<String>,
    pub advertencias: Vec<String>,
    /// La marca «fija / recurrente» de cada una: la fusión conserva la del destino.
    pub origen_fija: bool,
    pub destino_fija: bool,
}

pub fn crear_categoria(
    nombre: &str,
    almacen: &mut impl AlmacenCatalogos,
) -> Result<CategoriaGuardada, ErrorAplicacion> {
    let nombre = catalogo::nombre_de_categoria(nombre)?;
    if almacen.categoria_existe(&nombre)? {
        return Err(ErrorDominio::CategoriaExistente.into());
    }
    let id = almacen.insertar_categoria(&nombre)?;
    Ok(CategoriaGuardada { id, nombre, fija: false })
}

pub fn eliminar_categoria(id: i64, almacen: &mut impl AlmacenCatalogos) -> Result<(), ErrorAplicacion> {
    let nombre = almacen.nombre_de_categoria(id)?;
    catalogo::puede_eliminarse_categoria(&nombre, almacen.gastos_de_categoria(id)?)?;
    almacen.eliminar_categoria(id)?;
    Ok(())
}

/// Marca o desmarca una categoría como gasto fijo/recurrente. Solo cambia esa marca: no toca ningún gasto, saldo ni
/// importe. Una categoría inexistente es un error de almacén (`NoEncontrado`), no un cambio silencioso.
pub fn marcar_categoria_fija(
    id: i64,
    fija: bool,
    almacen: &mut impl AlmacenCatalogos,
) -> Result<CategoriaGuardada, ErrorAplicacion> {
    let nombre = almacen.nombre_de_categoria(id)?;
    almacen.marcar_categoria_fija(id, fija)?;
    Ok(CategoriaGuardada { id, nombre, fija })
}

/// Cambia el nombre de una categoría. Devuelve la categoría con su nombre final (recortado). Renombrarla a su mismo
/// nombre no escribe nada; cambiarle solo las mayúsculas o las tildes sí. **No toca la marca «fija / recurrente»**.
pub fn renombrar_categoria(
    id: i64,
    nombre: &str,
    almacen: &mut impl AlmacenCatalogos,
) -> Result<CategoriaGuardada, ErrorAplicacion> {
    let todas = almacen.categorias()?;
    let (actual, fija) = todas
        .iter()
        .find(|c| c.id == id)
        .map(|c| (c.nombre.clone(), c.fija))
        .ok_or(ErrorAlmacen::NoEncontrado { entidad: "la categoría", id })?;
    catalogo::puede_renombrarse_categoria(&actual)?;
    let otros: Vec<String> = todas.into_iter().filter(|c| c.id != id).map(|c| c.nombre).collect();
    let nuevo = catalogo::nombre_para_renombrar(nombre, &otros)?;
    if nuevo != actual {
        almacen.renombrar_categoria(id, &nuevo)?;
    }
    Ok(CategoriaGuardada { id, nombre: nuevo, fija })
}

/// Las reglas comunes de la vista previa y de la fusión: devuelve los nombres de origen y destino.
fn validar_fusion(
    origen_id: i64,
    destino_id: i64,
    almacen: &impl AlmacenCatalogos,
) -> Result<(String, String), ErrorAplicacion> {
    if origen_id == destino_id {
        return Err(ErrorDominio::CategoriasIguales.into());
    }
    let origen = almacen.nombre_de_categoria(origen_id)?;
    let destino = almacen.nombre_de_categoria(destino_id)?;
    catalogo::puede_fusionarse_categoria_como_origen(&origen)?;
    Ok((origen, destino))
}

/// Qué pasaría al fusionar `origen_id` en `destino_id`. **Solo lee**: aplica las mismas reglas que la fusión, de modo
/// que lo que se previsualiza y se rechaza es lo mismo que se confirma y se rechaza.
pub fn previsualizar_fusion_categorias(
    origen_id: i64,
    destino_id: i64,
    almacen: &impl AlmacenCatalogos,
) -> Result<VistaPreviaFusion, ErrorAplicacion> {
    let (origen_nombre, destino_nombre) = validar_fusion(origen_id, destino_id, almacen)?;
    let gastos = almacen.gastos_de_categoria(origen_id)?;
    let fechas = almacen.fechas_de_gastos_de_categoria(origen_id)?;
    let (desde, hasta) = match catalogo::rango_de_fechas(&fechas) {
        Some((d, h)) => (Some(d), Some(h)),
        None => (None, None),
    };
    let marca = |id: i64| -> Result<bool, ErrorAplicacion> {
        Ok(almacen.categorias()?.iter().find(|c| c.id == id).map(|c| c.fija).unwrap_or(false))
    };
    let (origen_fija, destino_fija) = (marca(origen_id)?, marca(destino_id)?);
    let nombre_de_marca = |fija: bool| if fija { "fija / recurrente" } else { "variable" };
    let mut advertencias = Vec::new();
    if origen_fija != destino_fija {
        advertencias.push(format!(
            "«{origen_nombre}» es {} y «{destino_nombre}» es {}: tras la fusión, los gastos tomarán la marca de «{destino_nombre}» ({}).",
            nombre_de_marca(origen_fija),
            nombre_de_marca(destino_fija),
            nombre_de_marca(destino_fija),
        ));
    }
    if gastos == 0 {
        advertencias.push(format!("«{origen_nombre}» no tiene gastos: solo se eliminará la categoría."));
    }
    Ok(VistaPreviaFusion {
        origen_id,
        origen_nombre,
        destino_id,
        destino_nombre,
        gastos,
        desde,
        hasta,
        advertencias,
        origen_fija,
        destino_fija,
    })
}

/// Fusiona `origen_id` en `destino_id`: reasigna **todos** sus gastos al destino, anota el caso de auditoría y elimina
/// la categoría de origen. Devuelve cuántos gastos se movieron. El caso se anota **antes** de mover y borrar (como en
/// las demás correcciones): si algo falla, quien abrió la transacción la deshace entera y no queda un caso huérfano,
/// ni gastos movidos con el origen vivo. El destino conserva su nombre **y su marca «fija / recurrente»**: la del origen
/// se va con él.
pub fn fusionar_categorias(
    origen_id: i64,
    destino_id: i64,
    almacen: &mut (impl AlmacenCatalogos + RegistroDeCorrecciones),
) -> Result<i64, ErrorAplicacion> {
    let (origen, destino) = validar_fusion(origen_id, destino_id, almacen)?;
    let gastos = almacen.gastos_de_categoria(origen_id)?;
    let motivo = crate::dominio::correccion::motivo_de_correccion(&catalogo::motivo_de_fusion(&origen, &destino, gastos))?;
    almacen.anotar_caso(&CasoAAnotar {
        tipo: "categoria".to_string(),
        // El origen se elimina: el caso apunta al destino, que sigue existiendo.
        referencia_id: destino_id,
        descripcion: catalogo::descripcion_de_fusion(&origen, &destino, gastos),
        importe: None,
        divisa: None,
        motivo,
        detalle: None,
    })?;
    let movidos = almacen.mover_gastos_de_categoria(origen_id, destino_id)?;
    if movidos != gastos {
        return Err(ErrorAlmacen::Fallo(format!(
            "se esperaba mover {gastos} gastos y se movieron {movidos}: no se elimina la categoría"
        ))
        .into());
    }
    almacen.eliminar_categoria(origen_id)?;
    Ok(movidos)
}

pub fn crear_cliente(
    rnc: &str,
    nombre: &str,
    almacen: &mut impl AlmacenCatalogos,
) -> Result<ClienteGuardado, ErrorAplicacion> {
    let (rnc, nombre) = catalogo::datos_de_cliente(rnc, nombre)?;
    if almacen.rnc_registrado(&rnc)? {
        return Err(ErrorDominio::ClienteDuplicado.into());
    }
    let id = almacen.insertar_cliente(&rnc, &nombre)?;
    Ok(ClienteGuardado { id, rnc, nombre })
}

pub fn eliminar_cliente(id: i64, almacen: &mut impl AlmacenCatalogos) -> Result<(), ErrorAplicacion> {
    catalogo::puede_eliminarse_cliente(almacen.facturas_de_cliente(id)?)?;
    almacen.eliminar_cliente(id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::puertos::dobles::CatalogosEnMemoria;

    fn almacen() -> CatalogosEnMemoria {
        CatalogosEnMemoria::nuevo().con_categoria("Otros").con_categoria("Comida")
    }

    fn dominio(e: ErrorAplicacion) -> ErrorDominio {
        match e {
            ErrorAplicacion::Dominio(d) => d,
            otro => panic!("se esperaba un error de dominio y fue {otro:?}"),
        }
    }

    #[test]
    fn crear_una_categoria_la_guarda_recortada_y_devuelve_su_id() {
        let mut a = almacen();
        let c = crear_categoria("  Mascotas ", &mut a).unwrap();
        assert_eq!(c.nombre, "Mascotas");
        assert_eq!(a.id_de_categoria("Mascotas"), c.id);
    }

    #[test]
    fn una_categoria_vacia_o_repetida_en_cualquier_caja_no_se_guarda() {
        let mut a = almacen();
        let antes = a.categorias.len();
        assert_eq!(dominio(crear_categoria("  ", &mut a).unwrap_err()), ErrorDominio::CategoriaSinNombre);
        for repetida in ["Comida", "comida", "COMIDA", " comida "] {
            assert_eq!(dominio(crear_categoria(repetida, &mut a).unwrap_err()), ErrorDominio::CategoriaExistente, "{repetida}");
        }
        assert_eq!(a.categorias.len(), antes);
    }

    #[test]
    fn eliminar_categoria_respeta_la_de_sistema_y_los_gastos() {
        let mut a = almacen();
        let otros = a.id_de_categoria("Otros");
        let comida = a.id_de_categoria("Comida");

        assert_eq!(dominio(eliminar_categoria(otros, &mut a).unwrap_err()), ErrorDominio::CategoriaDelSistema);
        // Con gastos, «Otros» sigue nombrándose como la de sistema: es la primera guarda.
        a.gastos_por_categoria.insert(otros, 3);
        assert_eq!(dominio(eliminar_categoria(otros, &mut a).unwrap_err()), ErrorDominio::CategoriaDelSistema);

        a.gastos_por_categoria.insert(comida, 1);
        assert_eq!(dominio(eliminar_categoria(comida, &mut a).unwrap_err()), ErrorDominio::CategoriaConGastos);
        assert_eq!(a.categorias.len(), 2, "no se borró nada");

        a.gastos_por_categoria.insert(comida, 0);
        eliminar_categoria(comida, &mut a).unwrap();
        assert_eq!(a.categorias.len(), 1);
    }

    #[test]
    fn eliminar_una_categoria_inexistente_es_un_error_de_almacen_y_no_borra() {
        let mut a = almacen();
        let e = eliminar_categoria(999, &mut a).unwrap_err();
        assert!(matches!(e, ErrorAplicacion::Almacen(ErrorAlmacen::NoEncontrado { id: 999, .. })), "{e:?}");
        assert_eq!(a.categorias.len(), 2);
    }

    #[test]
    fn una_categoria_nueva_nace_variable() {
        let mut a = almacen();
        let c = crear_categoria("Mascotas", &mut a).unwrap();
        assert!(!c.fija);
        assert!(a.categorias.iter().all(|c| !c.fija));
    }

    #[test]
    fn marcar_y_desmarcar_una_categoria_solo_cambia_esa() {
        let mut a = almacen();
        let comida = a.id_de_categoria("Comida");
        let otros = a.id_de_categoria("Otros");

        let c = marcar_categoria_fija(comida, true, &mut a).unwrap();
        assert_eq!((c.id, c.nombre.as_str(), c.fija), (comida, "Comida", true));
        let marcas: Vec<(String, bool)> = a.categorias.iter().map(|c| (c.nombre.clone(), c.fija)).collect();
        assert_eq!(marcas, vec![("Otros".to_string(), false), ("Comida".to_string(), true)]);

        // Marcar dos veces es lo mismo que una.
        marcar_categoria_fija(comida, true, &mut a).unwrap();
        assert!(a.categorias.iter().find(|c| c.id == comida).unwrap().fija);

        let c = marcar_categoria_fija(comida, false, &mut a).unwrap();
        assert!(!c.fija);
        assert!(a.categorias.iter().all(|c| !c.fija));
        // La de sistema también puede marcarse: la marca es del titular.
        marcar_categoria_fija(otros, true, &mut a).unwrap();
        assert!(a.categorias.iter().find(|c| c.id == otros).unwrap().fija);
    }

    #[test]
    fn marcar_una_categoria_inexistente_es_un_error_legible_y_no_cambia_nada() {
        let mut a = almacen();
        let e = marcar_categoria_fija(999, true, &mut a).unwrap_err();
        assert!(matches!(e, ErrorAplicacion::Almacen(ErrorAlmacen::NoEncontrado { id: 999, .. })), "{e:?}");
        assert!(!e.to_string().is_empty());
        assert!(a.categorias.iter().all(|c| !c.fija));
    }

    // --- renombrar ---

    fn nombre_de(a: &CatalogosEnMemoria, id: i64) -> String {
        a.nombre_de_categoria(id).unwrap()
    }

    #[test]
    fn renombrar_guarda_el_nombre_recortado_y_conserva_el_id() {
        let mut a = almacen();
        let comida = a.id_de_categoria("Comida");
        let c = renombrar_categoria(comida, "  Alimentos  ", &mut a).unwrap();
        assert_eq!((c.id, c.nombre.as_str()), (comida, "Alimentos"));
        assert_eq!(nombre_de(&a, comida), "Alimentos");
        assert_eq!(a.categorias.len(), 2);
    }

    #[test]
    fn renombrar_a_si_misma_con_otra_capitalizacion_o_tildes_se_permite() {
        let mut a = CatalogosEnMemoria::nuevo().con_categoria("Alimentacion").con_categoria("Otros");
        let id = a.id_de_categoria("Alimentacion");
        renombrar_categoria(id, "ALIMENTACIÓN", &mut a).unwrap();
        assert_eq!(nombre_de(&a, id), "ALIMENTACIÓN");
        renombrar_categoria(id, "alimentación", &mut a).unwrap();
        assert_eq!(nombre_de(&a, id), "alimentación");
    }

    #[test]
    fn renombrar_con_el_mismo_nombre_no_escribe() {
        let mut a = almacen();
        a.fallar_en = Some("renombrar_categoria");
        let comida = a.id_de_categoria("Comida");
        // Si escribiera, el fallo inyectado lo delataría.
        assert_eq!(renombrar_categoria(comida, " Comida ", &mut a).unwrap().nombre, "Comida");
    }

    #[test]
    fn renombrar_rechaza_vacio_largo_reservado_y_duplicados_sin_cambiar_nada() {
        let mut a = almacen().con_categoria("Alimentación");
        let comida = a.id_de_categoria("Comida");
        assert_eq!(dominio(renombrar_categoria(comida, "   ", &mut a).unwrap_err()), ErrorDominio::CategoriaSinNombre);
        assert_eq!(
            dominio(renombrar_categoria(comida, &"x".repeat(61), &mut a).unwrap_err()),
            ErrorDominio::CategoriaNombreLargo { maximo: 60 }
        );
        assert!(matches!(
            dominio(renombrar_categoria(comida, "Cargos derivados", &mut a).unwrap_err()),
            ErrorDominio::CategoriaNombreReservado { .. }
        ));
        for repetida in ["alimentacion", "ALIMENTACIÓN", " alimentación "] {
            assert_eq!(dominio(renombrar_categoria(comida, repetida, &mut a).unwrap_err()), ErrorDominio::CategoriaExistente, "{repetida}");
        }
        // «Otros» ya existe (y es la de sistema): nadie más puede llamarse así, ni con otra capitalización.
        assert_eq!(dominio(renombrar_categoria(comida, "OTROS", &mut a).unwrap_err()), ErrorDominio::CategoriaExistente);
        assert_eq!(nombre_de(&a, comida), "Comida");
    }

    #[test]
    fn renombrar_una_categoria_del_sistema_se_rechaza_y_dice_para_que_la_usa() {
        let mut a = almacen().con_categoria("Suscripciones").con_categoria("Impuestos");
        for nombre in ["Otros", "Suscripciones", "Impuestos"] {
            let id = a.id_de_categoria(nombre);
            let e = dominio(renombrar_categoria(id, "Cualquiera", &mut a).unwrap_err());
            assert!(matches!(e, ErrorDominio::CategoriaUsadaPorElSistema { accion: "renombrar", .. }), "{nombre}");
            assert!(e.to_string().contains("el sistema la usa para"), "{e}");
            assert_eq!(nombre_de(&a, id), nombre, "no se renombró");
        }
    }

    #[test]
    fn renombrar_una_categoria_inexistente_es_un_error_de_almacen() {
        let mut a = almacen();
        let e = renombrar_categoria(999, "Nueva", &mut a).unwrap_err();
        assert!(matches!(e, ErrorAplicacion::Almacen(ErrorAlmacen::NoEncontrado { id: 999, .. })), "{e:?}");
    }

    // --- fusionar ---

    fn con_renta_y_alquiler() -> CatalogosEnMemoria {
        almacen()
            .con_categoria("Renta")
            .con_categoria("Alquiler")
            .con_gastos("Renta", &["05/01/2027", "01/02/2026", "30/12/2026"])
            .con_gastos("Alquiler", &["15/03/2027"])
    }

    #[test]
    fn fusionar_mueve_todos_los_gastos_elimina_el_origen_y_anota_el_caso() {
        let mut a = con_renta_y_alquiler();
        let (renta, alquiler) = (a.id_de_categoria("Renta"), a.id_de_categoria("Alquiler"));

        let movidos = fusionar_categorias(renta, alquiler, &mut a).unwrap();

        assert_eq!(movidos, 3);
        assert!(a.categorias.iter().all(|c| c.id != renta), "el origen se eliminó");
        assert_eq!(a.gastos_de_categoria(alquiler).unwrap(), 4, "el destino suma los suyos y los del origen");
        assert_eq!(a.gastos_de_categoria(renta).unwrap(), 0);
        assert_eq!(nombre_de(&a, alquiler), "Alquiler", "el destino conserva su nombre");
    }

    #[test]
    fn la_fusion_deja_un_caso_de_auditoria_que_pasa_el_umbral_del_registro() {
        let mut a = con_renta_y_alquiler();
        let (renta, alquiler) = (a.id_de_categoria("Renta"), a.id_de_categoria("Alquiler"));
        fusionar_categorias(renta, alquiler, &mut a).unwrap();

        assert_eq!(a.casos.len(), 1);
        let c = &a.casos[0];
        assert_eq!(c.tipo, "categoria");
        assert_eq!(c.descripcion, "Fusión de «Renta» en «Alquiler» (3 gastos)");
        assert_eq!((c.importe, c.divisa.as_deref(), c.detalle.as_deref()), (None, None, None));
        assert_eq!(c.referencia_id, alquiler);
        assert!(crate::dominio::correccion::motivo_de_correccion(&c.motivo).is_ok(), "{}", c.motivo);
        assert!(c.motivo.contains("Renta") && c.motivo.contains("Alquiler"));
    }

    #[test]
    fn fusionar_una_categoria_sin_gastos_solo_la_elimina() {
        let mut a = almacen().con_categoria("Vacía").con_categoria("Alquiler").con_gastos("Alquiler", &["15/03/2027"]);
        let (vacia, alquiler) = (a.id_de_categoria("Vacía"), a.id_de_categoria("Alquiler"));
        assert_eq!(fusionar_categorias(vacia, alquiler, &mut a).unwrap(), 0);
        assert!(a.categorias.iter().all(|c| c.id != vacia));
        assert_eq!(a.gastos_de_categoria(alquiler).unwrap(), 1);
        assert_eq!(a.casos[0].descripcion, "Fusión de «Vacía» en «Alquiler» (0 gastos)");
    }

    #[test]
    fn fusionar_en_si_misma_o_con_una_inexistente_no_toca_nada() {
        let mut a = con_renta_y_alquiler();
        let (renta, alquiler) = (a.id_de_categoria("Renta"), a.id_de_categoria("Alquiler"));
        assert_eq!(dominio(fusionar_categorias(renta, renta, &mut a).unwrap_err()), ErrorDominio::CategoriasIguales);
        for (o, d) in [(999, alquiler), (renta, 999)] {
            let e = fusionar_categorias(o, d, &mut a).unwrap_err();
            assert!(matches!(e, ErrorAplicacion::Almacen(ErrorAlmacen::NoEncontrado { id: 999, .. })), "{e:?}");
        }
        assert_eq!(a.categorias.len(), 4);
        assert_eq!(a.gastos_de_categoria(renta).unwrap(), 3);
        assert!(a.casos.is_empty());
    }

    #[test]
    fn una_categoria_del_sistema_no_se_fusiona_como_origen_pero_si_como_destino() {
        let mut a = con_renta_y_alquiler().con_categoria("Suscripciones").con_categoria("Impuestos");
        let (otros, subs, imp, renta) = (
            a.id_de_categoria("Otros"), a.id_de_categoria("Suscripciones"), a.id_de_categoria("Impuestos"), a.id_de_categoria("Renta"),
        );
        for origen in [otros, subs, imp] {
            let e = dominio(fusionar_categorias(origen, renta, &mut a).unwrap_err());
            assert!(matches!(e, ErrorDominio::CategoriaUsadaPorElSistema { accion: "fusionar en otra", .. }), "{e:?}");
            assert!(e.to_string().contains("el sistema la usa para"), "{e}");
        }
        assert_eq!(a.categorias.len(), 6);
        assert!(a.casos.is_empty());

        // Como destino, «Otros» recibe los gastos de «Renta».
        assert_eq!(fusionar_categorias(renta, otros, &mut a).unwrap(), 3);
        assert_eq!(a.gastos_de_categoria(otros).unwrap(), 3);
        assert_eq!(nombre_de(&a, otros), "Otros");
    }

    #[test]
    fn un_fallo_a_medias_no_deja_nada_a_medias() {
        for punto in ["anotar_caso", "mover_gastos_de_categoria", "eliminar_categoria"] {
            let mut a = con_renta_y_alquiler();
            let (renta, alquiler) = (a.id_de_categoria("Renta"), a.id_de_categoria("Alquiler"));
            a.fallar_en = Some(punto);

            let e = a.en_transaccion(|a| fusionar_categorias(renta, alquiler, a)).unwrap_err();

            assert!(matches!(e, ErrorAplicacion::Almacen(ErrorAlmacen::Fallo(_))), "{punto}: {e:?}");
            assert_eq!(a.categorias.len(), 4, "{punto}: la categoría sigue");
            assert_eq!(a.gastos_de_categoria(renta).unwrap(), 3, "{punto}: sus gastos siguen en el origen");
            assert_eq!(a.gastos_de_categoria(alquiler).unwrap(), 1, "{punto}: el destino no recibió nada");
            assert!(a.casos.is_empty(), "{punto}: sin caso huérfano");
        }
    }

    // --- vista previa ---

    #[test]
    fn la_vista_previa_cuenta_los_gastos_da_el_rango_y_no_escribe() {
        let a = con_renta_y_alquiler();
        let (renta, alquiler) = (a.id_de_categoria("Renta"), a.id_de_categoria("Alquiler"));
        let antes = a.clone();

        let v = previsualizar_fusion_categorias(renta, alquiler, &a).unwrap();

        assert_eq!((v.origen_nombre.as_str(), v.destino_nombre.as_str(), v.gastos), ("Renta", "Alquiler", 3));
        assert_eq!((v.desde.as_deref(), v.hasta.as_deref()), (Some("01/02/2026"), Some("05/01/2027")));
        assert!(v.advertencias.is_empty());
        // Solo recibe `&`: no pudo escribir; además el estado es idéntico.
        assert_eq!(a.categorias, antes.categorias);
        assert_eq!(a.gastos_por_categoria, antes.gastos_por_categoria);
        assert!(a.casos.is_empty());
    }

    #[test]
    fn la_vista_previa_sin_gastos_lo_advierte_y_no_da_rango() {
        let a = almacen().con_categoria("Vacía");
        let v = previsualizar_fusion_categorias(a.id_de_categoria("Vacía"), a.id_de_categoria("Comida"), &a).unwrap();
        assert_eq!((v.gastos, v.desde, v.hasta), (0, None, None));
        assert_eq!(v.advertencias, vec!["«Vacía» no tiene gastos: solo se eliminará la categoría.".to_string()]);
    }

    #[test]
    fn la_vista_previa_rechaza_lo_mismo_que_la_fusion() {
        let a = con_renta_y_alquiler();
        let (otros, renta) = (a.id_de_categoria("Otros"), a.id_de_categoria("Renta"));
        assert_eq!(dominio(previsualizar_fusion_categorias(renta, renta, &a).unwrap_err()), ErrorDominio::CategoriasIguales);
        assert!(matches!(
            dominio(previsualizar_fusion_categorias(otros, renta, &a).unwrap_err()),
            ErrorDominio::CategoriaUsadaPorElSistema { .. }
        ));
        assert!(previsualizar_fusion_categorias(999, renta, &a).is_err());
    }

    // --- hallazgos (documentados, no corregidos: protocolo del proyecto) ---

    /// HALLAZGO: `eliminar_categoria` solo protege «Otros». «Suscripciones» e «Impuestos», que el código también
    /// busca por nombre, se eliminan si no tienen gastos (la interfaz esconde el botón de «Suscripciones», pero el
    /// comando no lo impide). Consecuencia acotada: los cargos recurrentes caen en «Otros» o recrean «Suscripciones»;
    /// sin «Impuestos» se pierden la exención y la comisión fija hasta crearla de nuevo. No se corrige aquí.
    #[test]
    fn hallazgo_eliminar_categoria_no_protege_suscripciones_ni_impuestos() {
        let mut a = almacen().con_categoria("Suscripciones").con_categoria("Impuestos");
        for nombre in ["Suscripciones", "Impuestos"] {
            let id = a.id_de_categoria(nombre);
            assert!(eliminar_categoria(id, &mut a).is_ok(), "hoy se elimina {nombre}");
        }
        assert_eq!(dominio(eliminar_categoria(a.id_de_categoria("Otros"), &mut a).unwrap_err()), ErrorDominio::CategoriaDelSistema);
    }

    /// HALLAZGO: `crear_categoria` no tiene longitud máxima ni rechaza los nombres reservados («Cargos derivados»,
    /// «Sin categoría»); solo `renombrar_categoria` lo hace. No se corrige aquí.
    #[test]
    fn hallazgo_crear_categoria_no_tiene_longitud_maxima_ni_nombres_reservados() {
        let mut a = almacen();
        assert!(crear_categoria(&"x".repeat(200), &mut a).is_ok());
        assert!(crear_categoria("Cargos derivados", &mut a).is_ok());
    }

    /// HALLAZGO: la unicidad de `crear_categoria` ignora mayúsculas pero **no tildes** («Alimentación» y
    /// «Alimentacion» conviven); la de `renombrar_categoria` ignora ambas. No se corrige aquí.
    #[test]
    fn hallazgo_crear_categoria_distingue_tildes() {
        let mut a = almacen().con_categoria("Alimentación");
        assert!(crear_categoria("Alimentacion", &mut a).is_ok());
    }

    // --- la marca «fija / recurrente» ---

    fn marcar(a: &mut CatalogosEnMemoria, nombre: &str, fija: bool) {
        let id = a.id_de_categoria(nombre);
        a.categorias.iter_mut().find(|c| c.id == id).unwrap().fija = fija;
    }

    fn marca_de(a: &CatalogosEnMemoria, nombre: &str) -> bool {
        a.categorias.iter().find(|c| c.nombre == nombre).unwrap().fija
    }

    #[test]
    fn renombrar_no_toca_la_marca_fija() {
        let mut a = almacen();
        marcar(&mut a, "Comida", true);
        let comida = a.id_de_categoria("Comida");
        let c = renombrar_categoria(comida, "Alimentos", &mut a).unwrap();
        assert!(c.fija, "el resultado trae la marca que tenía");
        assert!(marca_de(&a, "Alimentos"));
        marcar(&mut a, "Alimentos", false);
        assert!(!renombrar_categoria(comida, "Comida", &mut a).unwrap().fija);
    }

    #[test]
    fn la_fusion_conserva_la_marca_del_destino_sea_cual_sea_la_del_origen() {
        for (origen_fija, destino_fija) in [(true, false), (false, true), (true, true), (false, false)] {
            let mut a = con_renta_y_alquiler();
            marcar(&mut a, "Renta", origen_fija);
            marcar(&mut a, "Alquiler", destino_fija);
            let (renta, alquiler) = (a.id_de_categoria("Renta"), a.id_de_categoria("Alquiler"));
            fusionar_categorias(renta, alquiler, &mut a).unwrap();
            assert_eq!(marca_de(&a, "Alquiler"), destino_fija, "origen {origen_fija}, destino {destino_fija}");
        }
    }

    #[test]
    fn la_vista_previa_advierte_solo_si_origen_y_destino_difieren_en_la_marca() {
        let mut a = con_renta_y_alquiler();
        let (renta, alquiler) = (a.id_de_categoria("Renta"), a.id_de_categoria("Alquiler"));

        let v = previsualizar_fusion_categorias(renta, alquiler, &a).unwrap();
        assert!(v.advertencias.is_empty());
        assert_eq!((v.origen_fija, v.destino_fija), (false, false));

        marcar(&mut a, "Renta", true);
        let v = previsualizar_fusion_categorias(renta, alquiler, &a).unwrap();
        assert_eq!((v.origen_fija, v.destino_fija), (true, false));
        assert_eq!(
            v.advertencias,
            vec!["«Renta» es fija / recurrente y «Alquiler» es variable: tras la fusión, los gastos tomarán la marca de «Alquiler» (variable).".to_string()]
        );

        marcar(&mut a, "Alquiler", true);
        marcar(&mut a, "Renta", false);
        let v = previsualizar_fusion_categorias(renta, alquiler, &a).unwrap();
        assert!(v.advertencias[0].contains("«Renta» es variable y «Alquiler» es fija / recurrente"), "{:?}", v.advertencias);

        marcar(&mut a, "Renta", true);
        assert!(previsualizar_fusion_categorias(renta, alquiler, &a).unwrap().advertencias.is_empty(), "iguales: sin aviso");
    }

    #[test]
    fn crear_un_cliente_lo_guarda_recortado() {
        let mut a = almacen();
        let c = crear_cliente(" 101 ", " Acme ", &mut a).unwrap();
        assert_eq!((c.rnc.as_str(), c.nombre.as_str()), ("101", "Acme"));
        assert_eq!(a.clientes.len(), 1);
    }

    #[test]
    fn un_cliente_sin_datos_o_con_rnc_repetido_no_se_guarda() {
        let mut a = almacen();
        for (r, n) in [("", "x"), ("x", ""), (" ", " ")] {
            assert_eq!(dominio(crear_cliente(r, n, &mut a).unwrap_err()), ErrorDominio::ClienteSinDatos);
        }
        crear_cliente("101", "Acme", &mut a).unwrap();
        assert_eq!(dominio(crear_cliente(" 101 ", "Otro", &mut a).unwrap_err()), ErrorDominio::ClienteDuplicado);
        assert_eq!(a.clientes.len(), 1);
    }

    #[test]
    fn eliminar_un_cliente_con_facturas_no_lo_borra() {
        let mut a = almacen();
        let id = crear_cliente("101", "Acme", &mut a).unwrap().id;
        a.facturas_por_cliente.insert(id, 2);
        assert_eq!(dominio(eliminar_cliente(id, &mut a).unwrap_err()), ErrorDominio::ClienteConFacturas);
        assert_eq!(a.clientes.len(), 1);

        a.facturas_por_cliente.insert(id, 0);
        eliminar_cliente(id, &mut a).unwrap();
        assert!(a.clientes.is_empty());
    }
}
