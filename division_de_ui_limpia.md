# División de `ui.ts`, diseño «B» (limpio): vistas con dependencias inyectadas

> **Estado (2026-10-01): diseño B con puente `window.appUI`, elegido por el titular**, que prefiere el método limpio aunque exija reescribir cuerpos. **PR 0 hecho en 1.51.0** (servicios, composición, registro de vistas, `index.html`, y las pruebas de contrato leen toda la interfaz). **PR 1 hecho en 1.52.0: `efectivo` extraída; PR 2 en 1.53.0: `cuentas`; PR 3 en 1.54.0: `resumen`; PR 4 en 1.55.0: `dashboard`; PR 5 en 1.56.0: `capital`; PR 6 en 1.57.0: `suscripciones`; PR 7 en 1.58.0: `gastos`** con el prototipo de §3 y sus pruebas de Node; el registro de extracciones, con lo previsto frente a lo medido, está en §9. La medición y el análisis de abajo los produjo un agente independiente y se verificaron antes de adoptarlos.


Estado: **modelo, medición y un prototipo (la vista `efectivo`) en la rama `modelo/division-limpia`. Nada fusionado.** Es la alternativa al diseño «A» (mecánico) de [division_de_ui.md](division_de_ui.md); la decisión 1 de ese documento (§7) se toma comparando ambos con números, aquí.

Todo lo medido sale de `herramientas/medir_division_ui.mjs`, que analiza `ui.ts` con el compilador de TypeScript (grafo de llamadas por alcanzabilidad desde cada `render*`, igual que el plan). Se ejecuta con `node herramientas/medir_division_ui.mjs [ruta/ui.ts]`; las cifras de este documento son las de `ui.ts` **antes** del prototipo (1.48.0). Los datos de las pruebas son inventados; no se usó ningún dato real.

## 1. Resumen

| | A (mecánico) | B (limpio) |
|---|---|---|
| Cuerpos de método que cambian | **0** | **93 de 102** (79 solo por lo compartido y el estado) |
| Líneas que cambian | 0 (más las del tipado, iguales en ambos) | **≈619 de 4 681** (13 %), casi todas sustituciones textuales |
| `this` | el del objeto mezclado (todo el `appUI`) | el de la propia clase; el resto entra por el constructor |
| Acoplamiento entre vistas | oculto (todas ven todo vía `this`) | imposible: una vista solo ve lo que se le inyecta |
| ¿Se prueba una vista en Node? | no sin navegador | sí, con dobles (prototipo: 8 pruebas) |
| Cableado de `onclick="appUI.x()"` | intacto | intacto (puente) o reescrito (`data-accion`) |

Recomendación (§8): **B con puente `window.appUI`, vista a vista**, porque pasar por A y luego a B mueve cada vista dos veces.

## 2. Arquitectura

### 2.1 Servicios inyectados (`src/js/ui/servicios.ts`, ya existe)
Solo interfaces. Una vista declara en su constructor el subconjunto que usa.

```ts
export interface Avisos     { mostrar(mensaje: string, tipo?: TipoAviso): void }   // showToast
export interface Formato    { importe(valor: number | string): string }           // formatMoney
export interface Enrutador  { mostrar(ruta: string): Promise<void> }              // render(ruta)
export interface Pantalla   { readonly contenido: { innerHTML: string } }         // contentContainer
export interface Dom        { elemento<T extends HTMLElement = HTMLElement>(id: string): T }
export type      Reloj      = () => Date                                          // new Date()
export type      ApiDe<K extends keyof typeof AppAPI> = Pick<typeof AppAPI, K>     // AppAPI, recortada
export interface Vista      { render(): Promise<void> }
```

Faltan dos que el prototipo no necesita pero 11 de las vistas sí (medido: **25** usos de `confirm(`/`prompt(`, que en Node no existen y hacen imposible probar los borrados):

```ts
export interface Dialogos { confirmar(texto: string): boolean; pedirTexto(texto: string): string | null }
export interface Motivo   { pedir(queOcurre: string, consecuencia: string): string | null }  // pedirMotivoDeCorreccion
```

`Motivo` es el único «compartido» con lógica (usa `prompt` y un aviso): pasa a ser un servicio construido con `Dialogos` y `Avisos`. `TASA_USD_A_DOP` (5 usos, resumen y pasivos) pasa a `nucleo/` como constante exportada. `buscar()` se añade a `Dom` cuando una vista lo pida.

### 2.2 Forma de una vista
Una **clase** por pestaña, con un objeto de dependencias tipado y solo lo que usa:

```ts
export class VistaEfectivo implements Vista {
    constructor(private readonly dep: DependenciasEfectivo) {}
    async render() {...}
    async handleRetirarAEfectivo(e: EventoDeFormulario) {...}
}
export function puenteEfectivo(vista: VistaEfectivo): Record<ManejadorEfectivo, ...>   // lo que expone appUI
```

* Los métodos que solo se llaman entre sí **siguen siendo `this.metodo()`**: es lo único que B conserva de `this`. Las 34 referencias a métodos propios (grupo a de §4) no se tocan.
* El estado propio se vuelve campo privado de su clase: `selectedGastosMonth` (solo gastos, 2 usos) y `_menuPasivoAbort` (solo préstamos, 4 usos). `contentContainer` pasa a `Pantalla`; `notifContainer` es detalle del servicio `Avisos`.
* **Una vista no importa otra** (medido: 0 referencias entre vistas, §4); las pestañas se piden por ruta al `Enrutador`.

### 2.3 Raíz de composición (`src/js/composicion.ts`, ya existe)
Módulo ES que carga `index.html` tras `app.js` (`<script type="module">`: se ejecuta después de leer el documento y antes de `DOMContentLoaded`, cuando `app.ts` dibuja la primera pestaña). Es **el único sitio que conoce las piezas reales**: crea los servicios, construye cada vista con `{ ...servicios, api: AppAPI }` y la registra:

```ts
const efectivo = new VistaEfectivo({ ...servicios, api: AppAPI });
appUI.registrarVista('efectivo', efectivo, puenteEfectivo(efectivo));
```

Mientras queden vistas sin extraer, `servicios` delega en la clase vieja (`serviciosDesdeAppUI`). Con la última vista desaparecen la clase y ese adaptador, y los servicios pasan a tener implementación propia.

### 2.4 Registro y refresco
`AppUI.registrarVista(ruta, vista, puente)` guarda la vista en un `Map` y copia el puente en `appUI`. `render(ruta)` mira primero el `Map` y solo si no está cae al `switch` viejo: cada PR quita un `case`. Refrescar la pestaña es `enrutador.mostrar(ruta)`, que es el mismo `render` de hoy (muestra «Cargando…» y vuelve a pedir los datos).

