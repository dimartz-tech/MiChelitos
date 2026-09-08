# 🏗️ Plan de Refactorización — Arquitectura Hexagonal y SOLID

**Proyecto:** MiChelitosTauri v1.3.5
**Fecha del plan:** 2026-09-08
**Estado:** PROPUESTA — no se ha modificado ningún archivo de código.

---

## 0. Resumen ejecutivo

Este documento propone reorganizar internamente MiChelitosTauri hacia una **arquitectura hexagonal (puertos y adaptadores)** guiada por **principios SOLID**, sin cambiar el stack, sin añadir dependencias de ejecución y sin alterar la experiencia de uso existente.

El objetivo no es "modernizar por moda": es que las **reglas financieras** de la aplicación (comisiones, retenciones, conversión de divisa, disponibilidad de crédito) puedan **probarse de forma aislada y demostrarse correctas**, algo que hoy es imposible porque están entrelazadas con SQL y con generación de HTML.

**Restricciones de diseño aceptadas como no negociables:**

| Restricción | Cómo se respeta |
|---|---|
| Mantener la estructura | Se conserva `src/` + `src-tauri/`, las 11 pestañas, los flujos y el look actual |
| Peso contenido | Cero dependencias nuevas en tiempo de ejecución; sin bundler; sin framework de test externo |
| Responsable | Toda mutación de saldo sigue dentro de transacciones SQL; se añade rastro de auditoría |
| Auditable | Git + commits por vertical + pruebas antes/después + bitácora de decisiones |
| Mejorar usabilidad | Cambios acotados de UX que no alteran la estructura de navegación |

---

## 1. Copia de seguridad realizada

Antes de cualquier planificación se generó una copia íntegra del proyecto y de los datos.

```
/Users/wilfoddiaz/Desktop/MiChelitosTauri_backup_2026-09-08/
├── proyecto/              (40 archivos — código, docs, configuración, íconos)
└── base_de_datos_viva/    (~/.michelitos/databases/ completo: sql/ + nosql/ + respaldos)
```

**Verificación de integridad:**

* SHA-1 de la base de datos viva: `e1bdfeb126ac48c8e190842c9e7a8edd43618389` — **idéntico** en origen y copia.
* Conteo de archivos del proyecto: **40 = 40**.
* Peso total de la copia: **5.0 MB**.

**Exclusión deliberada:** `src-tauri/target/` (3.2 GB) y `node_modules/` (14 MB) no se copiaron por ser **artefactos reproducibles** — se regeneran con `cargo build` y `npm install`. Copiarlos habría inflado el respaldo 640× sin aportar información recuperable.

> ⚠️ **Hallazgo relevante durante la copia:** el archivo `michelitos.db` en la raíz del proyecto (fecha 2026-08-17) **no es la base de datos que usa la aplicación**. El backend lee y escribe en `~/.michelitos/databases/sql/michelitos.db` (modificado hoy, 2026-09-08). El de la raíz es una copia obsoleta con datos reales, desincronizada hace tres semanas. Ver §2.

---

## 2. Reglas a añadir a `.gitignore`

### 2.1 Estado actual

```gitignore
node_modules/
src-tauri/target/
.vscode/
.idea/
.DS_Store
```

### 2.2 Adiciones propuestas

```gitignore
# --- Datos financieros reales (NUNCA versionar) ---
*.db
*.db-journal
*.db-wal
*.db-shm
*.sqlite
*.sqlite3
*.backup_*
michelitos.db*

# --- Respaldos y copias del proyecto ---
*_backup_*/
*.bak

# --- Artefactos de empaquetado macOS ---
*.dmg
*.app/

# --- Entorno y secretos ---
.env
.env.local
.env.*.local

# --- Resultados de pruebas y cobertura ---
/cobertura/
/coverage/
*.profraw
*.gcda
```

### 2.3 ¿Por qué exactamente estas reglas?

**`*.db` y derivados — la regla crítica.**
Hoy el `.gitignore` **no excluye ningún archivo de base de datos**. En el momento en que se ejecute `git init && git add .` —paso obligatorio para lograr la auditabilidad que se pide— quedarían versionados de forma permanente `michelitos.db` y `michelitos.db.backup_2026-08-17_162833`, que contienen **balances bancarios reales, RNC de clientes, números de factura y montos cobrados**. Un dato committeado sobrevive al `git rm`: permanece en el historial y en cualquier clon o remoto. Esta regla debe existir **antes** del primer commit, no después.

**`*-journal`, `*-wal`, `*-shm`.**
SQLite genera estos archivos temporales durante transacciones. Versionarlos produce conflictos ilegibles y puede dejar la base en estado inconsistente al hacer checkout.

**`*.backup_*` y `*_backup_*/`.**
El protocolo actual del proyecto (documentado en `archivo_de_control.md` §1.3.5) crea respaldos con marca de tiempo antes de cada migración. Es una buena práctica que se debe conservar — pero esos respaldos son *datos*, no *código*, y engordan el repositorio con binarios de ~90 KB cada uno que nunca se diferencian bien.

