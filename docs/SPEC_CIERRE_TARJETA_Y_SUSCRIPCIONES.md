# SPEC-RC-TAR — Cierre de tarjeta con suscripciones

Estado: criterio aprobado para planificar la RC; **pendiente de implementación y pruebas**. No modifica reglas de cobro ni registros existentes. Solo usa datos sintéticos en pruebas.

## Flujo observable

1. Al elegir «Desactivar tarjeta», la aplicación muestra el balance DOP y USD y todas las suscripciones vinculadas, con nombre, importe, divisa, frecuencia y próximo cargo. Si cualquier balance difiere de cero centavos, detiene el cierre y distingue deuda de saldo a favor del titular.
2. Por cada suscripción, el usuario elige una tarjeta destino activa y distinta de la que cierra. La pantalla propone una tarjeta, explica brevemente por qué y permite cambiarla. Una sugerencia no ejecuta un traslado.
3. Antes de confirmar, se muestra un resumen «suscripción → tarjeta destino», importes por divisa y fechas de próximo cargo. Si no existe destino válido para alguna suscripción, el cierre no avanza; tampoco se descarta ni borra la suscripción en silencio.
4. Una única transacción valida de nuevo saldos y destinos, traslada las referencias futuras y desactiva la tarjeta. Si falla cualquier paso, nada cambia. El cargo automático no puede intercalarse entre esa validación y la desactivación.
5. Se conservan el ID de cada suscripción, `fecha_ultimo_pago`, `fecha_proximo_cobro` y su frecuencia. Los gastos/pagos ya asentados permanecen vinculados a la tarjeta original. La siguiente ejecución automática, si corresponde, usará la tarjeta destino. Una revisión de vencimientos pendientes sigue siendo obligatoria antes de declarar la RC lista.

## Recomendación explicable, no automática

Primero se descartan la tarjeta de origen, las inactivas y las que no tengan cupo operativo en la divisa del cargo. La comparación de importes usa centavos exactos por divisa, nunca un total DOP+USD ni una tasa supuesta. La proyección resta **todas** las suscripciones que el usuario haya asignado a cada destino, para no recomendar varias por separado sobre el mismo cupo. Se prefiere una tarjeta con margen positivo sin depender del sobregiro. Para un cargo USD, se prefiere una política que lo conserve en USD (`origen`); una política que lo traduzca a DOP se muestra con aviso de conversión pendiente y no se presenta como equivalente. Entre destinos comparables, se prefiere el que ofrezca más días entre el próximo cargo declarado y su siguiente fecha de corte; esto es una estimación del ciclo, no una promesa de financiación sin intereses. Empates: mayor margen remanente y luego ID, para que la sugerencia sea estable.

Si faltan fecha o cupo fiable, se muestran alternativas elegibles sin una recomendación fuerte. El usuario puede elegir otra tarjeta válida; la confirmación enumera los riesgos visibles. No se modifica el monto, la divisa, la frecuencia ni el calendario de la suscripción durante el traslado.

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

La RC queda bloqueada hasta que estas pruebas pasen con la implementación real y se ensaye el flujo completo en una instalación desechable. No se abrirá ni migrará la base de la versión anterior para estas pruebas.
