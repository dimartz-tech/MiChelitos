# language: es

Característica: Abono en dólares a tarjeta de crédito desde cuenta en pesos
  Como titular de una tarjeta de crédito con saldo en USD
  Quiero abonar desde mi cuenta de ahorros en DOP indicando la tasa de cambio
  Para saldar la deuda en dólares sin descuadrar el balance en pesos

  Antecedentes:
    Dado que existe la cuenta de ahorros "Cuenta Ahorros DOP" en "DOP" con balance 500000.00
    Y que existe la tarjeta "Tarjeta Ejemplo" del "Banco Ejemplo"
    Y que la tarjeta "Tarjeta Ejemplo" tiene balance 1000.00 en "USD"

  Escenario: Abono exitoso con conversión de divisa y comisión bancaria
    Cuando registro un abono de 500.00 "USD" a la tarjeta "Tarjeta Ejemplo"
      Desde la cuenta "Cuenta Ahorros DOP" con una tasa de cambio de 60.00
    Entonces el balance de la tarjeta "Tarjeta Ejemplo" en "USD" debe ser 500.00
    Y el equivalente debitado en pesos debe ser 30000.00
    Y se debe registrar una comisión bancaria de 60.00 "DOP"
    Y el balance de la cuenta "Cuenta Ahorros DOP" debe ser 469940.00
    Y debe existir un gasto de categoría "Otros" por 60.00 "DOP"

  Escenario: La tasa de cambio es obligatoria al cruzar divisas
    Cuando registro un abono de 500.00 "USD" a la tarjeta "Tarjeta Ejemplo"
      Desde la cuenta "Cuenta Ahorros DOP" sin tasa de cambio
    Entonces la operación debe fallar con el error "TasaDeCambioRequerida"
    Y el balance de la cuenta "Cuenta Ahorros DOP" debe ser 500000.00
    Y el balance de la tarjeta "Tarjeta Ejemplo" en "USD" debe ser 1000.00

  Escenario: Un abono en la misma divisa no aplica conversión
    Dado que existe la cuenta de ahorros "Cuenta Ahorros USD" en "USD" con balance 2000.00
    Cuando registro un abono de 500.00 "USD" a la tarjeta "Tarjeta Ejemplo"
      Desde la cuenta "Cuenta Ahorros USD" sin tasa de cambio
    Entonces el balance de la tarjeta "Tarjeta Ejemplo" en "USD" debe ser 500.00
    Y el balance de la cuenta "Cuenta Ahorros USD" debe ser 1499.00

  Escenario: Atomicidad — un fallo a mitad de operación no deja saldos alterados
    Dado que el repositorio de tarjetas fallará al guardar
    Cuando registro un abono de 500.00 "USD" a la tarjeta "Tarjeta Ejemplo"
      Desde la cuenta "Cuenta Ahorros DOP" con una tasa de cambio de 60.00
    Entonces la operación debe fallar
    Y el balance de la cuenta "Cuenta Ahorros DOP" debe ser 500000.00
    Y no debe existir ningún gasto de categoría "Otros"
