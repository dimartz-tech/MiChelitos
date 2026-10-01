# Actualizar la aplicación instalada: estabilidad y procedimiento

Estado a 2026-10-02. **Instalada: 1.68.0** (actualizada el 2026-10-01 en tres pasos: 1.36.0 → 1.62.0, 1.62.0 → 1.65.0 y 1.65.0 → 1.68.0; ver «Actualizaciones hechas»). Lo que sigue en esta guía es el análisis de la primera actualización (1.36.0 → 1.48.0) y el procedimiento, que se siguió tal cual y vale para la próxima.

## Actualizaciones hechas

### 1.65.0 → 1.68.0 (2026-10-01)
A petición expresa del titular, con el mismo procedimiento y la misma prueba previa contra una copia de los datos:
* Respaldo `michelitos_2026-10-01T21-54-11_antes-de-actualizar-a-1.68.0` (base y capital, integridad correcta) y copia de la 1.65.0 en `~/.michelitos/aplicacion-anterior/MiChelitos-1.65.0.app`.
* **Sin cambios en Rust** y sin migraciones (`user_version` sigue en 15): solo frontend. Trae tres correcciones: las cuatro bajas que no avisaban si Rust fallaba (1.66.0), el panel de «Casos de corrección» (1.67.0) y el **escape general de las plantillas** (1.68.0: ningún texto escrito por el titular se interpreta ya como HTML ni como código, y los botones con datos en el manejador funcionan con nombres con comillas).
* Probada contra una copia (viva a los 16 s, esquema 15, integridad correcta, mismos conteos por tabla), instalada y abierta: esquema 15, integridad correcta, mismos conteos por tabla.
* Volver atrás: cerrar y restituir `MiChelitos-1.65.0.app` (o una anterior) en `/Applications`; la base no se toca.

### 1.62.0 → 1.65.0 (2026-10-01)
A petición expresa del titular, con el mismo procedimiento y la misma prueba previa contra una copia de los datos:
* Respaldo `michelitos_2026-10-01T20-43-54_antes-de-actualizar-a-1.65.0` (base y capital, integridad correcta) y copia de la 1.62.0 en `~/.michelitos/aplicacion-anterior/MiChelitos-1.62.0.app`.
* **Sin cambios en Rust** desde la 1.62.0 y sin migraciones (`user_version` sigue en 15): lo nuevo es solo la **división de `ui.ts` en once vistas** (1.51.0 a 1.65.0), sin cambios visibles.
* Probada contra una copia (viva a los 16 s, esquema 15, integridad correcta, mismos conteos por tabla), instalada y abierta: esquema 15, integridad correcta, 0 referencias rotas, mismos conteos por tabla.
* Volver atrás: cerrar y restituir `MiChelitos-1.62.0.app` (o `MiChelitos-1.36.0.app`) en `/Applications`; la base no se toca.

### 1.36.0 → 1.62.0 (2026-10-01)
A petición expresa del titular. Se siguió el procedimiento de abajo, con una prueba previa más:
1. App cerrada; respaldo `michelitos_2026-10-01T19-44-28_antes-de-actualizar-a-1.62.0` (base y capital, integridad correcta) y copia de la 1.36.0 en `~/.michelitos/aplicacion-anterior/MiChelitos-1.36.0.app`.
2. Compilada desde `main` (`npx tauri build -b app`; el `.app` sale de `src-tauri/target/release/bundle/macos/`).
3. **Antes de instalar**, el paquete nuevo se arrancó contra una **copia** de la base: viva a los 14 s, esquema 15, integridad correcta y los mismos conteos de filas por tabla.
4. Instalada en `/Applications` y abierta: esquema 15, integridad correcta, 0 referencias rotas, mismos conteos por tabla. **Sin migraciones** desde la 1.36.0 (`user_version` sigue en 15).
5. Lo que trae de nuevo para el titular, además de las correcciones de la 1.36.0 → 1.48.0: la corrección de las bajas y correcciones con motivo (1.62.0: `prompt()` devolvía `null` y `confirm()` no esperaba la respuesta en el WebView; ver `historial_versiones.md`), la carga fija del Resumen por divisa (1.60.0) y el panel de restaurar respaldos.
* **Volver atrás:** cerrar y restituir `MiChelitos-1.36.0.app` en `/Applications`; la base no se toca (mismo esquema). Un estado de datos anterior se restaura desde Ajustes.
* Lo posterior a la 1.62.0 llegó con la actualización a la 1.65.0 (arriba).

## Antes de la primera actualización (1.36.0 → 1.48.0)
Esta sección es el análisis que se hizo entonces; se conserva como registro.

## Veredicto
**Estable para actualizar**, con las precauciones de abajo. Lo que cambia entre las dos versiones es pequeño en el núcleo y grande en la forma del frontend, y los dos se comprobaron con tus datos reales (sobre copias).