**`*.dmg` y `*.app/`.**
El `tauri.conf.json` genera un instalador DMG por compilación. Son artefactos de decenas de MB, reproducibles desde el código fuente.

**`.env*` y cobertura.**
Preventivo. Si en el futuro se añaden claves de API (tasas de cambio automáticas, por ejemplo) o se activa cobertura de pruebas, las reglas ya estarán.

### 2.4 Acción complementaria recomendada (no incluida en `.gitignore`)

Ignorar `michelitos.db` de la raíz **no basta**: el archivo debería **eliminarse del directorio del proyecto**, ya que:

1. No lo usa la aplicación (el backend apunta a `~/.michelitos/`).
2. Su sola presencia induce al error de creer que es la base activa.
3. Ya está respaldado en `MiChelitosTauri_backup_2026-09-08/`.

*(Esta eliminación no se ha ejecutado — requiere confirmación explícita.)*

---

## 3. Diagnóstico: por qué se propone el cambio

### 3.1 Medición del estado actual

| Componente | Líneas | Responsabilidades mezcladas |
|---|---:|---|
| `src/js/ui.js` | 2,874 | Enrutado, render HTML, validación, formato de moneda, cálculo de KPI, llamadas IPC, modales |
| `src-tauri/src/main.rs` | 1,504 | 39 comandos Tauri + SQL crudo + reglas de negocio + mapeo de errores |
| `src-tauri/src/db_sql.rs` | 264 | Esquema + migraciones ad-hoc + semillas |
| `src/js/api.js` | 254 | Puente IPC (**única capa hoy bien delimitada**) |
| `src/js/app.js` | 91 | Bootstrap + router |
| `src-tauri/src/db_nosql.rs` | 72 | Persistencia JSON de capital |

### 3.2 Los tres problemas concretos que esto causa

**Problema 1 — Las reglas financieras no son verificables.**
La excepción impositiva del TSS vive dentro de `crear_gasto()`, mezclada con la apertura de conexión SQLite, la consulta de la categoría y el `UPDATE` del balance:

```rust
let is_tss_tax = cat_nom.to_lowercase() == "impuestos"
              && input.descripcion.to_uppercase().contains("TSS");
if !is_tss_tax { costo_adicional = (input.monto * 0.002).round(); }
```

Para probar esta regla —que decide si se cobran 0.20 % o 0.00 %— hoy hay que levantar una base de datos, insertar una categoría, insertar un gasto y leer el resultado. **Por eso no existen pruebas.** Y sin pruebas, cada release nuevo puede romper silenciosamente un cálculo de dinero, que es exactamente el tipo de fallo que el usuario descubre semanas después, cuadrando saldos a mano.

**Problema 2 — Cada funcionalidad nueva agranda el mismo archivo.**
El `historial_versiones.md` lo evidencia: de 1.2.0 a 1.3.5, cada versión añadió lógica dentro de `ui.js` y `main.rs`. La v1.3.4 tuvo que corregir un descuadre de divisas (`Normalización de Visualización en USD`) que se originó precisamente porque la suma de montos estaba dispersa entre varios `renderX()`, sin un único lugar que supiera "no se suman divisas distintas".

**Problema 3 — La reversión destruye el rastro.**
Los comandos `eliminar_gasto`, `eliminar_ingreso`, `eliminar_transaccion_cuenta` **borran el registro** y revierten el saldo. Correcto desde el punto de vista del balance; grave desde el punto de vista de la auditabilidad: no queda constancia de que la transacción existió ni de que fue anulada. En un sistema financiero esto se resuelve con **asientos de compensación**, no con `DELETE`.

### 3.3 Lo que ya está bien y NO se debe tocar

Es importante delimitar el alcance para no romper lo que funciona:

* ✅ **`api.js` ya es un puerto.** Un método por comando, sin lógica. Se formaliza, no se reescribe.
* ✅ **`ui.js` ya está seccionado por vista** (`renderDashboard`, `renderGastos`, `renderTarjetas`…). Las costuras para dividirlo **ya existen**; el trabajo es mecánico, no de rediseño.
* ✅ **El uso de transacciones SQL** para operaciones de saldo (regla ya documentada en `archivo_de_control.md` §2). Se conserva y se refuerza.
* ✅ **El protocolo de respaldo pre-migración.** Se automatiza, no se sustituye.
* ✅ **El enfoque sin bundler.** Es la razón por la que el proyecto es ligero y auditable. Se mantiene con módulos ES nativos.

---

## 4. Arquitectura objetivo

### 4.1 El hexágono aplicado a este proyecto

