# 🧱 Fase 0 — Cimientos

**Documento de detalle del plan** `plan_arquitectura_hexagonal.md` §11
**Fecha:** 2026-09-08
**Enfoque adoptado:** A (estrangulamiento por vertical), con C como fase inicial

---

## 0. Por qué esta fase cambió respecto al plan original

El plan §9.3 asumía que había que ejecutar `git init` desde cero porque el directorio local no tenía historial. La consulta al remoto reveló otra situación:

| | Remoto `dimartz-tech/MiChelitos` | Local |
|---|---|---|
| Versión | **1.2.0** | **1.3.5** |
| Commits | 1 (`a097a14`, 2026-07-13) | ninguno (sin `.git`) |
| `ui.js` | 98,952 bytes | 182,741 bytes |
| `main.rs` | 26,846 bytes | 52,685 bytes |
| Documentación | ausente | `historial_versiones.md`, `archivo_de_control.md` |

Existe un **commit base legítimo** en el remoto. Eso cambia la estrategia: en lugar de fundar una historia nueva que empiece en el presente —borrando implícitamente que el proyecto viene de algún sitio—, se reconstruye la continuidad **sobre** `a097a14`.

**Por qué importa:** el criterio de auditabilidad exige poder responder "¿qué cambió y desde qué punto?". Un `git init` local habría producido un primer commit de 4 378 líneas sin antecedente, indistinguible de código escrito hoy. Enlazar con el remoto convierte esas mismas líneas en un **diff de evolución v1.2.0 → v1.3.5**, que es información real y revisable.

---

## 1. Pasos de la fase

### ✅ 0.1 — Respaldo verificado *(completado)*

```
<DIRECTORIO_DE_RESPALDO>/MiChelitosTauri_backup_<FECHA>/
├── proyecto/            recuento de archivos coincidente con el origen
└── base_de_datos_viva/  suma de comprobación idéntica al origen
```

**Por qué primero:** todo lo que sigue toca el directorio de trabajo o publica contenido. Sin un punto de retorno verificado, cualquier error es irreversible. Se excluyeron `src-tauri/target/` (3.2 GB) y `node_modules/` por reproducibles.

---

### ✅ 0.2 — Reglas de exclusión *(completado)*

`.gitignore` ampliado con las reglas de `plan_arquitectura_hexagonal.md` §2.

**Por qué antes del primer `git add`:** el `.gitignore` heredado del commit v1.2.0 no excluía ningún `*.db`. El archivo `michelitos.db` y sus respaldos están en la raíz del proyecto y contienen saldos, RNC de clientes y facturas reales. Un dato committeado sobrevive al `git rm`: permanece en el historial y en todo clon existente. La regla tenía que existir **antes**, no después.

**Verificación ejecutada:**

```
git status --porcelain --ignored | grep '^!!'
  !! michelitos.db
  !! michelitos.db.backup_<marca de tiempo>
  !! node_modules/ · src-tauri/target/ · .DS_Store

git status --porcelain -uall | grep -iE "\.db|backup"
  (vacío) → ninguna base de datos es visible para git
```

---

### ✅ 0.3 — Enlace del árbol local con el commit base *(completado)*

```bash
git init
git remote add origin https://github.com/dimartz-tech/MiChelitos.git
git fetch origin main
git reset origin/main          # MIXED: mueve HEAD e índice, NO toca el árbol de trabajo
```

**Por qué `reset` y no `checkout` / `pull`:** un `git checkout main` o un `git pull` habrían intentado escribir los archivos v1.2.0 del remoto **encima** de los archivos v1.3.5 locales, destruyendo dos meses de trabajo no versionado. El `reset` en modo *mixed* (el predeterminado) reposiciona `HEAD` y el índice sobre `a097a14` pero **deja el árbol de trabajo intacto**. El resultado es que git ve los archivos locales como *modificaciones* sobre v1.2.0 — que es exactamente lo que son.

**Verificación:** `ui.js` conservó sus 2 874 líneas y `main.rs` sus 1 504 tras el reset.

---

### ✅ 0.4 — Anonimización documental *(completado)*

Antes de publicar en un repositorio **público**, se revisaron los tres `.md` en busca de datos financieros personales.

