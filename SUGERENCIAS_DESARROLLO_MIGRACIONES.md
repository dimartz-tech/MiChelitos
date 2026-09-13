# Sugerencias de desarrollo: migraciones de datos seguras

Este documento contiene recomendaciones genéricas. No incluye datos personales, información financiera, nombres de repositorios, rutas locales, identificadores de versiones del proyecto ni fragmentos de su código. Es una propuesta de trabajo, no una certificación del estado de una aplicación.

## Objetivo

Permitir que una instalación existente actualice su estructura de almacenamiento de forma verificable, recuperable y compatible con sus datos, preservando la separación entre dominio, aplicación e infraestructura.

## 1. Diseñar el contrato de migración

- Definir una secuencia de versiones de esquema con identificadores únicos y ordenados.
- Para SQLite, utilizar `PRAGMA user_version` o una tabla de control equivalente.
- Registrar la nueva versión únicamente cuando los cambios y sus verificaciones hayan terminado correctamente.
- Rechazar de forma explícita una base cuya versión sea superior a la soportada por la aplicación.
- Tratar las migraciones ya publicadas como inmutables: toda corrección posterior debe ser una nueva migración.

**Criterio de aceptación:** el programa identifica sin ambigüedad la versión instalada y ejecuta exclusivamente las migraciones pendientes.

## 2. Incorporar instalaciones anteriores al versionado

Una base sin versión no debe considerarse automáticamente vacía ni compatible. Puede contener cambios aplicados parcialmente.

- Diferenciar una instalación nueva de una base histórica sin versionar.
- Inspeccionar tablas, columnas, restricciones e índices para reconocer los esquemas históricos soportados.
- Verificar estructuras completas; la existencia de una columna aislada no demuestra que toda una migración se aplicó.
- Definir una ruta de actualización para cada estructura reconocida.
- Si la estructura es desconocida o inconsistente, detener la actualización con un diagnóstico útil.
- No deducir datos financieros faltantes de forma silenciosa. Cualquier transformación con significado de negocio requiere una regla aprobada.

**Criterio de aceptación:** las bases antiguas admitidas conservan sus datos; las incompatibles se rechazan antes de alterarlas.

## 3. Garantizar atomicidad y exclusión de escritores

- Ejecutar cada migración dentro de una transacción que incluya la modificación del esquema, la transformación de datos y el cambio de versión.
- Impedir operaciones normales de la aplicación durante la actualización.
- Coordinar también otras instancias del proceso mediante el bloqueo transaccional adecuado; un bloqueo en memoria no protege frente a otra instancia.
- Definir si el conjunto completo de migraciones es atómico o si cada versión confirmada constituye un punto válido de reanudación.
- Tras un fallo, no habilitar la aplicación con una versión de esquema que todavía no soporta.
- Revisar las particularidades de SQLite al reconstruir tablas o cambiar restricciones de claves foráneas: no asumir que todos los ajustes de PRAGMA pueden hacerse dentro de una transacción.

**Criterio de aceptación:** un fallo intermedio revierte la migración en curso y deja una versión consistente y reconocible.

## 4. Propagar errores sin ocultarlos

- Evitar descartar resultados de sentencias de esquema o actualizaciones de datos.
- Diferenciar una condición esperada de un fallo por permisos, bloqueo, falta de espacio, corrupción o restricción incumplida.
- Incluir en el error el identificador de la migración y la etapa fallida, sin valores de registros ni rutas personales.
- Validar cuántas filas fueron afectadas cuando la operación tenga una cardinalidad esperada.
- Si un error impide garantizar consistencia, abortar la transacción y conservar el diagnóstico.

**Criterio de aceptación:** ninguna migración se declara exitosa después de ignorar un error.

## 5. Respaldar y practicar la restauración

- Crear un respaldo consistente antes de modificar una base existente.
- Utilizar un mecanismo compatible con SQLite y con su modo de journaling; copiar solo el archivo principal mientras hay escrituras o WAL activo puede producir un respaldo incompleto.
- Verificar que el respaldo puede abrirse y supera las comprobaciones de integridad.
- Mantener respaldos fuera del repositorio y establecer acceso restringido y una política de retención.
- Probar el procedimiento de restauración sobre una ubicación temporal.
- Evitar restauraciones automáticas que sobrescriban datos nuevos sin comprobar su procedencia y estado.