```
        ┌──────────── ADAPTADORES PRIMARIOS (conducen) ────────────┐
        │   Comandos Tauri (Rust)      Vistas del navegador (JS)   │
        └────────────────────┬─────────────────────────────────────┘
                             │  llaman a
        ┌────────────────────▼─────────────────────────────────────┐
        │                  APLICACIÓN (casos de uso)                │
        │   RegistrarGasto · AbonarTarjeta · TransferirEntreCuentas │
        │   CobrarFactura · RevertirMovimiento · ProcesarSuscrip.   │
        ├───────────────────────────────────────────────────────────┤
        │                 DOMINIO (núcleo puro)                     │
        │   Dinero · Divisa · TasaCambio · Comisiones · Retenciones │
        │   DisponibleTarjeta · CicloFacturación                    │
        │   ── sin SQL, sin HTML, sin Tauri, sin fecha del sistema ─│
        └────────────────────┬─────────────────────────────────────┘
                             │  define (traits / contratos)
        ┌────────────────────▼─────────────────────────────────────┐
        │                     PUERTOS                               │
        │   RepositorioGastos · RepositorioCuentas · RepoTarjetas   │
        │   RelojDelSistema · RepositorioCapital                    │
        └────────────────────┬─────────────────────────────────────┘
                             │  implementados por
        ┌────────────────────▼─────────────────────────────────────┐
        │           ADAPTADORES SECUNDARIOS (conducidos)            │
        │   SQLite (rusqlite) · JSON (capital) · Reloj real         │
        │   ── y en pruebas: dobles en memoria ──                   │
        └───────────────────────────────────────────────────────────┘
```

**La regla de dependencia es la única que importa:** las flechas apuntan siempre hacia adentro. El dominio no sabe que existe SQLite, ni Tauri, ni el DOM. Eso es lo que permite probarlo sin infraestructura.

### 4.2 Estructura de carpetas propuesta — Backend (Rust)

```
src-tauri/src/
├── main.rs                        # SOLO bootstrap + generate_handler!  (~60 líneas)
│
├── dominio/                       # Núcleo puro. Cero dependencias externas.
│   ├── mod.rs
│   ├── dinero.rs                  # Divisa, Monto, TasaCambio — impide sumar DOP + USD
│   ├── comisiones.rs              # 0.20 %, LBTR +100, excepción TSS
│   ├── retenciones.rs             # ISR 15 % en facturas formales
│   ├── tarjeta.rs                 # disponible = límite + sobregiro − uso; "al día"; sugerencia
│   ├── suscripcion.rs             # ¿corresponde cobrar hoy? (idempotencia)
│   └── errores.rs                 # ErrorDominio tipado (reemplaza String)
│
├── aplicacion/                    # Casos de uso. Orquestan puertos. Sin SQL.
│   ├── mod.rs
│   ├── registrar_gasto.rs
│   ├── abonar_tarjeta.rs          # incluye conversión DOP→USD con tasa
│   ├── transferir_entre_cuentas.rs
│   ├── cobrar_factura.rs
│   ├── cobrar_ingreso_informal.rs
│   ├── revertir_movimiento.rs
│   └── procesar_suscripciones.rs
│
├── puertos/
│   ├── mod.rs
│   ├── repositorios.rs            # traits: RepositorioGastos, RepositorioCuentas, …
│   └── reloj.rs                   # trait Reloj — hace las fechas deterministas en test
│
└── adaptadores/
    ├── mod.rs
    ├── sqlite/
    │   ├── mod.rs
    │   ├── conexion.rs
    │   ├── migraciones.rs         # versionadas con PRAGMA user_version
    │   ├── gastos.rs · cuentas.rs · tarjetas.rs · ingresos.rs · …
    ├── json/
    │   └── capital.rs             # el actual db_nosql.rs
    └── tauri/                     # Los 39 comandos, agrupados por dominio
        ├── mod.rs
        ├── gastos.rs · cuentas.rs · tarjetas.rs · ingresos.rs · ajustes.rs
```

### 4.3 Estructura de carpetas propuesta — Frontend (JS)

```
src/js/
├── app.js                         # bootstrap + router (se conserva casi igual)
│
├── nucleo/                        # Puro. Testeable en Node, sin DOM, sin Tauri.
│   ├── dinero.js                  # formato Intl.NumberFormat por divisa
│   ├── totales.js                 # agregación por divisa — impide mezclar DOP/USD
│   ├── desglose.js                # % por categoría (la lógica de la v1.3.3)
│   ├── ciclo-tarjetas.js          # sugerencia de tarjeta por corte más lejano
│   └── validacion.js              # reglas de formulario reutilizables
│
├── puertos/
│   └── api.js                     # contrato IPC — YA EXISTE, solo se documenta
│
├── adaptadores/
│   ├── tauri-ipc.js               # invoke real (extraído de api.js)
│   └── vistas/                    # un módulo por pestaña — extraídos de ui.js
│       ├── dashboard.js · ingresos.js · gastos.js · tarjetas.js
│       ├── suscripciones.js · cuentas.js · efectivo.js · capital.js
│       ├── prestamos.js · resumen.js · ajustes.js
│
└── ui/
    ├── toast.js
    ├── modal.js
    └── componentes.js             # kpi-box, tabla, barra de progreso
```

**Mecanismo:** módulos ES nativos vía `<script type="module">`. Funcionan en el WebView de Tauri sin transpilación. **No se añade bundler, ni npm run build, ni dependencias.** El peso del `.dmg` no aumenta.

### 4.4 Tipado del frontend — decisión diferida a la Fase 7

