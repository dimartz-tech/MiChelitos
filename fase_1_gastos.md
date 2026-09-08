# 💸 Fase 1 — Vertical de Gastos

**Documento de detalle del plan** `plan_arquitectura_hexagonal.md` §11
**Fecha:** 2026-09-08
**Precondición:** Fase 0 cerrada salvo el paso 0.8 (no bloqueante para el backend)
**Estado:** PLANIFICACIÓN — no se ha escrito código de esta fase

---

## 1. Por qué Gastos va primero

De los siete verticales, es el que **concentra más reglas de negocio por línea de código**:

* Retención impositiva del 0.20 % sobre transferencias
* Exención tributaria del TSS
* Comisión de servicio del LBTR: +100.00 DOP
* Tres métodos de pago con efectos distintos sobre los saldos
* Reversión con restauración de balance

Extraerlo primero maximiza el retorno de la infraestructura construida en la Fase 0. Además, `crear_gasto` es la función que la §3.2 del plan usa como ejemplo del problema: si la refactorización funciona aquí, funciona en el resto.

---

## 2. El código actual, leído en detalle

Fuente: `src-tauri/src/main.rs:271-354` (`crear_gasto`) y `1326-1369` (`eliminar_gasto`).

### 2.1 Reglas identificadas

| # | Regla | Naturaleza | Condición |
|---|---|---|---|
| R1 | Retención impositiva 0.20 % redondeada a unidades enteras | **impuesto** | solo si `metodo_pago == "transferencia"` |
| R2 | Exención del TSS: retención 0.00 | **exención impositiva** | categoría en minúsculas `== "impuestos"` **y** descripción en mayúsculas contiene `"TSS"` |
| R3 | Cargo LBTR +100.00 | **comisión de servicio** | si `es_lbtr`, se suma **después** de R1/R2 |
| R4 | Pago con tarjeta incrementa la deuda | `balance_pesos` o `balance_dolares` según divisa |
| R5 | Transferencia debita `monto + retención + comisión` | de la cuenta de ahorro indicada |
| R6 | Efectivo debita `monto` | de la cuenta llamada `"Efectivo DOP"` o `"Efectivo USD"` |
| R7 | Todo ocurre dentro de una transacción SQL | `conn.transaction()` … `commit()` |

### 2.2 Hallazgos que las pruebas de caracterización deben fijar

Estos puntos se detectaron leyendo el código. **Ninguno se corrige en esta fase**: se documentan, se cubren con pruebas que capturan el comportamiento actual, y se decide después. Es el paso 5 del protocolo (`plan_arquitectura_hexagonal.md` §6.2).

El primero se consultó con el usuario y quedó **confirmado como correcto**; los cuatro restantes siguen abiertos.

**H1 — RESUELTO: el comportamiento actual es correcto.**
`main.rs:287-293`. Se planteó si era un descuido que el `+= 100.00` del LBTR se aplicara también a los pagos exentos del 0.20 %. **No lo es**, y la razón está en que R1 y R3 son cosas distintas:

* El **0.20 % es una retención impositiva**. El TSS goza de una exención tributaria, de modo que las transferencias destinadas a él no están gravadas.
* Los **100.00 DOP del LBTR son una comisión de servicio** del banco por usar el carril de liquidación en tiempo real. Es opcional porque no toda transferencia se cursa por esa vía.

Una exención de impuestos no exime de pagar un servicio. Si un pago de TSS se cursa por LBTR, corresponde que no tribute el 0.20 % **y** que sí pague los 100.00 de la operación. El orden del código —exención primero, comisión después— refleja exactamente eso.

**Consecuencia para el modelo de dominio:** el campo `costo_adicional` fusiona hoy en un solo número un impuesto y una comisión, que responden a reglas, exenciones y tratamientos contables distintos. Al extraer el dominio se separarán como conceptos —conservando la suma al persistir, para no alterar el esquema ni el comportamiento— de modo que la exención se aplique sobre el concepto que corresponde en lugar de sobre un total indiferenciado. Como efecto secundario, pasa a ser posible responder cuánto se pagó en impuestos de transferencia frente a cuánto en comisiones bancarias, hoy indistinguible.

**H2 — La divisa del gasto y la de la cuenta no se comparan.**
`main.rs:316-323`. Un gasto en USD pagado por transferencia debita `monto + costo_adicional` de la cuenta indicada **sin verificar que esa cuenta sea en USD**. Si es una cuenta DOP, se le restan unidades de dólar de un saldo en pesos. Es exactamente la clase de error que el tipo `Dinero` de la Fase 0 existe para impedir.

