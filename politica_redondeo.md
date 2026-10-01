# Política de redondeo

Cómo se decide un céntimo en este sistema, y por qué. Se abordó en cuatro
tramos; este documento recoge el estado de cada uno y **las decisiones que
cambiaron sobre la marcha**, que son la parte útil.

## El problema

Sumar dinero expresado en centavos es exacto. El riesgo no está ahí, sino al
**producirlos**: un porcentaje, una conversión o un interés generan fracciones
de céntimo que hay que decidir.

## Tramo 1 — el céntimo se decide en enteros ✅

`dividir_redondeando` es el único lugar del sistema donde se decide un
céntimo. Redondea la mitad alejándose de cero, en aritmética entera.

**La regla anterior no era la que decía ser.** `f64::round()` aplica esa misma
regla, pero sobre el valor binario y no sobre el decimal escrito: `1.005` se
guarda como 1.00499… y bajaba, mientras que `2.675` —cuyo error se cancela al
multiplicar por 100— subía. Dos importes de la misma forma, en direcciones
opuestas, por un accidente de representación.

`Porcentaje` y `TasaCambio` pasaron a ser enteros con escala declarada.

### Tolerancia de representación: 0.01 centavos

Acota **una** de las dos fuentes de desviación, y confundirlas lleva a
rechazar operaciones normales:

| Fuente | ¿Acotable? | Magnitud |
|---|---|---|
| Redondeo final al céntimo | **No** — es una decisión, no un error | hasta 0.5 ¢ por definición |
| Representación de la tasa | **Sí** — depende de la escala | la fija la tolerancia |

La tolerancia obligó a subir la escala de los porcentajes de millonésimas a
mil-millonésimas.

## Tramo 2 — la reversión devuelve lo guardado ✅

Deshacer una operación es reponer **lo que ocurrió**, no lo que hoy creemos
que debió ocurrir. Si la regla de redondeo cambia, o un importe se corrigió a
mano, recalcular deja un residuo que nadie ve.

Corrige una decisión anterior de este mismo proyecto, que recalculaba y lo
argumentaba. El argumento era malo, y se vio en el único caso real que se puso
a prueba.

## Tramo 3 — la tasa persistida como entero ❌ **descartado**

Era el siguiente paso previsto: sacar `f64` también de las columnas de tasa,
que SQLite guarda como `REAL`.

**No se hace, porque no compra nada.** Se comprobó antes de implementarlo: una
tasa sobrevive intacta al viaje `entero → REAL → entero`. A escala de
mil-millonésimas, una tasa del rango plausible necesita unos doce dígitos
significativos y `f64` ofrece quince. La holgura es amplia, y lo mismo vale
para los porcentajes de interés.

Lo que sí hacía falta era **proteger esa holgura**, porque es la razón de que
el tramo sobre. Dos pruebas de barrido lo fijan: si algún día se subiera la
escala hasta agotarla, el viaje empezaría a perder micro-unidades y se sabría
ahí, antes de que un importe se desviara.

Una migración de tres columnas, su relleno y su verificación, sustituidos por
dos pruebas. **El tramo se cierra declarándolo innecesario, no haciéndolo.**

## La regla: toda columna `REAL` está clasificada

En un esquema migrado, **cada columna `REAL` tiene que estar declarada** en
`COLUMNAS_DE_DINERO` —y entrar en el redondeo al céntimo— o en
`COLUMNAS_DE_TASA`. Ninguna puede quedarse sin clasificar, y una prueba lo
exige.

Existe porque la guardiana anterior protegía en un solo sentido: impedía meter
una tasa entre el dinero, pero **no impedía olvidar una columna de dinero**.
Cuatro se olvidaron por ahí —todas añadidas en migraciones posteriores a la 3,
que es la que redondea y verifica— y una de ellas,
`cuentas_ahorro.comision_pago_impuestos`, se escribía además sin pasar por
`Dinero`: era la única vía por la que un importe llegaba a la base sin que el
núcleo decidiera su céntimo.

El coste de cumplir la regla es una línea en una lista. El de no tenerla ya se
pagó.

## Tramo 4a — el IPC en texto ✅

El usuario decide el céntimo con **sus dígitos**. `Dinero::desde_texto`
redondea sobre lo escrito, y no sobre el `f64` en que se había convertido:
`1.005` sube a 1.01 en vez de bajar a 1.00.

Salió de investigar este tramo, y el argumento no estaba en el planteamiento:
**los tramos 1 y 2 no eliminaron la conversión desde coma flotante, la
centralizaron.** El defecto que `dividir_redondeando` documenta vivía también
en la puerta de entrada.