Existe hoy una **asimetría** entre los dos lados de la aplicación. El backend Rust obtiene seguridad de divisas en **tiempo de compilación**: `Dinero::sumar()` entre DOP y USD no llega a ejecutarse mal. El núcleo JS replica la misma invariante, pero solo puede lanzar el error **en tiempo de ejecución** — es decir, cuando el usuario ya pulsó el botón.

Cerrar esa brecha en el frontend es deseable, pero la forma de hacerlo condiciona el criterio de peso contenido. Se evalúan tres opciones en la Fase 7, cuando exista el código real que tipar:

| Opción | Detecta errores | Paso de compilación | Dependencias | Archivo ejecutado |
|---|---|---|---|---|
| **A. JS puro** (actual) | en ejecución | no | 0 | el `.js` escrito |
| **B. JSDoc + `@ts-check`** | al escribir | **no** | 1 de desarrollo | el `.js` escrito |
| **C. TypeScript completo** | al escribir | **sí** | varias + bundler | un `.js` generado |

**Opción B como candidata principal.** TypeScript puede verificar archivos `.js` corrientes cuando los tipos se declaran en comentarios JSDoc y el archivo abre con `// @ts-check`. Se obtiene la comprobación estática y el autocompletado del editor **sin compilar nada, sin generar archivos y sin tocar el runtime**: el `.js` que se escribe sigue siendo exactamente el que el WebView ejecuta.

```js
// @ts-check
/** @typedef {'DOP' | 'USD'} Divisa */

/**
 * @param {{monto: number, divisa: Divisa}} a
 * @param {{monto: number, divisa: Divisa}} b
 * @returns {{monto: number, divisa: Divisa}}
 */
export function sumar(a, b) { /* ... */ }
```

**Por qué se difiere y no se decide ahora:** tipar es una operación sobre código que todavía no existe. `ui.js` sigue siendo un archivo de 2 874 líneas; los once módulos de vista que se tiparían nacen precisamente en la Fase 7. Decidirlo antes obligaría a elegir a ciegas.

**Criterios con los que se decidirá en su momento:**

1. ¿La opción B detecta los errores que realmente aparecen en este código (divisas cruzadas, campos ausentes en respuestas IPC, `null` no contemplado)? Si los cubre, gana por no requerir compilación.
2. ¿La verbosidad de JSDoc degrada la legibilidad más de lo que aporta? En funciones de render con muchos parámetros puede ser costoso.
3. ¿La opción C introduciría un `node_modules` de producción o un artefacto generado? Si es así, contradice el criterio de peso contenido y queda descartada salvo que B resulte insuficiente.
4. La opción A permanece como salida válida: **no tipar es una decisión legítima** si el coste supera al beneficio en este proyecto.

**Restricción previa:** cualquiera de las tres opciones exige que el paso 0.8 —módulos ES confirmados en el WebView— esté verificado. Sin eso, la división de `ui.js` no puede empezar y esta decisión no llega a plantearse.

---

## 5. Principios SOLID — aplicación concreta

No como teoría, sino señalando el punto exacto del código actual que cada principio corrige:

| Principio | Dónde se incumple hoy | Cómo se resuelve |
|---|---|---|
| **S** — Responsabilidad única | `crear_gasto()` calcula comisión + consulta categoría + inserta gasto + actualiza balance de cuenta/tarjeta/efectivo | Se separa en `dominio::comisiones` (calcula), `aplicacion::RegistrarGasto` (orquesta), `adaptadores::sqlite` (persiste) |
| **O** — Abierto/cerrado | Añadir un método de pago obliga a editar el `match` dentro de `crear_gasto` | `MetodoPago` como enum del dominio con su propia lógica de afectación de saldo; añadir uno no toca los existentes |
| **L** — Sustitución de Liskov | No aplica hoy (no hay abstracciones) | Cualquier implementación de `RepositorioCuentas` (SQLite o en memoria) debe ser intercambiable — se verifica con la **misma batería de pruebas de contrato** aplicada a ambas |
| **I** — Segregación de interfaces | Un único `obtener_conexion()` global que todo comando usa | Puertos pequeños y específicos: `RepositorioGastos` no expone métodos de tarjetas |
| **D** — Inversión de dependencias | `main.rs` depende directamente de `rusqlite::Connection` | Los casos de uso dependen de *traits* definidos por el dominio; SQLite se inyecta en `main.rs` |

**El principio D es el que habilita todo lo demás:** sin él, no hay forma de sustituir SQLite por un doble en memoria, y sin eso no hay pruebas unitarias rápidas.

---

## 6. Estrategia de pruebas

### 6.1 Herramientas — y por qué estas (peso contenido)

| Capa | Herramienta | Dependencias añadidas |
|---|---|---|
| Rust — unitarias | `cargo test` (nativo, `#[cfg(test)]`) | **0** |
| Rust — Gherkin | `cucumber` crate como `[dev-dependencies]` | 0 en el binario final |
| JS — unitarias | `node:test` + `node:assert` (nativo en Node 18+) | **0** |
| JS — Gherkin | Runner propio de ~80 líneas sobre `node:test` | **0** |

