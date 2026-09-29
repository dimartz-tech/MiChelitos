# Fase 6 — Capital y préstamos

La fase de **menor densidad de reglas** del plan, y por eso va al final. Los
dos verticales llegaban a ella en estados muy distintos.

## Préstamos: ya migrado, documentado aquí en retrospectiva

El saldo vivo, el desglose de cuota entre interés y capital, el vínculo con
la tarjeta que cobra una facilidad revolvente y la conciliación contra el
estado de cuenta se resolvieron en la versión 1.6.0, antes de que este
proyecto empezara a numerar fases. `dominio::prestamo` existe, tiene pruebas
propias, y la caracterización lo cubre en `c34`–`c44`.

La regla central: **el pasivo dejó de deducirse y pasó a llevarse**. La
fórmula anterior —monto original por fracción de cuotas pendientes— suponía
amortización lineal, falsa en todo préstamo real, y no se aplicaba en
absoluto a una línea de crédito, cuyo pasivo quedaba fijado en el desembolso
inicial para siempre. Ahora cada cuota mueve el saldo por su parte de
capital, no por su importe entero, y el saldo admite tanto capital negativo
—la cuota no cubre el interés, la deuda crece— como capital que excede la
deuda —se paga de más—. Ninguno se recorta: un saldo que se mueve en la
dirección incómoda debe verse.

No hay nada más que hacer aquí para esta fase. Se documenta para que el
archivo de la fase exista, no porque quedara trabajo pendiente.

## Capital: caracterizado por primera vez

El capital —propiedades, certificados, inversiones de bolsa— vivía como un
único documento JSON sin ningún módulo de dominio, sin pruebas y con una
regla —la alerta de vencimiento— duplicada dentro del comando que lee la
colección.

### El defecto: un valor calculado que se guardaba como declarado

`obtener_capital` calculaba, para cada certificado y cada inversión, cuántos
días faltan para su vencimiento y si corresponde avisar. El cálculo se
escribía **en el mismo objeto** que la función devolvía.

El problema es lo que hace la interfaz con ese objeto. Las seis acciones de
la vista de capital —añadir o quitar un certificado, una inversión, un
bien— siguen todas el mismo patrón: leer el capital completo, mutar una sola
colección, guardar el objeto entero de vuelta. Como la lectura ya traía la
alerta calculada, **cada una de esas seis acciones grababa esa alerta en el
archivo**, con la fecha del día en que se guardó por última vez. No es
hipotético: se confirmó contra la base real, donde una inversión de bolsa ya
tenía `alerta_vencimiento` y `dias_restantes` guardados en disco, sin haberse
vuelto a actualizar desde entonces.

### La corrección

`dominio::capital::calcular` es ahora la única fórmula, con sus propias
pruebas, y `obtener_capital` la usa en vez de tener la lógica duplicada para
certificados y para bolsa por separado.

Pero el cálculo correcto no basta: el defecto estaba en la escritura, no en
la lectura. `guardar_capital` retira ahora `alerta_vencimiento`,
`dias_restantes` y `alerta_msg` de cada entrada **antes** de persistir, sea
cual sea su origen. Se hace en el único punto de escritura, no confiando en
que cada una de las seis acciones de la interfaz recuerde omitirlos —que es
exactamente la confianza que produjo el defecto.

La base real ya tenía el archivo contaminado. Se limpió a mano, con un
respaldo tomado antes de escribir, fuera del repositorio.

### Lo que queda declarado y sin resolver

**El capital no pasa por `Dinero`.** Un monto negativo, con fracción de
céntimo o una fecha ilegible se guarda tal cual: es la vía de dinero menos
vigilada del proyecto. Es justo el motivo por el que esta fase tiene menor
densidad de reglas, y se deja fuera de esta entrega en vez de ampliarla sin
que se haya pedido.

## Verificación

498 pruebas en verde, 9 nuevas para capital.

| Mutación | Resultado |
|---|---|
| Se quita la limpieza al guardar (el defecto original) | mueren 2 |
| La ventana de aviso se acorta a 9 días | mueren 2 |