El alcance se midió antes de aplicarlo: 52 de 53 campos llevan `step="0.01"`,
que el navegador valida al enviar, y con dos decimales el viaje
`texto → Number → texto` no pierde nada. Solo tres importes esquivan esa
validación, y son los tres migrados. Los cuarenta y un parámetros restantes
son trabajo mecánico que el `step` ya cubre.

## Tramo 4b — columnas `INTEGER` ❌ **descartado**

Era la otra mitad: guardar centavos enteros en lugar de unidades en `REAL`.
**No se hace, porque la premisa era falsa**, y solo se supo midiendo.

| Lo que se daba por supuesto | Lo que resultó |
|---|---|
| `INTEGER` impide guardar una fracción | Una columna `INTEGER` acepta `75.005` y lo guarda como `real`. La afinidad **solo convierte cuando no pierde**: no restringe nada |
| `REAL` pierde precisión en centavos | Exacto hasta 2,5·10¹⁶ centavos |
| Sumar en `REAL` acumula error | Un millón de filas mezclando magnitudes: desvío de **0,000000 ¢** |

El cambio de tipo no compraba precisión, ni rango, ni exactitud en las sumas.
Lo único que cambiaba era el modo de fallo, y en dos direcciones de tres lo
cambiaba **a peor**: leer un `INTEGER` como `f64` devuelve el entero crudo sin
error —cien veces el importe, en silencio— y escribir un `i64` en una columna
`REAL` hace lo mismo. Solo leer un `REAL` como `i64` falla en voz alta.

Es el mismo desenlace que el tramo 3: **se cierra declarándolo innecesario**,
y las mediciones quedan como pruebas para que el argumento siga
comprobándose.

## La restricción de céntimo ✅

Lo que sí hacía falta del tramo 4b era rechazar la fracción **al escribir**, y
no solo al migrar. Eso no necesitaba cambiar la unidad: es un `CHECK`.

```sql
CHECK (ROUND(v, 2) = v)
```

**No es una elección de estilo frente a las variantes con `* 100`.** Se
midieron las tres: las que multiplican por cien rechazan **céntimos
legítimos** —once en un barrido, empezando por 4,77— porque multiplicar
introduce el error que se pretendía detectar. `ROUND(v, 2) = v` no rechazó
ninguno en doscientos mil valores densos ni por magnitudes hasta 10¹⁴.

Añadirla costó dos cosas que solo aparecieron al probar contra datos reales:

**Doce importes reales no pasaban.** No eran fracciones —su fracción medida
era cero— sino ruido de representación de ~10⁻¹². Las migraciones 3 y 9
redondean con una tolerancia de `1e-6` y los saltaban. La migración 10
redondea por **exactitud**, con el mismo criterio que impone el `CHECK`: usar
uno en la migración y otro en la restricción es la vía a un esquema que no
admite sus propios datos.

**La aplicación habría dejado de funcionar.** `SET v = v + ?` suma en coma
flotante, y el 13 % de esas escrituras produce un valor que el `CHECK`
rechaza; la quinta ya fallaba. De ahí venían los doce. Las nueve
acumulaciones en SQL pasaron a `ROUND(v ± ?, 2)`, que con los dos operandos
exactos corrige como mucho 1,5·10⁻⁵ unidades —por debajo de la tolerancia de
representación— y por tanto **nunca decide un céntimo**. Una prueba recorre
las fuentes para que no reaparezca una acumulación sin redondear.

### Lo que costó en el esquema

SQLite no tiene `ALTER TABLE ADD CONSTRAINT`: hay que rehacer la tabla. Son
**doce**, y la migración 11 las reconstruye insertando restricciones *de
tabla* antes del paréntesis de cierre, en vez de analizar la definición de
cada columna.

Con `foreign_keys` encendido, `ALTER TABLE ... RENAME` reescribe las cláusulas
`REFERENCES` de las demás tablas —**y lo hace aunque `legacy_alter_table` esté
activo**, que es el detalle que ninguna suposición previa contenía: el
`PRAGMA` se lee como encendido y no surte efecto—. Renombrar `cuentas_ahorro`
dejaba a `gastos` apuntando a una tabla temporal que la propia migración
borra, y la suite entera se cayó de golpe.

De modo que las migraciones corren ahora con las claves ajenas apagadas, como
SQLite prescribe, y cada una termina con `foreign_key_check` **dentro de su
transacción**: la comprobación pasa de ser por sentencia a ser por migración,
que para un cambio de esquema es el grano correcto.

