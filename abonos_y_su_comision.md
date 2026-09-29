# Abonos: la comisión que se puede borrar por separado

Estado: **defecto conocido, fijado con una prueba (`c136`), sin corregir.** Este documento es el plan.

## Qué pasa hoy
Un abono a tarjeta pagado desde una cuenta crea dos cosas: el abono (`pagos_tarjeta`) y un gasto con su comisión (`gastos`, método `transferencia`), enlazado por `pagos_tarjeta.gasto_comision_id`. Ese gasto aparece en la lista de gastos y **se puede borrar por separado**:

1. Al borrarlo, `revertir_gasto` devuelve su importe a la cuenta y la clave foránea (`ON DELETE SET NULL`) deja el vínculo del abono en `NULL`.
2. El abono conserva anotado que debitó `importe + comisión`.
3. Al revertir el abono después, se devuelve `importe + comisión`: **la comisión vuelve dos veces** y la cuenta acaba por encima de lo que tenía (24 sobre el saldo inicial en la prueba, con importes sintéticos).

Es la misma clase de defecto que tuvo el cargo del avance de efectivo, que sí está protegido (`eliminar_gasto` lo rechaza).

## En los datos reales
Comprobado en solo lectura con una consulta agregada: ningún abono con comisión ha perdido su gasto enlazado. **No ha ocurrido.** (No se anotan aquí cifras ni importes de los datos vivos.)

## Corrección propuesta (pendiente de visto bueno)
1. **Guarda por lista, como la de eliminar cuenta.** Las claves foráneas hacia `gastos` son tres: `pagos_tarjeta.gasto_comision_id` (comisión de un abono), `avances_efectivo.gasto_cargo_id` (cargo de un avance) y `bonificaciones.gasto_id` (vínculo informativo, el gasto no depende de él). Una lista `GASTOS_DERIVADOS` con las dos primeras y el motivo de cada una sustituye la comprobación suelta del avance en `eliminar_gasto`.
2. **Prueba de esquema**, como `toda_clave_ajena_hacia_una_cuenta_esta_declarada…`: toda clave foránea hacia `gastos` debe estar en `GASTOS_DERIVADOS` o en la lista de vínculos informativos. Una tabla nueva que enlace un gasto **falla la prueba** hasta que alguien decida.
3. **Mensaje** parecido al del avance: «Este gasto es la comisión de un abono. Revierte el abono completo…».
4. `c136` se convierte en la prueba de que se rechaza y la cuenta no cambia; se añade una para el avance (ya cubierto por `c123`) usando la misma lista.

## Qué no cambia
Revertir el abono completo sigue borrando la comisión, como hasta ahora (`c64`). Los datos existentes no se tocan: no hay migración.

## Preguntas abiertas
* Si un abono ya perdió su vínculo (caso hipotético, no presente en los datos), ¿se ofrece reparar?: **propuesta: no**, se documenta y se rechaza revertirlo a ciegas.