| Archivo | Contenido retirado | Sustituido por |
|---|---|---|
| `historial_versiones.md` | Emisor, producto, límites bimoneda concretos y días de corte/pago reales (v1.3.5) | Descripción de la *capacidad*: alta de tarjetas con límites independientes por divisa y ciclo parametrizable |
| `archivo_de_control.md` | Ficha completa de configuración de una tarjeta real | Tabla de *parámetros soportados* por el sistema + nota de privacidad |
| `plan_arquitectura_hexagonal.md` | Nombres reales de banco, tarjeta y cuentas en la prueba Gherkin y en los ejemplos de UX | `Banco Ejemplo`, `Tarjeta Ejemplo`, `Cuenta Ahorros DOP/USD` |

**Por qué:** el criterio "no cargues datos de bases de datos" protege un objetivo —que las finanzas del usuario no se publiquen—, no un formato de archivo. Un límite de crédito real escrito en un changelog Markdown es tan sensible como el mismo dato dentro del `.db`, y en un repositorio público queda indexable de forma permanente.

**Regla permanente derivada:** la documentación describe **capacidades del sistema**; los valores concretos son datos de usuario y viven solo en `~/.michelitos/`. Toda entrada futura del changelog debe redactarse en términos de qué se puede hacer, no de qué se registró.

**Verificación:** barrido de nombres de entidades financieras, importes con separador de miles e identificadores largos sobre todos los `.md` → sin residuos.

---

### ✅ 0.5 — Commit evolutivo *(completado)*

Publicado como `991dcdd` sobre `a097a14` en `origin/main`. Auditoría posterior contra el remoto: 37 archivos, ninguna base de datos, cero coincidencias de datos financieros reales en los `.md`.


Un único commit sobre `a097a14` que documenta el salto v1.2.0 → v1.3.5 y deja constancia de las seis versiones intermedias.

**Por qué un solo commit y no seis:** reconstruir un commit por versión exigiría el estado del código en cada fecha, y ese estado no existe — nunca se versionó. Fabricar commits retroactivos con el contenido de hoy sería un historial **falso**, justo lo contrario de auditable. La solución honesta es un commit que declara el rango cubierto y remite al changelog para el desglose.

---

### ✅ 0.6 — Núcleo mínimo del dominio *(completado)*

Primer código de la arquitectura nueva, en la rama `refactor/fase-0-cimientos`. Alcance deliberadamente estrecho:

```
src-tauri/src/dominio/
├── mod.rs
├── dinero.rs              # Divisa, Dinero, TasaCambio
└── errores.rs             # ErrorDominio

src-tauri/src/puertos/
├── mod.rs
└── reloj.rs               # trait Reloj + RelojFijo (doble de prueba)

src-tauri/src/adaptadores/
├── mod.rs
└── reloj_sistema.rs       # RelojSistema (adaptador real)

src/js/nucleo/
└── dinero.js              # espejo del dominio en el frontend
```

**Decisiones tomadas y su motivo:**

* **`Dinero` mantiene `f64`, no enteros de centavos.** Migrar a centavos cambiaría el redondeo respecto a lo que hoy está grabado en la base de datos. Según el paso 5 del protocolo de pruebas, una corrección de fórmula debe ser una decisión consciente y versionada, no un efecto colateral de la refactorización. Queda anotado como hallazgo para evaluar en la Fase 1, cuando existan las pruebas de caracterización de comisiones.
* **`TasaCambio` se expresa en DOP por 1 USD**, que es la convención con la que el usuario la introduce en la interfaz. Rechaza cero y negativos con `TasaDeCambioRequerida`, exactamente el error del segundo escenario Gherkin.
* **`impl From<ErrorDominio> for String`.** Los 39 comandos Tauri devuelven `Result<_, String>`. Esta conversión les permite adoptar errores de dominio en la Fase 1 **sin cambiar sus firmas**, que es lo que hace viable el estrangulamiento incremental.
* **`RelojFijo` va tras `#[cfg(test)]`**, de modo que el doble de prueba no existe en el binario de producción.
* **El frontend replica la regla, no la comparte.** `src/js/nucleo/dinero.js` no puede reutilizar el tipo de Rust, así que duplica la invariante con su propia batería de pruebas. La duplicación es deliberada: la alternativa sería generar tipos, y eso exigiría un paso de compilación que el criterio de peso contenido descarta.

**Por qué `Dinero` primero:** es el tipo del que dependen todas las demás reglas (comisiones, retenciones, disponibilidad, conversión). Extraerlo después obligaría a reescribir lo construido encima. Además, hacer que la divisa forme parte del tipo convierte en **error de compilación** la suma DOP + USD, que es la causa raíz del descuadre corregido a mano en la v1.3.4.

