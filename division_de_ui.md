# División de `ui.ts` por pestañas

Estado: **decidido el diseño B (limpio, con dependencias inyectadas y puente `window.appUI`)**, que sustituye al diseño mecánico de §2: ver [division_de_ui_limpia.md](division_de_ui_limpia.md), donde está modelado y medido (93 de 102 cuerpos a reescribir, unas 710 líneas). **PR 0 hecho en 1.51.0; PR 1 (`efectivo`) en 1.52.0; PR 2 (`cuentas`) en 1.53.0; PR 3 (`resumen`) en 1.54.0**, con el registro de lo previsto frente a lo medido en `division_de_ui_limpia.md` §9; el siguiente es `dashboard`. Las medidas de §1 y el orden de §3 siguen valiendo; las decisiones de §7 están respondidas: diseño B, de menor a mayor, y pruebas de interacción en Node (1.49.0).

Contexto: [fase_7_frontend.md](fase_7_frontend.md) (por qué dividir), [migracion_a_typescript.md](migracion_a_typescript.md) (cómo está el tipado).

## 1. Qué hay que dividir (medido)

| | |
|---|---|
| `src/js/ui.ts` | 4 704 líneas, una clase `AppUI` con **103 métodos** y una instancia global `appUI` |
| Estado de la instancia | `contentContainer`, `notifContainer`, `selectedGastosMonth`, `_menuPasivoAbort` |
| Manejadores en línea | **99** atributos `on…=` que llaman a **64** métodos de `appUI` (73 referencias) y a `navigate` (15) |
| De ellos, con `elemento()`/`buscar()` dentro del propio atributo | 12 |
| Estado del tipado | `ui.ts` comprueba con 0 errores; con `noImplicitAny` faltan **169** parámetros |

### La división casi se hace sola
Se calculó, con el grafo de llamadas entre métodos (no por el nombre), a qué pestaña pertenece cada método: el que solo es alcanzable desde el `render` de una pestaña es de esa pestaña.

**98 de los 103 métodos pertenecen a una sola pestaña. Solo 4 son compartidos** (89 líneas): `render`, `showToast`, `formatMoney` y `pedirMotivoDeCorreccion`.

| Pestaña | Métodos | Líneas |
|---|---|---|
| tarjetas | 17 | 837 |
| ajustes | 16 | 736 |
| préstamos | 20 | 705 |
| ingresos | 11 | 519 |
| gastos | 9 | 480 |
| suscripciones | 10 | 361 |
| capital | 7 | 294 |
| cuentas | 3 | 183 |
| resumen | 1 | 168 |
| dashboard | 1 | 160 |
| efectivo | 3 | 151 |
| **Compartido** | 4 | 89 |

Dos medidas más que definen el riesgo de **mover** un método:

* **0 métodos pasados como valor** (`addEventListener('x', this.metodo)`, `.map(this.metodo)`), 0 `bind`/`call`/`apply`: no hay forma de que un `this` se pierda al cambiar de sitio un método. Todo uso es `this.metodo(...)`.
* **0** funciones clásicas con `this` dentro de la clase; las 15 flechas con `this` lo conservan.

## 2. Diseño

**Cada vista es un módulo con un objeto de métodos; `appUI` los mezcla.** Los métodos se mueven **sin cambiar una línea de su cuerpo**: siguen usando `this.showToast`, `this.formatMoney`… porque `this` es el objeto mezclado, que contiene los métodos de todas las vistas y los compartidos.

```ts
// src/js/vistas/efectivo.ts
export const vistaEfectivo = {
    async renderEfectivo() { /* igual que hoy */ },
    handleAgregarEfectivoInformal(e) { /* igual */ },
} satisfies Vista;            // y ThisType<AppUI> para que `this` esté tipado

// src/js/ui/appUI.ts (el «puente»)
export const appUI: AppUI = Object.assign(Object.create(null), estado, compartido, vistaEfectivo, /* … */);
(window as any).appUI = appUI;   // los onclick="appUI.x()" siguen funcionando
```

