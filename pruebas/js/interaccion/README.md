# Pruebas de interacción (Node, sin navegador)

Comprueban **lo que ocurre al pulsar**: ejecutan los manejadores de `appUI` (los
que enlazan los atributos `onclick="appUI.x()"` / `onsubmit="appUI.x(event)"`)
con un DOM falso y una API falsa, y verifican **qué se envía a Rust** (método de
`AppAPI` y argumentos exactos) y **qué avisos (toasts) se muestran**.

Complementan, sin sustituirlas:

* `contrato/manejadores.test.js`: cada `appUI.x` de un atributo tiene destino.
* `contrato/importes.test.js`: los campos de importe tienen `step="0.01"`.
* `herramientas/comparar_vistas`: lo que se **pinta**, con datos reales.

Sin dependencias: `node:test`, `node:assert`, `new Function`. `npm test` compila
antes, y estas pruebas leen el JavaScript **compilado** (`src/js/*.js`).

## Piezas

| Archivo | Qué es |
|---|---|
| `ayudas/cargar_interfaz.js` | **Único punto de carga.** `cargarInterfaz({ campos, api, confirm, prompt })`. |
| `ayudas/dom_falso.js` | `document` mínimo: elementos por id, `select` con opciones, `remove()`, etc. |
| `ayudas/afirmar.js` | Afirmaciones comunes (`llamoUnaVez`, `noLlamoANada`, `rechazoSeMuestra`…). |
| `interaccion/*.test.js` | Una por pestaña o grupo. |
| `interaccion/ayudas.test.js` | Prueba el propio andamiaje. |
| `interaccion/cobertura.test.js` | Falla si un `handle*` no tiene prueba ni declaración. |

## Usar el helper

```js
import { cargarInterfaz, crearEvento, AUSENTE } from '../ayudas/cargar_interfaz.js';
import { llamoUnaVez, noLlamoANada, avisoExito, unAvisoDeError, redibujo, rechazoSeMuestra } from '../ayudas/afirmar.js';

const ui = cargarInterfaz({
    campos: { cat_nom: 'Categoría de Prueba' },   // el DOM, por id
    api: { crearCategoria: 7 },                    // respuestas de AppAPI (valor o función)
    confirm: true,                                 // confirm(): true | false | función | cola []
    prompt: 'texto',                               // prompt():  texto | null (cancelar) | función | cola []
                                                   // (una función puede devolver una **promesa** que se resuelve tarde:
                                                   //  así responde el WebView real, y es lo que prueba `contrato/dialogos.test.js`)
});
await ui.appUI.handleAgregarCategoria(crearEvento());   // SIEMPRE por appUI.<método>
llamoUnaVez(ui, 'crearCategoria', ['Categoría de Prueba']);
avisoExito(ui, /Categoría agregada/);
redibujo(ui, 'ajustes');
```

`cargarInterfaz` devuelve: `appUI`, `api` (el doble), `llamadas` (`[{ metodo, args }]`),
`avisos` (`[{ tipo, mensaje }]`), `renders` (rutas que `render()` recibió),
`confirmaciones`/`preguntas` (textos de los diálogos), `eliminados` (ids de
elementos a los que se llamó `remove()`: modales cerrados), `navegaciones`,
`temporizadores`, y los atajos `elemento(id)`, `fijar(id, spec)`,
`responder(metodo, valor)`, `rechazar(metodo, error)`, `llamadasA(metodo)`.

### Campos (DOM falso)

`campos` es `{ id: spec }`. `spec` puede ser:

* texto o número → `{ value }`;
* booleano → `{ checked }`;
* objeto → propiedades del elemento: `value`, `checked`, `hidden`, `dataset`,
  `textContent`, y para un `<select>` `options: [{ value, text, dataset, attrs }]` +
  `selectedIndex` (el `value` sigue a la opción elegida);
* `AUSENTE` → el id **no existe** (`getElementById` da `null`): para los `buscar()`
  opcionales (p. ej. un campo que solo se dibuja a veces).

