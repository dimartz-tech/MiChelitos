# Abonos: la comisión que se puede borrar por separado

Estado: **corregido en 1.40.0.** Este documento recoge el defecto, la corrección y lo que no se hizo.

## Qué pasa hoy
Un abono a tarjeta pagado desde una cuenta crea dos cosas: el abono (`pagos_tarjeta`) y un gasto con su comisión (`gastos`, método `transferencia`), enlazado por `pagos_tarjeta.gasto_comision_id`. Ese gasto aparece en la lista de gastos y **se puede borrar por separado**:

1. Al borrarlo, `revertir_gasto` devuelve su importe a la cuenta y la clave foránea (`ON DELETE SET NULL`) deja el vínculo del abono en `NULL`.
2. El abono conserva anotado que debitó `importe + comisión`.
3. Al revertir el abono después, se devuelve `importe + comisión`: **la comisión vuelve dos veces** y la cuenta acaba por encima de lo que tenía (24 sobre el saldo inicial en la prueba, con importes sintéticos).

Es la misma clase de defecto que tuvo el cargo del avance de efectivo, que sí está protegido (`eliminar_gasto` lo rechaza).

## En los datos reales
Comprobado en solo lectura con una consulta agregada: ningún abono con comisión ha perdido su gasto enlazado. **No ha ocurrido.** (No se anotan aquí cifras ni importes de los datos vivos.)

## Corrección (1.40.0)
1. **Guarda por lista**: `db_sql::GASTOS_DERIVADOS` declara qué gastos creó otra operación y por qué no se borran solos (el cargo de un avance y la comisión de un abono); `motivo_de_no_borrar_gasto` la consulta y `eliminar_gasto` la aplica. Sustituye la comprobación suelta del avance, con el mismo mensaje.
2. **Prueba de esquema** (`toda_clave_ajena_hacia_un_gasto_esta_declarada_como_derivada_o_informativa`): toda clave foránea real hacia `gastos` debe estar en `GASTOS_DERIVADOS` o en `VINCULOS_INFORMATIVOS_CON_GASTOS` (hoy, `bonificaciones.gasto_id`, que solo informa). Una tabla nueva que enlace un gasto falla la prueba hasta que alguien decida.
3. Pruebas: `c136` (se rechaza y nada se mueve), `c137` (revertir el abono deja la cuenta exacta), `c138` (un gasto con bonificación sí se borra). `c133` reproduce con SQL directo el estado que ya no se puede provocar desde la aplicación.
4. Sin migración: no hay datos afectados.

## Lo que no se hizo, y por qué
Se había propuesto también **rechazar revertir un abono que perdió su vínculo**. No se implementó: un abono con cuenta y comisión pero sin gasto enlazado **no se distingue** de un abono histórico legítimo (las pruebas de reversión ya lo dan por válido), así que rechazarlo bloquearía casos que hoy funcionan. Como el vínculo solo se perdía al borrar la comisión por separado, y eso ya no es posible, el caso no puede aparecer de nuevo. Si algún día apareciera, se trataría como un caso aparte, con los datos delante.
