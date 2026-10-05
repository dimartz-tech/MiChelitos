//! Casos de uso de los catálogos: crear y eliminar categorías y clientes.
//!
//! Orquestan las reglas de `dominio::catalogo` sobre el puerto `AlmacenCatalogos`. La transacción la
//! abre y confirma quien llama. El **orden** de las comprobaciones es el de los comandos que sustituyen
//! (se valida primero la entrada, después el duplicado o la guarda de borrado): lo fijan las pruebas de
//! caracterización `k1`–`k7`.

use super::ErrorAplicacion;
use crate::dominio::catalogo;
use crate::dominio::errores::ErrorDominio;
use crate::puertos::repositorios::*;

pub fn crear_categoria(
    nombre: &str,
    almacen: &mut impl AlmacenCatalogos,
) -> Result<CategoriaGuardada, ErrorAplicacion> {
    let nombre = catalogo::nombre_de_categoria(nombre)?;
    if almacen.categoria_existe(&nombre)? {
        return Err(ErrorDominio::CategoriaExistente.into());
    }
    let id = almacen.insertar_categoria(&nombre)?;
    Ok(CategoriaGuardada { id, nombre })
}

pub fn eliminar_categoria(id: i64, almacen: &mut impl AlmacenCatalogos) -> Result<(), ErrorAplicacion> {
    let nombre = almacen.nombre_de_categoria(id)?;
    catalogo::puede_eliminarse_categoria(&nombre, almacen.gastos_de_categoria(id)?)?;
    almacen.eliminar_categoria(id)?;
    Ok(())
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