**H3 — El gasto en efectivo busca la cuenta por nombre literal.**
`main.rs:327-331`. `UPDATE … WHERE nombre = 'Efectivo DOP'`. Si la fila no existe o fue renombrada, el `UPDATE` afecta a 0 filas y **devuelve `Ok`**: el gasto queda registrado y ningún saldo se mueve, sin aviso.

**H4 — El pago con tarjeta sin `tarjeta_id` se registra igual.**
`main.rs:299-313`. Si `metodo_pago == "tarjeta"` pero `tarjeta_id` es `None`, el `if let` no entra, el gasto se inserta y ninguna deuda se incrementa.

**H5 — La reversión de tarjeta recorta en cero; la de cuenta no.**
`main.rs:1340` usa `MAX(0.0, balance_dolares - ?)`, mientras que la reversión de cuenta (`1353`) y la de efectivo (`1360`) suman sin límite. Consecuencia: **crear y luego eliminar un gasto de tarjeta no siempre es una operación inversa exacta.** Si la deuda de la tarjeta es 100 y se revierte un gasto de 150, el saldo queda en 0 en vez de −50, y esos 50 desaparecen sin registro.

> **H5 es el hallazgo más serio** y refuerza la propuesta de §9.3 del plan: sustituir el borrado por **asientos de compensación**. Un `DELETE` que además recorta saldos no deja forma de auditar qué se perdió.

---

## 3. Estructura objetivo

```
src-tauri/src/
├── dominio/
│   ├── dinero.rs            ← ya existe (Fase 0)
│   ├── errores.rs           ← ya existe (Fase 0)
│   ├── cargos.rs            ← NUEVO: RetencionTransferencia (R1, R2)
│   │                                 ComisionServicio (R3)
│   └── gasto.rs             ← NUEVO: MetodoPago y su efecto sobre saldos
│
├── puertos/
│   ├── reloj.rs             ← ya existe (Fase 0)
│   └── repositorios.rs      ← NUEVO: RepositorioGastos, RepositorioCuentas,
│                                     RepositorioTarjetas, RepositorioCategorias
│
├── aplicacion/
│   ├── registrar_gasto.rs   ← NUEVO: orquesta R4, R5, R6, R7
│   └── revertir_gasto.rs    ← NUEVO
│
└── adaptadores/
    ├── sqlite/gastos.rs     ← NUEVO: implementa los puertos con rusqlite
    └── tauri/gastos.rs      ← NUEVO: los comandos, reducidos a traducción
```

`crear_gasto` y `eliminar_gasto` quedan en `adaptadores/tauri/gastos.rs` como envoltorios delgados: deserializan, llaman al caso de uso y convierten `ErrorDominio` en `String` mediante el `From` construido en la Fase 0. **Su firma pública no cambia**, de modo que `api.js` y `ui.js` no se tocan en esta fase.

### 3.1 Diseño de `MetodoPago` (principio abierto/cerrado)

Hoy el método de pago es un `String` comparado con `==` en cuatro sitios distintos de dos funciones. Añadir un método obliga a encontrar y editar todos. Como enum de dominio con la lógica de afectación asociada, añadir uno nuevo no toca los existentes:

```rust
pub enum MetodoPago {
    Efectivo,
    Tarjeta { tarjeta_id: i64 },
    Transferencia { cuenta_id: i64, es_lbtr: bool },
}
```

Obsérvese que el identificador va **dentro** de la variante. Eso hace que H4 —tarjeta sin `tarjeta_id`— sea irrepresentable: el tipo no permite construir un pago con tarjeta sin decir cuál.

---

## 4. Protocolo de pruebas antes/después

### 4.1 Pruebas de caracterización (ANTES)

Se escriben **contra el código actual sin modificarlo** y deben pasar en verde antes de extraer nada. Requieren una base SQLite en memoria (`rusqlite` la soporta de fábrica) sembrada con el esquema real.

| # | Caso | Resultado esperado (comportamiento HOY) |
|---|---|---|
| C1 | Transferencia de 10 000 DOP, categoría "Alimentación" | `costo_adicional == 20.0` |
| C2 | Transferencia de 10 000, categoría "Impuestos", desc. "Pago TSS" | `costo_adicional == 0.0` |
| C3 | Igual que C2 pero categoría "Alimentación" | `costo_adicional == 20.0` |
| C4 | Igual que C2 pero descripción "Pago ITBIS" | `costo_adicional == 20.0` |
| C5 | Transferencia LBTR de 10 000, categoría normal | `costo_adicional == 120.0` |
| C6 | **Transferencia LBTR de TSS** | `costo_adicional == 100.0` — exento del impuesto, sí paga la comisión (**H1**) |
| C7 | Redondeo: transferencia de 1 250 | `(1250*0.002).round() == 3.0` (no 2.5) |
| C8 | Gasto con tarjeta en USD | `balance_dolares` sube; `balance_pesos` intacto |
| C9 | Gasto en efectivo DOP | `"Efectivo DOP"` baja el monto exacto |
| C10 | **Gasto en efectivo con la cuenta renombrada** | devuelve `Ok`, ningún saldo cambia → fija **H3** |
| C11 | **Gasto con tarjeta y `tarjeta_id: None`** | se inserta, ninguna deuda cambia → fija **H4** |
| C12 | **Crear y eliminar gasto de tarjeta de 150 con deuda previa 100** | balance final `0.0`, no `-50.0` → fija **H5** |
| C13 | Crear y eliminar gasto por transferencia | saldo de la cuenta vuelve al valor exacto inicial |
| C14 | Fallo a mitad de la transacción | ningún saldo alterado, ningún gasto insertado |

