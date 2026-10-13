# Ruta de integración SDD → MiChelitos RC

Estado: propuesta ejecutable, no autorización de fusión a `main` ni declaración de release candidate. Línea de trabajo aislada: `feature/rc-integracion-sdd-ui-tarjetas`, creada sobre `6cf2729`. Solo se reutiliza código, especificaciones y pruebas; nunca una base ni rutas de datos de la versión independiente.

## Decisión de integración

No conviene un `merge` completo de la rama SDD: allí hay un esquema SQLite nuevo, contratos monetarios en unidades menores y una implementación JavaScript distinta del proyecto activo en TypeScript. Un merge textual mezclaría modelos financieros incompatibles y rutas de almacenamiento. La vía es portar verticales seleccionados, cada uno con especificación → prueba de caracterización → adaptación → prueba de contrato/integración → ensayo UI. Los commits SDD son fuentes para revisar, no cherry-picks automáticos.

| Orden | Tramo | Fuente a estudiar | Adaptación al proyecto activo | Puerta de salida |
|---|---|---|---|---|
| 0 | Base segura | `6cf2729` y migraciones 1–15 de main | Mantener respaldo previo a migración y trabajo aislado; no abrir datos reales en pruebas. | `git diff` revisado, árbol limpio, pruebas base. |
| 1 | Lenguaje visual | `d6ea88a` (centro operativo) y propuesta local `2aea2e7` | `4725e45` incorpora «Libro y contexto» como candidato. Adaptar las tarjetas, el inicio y los resúmenes a azul profundo, esmeralda y dorado sin sumar divisas. | Contraste, foco visible, teclado, 200 % de zoom y pruebas de vista; aprobación visual. |
| 2 | Exactitud monetaria | `a15804d`, `ac64d57`, `e222e87`, `14957fa` | Comparar campo por campo los contratos IPC, intereses, límites y reversión con los de main; portar solo las diferencias demostradas mediante pruebas de centavo y rollback. | Ningún `f64` genera importes persistidos nuevos; DOP/USD y cargo 0,20 % verificados. |
| 3 | Operaciones y ciclo de tarjeta | especificaciones SDD y estructura relacional de main | Desactivación reversible sin `DELETE`; preservar saldos, historial y referencias. Suspender cargos automáticos de suscripciones asociadas mientras esté inactiva. | Migración atómica, prueba de dependencias, reactivación y revisión de vencimientos. |
| 4 | Respaldo y recuperación | `9510a16`, `92ef32f`, `724920f`, `aa5bd2d`, `c8dc8f7` | Portar el formato y la verificación solo tras definir compatibilidad con la base activa, custodia de clave, sustitución atómica y rollback. | Ida/vuelta SQL + estado auxiliar sintético; prueba en destino sincronizado; ninguna señal falsa de éxito remoto. |
| 5 | RC | ambos proyectos | Congelar contratos, ejecutar suites Rust/TS, instalación nueva y actualización desde copia sintética, restauración, prueba manual macOS/Tauri 2 y accesibilidad. | Checklist firmado; después considerar cherry-pick de commits de esta rama a la rama de entrega, nunca merge ciego. |

## Regla de eliminación de tarjeta

La acción visible será «Desactivar tarjeta» y tendrá inversa «Reactivar tarjeta». No se ejecutará `DELETE FROM tarjetas`: las claves foráneas actuales borrarían por cascada pagos, bonificaciones y suscripciones. Una tarjeta desactivada permanece en el historial, conserva deuda y puede recibir abonos para saldarla; no se propone para nuevos consumos o avances, ni para cargos automáticos de suscripciones. La UI debe distinguir su estado con texto, no solo color, y exigir confirmación informada. El detalle de reanudación de períodos vencidos de suscripciones al reactivar requiere decisión explícita; hasta resolverla, el tramo no se declara listo para RC.

## Alcance de esta rama y salida a la principal

Cada tramo se entrega en commits pequeños y puede llevarse mediante cherry-pick después de comparar diffs y ejecutar pruebas en la rama destino. `main`, la carpeta del proyecto activo y la rama SDD con cambios locales ajenos permanecen intactos durante esta evaluación. La fusión definitiva exige revisión de migraciones, respaldo verificable previo, comprobación de ausencia de datos personales o claves y ensayo de restauración, no solo compilación.
