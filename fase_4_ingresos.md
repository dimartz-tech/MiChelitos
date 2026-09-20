# Fase 4 — Ingresos

## 4.1 Caracterización

Quince pruebas que fijan la conducta vigente del vertical de ingresos antes de
extraer nada. **Cinco documentan defectos, no aciertos**: existen para que la
extracción no los corrija por accidente, y para que corregirlos sea después una
decisión visible en vez de un efecto colateral.

## 1. El hallazgo que engloba a los demás

Tres de los cinco defectos son **reapariciones literales** de otros ya resueltos:

| Aquí | Ya resuelto en |
|---|---|
| H16 — redondeo a unidades | **H8**, gastos |
| H17 — cuenta localizada por su nombre | **H3**, caja de efectivo |
| H20 — recorte a cero al revertir | **H5** y **H10**, tarjetas y transferencias |

Que el mismo error viva en cuatro sitios distintos es, en sí, más informativo
que cualquiera de ellos por separado: **no había nada compartido que impidiera
repetirlo**. Cada vertical resolvió su copia por su cuenta y ninguno pudo
ayudar al siguiente.

Es exactamente el argumento del plan para extraer un dominio: no para que el
código quede más bonito, sino para que una regla exista **una sola vez** y su
corrección alcance a todos los que la usan.

## 2. Los cinco defectos

### H16 — La retención se redondea a unidades, no a céntimos

`crear_ingreso` y `actualizar_ingreso` calculan `(monto × pct/100).round()`,
que redondea a pesos enteros. El 15 % de 1 234.56 son 185.184: deberían ser
**185.18** y quedan en **185.00**.

Fijado por `c71` y `c74`.

### H17 — La cuenta se localiza por su nombre y el fallo se descarta

`marcar_ingreso_pagado` y `marcar_informal_pagado` acreditan con
`UPDATE ... WHERE nombre = ?` y **descartan el resultado con `let _ =`**. Si el
nombre no coincide, la factura queda cobrada y ningún saldo se mueve, sin
aviso.

Fijado por `c76` y `c82`.

### H18 — Cobrar una factura inexistente no falla

El `UPDATE` afecta a cero filas y devuelve `Ok`. El sistema no distingue entre
haber cobrado y no haber encontrado nada que cobrar.

Fijado por `c77`.

### H19 — El importe se acredita sin mirar la divisa de la cuenta

`ingresos` **no tiene columna de divisa**: el importe es implícitamente en
pesos. Pero la cuenta de destino se elige por nombre y puede ser en dólares, de
modo que se suman unidades de peso a un saldo en dólares.

Es H2 otra vez, y el tipo `Dinero` existe precisamente para impedirlo.

Fijado por `c78`.

### H20 — El borrado recorta el saldo en cero

`eliminar_ingreso` y `eliminar_ingreso_informal` revierten con
`MAX(0.0, balance - ?)`. Si el dinero cobrado ya se gastó, revertir deja la
cuenta en cero y la diferencia desaparece sin registro.

Fijado por `c80`.

## 3. Lo que esta etapa no hace

**No corrige ninguno de los cinco.** Los documenta y los cubre con pruebas, que
es el protocolo del proyecto: primero se fija lo que hay, después se decide qué
cambiar. Las correcciones de H16 y H20 tocan importes de dinero y por tanto se
consultan antes de aplicarse.

## 4. Verificación

362 pruebas en verde. Dos mutaciones confirman que las pruebas discriminan:

| Mutación | Resultado |
|---|---|
| Redondear la retención al céntimo | falla `c71` |
| Retirar el recorte a cero | falla `c80` |

Ambas son **las correcciones futuras**, no averías: que hagan fallar la
caracterización es justo lo que debe pasar, y es la señal de que el día que se
apliquen no pasarán inadvertidas.

---

## 4.2 Dominio y cierre de los tres defectos restantes

Los cinco hallazgos quedan resueltos. Los dos que tocaban dinero se
consultaron —H16 y H20—; estos tres son correcciones de conducta sin pérdida,
y se hicieron dentro de la extracción como estaba acordado.

### H17 — la cuenta deja de referenciarse por su nombre

Era H3 otra vez. Se cierra igual: **por referencia, no por texto**. La
migración 7 añade `cuenta_ahorro_id` a las dos tablas de ingresos y lo rellena
desde el nombre guardado.

El nombre **se conserva**. Sirve para leer el histórico, y para los cobros
registrados contra una cuenta que ya no existe: ahí no hay identificador que
poner, y borrar el dato dejaría el asiento sin explicación.

### H18 — cobrar una factura inexistente ya falla

La condición del `UPDATE` exige además que la factura esté **pendiente**, de
modo que cobrar dos veces tampoco pasa inadvertido. Era un caso que nadie
había mirado: el defecto original solo hablaba de facturas que no existen.

### H19 — el importe entra en la divisa de su cuenta

El tipo `Deposito` ata el importe a la divisa de la cuenta que lo recibe, como
`ExtremoCuenta` hace con las transferencias.

**El primer intento no lo resolvió, y merece contarse.** `resolver_deposito`
denominaba el importe **con la divisa de la cuenta** y acto seguido pedía a
`Deposito::nuevo` que comprobara que coincidían. Comparaba esa divisa consigo
misma: la comprobación existía en el tipo y era **vacua en la llamada**. El
caso real —8 500 pesos entrando como 8 500 dólares— seguía pasando.

La corrección es que la divisa de lo cobrado llegue **desde fuera**. Una
factura se emite en moneda local —`ingresos` no tiene columna de divisa—, así
que cobrarla en una cuenta en otra divisa exigiría una conversión que nadie ha
declarado. Se rechaza en vez de inventarla.

## Qué comprueba `resolver_deposito`

Tres cosas, y enumerarlas fue lo que destapó el fallo anterior:

1. **Que la cuenta exista** (H17).
2. **Que el importe no sea negativo**, dentro de `Deposito`.
3. **Que la divisa de lo cobrado sea la de la cuenta** (H19).

La tercera era la que faltaba, y faltaba de una forma difícil de ver: no por
ausencia de código sino porque sus dos operandos eran el mismo valor.

## La lección: una comprobación que no puede fallar no protege

La versión anterior de `acreditar` repetía la verificación de la cuenta «por si
acaso», y se documentó como respaldo inalcanzable. **Se retira.**

Una comprobación que no puede fallar aparenta una garantía que ninguna prueba
sostiene, y anima a confiar en ella. Fue exactamente lo que pasó con la de
divisas: parecía cubierta por el tipo y no lo estaba. La garantía de existencia
vive ahora en un solo sitio, donde sí se ejercita.

El criterio que queda: **si una mutación que rompe una comprobación no hace
fallar ninguna prueba, la comprobación no está validando nada** — o sobra, o
sus operandos no son independientes. Ambas cosas hay que mirarlas, no
documentarlas.

## Verificación

400 pruebas en verde. Las mutaciones sobre los defectos cerrados:

| Mutación | Resultado |
|---|---|
| El cobro no comprueba que la factura exista | falla `c77` |
| El depósito no compara divisas | falla `c78` y la prueba de dominio |
| Se denomina el importe con la divisa de la cuenta | falla `c78` — reproduce el fallo entregado |

La última es la importante: reproduce el error que este documento describía
como resuelto, y ahora se detecta.
