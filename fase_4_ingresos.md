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
`ExtremoCuenta` hace con las transferencias. Sumar pesos a un saldo en dólares
deja de ser representable en vez de quedar prohibido por una comprobación que
alguien debe acordarse de escribir.

## Una comprobación que ninguna prueba ejercita

`acreditar` verifica que el `UPDATE` afectó a alguna fila. **Es inalcanzable
hoy**: quien llega ahí pasó antes por `resolver_deposito`, que ya verificó la
cuenta dentro de la misma transacción.

Se comprobó retirándola, y la suite siguió en verde. Se deja porque cuesta
nada y protegería si algún día las dos operaciones se separan, pero queda
dicho en el código que **no está cubierta**, para que nadie la lea como una
garantía verificada. Una comprobación que parece protegida y no lo está es
peor que no tenerla.

## Verificación

399 pruebas en verde. Tres mutaciones sobre los defectos cerrados:

| Mutación | Resultado |
|---|---|
| El cobro no comprueba que la factura exista | falla `c77` |
| El depósito no comprueba la divisa | falla la prueba de H19 |
| `acreditar` descarta el fallo | **no la detecta nadie** — ver arriba |