## Tramo 4c — los importes que todavía viajan como número (análisis, 2026-09-29)

Estado: **cerrado con la opción 1 (1.41.0): no se convierte, y la premisa queda fijada con una prueba.** Decisión del titular el 2026-09-29.

El tramo 4a dejó pendientes «cuarenta y un parámetros» de importe que llegan a
Rust como `f64`. Antes de convertirlos se midió qué exponen.

### Inventario
Dieciséis flujos envían algún importe como número (el análisis original contó doce por mirar solo los parámetros sueltos; los cuatro que llegan en una estructura —crear gasto, crear ingreso, crear y editar préstamo— pasan por los mismos formularios y quedan cubiertos igual). Los doce sueltos son: cobrar una factura formal e
informal, crear un ingreso informal, crear una tarjeta y actualizar sus
límites, abonar a una tarjeta, crear una cuenta, transferir (y retirar a
efectivo), corregir una factura, cobrar en efectivo, crear una bonificación y
liquidar un consumo pendiente. Además hay **tasas** como número
(`tasa_cambio`, `porcentaje_retencion`, `porcentaje` del avance): no son
dinero y no entran en este análisis.

Los doce se alimentan **solo** de campos de formulario. El recorrido de la
interfaz da esto:

| Hecho | Medida |
|---|---|
| Campos `type="number"` | 61 |
| Con `step="0.01"` (todo el dinero) | 52 |
| Otros: tasa (`0.0001`), días de facturación y cuotas | 9, ninguno es dinero |
| Formularios con `novalidate` | 0 |
| Manejadores que envían dinero | 16 de 16 por `onsubmit`, es decir, tras la validación del navegador |
| Importes calculados en JavaScript y enviados | 0 (el único derivado, el cobro parcial de una factura, ya viaja como texto) |

### Qué se pierde al viajar como número
Se midió el recorrido exacto que hace un importe: texto del campo → número JSON
→ `f64` → `(x · 100).round()` en `Dinero::nuevo`.

* **Con dos decimales no se pierde nada.** Barrido exhaustivo de 0,00 a
  2 000 000,00 (2·10⁸ importes) y 5·10⁷ importes más repartidos por
  magnitudes hasta ~10¹⁵ centavos: **0 discrepancias**.
* **Con más de dos decimales sí se decide distinto.** De cada 100 importes
  terminados en 5 en el tercer decimal, **6,6** los decide `f64` de forma
  diferente a la regla del sistema (mitad alejándose de cero sobre los
  dígitos): `1.005` baja a 1,00 en vez de subir a 1,01.

Es decir, **el camino por número solo falla si llega un importe con más de dos
decimales**, y eso es justo lo que `step="0.01"` impide al enviar.

### Comprobado en la aplicación empaquetada
La validación de `step` se apoya en el navegador. El titular escribió `1.005`
en un campo de importe de la aplicación empaquetada (WKWebView) e intentó
guardar: **el navegador mostró su aviso de valor no válido y no se guardó
nada.** La premisa se cumple donde importa, no solo en el código.

### Opciones
1. **No convertir y fijar la premisa con una prueba** *(recomendada)*. Una
   prueba de contrato sobre `ui.js` exige que todo campo de importe lleve
   `step="0.01"`, que ningún formulario use `novalidate` y que cada envío de
   dinero pase por `onsubmit`. Un campo nuevo sin `step` **falla la prueba**.
   Coste: una prueba. Es el mismo desenlace que los tramos 3 y 4b: se cierra
   declarándolo innecesario y la medición queda como comprobación.
2. **Convertir los doce comandos a `ImporteDecimal`** y enviar el texto del
   campo en lugar de `Number(...)`. Elimina la dependencia del navegador.
   Coste: unos cuarenta parámetros, doce comandos, sus envoltorios en `api.js`
   y las pruebas que los llaman; el riesgo de regresión es real y el beneficio
   solo aparece si el `step` fallara. Se puede hacer por lotes, un comando por
   PR, si más adelante se decide.
3. **Rechazar en Rust cualquier número con fracción de céntimo** en esos doce
   comandos, sin cambiar la interfaz. Defensa en profundidad barata, pero
   repartida en muchos puntos de entrada y sin la ventaja de tipo que da la
   opción 2.

