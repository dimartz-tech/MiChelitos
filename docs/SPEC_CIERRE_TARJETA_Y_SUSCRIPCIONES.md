# SPEC-RC-TAR — Cierre de tarjeta con suscripciones

Estado: criterio aprobado para planificar la RC; **pendiente de implementación y pruebas**. No modifica reglas de cobro ni registros existentes. Solo usa datos sintéticos en pruebas.

## Flujo observable

1. Al elegir «Desactivar tarjeta», la aplicación muestra el balance DOP y USD y todas las suscripciones vinculadas, con nombre, importe, divisa, frecuencia y próximo cargo. Si cualquier balance difiere de cero centavos, detiene el cierre y distingue deuda de saldo a favor del titular.
2. Por cada suscripción, el usuario elige una tarjeta destino activa y distinta de la que cierra. La pantalla propone una tarjeta, explica brevemente por qué y permite cambiarla. Una sugerencia no ejecuta un traslado.
3. Antes de confirmar, se muestra un resumen «suscripción → tarjeta destino», importes por divisa y fechas de próximo cargo. Si no existe destino válido para alguna suscripción, el cierre no avanza; tampoco se descarta ni borra la suscripción en silencio.
4. Una única transacción valida de nuevo saldos y destinos, traslada las referencias futuras, registra el traslado y desactiva la tarjeta. Si falla cualquier paso, nada cambia. El cargo automático no puede intercalarse entre esa validación y la desactivación.
5. Se conservan el ID de cada suscripción, `fecha_ultimo_pago`, `fecha_proximo_cobro` y su frecuencia. Los gastos/pagos ya asentados permanecen vinculados a la tarjeta original. La siguiente ejecución automática, si corresponde, usará la tarjeta destino. Al reactivar la tarjeta antigua, ninguna suscripción vuelve a ella automáticamente; volver a trasladarla exige otra elección explícita.

## Diseño de implementación

**Previsualización.** Un caso de uso de aplicación lee la tarjeta, sus balances por divisa, todas las suscripciones que la referencian y las tarjetas candidatas. Entrega un DTO de solo lectura con `id` y estado de cada candidato, proyección de cupo en centavos, motivo de la sugerencia y advertencias. La UI de Tarjetas abre un diálogo con una selección inicial «usar esta para todas» y una selección modificable por suscripción. Muestra tanto las mensuales como las anuales y permite revisar cada importe/fecha. «Confirmar cierre» permanece deshabilitado si falta un destino o hay saldo distinto de cero. Esto solo ayuda al usuario: la validación autoritativa ocurre de nuevo al confirmar.

**Persistencia.** Una migración versionada añade estado activo/inactivo a `tarjetas` (el estado inicial de las existentes es activo) y una bitácora de traslados `suscripcion_id`, tarjeta origen/destino y fecha del cambio; no reescribe historial de gastos ni borra filas. Un puerto de aplicación concentra la operación `cerrar_tarjeta_con_traslados(origen, [(suscripcion_id, destino_id)])`. El adaptador SQLite debe compartir **una** transacción de escritura entre tarjetas y suscripciones —el adaptador de tarjetas actual no incluye ambas—, comprobar saldos exactos DOP/USD, exigir que la selección cubra **exactamente** las suscripciones actualmente vinculadas, comprobar destinos activos y distintos del origen, validar cupo agregado por divisa, actualizar solo `tarjeta_id`, registrar cada traslado y marcar la tarjeta inactiva antes de confirmar. Cualquier error revierte todo. El comando Tauri traduce entrada/salida; no decide reglas ni abre una segunda transacción por cada suscripción.

**Cobro y otras escrituras.** El procesamiento actual obtiene un listado y más tarde registra cada cargo en otra transacción. Ese listado puede quedar obsoleto tras el cierre: `asentar_cargo` debe releer la suscripción por ID dentro de **su** transacción y verificar tarjeta activa, vencimiento y marcador actuales antes de escribir el gasto. Si cambió, descarta el elemento obsoleto y vuelve a evaluar en una ejecución posterior; nunca carga la tarjeta anterior usando el snapshot. Altas/ediciones de suscripciones y nuevos consumos/avances también deben rechazar tarjetas inactivas en el backend. Abonos y reversiones históricas requieren pruebas específicas para no perder su trazabilidad; no se eliminan referencias.

**Reactivación.** Cambia únicamente el estado de la tarjeta. No recupera suscripciones, fechas ni cargos anteriores. La pantalla muestra que permanece sin suscripciones hasta que el usuario las reasigne explícitamente desde Suscripciones.

## Secuencia de entrega en la rama aislada

