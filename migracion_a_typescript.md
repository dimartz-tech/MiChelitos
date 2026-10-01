# Migración del frontend a TypeScript

Estado: **1.44.0** infraestructura y conversión de todos los archivos; **1.46.0** `ui.ts` ya se comprueba (ayudante de DOM, sin `@ts-nocheck`). Queda tipar los parámetros (`noImplicitAny`) y dividir `ui.ts`. Decisión del titular el 2026-09-30: ir a TypeScript con solo `tsc` y **empezar ya**, porque migrar es más caro cuanto más código nuevo se escribe en JavaScript.

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

### Lo que sigue
1. **Tipar los parámetros y activar `noImplicitAny`.** Con la opción encendida hay **303 errores**: 134 en `api.ts` (los parámetros de los envoltorios) y 169 en `ui.ts` (parámetros de los métodos). Son casi todos TS7006, «parámetro sin tipo». Los de `api.ts` se pueden derivar del mapa `Comandos`; los de `ui.ts` son trabajo a mano y conviene hacerlo **a la vez que se divide**, pestaña a pestaña.
2. **Dividir `ui.ts` por pestañas** (Fase 7). Con el archivo ya comprobado, cada vista extraída nace tipada y cada PR comprueba que no rompe nada.
3. **Opcional**: pasar las pruebas de `pruebas/js/` a TypeScript (hoy son JavaScript escrito a mano que lee lo compilado).

## Costes y riesgos asumidos
* Hay un **paso de compilación** y archivos generados junto a los fuentes (ignorados). Se eligió compilar en el sitio, y no a una carpeta `dist/`, para no duplicar `index.html`, `css` y `assets` ni tocar `distDir`; el coste es que `src/js` mezcla `.ts` y `.js` generados.
* **Olvidarse de compilar antes de empaquetar con `cargo`** deja una app sin JavaScript. Por eso el hook de `tauri build` y este aviso.
* Node 22 o superior para el patrón de archivos de `node --test`.