Decisión clave: **nada de Jest, Vitest, Mocha o Chai.** Node trae un ejecutor de pruebas nativo desde la v18. Añadir Vitest supondría ~200 paquetes en `node_modules` y un `vite.config.js`, contradiciendo directamente el criterio de peso contenido. `cucumber` es la única excepción, y solo como dependencia de desarrollo: **no entra en el binario de producción** gracias a `[dev-dependencies]`.

### 6.2 Protocolo antes/después — obligatorio para cada componente

Este es el mecanismo que garantiza que la refactorización **no cambia el comportamiento financiero**:

```
  PASO 1 — ANTES (prueba de caracterización)
    Se escribe una prueba que ejercita el código ACTUAL, tal como está,
    y se registra el resultado que produce hoy — incluidas sus rarezas.
    Esta prueba DEBE PASAR contra el código sin refactorizar.
    ↓
  PASO 2 — EXTRACCIÓN
    Se mueve la lógica al dominio. No se corrige nada todavía.
    ↓
  PASO 3 — DESPUÉS (misma prueba, sin modificar)
    La prueba de caracterización se re-apunta al nuevo componente
    y DEBE SEGUIR PASANDO con resultados idénticos, al centavo.
    ↓
  PASO 4 — PRUEBAS UNITARIAS NUEVAS
    Ya con el componente aislado, se añaden los casos límite que antes
    era imposible probar (montos cero, negativos, tasa 0, divisa cruzada).
    ↓
  PASO 5 — DIVERGENCIAS
    Si el paso 3 revela un comportamiento incorrecto del código original,
    NO se corrige silenciosamente: se documenta en archivo_de_control.md
    como hallazgo, y se decide de forma explícita si se corrige o preserva.
```

> **La razón del paso 5:** en un sistema financiero, "arreglar" un cálculo durante una refactorización descuadra saldos históricos ya registrados en la base de datos. Toda corrección de fórmula debe ser una decisión consciente y versionada, nunca un efecto colateral.

### 6.3 Inventario de componentes estructurales y sus pruebas

| # | Componente | Prueba ANTES (caracterización) | Prueba DESPUÉS (unitaria aislada) |
|---|---|---|---|
| 1 | `dominio::comisiones` | Gasto por transferencia de 10 000 DOP → comisión 20 DOP | Redondeo, monto 0, LBTR fijo +100, comisión sobre montos con decimales |
| 2 | `dominio::comisiones` (TSS) | Categoría "Impuestos" + desc. "Pago TSS" → comisión 0.00 | Variantes de caja: "tss", "TSS agosto", categoría "impuestos" vs "Impuestos"; y el caso negativo: "Impuestos" sin TSS → 0.20 % |
| 3 | `dominio::retenciones` | Factura de 100 000 → retenido 15 000 | Porcentaje configurable, informal → retención 0 |
| 4 | `dominio::dinero` | Suma de dos montos DOP | **Sumar DOP + USD debe fallar en compilación o devolver error** (raíz del bug corregido en v1.3.4) |
| 5 | `dominio::tarjeta` | Disponible = límite + sobregiro − balance, por divisa | Sobregiro cero, balance sobre el límite, "al día" con corte 0 |
| 6 | `dominio::tarjeta` (sugerencia) | Devuelve la tarjeta con corte más lejano | Empates, mes con cambio de año, corte día 31 en febrero |
| 7 | `dominio::suscripcion` | Cobra si hoy ≥ día_facturación y no se cobró este mes | **Idempotencia**: dos arranques el mismo día no duplican el cargo |
| 8 | `aplicacion::AbonarTarjeta` | Abono USD desde cuenta DOP: debita monto×tasa, acredita USD, registra comisión | Tasa 0 → error de dominio; misma divisa → sin conversión; fondos insuficientes |
| 9 | `aplicacion::TransferirEntreCuentas` | Debita origen + cargo, acredita destino | Atomicidad: si falla el crédito, no se debita nada |
| 10 | `aplicacion::RevertirMovimiento` | Revertir gasto restaura el balance exacto | Reversión doble; reversión de gasto ya revertido |
| 11 | `adaptadores::sqlite::*` | Contrato: guardar → leer devuelve lo mismo | **Misma batería aplicada al doble en memoria** (verificación de Liskov) |
| 12 | `adaptadores::sqlite::migraciones` | BD v1.3.5 real migra sin pérdida | Migración desde esquema vacío; migración repetida es no-op |
| 13 | `nucleo/dinero.js` | Formatea 1234.5 DOP → "RD$ 1,234.50" | Cero, negativos, USD, miles de millones |
| 14 | `nucleo/totales.js` | Totales del mes por divisa, separados | Lista vacía, una sola divisa, ambas divisas |
| 15 | `nucleo/desglose.js` | % por categoría suma 100 | Categoría única = 100 %, sin gastos = sin división por cero |
| 16 | `nucleo/validacion.js` | Monto vacío → inválido | Coma decimal, notación científica, espacios |

**Criterio de aceptación de la refactorización:** las 16 pruebas de caracterización pasan idénticas antes y después. Sin esto, la migración no se da por buena.

### 6.4 Prueba Gherkin de caso de uso

