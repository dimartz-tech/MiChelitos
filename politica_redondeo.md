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

## Tramo 4 — columnas `INTEGER` e IPC en texto ⏳

Pendiente. Es el grande: 53 columnas de dinero, 53 comandos que reciben `f64`,
y todo el frontend. Encaja en la Fase 7.

A diferencia del tramo 3, aquí **sí** hay pérdida posible: un importe con
fracción de céntimo guardado en `REAL` no se distingue de uno redondeado, y la
migración 3 tuvo que normalizarlos. Pero conviene medir el beneficio antes de
pagar el coste, igual que se hizo con el tramo 3.
