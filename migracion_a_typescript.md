# Migración del frontend a TypeScript

Estado: **1.44.0** infraestructura y conversión de todos los archivos; **1.46.0** `ui.ts` ya se comprueba (ayudante de DOM, sin `@ts-nocheck`); **1.47.0** `api.ts` tiene todos sus parámetros tipados y se comprueba con `noImplicitAny`. Queda tipar los parámetros de `ui.ts` y dividirlo. Decisión del titular el 2026-09-30: ir a TypeScript con solo `tsc` y **empezar ya**, porque migrar es más caro cuanto más código nuevo se escribe en JavaScript.

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

## La deuda

### Paso 1, hecho (1.46.0): `ui.ts` ya se comprueba
`ui.ts` se renombró en 1.44.0 con `@ts-nocheck`: quitarlo daba **472 errores** con `strict`. Hoy **no lleva ninguna supresión y tiene 0 errores**. Lo que se hizo:

| Causa | Errores | Cómo se resolvió |
|---|---|---|
| `getElementById` devuelve `HTMLElement \| null` y se lee `.value`, `.style`… | 377 | Dos ayudantes en `src/js/ui/dom.ts`: `elemento<T>(id)` devuelve el elemento o **falla diciendo cuál falta**, y `buscar<T>(id)` conserva el `null` donde la ausencia es legítima. Se aplicaron a los 232 accesos (y a `app.ts`) con un cambio mecánico, demostrado equivalente: el JavaScript compilado es idéntico **byte a byte** una vez normalizados los dos ayudantes |
| `catch (err)`: `err` es `unknown` | 52 | `String(err)` en lugar de `err.toString()` (idéntico para errores y cadenas) |
| Campos de la clase sin declarar | 18 | Declarados, con su tipo |
| Resto (arrays sin tipo, `null` mal inferido, una resta de fechas, tres parámetros por defecto `= null`) | ~25 | Anotaciones y `?? 0` donde la comparación ya trataba `null` igual |

Al quitar el `@ts-nocheck` apareció además **un defecto real** (el panel «Casos de corrección» llamaba a un método de `AppAPI` que no existía: corregido en 1.45.0).

Reglas que lo conservan (`pruebas/js/contrato/dom.test.js`): ningún fuente del frontend usa `@ts-nocheck`, `@ts-ignore` ni `@ts-expect-error`; nadie llama a `document.getElementById` salvo `ui/dom.ts`; `index.html` carga el ayudante antes que la interfaz.

### Paso 2, hecho (1.47.0): los parámetros de `api.ts`
Los **61 envoltorios** de `api.ts` tienen todos sus parámetros tipados (134 errores de `noImplicitAny` → 0). Los tipos **no se escribieron a mano**: se derivaron del mapa `Comandos` generado desde Rust según cómo cada envoltorio pasa el parámetro al comando:

| Cómo viaja | Tipo del parámetro |
|---|---|
| `Number(x)`: acepta lo que escribió el titular | `number \| string` |
| `x === null \|\| x === '' ? null : Number(x)` | `number \| string \| null` |
| `String(x)` y los importes que Rust lee como texto | `string` / `string \| number` |
| Sin conversión (`x`, `x ?? y`, `x \|\| y`) | el tipo exacto de Rust, más `null \| undefined` si es opcional |
| Estructuras de entrada (`GastoInput`…) | el tipo generado |

Cuatro no se dedujeron solos y se resolvieron a mano; uno es `any` **explícito**: `guardarCapital`, porque Rust recibe el documento de capital como JSON libre (`Value`).

Qué se comprobó: el **JavaScript compilado de `api` es idéntico byte a byte** al de antes (los tipos no cambian nada); ninguna llamada de `ui.ts` incumplía los tipos nuevos; y tres mutaciones se detectan: un parámetro sin tipo, un tipo que la interfaz no cumple (el error aparece **en `ui.ts`**, en la llamada) y un envoltorio nuevo sin tipos.

`tsconfig.estricto.json` aplica `noImplicitAny` a lo que ya no tiene deuda (`api.ts`, `ui/dom.ts`, `nucleo/`, `tipos-ipc.d.ts`) y forma parte de `npm run compilar` y de `npm run tipos`: **una violación detiene también `tauri build`**.

### Lo que sigue
1. **Tipar los parámetros de `ui.ts`**: con `noImplicitAny` hay **169 errores**, todos de «parámetro sin tipo» en los métodos de la clase. Es trabajo a mano y conviene hacerlo **a la vez que se divide**, pestaña a pestaña: cada vista extraída nace estricta y se incorpora a `tsconfig.estricto.json`.
2. **Dividir `ui.ts` por pestañas** (Fase 7). Con el archivo comprobado y la API tipada, cada PR comprueba que no rompe nada.
3. **Opcional**: pasar las pruebas de `pruebas/js/` a TypeScript.

## Costes y riesgos asumidos
* Hay un **paso de compilación** y archivos generados junto a los fuentes (ignorados). Se eligió compilar en el sitio, y no a una carpeta `dist/`, para no duplicar `index.html`, `css` y `assets` ni tocar `distDir`; el coste es que `src/js` mezcla `.ts` y `.js` generados.
* **Olvidarse de compilar antes de empaquetar con `cargo`** deja una app sin JavaScript. Por eso el hook de `tauri build` y este aviso.
* Node 22 o superior para el patrón de archivos de `node --test`.