### 2.5 Cableado de los manejadores en línea: dos opciones

Medido en `ui.ts`: **91** atributos `on…=` (88 con comillas dobles, 3 con simples) más 11 en `index.html`; 76 referencias a `appUI.x`; **12** llevan `elemento()`/`buscar()` dentro del propio atributo; **3** pasan un objeto serializado (`onclick='appUI.abrirEdicionCuenta(${JSON.stringify(c)})'`: suscripciones, préstamos, ajustes); 1 usa `event`. Eventos usados: `onclick` (49), `onsubmit` (25), `onchange` (14), `oninput` (3): todos burbujean.

| | Opción 1: puente `window.appUI` | Opción 2: delegación con `data-accion` |
|---|---|---|
| Qué es | `registrarVista` copia en `appUI` unos métodos que delegan en la vista; los atributos no cambian | Un solo oyente en el contenedor lee `data-accion="efectivo.retirar"` y llama a la vista registrada; los `on…=` desaparecen |
| HTML a tocar | **0 atributos** | **~91 + los 11 de `index.html`**, y los 12 con `elemento()` y los 3 con `JSON.stringify` exigen **rediseñar** (pasar un id y que la vista busque el objeto) |
| Líneas nuevas por vista | ≈ nº de manejadores (3 en efectivo) | un mapa acción→método por vista + el despachador común |
| Cambia el comportamiento | no | sí: `this`, `event` y los argumentos se obtienen de otro modo |
| `manejadores.test.js` | sigue valiendo | se sustituye por «toda `data-accion` está registrada» (más fuerte: comprueba el registro real, no el texto) |
| Globales | sigue habiendo `window.appUI` | ninguna (excepto `navigate`, si no se convierte también) |
| Riesgo | bajo y localizado | alto: un atributo mal escrito falla al pulsar, sin error al cargar |
| Prueba en Node | el puente se prueba (prototipo, última prueba) | el despachador se prueba una vez, con un DOM falso |

**Conclusión de la comparación:** la opción 1 es la que cuesta lo que B debe costar. La opción 2 es mejor arquitectura final, pero es un segundo proyecto (reescribir HTML, no solo JS) y su riesgo es de la clase que `comparar_vistas` no ve («pintar ≠ pulsar»). Se puede añadir **después**, vista por vista, sobre clases que ya existen; no es requisito de B.

### 2.6 Tipado
* Cada vista entra en `tsconfig.estricto.json` (`src/js/vistas/*.ts`, `ui/servicios.ts`, `composicion.ts`): `noImplicitAny` desde el primer día.
* `ApiDe<K>` recorta `typeof AppAPI` por nombres: **el compilador comprueba que cada vista solo usa envoltorios que existen** y con los tipos de Rust. Esto reemplaza (para las vistas extraídas) a la prueba de texto «toda llamada a `AppAPI` tiene su envoltorio»: un nombre inexistente es error de compilación, no una cadena que la prueba tiene que acertar a leer.
* Con A, `this` se tipa con `ThisType<AppUI>` y `AppUI` es la intersección de todas las vistas, que a su vez declaran `ThisType<AppUI>`: es una referencia circular de tipos que **no se probó** aquí y es la parte incierta de A.
* Los 169 parámetros sin tipo de `ui.ts` hay que tiparlos en ambos diseños: no es coste diferencial.

### 2.7 Cómo se prueba una vista en Node
`pruebas/js/vistas/efectivo.test.js` (ya existe). Se importa el JS compilado de la vista y se construye con dobles: API falsa que anota las llamadas, `avisos`, `enrutador`, `pantalla` y `ahora` falsos, y un `dom` que devuelve `{ value }` por id. Se comprueba qué envía a la API, qué avisa y si vuelve a dibujar. No hay navegador, ni JSDOM, ni dependencias nuevas.

## 3. El prototipo: `efectivo`

Archivos nuevos: `src/js/ui/servicios.ts` (88 líneas, casi todo interfaces), `src/js/vistas/efectivo.ts` (193), `src/js/composicion.ts` (21), `pruebas/js/vistas/efectivo.test.js` (127). Cambios: `ui.ts` (−148, +22: se retiran los 3 métodos, se añade `vistas` y `registrarVista` y un desvío en `render`), `index.html` (+2), `tsconfig.estricto.json` (+3), `importes.test.js` (+9/−4, ver abajo).

* `npm test`: 50 pruebas, 0 fallos. `npm run tipos`: limpio (incluye la vista con `noImplicitAny`). `npm run compilar`: limpio.
* **Una prueba de contrato tuvo que adaptarse:** `importes.test.js` lee `ui.js` y busca `AppAPI.metodo(` para exigir que todo envío de dinero salga de un `onsubmit`. En la vista la API se llama `api.metodo(` y el archivo es otro, así que la prueba fallaba («`crearCobroEfectivoInformal` ya no se usa»). Ahora lee `ui.js` más `vistas/*.js` y acepta `AppAPI` o `api`. Es un **coste real y recurrente de B**: las pruebas que leen el texto de `ui.ts` deben aprender a leer las vistas (con A pasa lo mismo, pero solo con los archivos, no con los nombres). `ipc.test.js` (interfaz → `api.ts`) y `tipos.test.js` siguen leyendo solo `ui.ts`; para `efectivo` el compilador cubre lo que ellas cubrían (`ApiDe`), y habrá que ampliarlas si alguna vista reintroduce lecturas de campos.
* `manejadores.test.js` **no necesitó cambios**: los métodos con el nombre del manejador existen en la clase y los `appUI.x(` de la plantilla siguen en el texto.

### Qué demostraron las pruebas de Node
Ocho pruebas: la entrada informal envía a la API los campos con el monto como número, avisa y refresca `efectivo`; si la API falla, avisa con tipo `error` y no refresca; el retiro desde una cuenta en dólares va a «Efectivo USD» y desde pesos a «Efectivo DOP» (con monto y cargo numéricos); una cuenta origen inexistente no llega a la API; la falta de la cuenta de efectivo de destino dice cuál falta; el dibujo usa la fecha inyectada, el formato inyectado y excluye las cuentas de efectivo del origen; y **el puente expone todo `appUI.x(` que escribe la plantilla** (esto detecta en Node el fallo que hoy solo se vería al pulsar).

**Mutaciones** (cada una rompe el manejador a propósito; se restauró tras cada una; `npm test` final 50/50):
1. destino siempre «Efectivo DOP»: fallan 2 pruebas;
2. el `catch` deja de avisar: falla 1 prueba;
3. el puente renombra un manejador: **falla la compilación** (tipos);
4. el monto deja de pasar por `Number()`: falla 1 prueba.