## Qué cambia de 1.36.0 a hoy
13 PR. En Rust, sin contar pruebas: **221 líneas añadidas y 23 borradas**, en tres sitios:

| Cambio | Efecto |
|---|---|
| `respaldo.rs` | Restaurar ya no se bloquea cuando lo actual está dañado (1.38.0) |
| `db_sql.rs` + `eliminar_gasto` | La comisión de un abono no se borra por separado: se acabó el reembolso doble (1.40.0) |
| Pruebas y guardas | Sin efecto en la aplicación |

**Sin migraciones de esquema:** `user_version` sigue en **15**. La base ya migrada por 1.36.0 es la misma que abre la nueva, y **también la abriría la 1.36.0 de vuelta**.

En el frontend: pasa a TypeScript compilado (1.44.0), con un ayudante de DOM (1.46.0) y envoltorios tipados (1.47.0), más tres correcciones de defectos que están en la instalada:
* el aviso «Cobro próximo» mostraba «undefined» en lugar de la fecha (1.43.0);
* el panel «Casos de corrección» no abría: faltaba un envoltorio (1.45.0);
* el botón de restaurar respaldos solo existe desde 1.37.0.

## Evidencia (todo sobre COPIAS de tus datos, en carpetas temporales ya borradas)

**Backend, con la base real:**
* El arranque (preparar el esquema) no cambia ninguna tabla ni ninguna fila: esquema 15 → 15, integridad correcta, 0 referencias rotas.
* Los **16 comandos de lectura** (y los de cada tarjeta y cada préstamo) devuelven sus filas sin fallos.
* El cobro automático de suscripciones es idempotente: dos pasadas seguidas, 0 cargos nuevos.
* Respaldar y restaurar sobre datos reales: integridad correcta, capital restaurado.
* La aplicación **empaquetada**, construida con `tauri build`, arrancada contra una copia de la base real: sigue viva a los 12 s, esquema 15, integridad correcta, 0 referencias rotas, las mismas filas (no añade ni quita ninguna), y toma su respaldo de arranque.
* 613 pruebas de Rust y 42 de JavaScript pasan; la comprobación de tipos pasa.

**Frontend, con los datos reales:**
* Las **once pestañas pintadas con tus datos, con la versión instalada y la nueva a la vez**: diez idénticas carácter a carácter (de 655 a 26 435 caracteres cada una); la de Ajustes difiere solo en el panel nuevo de restaurar respaldos. Sin `undefined`, `NaN` ni `[object`, sin errores de consola y sin comandos sin respuesta.
* El JavaScript compilado de `ui` es idéntico byte a byte al anterior salvo 123 líneas, revisadas una a una.

## Lo que no está cubierto (riesgo residual)
* **Pulsar.** Se comprobó lo que se **pinta** y que los manejadores tienen destino, no cada formulario con la aplicación empaquetada y tus datos. Los envíos de dinero son los mismos comandos de Rust de antes (y están cubiertos por las pruebas), pero la ruta formulario → comando solo se probó por muestreo (categoría, restaurar, casos de corrección, aviso de cobro).
* **`tauri dev`** no se ha probado con el esquema nuevo (no afecta a la aplicación instalada).
* **Sin firma:** la aplicación sigue sin firmar, como ahora; macOS puede pedir confirmación la primera vez.
* **Un detalle del aviso de cobro:** hoy ninguna suscripción tiene aviso, así que esa corrección no se vio con tus datos, solo con datos simulados.

## Procedimiento
1. **Cerrar la aplicación.** Si está abierta, `/Applications/MiChelitos.app` está en uso y la base puede tener una escritura en curso.
2. **Respaldo fresco** de la base y el capital (con `sqlite3 … "VACUUM INTO …"` o, ya instalada la nueva, el botón de Ajustes).
3. **Guardar la aplicación actual** (`~/.michelitos/aplicacion-anterior/MiChelitos-1.36.0.app`) para poder volver.
4. `npm run tauri build` (compila el frontend y construye). **Importante:** el empaquetado borra el `.app`; se toma del `.dmg` o de `src-tauri/target/release/bundle/macos/`.
5. Copiar a `/Applications` y abrir.
6. **Comprobar** (2 minutos): que arranca; en Ajustes aparece «Restaurar un respaldo» con los respaldos; el panel «Casos de corrección» abre; pestañas Gastos, Tarjetas y Cuentas con las cifras de siempre; escribir `1.005` en un importe sigue dando el aviso del navegador.

## Cómo deshacerlo
* **Volver a la 1.36.0:** cerrar, restituir `MiChelitos-1.36.0.app` en `/Applications`. **No hace falta tocar la base**: el esquema es el mismo.
* **Volver a un estado de datos anterior:** Ajustes → Restaurar un respaldo (toma antes una copia de lo actual).
* La 1.3.5 (guardada en `~/.michelitos/aplicacion-anterior/`) **no** puede abrir esta base: no volver a ella.