**Por qué C6, C10, C11 y C12 son los más valiosos:** son los que fijan comportamientos que a primera vista parecen defectos. Sin ellos, la refactorización podría "arreglarlos" sin que nadie se entere, y los saldos históricos ya grabados dejarían de cuadrar con la lógica nueva. C6 es el ejemplo perfecto: parecía un descuido y resultó ser la regla correcta.

### 4.2 Pruebas unitarias aisladas (DESPUÉS)

Ya con el dominio extraído, sin base de datos y en microsegundos:

* `dominio::cargos` — R1, R2, R3 y sus bordes: monto 0, montos con decimales, `"tss"` en minúsculas, `"TSS"` dentro de una palabra mayor, categoría `"Impuestos"` con distinta capitalización.
* `dominio::gasto::MetodoPago` — qué saldo afecta cada variante y en qué signo.
* `aplicacion::RegistrarGasto` — con repositorios **dobles en memoria**: fondos insuficientes, divisa de cuenta incompatible (H2), repositorio que falla a mitad (atomicidad).
* `adaptadores::sqlite::gastos` — pruebas de contrato ejecutadas **sobre la implementación SQLite y sobre el doble en memoria**, verificando que son intercambiables (sustitución de Liskov).

### 4.3 Criterio de aceptación

Las 14 pruebas de caracterización pasan **idénticas** antes y después de la extracción. Cualquier divergencia detiene la fase y se registra en `archivo_de_control.md` para decisión explícita.

---

## 5. Prueba Gherkin del vertical

**Archivo:** `features/registro_de_gastos.feature`

```gherkin
# language: es

Característica: Registro de gastos con retenciones y comisiones bancarias
  Como responsable de las finanzas
  Quiero que cada gasto descuente el saldo correcto y aplique la comisión que corresponda
  Para que los balances reflejen la realidad sin tener que cuadrarlos a mano

  Antecedentes:
    Dado que existe la cuenta de ahorros "Cuenta Ahorros DOP" en "DOP" con balance 100000.00
    Y que existe la categoría "Alimentación"
    Y que existe la categoría "Impuestos"

  Escenario: Una transferencia ordinaria aplica la comisión del 0.20 %
    Cuando registro un gasto por transferencia de 10000.00 "DOP"
      En la categoría "Alimentación" con descripción "Compra semanal"
      Desde la cuenta "Cuenta Ahorros DOP"
    Entonces la retención impositiva debe ser 20.00 "DOP"
    Y el balance de la cuenta "Cuenta Ahorros DOP" debe ser 89980.00

  Escenario: El pago del TSS está exento de la comisión porcentual
    Cuando registro un gasto por transferencia de 10000.00 "DOP"
      En la categoría "Impuestos" con descripción "Pago TSS agosto"
      Desde la cuenta "Cuenta Ahorros DOP"
    Entonces la retención impositiva debe ser 0.00 "DOP"
    Y el balance de la cuenta "Cuenta Ahorros DOP" debe ser 90000.00

  Escenario: La exención exige que se cumplan las dos condiciones
    Cuando registro un gasto por transferencia de 10000.00 "DOP"
      En la categoría "Alimentación" con descripción "Pago TSS agosto"
      Desde la cuenta "Cuenta Ahorros DOP"
    Entonces la retención impositiva debe ser 20.00 "DOP"

  Escenario: Una transferencia LBTR añade el cargo fijo del servicio
    Cuando registro un gasto por transferencia LBTR de 10000.00 "DOP"
      En la categoría "Alimentación" con descripción "Pago proveedor"
      Desde la cuenta "Cuenta Ahorros DOP"
    Entonces la retención impositiva debe ser 20.00 "DOP"
    Y la comisión de servicio debe ser 100.00 "DOP"
    Y el cargo total registrado debe ser 120.00 "DOP"

  Escenario: La exención del TSS no alcanza a la comisión del LBTR
    # Una exención tributaria libera del impuesto, no del precio de un servicio.
    Cuando registro un gasto por transferencia LBTR de 10000.00 "DOP"
      En la categoría "Impuestos" con descripción "Pago TSS agosto"
      Desde la cuenta "Cuenta Ahorros DOP"
    Entonces la retención impositiva debe ser 0.00 "DOP"
    Y la comisión de servicio debe ser 100.00 "DOP"
    Y el cargo total registrado debe ser 100.00 "DOP"
    Y el balance de la cuenta "Cuenta Ahorros DOP" debe ser 89900.00

  Escenario: Atomicidad — un fallo no deja el saldo movido
    Dado que el repositorio de gastos fallará al guardar
    Cuando registro un gasto por transferencia de 10000.00 "DOP"
      En la categoría "Alimentación" con descripción "Compra semanal"
      Desde la cuenta "Cuenta Ahorros DOP"
    Entonces la operación debe fallar
    Y el balance de la cuenta "Cuenta Ahorros DOP" debe ser 100000.00
    Y no debe existir ningún gasto registrado
```

