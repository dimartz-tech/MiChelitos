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
~/Desktop/MiChelitosTauri_backup_2026-09-08/
├── proyecto/            40 archivos (= 40 en origen)
└── base_de_datos_viva/  SHA-1 e1bdfeb126ac48c8e190842c9e7a8edd43618389 (idéntico)
```

**Por qué primero:** todo lo que sigue toca el directorio de trabajo o publica contenido. Sin un punto de retorno verificado, cualquier error es irreversible. Se excluyeron `src-tauri/target/` (3.2 GB) y `node_modules/` por reproducibles.

---

### ✅ 0.2 — Reglas de exclusión *(completado)*

`.gitignore` ampliado con las reglas de `plan_arquitectura_hexagonal.md` §2.

**Por qué antes del primer `git add`:** el `.gitignore` heredado del commit v1.2.0 no excluía ningún `*.db`. Los archivos `michelitos.db` y `michelitos.db.backup_2026-08-17_162833` están en la raíz del proyecto y contienen saldos, RNC de clientes y facturas reales. Un dato committeado sobrevive al `git rm`: permanece en el historial y en todo clon existente. La regla tenía que existir **antes**, no después.

**Verificación ejecutada:**

```
git status --porcelain --ignored | grep '^!!'
  !! michelitos.db
  !! michelitos.db.backup_2026-08-17_162833
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

### 🔄 0.5 — Commit evolutivo *(en curso)*

Un único commit sobre `a097a14` que documenta el salto v1.2.0 → v1.3.5 y deja constancia de las seis versiones intermedias.

**Por qué un solo commit y no seis:** reconstruir un commit por versión exigiría el estado del código en cada fecha, y ese estado no existe — nunca se versionó. Fabricar commits retroactivos con el contenido de hoy sería un historial **falso**, justo lo contrario de auditable. La solución honesta es un commit que declara el rango cubierto y remite al changelog para el desglose.

---

### ⬜ 0.6 — Núcleo mínimo del dominio *(pendiente)*

Primer código de la arquitectura nueva. Alcance deliberadamente estrecho:

```
src-tauri/src/dominio/
├── mod.rs
├── dinero.rs      # Divisa, Monto, TasaCambio
└── errores.rs     # ErrorDominio

src-tauri/src/puertos/
└── reloj.rs       # trait Reloj
```

**Por qué `Dinero` primero:** es el tipo del que dependen todas las demás reglas (comisiones, retenciones, disponibilidad, conversión). Extraerlo después obligaría a reescribir lo construido encima. Además, hacer que la divisa forme parte del tipo convierte en **error de compilación** la suma DOP + USD, que es la causa raíz del descuadre corregido a mano en la v1.3.4.

**Por qué `Reloj` como puerto:** hoy las reglas de suscripciones y ciclos de corte leen la fecha del sistema directamente, lo que hace que su comportamiento dependa del día en que se ejecute la prueba. Un `trait Reloj` inyectable permite fijar la fecha en los tests y probar el 29 de febrero, el cambio de año o el día 31 en un mes de 30.

---

### ⬜ 0.7 — Andamiaje de pruebas *(pendiente)*

* Rust: `cargo test` nativo, sin dependencias.
* JS: `node:test` + `node:assert`, sin dependencias.
* Prueba humo en cada lado que falle a propósito y luego pase, para confirmar que el ejecutor corre de verdad.

**Por qué una prueba que falla primero:** un ejecutor mal configurado que no encuentra ningún test reporta éxito. Verificar que sabe fallar es la única forma de confiar en el verde posterior.

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
- [ ] Commit evolutivo publicado en `origin/main`
- [ ] `dominio::dinero` compila y tiene pruebas en verde
- [ ] Ejecutores de prueba de Rust y JS demostrados funcionales
- [ ] Módulos ES confirmados en el WebView

**Ninguna regla de negocio se ha movido todavía.** La Fase 0 solo construye el terreno; la primera extracción real ocurre en la Fase 1 (Gastos).

---

## 3. Riesgos vivos de esta fase

| Riesgo | Estado | Mitigación |
|---|---|---|
| Publicar datos financieros en repo público | **Resuelto** | `.gitignore` + anonimización documental + doble verificación |
| Sobrescribir el trabajo local v1.3.5 con el remoto v1.2.0 | **Resuelto** | `git reset` mixed en lugar de `checkout`/`pull`; respaldo previo |
| Sin credenciales de push | **Abierto** | Ver §4 |
| Los módulos ES no cargan en el WebView | Abierto | Paso 0.8 lo verifica antes de comprometer el diseño del frontend |

---

## 4. Bloqueo activo: credenciales de publicación

El commit puede crearse en local, pero **el push no**. Diagnóstico:

```
gh CLI               no instalado
GITHUB_TOKEN         ausente
~/.ssh/wilfodmba     la clave existe...
ssh -T git@github.com  → Permission denied (publickey)
```

La clave existe en el equipo pero **no está registrada en la cuenta de GitHub**, y no hay `~/.ssh/config` que la asocie al host. Además el agente SSH no tiene identidades cargadas.

**Opciones para desbloquear**, de menor a mayor esfuerzo:

1. **Registrar la clave existente** — copiar el contenido de `~/.ssh/wilfodmba.pub` en GitHub → *Settings → SSH and GPG keys → New SSH key*, y cambiar el remoto a SSH:
   ```bash
   git remote set-url origin git@github.com:dimartz-tech/MiChelitos.git
   ```
2. **Token de acceso personal** — crear un PAT con permiso `repo` en *Settings → Developer settings → Personal access tokens*, y usarlo como contraseña en el primer push HTTPS (macOS lo guarda en el llavero).
3. **Instalar `gh`** — `brew install gh && gh auth login`, que resuelve credenciales y remoto de una vez.

La identidad de git ya está configurada correctamente y coincide con el propietario del repositorio (`dimartz-tech`), así que la autoría del commit será correcta sin ajustes adicionales.