### Decisión
**Opción 1**, como salvaguarda mientras dure la transición. **No sustituye a la
convención vigente**, acordada en 1.21.0: los importes viajan como **texto**, tal
cual se escribieron, y el número que `ImporteDecimal` admite hoy es transitorio;
cuando no quede ninguna llamada que lo envíe, esa rama se retira. La conversión
de los parámetros que aún viajan como número **sigue pendiente**, un comando por
PR; la opción 1 solo garantiza que, hasta entonces, ninguno trae fracción de
céntimo. (Una versión anterior de este párrafo decía que la conversión solo se
retomaría ante una vía nueva sin formulario: contradecía la convención y se
corrigió en 1.50.0. La primera excepción que se encontró fue la comisión del
alta de cuenta, que 1.21.0 daba por migrada y seguía enviándose como número.)

### Qué fija la prueba (`src/js/contrato/importes.test.js`)
* Todo campo numérico de la interfaz lleva `step="0.01"` o está **declarado por
  su id exacto** como no monetario, con su razón (una comparación por prefijo
  dejaría pasar `tar_cor_dop`, un importe, detrás de `tar_cor`, un día).
* La lista de no monetarios no tiene entradas obsoletas ni lleva `step` de
  céntimo.
* Ningún formulario usa `novalidate`.
* Los dieciséis envíos de dinero salen de un manejador enlazado con `onsubmit`
  —tras la validación—, y ninguno recibe un importe calculado en el navegador.
* Cinco mutaciones (quitar un `step`, añadir `novalidate`, calcular un importe,
  pasar un envío a `onclick`, dejar un id sin declarar): las cinco hacen fallar
  la prueba.

## Migración de los importes a texto (en curso)

La puerta de entrada ya acepta texto (`ImporteDecimal`, tramo 4a), pero **quedan comandos cuyos importes siguen
siendo un `f64`**: la interfaz los envía con `Number(...)` y el núcleo decide el céntimo sobre un número que ya no
vale lo que el titular tecleó. Se migran **uno por PR** (Rust: `ImporteDecimal` y `unidades()` al guardar; envoltorio:
`String(x)`; vista: el texto recortado, sin convertir), porque cada uno toca un comando de dinero y merece su propia
comprobación. La lista de lo que falta es una **prueba** (`pruebas/js/contrato/importes_texto.test.js`): solo puede
encogerse, y un importe nuevo que viajara como número rompería la prueba.

| Comando | Importes | Estado |
|---|---|---|
| `crear_suscripcion`, `actualizar_suscripcion` | `monto` | ✅ texto (1.2x) |
| `declarar_saldo_prestamo` | `saldo` | ✅ texto |
| `crear_cuenta`, `actualizar_cuenta` | `comision_pago_impuestos` | ✅ texto (el saldo inicial de `crear_cuenta`, no) |
| `actualizar_ingreso` | `cobro_parcial` | ✅ texto (el total, no) |
| `simular_avance_efectivo`, `registrar_avance_efectivo` | `monto`, `cargo_fijo` | ✅ texto |
| **`crear_ingreso_informal`** | `monto` | ✅ **texto (1.69.0)**: antes guardaba el número tal cual, sin que el núcleo decidiera el céntimo |
| `marcar_ingreso_pagado`, `marcar_informal_pagado` | `monto_recibido` | pendiente |
| `crear_cobro_efectivo_informal` | `monto` | pendiente |
| `crear_bonificacion` | `monto` | pendiente |
| `liquidar_consumo_pendiente` | `monto_liquidado` | pendiente |
| `registrar_pago_tarjeta` | `monto` (la tasa de cambio es una tasa, no un importe) | pendiente |
| `crear_cuenta` | `balance` | pendiente |
| `transferir_entre_cuentas` | `monto_origen`, `monto_destino`, `cargo` | pendiente |
| `actualizar_ingreso` | `monto_total` (el porcentaje de retención es una tasa) | pendiente |
| `crear_tarjeta` | ocho límites, sobregiros, balances y cortes | pendiente |
| `actualizar_limites_tarjeta` | seis importes y dos límites ajustados | pendiente |
| `crear_gasto` (`GastoInput`), `crear_ingreso` (`IngresoInput`), `crear_prestamo` y `actualizar_prestamo` (estructuras de entrada) | `monto`, `monto_total`, `monto_prestamo`, `monto_cuota`, `saldo_actual`, `limite_credito` | pendiente (requieren cambiar el tipo de los campos de la estructura) |

**Cuando no quede ninguno**, la rama «número» de `ImporteDecimal` se retira y la coma flotante deja de entrar por la
frontera (así lo dice `ipc.rs`). Los porcentajes y las tasas **no se migran**: son tasas, no importes, y su
representación se midió en el tramo 3.
