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

## 6. Puertos, casos de uso y traducción (3.3 – 3.5)

`RepositorioTransferencias` se suma a `RepositorioCuentas`, y el trait compuesto `AlmacenTransferencias` expresa lo que una operación sobre transferencias necesita. El adaptador SQLite y el doble en memoria lo implementan; el doble replica las rarezas en lugar de idealizarlas.

Dos métodos del puerto existen para que los hallazgos **se vean en el contrato** en vez de quedar enterrados en una consulta:

* `reducir_saldo_con_recorte` — el `MAX(0.0, …)` de **H10**, con nombre propio.
* `transferencias_que_referencian` — lo que la guarda de borrado **no** consulta (**H13**), listo para cuando se decida.

Los casos de uso `transferir`, `revertir_transferencia` y `eliminar_cuenta` orquestan sobre los puertos, y los tres comandos quedan reducidos a traducción: abrir la transacción, convertir números en importes de la divisa que corresponde a cada cuenta, y confirmar o deshacer.

### Un matiz sobre H12 que apareció al conectarlo

`Transferencia::nueva` impide construir un importe con la divisa equivocada, de modo que **ningún camino del código puede producir H12**. Pero eso cierra el error del *programador*, no el del *usuario*.

El formulario pide dos números sueltos y la divisa de cada uno **se deduce de la cuenta elegida, no se declara**. No hay ninguna declaración que el dominio pueda contradecir: quien teclea 6 000 pensando en pesos y elige una cuenta en dólares acredita seis mil dólares, y el sistema no tiene forma de saberlo.

Esa mitad se mitiga donde se comete el error. El formulario ahora rotula cada importe con la divisa de su cuenta y avisa cuando la transferencia cruza divisas. **C50 cambió de nombre para decir esto**, en vez de dar por cerrado algo que no lo está.

### Una guarda que se cayó y volvió

Al reducir `eliminar_cuenta` a traducción se perdió la protección de la caja de efectivo. Lo detectó C10b al fallar. Vuelve como `es_caja` en el puerto, comprobada **antes** que la de gastos: una caja con gastos daría el mensaje genérico y dejaría creer que basta con borrar los gastos.

---

## 7. Lo que viene, y en qué orden

1. ~~**3.1 — Caracterización.**~~ Cerrado por C45–C54.
2. ~~**3.2 — Dominio.**~~ Cerrado por `dominio::cuenta`.
3. ~~**3.3 — Puertos.**~~ ~~**3.4 — Casos de uso.**~~ ~~**3.5 — Traducción.**~~ Cerrados.
4. **Decisión de H10, H13 y H14**, por separado y cada una explícita.

**La extracción está completa.** Lo que queda son decisiones, no trabajo de estructura.

**H11 queda cerrado por el tipo.** **H12, a medias**: cerrado en el código, mitigado en la interfaz, imposible de cerrar del todo mientras el importe sea un número suelto sin divisa declarada.

### Las tres decisiones, resueltas

Se decidieron con evidencia de la base real, no por analogía.

**H10 — el recorte al revertir. Decisión: reversión simétrica.**

Revertir significa *«esta transferencia nunca ocurrió»*. No es devolver el dinero —eso sería otra transferencia, un hecho nuevo—, sino retirar un registro mal hecho. Si nunca ocurrió, las dos cuentas deben quedar exactamente como estaban.

En el caso corriente —revertir enseguida, para corregir— el recorte y la simetría dan lo mismo, porque el destino todavía tiene el dinero. Solo divergen cuando el destino ya gastó parte, y ahí el negativo **informa**:

> Si la transferencia nunca ocurrió, el destino nunca tuvo ese dinero. Haberlo gastado significa que **faltan movimientos por registrar en esa cuenta**, y eso es lo que un saldo negativo señala.

El recorte ocultaba esa señal y además inflaba el patrimonio. La decisión es coherente con **C53**, que ya admitía que una transferencia dejara el origen en negativo sin llamarlo defecto.

**H13 — el borrado que arrastra el historial. Decisión: bloquear.**

Una cuenta que participó en alguna transferencia no se borra. Misma forma que la guarda de gastos que ya existía, y el mensaje dice qué se perdería. Afecta a 9 de las 11 cuentas registradas, pero la alternativa era destruir historial en silencio y dejar a la contraparte con dinero sin procedencia.

**H14 — el descuadre en la misma divisa. Decisión: rechazar.**

Sin cambio de divisa, lo que sale y lo que entra tienen que coincidir. La diferencia solo puede ser una comisión —y para eso está el cargo, que sale aparte, a cuenta del origen— o un error de tecleo. El mensaje de error orienta hacia el cargo en vez de limitarse a rechazar.

La evidencia decidió el riesgo: de **25 transferencias registradas, todas en la misma divisa, ninguna tiene descuadre**. Rechazarlo no invalida nada del historial ni bloquea ningún flujo en uso.

### Que las correcciones están fijadas

Cada una se comprobó revirtiéndola y verificando que fallan sus pruebas, no otras:

| Mutación | Pruebas que fallaron |
|---|---|
| Volver a recortar el destino al revertir | 2 — la del caso de uso y C48 |
| Quitar la guarda de transferencias | 2 — la del caso de uso y C51 |
| Admitir el descuadre | 3 — dos del dominio y una del caso de uso |

---

## 8. Estado final

**La Fase 3 está cerrada.** Los cinco hallazgos del vertical —H10 a H14— están resueltos:

| | Cómo se cerró |
|---|---|
| **H10** | Corregido: reversión simétrica |
| **H11** | Irrepresentable: el tipo no lo construye |
| **H12** | Irrepresentable en el código; mitigado en la interfaz para la mitad que depende de lo que se teclea |
| **H13** | Corregido: guarda de borrado |
| **H14** | Corregido: los importes deben cuadrar |