### Diferencia medida entre lo previsto y lo real
El script preveía 23 líneas a reescribir en `efectivo`; el diff real de los cuerpos es **27 líneas cambiadas de 142 (19 %)**, más 31 nuevas: las 4 de diferencia son `const { … } = this.dep;` y las firmas tipadas. Es decir, **el script subestima ≈15 %**, y el porcentaje es mayor en vistas pequeñas (el ruido fijo pesa más).

## 4. Medición por vista (antes del prototipo)

`a` = `this.` a métodos de la propia vista; `b` = a los 4 compartidos; `c` = a estado; `d` = a otras vistas. Manejadores: atributos `on…=` que escribe la vista. «Líneas B» = líneas del cuerpo que tocan algo que B inyecta: `this.showToast/formatMoney/render/pedirMotivo/contentContainer…` (B mínimo) más `AppAPI.`, `elemento()`, `buscar()`, `document.`, `window.`, `navigate(` (B total).

| Vista | Métodos | Líneas | a | b | c | d | Manej. | `elemento`/`buscar` | `AppAPI.` | Cuerpos B (mín / total) | Líneas B (mín / total) |
|---|---|---|---|---|---|---|---|---|---|---|---|
| tarjetas | 17 | 835 | 8 | 55 | 1 | 0 | 17 | 40 | 15 | 12 / 16 | 53 / 109 |
| ajustes | 16 | 714 | 0 | 55 | 1 | 0 | 18 | 39 | 23 | 16 / 16 | 56 / 103 |
| préstamos | 20 | 707 | 20 | 32 | 5 | 0 | 6 | 31 | 8 | 12 / 15 | 36 / 77 |
| ingresos | 11 | 518 | 0 | 32 | 1 | 0 | 16 | 41 | 11 | 9 / 11 | 32 / 81 |
| gastos | 9 | 480 | 3 | 25 | 3 | 0 | 11 | 31 | 6 | 6 / 8 | 28 / 63 |
| suscripciones | 10 | 366 | 2 | 27 | 1 | 0 | 8 | 15 | 9 | 7 / 9 | 28 / 51 |
| capital | 7 | 294 | 0 | 20 | 1 | 0 | 8 | 14 | 13 | 7 / 7 | 21 / 48 |
| cuentas | 3 | 193 | 0 | 8 | 1 | 0 | 3 | 10 | 3 | 2 / 3 | 9 / 22 |
| resumen | 1 | 168 | 0 | 15 | 1 | 0 | 0 | 0 | 7 | 1 / 1 | 16 / 23 |
| dashboard | 1 | 161 | 0 | 8 | 1 | 0 | 2 | 0 | 6 | 1 / 1 | 8 / 14 |
| efectivo | 3 | 144 | 0 | 9 | 1 | 0 | 2 | 9 | 4 | 3 / 3 | 10 / 23 |
| compartido | 4 | 101 | 1 | 0 | 3 | 12* | 0 | 0 | 0 | 3 / 3 | 4 / 5 |
| **Total** | **102** | **4 681** | **34** | **286** | **20** | **0** | **91** | **230** | **105** | **79 / 93** | **301 / 619** |

\* Las 12 referencias «a otras vistas» del grupo compartido son las del `switch` de `render` (`this.renderX()`), es decir, el enrutador, que es lo que `registrarVista` sustituye. **Entre las 11 vistas hay 0** referencias cruzadas: confirmado, como decía el plan. Tampoco hay métodos alcanzados desde dos vistas ni huérfanos.

Otros datos que condicionan B: `confirm(`/`prompt(` por vista: ajustes 11, tarjetas 4, suscripciones 3, capital 3, préstamos 2, ingresos 1 (+1 en `pedirMotivoDeCorreccion`); `new Date` en 10 sitios (→ `Reloj`); `document.`/`window.` directos en 20 (ingresos 6, préstamos 7: menús y cierres de panel); 4 `navigate(`.

### Cuerpos y líneas a reescribir

| | Cuerpos | Líneas |
|---|---|---|
| **A** | **0** | **0** |
| **B mínimo** (solo lo compartido y el estado) | 79 de 102 | 301 |
| **B total** (más API, DOM, `document`, `navigate`) | **93 de 102** | **619 de 4 681 (13 %)** |
| B total + diálogos a inyectar | 93 | 619 + 25 |
| Corregido por el sesgo medido en el prototipo (+15 %) | — | ≈ 710 |

Casi todo es **mecánico y verificable**: sustituciones textuales de `this.showToast(`, `this.formatMoney(`, `this.render(`, `AppAPI.`, `elemento<Campo>(`, `this.contentContainer`. Lo que **no** es una sustitución (a mano, con juicio): `pedirMotivoDeCorreccion` (8 usos; pasa a servicio), `_menuPasivoAbort` (4 usos en préstamos, con `AbortController`), `selectedGastosMonth` (2), los 20 usos de `document.`/`window.`, los 25 de `confirm`/`prompt` y los 3 manejadores con `JSON.stringify`.

### Esfuerzo relativo por vista (efectivo = 1)
Base: líneas B total ÷ 23; sumando los puntos no mecánicos. Es una estimación de orden de magnitud, no de horas.

| Vista | Relativo | Qué lo encarece |
|---|---|---|
| efectivo | 1 | hecho |
| cuentas | 1 | un manejador abre el editor con el objeto serializado |
| resumen | 1 | 1 método de 168 líneas; usa `TASA_USD_A_DOP` (a `nucleo/`) |
| dashboard | 0,6 | solo lectura |
| capital | 2 | 3 diálogos, 13 llamadas a la API |
| suscripciones | 2,5 | 3 diálogos, 1 manejador con JSON |
| gastos | 3 | estado `selectedGastosMonth` |
| ingresos | 4 | 41 accesos al DOM, 5 manejadores con `elemento()` en el atributo, usa `pedirMotivo` |
| préstamos | 4 | `_menuPasivoAbort`, 7 `document.`, 20 llamadas internas, 1 JSON |
| ajustes | 6 | 11 diálogos, 23 llamadas a la API, `pedirMotivo` ×4, 1 JSON |
| tarjetas | 6 | la mayor, 17 manejadores, 4 diálogos |
| **Infraestructura común** | ≈ 3 | `Dialogos`, `Motivo`, `Reloj`, implementación real de `Avisos` (con `notifContainer`), tasa a `nucleo/` |

## 5. Comparación A / B sobre `efectivo`