> Los dos últimos escenarios distinguen explícitamente **retención impositiva** de **comisión de servicio**, aunque la base de datos siga guardando su suma en `costo_adicional`. Esa distinción es la que hace evidente, al leer la prueba, por qué la exención se aplica a un concepto y no al otro.

---

## 6. Orden de ejecución

```
  1.1  Pruebas de caracterización C1–C14 contra el código actual        → verde
  1.2  Extraer dominio::cargos: RetencionTransferencia + ComisionServicio → C1–C7 verdes
  1.3  Extraer dominio::gasto::MetodoPago (R4, R5, R6)                   → C8–C13 siguen verdes
  1.4  Definir puertos::repositorios y sus dobles en memoria
  1.5  Extraer aplicacion::RegistrarGasto y RevertirGasto (R7)
  1.6  Implementar adaptadores::sqlite::gastos
  1.7  Reducir crear_gasto/eliminar_gasto a envoltorios en adaptadores::tauri
  1.8  Pruebas unitarias aisladas + Gherkin del vertical
  1.9  Registrar H2–H5 en archivo_de_control.md como decisiones pendientes
       (H1 ya resuelto: comportamiento confirmado correcto)
```

Cada paso deja la aplicación compilando y con la suite en verde. El paso 1.7 es el único que toca código en producción de forma visible, y para entonces ya está cubierto por las catorce pruebas del paso 1.1.

---

## 7. Criterios de salida

- [ ] C1–C14 en verde contra el código original
- [ ] C1–C14 en verde contra el dominio extraído, con resultados idénticos al centavo
- [ ] `crear_gasto` y `eliminar_gasto` reducidos a traducción, sin reglas ni SQL
- [ ] Firmas de los comandos sin cambios: `api.js` y `ui.js` intactos
- [ ] Pruebas de contrato pasando sobre SQLite y sobre el doble en memoria
- [ ] Gherkin del vertical en verde
- [ ] H2–H5 documentados en `archivo_de_control.md` con su decisión o su aplazamiento explícito
- [ ] Retención impositiva y comisión de servicio modeladas como conceptos separados, persistiendo su suma
- [ ] `cargo build` sin advertencias nuevas

---

## 8. Riesgos

| Riesgo | Probabilidad | Mitigación |
|---|---|---|
| Corregir H2–H5 sin querer durante la extracción | **Alta** | Las pruebas C6, C10, C11 y C12 fijan el comportamiento actual y fallarían |
| Fusionar de nuevo impuesto y comisión al persistir | Media | C5 y C6 verifican el total; las pruebas unitarias verifican cada concepto por separado |
| El redondeo cambia al mover la fórmula | Media | C7 lo fija explícitamente; se conserva `f64` y `.round()` tal cual |
| La reversión deja saldos distintos a los originales | Media | C12 y C13 comparan el saldo antes y después del ciclo completo |
| El alcance crece hacia Tarjetas o Cuentas | Media | Los puertos de esas entidades se definen aquí pero se implementan mínimos; su vertical es la Fase 2 y 3 |
| Cambiar la firma de los comandos rompe el frontend | Baja | Criterio de salida explícito: `api.js` y `ui.js` no se tocan en esta fase |

---

## 9. Lo que esta fase NO hace

* No corrige H2–H5. Los documenta y los cubre con pruebas. H1 quedó resuelto como comportamiento correcto.
* No altera el esquema de la base de datos: `costo_adicional` sigue guardando la suma de retención y comisión.
* No sustituye el `DELETE` por asientos de compensación — eso es la Fase 8, y depende de decidir H5.
* No toca el frontend. `ui.js` sigue con sus 2 874 líneas hasta la Fase 7.
* No migra a enteros de centavos. Sigue pendiente de las pruebas de caracterización de cargos, que esta fase produce.
