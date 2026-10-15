# Fase 2 — Tarjetas y abonos

Cierra el vertical que quedó abierto: el abono a tarjeta seguía siendo SQL
crudo dentro del comando de Tauri, y la prueba Gherkin de §6.4 —pedida en el
encargo inicial— nunca llegó a escribirse.

## 1. Por qué este vertical y por qué al final

El trabajo de tarjetas se fue haciendo a trozos —conversión de divisa,
liquidación pendiente, bonificaciones, reversión de abonos— a medida que hacía
falta. Cada pieza se hizo bien, pero **el registro del abono, que es la
operación central, se quedó sin extraer**. La reversión existía como caso de
uso y su contraria no.

Esa asimetría tenía una consecuencia concreta: la prueba Gherkin no tenía nada
que ejercitar. §6.4 la describe como el caso que «atraviesa el hexágono
completo», y sin caso de uso solo se podía probar contra el comando, es decir,
contra SQLite y Tauri a la vez.

## 2. Lo que cambia

| Antes | Ahora |
|---|---|
| 86 líneas de SQL en el comando | Caso de uso sobre puertos; el comando traduce |
| La tasa se aplicaba si llegaba | Se exige solo cuando las divisas difieren |
| Abono insertado y luego enlazado con `UPDATE` | Insertado completo, con su vínculo |
| Sin prueba legible por el titular | Cuatro escenarios en español |

## 3. H15 — divergencia declarada

El comando aplicaba la tasa **aunque no hubiera cambio de divisa**. Un abono en
pesos desde una cuenta en pesos con una tasa de 60 debitaba sesenta veces el
importe.

Era conducta conocida: el comentario de la Fase 1 decía «se conserva». Pero
**ninguna prueba la cubría** —todas pasaban tasa `0.0`—, y el campo de tasa
está siempre visible en la interfaz, así que era alcanzable tecleando.

La extracción la cambia. **No es una extracción neutra**, y por eso queda aquí
declarada y fijada por la prueba `h15_una_tasa_sobrante_ya_no_infla_el_debito`
en lugar de pasar inadvertida. La regla nueva es la correcta: convertir sin
cruzar divisas no significa nada, y el importe inflado salía de una cuenta
real.

## 4. La prueba Gherkin

**Archivo:** `features/abono_tarjeta_multidivisa.feature`
**Intérprete:** `src-tauri/src/gherkin.rs`
**Pasos:** `src-tauri/src/caso_de_uso_gherkin.rs`

### 4.1 Por qué un intérprete propio y no `cucumber`

El plan contemplaba la caja `cucumber` como dependencia de desarrollo. Se
descarta. El proyecto lleva **cero dependencias nuevas desde la Fase 0**, y el
criterio de peso contenido y auditable pesa más que la comodidad de un runner
completo. Lo que hace falta son cuatro escenarios sobre dobles en memoria, y
eso cabe en un archivo que cualquiera puede leer entero.

El intérprete soporta lo que el archivo usa —característica, antecedentes,
escenarios, los cuatro conectores y continuaciones de línea— y nada más. Sin
esquemas de escenario, tablas ni etiquetas: añadirlos sin un caso que los pida
sería construir un framework en vez de una prueba.

### 4.2 Qué protege de que pase por no hacer nada

Tres cosas, porque una prueba en verde que no ejecuta nada es peor que no
tenerla:

1. **El intérprete tiene sus propias pruebas.** Si analizara mal, los
   escenarios pasarían vacíos.
2. **Un paso no reconocido entra en pánico.** No hay forma de que una línea del
   archivo se ignore en silencio.
3. **Una prueba estructural** exige que haya cuatro escenarios y que ninguno
   quede sin pasos.

Verificado además por mutación: quitar la comisión del débito hace fallar los
escenarios 1 y 3; debitar la cuenta antes de tocar la tarjeta hace fallar el 4.

### 4.3 Qué prueba de verdad el escenario 4

Dice «atomicidad», y conviene ser preciso sobre su alcance. Se ejecuta sobre
los dobles en memoria, que no tienen transacciones. Lo que verifica es que el
caso de uso **resuelve primero lo que puede fallar y toca los saldos después**,
de modo que un fallo al guardar la tarjeta ocurre antes de mover la cuenta.

La atomicidad real frente a SQLite la da la transacción del comando, y eso es
otra garantía. El escenario protege la mitad que depende del orden, que es la
que un refactor puede romper sin darse cuenta.

### 4.4 Divergencia respecto al texto del plan

§6.4 esperaba la categoría `"Comisiones Bancarias"`. El sistema real anota las
comisiones bajo `"Otros"`, así que el archivo dice `"Otros"`. Cambiar el
sistema para que encajara con el documento sería mover el mundo para salvar el
mapa.

## 5. Lo que esta fase no hace

* No toca la interfaz: el formulario de abono sigue igual.
* No unifica el campo de tasa con la divisa de la cuenta. Hoy la interfaz
  muestra el campo siempre, y tras H15 escribir en él cuando no hay cruce es
  inofensivo pero inútil. Ocultarlo es trabajo de la Fase 7.
* No modela el cargo del banco **receptor** —el de balance mínimo—, que hoy no
  cabe dentro de una transferencia porque el cargo se define como pagado por el
  origen. Queda anotado como hueco de modelo.
