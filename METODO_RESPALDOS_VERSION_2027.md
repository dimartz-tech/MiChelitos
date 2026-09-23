# Método de copias de seguridad para la versión 2027

Estado: método acordado, parcialmente implementado en una rama independiente y **no activo en `main`**. Esta nota no cambia el mecanismo vigente ni autoriza importar datos de la instalación anterior.

## Cadencia y destinos

| Copia | Momento | Destino | Retención |
|---|---|---|---|
| Diaria | Primera apertura del día local; reintento tras fallo | Carpeta sincronizada de OneDrive o iCloud Drive elegida expresamente | Al menos 366 períodos de 24 horas |
| Mensual | Primera apertura del mes local | Carpeta local independiente | Al menos hasta verificar el cierre anual; plazo posterior por definir |
| Cierre anual | Tras el 31 de diciembre, antes de la primera operación del año nuevo | Copia local y copia en carpeta sincronizada | Sin borrado automático |

No se promete una copia de un día en que la aplicación no se abrió. «Entregado a carpeta sincronizada» no significa que la nube terminó de recibirla. Si ya hubo operaciones del año nuevo, el estado actual no puede etiquetarse como cierre exacto del 31 de diciembre.

## Contenido y protección

Cada paquete contendrá una instantánea consistente de **toda la SQLite de la versión 2027**, el JSON vigente de patrimonio y un manifiesto con identidad de aplicación, versión de esquema, fecha, tipo, archivos, tamaños y huellas. Se excluyen credenciales, registros de depuración, rutas personales, temporales y archivos de la versión anterior. La SQLite se captura mediante su API de respaldo, no copiando el archivo activo.

El paquete se cifra y autentica antes de entrar en la carpeta sincronizada. La identidad X25519 se guarda en el Llavero de macOS; la persona recibe una clave de recuperación una vez y debe confirmar su custodia antes de activar copias automáticas. Ninguna clave se versiona ni se incluye dentro del paquete.

## Retención y resumen

Un diario solo vence después de **más** de 366 × 24 horas. Se resume un mes únicamente cuando hasta su último diario venció: se conserva ese último paquete completo y restaurable más un resumen técnico cifrado de identificadores, fechas, versiones y huellas. Un resumen solo no sirve para restaurar. El cierre anual queda aparte y no se elimina automáticamente.

La consolidación no implica borrado. Antes de retirar diarios se verifica el paquete conservado y se ensaya su restauración. En OneDrive/iCloud no habrá limpieza automática hasta confirmar de forma fiable la sincronización o recibir autorización expresa tras comprobarla. Nunca se recorren o borran archivos ajenos al formato y carpeta administrados.

## Restauración

Se selecciona un paquete y, si el Llavero no está disponible, se aporta la clave de recuperación. La aplicación descifra en una carpeta temporal nueva, comprueba autenticidad, integridad, identidad, esquema, SQLite, relaciones y JSON, y muestra una vista previa no sensible. Tras confirmación expresa, crea un punto de retorno, cierra conexiones y sustituye SQLite y JSON con posibilidad de rollback. Un fallo o un paquete ajeno, futuro o alterado deja intacto el estado vigente.

## Relación con `main` y condición de adopción

El mecanismo actual en [src-tauri/src/respaldo.rs](src-tauri/src/respaldo.rs) crea copias SQLite antes de cambios de esquema y conserva hasta diez: **sigue siendo el comportamiento operativo de `main`**. Esta nota no lo reemplaza ni altera su retención.

En la rama aislada `refactor/centavos-y-verticales`, los commits `7237c4c` y `b869621` contienen componentes de captura, cifrado, custodia, entrega local, calendario y resumen probados con datos sintéticos. Aún no están conectados al arranque ni a una carpeta real de nube.

Antes de adoptar el método 2027 faltan: bloqueo común de escrituras SQL/JSON, alta y confirmación de clave, estado independiente por destino, cierre anual antes de escrituras nuevas, restauración con rollback probado, ensayos de fallo y primer arranque, pruebas de accesibilidad y proveedor, y revisión de dependencias criptográficas. No se mezclan bases ni rutas de ambas versiones.
