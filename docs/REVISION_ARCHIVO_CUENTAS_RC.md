# Revisión rápida — archivo de cuentas para la RC

Fecha: 13 de octubre de 2026. Alcance: código de la rama aislada `feature/rc-integracion-sdd-ui-tarjetas`; lectura del proyecto en curso sin modificar sus cambios locales. No se consultó ninguna base de datos.

## Estado comprobado

- `cuentas_ahorro.nombre` aún tiene `UNIQUE` global y no existe columna de archivo (`src-tauri/src/db_sql.rs`). El comando `eliminar_cuenta` termina en `DELETE`, aunque hoy bloquea cajas y cuentas referenciadas. Por tanto, el archivo reversible y el saldo cero son **especificación, no implementación**.
- `CatalogoDeCuentas::cuentas`/`obtener_cuentas` devuelven todas las cuentas. Su lista alimenta selectores de gastos, transferencias, ingresos y abonos, además de saldos, Cuentas y Ajustes. Filtrar solo en la UI dejaría otros comandos abiertos a IDs archivados.
- Los historiales de transferencias y abonos hacen `JOIN` por ID; deben seguir viendo cuentas archivadas. `nombre_de_cuenta(id)` también sirve al contexto histórico. En cambio, `cuenta_por_nombre(nombre)` en operaciones nuevas deberá limitarse a activas para que un nombre repetido no sea ambiguo.
- La migración 11 ya reconstruye tablas con claves ajenas y verifica que no se pierdan índices/disparadores. Esa técnica es precedente, pero la nueva migración deberá preservar explícitamente el contador `AUTOINCREMENT`, índices, integridad referencial y los ID antiguos; no debe tocar datos reales en las pruebas.

## Contrato de lecturas recomendado

| Lectura / operación | Archivadas | Motivo |
|---|---|---|
| Selector para nuevo gasto, transferencia, ingreso, abono o avance | Excluir | No crear movimientos nuevos en una cuenta cerrada. |
| Saldos operativos, tarjetas/Inicio/Cuentas y recomendaciones | Excluir | Una cuenta cerrada no es recurso disponible; su saldo debe ser cero. |
| Ajustes: lista de cuentas y acción Reactivar | Incluir, etiquetadas y separadas | Permitir consulta y reapertura explícita. |
| Historial de transferencias, ingresos, gastos, pagos, avances y correcciones | Incluir siempre, por ID | No desaparecer movimientos ni cambiar su titular histórico. |
| Respaldo, restauración, auditoría y exportación histórica | Incluir | La copia debe poder reconstruir el estado completo. |
| `nombre_de_cuenta(id)` | Incluir | Lectura descriptiva de registros existentes. |
| `cuenta_por_nombre(nombre)` para un movimiento nuevo | Solo activas; preferir ID en todo contrato nuevo | Dos filas podrán compartir nombre cuando una esté archivada. |
| Caja de efectivo por divisa | Activa obligatoria; no archivable | Es infraestructura del flujo de efectivo. |

No aplicar `WHERE archivada = 0` a un repositorio genérico compartido por historiales. Exponer dos intenciones distintas, por ejemplo `cuentas_operativas()` y `cuentas_para_administrar()`, y probar cada consumidor. Las escrituras nuevas deben volver a validar estado activo dentro de su transacción; un selector visual no es una barrera de integridad. Una reversión histórica que dejaría balance no nulo en una cuenta archivada debe exigir reactivación previa (o un caso de uso explícito que la reactive en la misma transacción); nunca alterar en silencio una cuenta cerrada.

## Nombre reutilizable: alternativas

**A — recomendada: unicidad parcial de activos.** Reconstruir `cuentas_ahorro` sin `UNIQUE` en la columna, añadir `archivada INTEGER NOT NULL DEFAULT 0 CHECK(archivada IN (0,1))`, y crear `CREATE UNIQUE INDEX ... ON cuentas_ahorro(nombre) WHERE archivada = 0`. Se conserva `nombre` e ID de la archivada; un alta activa puede reutilizarlo. Una reactivación colisionante se rechaza con mensaje para renombrar antes de reactivar. Mantiene los `JOIN` históricos por ID y evita claves artificiales. Coste: migración delicada que exige prueba de rollback, `foreign_key_check`, recuentos, índice y secuencia `AUTOINCREMENT`, además de respaldo previo verificado.

**B — menor cambio de esquema, peor contrato: alias interno.** Mantener `nombre UNIQUE`, renombrar la archivada con un sufijo técnico y añadir un nombre visible original. Permite reutilización sin reconstruir la tabla, pero duplica significado, puede contaminar historiales y búsquedas por nombre, y exige adaptar muchos consumidores. No la recomiendo salvo que una migración de A resulte inviable tras pruebas sintéticas.

No usar `DELETE`, ni una tabla separada de archivadas: la primera rompe historial; la segunda obliga a mover filas referenciadas por claves ajenas.

## Puerta de aceptación

La RC no pasa mientras `eliminar_cuenta` pueda borrar, un selector incluya archivadas, un historial las oculte, una escritura nueva acepte su ID o se pueda archivar con `+0.01`/`-0.01`. Pruebas sintéticas: archivo con saldo cero; rechazo exacto de un centavo; reutilización del nombre; colisión al reactivar; historial por ID intacto; caja no archivable; carrera entre selección y escritura; reversión de movimiento histórico; migración desde esquema 15 con rollback e integridad referencial.
