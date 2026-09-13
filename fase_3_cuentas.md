# Fase 3 — Cuentas de ahorro y transferencias

Este documento cubre la extracción del vertical de **Cuentas** al núcleo hexagonal, siguiendo el mismo protocolo que la Fase 1: primero se fija la conducta vigente con pruebas de caracterización, después se extrae, y **ningún hallazgo se corrige durante la extracción**.

---

## 1. Por qué este vertical y por qué ahora

El plan situaba Cuentas en la Fase 3 y así se mantiene, pero conviene decir por qué sigue siendo la prioridad después de haber ejecutado préstamos —que era la Fase 6— fuera de orden.

De los cuatro verticales que todavía no tienen módulo de dominio —cuentas, ingresos, suscripciones y capital—, **Cuentas es el que mueve saldos entre dos entidades a la vez**. Una transferencia toca dos filas y escribe un asiento; es la única operación del sistema con tres efectos que deben ocurrir juntos o no ocurrir. Los demás verticales mueven un saldo o ninguno.

Además es el vertical del que ya salieron dos hallazgos por la puerta de al lado: H2 —divisas que no se comparan— y H3 —la caja localizada por su nombre— aparecieron mirando gastos, pero viven en cuentas.

---

## 2. Estado antes de la extracción

Seis comandos, todos con SQL literal dentro del manejador de Tauri:

| Comando | Qué hace |
|---|---|
| `obtener_cuentas` | Lectura |
| `crear_cuenta` | Alta |
| `eliminar_cuenta` | Baja, con una guarda |
| `transferir_entre_cuentas` | Dos saldos y un asiento, en una transacción |
| `obtener_transacciones_cuentas` | Lectura con dos `JOIN` |
| `eliminar_transaccion_cuenta` | Revierte los dos saldos y borra el asiento |

---

## 3. Hallazgos

Cuatro. Se documentan y se fijan con pruebas; **no se corrigen en esta fase**.

### H10 — La reversión de una transferencia recorta el destino en cero

`main.rs`, en `eliminar_transaccion_cuenta`. Al revertir, el origen recibe de vuelta `monto + cargo` **sin límite**, pero al destino se le descuenta con `MAX(0.0, balance_actual - ?)`.

Si el titular gastó parte de lo recibido antes de advertir el error de registro, la diferencia **desaparece sin dejar rastro**: el origen recupera todo y el destino se queda en cero en lugar de en negativo. Crear y revertir dejan de ser operaciones inversas.

> Es **la misma forma exacta que H5**, que ya se resolvió en el balance de tarjetas admitiendo saldo a favor. Existe precedente de decisión, pero la decisión hay que tomarla igual: aquí lo que quedaría en negativo es una cuenta de ahorro, no una deuda, y un saldo negativo en una cuenta significa otra cosa —un sobregiro— que un saldo negativo en una tarjeta.

Fijado por **C48**.

### H11 — Una transferencia de una cuenta a sí misma se acepta

No se comprueba que origen y destino difieran. La operación resta `monto + cargo` y suma `monto` sobre la misma fila: el saldo queda alterado exactamente por el cargo y el asiento no representa ningún movimiento real.

Fijado por **C49**.

### H12 — No se comprueba la divisa del importe de destino

El importe de destino se acredita tal cual, sin verificar que corresponda a la divisa de esa cuenta, y la tasa de cambio se deduce dividiendo los dos importes. Una transferencia entre divisas en la que el importe de destino venga en la divisa equivocada se registra sin aviso y con una tasa de 1.

Es H2 en el vertical donde H2 realmente vive. En gastos quedó cerrado al convertirse el importe en un `Dinero`; aquí sigue abierto.

Fijado por **C50**.

### H13 — Borrar una cuenta arrastra su historial sin revertir saldos

La guarda de `eliminar_cuenta` cuenta los **gastos** que referencian la cuenta, pero no las **transferencias**. Como la clave foránea de `transacciones_cuentas` es `ON DELETE CASCADE`, borrar una cuenta que participó en transferencias elimina esos asientos en silencio — y los saldos de las contrapartes no se revierten: la otra cuenta conserva el dinero recibido sin que quede constancia de dónde salió.

Fijado por **C51**.

### H14 — Los importes pueden no cuadrar dentro de la misma divisa

Apareció al construir el tipo, no al leer el código: dentro de una misma divisa nada impide acreditar al destino algo distinto de lo que salió del origen. El cargo no lo explica —ese sale aparte, a cuenta del origen—, de modo que la diferencia **hace aparecer o desaparecer dinero entre las dos cuentas**.