**Por qué `Reloj` como puerto:** hoy las reglas de suscripciones y ciclos de corte leen la fecha del sistema directamente, lo que hace que su comportamiento dependa del día en que se ejecute la prueba. Un `trait Reloj` inyectable permite fijar la fecha en los tests y probar el 29 de febrero, el cambio de año o el día 31 en un mes de 30.

---

### ✅ 0.7 — Andamiaje de pruebas *(completado)*

| Lado | Ejecutor | Dependencias añadidas | Pruebas |
|---|---|---:|---:|
| Rust | `cargo test` nativo | 0 | 22 |
| JS | `node:test` + `node:assert` nativos | 0 | 12 |
| | | | **34** |

```bash
npm test          # solo frontend
npm run test:rust # solo backend
npm run test:todo # ambos
```

`package.json` incorpora `"type": "module"` para que Node interprete los módulos ES del núcleo igual que el navegador. No afecta al CLI de Tauri, que resuelve su propio paquete.

**Por qué una prueba que falla primero:** un ejecutor mal configurado que no encuentra ningún test reporta éxito. Verificar que sabe fallar es la única forma de confiar en el verde posterior.

**Se verificó de forma no planificada.** La primera ejecución de `cargo test` falló con 20 pasando y 1 fallando: la prueba del reloj situaba una fecha en el 29 de febrero de **2026**, que no existe porque 2026 no es bisiesto. El error estaba en la prueba, no en el código, pero sirvió como demostración de que el ejecutor detecta fallos reales. Se corrigió a 2028 y se añadió una prueba que fija esa invariante (`una_fecha_inexistente_no_se_construye_en_silencio`). En el lado JS ocurrió lo mismo: `Object.create(null)` hacía fallar `deepEqual` frente a un objeto literal pese a tener los mismos valores; se simplificó a `{}` porque las claves son un conjunto fijo ya validado.

---

### ⬜ 0.8 — Verificación de módulos ES en el WebView *(pendiente)*

Cargar un módulo trivial vía `<script type="module">` y confirmar que el WebView de Tauri lo resuelve.

**Por qué antes de dividir `ui.js`:** toda la estrategia del frontend (§4.3 del plan) depende de que los módulos ES nativos funcionen sin bundler. Si no funcionaran, hay que saberlo **antes** de partir 2 874 líneas en once archivos, no después.

---

## 2. Criterios de salida de la Fase 0

La fase se da por cerrada cuando:

- [x] Respaldo verificado por suma de comprobación
- [x] Ninguna base de datos es visible para git
- [x] Árbol local enlazado al commit base del remoto, sin pérdida de trabajo
- [x] Documentación libre de datos financieros personales
- [x] Commit evolutivo publicado en `origin/main` (`991dcdd`)
- [x] `dominio::dinero` compila y tiene pruebas en verde (22 Rust + 12 JS)
- [x] Ejecutores de prueba de Rust y JS demostrados funcionales
- [ ] Módulos ES confirmados en el WebView de Tauri

**Ninguna regla de negocio se ha movido todavía.** La Fase 0 solo construye el terreno; la primera extracción real ocurre en la Fase 1 (Gastos).

---

## 3. Riesgos vivos de esta fase

| Riesgo | Estado | Mitigación |
|---|---|---|
| Publicar datos financieros en repo público | **Resuelto** | `.gitignore` + anonimización documental + doble verificación |
| Sobrescribir el trabajo local v1.3.5 con el remoto v1.2.0 | **Resuelto** | `git reset` mixed en lugar de `checkout`/`pull`; respaldo previo |
| Sin credenciales de push | **Resuelto** | Clave registrada en GitHub y `~/.ssh/config` con `UseKeychain` |
| Los módulos ES no cargan en el WebView | **Abierto** | Paso 0.8, aún sin verificar; no bloquea porque el núcleo todavía no se importa desde `index.html` |

---

## 4. Publicación — resuelto

La autenticación SSH con el remoto quedó verificada y funcionando: `git` publica sin pedir credenciales ni requerir variables de entorno.

Por higiene, este documento no detalla qué clave se usa, dónde reside ni cómo está configurada: son datos operativos del equipo local que no aportan nada al repositorio y sí describen la superficie de acceso.

---

## 5. Siguiente paso

**0.8 — Verificación de módulos ES en el WebView**, único punto abierto de la fase. Requiere levantar la aplicación (`npm run tauri dev`) y confirmar que el WebView resuelve un `<script type="module">` servido por el protocolo de activos de Tauri.

No se ha dado por verificado sin ejecutarlo. El núcleo creado en 0.6 todavía no se importa desde `index.html`, así que la aplicación actual no depende de ello y el riesgo permanece acotado hasta la Fase 7.