**Todo id que el manejador lea y la prueba no declare hace fallar la prueba**
con el mensaje «La prueba no declaró el elemento #x…», aunque el manejador lo
capture con su `try/catch` (el helper lo comprueba al terminar cada llamada a
`appUI`). Lo mismo con `confirm()` y `prompt()` sin configurar. Así cada prueba
documenta exactamente qué lee su manejador.

### Lo que se sustituye

* `AppAPI`: solo existen los métodos **reales** de `api.ts` (se evalúa `api.js`
  para leer sus nombres): un nombre mal escrito falla como en la aplicación.
  Devuelven `undefined` salvo que `api`/`responder` digan otra cosa;
  `rechazar(metodo, 'texto')` hace que la promesa rechace (Tauri rechaza con texto).
  **Las pruebas cubren `ui → AppAPI`; `AppAPI → invoke` (conversión `Number()`,
  nombres de comando) lo cubre `contrato/ipc.test.js`.**
* `render(ruta)` solo **registra** la ruta (`ui.renders`): dibujar una pestaña
  entera es otra prueba (`comparar_vistas`). `renderReal: true` lo deja real.
  Los métodos secundarios que un manejador invoca (`alternarAbonos`…) se pisan
  en la prueba: `ui.appUI.alternarAbonos = async id => …`.
* `showToast` se envuelve: registra `{ tipo, mensaje }` **y** ejecuta el código real.
* `setTimeout` no corre (se guarda en `temporizadores`): nada se retrasa.

## Añadir la prueba de un manejador nuevo

1. Lee el manejador y anota los ids que lee (`elemento('…')`/`buscar('…')`).
2. Escribe en el archivo de su pestaña (o crea uno): camino feliz con valores
   **distintos por campo** (para que un par de argumentos intercambiados no pase),
   comparando con `llamoUnaVez(ui, 'metodo', [args exactos])`. La comparación
   es estricta: `123.45` ≠ `'123.45'`, `null` ≠ `0`.
3. Un camino por cada validación del cliente → `noLlamoANada(ui)` + `unAvisoDeError(ui, /…/)`.
4. Un camino «Rust rechaza» → `rechazoSeMuestra(ui, () => ui.appUI.x(...))`.
5. Llama siempre a `ui.appUI.<método>(` escrito en claro: `cobertura.test.js`
   lo busca así. Si un manejador no se puede probar, decláralo con su razón en
   `NO_CUBIERTOS` de ese archivo.
6. Usa solo datos inventados. El repositorio es público.

## Tras la división de `ui.ts`

Solo hay que adaptar `cargar_interfaz.js`: la lista `SCRIPTS` (qué archivos se
evalúan, en el orden de `index.html`) y la última línea de `evaluar`
(`return appUI;`, que hoy es la `const` del script) para devolver el objeto
mezclado de las vistas. Las pruebas llaman a `appUI.<método>` y no cambian.
Si las vistas pasan a ser módulos ES, `evaluar` se sustituye por una
`import()` con `globalThis.document/AppAPI/…` fijados antes de importar.

## Límites

* **No es un navegador.** No hay CSS, ni layout, ni eventos que se propaguen, ni
  validación nativa (`required`, `step`, `type=number`): lo que el navegador
  impide antes de llamar al manejador **no está probado aquí** (lo vigila
  `importes.test.js` sobre el HTML).
* `innerHTML` es una cadena opaca; `querySelector*` solo entiende `#id`.
* No se prueba el HTML que dibujan los `render*` ni que los atributos
  `onclick`/`onsubmit` pasen los argumentos correctos (`${id}` mal interpolado):
  eso es de `comparar_vistas` y de la prueba de manejadores.
* `render` está sustituido, así que no se ve si una pantalla queda rota tras
  guardar; sí que el manejador la pide.