| | A (estimado, no implementado) | B (implementado) |
|---|---|---|
| Diff del código de la vista | ≈ 144 líneas movidas, 0 cuerpos cambiados, ≈ 10 de envoltorio; el tipado de 3 firmas es común | −148/+22 en `ui.ts`; 193 líneas nuevas; **27 líneas de los cuerpos cambiadas (19 %)** |
| Infraestructura | `Object.assign`, `ThisType` | `servicios.ts` 88 + `composicion.ts` 21 (se amortizan con las demás vistas) |
| Archivos nuevos | 1 (la vista) | 3 + 1 de pruebas |
| Líneas totales añadidas (sin pruebas ni documentos) | ≈ 160 | **≈ 300** (193 + 88 + 21) |
| Riesgo de regresión | el más bajo: ni un cuerpo cambia; solo se rompe un `this` o un puente | bajo-medio: 27 líneas cambiadas, pero **cada una es una sustitución comprobable por el compilador** (un servicio mal nombrado no compila) |
| Pruebas unitarias de la vista | no (exige navegador) | **sí**: 8, y 4 mutaciones detectadas (3 por prueba, 1 por compilación) |
| Tipos | `this` por `ThisType` circular, no probado | cada dependencia con su tipo; la API recortada por nombres y comprobada |
| Acoplamiento | implícito: la vista ve todo `appUI` | explícito: 7 dependencias en un tipo |
| Cableado | sin cambio | sin cambio (puente) |
| Pruebas de contrato | se adaptan a otros archivos | ídem y a otro nombre (`api.`); una se tocó |

Qué se gana con B: probar lo que **hace al pulsar**, que es lo que `comparar_vistas` y `manejadores.test.js` no cubren (el plan, §6, lo reconoce como el hueco sin automatizar); detectar errores de cableado en la compilación; y dejar de depender de globales y de `this`. Qué cuesta: 93 cuerpos retocados, ≈ 2× las líneas añadidas en una vista pequeña, un adaptador temporal (`serviciosDesdeAppUI`) y el cambio de las pruebas que leen texto.

## 6. Lo que NO se pudo comprobar
* **En la aplicación empaquetada o en un navegador.** Por indicación, no se usó navegador ni se compiló la app. Lo que sí se comprobó: compilación, tipos, 50 pruebas y las mutaciones. Queda por comprobar a mano (en el PR real): que `<script type="module" src="js/composicion.js">` ejecute antes del primer `navigate('dashboard')` y que la pestaña «Efectivo» se dibuje y sus dos formularios funcionen. `division_de_ui.md` §2 ya recoge que los módulos ES se comprobaron en la app empaquetada; lo nuevo aquí es que un módulo lea los globales `appUI`/`AppAPI`/`elemento` de scripts clásicos (es el comportamiento estándar del ámbito léxico global, pero no se ejecutó).
* **`herramientas/comparar_vistas`**: no se usó porque `preparar.sh` copia datos reales. Con B es **más** necesaria, porque cambian cuerpos que pintan. En esta vista el HTML de la plantilla es idéntico carácter a carácter salvo las sustituciones de `this.formatMoney(` por `formato.importe(` y del contenedor, verificado con `diff`, y la prueba de dibujo lo ejerce con datos inventados; no equivale a compararlo con datos reales.
* La estimación de esfuerzo es de orden de magnitud, calibrada con una sola vista.

## 7. Discrepancias con `division_de_ui.md`
* **102 métodos, no 103** (el plan cuenta quizá el constructor). Las 98 + 4 siguen cuadrando.
* **Líneas por vista**: salen distintas en ±5 % por el criterio de conteo (comentarios y blancos previos); p. ej. efectivo 144 vs 151, ajustes 714 vs 736, cuentas 193 vs 183. Los métodos por vista **coinciden exactamente** y las proporciones también.
* **«99 manejadores»**: son 88 en `ui.ts` con comillas dobles + 11 en `index.html`. En `ui.ts` hay además **3 con comillas simples** (`abrirEdicionSuscripcion`, `abrirMenuPasivo`, `abrirEdicionCuenta`) que **`manejadores.test.js` no ve** (su expresión solo reconoce comillas dobles): la red de contrato tiene ese hueco hoy, independientemente del diseño elegido. Conviene cerrarlo en el PR 0.
* El plan dice «0 cuerpos» para A: cierto, pero omite que `ThisType<AppUI>` sobre una intersección circular no está probado.
* Las referencias a `navigate` en manejadores (15 en el plan) incluyen las del `index.html`; en `ui.ts` solo hay 4 usos en código.

## 8. Recomendación y orden

**B, con puente `window.appUI`, vista a vista, y diferir `data-accion`.** Razones:
1. Con A ya extraída una vista, convertirla en B es mover y reescribir de nuevo: dos PR por vista. Si el destino es B, ir directo.
2. El coste diferencial de B (≈ 13 % de las líneas, casi todo sustituciones que el compilador verifica) es proporcional y acotado; el beneficio (pruebas de lo que ocurre al pulsar, tipos reales, acoplamiento visible) es justo lo que el plan declara sin cubrir.
3. La opción 2 (`data-accion`) añade un proyecto de reescritura de HTML con riesgo de pulsar; no es necesaria para B.

**Híbrido razonable** si se quiere acotar riesgo: B en todas, pero para `dashboard` y `resumen` (una sola función de solo lectura, 0 manejadores) basta la forma mínima de B sin más pruebas que la de dibujo; el valor de las pruebas está en las vistas que **mueven dinero** (ingresos, tarjetas, préstamos, ajustes, cuentas, efectivo).

Orden y esfuerzo (efectivo = 1):
0. **PR 0 (≈ 3):** `servicios.ts`, `composicion.ts`, `registrarVista`, `index.html`, ampliación de las pruebas de contrato (leer `vistas/`, aceptar `api.`, **cerrar el hueco de las comillas simples**). Es lo que contiene el prototipo, salvo `Dialogos`/`Motivo`.
1. efectivo (1, hecho como prototipo) → cuentas (1) → resumen (1) → dashboard (0,6): sin diálogos.
2. `Dialogos` + `Motivo` + `Reloj` en servicio real, con capital (2) → suscripciones (2,5).
3. gastos (3, estado propio) → ingresos (4) → préstamos (4).
4. ajustes (6) → tarjetas (6), al final, por tamaño y por número de manejadores y diálogos.
5. Retirar la clase `AppUI` y `serviciosDesdeAppUI`. Opcional: `data-accion`.

Total ≈ 34 unidades frente a ≈ 13 PR del plan. En cada PR: `npm test`, `npm run tipos`, `comparar_vistas` con datos reales, y revisión manual de lo que se hace al pulsar; con B, además, **la prueba de Node de los manejadores de esa vista** como parte del PR.

## 9. Registro de extracciones: lo previsto frente a lo medido

Se actualiza con cada PR. «Previsto» es lo que decían `division_de_ui.md` (diseño mecánico) y la medición de §4; «medido» sale del PR.