* `AppUI` es el tipo intersección de las vistas; los métodos declaran `ThisType<AppUI>`. Los tipos entre vistas se importan con `import type` (no crean ciclo en ejecución).
* **Los 99 manejadores en línea no se tocan**: `window.appUI` y `window.navigate` los mantienen vivos. Los 12 que usan `elemento()`/`buscar()` exigen que esas funciones sigan colgando de `window` (la prueba lo vigila).
* **Incremental:** durante la migración, `appUI` es la instancia de la clase vieja **más** `Object.assign(appUI, vistaX)` de lo ya extraído. Cada PR saca una vista de la clase; la aplicación funciona entera tras cada uno. La clase desaparece con la última.
* **Módulos ES** (`<script type="module">`): comprobado en la aplicación empaquetada (import estático y dinámico, `await` de nivel superior, `__TAURI__` disponible). Los módulos se ejecutan diferidos, antes de `DOMContentLoaded`, y los manejadores en línea solo disparan tras cargar.
* Quedan como script clásico, por ahora, `api.ts` (`AppAPI` global); `dom` y `respaldos` pasan a módulos y, donde un manejador en línea los necesita, se exponen en `window`.

## 3. Orden

0. **Infraestructura** (sin mover ninguna vista): carpeta `vistas/`, módulo `compartido` (los 4 métodos), puente `appUI`, `app.ts` como módulo, helper de las pruebas que lee **todos** los fuentes de la interfaz (hoy leen `ui.ts`) y los `tsconfig`. Es el único PR que toca pruebas y `index.html`.
1. **De menor a mayor riesgo**: efectivo (151) → cuentas (183) → dashboard (160) → resumen (168) → capital (294) → suscripciones (361) → gastos (480) → ingresos (519) → préstamos (705) → ajustes (736) → tarjetas (837).
2. Retirar la clase `AppUI` vacía.

Son **13 PR**. Los de ajustes y tarjetas son los últimos por tamaño y por número de manejadores.

## 4. Qué comprueba cada PR (antes de fusionar)
1. `npm test` y `npm run tipos`.
2. **`manejadores.test.js`**: cada `appUI.metodo()` de un `onclick` sigue teniendo destino. Es la red contra lo único que un movimiento puede romper en silencio.
3. **`herramientas/comparar_vistas`**: las once pestañas, con tus datos reales, idénticas entre el commit anterior y el actual (salvo lo que el cambio deba modificar). Es la prueba de que se **pinta** igual.
4. **Los parámetros de la vista se tipan en el mismo PR** (`noImplicitAny`) y la vista se añade a `tsconfig.estricto.json`: cada extracción reduce los 169 pendientes y nace estricta.
5. **Pruebas de interacción** (`pruebas/js/interaccion`, 1.49.0): los 49 manejadores se ejecutan en Node y comprueban qué se envía a Rust; solo hay que adaptar `cargar_interfaz.js`. La revisión manual de la pestaña movida queda para lo que ninguna cubre: CSS, validación nativa y que los `onclick` interpolen bien.
6. Versión e historial, como siempre.

## 5. Lo que ya está hecho en 1.48.0
* **`manejadores.test.js`** (4 pruebas): hay decenas de manejadores; todo `appUI.x()` existe como método; `navigate` es una función global de `app.ts`; los manejadores solo llaman a destinos globales declarados (`appUI`, `navigate`, `elemento`, `buscar`, `String`, `Number`). Con mutaciones detecta un método renombrado, un objeto inexistente y la desaparición de `navigate`.
* **`herramientas/comparar_vistas/`**: `preparar.sh`, `mock.js`, `comparar.js` y una prueba `#[ignore]` que vuelca lo que leen las vistas. Probada contra la versión instalada: diez pestañas idénticas y la de Ajustes con solo el panel nuevo de restaurar. Trabaja sobre una copia temporal, solo lee la base viva y se limpia con un comando.

## 6. Riesgos y qué queda sin cubrir
* **Pintar ≠ pulsar.** La comparación no ejerce los formularios. Desde 1.49.0 lo cubren las **pruebas de interacción en Node** (sin navegador ni dependencias nuevas); queda sin cubrir lo que solo hace un navegador (CSS, `required`/`step`, interpolación de los `onclick`).
* **Las pruebas de contrato leen el texto de `ui.ts`** (`importes`, `tipos`, `manejadores`): hay que enseñarles a leer todas las vistas; se hace una vez, en el PR 0.
* **Orden de carga:** pasar a módulos cambia cuándo se ejecuta el código. Se comprobó para `dom`; el PR 0 lo vuelve a comprobar en la aplicación empaquetada con el `HOME` aislado.

## 7. Decisiones que se piden
1. **Diseño:** ¿objetos de métodos mezclados en `appUI` con `this` (propuesto) o clases/composición con inyección de dependencias? Lo propuesto es mecánico y reversible; lo segundo es más limpio pero obliga a reescribir cuerpos.
2. **Orden:** ¿de menor a mayor, como arriba?
3. **Pruebas de interacción:** ¿basta la revisión manual por pestaña, o aceptas una dependencia de desarrollo para pruebas de navegador?