Se elige el **abono en USD a tarjeta desde cuenta en DOP** (funcionalidad de la v1.3.4) porque es el único flujo que **atraviesa el hexágono completo**: entrada de usuario → conversión de divisa → comisión bancaria → débito de cuenta → crédito de tarjeta → registro de gasto derivado. Si esta prueba pasa, la arquitectura funciona de extremo a extremo.

**Archivo:** `features/abono_tarjeta_multidivisa.feature`

```gherkin
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
    Y debe existir un gasto de categoría "Comisiones Bancarias" por 60.00 "DOP"

  Escenario: La tasa de cambio es obligatoria al cruzar divisas
    Cuando registro un abono de 500.00 "USD" a la tarjeta "Tarjeta Ejemplo"
      Desde la cuenta "Cuenta Ahorros DOP" con una tasa de cambio de 0.00
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
    Y no debe existir ningún gasto de categoría "Comisiones Bancarias"
```

> **Por qué el cuarto escenario:** la regla ya documentada en `archivo_de_control.md` §2 ("toda operación que afecte saldos debe ocurrir dentro de una transacción SQL") hoy es una convención que nadie verifica. Este escenario la convierte en una **prueba ejecutable**: si alguien en el futuro escribe un caso de uso sin transacción, la suite falla.

---

## 7. Enfoques de abordaje

### Enfoque A — Estrangulamiento por vertical de dominio ⭐ **RECOMENDADO**

Se migra **un dominio completo a la vez** (dominio → puerto → adaptador → comandos → vista), dejando el resto del sistema intacto y funcionando. El código viejo y el nuevo coexisten hasta que el vertical queda cerrado.

**Orden propuesto, por densidad de reglas de negocio:**

| Fase | Vertical | Por qué en esta posición |
|---|---|---|
| 0 | Cimientos: `git init`, `.gitignore`, `dominio::dinero`, `puertos::Reloj`, andamiaje de pruebas | Nada se puede probar sin base; `Dinero` es el tipo del que dependen todos los demás |
| 1 | **Gastos** | Concentra más reglas que ningún otro: 0.20 %, LBTR, excepción TSS, tres métodos de pago, reversión |
| 2 | **Tarjetas y abonos** | Conversión de divisa, doble límite, sobregiro, ciclo de facturación |
| 3 | **Cuentas y transferencias** | Atomicidad entre dos saldos |
| 4 | **Ingresos** (formales + informales) | Retención 15 %, cobro con abono a cuenta |
| 5 | **Suscripciones** | Idempotencia y dependencia de fecha |
| 6 | **Capital y préstamos** | Menor densidad de reglas |
| 7 | Frontend: división de `ui.js` por vista + `nucleo/` | Se apoya en el backend ya estabilizado |
| 8 | Auditoría: asientos de compensación, migraciones versionadas, cierre de `allowlist` | Requiere el resto en su sitio |

**Ventajas**
* La aplicación **compila y funciona al terminar cada fase**. En ningún momento hay una versión rota.
* Cada fase es un commit revisable de tamaño humano → **auditabilidad real**.
* Si el proyecto se interrumpe en la fase 3, lo hecho ya aporta valor: Gastos y Tarjetas ya están probados.
* Permite seguir publicando funcionalidades entre fases.
* El riesgo se acota por vertical: un error en Gastos no puede afectar a Préstamos.

**Desventajas**
* Convivencia temporal de dos estilos de código (~el tiempo que dure la migración). Puede confundir si no se documenta qué vertical ya migró.
* Requiere disciplina: la tentación de "aprovechar y arreglar aquello" rompe la paridad de comportamiento.
* Total de trabajo ligeramente mayor por el andamiaje de coexistencia.

**Mitigación de la desventaja principal:** una tabla de estado al inicio de `archivo_de_control.md` indicando qué verticales están migrados y cuáles no.

---

### Enfoque B — Reestructuración por capas en una sola pasada

Se crea el esqueleto hexagonal completo y se reubican **los 39 comandos y las 11 vistas de una vez**, antes de escribir ninguna prueba.

**Ventajas**
* Coherencia estructural inmediata: no hay período híbrido.
* Un único cambio conceptual que entender.
* Menor trabajo total de andamiaje.

**Desventajas**
* **Ventana prolongada con la aplicación sin compilar.** Sobre 4 400 líneas movidas simultáneamente, cualquier error se manifiesta al final, cuando ya hay cientos de cambios superpuestos.
* **Contradice el requisito de auditabilidad.** Un commit de 4 400 líneas movidas es, en la práctica, irrevisable: el `diff` no distingue "movido" de "modificado", y ahí es exactamente donde se cuela un `0.002` convertido en `0.02`.
* **Rompe el protocolo antes/después.** No hay línea base de comportamiento contra la que comparar, porque el código original desaparece en el mismo cambio.
* Si se detiene a mitad, no queda nada aprovechable.

**Cuándo sí tendría sentido:** si el proyecto ya tuviera una suite de pruebas de integración completa que actuara como red. **No es el caso.**

---

### Enfoque C — Núcleo puro primero (alcance mínimo)