### PR 1 — `efectivo` (1.52.0)

| Qué | Previsto | Medido |
|---|---|---|
| Métodos que se mueven | 3 (`renderEfectivo` y dos manejadores) | 3 |
| Líneas de la vista | 151 (plan) · 144 (medición del agente) | 140 de métodos + 2 comentarios y espacios |
| Cuerpos reescritos | 19 % (prototipo del agente); 0 % con el diseño mecánico | **17 %**: 24 de 134 líneas (el `render` solo un 6 %; los manejadores, 66 % y 45 %, porque son cortos y casi todo lo que tocan es compartido) |
| Diff de `ui.ts` | −148 / +22 (prototipo) | **−147 / +0** (el +22 del registro ya estaba en el PR 0) |
| Archivos nuevos | `vistas/efectivo.ts`, `ui/servicios.ts`, `composicion.ts` | `vistas/efectivo.ts` (193), `vistas/registro.ts` (18) y pruebas (127); servicios y composición ya existían |
| Adaptar `cargar_interfaz.js` a módulos con `import()` | «hay que adaptar el helper; si las vistas son módulos, `import()`» | **No hizo falta `import()`**: el helper importa estáticamente `registro.js` y `servicios.js` (sin estado) y sigue siendo síncrono; **25 líneas** y ninguna prueba cambió. Mejor de lo previsto |
| Adaptar pruebas de contrato | «`importes.test.js` +9/−4» | **+4/−2**: las llamadas de la API en una vista son `api.x(`, no `AppAPI.x(` |
| Parámetros sin tipo (`noImplicitAny`) | 169 en `ui.ts` | **167**; la vista nace estricta (`tsconfig.estricto.json`) |

**Lo que se comprobó, y cómo se compara con los seis controles del plan (§4 de `division_de_ui.md`):**

| Control del plan | Resultado |
|---|---|
| 1. `npm test` y `npm run tipos` | 231 pruebas (227 pasan, 4 `todo` conocidos, 0 fallan); 0 errores de tipos |
| 2. `manejadores.test.js` | pasa; los dos manejadores siguen existiendo (ahora en la vista) |
| 3. `comparar_vistas` con datos reales | **las once pestañas idénticas**, `efectivo` incluida (929 caracteres), sin `undefined`/`NaN`/errores |
| 4. Parámetros tipados en el mismo PR | sí; la vista entra en `tsconfig.estricto.json` |
| 5. Pruebas de interacción | las 8 de la vista (inyectando dobles) y las de `cuentas_y_efectivo` (a través de `appUI`, sin tocarlas) |
| 6. Revisión manual | **ejercí los dos formularios en la versión anterior y en la nueva con tus datos** (copia temporal) y comparé lo que envían: mismos comandos, mismos argumentos, mismos avisos, 0 errores |

Fuera del plan, también: **cuatro mutaciones** (retiro siempre a efectivo en pesos, vista sin registrar, puente sin un manejador, monto sin convertir): las cuatro se detectan (por las pruebas de la vista, por las de interacción o por el compilador). Y **en la aplicación empaquetada** (`tauri build`, `HOME` aislado): la vista queda registrada, se dibuja («Caja y Efectivo»), tiene su formulario y el manejador existe.

**Lo que dijo la medición y se confirmó:** casi todo lo reescrito es sustitución textual (`this.showToast` → `avisos.mostrar`, `this.formatMoney` → `formato.importe`, `this.contentContainer` → `pantalla.contenido`, `elemento()` → `dom.elemento()`) y el compilador la verifica.

**Lo que no estaba en el plan:** `AppAPI` global ya no se usa dentro de la vista, que recibe `api` recortada a tres comandos: una prueba puede darle una API falsa sin tocar globales.

### PR 2 — `cuentas` (1.53.0)

| Qué | Previsto (medición del agente) | Medido |
|---|---|---|
| Métodos que se mueven | 3 | 3 (`renderCuentas`, `rotularDivisasTransferencia` y `handleTransferirCuentas`) |
| Líneas de la vista | 193 | 196 quitadas de `ui.ts`, con sus comentarios |
| Líneas a reescribir | 22 (el script subestima ≈15 %, así que ≈25) | **24** de 173 (**13 %**): `render` 6 %, el reactor de divisas 19 %, el manejador del formulario 73 % |
| Diff de `ui.ts` | — | **−196 / +0** |
| Archivos nuevos | — | `vistas/cuentas.ts` (243), `pruebas/js/vistas/cuentas.test.js` (164); `registro.ts` +4 |
| Pruebas que hubo que tocar | ninguna prevista | **ninguna**: `cargar_interfaz.js` y las pruebas de contrato ya valían |
| Esfuerzo relativo (efectivo = 1) | 1 | ≈1: la extracción fue casi mecánica con la receta de `efectivo` |
| Parámetros sin tipo en `ui.ts` | 167 | **163** |

**Discrepancia con el modelo:** la nota de esfuerzo de `cuentas` decía «un manejador abre el editor con el objeto serializado»; ese manejador (`abrirEdicionCuenta`) **pertenece a Ajustes** según el grafo de llamadas (se dibuja en esa pestaña), no a `cuentas`. Esa dificultad llegará con `ajustes`. La medición por alcanzabilidad acertó el reparto; la nota era un error de atribución.

**Los seis controles del plan:**

| Control | Resultado |
|---|---|
| 1. `npm test` y `npm run tipos` | 241 pruebas (237 pasan, 4 `todo` conocidos, 0 fallan); 0 errores de tipos |
| 2. `manejadores.test.js` | pasa |
| 3. `comparar_vistas` con datos reales | **las once pestañas idénticas**, `cuentas` incluida (5 004 caracteres), 0 `undefined`/`NaN`/errores |
| 4. Parámetros tipados en el mismo PR | sí; `vistas/*.ts` ya está en `tsconfig.estricto.json` |
| 5. Pruebas de interacción | las de transferencia y reactores, a través de `appUI`, **sin tocarlas**, más 10 de la vista con dobles |
| 6. Revisión manual con tus datos | **el reactor de divisas** (con cuentas de divisas distintas muestra el aviso de cruce y los rótulos «en DOP»/«en USD»; con la misma, lo oculta) **y la transferencia**, en las dos versiones: mismo estado del DOM, mismos comandos, mismos argumentos y mismo aviso, 0 errores |

**Más controles:** cinco mutaciones (importes de origen y destino intercambiados, aviso de cruce siempre visible, vista sin registrar, puente sin un manejador, fecha real en vez del reloj inyectado): las cinco se detectan, dos de ellas por el compilador. **En la aplicación empaquetada**: dos vistas registradas, se dibuja «Cuentas de Ahorro», el formulario existe y los dos manejadores son funciones.