No se corrige al extraer. `Transferencia` lo admite y lo expone con `importes_cuadran()` y `descuadre()`, para que el caso de uso pueda actuar cuando se decida qué hacer. Rechazarlo en el constructor habría sido cambiar la conducta durante una extracción, que es justo lo que el protocolo prohíbe.

### Lo que no es un hallazgo

**Una transferencia puede dejar el origen en negativo.** No se comprueban fondos. Se fija como conducta vigente en **C53** sin llamarlo defecto: decidir si un sobregiro es legítimo corresponde al titular y a su banco, no al tipo de dato.

---

## 4. Pruebas de caracterización (C45–C54)

| Prueba | Qué fija |
|---|---|
| C45 | La transferencia mueve los dos saldos y **el cargo lo paga el origen** |
| C46 | La tasa se deduce como `monto_destino / monto_origen` |
| C47 | Revertir restituye monto y cargo exactamente |
| C48 | **H10** — el destino se recorta en cero |
| C49 | **H11** — se acepta una transferencia a la misma cuenta |
| C50 | **H12** — no se comprueba la divisa del destino |
| C51 | **H13** — borrar la cuenta se lleva el historial |
| C52 | La guarda de borrado **sí** cubre los gastos |
| C53 | El origen puede quedar en negativo (conducta, no defecto) |
| C54 | Revertir dos veces falla la segunda, sin doble restitución |

### Que las pruebas discriminan

Se comprobó introduciendo dos defectos deliberados y verificando que fallan las pruebas correctas, no cualquiera:

| Mutación | Pruebas que fallaron |
|---|---|
| Retirar el recorte del destino al revertir | C48, y solo C48 |
| Cobrar el cargo al destino en vez de al origen | C45 y C47 |

Una prueba de caracterización que no falla cuando la conducta cambia no fija nada.

---

## 5. El dominio (3.2)

`dominio::cuenta` convierte la transferencia en un tipo que **no se puede construir mal**.

`ExtremoCuenta` lleva el identificador **y la divisa** de cada lado. Esa es la pieza que lo hace posible: sin la divisa a mano no hay nada contra lo que validar el importe.

`Transferencia::nueva` rechaza lo que no sería una transferencia:

| Condición | Hallazgo que cierra |
|---|---|
| Origen y destino distintos | **H11** |
| Cada importe en la divisa de su extremo | **H12** |
| El cargo en la divisa del origen, porque lo paga el origen | — |
| Importes positivos, cargo no negativo | — |

**H12 no se corrige: se vuelve irrepresentable.** No hay forma de acreditar pesos a una cuenta en dólares si el tipo lo impide. Es el mismo camino por el que se cerró H2.

Dos métodos concentran reglas que estaban repartidas:

* `debito_al_origen()` devuelve el importe **más el cargo**, ya con signo negativo. Que el cargo lo pague el origen estaba duplicado entre el comando que transfiere y el que revierte; ahora se dice una vez.
* `tasa()` devuelve `None` cuando no hay cambio de divisa, en vez del `1.0` que el código actual guarda y que no significa nada.

Y una propiedad que hace de red: **aplicar el débito y el crédito conserva el dinero salvo el cargo.** Si esa igualdad se rompe, la transferencia dejó de serlo.

---

## 6. Lo que viene, y en qué orden

1. ~~**3.1 — Caracterización.**~~ Cerrado por C45–C54.
2. ~~**3.2 — Dominio.**~~ Cerrado por `dominio::cuenta`.
3. **3.3 — Puertos.** `RepositorioTransferencias` junto al `RepositorioCuentas` que ya existe, más el doble en memoria y la incorporación al contrato compartido.
4. **3.4 — Casos de uso.** `TransferirEntreCuentas` y `RevertirTransferencia` sobre los puertos.
5. **3.5 — Adaptador y traducción.** Los seis comandos quedan reducidos a traducción, como ocurrió con `crear_gasto` en la Fase 1.7.
6. **Decisión de H10, H13 y H14**, por separado y cada una explícita.

**H11 y H12 quedan cerrados por el tipo.** No habrá que acordarse de comprobarlos.

**H13 es una invariante de guarda** y se resuelve en el caso de uso de borrado.

**H10 y H14 requieren tu decisión.** H10 tiene la misma forma que H5 pero no necesariamente la misma respuesta: allí lo que quedaba en negativo era una deuda y significaba saldo a favor; aquí sería una cuenta de ahorro, y eso es un sobregiro. H14 es nuevo y hay al menos tres respuestas razonables — rechazar el descuadre, admitirlo asentando la diferencia como un cargo, o admitirlo y avisar.