1. **Núcleo y migración:** nueva versión de esquema en `src-tauri/src/db_sql.rs` y `migraciones.rs`; caso de uso en `aplicacion/tarjetas.rs`, puerto en `puertos/repositorios.rs` y adaptador SQLite. Primero pruebas TAR-01–TAR-07 con dobles y una base temporal sintética.
2. **Frontera de cobro:** relectura transaccional en `aplicacion/suscripciones.rs` y adaptador, guardas de nuevas escrituras hacia tarjetas inactivas. Reproducir la carrera TAR-09 y las reversiones; no cambiar el calendario financiero.
3. **Interfaz:** contrato IPC/comando Tauri y vista `src/js/vistas/tarjetas.ts` con diálogo accesible. Excluir inactivas de los selectores para nuevas suscripciones, gastos y avances, pero conservarlas en historiales. Probar TAR-03/TAR-08/TAR-10 y errores de redibujado.
4. **Puerta RC:** suites Rust y JavaScript con `pnpm`, migración desde una copia **sintética** de esquema anterior, ensayo manual de teclado/zoom/lector, prueba de cargo después del traslado y revisión del diff. Solo después se considera el cherry-pick de los commits de esta rama a la rama de entrega.

## Recomendación explicable, no automática

Primero se descartan la tarjeta de origen, las inactivas y las que no tengan cupo operativo en la divisa del cargo. La comparación de importes usa centavos exactos por divisa, nunca un total DOP+USD ni una tasa supuesta. La proyección resta **todas** las suscripciones que el usuario haya asignado a cada destino, para no recomendar varias por separado sobre el mismo cupo. Se prefiere una tarjeta con margen positivo sin depender del sobregiro. Para un cargo USD, se prefiere una política que lo conserve en USD (`origen`); una política que lo traduzca a DOP se muestra con aviso de conversión pendiente y no se presenta como equivalente. Entre destinos comparables, se prefiere el que ofrezca más días entre el próximo cargo declarado y su siguiente fecha de corte; esto es una estimación del ciclo, no una promesa de financiación sin intereses. Empates: mayor margen remanente y luego ID, para que la sugerencia sea estable.

Si faltan fecha o cupo fiable, se muestran alternativas elegibles sin una recomendación fuerte. Al calcular el corte de un mes corto, el día 29–31 se limita al último día real de ese mes, también en febrero bisiesto; se prueban esos bordes. El usuario puede elegir otra tarjeta válida; la confirmación enumera los riesgos visibles. No se modifica el monto, la divisa, la frecuencia ni el calendario de la suscripción durante el traslado.

## Matriz SDD mínima

| ID | Dado / cuando / entonces | Prueba requerida |
|---|---|---|
| TAR-01 | DOP `0.00` y USD `0.00`, sin suscripciones → desactiva y conserva la tarjeta. | Unidad + SQLite + UI. |
| TAR-02 | Deuda `+0.01` o saldo a favor `-0.01` en cualquiera de las divisas → no desactiva e informa importe/divisa. | Unidad + SQLite + UI. |
| TAR-03 | Dos suscripciones de distintas divisas → avisa de ambas y exige destino para cada una; no mezcla importes. | UI + contrato IPC. |
| TAR-04 | Dos suscripciones compiten por un cupo → la recomendación considera la suma asignada y no sobrepromete margen. | Unidad con centavos. |
| TAR-05 | Destino inactivo, misma tarjeta, sin cupo o desaparecido entre vista y confirmación → rechazo completo sin cambio parcial. | Unidad + SQLite/rollback. |
| TAR-06 | Traslado correcto → cambia solo la referencia futura; calendario, marcador de pago e historial anterior intactos. | SQLite + integración de cargo automático. |
| TAR-07 | Fallo en el segundo traslado o en la desactivación → toda la transacción revierte. | SQLite con fallo inyectado. |
| TAR-08 | Pantalla con teclado/lector → lista, sugerencia, cambio de destino, resumen y errores accesibles sin depender del color. | Ensayo UI WCAG 2.2 AA. |
| TAR-09 | El procesador obtuvo un cargo antes del traslado y lo intenta asentar después → no usa la tarjeta antigua ni crea un cargo duplicado; reevalúa el estado vigente. | Integración SQLite con intercalación controlada. |
| TAR-10 | Reactivar la tarjeta antigua → ninguna suscripción ni fecha vuelve sin una acción expresa. | Unidad + SQLite + UI. |

La RC queda bloqueada hasta que estas pruebas pasen con la implementación real y se ensaye el flujo completo en una instalación desechable. No se abrirá ni migrará la base de la versión anterior para estas pruebas.
