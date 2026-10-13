# Ruta de integración SDD → MiChelitos RC

Estado: propuesta ejecutable, no autorización de fusión a `main` ni declaración de release candidate. Línea de trabajo aislada: `feature/rc-integracion-sdd-ui-tarjetas`, creada sobre `6cf2729`. Solo se reutiliza código, especificaciones y pruebas; nunca una base ni rutas de datos de la versión independiente.

## Decisión de integración

No conviene un `merge` completo de la rama SDD: allí hay un esquema SQLite nuevo, contratos monetarios en unidades menores y una implementación JavaScript distinta del proyecto activo en TypeScript. Un merge textual mezclaría modelos financieros incompatibles y rutas de almacenamiento. La vía es portar verticales seleccionados, cada uno con especificación → prueba de caracterización → adaptación → prueba de contrato/integración → ensayo UI. Los commits SDD son fuentes para revisar, no cherry-picks automáticos.

| Orden | Tramo | Fuente a estudiar | Adaptación al proyecto activo | Puerta de salida |
|---|---|---|---|---|
| 0 | Base segura | `6cf2729` y migraciones 1–15 de main | Mantener respaldo previo a migración y trabajo aislado; no abrir datos reales en pruebas. | `git diff` revisado, árbol limpio, pruebas base. |
| 1 | Lenguaje visual | `d6ea88a` (centro operativo) y propuesta local `2aea2e7` | `4725e45` incorpora «Libro y contexto» como candidato. Adaptar las tarjetas, el inicio y los resúmenes a azul profundo, esmeralda y dorado sin sumar divisas. | Contraste, foco visible, teclado, 200 % de zoom y pruebas de vista; aprobación visual. |
| 2 | Exactitud monetaria | `a15804d`, `ac64d57`, `e222e87`, `14957fa` | Comparar campo por campo los contratos IPC, intereses, límites y reversión con los de main; portar solo las diferencias demostradas mediante pruebas de centavo y rollback. | Ningún `f64` genera importes persistidos nuevos; DOP/USD y cargo 0,20 % verificados. |
| 3 | Operaciones y ciclos de tarjeta/cuenta | especificaciones SDD y estructura relacional de main | Desactivación de tarjeta y archivo de cuenta reversibles, sin `DELETE`; preservar saldos, historial y referencias. Exigir saldo cero por divisa antes de cerrar; suspender cargos automáticos de suscripciones asociadas a tarjetas inactivas. | Migración atómica, pruebas de saldo exacto y dependencias, reactivación y revisión de vencimientos; caja de efectivo protegida. |
| 4 | Respaldo y recuperación | `9510a16`, `92ef32f`, `724920f`, `aa5bd2d`, `c8dc8f7` | Portar el formato y la verificación solo tras definir compatibilidad con la base activa, custodia de clave, sustitución atómica y rollback. | Ida/vuelta SQL + estado auxiliar sintético; prueba en destino sincronizado; ninguna señal falsa de éxito remoto. |
| 5 | RC | ambos proyectos | Congelar contratos, ejecutar suites Rust/TS, instalación nueva y actualización desde copia sintética, restauración, prueba manual macOS/Tauri 2 y accesibilidad. | Checklist firmado; después considerar cherry-pick de commits de esta rama a la rama de entrega, nunca merge ciego. |

## Regla de eliminación de tarjeta

La acción visible será «Desactivar tarjeta» y tendrá inversa «Reactivar tarjeta». No se ejecutará `DELETE FROM tarjetas`: las claves foráneas actuales borrarían por cascada pagos, bonificaciones y suscripciones. Para cerrar, **ambos balances, DOP y USD, deben ser exactamente cero centavos**. Un balance positivo es deuda pendiente; uno negativo es saldo a favor del titular y se avisará con su importe y divisa. En cualquiera de los dos casos se bloquea el cierre, también si el otro balance está en cero. Una tarjeta desactivada permanece en el historial y no se propone para nuevos consumos o avances, ni para cargos automáticos de suscripciones. La UI debe distinguir su estado con texto, no solo color, y exigir confirmación informada. El detalle de reanudación de períodos vencidos de suscripciones al reactivar requiere decisión explícita; hasta resolverla, el tramo no se declara listo para RC.

Las suscripciones vinculadas deben mostrarse y reasignarse a destinos elegidos por el usuario como parte de la misma transacción de cierre. La recomendación de destino y las pruebas de este flujo están en [SPEC-RC-TAR](SPEC_CIERRE_TARJETA_Y_SUSCRIPCIONES.md); no se trasladan cargos históricos ni se recomienda una tarjeta solo por su fecha de corte.

## Regla de cierre de cuenta

La petición de «Eliminar cuenta» se sustituirá por «Archivar cuenta» y su inversa «Reactivar cuenta». El caso de uso actual `eliminar_cuenta` y la UI de Ajustes aún ejecutan un `DELETE` cuando no hay referencias: **no representan la política aprobada y no deben exponerse como cierre en la RC**. Para archivar, el balance de la cuenta debe ser **exactamente cero centavos en su propia divisa**. Si es positivo, se avisa del dinero a favor que debe retirarse o transferirse; si es negativo, se avisa del saldo pendiente de conciliar. En ambos casos se bloquea el archivo. La cuenta archivada conservará su identificador, divisa, saldo e historial; no se ofrecerá como origen o destino de nuevas operaciones. Las correcciones y reversiones de movimientos históricos requieren una ruta explícita que no rompa la conservación de saldos. La caja de efectivo interna se mantendrá protegida.

## Puerta obligatoria de RC: cierre sin pérdida de saldo

Esta condición no está implementada por el mero hecho de estar documentada. La RC **no puede aprobarse** mientras el comando antiguo permita `DELETE` o mientras cualquiera de estas pruebas falte o falle:

| Caso sintético | Resultado exigido |
|---|---|
| Cuenta DOP o USD con `0.00` y sin papel de caja | Se archiva sin borrar movimientos; la reactivación conserva el mismo ID. |
| Cuenta con `+0.01` o `-0.01` | Se rechaza sin cambiar estado; mensaje con importe y divisa. |
| Tarjeta con `0.00 DOP` y `0.00 USD` | Se desactiva sin cascadas; historial y suscripciones permanecen. |
| Tarjeta con `+0.01` de deuda o `-0.01` a favor en cualquiera de las divisas | Se rechaza sin cambiar estado; «saldo a favor» se distingue de deuda. |
| Caja de efectivo, tarjeta o cuenta inexistente, fallo de escritura y carrera entre consulta y cierre | No se archiva ni se informa éxito; la validación y el cambio son atómicos. |

El núcleo compara unidades menores enteras o `Dinero` ya validado; no usa tolerancias `f64` ni redondea una diferencia para declararla cero. La interfaz muestra la causa, pero la decisión se repite dentro de la transacción de persistencia. Las pruebas deben usar datos sintéticos y verificar también que la UI no ofrece una operación de cierre válida cuando existe balance a favor.

## Alcance de esta rama y salida a la principal

Cada tramo se entrega en commits pequeños y puede llevarse mediante cherry-pick después de comparar diffs y ejecutar pruebas en la rama destino. `main`, la carpeta del proyecto activo y la rama SDD con cambios locales ajenos permanecen intactos durante esta evaluación. La fusión definitiva exige revisión de migraciones, respaldo verificable previo, comprobación de ausencia de datos personales o claves y ensayo de restauración, no solo compilación.