### PR 3 — `resumen` (1.54.0)

| Qué | Previsto (medición del agente) | Medido |
|---|---|---|
| Métodos que se mueven | 1 | 1 (`renderResumen`, 164 líneas de cuerpo; solo lectura, sin manejadores) |
| Líneas a reescribir | 23 (≈26 con la corrección del 15 %) | **25** de 164 (**15 %**) |
| Diff de `ui.ts` | — | **−171 / +7** (el +7 es la tasa de referencia, ver abajo) |
| Archivos nuevos | — | `vistas/resumen.ts` (205), `pruebas/js/vistas/resumen.test.js` (150), `pruebas/js/contrato/rutas.test.js` (48); `registro.ts` +4, `servicios.ts` +13 |
| Esfuerzo relativo (efectivo = 1) | 1 | ≈1,3: hubo que decidir cómo inyectar una constante compartida y se añadió una prueba general |
| Parámetros sin tipo en `ui.ts` | 163 | 163 (la vista no tiene parámetros) |

**Dos cosas que el modelo anticipó a medias o no anticipó:**
* **`TASA_USD_A_DOP`.** El modelo la señaló («a `nucleo/`»), pero no que **la leen otros dos sitios de `ui.ts`** que aún no se han extraído. Moverla a `nucleo/` habría obligado a duplicarla (justo lo que su comentario original prohíbe: «dos copias de una tasa se desincronizan») o a que `ui.ts` importara un módulo, cosa que un script clásico no puede. Decisión: **un servicio inyectado, `Referencias`**, que la vista recibe; la constante sigue en `ui.ts` y la clase la expone (`appUI.tasaUsdADop`) mientras queden lectores; cuando se extraiga el último, pasa a la composición. Es el patrón de §2 aplicado a un valor, no a una función. Una prueba comprueba que la vista usa la tasa inyectada y no una escrita dentro (con 50 y con 70).
* **Un riesgo del `switch`.** `render()` termina en `default: renderDashboard()`: una vista extraída que **deja de registrarse no da error, pinta el Dashboard**. Las pruebas de la vista ejercen la clase, no el registro, y el Resumen no tiene manejadores que lo delaten. **Nueva prueba general** (`rutas.test.js`): cada pestaña del menú se dibuja desde un solo sitio (una vista registrada o un `case`), y ninguna vista registrada tiene una ruta que el menú no ofrezca. Protege también todas las extracciones siguientes.

**Los seis controles del plan:**

| Control | Resultado |
|---|---|
| 1. `npm test` y `npm run tipos` | 254 pruebas (250 pasan, 4 `todo` conocidos, 0 fallan); 0 errores de tipos |
| 2. `manejadores.test.js` | pasa (la vista no añade manejadores) |
| 3. `comparar_vistas` con datos reales | **las once pestañas idénticas**, `resumen` incluida (656 caracteres), sin `undefined`/`NaN`/errores |
| 4. Parámetros tipados en el mismo PR | sí; entra en `tsconfig.estricto.json` |
| 5. Pruebas de interacción | no hay manejadores que ejercer; en su lugar, **10 pruebas de las reglas de cálculo** (ver abajo) |
| 6. Revisión manual con tus datos | el Resumen **calcula**, así que se comparó, además del texto, **cada cifra que muestra**: 17 cifras, las mismas en el mismo orden en las dos versiones; y se repintó tres veces y tras ir a otra pestaña y volver: el mismo texto |

**Lo que ganó la vista con la inyección:** el Resumen es una pantalla de solo lectura pero **contiene reglas de dinero** (patrimonio neto, ratio de endeudamiento con sus tres rótulos, qué cuotas cuentan, suscripciones anuales entre 12, qué entra en el mes). **Hasta ahora ninguna prueba las ejercía.** Con datos inventados y resultados comprobables a mano ahora sí: tasa inyectada, mes del reloj inyectado, préstamo flexible frente a uno sin cuotas pendientes, sin activos (sin dividir por cero), balance negativo, capital `null`.

**Más controles:** seis mutaciones (ratio al revés, gastos de todos los meses, tasa escrita dentro, cuota sin cuotas pendientes que sigue sumando, anuales sin dividir entre 12, vista sin registrar): las seis se detectan, y la última solo la detecta la prueba nueva de rutas. **En la aplicación empaquetada**: tres vistas registradas, se dibuja «Resumen Ejecutivo» sin error y la tasa llega como 60. (Una sonda de texto buscó «Patrimonio Neto» y no lo halló: `innerText` aplica el `text-transform: uppercase` del CSS y devuelve «PATRIMONIO NETO»; es un defecto de la sonda, no de la vista, y las pruebas de Node comprueban las etiquetas en el HTML.)

### PR 4 — `dashboard` (1.55.0)

| Qué | Previsto (medición del agente) | Medido |
|---|---|---|
| Métodos que se mueven | 1 | 1 (`renderDashboard`, 157 líneas de cuerpo; solo lectura, sin manejadores) |
| Líneas a reescribir | 14 (≈16 con la corrección del 15 %) | **17** de 157 (**10 %**) |
| Diff de `ui.ts` | — | **−165 / +2** (los +2 son el `default:` de `render()`, ver abajo) |
| Archivos nuevos | — | `vistas/dashboard.ts` (209), `pruebas/js/vistas/dashboard.test.js` (139); `registro.ts` +4 |
| Esfuerzo relativo (efectivo = 1) | 0,6 | ≈0,9: la extracción fue la más limpia, pero hubo que tocar el enrutador y tipar el capital |
| Parámetros sin tipo en `ui.ts` | 163 | **161** (dos lambdas sobre el capital, ahora tipadas en la vista) |

**Lo que el modelo no preveía:**
* **El `default:` de `render()` era `renderDashboard()`.** Una ruta que nadie reconoce cae en el Dashboard; al quitar el método, esa rama habría dejado de funcionar o fallado. Ahora busca la vista registrada (`this.vistas.get('dashboard')?.render()`). Una prueba con el render real lo fija; con la mutación (buscar otra clave) falla. Es la segunda vez que el `switch` esconde un acoplamiento que el análisis de métodos no ve: **cuando se extraiga la última vista habrá que revisar el enrutador entero**.
* **El capital es `any`.** Rust lo devuelve como JSON libre, y las lambdas sobre `certificados` y `bolsa` eran parámetros sin tipo. La vista declara **solo lo que lee** (`ElementoConAlerta`), no un modelo del capital.
* **Una laguna de mis propias pruebas.** La primera versión de la prueba de avisos comprobaba los textos pero no el **nivel** de cada uno (el nivel decide color e icono); la mutación «el aviso de pago de tarjeta sale como `info`» pasaba. Se reforzó: ahora se fija el nivel de los cinco tipos de aviso. Detectada por el mismo control de mutaciones que se está aplicando a cada vista.

