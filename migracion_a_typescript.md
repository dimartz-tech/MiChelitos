# Migración del frontend a TypeScript

Estado: **primer paso hecho en 1.44.0** (infraestructura y conversión de todos los archivos; los tipos se aprietan después). Decisión del titular el 2026-09-30: ir a TypeScript con solo `tsc` y **empezar ya**, porque migrar es más caro cuanto más código nuevo se escribe en JavaScript.

Contexto y comparación con JSDoc: [fase_7_frontend.md](fase_7_frontend.md), §4.

## Cómo queda

| | |
|---|---|
| Fuentes | `src/js/**/*.ts` (más `tipos-ipc.d.ts`, generado, y `globales.d.ts`) |
| Salida | `src/js/**/*.js`, junto a los fuentes, **ignorada por git**; es lo que empaqueta Tauri (`distDir` sigue siendo `src`) |
| Compilar | `npm run compilar` = `herramientas/limpiar_js_generado.mjs` + `tsc -p tsconfig.build.json` (≈1 s) |
| Al construir la app | `beforeBuildCommand` ejecuta `npm run compilar` |
| Pruebas de JavaScript | en `pruebas/js/` (fuera de `src`, **ya no se empaquetan con la app**); `npm test` compila y las ejecuta sobre lo compilado |
| Tipos de Rust | `src/js/tipos-ipc.d.ts`, generado de `main.rs` con `npm run tipos:generar`; una prueba exige que esté al día |

Sin empaquetador, sin dependencias de producción: la salida son módulos ES (`nucleo/dinero`) y scripts clásicos (`api`, `ui`, `app`, `respaldos`) normales.

### Regla
**Todo `.js` de `src/js/` es generado.** La limpieza previa a compilar los borra todos, de modo que un `.ts` eliminado no deja un `.js` huérfano que acabe empaquetado. El código nuevo del frontend se escribe en `.ts`.

### Flujo de desarrollo
* `npm run compilar` una vez, y `npm run vigilar` (`tsc --watch`) mientras se edita.
* **`cargo build` o `cargo run` directos no compilan el frontend**: Tauri empaqueta lo que haya en `src/`. Usa `npm run tauri build`, o `npm run compilar` antes.
* `cargo test` no se ve afectado.
* **`tauri dev` no se ha probado** con este esquema. La hipótesis es `compilar` + `vigilar` en otra terminal.

## Qué se comprobó
* `npm test` (30 pruebas) y `npm run tipos` pasan sobre lo compilado.
* Mutaciones: un error de tipos en `api.ts` detiene `npm test` en la compilación; un `.js` huérfano no sobrevive a `compilar`; un `.ts` nuevo con un tipo incorrecto falla `npm run tipos`.
* En un navegador con datos simulados, el JavaScript compilado de `ui` hace lo mismo que el anterior: las 11 pestañas pintan, el aviso de cobro muestra la fecha, el botón de restaurar llama a `restaurar_respaldo` con el nombre elegido; sin errores en consola.
* **En la aplicación empaquetada**, construida con `tauri build` **partiendo de cero `.js`** en el propio repositorio: el hook compiló, el binario incluyó los `.js` generados **aunque estén en `.gitignore`**, y `AppAPI`, `appUI`, `describirRespaldo` y `navigate` existían (comprobado con una marca temporal, revertida).

## La deuda, medida
`ui.ts` es TypeScript pero lleva `// @ts-nocheck`: se renombró tal cual (el historial de git conserva el archivo). Quitar esa línea da hoy **472 errores** con `strict` (y `noImplicitAny` apagado):

| Causa | Errores |
|---|---|
| `getElementById(...)` devuelve `HTMLElement \| null`: hay que aceptar el `null` | 143 + 41 |
| `.value`, `.checked`… sobre `HTMLElement` | 193 |
| `catch (err)`: `err` es `unknown` | 51 |
| Campos de la clase sin declarar (`contentContainer`, `_menuPasivoAbort`…) | 18 |
| Resto | ~20 |

Son casi todo **ergonomía del DOM**, no defectos: un ayudante `elemento<T extends HTMLElement>(id)` que devuelva el elemento o falle con un mensaje claro eliminaría de golpe la mayoría (los ~380 de las dos primeras filas).

### Orden para apretar los tipos
1. **Ayudante de DOM y de errores** en un módulo compartido (`ui/dom.ts`) y declarar los campos de la clase. Sin quitar el `@ts-nocheck` todavía.
2. **`api.ts`**: tipar los parámetros de los envoltorios (más de un centenar) y activar `noImplicitAny`.
3. **Dividir `ui.ts` por pestañas** (Fase 7): cada vista extraída nace **sin** `@ts-nocheck` y con tipos. El `@ts-nocheck` no se puede quitar «a medias» de un archivo, así que la extracción es el mecanismo: el archivo original se vacía y desaparece con la última vista.
4. **Opcional**: pasar las pruebas de `pruebas/js/` a TypeScript (hoy son JavaScript escrito a mano que lee lo compilado).

## Costes y riesgos asumidos
* Hay un **paso de compilación** y archivos generados junto a los fuentes (ignorados). Se eligió compilar en el sitio, y no a una carpeta `dist/`, para no duplicar `index.html`, `css` y `assets` ni tocar `distDir`; el coste es que `src/js` mezcla `.ts` y `.js` generados.
* **Olvidarse de compilar antes de empaquetar con `cargo`** deja una app sin JavaScript. Por eso el hook de `tauri build` y este aviso.
* Node 22 o superior para el patrón de archivos de `node --test`.
