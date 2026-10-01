# Comparar vistas con datos reales

Comprueba que **dos versiones del frontend pintan lo mismo** con tus datos de verdad: las once pestañas, una al lado de la otra, sin tocar la base viva ni mostrar importes. Existe para la división de `ui.ts` (ver `division_de_ui.md`): mover código sin romper nada solo se demuestra con datos reales, porque con datos de juguete casi todas las ramas de una vista quedan sin ejecutar.

## Cómo se usa

```bash
herramientas/comparar_vistas/preparar.sh <referencia-git-anterior>
```

`<referencia-git-anterior>` es el commit con el frontend «de antes» (por ejemplo `origin/main`, o el commit previo a la extracción). El script, **todo en `/tmp/comparar-vistas`**:

1. copia la base con `VACUUM INTO` (solo **lee** la viva) y el `capital.json`;
2. vuelca con el Rust actual lo que leen las vistas, sobre la copia (`volcado_de_datos_para_comparar_vistas`, una prueba `#[ignore]` de `caracterizacion.rs`);
3. construye el frontend anterior desde un `git worktree` de esa referencia y el actual desde el árbol de trabajo;
4. inserta `mock.js` (sustituye al puente de Tauri y responde con el volcado) en las dos copias.

Después, el servidor local y la comparación:

```bash
(cd /tmp/comparar-vistas && python3 -m http.server 8770 --bind 127.0.0.1)
# abre http://127.0.0.1:8770/comparar.html y pega comparar.js en la consola del navegador
```

`comparar.js` devuelve **solo un resumen**: por pestaña, «idéntica» o cuántas líneas difieren (con los dígitos enmascarados), el texto con `undefined`/`NaN`/`[object`, los errores de consola y los comandos que ninguna copia sabía contestar.

## Cómo leer el resultado
* **Idéntica** en todo salvo lo que el cambio debía modificar: bien.
* **`textoDefectuoso` o `errores` no vacíos**, o un **comando desconocido**: hay algo roto; mirar antes de fusionar.
* Una diferencia esperada (un panel nuevo) se reconoce por sus líneas; cualquier otra, no.

## Protección de datos
* La carpeta de trabajo contiene **una copia de tus datos**. El repositorio solo contiene código.
* El servidor escucha **solo en 127.0.0.1**.
* **Al terminar: `herramientas/comparar_vistas/preparar.sh limpiar`** borra la carpeta, incluida la copia.
* No pegues en ningún sitio el contenido de `volcado.json`; el resumen de `comparar.js` es lo único que se comparte.

## Qué no cubre
Comprueba **lo que se pinta**, no lo que ocurre al pulsar. Los manejadores se vigilan con la prueba de contrato `manejadores.test.js` y, uno a uno, con la revisión manual de cada pestaña en la aplicación.