Se extraen **únicamente las reglas financieras puras** a `dominio/`, dejando el acceso SQL y los comandos Tauri exactamente donde están. `crear_gasto()` sigue en `main.rs`, pero llama a `dominio::comisiones::calcular(...)` en vez de calcular en línea.

**Ventajas**
* **El menor esfuerzo por unidad de riesgo eliminado.** Se puede completar en una fracción del tiempo de A.
* Ataca directamente el problema más grave (§3.1): las fórmulas de dinero pasan a ser verificables de inmediato.
* Cambio casi invisible: `main.rs` apenas se reduce, el riesgo de regresión es mínimo.
* No requiere reorganizar carpetas ni tocar el frontend.

**Desventajas**
* **No es arquitectura hexagonal.** No hay puertos ni inversión de dependencias; los casos de uso siguen acoplados a `rusqlite`.
* `ui.js` sigue con 2 874 líneas: el problema de mantenibilidad del frontend queda intacto.
* No permite probar casos de uso completos ni ejecutar la prueba Gherkin de §6.4, que necesita dobles de repositorio.
* Cumple parcialmente los criterios solicitados (SOLID sí, hexagonal no).

---

### Comparativa y recomendación

| Criterio solicitado | A — Vertical | B — Una pasada | C — Núcleo puro |
|---|:---:|:---:|:---:|
| Arquitectura hexagonal completa | ✅ | ✅ | ❌ |
| Principios SOLID | ✅ | ✅ | 🟡 parcial |
| Pruebas antes/después por componente | ✅ | ❌ | 🟡 solo dominio |
| Gherkin de caso de uso completo | ✅ | ✅ | ❌ |
| Mantiene la estructura | ✅ | ✅ | ✅ |
| Peso contenido | ✅ | ✅ | ✅ |
| Auditable | ✅ | ❌ | ✅ |
| App siempre funcional | ✅ | ❌ | ✅ |
| Riesgo de descuadre de saldos | Bajo | **Alto** | Muy bajo |

> ### 🎯 Recomendación
>
> **Enfoque A, tomando el Enfoque C como su Fase 0-1.**
>
> Es decir: empezar extrayendo el núcleo puro (rápido, bajo riesgo, resuelve el problema más grave de inmediato) y, desde ahí, continuar vertical por vertical hasta completar el hexágono. Esto da un **punto de salida seguro después de cada fase** — si en algún momento se decide detener el esfuerzo, lo construido queda coherente y aporta valor, en lugar de dejar el proyecto a medio desmontar.
>
> El **Enfoque B se desaconseja explícitamente** para este proyecto: mover 4 400 líneas sin línea base de pruebas, en un sistema que maneja saldos bancarios reales y sin historial Git al que volver, concentra todo el riesgo en un único punto sin red de seguridad.

---

## 8. Mejoras de usabilidad propuestas

Todas respetan la estructura de navegación actual: no se añaden ni se eliminan pestañas.

| # | Mejora | Por qué |
|---|---|---|
| 1 | Sustituir el `prompt()` de tasa de cambio por validación en línea en el formulario | El `prompt()` nativo (fallback de v1.3.4) es modal bloqueante, no se puede corregir sin recomenzar, no muestra el equivalente calculado y desentona con el diseño macOS del resto de la app |
| 2 | Vista previa del cálculo **antes** de confirmar | Hoy el usuario no ve cuánto se debitará en DOP hasta después de guardar. Mostrar "Se debitarán RD$ 30 060.00 (RD$ 30 000.00 + RD$ 60.00 de comisión)" evita el error y la posterior reversión |
| 3 | Botón de reversión **contextual** en cada listado | La corrección de transacciones vive escondida en Ajustes (v1.3.0). Quien detecta el error lo detecta *mirando el listado*, no en Ajustes |
| 4 | Confirmación explícita con resumen del impacto antes de revertir | Una reversión mueve dinero. Debe decir "Esto devolverá RD$ 5 000.00 a Cuenta Ahorros DOP" antes de ejecutarse |
| 5 | Errores de dominio tipados con mensaje accionable | Hoy el backend devuelve `Result<_, String>` y el usuario ve mensajes técnicos. Con `ErrorDominio` tipado, "FondosInsuficientes" se traduce a "La cuenta Cuenta Ahorros DOP tiene RD$ 12 000.00; el gasto requiere RD$ 15 000.00" |
| 6 | Estados vacíos y de carga explícitos | El render es todo-o-nada: mientras carga se ve la pantalla anterior, y sin datos se ve una tabla vacía sin explicación |
| 7 | Formato monetario unificado vía `Intl.NumberFormat` | El formateo está disperso en `ui.js`; unificarlo en `nucleo/dinero.js` elimina inconsistencias visuales entre pestañas |
| 8 | Accesibilidad: `aria-label` en botones de emoji y foco atrapado en modales | Los botones ✏️ 🗑️ ▶️ no tienen texto alternativo; un modal sin foco atrapado permite tabular fuera del diálogo |
| 9 | Recordar el mes seleccionado al navegar entre pestañas | El filtro mensual (v1.3.3) se reinicia a "mes actual" cada vez, obligando a reseleccionar al comparar Gastos y Resumen |

---