* Los importes: casi todo se envía como **número** (`Number(...)`); solo
  avance de efectivo, suscripciones, capital (certificados, bolsa, propiedades),
  saldo declarado, comisión de cuenta editada y cobro parcial viajan como **texto**.
  Las pruebas fijan lo que hay: cambiar uno por otro las rompe.

## Cobertura (49 `handle*` + reactores)

Los 49 `handle*` están cubiertos; `cobertura.test.js` lo vigila.

| Pestaña | Manejadores |
|---|---|
| Gastos | `handleAgregarGasto`, `handleLiquidacionSubmit`, `handleSelectGastosMonth` |
| Ingresos | `handleAgregarIngreso`, `handleAgregarIngresoInformal`, `handleEdicionFormalSubmit`, `handleCobroFormalSubmit`, `handleCobroInformalSubmit`, `handleSelectCliente` |
| Tarjetas | `handleAgregarTarjeta`, `handleEdicionLimitesTarjetaSubmit`, `handleAbonoTarjeta`, `handleRevertirAbono`, `handleAvanceEfectivo`, `handleRevertirAvance`, `handleAgregarBonificacion`, `handleEliminarBonificacion` |
| Cuentas / Efectivo | `handleAgregarCuenta`, `handleEliminarCuenta`, `handleTransferirCuentas`, `handleAgregarEfectivoInformal`, `handleRetirarAEfectivo` |
| Suscripciones | `handleAgregarSuscripcion`, `handleEdicionSuscripcionSubmit`, `handleEliminarSuscripcion`, `handleCorregirProximoCobro`, `handleAsentarPendiente`, `handleDescartarPendiente` |
| Capital | `handleAgregarCertificado`, `handleEliminarCertificado`, `handleAgregarBolsa`, `handleEliminarBolsa`, `handleAgregarPropiedad`, `handleEliminarPropiedad` |
| Préstamos | `handleAgregarPrestamo`, `handleEdicionPrestamo`, `handleDeclararSaldo`, `handlePagarCuota`, `handleEliminarPrestamo` |
| Ajustes | `handleCrearRespaldo`, `handleRestaurarRespaldo`, `handleAgregarCategoria`, `handleEliminarCategoria`, `handleAgregarCliente`, `handleEliminarCliente`, `handleEliminarGastoCorr`, `handleEliminarIngresoCorr`, `handleEliminarIngresoInformalCorr`, `handleEliminarTransaccionCuentaCorr` |

Fuera de los `handle*`: `abrirEdicionCuenta` (tres `prompt`, comisión como texto),
y los reactores `aplicarTipoAbono`, `previsualizarTasa`,
`actualizarConversionGasto` y `alternarCobroParcial` (`reactores.test.js`).

**No cubiertos** (no son manejadores de dinero y casi no tienen lógica):
`toggleMetodoPago`, `toggleCamposPrestamos`, `alternarAvance`, `aplicarTipoAvance`,
`avisarFechasDerivadas`, `rotularDivisasTransferencia`, `alternarAbonos`,
`alternarAvances`, `alternarCasosDeCorreccion`, `abrirMenuPasivo`/`cerrarMenuPasivo`
(mostrar/ocultar y rotular), y los `abrir*` que solo construyen un modal
(`abrirEdicionFormal`, `abrirEdicionLimitesTarjeta`, `abrirCobroFormal`,
`abrirCobroInformal`, `abrirEdicionSuscripcion`, `abrirEdicionPrestamo`,
`abrirLiquidacionConsumo`): su producto es HTML, que es de `comparar_vistas`.

## Defectos reales encontrados (no corregidos)

Pruebas marcadas `todo` (describen el comportamiento deseable; su fallo no rompe
`npm test` y pasarán solas al corregir):

* `handleEliminarSuscripcion`, `handleEliminarCertificado`, `handleEliminarBolsa`
  y `handleEliminarPropiedad` **no tienen `try/catch`**: si Rust rechaza, la
  promesa del `onclick` rechaza sin que nadie la atrape y el usuario no ve ningún
  aviso (y la pantalla queda sin redibujar).