**Los seis controles del plan:**

| Control | Resultado |
|---|---|
| 1. `npm test` y `npm run tipos` | 263 pruebas (259 pasan, 4 `todo` conocidos, 0 fallan); 0 errores de tipos |
| 2. `manejadores.test.js` | pasa; el Dashboard no añade manejadores de `appUI` (sus enlaces son `navigate()`) |
| 3. `comparar_vistas` con datos reales | **las once pestañas idénticas**, `dashboard` incluido (903 caracteres), sin `undefined`/`NaN`/errores |
| 4. Parámetros tipados en el mismo PR | sí; entra en `tsconfig.estricto.json` |
| 5. Pruebas de interacción | sin manejadores; **10 pruebas de la vista** (mes del reloj, cargos sumados al gasto, patrimonio sin recalcular, niveles de los avisos, límite de tres tarjetas y préstamos, mensajes vacíos) + la de la ruta desconocida |
| 6. Revisión manual con tus datos | los **dos enlaces del Dashboard** («Detalles» de tarjetas y «Deudas») ejercidos en las dos versiones: mismo título, misma pestaña activa; y **una ruta inexistente cae en el Dashboard con el mismo texto** en las dos |

**Más controles:** seis mutaciones (vista sin registrar, `default:` sin buscar el Dashboard, gastos de todos los meses, cargos sin sumar, patrimonio recalculado en la vista, nivel del aviso equivocado): las seis se detectan (la sexta, tras reforzar la prueba). **En la aplicación empaquetada**: cuatro vistas registradas, una ruta inexistente cae en el Dashboard y sin error.

### PR 5 — `capital` (1.56.0)

| Qué | Previsto (medición del agente) | Medido |
|---|---|---|
| Métodos que se mueven | 7 | 7 (`render` y seis manejadores) |
| Líneas de la vista | 294 | **297** quitadas de `ui.ts` (294 de métodos y comentarios) |
| Líneas a reescribir | 48 (≈55 con la corrección del 15 %) | **58** de 272 (**21 %**): `render` 6 %; las altas y bajas, entre el 41 % y el 71 % |
| Diff de `ui.ts` | — | **−297 / +0** |
| Archivos nuevos | — | `vistas/capital.ts` (395), `pruebas/js/vistas/capital.test.js` (206); `registro.ts` +4 |
| Esfuerzo relativo (efectivo = 1) | 2 | ≈2 |
| Parámetros sin tipo en `ui.ts` | 161 | **146** (−15: los manejadores de esta vista) |

**Lo previsto y lo que ocurrió:** el modelo anticipaba «3 diálogos, 13 llamadas a la API» y **acertó**: tres diálogos (las confirmaciones de las bajas, ahora el servicio `Dialogos`, que ya existía desde el PR 0 y se usa por primera vez) y trece llamadas a la API (siete lecturas y seis escrituras). Lo que no mencionaba: **un `Date.now()`** que el modelo no mencionaba (el identificador de un bien nuevo): se inyectó el reloj en vez de dejar una fuente de tiempo global dentro de la vista. El capital es JSON libre: la vista declara solo lo que lee (`Certificado`, `InversionDeBolsa`, `Bien`).

**Una corrección a mi propio texto:** la primera versión del comentario de cabecera de la vista decía que mostraba los totales del capital. **No los muestra**: los ven el Dashboard y el Resumen. Se corrigió antes de publicar; el hecho de que la pantalla de capital no calcule ni muestre ninguna suma es justo lo que una de las pruebas de la vista fija. La relación del capital con `Dinero` y los huecos de esta pantalla están explicados en `capital_y_dinero.md`.

**Los seis controles del plan:**

| Control | Resultado |
|---|---|
| 1. `npm test` y `npm run tipos` | 277 pruebas (273 pasan, 4 `todo` conocidos, 0 fallan); 0 errores de tipos |
| 2. `manejadores.test.js` | pasa; los seis manejadores siguen existiendo (ahora en la vista) |
| 3. `comparar_vistas` con datos reales | **las once pestañas idénticas**, `capital` incluida (655 caracteres), sin `undefined`/`NaN`/errores |
| 4. Parámetros tipados en el mismo PR | sí; entra en `tsconfig.estricto.json` |
| 5. Pruebas de interacción | las del capital (a través de `appUI`) **sin tocar**, más 14 de la vista con dobles |
| 6. Revisión manual con tus datos | **cinco de los seis formularios**, ejercidos en las dos versiones sobre tu documento real: mismos documentos guardados (comparados sin imprimirlos), mismo texto de los avisos, 0 errores. La baja de certificado no se pudo ejercer: **tu capital no tiene certificados**; la cubren las pruebas de la vista |

**Dos precisiones de método, para no repetir mis tropiezos:** (1) en un campo `type="number"` un valor con espacios se **sanea a vacío**: mis primeras pruebas manuales con ` 123.455 ` no fallaron por el `step`, sino por el campo `required` vacío; se repitieron con valores limpios y se comprobó aparte que **tres decimales los detiene el `step`** (0 guardados, igual en las dos versiones). (2) El `.trim()` de la vista sobre un campo numérico no tiene efecto en el navegador; se mantiene porque era el comportamiento original.

**Más controles:** siete mutaciones (monto como número, baja del elemento siguiente, identificador de la fecha real en lugar del reloj, vista sin registrar, baja de bien invertida, bolsa guardada entre los certificados, baja sin confirmación): las siete se detectan. **En la aplicación empaquetada**: cinco vistas registradas, se dibuja «Capital y Activos» sin error y los seis manejadores son funciones.

### PR 6 — `suscripciones` (1.57.0)

| Qué | Previsto (medición del agente) | Medido |
|---|---|---|
| Métodos que se mueven | 10 | 10 (`render`, seis manejadores, el editor modal y dos ayudantes de fechas) |
| Líneas de la vista | 366 | **369** quitadas de `ui.ts` (366 de métodos y comentarios) |
| Líneas a reescribir | 51 (≈59 con la corrección del 15 %) | **59** de 317 (**18 %**): `render` 3 %, el editor modal 13 %, los manejadores entre el 26 % y el 80 % |
| Diff de `ui.ts` | — | **−369 / +0** |
| Archivos nuevos | — | `vistas/suscripciones.ts` (446), `pruebas/js/vistas/suscripciones.test.js` (231); `registro.ts` +4, `servicios.ts` +23, `cargar_interfaz.js` +6 |
| Esfuerzo relativo (efectivo = 1) | 3 | ≈3: un servicio nuevo, un modal y tres diálogos |
| Parámetros sin tipo en `ui.ts` | 146 | **134** |