**Criterio de aceptación:** existe una copia recuperable y un procedimiento de recuperación demostrado antes de actualizar instalaciones con datos.

## 6. Separar estructura, semillas y reglas de negocio

- Mantener SQL y control de versiones dentro del adaptador de persistencia.
- Ejecutar la preparación del almacenamiento desde la composición o arranque, antes de aceptar comandos.
- Mantener dominio y casos de uso independientes de conexiones, tablas y números de migración.
- Separar la creación de estructura, la transformación histórica y las semillas de sistema.
- Identificar registros de sistema por claves estables o roles, evitando depender de nombres editables.
- No convertir errores de lectura en colecciones vacías: ausencia de datos y fallo de almacenamiento son situaciones distintas.

**Criterio de aceptación:** las reglas de negocio pueden probarse sin ejecutar migraciones ni inicializar la interfaz.

## 7. Planificar las transformaciones monetarias por separado

- Antes de cambiar la representación de importes, definir precisión, límites, redondeo y tratamiento de valores históricos inválidos.
- Considerar enteros de unidades mínimas para importes y una representación de precisión explícita para tasas.
- Preservar la divisa y las relaciones entre importe original, importe liquidado y cargos.
- Reconciliar saldos y movimientos antes y después, por divisa y por entidad.
- No usar redondeos, recortes o valores por defecto para ocultar diferencias.
- Tratar cualquier desviación como un resultado que debe explicarse antes de aceptar la conversión.

**Criterio de aceptación:** la transformación conserva los importes conforme a la política aprobada y produce una reconciliación verificable.

## 8. Construir pruebas con datos sintéticos

Usar bases temporales independientes por prueba, sin leer bases reales ni cambiar globalmente el directorio personal del proceso.

| Escenario | Resultado esperado |
|---|---|
| Instalación vacía | Esquema actual y semillas válidas |
| Cada versión histórica soportada | Actualización correcta y datos preservados |
| Base histórica sin versión | Reconocimiento explícito o rechazo seguro |
| Base parcialmente actualizada | Diagnóstico o recuperación previamente definida |
| Segunda ejecución sin cambios | Ninguna transformación repetida |
| Fallo a mitad de migración | Rollback y versión anterior consistente |
| Versión más reciente que la aplicación | Rechazo sin modificar la base |
| Relaciones inválidas | Fallo informado y ninguna confirmación parcial |
| Dos instancias simultáneas | Sin doble ejecución ni escrituras concurrentes inseguras |
| Respaldo y restauración | Recuperación de una base utilizable |
| Transformación de importes | Reconciliación por divisa y entidad |

Complementar las pruebas con `PRAGMA foreign_key_check` e `integrity_check`. Estas comprobaciones estructurales no sustituyen la reconciliación de negocio.

## 9. Orden sugerido de implementación

1. Especificar versiones soportadas, esquema inicial y política de fallos.
2. Crear fixtures sintéticos de las estructuras históricas admitidas.
3. Implementar el ejecutor versionado y sus transacciones.
4. Incorporar respaldo consistente y restauración verificable.
5. Trasladar las transformaciones existentes a migraciones explícitas y eliminar errores descartados.
6. Validar actualizaciones, reanudación, concurrencia y conservación de datos.
7. Abordar las transformaciones monetarias mediante una migración independiente.
8. Documentar las garantías verificadas y los casos todavía no soportados.

## Criterios para cerrar la tarea

- Toda actualización tiene una versión y una ruta de ejecución explícitas.
- No se ignoran errores que afecten la estructura o los datos.
- Los fallos dejan un estado consistente y recuperable.
- Los respaldos y la restauración están probados.
- Las estructuras históricas soportadas tienen pruebas de actualización.
- Las restricciones y la reconciliación se verifican antes de aceptar el resultado.
- La documentación describe garantías comprobadas, sin datos del proyecto ni información personal.
