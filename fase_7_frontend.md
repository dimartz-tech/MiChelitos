# Fase 7 · Frontend — análisis

Estado: **análisis, sin cambios de código. Las decisiones que pide están al final y son del titular.**
Fecha: 2026-09-29. Todo lo que sigue se midió sobre `main` (1.41.0); nada usa datos vivos.

La fase tiene tres trabajos que el plan (§4.3, §4.4, §8, §11) dejó juntos: **dividir `ui.js`**, **decidir el tipado** y **las mejoras de usabilidad**. Aquí se separan, porque tienen riesgos distintos y no hace falta decidirlos a la vez.

## 1. Punto de partida

| Medida | Valor |
|---|---|
| `src/js/ui.js` | 4 697 líneas, una clase (`AppUI`) con **103 métodos** y una instancia global `appUI` |
| Resto del frontend | `api.js` 419, `app.js` 91, `nucleo/` 4 archivos (dinero, respaldos) |
| Métodos de vista | 11 `render<Pestaña>` (de 104 a 395 líneas cada uno; ajustes 395, tarjetas 343, gastos 293) |
| Manejadores | 49 `handle*` (1 112 líneas); 42 métodos «otros» (1 258 líneas) |
| Enlace HTML → JS | **72 atributos inline** (`onclick`, `onsubmit`, …) que llaman a **64 métodos** distintos de `appUI` |
| Estado de la instancia | solo `contentContainer`, `notifContainer`, `selectedGastosMonth`, `_menuPasivoAbort` |
| Acceso al DOM | 232 `getElementById`, 39 asignaciones a `innerHTML` |
| Diálogos nativos | 18 `confirm(`, 7 `prompt(` |
| Pruebas del frontend | solo `nucleo/` y los dos contratos (`ipc`, `importes`); **el código de vistas no tiene ninguna** |

Lo que más importa para dividir: **el acoplamiento entre pestañas es pequeño.** Los métodos que otros métodos usan de verdad son cuatro: `showToast` (53 usos), `render` (47), `formatMoney` (26) y `pedirMotivoDeCorreccion` (8). Todo lo demás lo usa una sola vista. Hay, pues, una capa compartida diminuta y once vistas casi independientes.

## 2. La restricción previa del plan, resuelta hoy

El plan dejó abierto el **paso 0.8**: comprobar que el WebView de la aplicación empaquetada carga `<script type="module">`. Sin eso no se podía ni plantear la división. **Se comprobó hoy, en la aplicación empaquetada**: una copia temporal con un módulo de prueba, ejecutada con un `HOME` aislado (los datos reales no se tocaron), y el módulo se anotó en la base temporal.

| Prueba dentro de un módulo | Resultado |
|---|---|
| `import { … } from './js/nucleo/dinero.js'` | funciona |
| `import()` dinámico | funciona |
| `await` de nivel superior | funciona |
| `window.__TAURI__.invoke` disponible | funciona (y el comando se ejecutó) |

Conclusión: **los módulos ES nativos sirven, sin bundler ni paso de compilación**, como preveía el plan. Queda la consecuencia conocida: un módulo no crea globales, así que los 72 manejadores inline necesitan un puente (ver §3).

## 3. Dividir `ui.js`

### Opciones
1. **Módulos ES por vista** (lo del plan). Cada pestaña es un módulo que exporta sus métodos. Los manejadores inline dejan de encontrar `appUI`, así que hace falta un puente `window.appUI` que los reúna, o reescribir los 72 enlaces con `addEventListener` (cambio mucho mayor y de riesgo mucho mayor).
2. **Scripts clásicos que amplían el prototipo** (`Object.assign(AppUI.prototype, {...})` en varios archivos). Sin depender de módulos y sin puente, pero conserva los globales y la carga por orden, y deja el tipado y las pruebas más difíciles.
3. **Módulos + puente, de una pestaña en una** *(recomendada)*. Como la 1, pero **incremental**: cada PR mueve una vista sin cambiar su comportamiento, y `app.js` monta `window.appUI` con lo que ya se ha extraído más el resto de la clase original. La aplicación funciona entera después de cada PR.

### Por qué la 3
* El plan ya concluyó que el enfoque «todo de golpe» es el que concentra el riesgo; la división por pestañas conserva la propiedad que ha servido en las fases anteriores: **un punto de salida seguro tras cada paso**.
* Es un **movimiento mecánico**: se cortan los métodos de una vista y se pegan sin editar su cuerpo. Los pasos que sí editan (el puente, la capa compartida) son pocos y pequeños.
* Se apoya en una red de seguridad que cuesta poco y hoy no existe: **una prueba de contrato que exija que los 64 métodos llamados desde el HTML existan en `window.appUI`**, en el mismo estilo que `ipc.test.js`. Es lo que se rompería en silencio al mover código: un `onclick` que apunta a algo que ya no está.

### Orden propuesto
0. **Infraestructura:** capa compartida (`toast`, formato de importes, `pedirMotivoDeCorreccion`), `app.js` como módulo, puente `window.appUI` y la prueba de contrato. Sin mover ninguna vista.
1. **Vistas de menor a mayor:** efectivo (104 líneas), préstamos (112), cuentas (141), dashboard (160), suscripciones (163), resumen (168), capital (193), ingresos (195), gastos (293), tarjetas (343), ajustes (395). Las últimas concentran más manejadores y más enlaces inline.
2. **Retirar** la clase original cuando quede vacía.