**Lo que estrena esta vista:**
* **El servicio `Modales`.** `ui.ts` repite nueve veces el mismo bloque (crear la capa `modal-overlay`, ponerle un identificador y un contenido, añadirla a `<body>`). El editor de suscripciones es el primero en moverse, y en lugar de arrastrar `document` a la vista se creó `Modales.abrir(id, html)`: las ocho extracciones que quedan con modal ya tienen su servicio. El doble de pruebas registra cada ventana abierta y la declara en el DOM falso, así que una prueba la encuentra y la cierra por identificador.
* **`Motivo` y `Dialogos` (con `prompt`) en uso real.** `descartar un período` pide un motivo; `corregir el próximo cobro` usa `prompt`; asentar y dar de baja usan `confirm`.
* **Un choque de nombres que el compilador atrapó:** el servicio se llamaba `motivo` y el manejador ya tenía una variable `motivo` (la respuesta). Se renombró el servicio localmente (`pedidorDeMotivo`).

**Una trampa de la transformación mecánica, anotada para las siguientes:** la sustitución `elemento(` → `dom.elemento(` **no debe tocar el HTML**. El editor lleva `onclick="elemento('modal-edit-sus-…').remove()"` dentro de la plantilla: es código que corre en el navegador, no en la vista, y reescribirlo lo habría roto. El script protege ahora los atributos `on…=`. También: la plantilla llamaba a `appUI.fechaAIso(...)` (global) y en la vista pasa a `this.fechaAIso(...)`, porque el ayudante ahora es privado de la vista.

**Los seis controles del plan:**

| Control | Resultado |
|---|---|
| 1. `npm test` y `npm run tipos` | 295 pruebas (291 pasan, 4 `todo` conocidos, 0 fallan); 0 errores de tipos |
| 2. `manejadores.test.js` | pasa, incluido el `onclick` con comillas simples que recibe el objeto en JSON |
| 3. `comparar_vistas` con datos reales | **las once pestañas idénticas**, `suscripciones` incluida (1 825 caracteres), sin `undefined`/`NaN`/errores |
| 4. Parámetros tipados en el mismo PR | sí |
| 5. Pruebas de interacción | las existentes, a través de `appUI`, sin tocar, más 18 de la vista con dobles (alta, edición con validaciones, modal, baja, corregir fecha, asentar, descartar con motivo, puente) |
| 6. Revisión manual con tus datos | **alta, editor modal, edición y baja**, ejercidos en las dos versiones: mismos comandos y argumentos, **mismo modal** (identificador, clase y campos), cerrado tras guardar, mismos avisos, 0 errores. **No se pudo ejercer** corregir fecha ni asentar/descartar: tus suscripciones no tienen ninguna parada ni períodos pendientes; las cubren las pruebas de la vista |

**Más controles:** ocho mutaciones (monto como número, aviso que vuelve al campo viejo `fecha_renovacion`, modal sin su identificador, edición que no cierra la ventana, descartar sin comprobar el motivo, baja sin confirmación, vista sin registrar, fecha sin convertir): las ocho se detectan, una por el compilador. **En la aplicación empaquetada**: seis vistas registradas, se dibuja sin error, **el modal real se crea en `<body>` con la clase `modal-overlay`** y los siete manejadores son funciones.

### PR 7 — `gastos` (1.58.0)

| Qué | Previsto (medición del agente) | Medido |
|---|---|---|
| Métodos que se mueven | 9 | 9 (`render`, seis manejadores, `formatMonthYearStr` y la previsualización de la tasa) |
| Líneas de la vista | 480 | **484** quitadas de `ui.ts` (479 de métodos, el `case` y el campo `selectedGastosMonth`) |
| Líneas a reescribir | ≈66 | **67** de 439 (**15 %**): `render` 6 %, los manejadores entre el 28 % y el 58 % |
| Diff de `ui.ts` | — | **−484 / +0** |
| Archivos nuevos | — | `vistas/gastos.ts` (556), `pruebas/js/vistas/gastos.test.js` (265); `registro.ts` +4 |
| Esfuerzo relativo (efectivo = 1) | 4 | ≈4: tres vías de alta, un modal y estado propio |
| Parámetros sin tipo en `ui.ts` | 134 | **121** |

**Lo que estrena esta vista:**
* **Estado propio.** `selectedGastosMonth` deja de ser un campo de `AppUI` y pasa a ser privado de la vista. La prueba de interacción que lo leía desde fuera fijaba la implementación: se reescribió para comprobar el comportamiento (la opción elegida sale `selected`, el resumen cambia de mes y solo se hacen lecturas).
* **Reloj inyectado** para el mes por defecto, y el modal de liquidación por `Modales.abrir`.

**Los seis controles del plan:**

| Control | Resultado |
|---|---|
| 1. `npm test` y `npm run tipos` | 314 pruebas (310 pasan, 4 `todo` conocidos, 0 fallan); 0 errores de tipos |
| 2. `manejadores.test.js` | pasa; los siete manejadores de la plantilla están en el puente |
| 3. `comparar_vistas` con datos reales | **las once pestañas idénticas**, `gastos` incluida (26 435 caracteres) |
| 4. Parámetros tipados en el mismo PR | sí |
| 5. Pruebas de interacción | la de elegir mes, reescrita; más 19 de la vista con dobles |
| 6. Revisión manual con tus datos | selector de mes (4 meses), visibilidad del método de pago, previsualización de la conversión y **las tres altas** (efectivo, tarjeta y transferencia con divisa cruzada): mismos argumentos y avisos en las dos versiones, 0 errores. **No se pudo ejercer** la liquidación de un consumo (modal y envío): tus datos no tienen consumos pendientes de conversión; la cubren las pruebas |

**Más controles:** ocho mutaciones detectadas y una **equivalente** (`cuenta_ahorro_id: cue` siempre: `cue` ya es nulo salvo en transferencia, el comportamiento no cambia). Una mutación sobrevivió al principio (`tarjeta_id` siempre enviado) porque el DOM real conserva los selectores ocultos: se añadió la prueba «los selectores ocultos no cuentan». **En la aplicación empaquetada**: siete vistas registradas, se dibuja «Gastos y Egresos» sin error, el campo ya no existe en `AppUI`, el modal real se crea en `<body>` y los siete manejadores son funciones. La persistencia del mes **no** se comprobó ahí: la base temporal no tiene gastos, el selector no ofrece el mes y el resultado no demostraba nada; la cubren las pruebas y la comparación con datos reales.

**Pendiente que sigue a la vista:** `handleAgregarGasto` aún envía el monto como `Number`; pasarlo a texto (convención 1.21.0) es un PR aparte, un comando por PR.