## 9. Peso, responsabilidad y auditabilidad

### 9.1 Peso contenido

| Medida | Efecto |
|---|---|
| Cero dependencias nuevas en producción (Rust y JS) | El `.dmg` no crece |
| `cucumber` solo en `[dev-dependencies]` | No entra en el binario |
| Módulos ES nativos, sin bundler | Sin `node_modules` en runtime, sin paso de compilación |
| Cerrar `allowlist: all` → solo lo usado | **Reduce el binario** (Tauri excluye las APIs no declaradas) *y* la superficie de ataque |
| Se conserva `opt-level = "z"`, `lto`, `strip` | El perfil de compilación actual ya está bien ajustado; no se toca |

### 9.2 Responsable (integridad de los datos)

1. **Transacciones SQL obligatorias** — regla existente, ahora **verificada por prueba** (escenario 4 del Gherkin).
2. **Tipo `Dinero` con divisa** — hace imposible en tiempo de compilación el error que causó el bug de la v1.3.4.
3. **Migraciones versionadas con `PRAGMA user_version`** — reemplaza el `columna_existe()` + `ALTER TABLE` condicional actual, que no permite saber en qué versión de esquema está una instalación.
4. **Respaldo automático pre-migración** — se automatiza el protocolo manual de la v1.3.5.
5. **Puerto `Reloj`** — hace deterministas las pruebas de suscripciones y ciclos de corte, hoy dependientes de la fecha real del sistema.

### 9.3 Auditable

1. **`git init`** — hoy no existe historial de versiones real. `historial_versiones.md` es excelente como bitácora editorial, pero no permite ver un diff, atribuir un cambio ni volver a un estado exacto. **Es el requisito previo a todo lo demás**, y debe hacerse *después* de aplicar el `.gitignore` de §2.
2. **Un commit por fase de vertical**, con las pruebas de caracterización incluidas en el mismo commit que la extracción.
3. **Asientos de compensación en lugar de `DELETE`** — reemplazar `eliminar_gasto` / `eliminar_ingreso` / `eliminar_transaccion_cuenta` por un movimiento inverso marcado como anulación, conservando ambos registros. Es la práctica contable estándar y la única forma de responder "¿por qué cambió este saldo el 12 de agosto?".
4. **Tabla `auditoria` de solo-inserción** — registro append-only de toda mutación de saldo: qué, cuándo, monto anterior, monto posterior.
5. **`archivo_de_control.md` como bitácora de decisiones** — se mantiene el formato actual, añadiendo los hallazgos del paso 5 del protocolo de pruebas (§6.2).

---

## 10. Riesgos y cómo se mitigan

| Riesgo | Probabilidad | Mitigación |
|---|---|---|
| Un cálculo cambia de resultado durante la extracción | Media | Pruebas de caracterización (§6.2) — el paso 3 lo detecta al centavo |
| La migración de esquema corrompe datos reales | Baja | Respaldo automático pre-migración + pruebas de migración contra copia de la BD real (§6.3 #12) |
| El refactor se detiene a medias | Media | Enfoque A garantiza coherencia al final de cada fase |
| Los módulos ES no cargan en el WebView de Tauri | Baja | Verificar en la Fase 0 con un módulo de prueba antes de dividir `ui.js` |
| Cerrar el `allowlist` rompe una funcionalidad | Media | Inventariar primero qué APIs de Tauri se usan realmente; cerrar al final (Fase 8) |
| Se filtran datos financieros al iniciar Git | **Alta si no se actúa** | `.gitignore` de §2 aplicado **antes** del primer `git add` |

---

## 11. Orden de ejecución sugerido

```
  Fase 0 · Cimientos          → .gitignore, git init, dominio::dinero, Reloj, andamiaje de pruebas
  Fase 1 · Gastos             → mayor densidad de reglas; incluye excepción TSS
  Fase 2 · Tarjetas y abonos  → conversión de divisa; aquí entra la prueba Gherkin de §6.4
  Fase 3 · Cuentas            → atomicidad entre saldos
  Fase 4 · Ingresos           → retenciones y cobros
  Fase 5 · Suscripciones      → idempotencia
  Fase 6 · Capital y préstamos
  Fase 7 · Frontend           → división de ui.js + nucleo/ + mejoras de usabilidad §8
                                 + evaluación de tipado (§4.4): JS puro vs JSDoc
                                   con @ts-check vs TypeScript completo
  Fase 8 · Auditoría y cierre → asientos de compensación, migraciones versionadas, allowlist
```

Cada fase termina con: pruebas en verde, aplicación compilando, `.dmg` generable, commit único y entrada en `archivo_de_control.md`.

---

## 12. Estado de este documento

**Nada de lo aquí descrito ha sido implementado.** No se ha modificado ningún archivo de código, configuración ni base de datos del proyecto.

Las únicas acciones ejecutadas fueron:

1. La copia de respaldo verificada en `~/Desktop/MiChelitosTauri_backup_2026-09-08/` (§1).
2. La creación de este archivo de planificación.

El `.gitignore` **no ha sido modificado**: las reglas de §2 son una propuesta pendiente de aprobación.
