# Avance de efectivo

Dinero que una tarjeta de crédito pone en una cuenta de ahorro. Es una función
nueva, no un arreglo: no reemplaza nada de lo que había.

## Qué mueve

| | Efecto |
|---|---|
| **Tarjeta** | La deuda sube por el monto **y** por el cargo |
| **Cuenta de ahorro** | Recibe el monto, **sin** el cargo |
| **Gastos** | El cargo queda como gasto de la tarjeta, para que cuente en lo que cuesta financiarse |

La diferencia entre lo que sube la deuda y lo que llega a la cuenta es el coste
del avance. Con 10 000 y un cargo del 6,25 %: la deuda sube 10 625, la cuenta
recibe 10 000 y aparece un gasto de 625.

## El cargo tiene tres formas

* **Porcentaje**, entre el 6 % y el 10 % del monto. El más reciente que se ha
  visto fue del 6,25 %, de modo que el campo admite dos decimales.
* **Monto fijo**, positivo, en la divisa del avance.
* **Exonerado**: la entidad no cobró. No genera gasto y admite una nota libre
  con el motivo.

Son tres variantes de un tipo y no un porcentaje con casos especiales: un cargo
fijo no tiene tasa, y una exoneración no es «un cargo del cero por ciento» sino
la declaración de que no hubo cargo. Un cargo fijo de cero se rechaza por eso:
es una exoneración y debe declararse como tal.

### Por qué la banda del 6 % al 10 %

Casi todo porcentaje fuera de ella es un tecleo —`0,8` por `8`, o `80` por
`8`—, y el importe resultante entraría en la deuda de la tarjeta sin que nada
lo cuestionara. Las dos constantes, `CARGO_MINIMO` y `CARGO_MAXIMO`, están en
`dominio::avance`: si una entidad cambia su tarifa, es el único sitio que se
toca.

## Decisiones tomadas sin que estuvieran especificadas

1. **El cargo lo paga la tarjeta, no la cuenta.** Es como lo asienta el emisor.
   Si en algún caso el banco lo descuenta del monto acreditado, la regla cambia
   aquí y en el caso de uso.
2. **Misma divisa.** Un avance se acredita en la divisa en que se carga. Una
   cuenta en otra divisa se rechaza en vez de convertir: convertir sería
   inventar una tasa que nadie ha declarado.
3. **El cargo es un gasto** de método «tarjeta». Como sube la deuda junto con el
   monto en una sola operación, **borrarlo por separado** bajaría la deuda por
   el cargo y dejaría el avance con un cargo que ya no existe. El comando
   `eliminar_gasto` lo rechaza y exige revertir el avance entero.
4. **Una cuenta que recibió un avance no se elimina.** Borrarla se llevaría el
   rastro de la tarjeta de la que salió el dinero.

## Se puede deshacer

Revertir un avance es el inverso exacto, con caso de auditoría y motivo escrito
como cualquier otra corrección: la deuda baja por el monto y el cargo, la cuenta
devuelve el monto, el gasto desaparece y el registro se borra.

Se repone **lo guardado**, no lo que hoy se recalcularía con el porcentaje. Y
sin recorte: si el dinero ya se gastó, devolverlo deja la cuenta en negativo, y
ese es el estado verdadero.

## Antes de registrar

La interfaz enseña lo que se va a mover y pide confirmación. Las cifras vienen
del núcleo (`simular_avance_efectivo`), no de una cuenta hecha en JavaScript:
la cifra que se confirma es exactamente la que se asienta.

## Lo que no hace, a propósito

* **No comprueba el disponible de la tarjeta.** Tampoco lo hacen las compras.
  Un avance puede dejar la deuda por encima del límite.
* **No calcula intereses del avance.** Muchas entidades los devengan desde el
  primer día; eso es un cálculo distinto y no se ha pedido.
* **No convierte entre divisas.**