Son unos **13 PR pequeños**. Cada uno con su versión e historial, como el resto del proyecto.

### Lo que la división no resuelve
El código de vista **sigue sin pruebas**, y ninguna prueba estática comprueba que un `getElementById('x')` siga encontrando su elemento tras un movimiento. La red es: movimiento sin editar cuerpos, la prueba de contrato de los manejadores, y **una revisión de cada pestaña con el backend simulado en un navegador** (como se hizo con el botón de restaurar) antes de fusionar. Es manual. Automatizarla exigiría una dependencia de pruebas de navegador que el proyecto no tiene; queda como decisión (§6).

## 4. Tipado

El plan planteó tres opciones (A: JS puro, B: JSDoc con `@ts-check`, C: TypeScript) y cuatro criterios. Se **midió** con TypeScript 5.9 instalado fuera del repositorio (nada se añadió al proyecto), ejecutando la comprobación sobre el código **tal como está, sin una sola anotación**:

| Conjunto | Hallazgos (laxo) | Hallazgos (estricto) |
|---|---|---|
| `ui.js` + `api.js` + `app.js` (juntos, como se cargan) | 210 | 885 |
| `nucleo/dinero.js` | 0 | 12 |

De los 210 hallazgos en modo laxo:

* **193** son ruido del DOM: `getElementById` devuelve `HTMLElement | null` y luego se lee `.value`.
* **8** son por falta de tipo para `window.__TAURI__` (se resuelve con una declaración).
* **3** son «propiedad inexistente en `never`»: una variable inicializada a `null`.
* **3** «no encuentra el nombre»: globales que viven en otro script clásico. **Con módulos e `import` explícitos desaparecen**; es un argumento a favor de dividir primero.
* **3** de tipos que resultaron correctos: una resta de fechas (dos avisos en la misma línea) y un booleano guardado en `localStorage`.

**Ninguno era un defecto real.** Sin tipos, la comprobación no encuentra nada que valga la pena. Lo que el plan esperaba de B —detectar **campos ausentes en las respuestas IPC**— exige antes escribir los tipos de esas respuestas: hay **16 estructuras** que Rust devuelve al frontend, y `api.js` tiene más de un centenar de métodos.

### Criterios del plan, con datos
1. *¿B detecta los errores que aparecen en este código?* **Sin anotar, no.** Con anotaciones no está medido; hay que medirlo antes de decidir (ver piloto).
2. *¿La verbosidad degrada la legibilidad?* Sí en las funciones de render de cientos de líneas con plantillas de HTML; poco en `api.js`, `nucleo/` y la capa compartida.
3. *¿C introduce `node_modules` de producción o un artefacto generado?* Sí. **C queda descartada**, como preveía el plan.
4. *A es legítima.* Lo es.

### Recomendación
* **Ahora: A**, sin tipar, y **no** meter esto en la división: son decisiones independientes y mezclarlas alarga cada PR.
* **Después de la división, un piloto acotado de B**: tipar las 16 estructuras IPC, `api.js`, `nucleo/` y la capa compartida (archivos pequeños, opt-in con `@ts-check`, sin tocar las vistas), y ejecutar la comprobación sobre las dos o tres primeras vistas extraídas. Si encuentra defectos reales, se extiende; si solo encuentra ruido, se cierra declarándolo innecesario, como los tramos 3 y 4b del redondeo. TypeScript sería una dependencia **de desarrollo**, sin efecto en el `.dmg`.

## 5. Mejoras de usabilidad (§8 del plan)

Solo se anota lo que se **verificó hoy** en el código; lo demás figura como no verificado.

| # | Mejora | Situación verificada |
|---|---|---|
| 1 | Sustituir el `prompt()` de tasa de cambio | **Pendiente**: sigue `prompt()` en el abono en dólares desde una cuenta en pesos; hay 7 `prompt(` en total |
| 7 | Formato monetario unificado | **Cumplido en la práctica**: `formatMoney`, 113 usos, sin `Intl` directo; no está en `nucleo/` |
| 8 | Accesibilidad | **Pendiente**: **0** `aria-label`, **18** botones solo con emoji |
| 9 | Recordar el mes seleccionado | **Parcial**: solo existe el estado de Gastos (`selectedGastosMonth`) |
| 2–6 | Vista previa, reversión contextual, confirmación con impacto, errores tipados, estados vacíos | **No verificados** en esta pasada (varias existen de hecho: las reversiones y su motivo, y previas en algunos formularios) |

La de accesibilidad (#8) es la de mejor relación entre esfuerzo y beneficio: son 18 botones y no cambia ningún comportamiento. Conviene hacerla **antes** de dividir, porque después habría que repartirla entre once módulos.

## 6. Decisiones que se piden

1. **División**: ¿la opción 3 (módulos + puente, una pestaña por PR, con la infraestructura primero)?
2. **Tipado**: ¿A ahora y piloto de B después de dividir, como se propone?
3. **Usabilidad**: ¿se hace #8 (accesibilidad) antes de dividir, y #1 (el `prompt()` de la tasa) como parte de la vista de tarjetas?
4. **Verificación de vistas**: ¿basta la revisión manual con backend simulado, o se acepta una dependencia de desarrollo para pruebas de navegador?

## 7. Qué no cambia
Los datos, el backend y la aplicación instalada no se tocan en esta fase. Nada de esto altera saldos ni esquema.
