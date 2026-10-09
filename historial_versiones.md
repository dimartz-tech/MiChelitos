# 📜 Historial de Versiones - MiChelitosTauri

Este archivo detalla la evolución de la aplicación de escritorio nativa macOS **MiChelitosTauri**, incluyendo características integradas, correcciones y cambios en la arquitectura de base de datos.

---

## 🚀 Versión 1.92.0 (Versión Actual) - 2026-10-09
**Corrección de abonos a tarjeta: sin cuenta elegida, el abono se paga de la caja de efectivo.** Antes, un abono registrado sin elegir cuenta solo bajaba la deuda de la tarjeta y **no movía ningún saldo**: el dinero seguía figurando en la caja y los saldos se descuadraban. Es un cambio de regla pedido por el titular.

### 🔧 Qué se hace
* **Regla nueva:** si no se elige cuenta, el abono sale de la caja de efectivo de **su divisa** («Efectivo DOP» o «Efectivo USD»), por el importe exacto y **sin comisión** (no hay transferencia). Una tasa de cambio sobrante se ignora: en efectivo no hay conversión.
* **Queda enlazado:** el abono guarda la caja y lo debitado, de modo que **revertirlo devuelve el efectivo a la caja** (antes devolvía solo la deuda: «El abono no tenía cuenta asociada»).
* **Si la caja no existe**, el abono se rechaza sin tocar nada, con un mensaje que lo dice (antes se aceptaba). Las cajas ya existen en toda instalación.
* **Formulario:** la opción vacía del selector pasa a llamarse «Efectivo (caja por defecto, sin comisión)».
* **Abonos antiguos sin cuenta** quedan como estaban; solo cambia lo que se registre desde ahora.

### 🧪 Pruebas
* 4 nuevas del caso de uso (pesos, dólares con tasa sobrante, caja inexistente, reversión) y `ab6` de caracterización; se actualizan 6 que fijaban el comportamiento anterior. Seis mutaciones (no debitar, caja de pesos para todo, caja inexistente tolerada, abono sin enlazar a la caja, tasa conservada, comisión en efectivo): todas detectadas. **Rust 835 pasan.**
* No se repitió en la app empaquetada.

---

## 🚀 Versión 1.90.0 - 2026-10-08
**Auditoría A-03, séptimo vertical: las suscripciones (el más pesado).** Alta, edición, listado, cobro automático y confirmación de períodos pendientes dejan de llevar SQL y reglas en `main.rs`. Sin cambios de reglas; un único cambio visible conocido (abajo).

### 🔧 Qué se hace
* **Orden de trabajo:** primero 4 pruebas de caracterización (`su1`–`su4`, en su propio commit y contra el código de antes; completan `s1`–`s25`), después la extracción. Dos más (`su5`, `su6`) nacieron de las mutaciones: los mensajes exactos del cobro y de confirmar, el importe y la divisa del caso de un período descartado, que editar no reinicia `fecha_ultimo_pago` y que la fecha se rechaza antes que la divisa.
* **Dominio** (`dominio::suscripcion`): `regla_de_un_registro`, `condiciones_de_suscripcion` (importe → frecuencia → día), `proximo_cobro_declarado` y `fecha_de_correccion`; siete errores nuevos con el texto exacto de siempre.
* **Caso de uso** (`aplicacion::suscripciones`): crear, editar, corregir el próximo cobro, eliminar, listar con aviso/impedimento/pendientes, categoría de las suscripciones, cobros automáticos, `asentar_cargo` y `confirmar_pendiente` (asentar o descartar con caso de corrección).
* **Puerto y adaptadores:** `AlmacenSuscripciones`, sobre `AlmacenSqlite` y su doble en memoria.
* **`main.rs`:** los comandos pasan por `con_almacen`; se conservan `suscripciones_con_aviso`, `procesar_suscripciones_con` y `confirmar_pendiente`. El cobro automático resuelve la categoría una vez y asienta **cada cobro en su propia transacción**. Llamadas directas a la base: **16 → 9** (de 54 al empezar A-03).
* **Cambio visible conocido:** los errores del esquema (restricciones de la base) llevan ahora el prefijo «Error de almacenamiento: »; los mensajes de dominio no cambian.

### 🧪 Pruebas
* 6 de caracterización y 9 del caso de uso con el doble. Veinticinco mutaciones (reglas de condiciones, orden fecha/divisa, categoría de respaldo, un solo período automático, fecha de hoy en lugar del vencimiento, puntero y marca, descartar que cobra o que no avanza, motivo sin exigir, orden del listado, editar que pisa el último pago, mensajes…): todas detectadas tras añadir `su5` y `su6`. **Rust 832 pasan.**
* No se repitió la comprobación en la app empaquetada: la cubren las pruebas de caracterización contra SQLite real.

---

## 🚀 Versión 1.89.0 - 2026-10-08
**Auditoría A-03, sexto vertical: los gastos.** Listar, crear y eliminar un gasto dejan de llevar SQL y reglas en `main.rs`. Sin cambios de reglas ni de mensajes.

### 🔧 Qué se hace
* **Orden de trabajo:** primero 5 pruebas de caracterización (`gt1`–`gt5`, en su propio commit, contra el código de antes; completan `c1`–`c14`, `c22`–`c24` y `c136`–`c137`) y después la extracción, con todas intactas. El registro y la reversión ya vivían en casos de uso; lo que seguía en `main.rs` era el listado y todo lo que rodea a eliminar.
* **Caso de uso** (`aplicacion::gastos`): `listar_gastos` y `eliminar_gasto`, con el orden de siempre: **primero** si lo creó otra operación (el cargo de un avance, la comisión de un abono: ese no se borra solo y su mensaje manda aunque el motivo sea corto), después el gasto, el motivo, el caso de corrección y, **al final**, la reversión.
* **Puerto y adaptadores:** `ConsultaDeGastos` (listado, resumen del gasto y «por qué no se borra solo»), implementado sobre `AlmacenSqlite` y su doble. La lista de operaciones que crean gastos sigue en `db_sql::GASTOS_DERIVADOS`, vigilada por su prueba contra las claves ajenas reales; el adaptador solo la consulta.
* **`main.rs`:** los tres comandos pasan por `con_almacen`. Comandos con SQL directo: **19 → 16** (de 54 al empezar A-03).
* **Sin cambios visibles.**

### 🧪 Pruebas
* 5 de caracterización y 4 del caso de uso con el doble. Doce mutaciones (el derivado que no se rechaza o que se rechaza después del motivo, motivo sin validar, revertir antes de abrir el caso, caso sin descripción o con importe cero o sin divisa, reversión que no revierte, orden del listado, columna del resumen, derivados sin vigilar, columnas cruzadas): todas detectadas. **Rust 813 pasan.**
* **Comprobado en la app empaquetada** (HOME temporal): gastos en efectivo (60.505 → 60.51, la caja queda en negativo si no tenía), por transferencia (con su comisión del 0.20 %) y con tarjeta, el rechazo de la tarjeta sin indicar, el listado, eliminar los tres (inexistente y motivo corto rechazados): la caja, la cuenta y la tarjeta **vuelven exactamente al inicio** y quedan tres casos; y el gasto derivado (la comisión de un abono) se rechaza con su mensaje aunque el motivo sea corto.

---

## 🚀 Versión 1.88.0 - 2026-10-08
**Auditoría A-03, quinto vertical (parte 4 de Tarjetas, la última): las bonificaciones. Con ella queda cerrado el vertical de Tarjetas.** Sin cambios de reglas ni de mensajes.

### 🔧 Qué se hace
* **Lo que quedaba era poco:** registrar y revertir una bonificación y liquidar un consumo pendiente ya vivían en casos de uso; en `main.rs` solo seguía el SQL del listado. Pasa al puerto `ConsultaDeBonificaciones` (con su adaptador sobre `AlmacenSqlite` y su doble) y al caso de uso `listar_bonificaciones`.
* **Orden de trabajo:** primero 2 pruebas de caracterización (`bo1`: el listado con los datos de la tarjeta, del más nuevo al más viejo; `bo2`: los mensajes de eliminar una bonificación inexistente y de liquidar lo que no se puede), en su propio commit y contra el código de antes; después la extracción.
* **`main.rs`:** los cuatro comandos (`obtener_bonificaciones`, `crear_bonificacion`, `eliminar_bonificacion`, `liquidar_consumo_pendiente`) pasan por `con_almacen`, que abre y confirma la transacción. Comandos con SQL directo: **23 → 19** (de 54 al empezar A-03).
* **Sin cambios visibles.**

### 🧪 Pruebas
* 2 de caracterización y 2 del caso de uso con el doble. Siete mutaciones (listado ascendente, columnas cruzadas, la divisa de la bonificación invertida, eliminar que no revierte, liquidar con otra divisa): todas detectadas. **Rust 804 pasan.**
* **Comprobado en la app empaquetada** (HOME temporal): una bonificación de 120.505 se guarda como 120.51 y baja la deuda en esa cifra, se rechazan el concepto vacío y el monto que redondea a cero, eliminarla repone la deuda exactamente, y se liquida un consumo pendiente en dólares (6050.005 → 6050.01, tasa deducida 60.5001) y liquidarlo de nuevo se rechaza.

---

## 🚀 Versión 1.87.0 - 2026-10-08
**Auditoría A-03, quinto vertical (parte 3 de Tarjetas): los avances de efectivo.** Registrar, listar y revertir un avance dejan de llevar SQL y reglas en `main.rs`. Sin cambios de reglas ni de mensajes.

### 🔧 Qué se hace
* **Orden de trabajo:** primero 5 pruebas de caracterización (`av1`–`av5`, en su propio commit, contra el código de antes; completan `c108`–`c127`) y después la extracción, con todas intactas.
* **Dominio** (`dominio::avance`): lo que llegaba de la interfaz y se resolvía en `main.rs` pasa a reglas con prueba: el cargo a partir de su tipo (`porcentaje`, `fijo` o `exonerado`: cada tipo exige lo suyo y **solo** lo suyo; un valor de más es una contradicción), la fecha (recortada, `dd/mm/aaaa` de un día que existe) y la nota (recortada; en blanco es ausente). Los cinco mensajes son los de siempre.
* **Caso de uso** (`aplicacion::avances`): `registrar_avance` (el cargo se asienta como gasto de la categoría de sistema «Otros»), `listar_avances` y `revertir_avance` (el avance, el motivo, el caso de corrección y, **al final**, la reversión).
* **Puertos y adaptadores:** `ConsultaDeAvances` (historial y resumen), implementado sobre `AlmacenSqlite` y su doble; la categoría de sistema pasa a su propio puerto `CategoriaDeSistema`, que comparten abonos y avances.
* **`main.rs`:** los cuatro comandos quedan delgados sobre `con_almacen`; se va `cargo_de_avance`. Comandos con SQL directo: **26 → 23** (de 54 al empezar A-03).
* **Sin cambios visibles.** Un detalle que la extracción dejó a la vista: la comprobación de la fecha del comando es más permisiva que el esquema (acepta `1/10/2026`, con el día sin cero) y es el esquema, estricto, quien la rechaza al guardar; queda dicho en una prueba.

### 🧪 Pruebas
* 5 de caracterización, 5 del dominio y 5 del caso de uso con el doble. Diecisiete mutaciones (cada tipo de cargo que deja pasar lo que no debe, fecha sin recortar o sin validar, nota en blanco o sin recortar, comisión sin categoría, motivo sin validar, revertir antes de abrir el caso, caso sin fecha o con importe cero, reversión que no revierte, orden y filtro del historial, columna equivocada del resumen): todas detectadas. **Rust 800 pasan.**
* **Comprobado en la app empaquetada** (HOME temporal): simular, registrar con porcentaje, fijo y exonerado (con fecha y nota con espacios), los cinco rechazos con su mensaje, el historial y revertir los tres: la tarjeta, la cuenta y los gastos **vuelven exactamente al inicio** y quedan tres casos de corrección.

---

## 🚀 Versión 1.86.0 - 2026-10-08
**Auditoría A-03, quinto vertical (parte 2 de Tarjetas): los abonos.** Registrar, listar y revertir un abono a tarjeta dejan de llevar SQL en `main.rs`. Sin cambios de reglas ni de mensajes.

### 🔧 Qué se hace
* **Orden de trabajo:** primero 5 pruebas de caracterización (`ab1`–`ab5`, en su propio commit, contra el código de antes; completan `c17`–`c20` y `c63`–`c67`) y después la extracción, con todas intactas. El registro y la reversión ya vivían en casos de uso (`registrar_pago_tarjeta`, `revertir_pago_tarjeta`); lo que seguía en `main.rs` era la categoría de la comisión, el historial y el caso de corrección de la reversión.
* **Caso de uso** (`aplicacion::abonos`): `registrar_abono` (la comisión se asienta como gasto de la categoría de sistema «Otros»), `listar_abonos` (del más nuevo al más viejo) y `revertir_abono` (el abono, el motivo, el caso de corrección y, **al final**, la reversión: si algo falla antes no queda un caso huérfano). El texto del resumen sigue formándose en el comando.
* **Puerto y adaptadores:** `ConsultaDeAbonos` (historial, resumen del abono y categoría de sistema), implementado sobre `AlmacenSqlite` y su doble.
* **`main.rs`:** los tres comandos delgados sobre `con_almacen`. Comandos con SQL directo: **29 → 26** (de 54 al empezar A-03).
* **Sin cambios visibles.**

### 🧪 Pruebas
* 5 de caracterización y 6 del caso de uso con el doble. Doce mutaciones (comisión sin categoría, motivo sin validar, revertir antes de abrir el caso, caso sin fecha o importe o divisa, reversión que no revierte, orden y filtro del historial, abono sin cuenta que desaparece, columna equivocada, categoría equivocada): todas detectadas. **Rust 785 pasan.**
* **Comprobado en la app empaquetada** (HOME temporal): un abono con cuenta (5432.005 → 5432.01, con su comisión), uno sin cuenta, uno en dólares con tasa, uno en cero rechazado, el historial, y revertir los tres (inexistente y motivo corto rechazados): la deuda y la cuenta **vuelven exactamente al inicio** y quedan tres casos de corrección.

---

## 🚀 Versión 1.85.0 - 2026-10-06
**Auditoría A-03, quinto vertical (parte 1 de Tarjetas): listar, crear y corregir los límites de las tarjetas salen de `main.rs`.** Sin cambios de reglas ni de mensajes. Quedan para las siguientes partes los abonos, los avances de efectivo y las bonificaciones.

### 🔧 Qué se hace
* **Orden de trabajo:** primero 7 pruebas de caracterización (`r1`–`r7`, en su propio commit, contra el código de antes) y después la extracción, con todas intactas.
* **Dominio** (`dominio::tarjeta`): el cupo por divisa que muestra el listado (efectivo y disponible, con su degradación a la cuenta en bruto ante datos raros) y los recordatorios de corte y de pago (alerta a 3 días o menos; el mes se aproxima con 30 días, como siempre) salen de la consulta.
* **Caso de uso** (`aplicacion::tarjetas`): listar (con la política normalizada, el cupo y los avisos), crear y corregir los límites (el cero es un tope deliberado, distinto de «sin ajuste»; una política ausente o desconocida es «origen»; el balance no se toca).
* **Puerto y adaptadores:** `CatalogoDeTarjetas`, `TarjetasSqlite` y el doble `TarjetasEnMemoria`.
* **`main.rs`:** los tres comandos quedan delgados sobre `con_tarjetas`; sale el ayudante `cupo`. Y `ImporteDecimal::unidades()`, que ya no usa nadie, se retira: todos los importes entran por `con_divisa(...)`. Comandos con SQL directo: **31 → 29** (de 54 al empezar A-03).
* **Un cambio visible, en un caso que sí puede ocurrir:** un día de corte o de pago fuera de 1–31 lo rechaza el esquema como siempre, pero el mensaje ahora empieza por «Error de almacenamiento: » (antes era el texto crudo de SQLite).

### 🔎 Hallazgos (sin corregir, a consultar; fijados por `r2` y `r4`)
* **Crear una tarjeta no recorta ni valida** entidad y nombre (acepta vacíos y con espacios) **ni que los límites sean positivos** (acepta uno negativo). Solo los días los valida el esquema.
* **Corregir los límites de una tarjeta que no existe no dice nada**: el `UPDATE` afecta a cero filas y el comando devuelve éxito (como pasaba con H18 en las facturas).

### 🧪 Pruebas
* 7 de caracterización, 3 del dominio y 6 del caso de uso con el doble. Dieciséis mutaciones (mes de 30 días, umbral de alerta, mensajes, cupo con y sin ajuste y con y sin balance, divisas cruzadas, política sin normalizar, corte con la fecha de pago, límites y fechas cruzados, cero como «sin ajuste», `UPDATE` sin `WHERE`, columnas cruzadas, orden del listado): todas detectadas (una, la de la cuenta en bruto, solo tras añadir un caso con un balance imposible). **Rust 774 pasan.**
* **Comprobado en la app empaquetada** (HOME temporal): alta con el céntimo decidido (1000.005 → 1000.01), días fuera de rango, el listado con cupo y recordatorio, corregir los límites (el ajuste en cero deja el efectivo en dólares en 0) y corregir una tarjeta inexistente.

---

## 🚀 Versión 1.84.0 - 2026-10-06
**Auditoría A-03, cuarto vertical: los préstamos y líneas de crédito salen de `main.rs`.** Los siete comandos (listar, crear, corregir condiciones, pagar cuota, declarar saldo, movimientos y eliminar) pasan a dominio, caso de uso y adaptador. Sin cambios de reglas ni de mensajes.

### 🔧 Qué se hace
* **Orden de trabajo:** primero 9 pruebas de caracterización (`q1`–`q9`, en su propio commit, contra el código de antes; completan `c34`–`c44`) y después la extracción, con todas intactas.
* **Dominio** (`dominio::prestamo`): las cuotas con las que nace un financiamiento (una línea flexible **no cuenta cuotas** y ignora las que se le den; los demás tipos exigen las dos y que las pendientes no superen a las totales), el día de pago (1–31), el límite de crédito (solo en una línea revolvente) y el **recordatorio de pago** (alerta a 3 días o menos; el mes se aproxima con 30 días, como siempre), antes mezclado con la consulta. Los siete mensajes de error son los de siempre.
* **Caso de uso** (`aplicacion::prestamos`): crear, corregir, pagar una cuota (amortiza solo el capital; si la cuota no cubre el interés la deuda crece; baja el contador sin pasar de cero), declarar el saldo (deja la diferencia como movimiento propio), listar (con saldo, cupo, el día de pago de la tarjeta y el recordatorio) y eliminar. Corregir sigue sin tocar el monto original ni el saldo.
* **Puerto y adaptadores:** `AlmacenPrestamos`, `PrestamosSqlite` y el doble `PrestamosEnMemoria`.
* **`main.rs`:** los comandos quedan delgados sobre `con_prestamos`; salen `estado_prestamo` y `asentar_movimiento`. Comandos con SQL directo: **37 → 31** (de 54 al empezar A-03).
* **Un cambio visible, mínimo:** el listado y los movimientos ahora corren dentro de una transacción de lectura y, como en los demás verticales, un error del esquema llevaría el prefijo «Error de almacenamiento: ». Los demás mensajes no cambian.

### 🧪 Pruebas
* 9 de caracterización, 5 del dominio y 12 del caso de uso con el doble. Veinticuatro mutaciones (reglas de cuotas, límites del día, límite en cualquier tipo, umbral y mes del recordatorio, saldo por omisión, orden de las comprobaciones, tarjeta sin comprobar, el saldo bajando por la cuota entera, contador sin descontar o sin tope, movimiento sin interés, signo de la declaración, día de la tarjeta, cupo, orden de las listas, saldo sin actualizar): todas detectadas. **Rust 758 pasan.**
* **Comprobado en la app empaquetada** (HOME temporal): alta con el céntimo decidido (100000.005 → 100000.01), una flexible que ignora las cuotas, los tres rechazos de alta, corregir (inexistente, tarjeta inexistente y válido), pagar (cuota de 5000.01: 1000 de interés y 4000.01 de capital), declarar, el libro de movimientos, y eliminar (dos veces) con su libro.

---

## 🚀 Versión 1.83.0 - 2026-10-06
**Auditoría A-03, tercer vertical (parte 3): los ingresos informales — listar, crear, cobrar, cobrar en efectivo y eliminar — salen de `main.rs`.** Sin cambios de reglas ni de mensajes.

### 🔧 Qué se hace
* **Orden de trabajo:** primero 8 pruebas de caracterización (`p1`–`p8`, en su propio commit, contra el código de antes; completan `c81`–`c84b`) y después la extracción, con todas intactas.
* **Caso de uso** (`aplicacion::informales`): `crear_ingreso_informal`, `crear_cobro_efectivo_informal` (el importe se convierte una sola vez para el ingreso y la caja; solo «USD» va a la caja de dólares, cualquier otra divisa a la de pesos), `marcar_informal_pagado` y `eliminar_ingreso_informal` (caso antes de borrar; revierte el abono sin recortar a cero). Comparte con las facturas la comprobación del depósito (`resolver_deposito`, que sale de `main.rs`).
* **Puertos:** `AlmacenInformales` (con su adaptador sobre `AlmacenSqlite` y su doble) y `BusquedaDeCuentas` (cuenta por nombre o por identificador), que sale de `AlmacenIngresos` porque ahora lo comparten las dos.
* **`main.rs`:** los cinco comandos quedan delgados; se eliminan `resolver_deposito` y `acreditar`, que ya no usa nadie. Comandos con SQL directo: **42 → 37** (de 54 al empezar A-03).

### 🔎 Hallazgos (sin corregir, a consultar; fijados por `p2` y `p6`)
* **Crear un informal acepta monto cero o negativo y datos vacíos** (fecha, descripción): el formulario los exige, el comando no. Mismo hueco que la factura.
* **Un cobro en efectivo sin caja registrada** (la cuenta «Efectivo DOP» o «Efectivo USD» renombrada o ausente) **queda registrado como cobrado y no mueve ningún saldo**, sin avisar: es el hueco que H3 cerró para los gastos en efectivo, y este comando sigue localizando la caja por nombre.

### 🐛 Una regresión cazada antes de publicar
* La comprobación en la app empaquetada reveló que, en mi primera versión de la extracción, **eliminar un cobro en efectivo en dólares fallaba** («No se pueden combinar montos en USD y DOP»): la reversión se expresaba en pesos y el saldo exacto de una caja en dólares la rechaza. Las pruebas de caracterización solo cubrían el caso en pesos. Corregido: los ajustes por nombre de cuenta (eliminar un informal o una factura, corregir una factura, el cobro en efectivo) se expresan **en la divisa de la propia cuenta**, como hacía el SQL de antes. Se añaden `p9` y una prueba del caso de uso.

### 🧪 Pruebas
* 9 de caracterización y 9 del caso de uso con el doble. Dieciséis mutaciones (monto cambiado, caja equivocada, efectivo sin acreditar o que exige caja, depósito sin comprobar, ingreso inexistente sin aviso, cobro sin acreditar, motivo sin validar, eliminar sin revertir o revirtiendo lo pendiente o sin borrar, caso sin descripción, cobro sin filtro de pendiente, listado ascendente, efectivo que nace pendiente) y la de «no convierte a la divisa de la cuenta»: todas detectadas. **Rust 731 pasan.**
* **Comprobado en la app empaquetada** (HOME temporal): crear con el céntimo decidido (300.005 → 300.01), cuenta e ingreso inexistentes, cuenta en dólares, cobrar y cobrar de nuevo, cobros en efectivo en pesos y en dólares, y eliminar todo con sus casos; al final **todos los saldos vuelven al inicio** (100, 0, 0).

---

## 🚀 Versión 1.82.0 - 2026-10-06
**Auditoría A-03, tercer vertical (parte 2): corregir y eliminar facturas salen de `main.rs`, y la regla del motivo de una corrección pasa al dominio.** Sin cambios de reglas ni de mensajes.

### 🔧 Qué se hace
* **Orden de trabajo:** primero 7 pruebas de caracterización (`o1`–`o7`, en su propio commit, contra el código de antes; completan `c79`, `c80` y `c85`–`c94`) y después la extracción, con todas intactas.
* **Dominio** (`dominio::correccion`): la regla «el motivo debe explicar» (mínimo 15 **caracteres**, recortado) deja de vivir mezclada con el SQL de `correcciones.rs`; el mensaje es el de siempre. `correcciones::registrar` (que usan los demás borrados) la toma de ahí: **una sola regla**.
* **Puerto nuevo** `RegistroDeCorrecciones` (anotar el caso y devolver su número), implementado sobre `AlmacenSqlite`, y métodos nuevos en `AlmacenIngresos` (estado de la factura, reescribirla, fijar lo recibido, eliminarla, cuenta por nombre).
* **Casos de uso** (`aplicacion::ingresos`): `actualizar_ingreso` (la factura, el cálculo, la fila y, **solo si hay dinero que mover**, el caso, lo recibido y el saldo de la cuenta de depósito) y `eliminar_ingreso` (abre el caso antes de borrar y revierte el abono sin recortar a cero). El comando solo da formato al texto del resultado.
* **`main.rs`:** los dos comandos quedan delgados. Comandos con SQL directo: **44 → 42**. Siguen usando `abrir_caso` y `correcciones::registrar` los borrados de gastos, abonos, traspasos, informales y avances (se irán con sus verticales).
* **Un borde documentado, no corregido (`o6`):** eliminar una factura cobrada en una cuenta que ya no existe la borra igual y no mueve saldos, mientras que corregirla se niega con un mensaje. Son los comportamientos de siempre. Y, como el ajuste ahora pasa por el saldo exacto de la cuenta, si la cuenta de depósito estuviera en otra divisa el ajuste se rechaza en vez de aplicarse a ciegas (cobrar en otra divisa ya estaba prohibido, H19).

### 🧪 Pruebas
* 7 de caracterización, 2 del dominio y 10 del caso de uso con el doble. Quince mutaciones (factura inexistente sin error, caso siempre o nunca, motivo sin validar, lo recibido sin fijar, ajuste al revés, cuenta desaparecida tolerada, eliminar sin revertir o sin borrar o revirtiendo lo no cobrado, umbral del motivo, bytes en vez de caracteres, motivo sin recortar, cuenta por nombre): todas detectadas (una, la guarda de estatus al eliminar, solo tras añadir una prueba con datos incoherentes). **Rust 712 pasan.**
* **Comprobado en la app empaquetada** (HOME temporal): corregir inexistente, con motivo corto, sin cobrar y al alza (se ajusta la cuenta en 850.00 y queda el caso), sin diferencia (no abre caso), eliminar inexistente, con motivo corto, cobrada (revierte el abono: la cuenta vuelve a 100) y pendiente, con sus casos de corrección.
* **Un cambio visible, en un caso que sí puede ocurrir:** si al corregir una factura se le pone un número que ya usa otra, el esquema lo rechaza como siempre, pero el mensaje ahora empieza por «Error de almacenamiento: » (antes era el texto crudo de SQLite). Mismo caso que en Cuentas.

---

## 🚀 Versión 1.81.0 - 2026-10-06
**Auditoría A-03, tercer vertical (primera parte): ingresos formales — listar, emitir y cobrar facturas — salen de `main.rs`.** Sin cambios de reglas ni de mensajes. Quedan para un segundo PR corregir y eliminar facturas, que además abren casos de corrección.

### 🔧 Qué se hace
* **Orden de trabajo:** primero 7 pruebas de caracterización (`n1`–`n7`, en su propio commit, contra el código de antes; completan `c70`–`c78b`) y después la extracción, con todas intactas.
* **Caso de uso** (`aplicacion::ingresos`): `crear_ingreso` (número repetido sin distinguir mayúsculas, **antes** de crear ningún cliente; cliente por RNC que conserva su nombre; retención al céntimo) y `marcar_ingreso_pagado` (cuenta, depósito en la divisa de la cuenta, factura pendiente y, solo al final, el saldo; cobrar dos veces no acredita dos veces).
* **Puerto y adaptadores:** `AlmacenIngresos`, implementado sobre `AlmacenSqlite` (que ya envuelve la transacción y el puerto de cuentas: el cobro toca las dos cosas), y el doble en memoria.
* **`main.rs`:** los tres comandos delgados sobre `con_almacen`. Comandos con SQL directo: **46 → 44**.
* **Sin cambios visibles:** los mensajes son los de siempre (la cuenta inexistente, la factura no pendiente, la divisa incompatible, el importe inválido, el número repetido). `resolver_deposito` se queda en `main.rs`: todavía lo usan los cobros informales.

### 🔎 Hallazgos (sin corregir, a consultar; fijados por `n4`)
* `crear_ingreso` **acepta un porcentaje de retención fuera de 0–100** (150 % o −5 %, con una retención mayor que el total o negativa) y **datos vacíos** (número de factura, RNC y nombre): el formulario los exige, el comando no. Es la misma clase de hueco que tenía el abono no positivo.

### 🧪 Pruebas
* 7 de caracterización y 7 del caso de uso con el doble. Doce mutaciones (sin chequeo de duplicado, cliente siempre nuevo, porcentaje o retención erróneos, sin comprobar la divisa del depósito, factura inexistente sin aviso, saldo antes de cobrar o sin acreditar, cobro sin filtro de pendiente, duplicado sensible a mayúsculas, listado ascendente, nombre de otra cuenta): todas detectadas. **Rust 691 pasan.**
* **Comprobado en la app empaquetada** (HOME temporal): emitir con el céntimo decidido (1234.565 → 1234.57, retención 185.19), número repetido sin distinguir mayúsculas, cuenta y factura inexistentes, cuenta en dólares, cobro que acredita lo mismo que guarda la fila, y cobrar de nuevo rechazado sin acreditar dos veces.

---

## 🚀 Versión 1.80.0 - 2026-10-05
**Auditoría A-03, segundo vertical: las cuentas salen de `main.rs`.** Alta, corrección, listado y el historial de transferencias pasan a dominio, caso de uso y adaptador. Sin cambios de reglas.

### 🔧 Qué se hace
* **Orden de trabajo:** primero 8 pruebas de caracterización (`m1`–`m8`, en su propio commit, contra el código de antes) y después la extracción, con esas pruebas intactas. Transferir y eliminar cuentas ya pasaban por casos de uso.
* **Dominio** (`dominio::cuenta`): nombre recortado y no vacío; entidad en blanco = ausente; comisión **ninguna ≠ cero**, y la negativa se rechaza. Los mensajes son los de siempre.
* **Caso de uso** (`aplicacion::cuentas`): crear y corregir cuenta, con el mismo orden de comprobaciones (nombre, luego comisión). El saldo **no** se corrige por aquí.
* **Puerto y adaptadores:** `CatalogoDeCuentas`, `CuentasSqlite` y el doble `CuentasEnMemoria`.
* **`main.rs`:** cuatro comandos delgados sobre `con_cuentas`. Comandos con SQL directo: **49 → 46** (de 54 al empezar A-03).
* **Un cambio visible, en casos que la interfaz casi no produce:** un nombre repetido o una divisa ilegal los rechaza el esquema como siempre, pero el mensaje ahora empieza por «Error de almacenamiento: » (antes era el texto crudo de SQLite). Cada comando corre ahora en una transacción.

### 🧪 Pruebas
* 8 de caracterización, 3 del dominio y 6 del caso de uso con el doble. Trece mutaciones (nombre sin recortar, entidad en blanco, comisión negativa o cero tratada como ninguna, comprobaciones en otro orden, inexistente sin aviso, saldo o entidad perdidos, orden de las listas, columnas cruzadas, comisión sin divisa local): todas detectadas. **Rust 677 pasan.**
* **Comprobado en la app empaquetada** (HOME temporal): alta con recorte y céntimo decidido (250.005 → 250.01; 75.005 → 75.01), vacío, negativa, repetida, divisa ilegal, corrección, inexistente y el historial de una transferencia con los nombres.

---

## 🚀 Versión 1.79.0 - 2026-10-05
**Auditoría A-03, primer vertical: categorías y clientes salen de `main.rs` y pasan a dominio, caso de uso y adaptador.** Sin cambios de reglas ni de mensajes.

### 🔧 Qué se hace
* **Orden de trabajo (el de la auditoría):** primero pruebas de caracterización de los seis comandos (`k1`–`k7`, en su propio commit, contra el código de antes), después la extracción, con las mismas pruebas intactas.
* **Dominio** (`dominio::catalogo`): nombre de categoría recortado y no vacío; la categoría de sistema «Otros» y las que tienen gastos no se eliminan (en ese orden); cliente con RNC y nombre; cliente con facturas no se elimina. Los siete mensajes de error son los de siempre.
* **Caso de uso** (`aplicacion::catalogos`): crear y eliminar categoría y cliente sobre el puerto `AlmacenCatalogos`, con el mismo orden de comprobaciones que los comandos.
* **Puerto y adaptadores:** `AlmacenCatalogos` (puerto), `CatalogosSqlite` (SQL sobre una transacción abierta) y `CatalogosEnMemoria` (doble de pruebas).
* **`main.rs`:** los seis comandos quedan en una línea o dos sobre un ayudante (`con_catalogos`, que abre y confirma la transacción). Comandos con SQL directo: **54 → 49**.
* **Un único cambio visible, en un caso que la interfaz no produce:** eliminar una categoría cuyo identificador no existe antes mostraba el texto interno de SQLite («Query returned no rows»); ahora dice «No se encontró la categoría con identificador N.». Ahora además cada comando corre dentro de una transacción.

### 🧪 Pruebas
* 7 de caracterización (mensajes, recorte, duplicado sin distinguir mayúsculas, orden, guardas), 5 del dominio y 7 del caso de uso con el doble. Doce mutaciones (guardas invertidas, comparación sin mayúsculas, sin recortar, validación a medias, `>=` por `>`, sin comprobar duplicado ni facturas, borrar antes de validar, SQL sin `LOWER`, orden descendente, tabla o columna equivocadas): todas detectadas. **Rust 660 pasan.**
* **Comprobado en la app empaquetada** (HOME temporal): crear recortado, duplicado, vacío, «Otros», inexistente, cliente duplicado y vacío, y borrados, con los mensajes de siempre; las categorías vuelven ordenadas.

---

## 🚀 Versión 1.78.0 - 2026-10-05
**Auditoría A-02, segunda mitad: la aplicación ahora tiene una política de contenido (CSP).**

### 🐛 Qué estaba mal
* `security.csp` era `null`: sin política, una inyección en la interfaz podía cargar scripts de cualquier sitio, abrir marcos, ejecutar `eval` o enviar datos a un servidor externo.

### 🔧 Qué se hace
* **CSP:** `default-src 'self'`; `script-src 'self'` (sin `unsafe-inline` ni `unsafe-eval`); `object-src 'none'`; `base-uri 'none'`; `form-action 'none'`; `connect-src 'self' ipc: http://ipc.localhost` (ningún origen externo); `font-src` y `style-src` con `fonts.googleapis.com`/`fonts.gstatic.com` solo para las fuentes; `img-src 'self' data: asset:`.
* **Dos excepciones, y son las únicas:** `script-src-attr 'unsafe-inline'` y `style-src-attr 'unsafe-inline'`, porque las plantillas usan manejadores en línea (`onclick=`, `onsubmit=`) y `style=`. Sin ellas la interfaz entera dejaría de responder. Quitarlas exige migrar esos manejadores a `addEventListener`, trabajo aparte; el contrato de pruebas lo deja anotado. Un script en línea, `eval` o una conexión externa **siguen bloqueados**.

### 🧪 Pruebas
* JS: 3 contratos nuevos en `tauri_superficie.test.js` (CSP no nula y cerrada por defecto; solo las excepciones esperadas; `index.html` sin scripts en línea, de otros orígenes ni `<style>`). Diez mutaciones (CSP nula, `unsafe-eval`, `unsafe-inline` en scripts, script o conexión externos, sin `object-src`, `form-action` abierto, `default-src *`, script y estilo en línea en `index.html`): todas detectadas.
* **Comprobado en la app empaquetada**, con un HOME temporal: los manejadores en línea y los estilos en atributo **funcionan**; un `<script>` inyectado, `eval`, `fetch` a un sitio externo quedan **bloqueados**; las **11 pestañas** se dibujan y un formulario con `onsubmit` real (agregar categoría) guarda sin navegar.

---

## 🚀 Versión 1.77.0 - 2026-10-02
**Auditoría A-01 y A-02: un capital ilegible ya no se sobrescribe, y la interfaz pierde las APIs nativas de Tauri que no usa.**

### 🐛 A-01 — Qué estaba mal
* `leer_coleccion` convertía cualquier fallo de lectura o de JSON en un capital **vacío**. La pestaña de capital mostraba «sin datos» y el siguiente guardado **sobrescribía sin aviso** el archivo dañado, justo cuando más falta hacía recuperarlo.

### 🔧 Qué se hace
* **A-01:** `leer_coleccion` devuelve `Result`: un archivo **ausente** sigue dando la estructura inicial (primer arranque), pero uno que existe y **no se puede leer** o **no es JSON válido** (incluido uno vacío) da un error con mensaje claro. `obtener_capital` lo muestra y `guardar_capital` **se niega a guardar**; el original queda intacto, sin siquiera escribir el temporal.
* **A-02 (allowlist):** `allowlist.all` pasa a `false` y se quita la función `api-all` de Cargo. La interfaz solo usa `invoke` hacia los comandos de Rust, que Tauri deja siempre disponible; el sistema de archivos, el shell, HTTP, diálogos y procesos ya no están al alcance de la capa web. `withGlobalTauri` se **mantiene**: `api.ts` depende de `window.__TAURI__`. La CSP (la otra mitad de A-02) queda para otro PR: la interfaz usa manejadores y estilos en línea y exige probarla en la app empaquetada. Al quitar `api-all` también salen del `Cargo.lock` 187 dependencias que no se usaban (de 584 a 397).

### 🧪 Pruebas
* Rust: 5 nuevas con archivos sintéticos (sin archivo, JSON truncado, archivo de cero bytes, un directorio donde iría el archivo = error de E/S, y el camino sano); en las de error se comprueba que el original queda byte a byte igual. JS: contrato nuevo de la superficie (allowlist cerrada, Cargo sin `api-all`, y que la interfaz solo use `invoke`). Mutaciones: 5 de A-01 (4 detectadas; la quinta es equivalente para el capital) y 4 de A-02 detectadas. **Rust 641, JS 584 pasan.**
* **Comprobado en la app empaquetada**, con un HOME temporal: `fs.readTextFile` y `shell.open` responden «módulo no habilitado»; `invoke` funciona; con un `capital.json` truncado, `obtener_capital` y `guardar_capital` fallan con el mensaje y el archivo queda intacto; con el capital sano, lee y guarda.

---

## 🚀 Versión 1.76.0 - 2026-10-02
**Aviso de cobros próximos para todas las suscripciones, también en el Resumen, con la tarjeta que cobra y la opción de cambiarla a tiempo.**

### 🔧 Qué se hace
* **Regla (Rust):** `avisa` deja de limitarse a las anuales: toda suscripción cuyo próximo cobro cae en los próximos siete días avisa, mensual o anual. Es una **decisión del titular** (2026-10-02) que revierte la anterior de avisar solo las anuales «para no hacer ruido»; el motivo es dar tiempo a cambiar la tarjeta que cobra, por ejemplo a una que bonifique las compras por internet.
* **Suscripciones:** el aviso «🔔 Cobro próximo» nombra la tarjeta que cobra y trae un botón **✏️ Cambiar tarjeta** (abre la edición) y una nota sobre bonificaciones.
* **Resumen:** nuevo aviso «🔔 Cobros próximos» arriba, con plataforma, importe, fecha y tarjeta de cada uno (texto escapado), y la indicación de editar la suscripción antes de la fecha.

### 🧪 Pruebas
* Rust: la prueba del dominio y la de caracterización (`s19`, que afirmaba que una mensual NO avisaba) se invierten y cubren la ventana de siete días y la ausencia de fecha. JS: 3 nuevas (aviso del Resumen con escape y tarjeta, sin aviso cuando no toca, aviso de Suscripciones con tarjeta y botón). Seis mutaciones del frontend y la de volver a «solo anuales» en Rust: todas detectadas. **Rust 636, JS 581 pasan.**

---

## 🚀 Versión 1.75.0 - 2026-10-02
**Suscripciones: «Cargos Activos» se divide en Mensuales y Anuales, cada bloque con su subtotal por divisa.**

### 🔧 Qué se hace
* La tabla de «Cargos Activos» pasa a dos bloques, **📅 Mensuales** y **🗓️ Anuales**, con la misma tabla y las mismas acciones (corregir, editar, eliminar). Cada bloque lleva su **subtotal por divisa** sin mezclarlas: el mensual «al mes» y el anual «al año» con su equivalente al mes (total / 12). Un bloque sin suscripciones no se dibuja; sin ninguna sigue el mensaje vacío.
* El formulario de alta sigue siendo **uno solo**, compartido (la frecuencia se elige en él).
* El Resumen ya mostraba mensuales y anuales por separado desde 1.74.0 (sus tres pruebas, que esperaban «Suscripciones Recurrentes», ya estaban ajustadas en ese PR).

### 🧪 Pruebas
* 4 nuevas (reparto por frecuencia, subtotales por divisa con el equivalente mensual, bloque vacío y formulario único). Cinco mutaciones (filtro de anuales invertido, divisas mezcladas, anual sin dividir entre 12, rótulo año/mes, bloque vacío visible): todas detectadas. **JS 578 pasan.**

---

## 🚀 Versión 1.74.0 - 2026-10-02
**Resumen: las suscripciones mensuales y anuales se muestran por separado.**

### 🔧 Qué se hace
* En la tarjeta «Carga Fija Mensual» las suscripciones se subdividen por frecuencia y por divisa (sin mezclarlas): **Suscripciones Mensuales** (suma de las mensuales) y **Suscripciones Anuales (al mes)**, con una nota «total del año» (la suma de lo que se cobra una vez al año). El equivalente al mes de las anuales se calcula **una sola vez** sobre ese total (total / 12), en vez de dividir cada suscripción.
* Los totales no cambian (la carga fija suma lo mismo que antes); solo se ve de dónde sale cada cifra. En divisas distintas de pesos la fila anual solo aparece si hay alguna anual.

### 🧪 Pruebas
* Tres pruebas existentes pasan a los nuevos rótulos y una nueva fija la subdivisión (dos anuales de 100 y 50 y una mensual de 7: 7, 12.50 al mes y «total del año 150.00»). Cinco mutaciones (dividir entre 11, intercambiar mensual y anual, no dividir la fila anual, mostrar siempre la fila anual en otras divisas, tomar «sin divisa» como dólares): todas detectadas. **JS 574 pasan.**

---

## 🚀 Versión 1.73.0 - 2026-10-02
**El núcleo rechaza un abono a tarjeta que no es positivo** (decisión del titular tras el hallazgo de 1.72.0).

### 🐛 Qué estaba mal
* `registrar_pago_tarjeta` aceptaba un abono de 0.00, de 0.004 (que redondea a cero) o **negativo, que subía la deuda**. La comprobación «mayor que cero» vivía solo en la interfaz.

### 🔧 Qué se hace
* La regla pasa al caso de uso: antes de tocar ningún saldo, un abono cero o negativo se rechaza con `ErrorDominio::AbonoSinImporte` («Un abono debe ser mayor que cero…»). Es la misma lógica que ya tenían la bonificación y la suscripción. La interfaz no cambia.

### 🧪 Pruebas
* La prueba de caracterización `c16c` se invierte (0.00, 0.004 y -100.00 se rechazan y la deuda no se mueve) y se añade una del caso de uso (cero y negativo no mueven ni la deuda ni la cuenta). Mutaciones: quitar el chequeo de cero o el de negativo, ambas detectadas. **Rust 636 pasan.**

---

## 🚀 Versión 1.72.0 - 2026-10-02
**Importes a texto: se migran los 12 comandos que quedaban y se retira la rama «número» de `ImporteDecimal`. La migración queda terminada.**

### 🐛 Qué estaba mal
* Quedaban comandos cuyos importes la interfaz convertía con `Number(...)`: el núcleo decidía el céntimo sobre un número que ya no valía lo que el titular tecleó (`1.005` llegaba como 1.00499…, y por texto sube a 1.01).

### 🔧 Qué se hace
* **Rust y envoltorios, un commit por comando:** `marcar_ingreso_pagado`, `marcar_informal_pagado`, `liquidar_consumo_pendiente`, `registrar_pago_tarjeta`, `crear_cuenta` (saldo inicial), `transferir_entre_cuentas` (origen, destino y cargo, cada uno con la divisa de su cuenta), `actualizar_ingreso` (total), `crear_tarjeta` (8 importes), `actualizar_limites_tarjeta` (6 + 2 ajustados; vacío sigue siendo «sin ajuste» y «0» un tope), `crear_gasto`, `crear_ingreso`, `crear_prestamo` y `actualizar_prestamo`.
* **Interfaz:** las vistas envían el texto recortado; donde el campo no es obligatorio y antes un vacío valía 0 (cargos, límites de tarjeta) se conserva ese comportamiento enviando «0». Las comparaciones (cotas, vista previa del neto) siguen usando el número, solo para comparar.
* **`ImporteDecimal` ya solo acepta texto:** un número JSON se rechaza. El tipo generado pasa de `string | number` a `string`, y la lista de pendientes del contrato queda vacía (debe seguir así).

### 🧪 Pruebas
* Pruebas Rust y JS nuevas por comando (céntimo decidido por texto, columnas sin cruzar, vacíos), mutaciones por comando (todas detectadas, salvo un mutante equivalente anotado). **Rust 635 pasan** (+2 ignoradas), **JS 573 pasan**.

### 🔎 Hallazgo (sin corregir, a consultar)
* `registrar_pago_tarjeta` **no rechaza un abono de 0.00, de 0.004 ni negativo** (uno negativo sube la deuda). Queda fijado por la prueba `c16c`; corregirlo es decisión del titular.

---

## 🚀 Versión 1.71.0 - 2026-10-02
**Importes a texto: `crear_bonificacion`, el tercero de los comandos que seguían enviándose como número** (quedan 12: ver `politica_redondeo.md`).

### 🐛 Qué estaba mal
* La bonificación (cashback, devolución promocional, recompensa) recibía el importe como `f64`, convertido en la interfaz con `Number(...)`, y lo pasaba a `Dinero::nuevo(monto, divisa)`: el núcleo decidía el céntimo, pero **sobre un número que ya no valía lo que el titular tecleó** (`1.005` llegaba como 1.00499…).

### 🔧 Qué se hace
* **Rust:** `monto` pasa a `ImporteDecimal` y se casa con la divisa declarada (`con_divisa`); el céntimo se decide con los dígitos escritos. Las reglas de la bonificación no cambian: un concepto vacío, un importe negativo o que redondea a cero se rechazan.
* **Interfaz:** el envoltorio manda `String(monto)` y la vista el texto recortado; el tipo del parámetro pasa a `string`. La comprobación «mayor que cero» de la vista sigue usando el número, **solo para comparar**: lo que viaja son los dígitos escritos.

### 🧪 Pruebas
* **JavaScript:** 2 nuevas (la fila del contrato, con la muestra `0075.250` que no sobrevive a ninguna conversión numérica, y el envío desde la vista con espacios, tres decimales y ceros) y dos existentes pasan de comprobar un número a un texto. **554 pruebas, todas pasan.**
* **Rust:** 2 nuevas (`c33b`: `1.005` en pesos baja la deuda 1.01 y `20.10` en dólares baja la de dólares por su cuenta, y lo guardado es el mismo céntimo; `c33c`: `-5.00`, `0.00` y `0.004` no son una bonificación y no mueven ningún saldo) y siete llamadas existentes se actualizan al nuevo tipo. **619 pasan** (más 2 ignoradas, las que vuelcan datos para comparar vistas).
* Mutaciones (ocho): el envoltorio de vuelta a `Number` o a `parseFloat`, la vista de vuelta a `Number`, sin recortar o sin la comprobación de «mayor que cero», y en Rust ignorar la divisa, multiplicar el importe por 100 o truncarlo a pesos enteros: todas se detectan.

### ✅ Comprobado a mano
* **En la aplicación empaquetada, con el IPC de Tauri, Rust y la base reales:** `1.005` en pesos baja la deuda de la tarjeta **1.01**; `20.10` en dólares baja **solo** la de dólares; el **formulario real** de Tarjetas registra 12.50 y avisa del éxito; un texto ilegible (`cien`) y un importe que redondea a cero (`0.004`) se rechazan; un número sigue admitido durante la transición; y las cuatro bonificaciones guardadas tienen el importe esperado.

---

## 🚀 Versión 1.70.0 - 2026-10-02
**Importes a texto: `crear_cobro_efectivo_informal`, el segundo de los comandos que seguían enviándose como número** (quedan 13: ver `politica_redondeo.md`).

### 🐛 Qué estaba mal
* El cobro informal en efectivo recibía el importe como `f64` (la interfaz lo convertía con `Number(...)`) y **lo usaba dos veces**: lo guardaba en el ingreso (`monto` y `monto_recibido`) y lo sumaba al saldo de la caja con `ROUND(balance_actual + ?, 2)`. El núcleo no decidía el céntimo en ninguno de los dos sitios.

### 🔧 Qué se hace
* **Rust:** `monto` pasa a `ImporteDecimal` y se convierte **una sola vez** con `unidades()`: el ingreso y la caja reciben exactamente el mismo valor, decidido con los dígitos escritos (`1.005` sube a 1.01).
* **Interfaz:** el envoltorio manda `String(monto)` y la vista el texto recortado; el tipo del parámetro pasa a `string`. Con dos decimales no cambia ningún importe (`321.10` ahora viaja como `"321.10"`, no como `321.1`).

### 🛡️ Cómo se lleva la cuenta
* `importes_texto.test.js` gana el envoltorio en su lista de migrados y lo pierde de la de pendientes. **Una mutación sobrevivió** (`String(parseFloat(monto))`) porque el importe de muestra, `1.005`, vuelve idéntico tras `parseFloat`; la muestra pasa a ser **`0075.250`**, que no sobrevive a ninguna conversión numérica (`1.005` se queda en una prueba aparte que delata el `Number`).

### 🧪 Pruebas
* **JavaScript:** 2 nuevas (la fila del contrato y el envío desde la vista con espacios, tres decimales, ceros y vacío) y dos existentes pasan de comprobar un número a un texto. **552 pruebas, todas pasan.**
* **Rust:** 2 nuevas (`c81b`: `1.005` deja el ingreso, lo recibido **y** la caja en el mismo 1.01; `c81c`: un cobro en dólares entra en la caja de dólares y no mueve la de pesos) y una existente se actualiza al nuevo tipo. **617 pasan** (más 2 ignoradas, las que vuelcan datos para comparar vistas).
* Mutaciones (ocho): el envoltorio de vuelta a `Number` o a `parseFloat`, la vista sin recortar o de vuelta a `Number`, y en Rust la caja con 100 veces el importe, el ingreso y la caja con valores distintos, el importe truncado y la divisa de dólares a la caja de pesos: todas se detectan.

### ✅ Comprobado a mano
* **En la aplicación empaquetada, con el IPC de Tauri, Rust y la base reales:** `1.005` por el IPC deja la caja de pesos en **+1.01** y el ingreso en 1.01 / 1.01; el **formulario real** de Efectivo, con 321.10 en dólares, suma 321.10 a la caja de dólares y guarda el ingreso como pagado; un texto ilegible (`cien`) se rechaza y no crea nada; un número sigue admitido durante la transición; y el aviso de éxito aparece.

---

## 🚀 Versión 1.69.0 - 2026-10-02
**Importes a texto: `crear_ingreso_informal` es el primero de los que seguían enviándose como número.**

### 🐛 Qué estaba mal
* La convención de 1.21.0 es que los importes viajan a Rust **como texto, tal cual se escribieron**, para que el núcleo decida el céntimo con los dígitos del titular y no con un `f64` que ya no vale lo que tecleó (`1.005` es 1.00499… en binario). Pero quedaban **once comandos y cuatro estructuras de entrada** que seguían recibiendo un `f64`: la interfaz los convertía con `Number(...)`.
* `crear_ingreso_informal` era el caso más limpio: **guardaba el número tal cual llegaba**, sin pasar por el núcleo.

### 🔧 Qué se hace
* **Rust:** `monto` pasa a `ImporteDecimal` y se guarda con `unidades()` (el céntimo ya decidido con los dígitos escritos).
* **Interfaz:** el envoltorio manda `String(monto)` (antes `Number(monto)`) y la vista envía el texto recortado, sin convertir. El tipo del parámetro pasa de `number | string` a `string`: el compilador impide mandar un número.
* **Con dos decimales no cambia ningún importe** (la medida de 1.21.0 y el `step="0.01"` lo garantizan). Con tres, en la frontera `1.005` sube a 1.01; el formulario no deja escribirlos.
* Los tipos generados (`tipos-ipc.d.ts`) reflejan `string | number` para el comando.

### 🛡️ Cómo se lleva la cuenta
* **`pruebas/js/contrato/importes_texto.test.js`:** (1) cada envoltorio **ya migrado** (nueve) se ejecuta con un `invoke` falso y debe mandar `1.005` **como texto con sus tres decimales**; (2) los envoltorios que **siguen** convirtiendo un importe con `Number(...)` están en una lista, que **solo puede encogerse**: uno nuevo que lo hiciera rompe la prueba, y uno migrado que siguiera listado también. El estado de los 14 que quedan está en `politica_redondeo.md`, «Migración de los importes a texto».

### 🧪 Pruebas
* **JavaScript:** 13 nuevas (los nueve envoltorios, el texto con ceros y espacios, las dos listas, y el envío desde la vista con espacios, tres decimales, ceros y vacío); dos pruebas existentes pasan de comprobar un número a un texto. **550 pruebas, todas pasan.**
* **Rust:** 2 nuevas (`c84b`: `1.005` se guarda como 1.01; `c84c`: dos decimales exactos y un texto ilegible no crea nada) y cuatro pruebas existentes se actualizan al nuevo tipo. **615 pruebas, todas pasan.**
* Mutaciones: el envoltorio de vuelta a `Number`, a `parseFloat`, la vista sin recortar o de vuelta a `Number` (esta la atrapa el compilador), y en Rust el importe multiplicado por 100 o truncado a entero: todas se detectan.

### ✅ Comprobado a mano
* **En la aplicación empaquetada, con el IPC de Tauri, Rust y la base reales:** `1.005` por el IPC se guarda como **1.01**; un número (`100`) sigue admitido durante la transición; un texto ilegible (`setenta`) **no crea nada** y Rust lo rechaza con un mensaje; y el **formulario real** de ingresos informales guarda **75.25** y avisa del éxito.

---

## 🚀 Versión 1.68.0 - 2026-10-02
**Corrección: ningún texto escrito por el titular se interpreta ya como HTML ni como código en ninguna pestaña.** Escape general de las plantillas.

### 🐛 Qué estaba mal
* La 1.67.0 corrigió el panel de «Casos de corrección» y reconoció que el problema era general. Lo era: las plantillas de las once vistas interpolaban en `innerHTML` **222 puntos de texto libre sin escapar** (nombres de cuentas, clientes y categorías; descripciones; conceptos; notas; plataformas; entidades; instituciones; motivos; mensajes de error), más los avisos emergentes y la tarjeta de error del enrutador, que pintaban cualquier mensaje tal cual. Un `<` en un nombre rompía la pantalla, y un `<img src=x onerror=…>` ejecutaba código en el WebView, cuyo puente con Rust admite todos los comandos.
* **Había una variante más grave, en los atributos.** Doce botones, en siete vistas, llevaban un dato de la fila dentro de su `onclick` (editar cuenta, suscripción, factura o tarjeta; el menú de un préstamo; liquidar un consumo; asentar o descartar un período; borrar un bien del capital), escrito a mano como `'${JSON}'` con un escape de comillas. El navegador **deshace el escape del atributo antes de ejecutar el manejador**, así que un `'` en el nombre cerraba la cadena y **lo que viniera detrás se ejecutaba como código al pulsar el botón** (y con una `\` o una comilla el botón simplemente dejaba de funcionar: «Cuenta O'Brien» no se podía editar).
* Quien escribe esos textos es el propio titular, en su propia aplicación: el riesgo real es bajo. Pero un texto guardado o importado no debe poder ejecutar nada, y un nombre con una comilla debe poder editarse.

### 🔧 Qué se hace
* **Dos ayudantes** en `nucleo/html.ts`: `escaparHtml(texto)` para el texto de un elemento o el valor de un atributo, y `argumentoJs(valor)` para el **argumento de un manejador** (genera el literal de JavaScript ya escapado para el atributo, sirve para cadenas, números y objetos y lo que recibe el método es exactamente el valor original).
* **222 interpolaciones** pasan por `escaparHtml` y **12 argumentos de manejadores** por `argumentoJs`; se eliminan los cinco escapes a mano. Los avisos, la tarjeta de error del enrutador y los diálogos escapan en su origen, así que **todo mensaje** queda cubierto aunque un caller nuevo no lo piense.
* **Lo que se ve no cambia:** el texto corriente (tildes, comas, `&`, comillas) se muestra igual.
* Las dos ventanas que reciben el objeto como JSON en texto (`abrirEdicionFormal` y `abrirEdicionLimitesTarjeta`) ahora tipan lo que leen con su tipo real en lugar de `any`: así los números dejan de contar como texto y el compilador comprueba los campos.

### 🛡️ Cómo se impide que vuelva
* **`pruebas/js/contrato/escape_html.test.js`:** el compilador de TypeScript analiza las 667 interpolaciones de las plantillas HTML (vistas y servicios): mira el **tipo** de cada una (texto libre, no un número ni un literal; **un `any` cuenta como texto**, que es justo lo que escondía el `JSON.parse`) y **dónde cae** (texto de un elemento, atributo, cadena JS dentro de un manejador, JSON en un atributo). Encuentra también las variables intermedias que una búsqueda por nombre de campo no vería. **Cero hallazgos o la prueba falla.** Una autoprueba del analizador con código de ejemplo fija que sigue viendo cada caso (y que no se queja de lo seguro).
* **`pruebas/js/ayudas/manejadores_html.js`:** ejecuta los manejadores de un HTML **como el navegador** (deshaciendo las entidades y evaluando el atributo) y devuelve los argumentos con que se llamaría al método. Las pruebas fijan el **efecto** y no la cadena: «Cuenta O'Brien» llega íntegra; uno `x'); alert(1); //` no ejecuta nada.

### 🧪 Pruebas
* 31 nuevas (netas): los ayudantes (10; el valor llega íntegro al manejador con cualquier carácter, y no puede salirse del argumento), el contrato y su autoprueba (6), los avisos y el enrutador (2) y casos hostiles en las vistas con datos en manejadores: ajustes (2), gastos (2), ingresos (2), tarjetas (3), préstamos (2), suscripciones (2) y capital (1); más dos pruebas existentes que fijaban el formato antiguo del atributo y ahora comprueban el efecto. **537 pruebas, todas pasan.**
* Mutaciones: desenvolver 40 puntos de escape al azar (39 las detecta el contrato; la que sobrevivió es el ayudante de los diálogos, que detecta su propia prueba), quitar cada carácter del escape, el escape de `argumentoJs` y el de los tres servicios: todas se detectan.

### ✅ Comprobado a mano
* **Con tus datos reales** (copia temporal, ya borrada): las once pestañas siguen **idénticas** (lo visible no cambia).
* **Con textos hostiles** puestos en todos los campos libres de las respuestas, navegando por las once pestañas y pulsando los botones que llevan datos en el manejador, antes y ahora: **antes**, todas las pestañas creaban imágenes inyectadas (de 1 a 738 por pestaña) y se ejecutó código 2 599 veces, y pulsar «editar» en suscripciones lo ejecutó; **ahora**, 0 imágenes inyectadas y 0 ejecuciones, y el nombre hostil llega íntegro al editor de la cuenta.
* **En la aplicación empaquetada, con Rust y la base reales:** una categoría y un cliente con nombre hostil guardados de verdad: 0 imágenes inyectadas, 0 ejecuciones, el texto se ve literal y el diálogo de baja abre sin ejecutar nada.

### ⚠️ Una limitación que conviene saber
* El analizador solo ve **plantillas HTML con interpolaciones**. Si alguien construye HTML concatenando cadenas (`'<td>' + x + '</td>'`) o llama a un ayudante propio que devuelve HTML sin escapar, no lo vería; hoy no hay ningún caso, y el único ayudante así (`mensajeAHtml`, en los diálogos) escapa y tiene su prueba. Una regla de revisión sigue siendo necesaria para HTML que no pase por plantillas.

---

## 🚀 Versión 1.67.0 - 2026-10-02
**Corrección: el panel de «Casos de corrección» ya no interpreta como HTML lo que escribe el titular.**

### 🐛 Qué estaba mal
* El panel pintaba el número de caso, la fecha, el tipo, la descripción, la divisa, **el motivo** y los mensajes de error con `innerHTML` **sin escapar**. El motivo lo escribe el titular y se guarda tal cual, así que un `<` se interpretaba como marcado, y un fragmento como `<img src=x onerror=…>` **ejecutaba código** en el WebView de la aplicación. Comprobado en el navegador con un caso inyectado: la versión anterior creaba las imágenes y ejecutaba el código (dos veces); la nueva no crea nada y muestra el texto tal cual.
* Quien escribe ese texto es el propio titular, en su propia aplicación, así que el riesgo real es bajo; pero un texto guardado (o importado en el futuro) no debe poder ejecutar nada, y menos en una aplicación cuyo puente con Rust permite todos los comandos.

### 🔧 Qué se hace
* Todo lo que el panel pinta de los casos y de los errores pasa por `escaparHtml` (el mismo ayudante que ya escapaba la lista de respaldos). El texto corriente se sigue viendo igual: tildes, comas, `&` y comillas.

### ⚠️ Una corrección a lo dicho antes
* Al anotarlo en la 1.63.0 se dijo que este panel era «el único sitio de la pestaña» que no escapaba. **Era inexacto**: la falta de escape es general. Las plantillas de las once vistas interpolan unos **58 campos de texto libre** (nombres de cuentas, clientes y categorías; descripciones; conceptos; notas; plataformas; entidades…) sin escapar. Este PR corrige **solo** el panel de casos. El resto **queda pendiente**: conviene hacerlo de una vez y con una prueba de contrato que impida que reaparezca (una lista de los campos de texto libre y una comprobación de que ninguno se interpola sin escapar), no sitio a sitio.

### 🧪 Pruebas
* 3 nuevas en `pruebas/js/vistas/ajustes.test.js`: el marcado del titular no se interpreta (número de caso, fecha, tipo, descripción, divisa y motivo), el texto corriente se sigue viendo igual (`Tom &amp; Jerry`, tildes, comillas angulares) y el mensaje de error también se escapa. **506 pruebas, todas pasan.** Ocho mutaciones (quitar el escape de cada campo y de los errores, y escapar dos veces): las ocho se detectan; una (la fecha) sobrevivió al principio porque el caso de prueba tenía una fecha limpia, y se reforzó.

### ✅ Comprobado a mano
* **Con tus datos reales** (copia temporal, ya borrada): el panel con tus casos se ve **idéntico** en las dos versiones. Con un caso hostil inyectado: antes, 2 imágenes creadas y 2 ejecuciones de código; ahora, 0 y 0, y el texto aparece literal.

---

## 🚀 Versión 1.66.0 - 2026-10-02
**Corrección: las cuatro bajas que no avisaban si Rust fallaba.** Baja de una suscripción, de un certificado, de una inversión de bolsa y de un bien del capital.

### 🐛 Qué estaba mal
* Estos cuatro manejadores, a diferencia de todos sus hermanos, no tenían `try/catch`. Si Rust rechazaba la operación (al **leer** el capital o al **guardarlo**, o al dar de baja la suscripción), la promesa del `onclick` rechazaba sin que nadie la atrapase: **no aparecía ningún aviso, la pantalla no se redibujaba y no había forma de saber que la baja no se había hecho**. Figuraba como hallazgo desde la 1.49.0, con pruebas marcadas `todo` que describían el comportamiento deseable.

### 🔧 Qué se hace
* Lo que va tras la confirmación entra en un `try/catch` que muestra el error, como en el resto de manejadores: **no se anuncia éxito ni se redibuja** si algo falla. Nada más cambia: ni la confirmación, ni el borrado, ni los avisos de éxito.
* **No se toca** que las bajas de certificados y de bolsa borren **por posición**, ni que el capital se lea y se guarde entero desde la vista: son decisiones de diseño anotadas en `capital_y_dinero.md`, no parte de esta corrección.

### 🧪 Pruebas
* Las cuatro pruebas `todo` pasan a pruebas normales y se amplían: cada baja del capital con el fallo **al guardar** y **al leer** (seis de interacción), y las de la vista (tres para el capital y una para la suscripción): se avisa el error, no se propaga, no se anuncia éxito, no se guarda ni se redibuja, y sin confirmar no se lee ni se guarda nada. **503 pruebas, todas pasan; ya no queda ningún `todo`.**
* Doce mutaciones (el `catch` que relanza, que calla o que anuncia éxito, en cada una de las cuatro bajas): las doce se detectan.

### ✅ Comprobado a mano
* **Con tus datos reales** (copia temporal, ya borrada), provocando el fallo de Rust en cada baja, en la versión anterior y en la nueva: **antes** el rechazo se propagaba y no salía ningún aviso en las cuatro; **ahora** no se propaga y sale «Error: …» como aviso de error, en las cuatro. Sin fallo, el camino normal es **idéntico** (mismo aviso de éxito y mismas llamadas). Las once pestañas siguen idénticas.

---

## 🚀 Versión 1.65.0 - 2026-10-02
**División de `ui.ts`, PR 12: se retira la clase `AppUI` y la división termina.** Sin cambios visibles.

### 🧩 Qué se hace
* **Lo último que quedaba en `ui.ts` pasa a su servicio**, con la misma lógica: los avisos (`crearAvisos`), el formato de importes, la petición del motivo de una corrección (`crearMotivo`), el enrutador de pestañas (`crearEnrutador`, ya sin `switch`: solo el registro de vistas, y una ruta desconocida cae en el Dashboard) y la tasa del dólar. `ui.ts` (130 líneas) se borra.
* **`componerInterfaz` (`ui/componer.ts`) sustituye a la clase** como lo que arma la aplicación: crea los servicios, registra las vistas y compone el puente `appUI` de los manejadores. Es una función para que la aplicación y las pruebas ejecuten el **mismo** cableado, y **rechaza lo que antes se pisaba en silencio**: dos vistas con la misma ruta o con un manejador del mismo nombre.
* `window.appUI` pasa de ser una clase a un objeto plano con los 70 manejadores de las once vistas, y nada más (los atributos `onclick` del HTML siguen llamándolo).
* `app.ts` (el arranque) pasa a **módulo**: recibe el enrutador, los avisos, el DOM y la API en lugar de llamar al global `appUI`; sigue colgando `navigate` de `window`. `index.html` queda con tres `<script>`: `api.js`, `ui/dom.js` y `composicion.js` (módulo).
* **TypeScript, una sola configuración estricta:** sin `ui.ts` no quedó ningún archivo con parámetros sin tipo, así que `tsconfig.estricto.json` desaparece y `tsconfig.build.json` pasa a `noImplicitAny: true`.

### 🧪 Pruebas
* 30 nuevas: `servicios_reales.test.js` (avisos con su icono y sus tiempos, formato, motivo con su mínimo, enrutador con «Cargando», ruta desconocida y tarjeta de error), `componer.test.js` (once vistas, el puente sin lo que fue de la clase, rechazo de rutas y manejadores repetidos, motivo con los avisos y diálogos de la carga) y `app.test.js` (navegar, suscripciones al arrancar, tema y menú lateral).
* Los contratos se refuerzan: **todo `appUI.x` que escribe el HTML está en el puente real compuesto**; cada pestaña tiene su vista y la dibuja ella; `index.html` solo carga tres scripts.
* El cargador de las pruebas de interacción se reescribió sobre `componerInterfaz`; las pruebas existentes pasan sin tocar salvo una. 496 pruebas (492 pasan, 4 `todo` conocidos). Treinta y dos mutaciones, las treinta y dos detectadas.

### ✅ Comprobado a mano, contra lo planificado
* **Con tus datos reales** (copia temporal, ya borrada): once pestañas idénticas y diez comprobaciones transversales en las dos versiones (menú lateral con clics reales, manejadores de las once pestañas, tema con su aviso, menú plegable, ruta desconocida, fallo de una vista y motivo corto): mismos resultados, 0 errores.
* **App empaquetada**: arranca con el Dashboard, navega por el menú, cambia el tema con su aviso, una ruta desconocida cae en el Dashboard, `window.appUI` tiene 70 manejadores y ninguno de la clase, y el diálogo de página funciona.
* Balance de la división y registro de previsto frente a medido: `division_de_ui_limpia.md` §9.

### Siguiente
* La división de `ui.ts` está terminada. Pendientes aparte, anotados sin corregir: el panel de casos de corrección pinta el motivo sin escapar (corregido en la 1.67.0, y el problema general en la 1.68.0), los importes que aún viajan como `Number` y los riesgos del capital. (Las cuatro bajas sin `try/catch` se corrigieron en la 1.66.0.)

---

## 🚀 Versión 1.64.0 - 2026-10-02
**División de `ui.ts`, PR 11: la pestaña «Tarjetas de Crédito» sale de la clase.** Sin cambios visibles. **Es la última de las once vistas.**

### 🧩 Qué se hace
* `src/js/vistas/tarjetas.ts`: la vista como clase con dependencias inyectadas (API recortada a 14 comandos, avisos, formato, enrutador, pantalla, DOM, diálogos, motivo, modales y reloj). Diecisiete métodos: el `render` con la recomendación de qué tarjeta usar hoy, el abono con su reactor y la tasa de cambio, el avance de efectivo (que se simula en el núcleo antes de confirmar), los dos historiales con su deshacer con motivo, las bonificaciones y la edición de límites y de la política de liquidación. **16 % de las líneas de sus cuerpos reescritas** (119 de 737).
* `TipoAviso` incluye `'info'`: Tarjetas avisa con él (el abono no propone importe) y el compilador señaló que el tipo se había quedado corto. Sin cambio de comportamiento.
* `ui.ts` pierde 835 líneas (965 → **130**); `registro.ts` +4. Parámetros sin tipo en `ui.ts`: 32 → 5.

### 🧪 Pruebas
* `pruebas/js/vistas/tarjetas.test.js` (40): la recomendación desde el reloj, el saldo a favor y el porcentaje de uso, las facilidades y las bonificaciones, proponer el importe del abono, el abono en efectivo y en dólares con la tasa preguntada (aceptar, cancelar, inválida), el avance (formulario por divisa y tipo, simulación, confirmación con **sus** cifras, monto y cargo fijo como **texto**), los historiales y deshacer con motivo, las bonificaciones, los límites y el puente en los dos sentidos. Los diálogos responden **tarde**, como el WebView real.
* Dos pruebas de interacción se reescribieron para comprobar el comportamiento (consulta y reabre el historial de esa tarjeta) en lugar de espiar `appUI`. 464 pruebas (460 pasan, 4 `todo` conocidos). Treinta y siete mutaciones, las treinta y siete detectadas.

### ✅ Comprobado a mano, contra lo planificado
* **Con tus datos reales** (copia temporal, ya borrada): once pestañas idénticas; **veintidós recorridos** ejercidos en las dos versiones (proponer el abono, abonos en efectivo y cruzados con la tasa, el avance completo con su simulación y su confirmación, los historiales, deshacer un abono con motivo, bonificaciones y límites): mismos comandos, argumentos y avisos, 0 errores. No se pudo ejercer deshacer un avance ni revertir una bonificación (no tienes de ninguno); las cubren las pruebas.
* **App empaquetada**: once vistas registradas, la ventana de límites en `<body>` y el historial de abonos que se abre y se pliega.
* Previsto frente a medido (≈125 → 119 líneas; esfuerzo 6 → ≈4) y los seis controles: `division_de_ui_limpia.md` §9.

### Siguiente
* PR 12, el último de la división: pasar a sus servicios lo que queda en `ui.ts` (130 líneas: `showToast`, el enrutador, `formatMoney` y `pedirMotivoDeCorreccion`), retirar la clase `AppUI` y el puente `window.appUI`.

---

## 🚀 Versión 1.63.0 - 2026-10-02
**División de `ui.ts`, PR 10: la pestaña «Ajustes» sale de la clase.** Sin cambios visibles.

### 🧩 Qué se hace
* `src/js/vistas/ajustes.ts`: la vista como clase con dependencias inyectadas (API recortada a 23 comandos, avisos, formato, enrutador, pantalla, DOM, diálogos y motivo). Dieciséis métodos: el `render`, los catálogos (categorías, clientes, cuentas y el alta de tarjetas), el editor de cuentas, el panel de correcciones con sus cuatro borrados con motivo, y los respaldos. **17 % de las líneas de sus cuerpos reescritas** (114 de 639).
* Es la vista con más diálogos y la que más se benefició de que `Dialogos` y `Motivo` sean asíncronos desde la 1.62.0: todo lo que pregunta lo **espera**.
* `nucleo/respaldos.ts` pasa a ser un módulo (`describirRespaldo` y `escaparHtml` los usa solo esta vista): sale su `<script>` de `index.html`.
* `ui.ts` pierde 720 líneas (1 678 → 965); `registro.ts` +4. Parámetros sin tipo en `ui.ts`: 46 → 32. Se retira el campo `_menuPasivoAbort`, huérfano desde `préstamos`.

### 🧪 Pruebas
* `pruebas/js/vistas/ajustes.test.js` (31): categorías del sistema, respaldos legibles y escapados, límites de filas del panel, las cuatro altas con su carga exacta (la comisión como **texto**), las bajas y las correcciones **esperando** las respuestas (que aquí llegan tarde), la edición de cuenta con sus validaciones, el panel de casos, respaldar y restaurar con su botón, y el puente en los dos sentidos.
* Las pruebas de interacción existentes pasan sin tocarlas. 424 pruebas (420 pasan, 4 `todo` conocidos). Veintitrés mutaciones, las veintitrés detectadas.

### ✅ Comprobado a mano, contra lo planificado
* **Con tus datos reales** (copia temporal, ya borrada): once pestañas idénticas; **veintiséis recorridos** ejercidos en las dos versiones con los diálogos reales (altas, edición de cuenta, bajas con y sin cancelar, panel de casos, las cuatro correcciones con motivo y con motivo corto, respaldar y restaurar): mismos comandos, argumentos y avisos, 0 errores.
* **App empaquetada**: diez vistas registradas y, con un respaldo real, la lista de respaldos sale legible.
* Previsto frente a medido (≈118 → 114 líneas; esfuerzo 6 → ≈4) y los seis controles: `division_de_ui_limpia.md` §9.
* Hallazgo sin corregir: el panel de casos de corrección pinta el motivo con `innerHTML` sin escapar (ver §9).

### Siguiente
* PR 11, el último: `tarjetas` (17 métodos, ≈837 líneas). Después se retira la clase `AppUI`.

---

## 🚀 Versión 1.62.0 - 2026-10-02
**Corrección: los borrados y correcciones ya preguntan de verdad.** Eliminar una transacción no hacía nada, y Cancelar no cancelaba.

### 🐛 Qué estaba mal
* **`prompt()` devuelve `null` al instante en el WebView de la aplicación** (Tauri 1.8 sobre `wry` 0.24, que no lo implementa en macOS): no muestra nada. Toda corrección que pedía un motivo —borrar un gasto, un ingreso, una factura, una transferencia, o corregir una factura cobrada— se abandonaba **en silencio**. Eso es lo que se veía al probar «Revertir» en Ajustes: no pasaba nada y no había aviso.
* **`confirm()` no devuelve un booleano:** Tauri lo sustituye por una versión asíncrona que devuelve una **promesa**, y una promesa es siempre «verdadera» en un `if (confirm(...))`. El diálogo de Tauri aparecía, pero la acción **ya se estaba ejecutando** sin esperar la respuesta: pulsar Cancelar no la detenía. Afectaba a toda baja con una simple confirmación (cliente, cuenta de ahorro, categoría, préstamo, suscripción, certificado, bien…).
* Comprobado en la aplicación empaquetada (`confirm` sustituido, `prompt` → `null` en 0 ms). Se escapó a todas las pruebas porque los dobles de `confirm`/`prompt` devolvían un booleano o un texto de verdad: el WebView real no.

### 🔧 Qué se hace
* **`Dialogos` y `Motivo` pasan a ser asíncronos** y la implementación real son **diálogos dentro de la página** (`crearDialogosDePagina`): no dependen de lo que haga el WebView y admiten texto, que el diálogo nativo de Tauri no ofrece. Escape cancela, Enter acepta, el mensaje se escapa (solo admite **negrita**) y en una confirmación el foco empieza en **Cancelar**, porque casi todas son borrados.
* Los 11 usos de las vistas extraídas y los 15 que quedaban en `ui.ts` usan el servicio y lo **esperan**. `ui.ts` lo recibe de `serviciosDesdeAppUI`.
* El comportamiento al responder no cambia: sin confirmar, con motivo cancelado o con motivo de menos de 15 caracteres no se envía nada.

### 🧪 Pruebas
* `pruebas/js/contrato/dialogos.test.js` (5): la interfaz **no usa** `confirm()`, `prompt()` ni `alert()` nativos; toda petición de diálogo o de motivo lleva `await`; y los borrados **esperan** una respuesta que llega tarde, como en el WebView real (no tocan la API hasta que se acepta, Cancelar no borra, un motivo corto no borra), en `ui.ts` y en una vista extraída.
* `pruebas/js/servicios/dialogos_pagina.test.js` (7): el diálogo real con un DOM de juguete (aceptar, cancelar, Escape, Enter, valor por defecto, foco, escape del mensaje, retirada al responder).
* 393 pruebas (389 pasan, 4 `todo` conocidos). Once mutaciones (confirmación sin esperar, motivo sin esperar, `await` olvidado donde el compilador no lo ve, `confirm` nativo, y siete del diálogo): las once se detectan, dos por el compilador.

### ✅ Comprobado a mano
* **En un navegador, con tus datos reales** (copia temporal, ya borrada): «Revertir» una transferencia pide la confirmación, luego el motivo (con la negrita), y al aceptar envía `eliminar_transaccion_cuenta` con el identificador y el motivo escritos; Cancelar, Escape en cualquiera de los dos diálogos y un motivo corto **no borran nada** (el corto avisa).
* **En la aplicación empaquetada:** el diálogo se abre en `<body>`, el foco empieza en Cancelar, aceptar y cancelar devuelven `true` y `false`, la pregunta devuelve el texto y parte del valor por defecto, y no queda ningún diálogo abierto.

### ⚠️ La aplicación instalada
* **La versión instalada (1.36.0) tiene los dos defectos.** Hasta actualizarla, en ella las bajas con confirmación se ejecutan aunque se pulse Cancelar, y las correcciones con motivo no hacen nada.

---

## 🚀 Versión 1.61.0 - 2026-10-02
**División de `ui.ts`, PR 9: la pestaña «Financiamientos y Deudas» sale de la clase.** Sin cambios visibles.

### 🧩 Qué se hace
* `src/js/vistas/prestamos.ts`: la vista como clase con dependencias inyectadas (API recortada a 7 comandos, avisos, formato, enrutador, pantalla, DOM, diálogos, modales, **menús**, referencias y reloj). Diecinueve métodos: el `render`, los cálculos del pasivo (`resumirPasivos`, `proximoVencimiento`, `agruparPasivosPorAcreedor`), las plantillas, el menú de acciones, el formulario con sus campos condicionales, la edición de condiciones y los manejadores de alta, conciliación, abono y baja. **21 % de las líneas de sus cuerpos reescritas** (125 de 569).
* **Nuevo servicio `MenuFlotante`** (`abrir` y `cerrar`): el menú de acciones de cada fila usaba `document`, `window`, `setTimeout` y un `AbortController` guardado en la clase. El estado pasa al servicio, que solo mantiene un menú abierto y retira sus listeners en cada cierre.
* La tasa del dólar entra por `Referencias` y «hoy» por el reloj: los cálculos de dinero de la pestaña se pueden probar sin dibujar.
* `ui.ts` pierde 706 líneas (2 384 → 1 678); `servicios.ts` +76; `registro.ts` +4. Parámetros sin tipo en `ui.ts`: 106 → 46.
* Dos guardas de contrato se adaptaron al tránsito de la división sin relajarlas: `ipc.test.js` cuenta también los comandos que las vistas declaran en `ApiDe<…>`, y el servicio guarda una referencia al menú en lugar de buscarlo con `document.getElementById` (reservado a `ui/dom.ts`).

### 🧪 Pruebas
* `pruebas/js/vistas/prestamos.test.js` (29): total, carga mensual, cupo y composición con cifras de mano y la tasa inyectada; vencimiento por distancia en días con el reloj; agrupación de facilidades en su tarjeta; campos condicionales; alta (consumo y línea, saldo en blanco frente a cero), edición, conciliación como texto, abono y baja; el menú y sus cuatro acciones; el puente.
* `pruebas/js/servicios/menu_flotante.test.js` (11): el servicio real con un DOM de juguete (uno a la vez, clic diferido, Escape, listeners retirados, colocación y volteo, acción tras cerrar).
* Las 16 pruebas de interacción de `prestamos` pasan sin tocarlas. 381 pruebas (377 pasan, 4 `todo` conocidos). Veintinueve mutaciones, las veintinueve detectadas.

### ✅ Comprobado a mano, contra lo planificado
* **Con tus datos reales** (copia temporal, ya borrada): once pestañas idénticas; el **menú real** (apertura, posición, estilo, Escape, clic fuera, uno a la vez), las cuatro acciones, la edición y las dos altas, ejercidos en las dos versiones: mismos comandos, argumentos, modal y avisos, 0 errores. El grupo de una tarjeta con facilidades no se pudo ejercer (tus préstamos no cuelgan de ninguna); lo cubren las pruebas.
* **App empaquetada**: nueve vistas registradas, el menú real en `<body>`, Escape lo cierra y elegir «editar» abre el modal.
* Previsto frente a medido (≈89 → 125 líneas por el menú, que el agente no contó; esfuerzo 4 → ≈5) y los seis controles: `division_de_ui_limpia.md` §9.

### Siguiente
* PR 10: `ajustes` (16 métodos, ≈736 líneas).

---

## 🚀 Versión 1.60.0 - 2026-10-02
**Corrección: la «Carga Fija Mensual» del Resumen ya no mezcla divisas.**

### 🐛 Qué estaba mal
* La tarjeta sumaba **todas** las suscripciones sin mirar su `divisa`: 10 USD contaban como 10 DOP. Con 10 de 11 suscripciones en dólares, la carga fija salía muy por debajo de la real, rotulada «DOP». Venía de antes de la división de `ui.ts` (las dos versiones pintaban lo mismo, por eso la comparación no lo detectó) y la prueba que la fijaba usaba solo pesos.

### 🔧 Qué se hace
* Las suscripciones se suman **por divisa, sin convertir**: no hay una tasa fiable para una carga que se cobra en dólares, y una cifra convertida con una constante no corresponde con lo que se paga. El total de la cabecera es el de pesos (cuotas de préstamos —que no tienen divisa— más suscripciones en DOP) y las otras divisas se muestran aparte: «DOP 45 + USD 20», con su propio renglón «Suscripciones en USD». Una divisa que no sea DOP ni USD tampoco se pierde ni se suma a los pesos; una suscripción sin divisa cuenta como pesos.
* Solo la carga fija. No cambia el pasivo de tarjetas (que sí convierte con la tasa de referencia) ni ninguna otra cifra.

### 🧪 Pruebas
* 4 nuevas en `pruebas/js/vistas/resumen.test.js`: los dólares no entran en la suma de pesos, otra tasa no cambia ninguna cifra de la tarjeta, sin dólares no aparece renglón de otra divisa, y una tercera divisa se muestra aparte. 318 pruebas (314 pasan, 4 `todo` conocidos). Cuatro variantes erróneas (suma sin mirar la divisa, conversión con la tasa, perder otras divisas, total con dólares sumados): las cuatro se detectan.

### ✅ Comprobado a mano
* **Con tus datos reales** (copia temporal, ya borrada): de las once pestañas, solo cambia el Resumen y solo en esa tarjeta; las otras diez, idénticas, sin errores. Las cifras de pesos y de dólares coinciden con la suma por divisa de tus suscripciones.

---

## 🚀 Versión 1.59.0 - 2026-10-02
**División de `ui.ts`, PR 8: la pestaña «Ingresos» sale de la clase.** Sin cambios visibles.

### 🧩 Qué se hace
* `src/js/vistas/ingresos.ts`: la vista como clase con dependencias inyectadas (API recortada a 9 comandos, avisos, formato, enrutador, pantalla, DOM, **diálogos**, **motivo**, **modales** y reloj). Once métodos: el `render`, las dos altas, la elección de cliente, la edición de una factura con su cobro parcial y el cobro de facturas y de informales. **20 % de las líneas de sus cuerpos reescritas** (96 de 479).
* El modal de edición lleva `cobrada` y `recibido` en su `dataset`: como `Modales.abrir` no devuelve el elemento, la vista lo busca por su identificador justo después de abrirlo. El comportamiento es el mismo y una prueba lo fija.
* `ui.ts` pierde 521 líneas; `registro.ts` +4. Parámetros sin tipo en `ui.ts`: 121 → 106.

### 🧪 Pruebas
* `pruebas/js/vistas/ingresos.test.js` (23): factura siguiente y fecha de hoy desde el reloj, qué facturas ofrecen «cobrar», las dos altas con su carga exacta, elegir cliente, las tres ventanas y sus envíos, la edición de una factura cobrada (motivo con el ajuste, confirmación con las cifras cuando no cambia saldo, cobro parcial como **texto** y sus tres rechazos) y el puente.
* Las 27 pruebas de interacción del archivo de `ingresos` (21 de esta pestaña, 6 de los borrados con motivo de Ajustes) pasan sin tocarlas.
* 337 pruebas (333 pasan, 4 `todo` conocidos). Dieciséis mutaciones, las dieciséis detectadas.

### ✅ Comprobado a mano, contra lo planificado
* **Con tus datos reales** (copia temporal, ya borrada): once pestañas idénticas; las dos altas, elegir cliente y la edición de una factura cobrada (mueve saldo y pide motivo) ejercidas en las dos versiones: mismos comandos, argumentos, modal y avisos, 0 errores. El cobro de una factura y el de un informal no se pudieron ejercer (no tienes facturas emitidas ni informales pendientes); los cubren las pruebas.
* **App empaquetada**: ocho vistas registradas, se dibuja sin error y los modales reales se crean en `<body>`.
* Previsto frente a medido (≈93 → 96 líneas; esfuerzo 4 → ≈4) y los seis controles: `division_de_ui_limpia.md` §9.

### Siguiente
* PR 9: `préstamos` (20 métodos, ≈705 líneas).

---

## 🚀 Versión 1.58.0 - 2026-10-02
**División de `ui.ts`, PR 7: la pestaña «Gastos» sale de la clase.** Sin cambios visibles.

### 🧩 Qué se hace
* `src/js/vistas/gastos.ts`: la vista como clase con dependencias inyectadas (API recortada a 6 comandos, avisos, formato, enrutador, pantalla, DOM, **modales** y reloj). Nueve métodos: el `render`, seis manejadores (selector de mes, método de pago, conversión, alta, liquidación y su previsualización) y un ayudante de fechas. **15 % de las líneas de sus cuerpos reescritas** (67 de 439).
* **Estado propio:** el mes elegido (`selectedGastosMonth`) deja de ser un campo de `AppUI` y es privado de la vista. El mes por defecto sale del reloj inyectado.
* `ui.ts` pierde 484 líneas; `registro.ts` +4. Parámetros sin tipo en `ui.ts`: 134 → 121.

### 🧪 Pruebas
* `pruebas/js/vistas/gastos.test.js` (19): mes por defecto y elegido, totales por divisa sin mezclar, visibilidad del método de pago, previsualización de la conversión, las tres altas con su carga exacta (los selectores ocultos no viajan), fallo de la API, liquidación (modal, envío y tasa) y el puente.
* La prueba de interacción de elegir mes se reescribió para comprobar el comportamiento y no el campo privado.
* 314 pruebas (310 pasan, 4 `todo` conocidos). Ocho mutaciones detectadas más una equivalente.

### ✅ Comprobado a mano, contra lo planificado
* **Con tus datos reales** (copia temporal, ya borrada): once pestañas idénticas; selector de mes, visibilidad del método, conversión y **las tres altas** ejercidos en las dos versiones: mismos argumentos y avisos, 0 errores. La liquidación de un consumo no se pudo ejercer (no tienes consumos pendientes de conversión); la cubren las pruebas.
* **App empaquetada**: siete vistas registradas, se dibuja sin error y el modal real se crea en `<body>`.
* Previsto frente a medido (≈66 → 67 líneas; esfuerzo 4 → ≈4) y los seis controles: `division_de_ui_limpia.md` §9.

### Siguiente
* PR 8: `ingresos` (11 métodos, ≈519 líneas).

---

## 🚀 Versión 1.57.0 - 2026-10-02
**División de `ui.ts`, PR 6: la pestaña «Suscripciones» sale de la clase.** Sin cambios visibles.

### 🧩 Qué se hace
* `src/js/vistas/suscripciones.ts`: la vista como clase con dependencias inyectadas (API recortada a 8 comandos, avisos, formato, enrutador, pantalla, DOM, **diálogos**, **motivo** y **modales**). Diez métodos: el `render`, seis manejadores, el editor modal y dos ayudantes de fechas, ahora privados de la vista. **18 % de las líneas de sus cuerpos reescritas** (59 de 317).
* **Nuevo servicio `Modales`:** `ui.ts` repite nueve veces el bloque que crea la capa `modal-overlay`; el editor de suscripciones es el primero en moverse y no arrastra `document` a la vista. Las ocho extracciones que quedan con modal ya tienen su servicio.
* `ui.ts` pierde 369 líneas; `registro.ts` +4; `servicios.ts` +23. Parámetros sin tipo en `ui.ts`: 146 → 134.

### 🧪 Pruebas
* `pruebas/js/vistas/suscripciones.test.js` (18): el aviso «Cobro próximo» muestra la fecha (lo que la 1.43.0 corrigió), períodos pendientes, suscripciones paradas, alta con el monto como **texto** y la fecha convertida de ISO a dd/mm/aaaa, el modal con su identificador y su tarjeta elegida, la edición con sus validaciones y el cierre de la ventana, baja con confirmación, corregir fecha, asentar y descartar con motivo.
* `cargar_interfaz.js` registra cada ventana modal abierta y la declara en el DOM falso.
* 295 pruebas (291 pasan, 4 `todo` conocidos). Ocho mutaciones, las ocho detectadas.

### ✅ Comprobado a mano, contra lo planificado
* **Con tus datos reales** (copia temporal, ya borrada): once pestañas idénticas; alta, **editor modal**, edición y baja ejercidos en las dos versiones: mismos comandos y argumentos, mismo modal, cerrado tras guardar, mismos avisos, 0 errores. Corregir fecha y asentar/descartar no se pudieron ejercer porque tus suscripciones no tienen ninguna parada ni períodos pendientes; las cubren las pruebas.
* **App empaquetada**: seis vistas registradas, y el modal real se crea en `<body>` con la clase `modal-overlay`.
* Previsto frente a medido (≈59 → 59 líneas; esfuerzo 3 → ≈3) y los seis controles del plan: `division_de_ui_limpia.md` §9. Incluye una trampa de la transformación mecánica: el `elemento(` que vive **dentro de un atributo `onclick` del HTML** no debe reescribirse.

### Siguiente
* PR 7: `gastos` (9 métodos, 480 líneas).

---

## 🚀 Versión 1.56.0 - 2026-10-01
**División de `ui.ts`, PR 5: la pestaña «Capital» sale de la clase.** Sin cambios visibles.

### 🧩 Qué se hace
* `src/js/vistas/capital.ts`: la vista como clase con dependencias inyectadas (API recortada a `obtenerCapital` y `guardarCapital`, avisos, formato, enrutador, pantalla, DOM, **diálogos** y reloj). Siete métodos: el `render` y seis manejadores (alta y baja de certificados, de inversiones en bolsa y de bienes). **21 % de las líneas de sus cuerpos reescritas** (58 de 272).
* Usa por primera vez el servicio **`Dialogos`** (las confirmaciones de las bajas) y un **reloj inyectado** para el identificador de un bien nuevo, que antes salía de `Date.now()`. El capital es JSON libre: la vista declara solo lo que lee (`Certificado`, `InversionDeBolsa`, `Bien`).
* `ui.ts` pierde 297 líneas; `registro.ts` +4. Parámetros sin tipo en `ui.ts`: 161 → 146.

### 💶 `capital_y_dinero.md`: la relación del capital con el dinero
* Rust es el único sitio que decide dinero (valida con `Dinero`, suma en centavos y devuelve los `totales`); la vista solo transporta texto y formatea. **No hay nada de `Dinero` que integrar en la interfaz**, y `nucleo/dinero.ts` no lo usa ninguna vista. El capital no mueve saldos ni es transaccional con las cuentas: es un inventario en un archivo JSON.
* Hallazgos, **sin corregir**: lectura-modificación-escritura del documento entero desde la vista («gana la última escritura»); certificados y bolsa se borran **por posición** y los bienes por identificador; las tres bajas no tienen `try/catch` (ya figuraba en 1.49.0). El documento explica la salida de fondo (comandos propios en Rust con identificadores) y por qué conviene **después** de dividir `ui.ts`.

### 🧪 Pruebas
* `pruebas/js/vistas/capital.test.js` (14): fija **qué documento se guarda**: el importe sale como **texto** tal cual se escribió y la tasa como número, el resto del documento viaja intacto, la baja quita el elemento de ese índice (o el bien de ese identificador), el identificador sale del reloj inyectado, y la pantalla no suma ni muestra totales.
* 277 pruebas (273 pasan, 4 `todo` conocidos). Siete mutaciones, las siete detectadas.

### ✅ Comprobado a mano, contra lo planificado
* **Con tus datos reales** (copia temporal, ya borrada): once pestañas idénticas; **cinco de los seis formularios, ejercidos en las dos versiones sobre tu documento, guardan lo mismo** (comparado sin imprimirlo), con los mismos avisos y sin errores. La baja de certificado no se pudo ejercer porque tu capital no tiene certificados.
* **App empaquetada**: cinco vistas registradas, se dibuja sin error y los seis manejadores existen.
* Previsto frente a medido (≈55 → 58 líneas; esfuerzo 2 → ≈2) y los seis controles del plan: `division_de_ui_limpia.md` §9.

### Siguiente
* PR 6: `suscripciones` (10 métodos, 361 líneas).

---

## 🚀 Versión 1.55.0 - 2026-10-01
**División de `ui.ts`, PR 4: la pestaña «Dashboard» sale de la clase.** Sin cambios visibles.

### 🧩 Qué se hace
* `src/js/vistas/dashboard.ts`: la vista como clase con dependencias inyectadas (seis consultas a la API, formato, pantalla y reloj). Solo lectura y sin manejadores de `appUI`: sus dos enlaces son `navigate()`, globales de `app.ts`. **10 % de las líneas de su cuerpo reescritas** (17 de 157). Declara solo lo que lee del capital (`ElementoConAlerta`), que Rust devuelve como JSON libre.
* **El `default:` de `render()` era `renderDashboard()`:** una ruta que nadie reconoce cae en el Dashboard. Ahora busca la vista registrada (`this.vistas.get('dashboard')?.render()`); sin eso, quitar el método habría roto esa rama. Es el segundo acoplamiento que el `switch` esconde y que el análisis por métodos no ve.
* `ui.ts` pierde 165 líneas y su `case` (+2 del `default:`); `registro.ts` +4. Parámetros sin tipo en `ui.ts`: 163 → 161.

### 🧪 Pruebas
* `pruebas/js/vistas/dashboard.test.js` (10): lee solo los datos del mes del reloj inyectado, suma los cargos al gasto, usa el patrimonio del núcleo sin recalcularlo, genera los avisos de tarjetas, certificados, bolsa y préstamos **cada uno con su nivel**, limita a tres las tarjetas y los préstamos, y dibuja los mensajes vacíos.
* `rutas.test.js` suma la prueba de la **ruta desconocida con el render real**: cae en el Dashboard.
* 263 pruebas (259 pasan, 4 `todo` conocidos). Seis mutaciones, las seis detectadas; **una escapó a la primera versión de las pruebas** (el nivel del aviso de pago de tarjeta) y se reforzó la prueba.

### ✅ Comprobado a mano, contra lo planificado
* **Con tus datos reales** (copia temporal, ya borrada): once pestañas idénticas; los dos enlaces del Dashboard, ejercidos en las dos versiones, llevan al mismo título y la misma pestaña activa; y una ruta inexistente cae en el Dashboard con el mismo texto.
* **App empaquetada**: cuatro vistas registradas, la ruta inexistente cae en el Dashboard sin error.
* Previsto frente a medido (14-16 → 17 líneas; esfuerzo 0,6 → ≈0,9) y los seis controles del plan: `division_de_ui_limpia.md` §9.

### Siguiente
* PR 5: `capital` (7 métodos, 294 líneas; tres diálogos y las llamadas a la API que guardan el capital).

---

## 🚀 Versión 1.54.0 - 2026-10-01
**División de `ui.ts`, PR 3: la pestaña «Resumen» sale de la clase.** Sin cambios visibles.

### 🧩 Qué se hace
* `src/js/vistas/resumen.ts`: la vista como clase con dependencias inyectadas (siete consultas a la API, formato, pantalla, reloj y la nueva `Referencias`). Es de solo lectura y sin manejadores, así que su puente es vacío. **15 % de las líneas de su cuerpo reescritas** (25 de 164).
* **Nuevo servicio `Referencias`** para la tasa de presentación `TASA_USD_A_DOP`: otros dos métodos de `ui.ts` aún la leen, y moverla a `nucleo/` habría obligado a duplicarla o a que un script clásico importase un módulo. La constante sigue en `ui.ts` y la clase la expone (`appUI.tasaUsdADop`); la vista la recibe inyectada. Una prueba verifica que usa la inyectada y no una escrita dentro.
* `ui.ts` pierde 171 líneas y su `case` (+7 por la tasa); `registro.ts` +4; `servicios.ts` +13.

### 🧪 Pruebas
* `pruebas/js/vistas/resumen.test.js` (10): el Resumen es de solo lectura pero **contiene reglas de dinero que ninguna prueba ejercía**: patrimonio neto, ratio de endeudamiento con sus tres rótulos (Saludable / Moderado / Alto Riesgo), qué cuotas de préstamo cuentan (flexibles y con cuotas pendientes), suscripciones anuales entre 12, y el balance del mes con un reloj inyectado. También: sin activos no divide por cero, balance negativo, capital `null`.
* **`pruebas/js/contrato/rutas.test.js`, nueva y general:** `render()` termina en `default: renderDashboard()`, así que una vista extraída que **deja de registrarse no da error, pinta el Dashboard**. La prueba exige que cada pestaña del menú se dibuje desde un solo sitio (vista registrada o `case`) y que ninguna vista registrada tenga una ruta que el menú no ofrezca. Protege también todas las extracciones siguientes.
* 254 pruebas (250 pasan, 4 `todo` conocidos). Seis mutaciones, las seis detectadas (la de «vista sin registrar», solo por la prueba nueva).

### ✅ Comprobado a mano, contra lo planificado
* **Con tus datos reales** (copia temporal, ya borrada): once pestañas idénticas; y como el Resumen **calcula**, se comparó además **cada cifra que muestra**: 17 cifras, las mismas en el mismo orden en las dos versiones, y el mismo texto tras repintar y volver de otra pestaña.
* **App empaquetada**: tres vistas registradas, se dibuja sin error y la tasa llega como 60.
* Previsto frente a medido (23-26 → 25 líneas; esfuerzo ≈1 → ≈1,3) y los seis controles del plan: `division_de_ui_limpia.md` §9.

### Siguiente
* PR 4: `dashboard` (161 líneas, solo lectura).

---

## 🚀 Versión 1.53.0 - 2026-10-01
**División de `ui.ts`, PR 2: la pestaña «Cuentas de Ahorro» sale de la clase.** Sin cambios visibles.

### 🧩 Qué se hace
* `src/js/vistas/cuentas.ts`: la vista como clase con dependencias inyectadas (API recortada a 3 comandos, avisos, formato, enrutador, pantalla, DOM y reloj). Son tres métodos: el `render`, el reactor que rotula las divisas y avisa cuando una transferencia cruza divisas, y el manejador del formulario. **13 % de las líneas de sus cuerpos reescritas** (24 de 173), casi todo sustitución textual que el compilador verifica.
* `registro.ts` suma la vista (+4); `ui.ts` pierde 196 líneas y su `case`. Los `onsubmit`/`onchange` no cambian: el puente cuelga los manejadores de `appUI`.
* **Ninguna prueba existente hubo que tocar**: el helper de las pruebas de interacción y las de contrato ya valían tras el PR 1. Parámetros sin tipo en `ui.ts`: 167 → 163.

### 🧪 Pruebas
* `pruebas/js/vistas/cuentas.test.js` (10): dibuja con el formato y el reloj inyectados, historial y mensaje vacío, transferir (argumentos y orden de los tres importes; error de la API sin redibujar), el reactor de divisas (cruce, misma divisa, sin selección, sin recuadro de aviso) y el puente.
* 241 pruebas (237 pasan, 4 `todo` conocidos). Cinco mutaciones: detectadas (dos por el compilador).

### ✅ Comprobado a mano, contra lo planificado
* **Con tus datos reales** (copia temporal, ya borrada): once pestañas idénticas; **el reactor de divisas y la transferencia, ejercidos en las dos versiones, dejan el mismo estado del DOM y envían los mismos comandos con los mismos argumentos y el mismo aviso**, 0 errores.
* **App empaquetada**: dos vistas registradas, se dibuja, formulario y manejadores presentes.
* Lo previsto frente a lo medido (24 frente a ≈25 líneas, esfuerzo ≈1) y los seis controles del plan: `division_de_ui_limpia.md` §9. Una discrepancia anotada: el modelo atribuía a `cuentas` un manejador que en realidad es de Ajustes (`abrirEdicionCuenta`).

### Siguiente
* PR 3: `resumen` (168 líneas, solo lectura).

---

## 🚀 Versión 1.52.0 - 2026-10-01
**División de `ui.ts`, PR 1: la pestaña «Caja y Efectivo» sale de la clase.** Sin cambios visibles.

### 🧩 Qué se hace
* `src/js/vistas/efectivo.ts`: la vista como **clase con sus dependencias inyectadas** (API recortada a 3 comandos, avisos, formato, enrutador, pantalla, DOM y reloj). Sin `this` compartido, sin `AppAPI`, `elemento()` ni `appUI` globales. Los cuerpos son los de `ui.ts`; **17 % de sus líneas se reescribieron** (24 de 134), casi todo sustitución textual que el compilador verifica.
* `src/js/vistas/registro.ts`: construye y registra cada vista. **Lo ejecutan tanto la aplicación (`composicion.ts`) como las pruebas**, así que no hay una copia del cableado que pueda desviarse.
* `ui.ts` pierde `renderEfectivo` y los dos manejadores (−147 líneas) y su `case` del `switch`. Los `onsubmit="appUI.…"` no cambian: el puente cuelga los manejadores de `appUI`.
* La vista entra en `tsconfig.estricto.json`: nace con `noImplicitAny`. Parámetros sin tipo en `ui.ts`: 169 → 167.

### 🧪 Pruebas
* `pruebas/js/vistas/efectivo.test.js` (8): la vista con API, avisos, DOM y reloj falsos, algo que el diseño mecánico no permitía sin navegador.
* `cargar_interfaz.js` registra las vistas con la misma `registrarVistas` de la aplicación: **no hizo falta `import()`** y ninguna prueba de interacción cambió. `importes.test.js` acepta `api.x(` además de `AppAPI.x(`.
* 231 pruebas (227 pasan, 4 `todo` conocidos). Cuatro mutaciones (retiro siempre a efectivo en pesos, vista sin registrar, puente sin un manejador, monto sin convertir): detectadas por las pruebas o por el compilador.

### ✅ Comprobado a mano, contra lo planificado
* **Con tus datos reales** (copia temporal, ya borrada): las **once pestañas idénticas** a `main`, y **los dos formularios ejercidos en las dos versiones envían los mismos comandos con los mismos argumentos y muestran los mismos avisos**, sin errores.
* **En la aplicación empaquetada**: la vista queda registrada, se dibuja y su manejador existe.
* Tabla completa de lo previsto frente a lo medido, y de los seis controles del plan: `division_de_ui_limpia.md` §9.

### 🛠️ Herramienta
* `herramientas/comparar_vistas/mock.js` registra ahora cada llamada a un comando (`window.__llamadas`), para comparar **lo que dos versiones envían a Rust**, no solo lo que pintan.

### Siguiente
* PR 2: `cuentas` (183 líneas).

---

## 🚀 Versión 1.51.0 - 2026-10-01
**División de `ui.ts`, PR 0: la infraestructura del diseño limpio.** Sin mover ninguna vista; sin cambios visibles.

### 🏗️ Qué se añade
* `src/js/ui/servicios.ts`: las interfaces que reciben las vistas por inyección (`Avisos`, `Formato`, `Enrutador`, `Pantalla`, `Dom`, `Reloj`, `Dialogos`, `Motivo` y la API recortada `ApiDe<K>`) y su cableado con la clase vieja. `Dialogos` y `Motivo` entran ya porque `confirm`/`prompt` (25 usos) impedirían probar los borrados en Node.
* `src/js/composicion.ts`: la raíz de composición, como módulo ES. Todavía no registra ninguna vista; construye los servicios.
* `AppUI.registrarVista(ruta, vista, puente)` y un registro de vistas que `render()` consulta antes del `switch`: cada PR siguiente saca una vista de la clase y la registra aquí. Los manejadores en línea siguen llamando a `appUI`.
* `index.html` carga `composicion.js` como `<script type="module">`; los dos archivos nuevos entran en `tsconfig.estricto.json` (`noImplicitAny`).
* `division_de_ui_limpia.md` y `herramientas/medir_division_ui.mjs` (obra de un agente independiente, verificada): el modelo del diseño B, con la medición por vista; el titular lo eligió frente al mecánico.

### 🧪 Pruebas
* **Helper compartido** `pruebas/js/ayudas/fuentes_interfaz.js`: las pruebas de contrato (`importes`, `tipos`, `ipc`, `manejadores`) piden «la interfaz» y no `ui.ts`, así que siguen valiendo cuando las vistas se repartan en `vistas/`.
* **Hueco cerrado:** `manejadores.test.js` no veía los tres `onclick='…'` con comillas simples (`abrirEdicionSuscripcion`, `abrirMenuPasivo`, `abrirEdicionCuenta`, que reciben un objeto en JSON). Ahora los ve: renombrar `abrirMenuPasivo` lo detecta, y antes no.
* 223 pruebas de JavaScript (219 pasan, 4 `todo` conocidos) y la comprobación de tipos.

### ✅ Comprobado como se prometió en el plan
* **Con tus datos reales** (copia temporal, ya borrada): las **once pestañas idénticas** a `main`, sin `undefined`/`NaN`, sin errores de consola y con el módulo cargado y 0 vistas registradas.
* **En la aplicación empaquetada** (construida con `tauri build`, `HOME` aislado, con una marca temporal ya retirada): el módulo se ejecuta con `document.readyState === 'interactive'`, **antes de `DOMContentLoaded`**, y ya ve `appUI` y `AppAPI`. Era la comprobación que el análisis había dejado abierta.
* Un detalle de la medición: `appUI` es una `const` de un script clásico, así que **no cuelga de `window`**: los `onclick` lo resuelven, pero código externo debe usar `eval` o inyectarlo. Se tiene en cuenta en las pruebas.

### Siguiente
* PR 1: `efectivo` (la vista más pequeña) con sus pruebas de Node, y `cargar_interfaz.js` adaptado a módulos.

---

## 🚀 Versión 1.50.0 - 2026-10-01
**El alta de cuenta envía la comisión como texto, como acordó la convención de 1.21.0.**

### 🐞 Qué faltaba
* 1.21.0 acordó que los importes viajan como **texto**, tal cual se escribieron, y dio por migrada «la comisión por pago de impuestos». Era cierto para **editar** una cuenta, pero el **alta** seguía convirtiéndola con `Number(...)`: la inconsistencia la señaló una de las pruebas de interacción de 1.49.0 (`crearCuenta` la mandaba como número y `abrirEdicionCuenta` como texto).
* Con dos decimales no cambia ningún importe (la medida de 1.21.0 y el `step="0.01"` lo garantizan); es coherencia con la convención y una excepción menos que retirar el día que se elimine el número de `ImporteDecimal`.

### ✅ Qué se hace
* `handleAgregarCuenta` envía la comisión recortada de espacios y **sin convertir**; `«0»` sigue siendo una tarifa gratuita, no «sin comisión». La prueba de interacción fija el texto (`'12.34'`, `'0'`, `'7.5'` desde `'  7.5  '`) y falla si se vuelve a enviar un número.
* **Corrección a `politica_redondeo.md`:** la decisión del tramo 4c (1.41.0) decía que la conversión a texto solo se retomaría ante una vía nueva sin formulario. Eso **contradecía la convención vigente**: la conversión de los parámetros que aún viajan como número sigue pendiente, un comando por PR; la opción 1 es solo la salvaguarda mientras tanto.

### 📌 Pendiente de la convención
* Siguen enviándose como número los parámetros de los demás flujos de dinero (gastos, ingresos, tarjetas, abonos, transferencias, préstamos…). Es trabajo mecánico y acotado por `ipc.test.js` y las pruebas de interacción, que ahora detectarían cada cambio.

---

## 🚀 Versión 1.49.0 - 2026-10-01
**Pruebas de interacción en Node: por fin hay una red de seguridad de lo que ocurre al pulsar.** Sin cambios en la aplicación.

### 🧪 Qué se añade (`pruebas/js/interaccion/`, `pruebas/js/ayudas/`)
* Los manejadores de `ui` se ejecutan en Node con un **DOM falso mínimo** y una **API falsa**, y se comprueba **qué se envía a Rust** (comando y argumentos) y qué avisos salen. Sin dependencias nuevas.
* **Los 49 `handle*` están cubiertos** y `cobertura.test.js` lo exige: si aparece uno sin prueba ni declaración, falla. Cada manejador tiene camino feliz, validación de cliente donde existe y «Rust rechaza». Además: los reactores (`aplicarTipoAbono`, `previsualizarTasa`, `actualizarConversionGasto`, `alternarCobroParcial`) y `abrirEdicionCuenta`.
* 223 pruebas de JavaScript en total (219 pasan, 4 `todo` que describen defectos reales, 0 fallan); 181 son de interacción.
* **Diseñado para sobrevivir a la división de `ui.ts`:** las pruebas llaman siempre a `appUI.<método>`, como los `onclick`, y el único punto de carga es `cargar_interfaz.js`: tras dividir solo hay que adaptar ese archivo.
* Un id que el manejador lee y la prueba no declaró, o un `confirm`/`prompt` sin configurar, **falla nombrándolo**, aunque el manejador lo capture en su `try/catch`; el doble de `AppAPI` solo tiene los métodos reales, así que un nombre mal escrito falla.

### ✅ Comprobado
* El agente que las escribió probó 17 mutaciones sobre `ui.ts` y las 17 fueron detectadas; yo repetí tres más por mi cuenta (restaurar con la etiqueta en vez del nombre, editar una suscripción sin validar el monto, intercambiar dos argumentos de `crearCuenta`): detectadas, esta última ya por el compilador gracias a los tipos de `api.ts`. `ui.ts` queda intacto.
* Límites declarados en el README: sin CSS ni layout, sin validación nativa del navegador (`required`, `step`), `innerHTML` es una cadena, y no se prueba que los atributos `onclick` interpolen bien los argumentos.

### 🔎 Hallazgos (sin corregir; pruebas `todo`)
* `handleEliminarSuscripcion`, `handleEliminarCertificado`, `handleEliminarBolsa` y `handleEliminarPropiedad` **no tienen `try/catch`**: si Rust rechaza, la promesa del `onclick` rechaza sin aviso y la pantalla no se redibuja. Las pruebas `todo` describen el comportamiento deseable y pasarán solas al corregirlo. Se corrige en un PR aparte.
* **Pregunta de dominio:** `crearCuenta` envía la comisión de pago de impuestos como número y `abrirEdicionCuenta` como texto. Se fijó tal cual está.

---

## 🚀 Versión 1.48.0 - 2026-10-01
**Preparación de la división de `ui.ts` y revisión de la estabilidad de actualizar.** Sin cambios en la aplicación.

### 🧭 División de `ui.ts` (plan en `division_de_ui.md`)
* Medido con el grafo de llamadas: **98 de los 103 métodos pertenecen a una sola pestaña**; solo 4 son compartidos (89 líneas). No hay métodos pasados como valor ni `bind`/`call`/`apply`: ningún `this` se puede perder al mover un método.
* Diseño propuesto: cada vista es un módulo con un objeto de métodos, mezclado en `appUI` (el mismo `this`), de modo que se mueven sin cambiar una línea de su cuerpo y los 99 manejadores en línea siguen funcionando. 13 PR, de la vista más pequeña a la más grande. **Espera tu visto bueno.**
* `pruebas/js/contrato/manejadores.test.js`: todo `appUI.x()` de un `onclick` existe como método, `navigate` es global y los manejadores solo llaman a destinos globales declarados. Tres mutaciones, las tres detectadas.
* `herramientas/comparar_vistas/`: pinta las once pestañas con **tus datos reales** (sobre una copia temporal, solo lee la base viva) con dos versiones del frontend y compara el texto, devolviendo solo un resumen con los dígitos enmascarados. Incluye una prueba `#[ignore]` que vuelca lo que leen las vistas.

### 🔍 Estabilidad de actualizar (detalle en `guia_de_actualizacion.md`)
* Veredicto: **estable para actualizar**. De 1.36.0 a hoy, 13 PR y solo 221 líneas de Rust (sin pruebas); **sin migraciones**: el esquema sigue en 15, así que se puede volver a la 1.36.0 sin tocar la base.
* Con datos reales: el arranque no cambia ninguna fila, los 16 comandos de lectura funcionan, el cobro automático es idempotente, respaldar y restaurar funcionan, la aplicación empaquetada arranca contra la copia y las once pestañas se pintan **idénticas** a la versión instalada (salvo el panel nuevo de restaurar), sin `undefined`, `NaN` ni errores.
* Riesgo residual declarado: se comprobó lo que se **pinta**, no cada formulario con la aplicación empaquetada.

---

## 🚀 Versión 1.47.0 - 2026-10-01
**Los 61 envoltorios de `api.ts` tienen todos sus parámetros tipados, y `noImplicitAny` los vigila.** Sin cambios de comportamiento.

### 🧩 Qué se hace
* Cada parámetro de `api.ts` lleva su tipo (134 errores de `noImplicitAny` → 0). **Se derivaron, no se escribieron a mano**: del mapa `Comandos` generado desde Rust, según cómo el envoltorio pasa cada parámetro (`Number(x)` → `number | string`, texto de importe → `string | number`, sin conversión → el tipo exacto de Rust, estructuras de entrada → el tipo generado). Cuatro se resolvieron a mano; uno queda como `any` explícito y comentado: el documento de capital, que Rust recibe como JSON libre.
* `tsconfig.estricto.json` aplica `noImplicitAny` a `api.ts`, `ui/dom.ts`, `nucleo/` y `tipos-ipc.d.ts`, e integra `npm run compilar` y `npm run tipos`: una violación detiene también `tauri build`.

### ✅ Comprobado
* El JavaScript compilado de `api` es **idéntico byte a byte** al de la versión anterior: los tipos no cambian el código.
* Ninguna llamada de `ui.ts` incumplía los tipos nuevos. Tres mutaciones, las tres detectadas: un parámetro sin tipo; un tipo que la interfaz no cumple (el error sale **en `ui.ts`**, en la llamada); un envoltorio nuevo sin tipos.
* 38 pruebas de JavaScript y la comprobación de tipos.

### Siguiente
* Los 169 parámetros sin tipo de `ui.ts`, a la vez que se divide por pestañas (`migracion_a_typescript.md`).

---

## 🚀 Versión 1.46.0 - 2026-09-30
**Ayudante de DOM: `ui.ts` se comprueba por fin, sin `@ts-nocheck` y con 0 errores.** Sin cambios de comportamiento salvo un mensaje de error mejor.

### 🧰 Qué se añade
* `src/js/ui/dom.ts` con dos ayudantes: `elemento<T>(id)` devuelve el elemento o **falla diciendo cuál falta** (antes: «Cannot read properties of null», sin decir qué id) y `buscar<T>(id)` conserva el `null` donde la ausencia es legítima. Se aplicaron a los 232 accesos a `getElementById` de `ui.ts` (y a los de `app.ts`), con los tipos correctos (`Campo`, `HTMLSelectElement`…).
* **Se demostró que el cambio es mecánico:** el JavaScript compilado es idéntico byte a byte con el de `main` una vez normalizados los dos ayudantes; las 123 líneas restantes distintas se revisaron una a una (52 `String(err)` por `err.toString()`, campos de clase y `?? 0` donde la comparación ya trataba `null` igual).
* `ui.ts` ya no lleva `@ts-nocheck`: de **472 errores** a **0**. Declarados los campos de la clase, tipados los arrays y los parámetros por defecto `= null` de `api.ts`, y `dom.iterable` añadido a `tsconfig.build.json`.
* Reglas que lo conservan (`dom.test.js`): ningún fuente usa `@ts-nocheck`/`@ts-ignore`/`@ts-expect-error`; nadie llama a `document.getElementById` salvo `ui/dom.ts`; `index.html` carga el ayudante antes que la interfaz. Cuatro mutaciones, las cuatro detectadas.

### 🔎 Qué encontró
* Un defecto real, corregido aparte en 1.45.0: el panel «Casos de corrección» llamaba a un método de `AppAPI` que nunca existió.

### ⚠️ Un cambio de comportamiento, deliberado
* Donde el código suponía que el elemento existía, un elemento ausente ahora lanza «No existe el elemento #x en la página» en vez de «Cannot read properties of null»: el mismo fallo, con un mensaje que sirve. Donde ya había una guarda (`if (x)`), se conservó `buscar` y no cambia nada.

### ✅ Comprobado
* 38 pruebas de JavaScript y `npm run tipos`. En un navegador con datos simulados: las 11 pestañas pintan, el aviso de cobro muestra la fecha, un formulario envía a Rust lo que se escribió, el panel de casos y restaurar funcionan y no hay errores en consola.
* Siguiente: `noImplicitAny` (303 errores, 300 de parámetros sin tipo) junto con la división de `ui.ts`; ver `migracion_a_typescript.md`.

---

## 🚀 Versión 1.45.0 - 2026-09-30
**El panel «Casos de corrección» vuelve a abrir: le faltaba el envoltorio de `api`.**

### 🐞 Qué fallaba
* El botón de Ajustes que despliega los **casos de auditoría** (los que abre borrar o corregir un movimiento) llamaba a `AppAPI.obtenerCorrecciones`, un método que **nunca se escribió**: el comando `obtener_correcciones` existía y estaba registrado en Rust, pero `api` no lo envolvía. El panel mostraba en rojo «TypeError: AppAPI.obtenerCorrecciones is not a function» en lugar de los casos. Es decir, **los casos de auditoría se abrían pero no se podían consultar** desde la aplicación. Lo señaló TypeScript al quitar el `@ts-nocheck` de `ui.ts`; ninguna prueba miraba ese tramo. Está en la versión instalada.

### ✅ Qué se hace
* `api.ts` envuelve `obtener_correcciones`; la interfaz ya leía los campos correctos de la respuesta (`numero_caso`, `fecha`, `tipo`, `descripcion`, `importe`, `divisa`, `motivo`).
* **Prueba nueva** (`ipc.test.js`): toda llamada de la interfaz a `AppAPI` tiene su envoltorio en `api.ts`. Cierra el tramo que faltaba: la prueba anterior solo miraba de `api` hacia Rust. Con la mutación (quitar el envoltorio) falla; con el arreglo, no hay ninguna otra llamada huérfana.
* Reproducido y verificado en un navegador con datos simulados: antes «TypeError: … is not a function», ahora «C-0001 2026-09-30 · gasto Compra de prueba — DOP 10.00 Error al teclear el importe».

---

## 🚀 Versión 1.44.0 - 2026-09-30
**El frontend pasa a TypeScript, con solo `tsc`: infraestructura y todos los archivos.** Sin cambios de comportamiento.

### 🔧 Qué cambia
* `api`, `app`, `ui`, `nucleo/dinero` y `nucleo/respaldos` son ahora `.ts` (`git mv`: el historial se conserva). `tsc` compila a `src/js/**/*.js`, junto a los fuentes, **ignorados por git**; Tauri sigue empaquetando `src` y su `beforeBuildCommand` ejecuta `npm run compilar`. Sin empaquetador ni dependencias de producción.
* `ui.ts` lleva `// @ts-nocheck`: se renombró tal cual para que el código nuevo nazca tipado y la migración no se encarezca. La deuda está medida: quitar esa línea da 472 errores, casi todo ergonomía del DOM (un ayudante para `getElementById` elimina la mayoría).
* Los tipos de Rust ahora son `src/js/tipos-ipc.d.ts` (generado con `npm run tipos:generar`).
* Las pruebas de JavaScript pasan a `pruebas/js/`: **dejan de empaquetarse dentro de la aplicación**. `npm test` compila y las ejecuta sobre lo compilado.
* `herramientas/limpiar_js_generado.mjs` borra todo `.js` de `src/js` antes de compilar: un `.ts` eliminado no deja un `.js` huérfano que acabe empaquetado.

### ✅ Comprobado
* 30 pruebas de JavaScript y la comprobación de tipos pasan. Tres mutaciones (error de tipos en `api.ts`, `.js` huérfano, `.ts` con un tipo incorrecto): detectadas.
* En un navegador con datos simulados, el `ui` compilado hace lo mismo: las 11 pestañas pintan, el aviso de cobro muestra la fecha, restaurar funciona; sin errores en consola.
* En la aplicación empaquetada, construida con `tauri build` **partiendo de cero `.js`**: el hook compiló y el binario incluyó los `.js` generados aunque estén en `.gitignore`.

### ⚠️ Avisos
* `cargo build` directo no compila el frontend: usar `npm run tauri build` o `npm run compilar` antes. `tauri dev` no se ha probado con este esquema. Detalle en `migracion_a_typescript.md`.

---

## 🚀 Versión 1.43.0 - 2026-09-30
**El aviso «Cobro próximo» de Suscripciones vuelve a mostrar la fecha.**

### 🐞 Qué fallaba
* El aviso «🔔 Cobro próximo» imprimía `s.fecha_renovacion`, un campo que Rust **dejó de enviar en la Fase 5** (el puntero de cobro pasó a `fecha_proximo_cobro`). Con una suscripción por cobrar aparecía «… USD 10.00 el undefined». Lo encontró la comprobación de tipos del piloto (1.42.0); nada más lo había señalado en cuatro versiones.
* Está en la versión instalada (1.36.0): un aviso de cobro que no dice cuándo.

### ✅ Qué se hace
* El aviso lee `fecha_proximo_cobro`. Reproducido y verificado en un navegador con datos simulados: antes «el undefined», ahora «el 30/09/2026».
* Una prueba fija que la interfaz no lea campos de suscripción que Rust no envía.

---

## 🚀 Versión 1.42.0 - 2026-09-30
**Piloto de tipos con JSDoc: los tipos del contrato con Rust se generan de `main.rs`, y la comprobación encontró un defecto real.**

### 🧩 Qué se añade
* `herramientas/generar_tipos_ipc.mjs` genera `src/js/tipos-ipc.js` desde `main.rs`: las 16 estructuras que Rust devuelve, las de entrada y un mapa de los **61 comandos** con sus argumentos y su respuesta. Una prueba exige que esté al día.
* `api.js` tipa `invoke` contra ese mapa, así que cada envoltorio hereda los tipos sin anotarse uno a uno; `nucleo/dinero.js` queda tipado y pasa en modo estricto. Sin cambios de comportamiento: solo comentarios y un `const` intermedio.
* `npm run tipos` (nueva dependencia **de desarrollo**: `typescript`; el `.dmg` no cambia) comprueba esos archivos y forma parte de `npm run test:todo` y de `herramientas/revisar.py`. `npm run tipos:vistas` comprueba `ui.js` sin anotarlo, como experimento informativo.
* Se comprobó con mutaciones que detecta una clave equivocada en un envoltorio, un comando inexistente, texto donde Rust espera un número y un campo que Rust renombra.

### 🔎 El hallazgo
* `ui.js` muestra en el aviso «Cobro próximo» de Suscripciones `s.fecha_renovacion`, campo que Rust ya no envía desde la Fase 5 (ahora es `fecha_proximo_cobro`): la fecha sale como `undefined`. **Se corrige en un PR aparte.**
* Sin tipos de respuesta, la misma comprobación no encontraba nada; con ellos, 1 defecto real entre 204 avisos, y cada cambio de un campo en Rust señala los lugares de la interfaz que lo usan.

### 📐 Validación de TypeScript con solo `tsc`
* Comprobado en una copia temporal, sin tocar el repositorio: `tsc` desde `beforeBuildCommand` de Tauri, salida en módulos ES sin empaquetador, ejecutada en la aplicación empaquetada. Resultados y comparación en `fase_7_frontend.md`, §4. También corrige el análisis: TypeScript con `tsc` no necesita dependencias de producción; el coste es un paso de compilación y una carpeta generada.

---

## 🚀 Versión 1.41.0 - 2026-09-29
**El tramo 4 del redondeo se cierra sin convertir nada: la premisa que lo hace innecesario queda fijada con una prueba.**

### 📏 Qué se midió
* Un importe con dos decimales viaja como número y da los mismos centavos que sus dígitos: **0 discrepancias** en 2·10⁸ importes exhaustivos y 5·10⁷ muestreados hasta ~10¹⁵ centavos. Con más de dos decimales, 6,6 de cada 100 importes terminados en 5 se deciden distinto: solo pasaría si llegara una fracción de céntimo.
* Esa fracción no llega: 52 de 61 campos numéricos llevan `step="0.01"` y los otros 9 no son dinero, ningún formulario desactiva la validación, y los 16 envíos de dinero salen de `onsubmit`. **Comprobado a mano en la aplicación empaquetada** (WKWebView): escribir `1.005` en un campo de importe muestra el aviso del navegador y no guarda nada.

### ✅ Qué se añade
* `src/js/contrato/importes.test.js` (25 pruebas de JavaScript en total): todo campo numérico lleva `step="0.01"` o está declarado por su id exacto como no monetario, sin `novalidate`, con todos los envíos de dinero por `onsubmit` y sin importes calculados en el navegador. Cinco mutaciones, las cinco fallan.
* Corrección al análisis: eran **16** flujos con dinero, no 12; los cuatro que llegan en una estructura pasan por los mismos formularios.
* Se retoma la conversión a `ImporteDecimal` solo si aparece una vía que envíe importes sin formulario o si el navegador dejara de aplicar el `step`.

---

## 🚀 Versión 1.40.0 - 2026-09-29
**La comisión de un abono ya no se puede borrar por separado.**

### 🛡️ Qué se corrige
* Borrar por separado el gasto de la comisión de un abono devolvía su importe a la cuenta, y al revertir después el abono se devolvía **otra vez**: la cuenta acababa por encima de lo que tenía. Ahora se rechaza con un mensaje que manda revertir el abono completo. Revertir el abono sigue funcionando y no devuelve nada de más.
* La guarda es una **lista** (`GASTOS_DERIVADOS`), no una comprobación por caso: el cargo de un avance y la comisión de un abono. Una prueba compara la lista con las claves foráneas reales hacia `gastos`, de modo que **una tabla nueva que enlace un gasto falla las pruebas** hasta que se decida si el gasto depende de la operación o solo la menciona.
* Sin migración: en los datos vivos no había ningún caso.

### ⚠️ Lo que no se hizo
* No se rechaza revertir un abono sin vínculo: no se distingue de un abono histórico legítimo y bloquearía casos que hoy funcionan. Como el vínculo solo se perdía al borrar la comisión, y eso ya no es posible, no puede reaparecer. Explicado en `abonos_y_su_comision.md`.

### ✅ Pruebas
* Nuevas: `c136`, `c137`, `c138` y la de esquema; `c133` reproduce con SQL directo lo que ya no se puede provocar. 613 en Rust. Tres mutaciones (quitar la entrada de la lista, no consultarla, quitar el vínculo informativo): fallan pruebas.

---

## 🚀 Versión 1.39.0 - 2026-09-29
**Se fija un defecto conocido y se documenta su corrección: la comisión de un abono se puede borrar por separado.** No cambia comportamiento.

### 🔎 El hallazgo
* La comisión de un abono a tarjeta es un gasto que aparece en la lista y se puede borrar por separado. Borrarlo devuelve su importe a la cuenta, pero el abono conserva anotado que la comisión salió; al revertir después el abono, se devuelve **otra vez**. La cuenta acaba por encima de lo que tenía. Es la misma clase de defecto que ya se cerró para el cargo de un avance de efectivo.
* Comprobado en solo lectura sobre los datos vivos: **no ha ocurrido**.

### 📌 Qué se hace en esta versión
* La prueba `c136` fija el comportamiento tal cual está hoy, con importes sintéticos, marcado como defecto conocido.
* `abonos_y_su_comision.md` recoge el plan: una lista `GASTOS_DERIVADOS` con prueba de esquema que obligue a declarar toda clave foránea hacia `gastos`, y el rechazo con mensaje claro. **La corrección queda pendiente de visto bueno** (protocolo de hallazgos: primero documentar y fijar).

---

## 🚀 Versión 1.38.0 - 2026-09-29
**Restaurar ya no se bloquea cuando lo actual está dañado.**

### 🐞 Qué fallaba
* Al ensayar la restauración sobre una copia de la base real, con el estado actual estropeado a propósito (una referencia rota), `restaurar_respaldo` se **negaba**: la copia de seguridad que toma antes de sustituir pasaba por la misma verificación que un respaldo normal, la suspendía y abortaba. Es decir, fallaba justo cuando hace falta restaurar. Lo introduje en 1.36.0.

### ✅ Qué hace ahora
* La copia de seguridad previa se guarda **aunque el estado esté dañado**. Si no supera la verificación, se conserva con `-sin-verificar` en el nombre para que nadie la tome por una copia sana; si ni `VACUUM INTO` puede leer la base, se copia el archivo tal cual; solo si tampoco se puede copiar se aborta.
* Una copia `sin-verificar` **no se puede volver a restaurar** desde la aplicación (solo se restaura lo que se puede verificar): se conserva para rescatar datos a mano.

### 🔍 Ensayo con datos reales (copia temporal, ya destruida)
* Esquema 15: se respaldó, se estropeó (5 gastos borrados, una cuenta renombrada, capital vaciado) y se restauró: **mismo contenido en todas las filas** y capital idéntico; la aplicación lee las cuentas, gastos, tarjetas y capital restaurados; preparar el esquema no toca nada.
* Respaldo anterior a la migración (esquema 0): se restauró y la aplicación lo migró al 15 con las mismas filas, integridad correcta y sin referencias rotas.
* Tres mutaciones (volver a bloquear, no marcar la copia, borrar la copia dañada): las tres hacen fallar pruebas.

---

## 🚀 Versión 1.37.0 - 2026-09-29
**Restaurar un respaldo desde Ajustes, sin comandos.**

### 🖱️ Qué se añade
* En **Ajustes → Respaldo de la base**, debajo de «Respaldar ahora», un selector con los respaldos disponibles, del más reciente al más antiguo, y el botón «Restaurar este respaldo». Cada uno se muestra como `29/09/2026 19:03:25 · antes de instalar`; los nombres antiguos que no siguen el patrón se muestran tal cual.
* Antes de restaurar, un aviso dice qué se sustituye (base y capital) y que se guardará una copia del estado actual. Al terminar se avisa si el respaldo traía capital o se conservó el actual, y cómo deshacerlo: restaurar la copia «antes de restaurar» más reciente.
* Si no hay respaldos, la tarjeta lo dice; si la lista no se puede leer, el resto de Ajustes sigue funcionando.
* Los nombres se escapan antes de ir al HTML: salen de una carpeta del usuario.

### ✅ Pruebas
* 4 pruebas nuevas del formato de los nombres y del escape (19 de JavaScript en total). La lógica de restaurar ya estaba probada en Rust (1.36.0); el contrato entre `api.js` y `main.rs` cubre los dos wrappers nuevos.
* Comprobado en el navegador con un backend simulado: el selector, la confirmación, la llamada con `nombre`, la lista actualizada y el mensaje final. **No se probó contra la base real dentro de la aplicación empaquetada.**

---

## 🚀 Versión 1.36.0 - 2026-09-29
**Los respaldos se pueden restaurar y ya incluyen el capital.**

### 💾 Qué faltaba
* Hasta ahora se sabía **crear** un respaldo verificado, pero no había forma de usarlo: restaurar era copiar un archivo a mano, sin comprobar nada y sin red. Y el respaldo dejaba fuera `capital.json`, que no se puede reconstruir desde los gastos.

### ✅ Qué hace ahora
* Cada respaldo lleva, junto a la base, una copia del capital con el mismo nombre (`…​.capital.json`); la poda se lleva las dos.
* `restaurar_respaldo(nombre)` devuelve base y capital al estado de un respaldo. Antes comprueba que el respaldo **abre, es íntegro y no viene de un esquema más nuevo** que el que esta versión conoce; **respalda el estado actual** («antes de restaurar») para poder deshacer; y sustituye con un renombrado, de modo que queda la base vieja entera o la nueva entera. Un respaldo antiguo sin capital **no borra** el capital actual, y la respuesta lo dice.
* `listar_respaldos` devuelve los nombres, del más reciente al más antiguo. Solo se aceptan nombres de la carpeta de respaldos: una ruta se rechaza.
* Un cambio solo en el capital también hace que el siguiente arranque tome copia.
* Procedimiento y límites en `respaldos_restauracion.md`. No hay interfaz todavía: son comandos.

---

## 🚀 Versión 1.35.0 - 2026-09-29
**El cobro de suscripciones pasa por `Dinero` y por el registro de gastos: un cargo en dólares a una tarjeta que traduce ya puede liquidarse.**

### 💳 Lo que el SQL directo se saltaba
* Un cargo recurrente es un consumo con tarjeta, pero se cobraba con SQL directo, duplicando lo que `registrar_gasto` ya hace y con el importe como `f64`. `registrar_gasto` decide, según la política de la tarjeta, si un consumo en divisa **queda pendiente de liquidar**; el cobro directo no, así que **un cargo en dólares a una tarjeta que traduce jamás figuraba como pendiente y no podía liquidarse**.
* No es teórico: diez de las once suscripciones son en dólares y las cobran dos tarjetas, y la política se declara por tarjeta.
* Ahora `cobrar_suscripcion` delega en `registrar_gasto`: no repite la regla, la hereda. Y cobrar a una tarjeta inexistente **falla**; el `UPDATE` directo no se enteraba de que no actualizaba nada.

### ✅ Las condiciones se validan
* Solo el `CHECK` del esquema atajaba algo, con su mensaje crudo, y atajaba poco: un importe **cero o negativo** entraba —y cobrarlo abonaba a la tarjeta cada período—, y también un día de facturación fuera de 1 a 31. El alta y la edición validan importe, divisa, frecuencia y día, y dicen cuál falla.
* El importe entra como **texto**, como el resto: `500.005` sube a `500.01`. El mensaje del cobro automático lleva el importe con dos decimales.

### 🔍 Comprobado con datos reales
* Sobre una copia de la base real, cobrando todo lo vencido: 6 cargos; la deuda de cada tarjeta y divisa sube **exactamente** lo que suman los gastos nuevos (invariante calculado aparte, en centavos enteros); repetir no duplica ninguno.
* Con la política `traduce` en todas las tarjetas, **los 6 quedan pendientes de liquidar**; con las tarjetas como están hoy, ninguno. Cinco mutaciones, las cinco fallan.

---

## 🚀 Versión 1.34.0 - 2026-09-29
**El capital pasa por `Dinero`: cierra la última vía de dinero que se guardaba sin comprobar nada.**

### 💰 Qué se exige al guardar
* Un importe **positivo y exacto al céntimo**, una tasa de 0 a 100, una fecha `dd/mm/aaaa` que **exista**, un emisor o nombre no vacío y un identificador de bien único. Antes se guardaba tal cual un monto negativo, una tasa del 850 % o un `31/02`.
* El mensaje dice **qué entrada y qué campo** —«Certificado 2 (Banco X): el monto no puede ser negativo»—: el capital se guarda entero y un error genérico deja al titular buscando entre todas.
* Una tasa absurda sugiere lo que probablemente se quiso escribir: `850` propone `8.5`.

### 🕰️ Solo lo que entra o cambia
* El documento se guarda completo en cada acción, así que exigir todo cada vez bloquearía por una entrada antigua que no se tocó, y como no hay edición la única salida sería borrarla. **Lo ya guardado que vuelve idéntico se conserva**; lo nuevo y lo modificado pasa por las reglas. La comparación no distingue `500` de `500.0`.

### 🔌 El formato en disco no cambia
* Los importes siguen siendo **números** exactos al céntimo, porque la aplicación instalada lee el mismo archivo. Al entrar se admite también un importe como **texto**, tal como se escribió, y lo deciden los dígitos. Un **número** que llega con fracción de céntimo se rechaza en vez de redondearse.

### ➕ Los totales se suman en el núcleo
* Antes había dos sumas de decimales en JavaScript (el panel y el resumen) con el ruido de coma flotante que eso arrastra. Ahora se suman en centavos enteros en el núcleo. Como la alerta de vencimiento, se calculan al leer y **no se guardan**.

### ✅ Comprobado con datos reales
* Contra el `capital.json` real: la validación estricta lo acepta entero, leer y guardar de vuelta lo deja idéntico, y el patrimonio coincide con una suma independiente en enteros.
* Cinco mutaciones, las cinco fallan: perder la excepción de lo ya guardado, no validar nunca, guardar los totales, redondear una fracción en silencio y aceptar un monto de cero.

---

## 🚀 Versión 1.33.0 - 2026-09-29
**Eliminar una cuenta protege ya todas las relaciones que la referencian, y una prueba impide que una nueva quede sin guarda.**

### 🕳️ El hueco
* El comando protegía cuatro de las siete relaciones que apuntan a una cuenta. Faltaban las **facturas cobradas en ella**, los **ingresos informales cobrados** y, salvo de rebote, los **abonos que pagó**. Sus claves ajenas son `SET NULL`: borrar la cuenta se permitía sin error y dejaba el cobro «pagado» sin constancia de dónde entró el dinero.
* Se reprodujo antes de corregir: una factura cobrada, la cuenta borrada sin error, la factura pagada con la cuenta de depósito nula, y borrar esa factura después sin devolver nada a ninguna parte. No se pierde ningún saldo, pero se pierde el rastro.
* Los abonos estaban cubiertos por casualidad: la comisión de un abono es un gasto que lleva la cuenta, y la guarda de gastos los frenaba. Ese gasto se puede borrar por separado, y con él caía la única protección.

### 📏 La regla
* `RELACIONES_CON_CUENTAS` declara las siete relaciones. El adaptador cuenta por esa lista, el comando bloquea si alguna tiene filas, y **una prueba la compara con las claves ajenas reales del esquema migrado**: una relación nueva sin declarar hace fallar las pruebas, en lugar de dejar el borrado silencioso para cuando alguien pierda un rastro.
* Sustituye a tres comprobaciones escritas a mano, una por relación, cada una nacida cuando alguien tropezó con su caso. Los mensajes que ya conocía el titular no cambian.
* Una segunda prueba exige que cada relación declarada tenga su frase propia de qué se perdería. La frase genérica del comando es una red, no un sustituto.
* Cuatro mutaciones: olvidar declarar facturas —el defecto original—, olvidar los abonos, un adaptador que no informa de nada y una relación sin frase. Las cuatro fallan.

### 📄 Documentación
* `avance_de_efectivo.md` marca como confirmada por el titular la decisión de que el cargo lo paga la tarjeta y la cuenta recibe el monto íntegro.

---

## 🚀 Versión 1.32.0 - 2026-09-29
**Avance de efectivo: la tarjeta pone dinero en una cuenta de ahorro, con su cargo porcentual, fijo o exonerado.**

### 💵 La función
* La deuda de la tarjeta sube por el monto **y** por el cargo; la cuenta recibe el monto **sin** el cargo. Con 10 000 y un cargo del 6,25 %: la deuda sube 10 625, la cuenta recibe 10 000 y aparece un gasto de 625.
* **El cargo tiene tres formas**: porcentaje entre el 6 % y el 10 %, monto fijo, o exonerado. Son tres variantes de un tipo y no un porcentaje con casos especiales: una exoneración no es «un cargo del cero por ciento» sino la declaración de que no hubo cargo. Un cargo fijo de cero se rechaza por eso.
* **La banda del 6 % al 10 %** se exige porque casi todo porcentaje fuera de ella es un tecleo —`0,8` por `8`— y entraría en la deuda sin que nada lo cuestionara. Admite dos decimales: el cargo más reciente del titular fue del 6,25 %.
* La interfaz enseña lo que se va a mover y pide confirmación. Las cifras vienen del núcleo (`simular_avance_efectivo`), no de una cuenta hecha en JavaScript: una prueba comprueba que la cifra simulada es **exactamente** la que se asienta, con importes que redondean.

### ↩️ Se puede deshacer
* Revertir es el inverso exacto, con caso de auditoría y motivo escrito. Se repone **lo guardado**, no lo que hoy se recalcularía; y sin recorte: si el dinero ya se gastó, devolverlo deja la cuenta en negativo, que es el estado verdadero.
* El gasto del cargo **no se borra por separado**: bajaría la deuda por el cargo y dejaría el avance con un cargo que ya no existe. Una cuenta que recibió un avance tampoco se elimina.

### 🗄️ Base de Datos
* **Migración 15**: tabla `avances_efectivo`, que nace con sus restricciones ya puestas —importes exactos al céntimo, fecha con forma de fecha, importe positivo, cargo no negativo, y `tasa` solo en la forma porcentual, exigido en los dos sentidos—. Las claves ajenas son `RESTRICT`.

### 🔍 Decisiones tomadas sin que estuvieran especificadas
* El cargo lo paga la tarjeta, no la cuenta. Un avance se acredita en la misma divisa en que se carga: no convierte. No comprueba el disponible de la tarjeta ni calcula intereses del avance. Todo está en `avance_de_efectivo.md`.

---

## 🚀 Versión 1.31.0 - 2026-09-29
**Cuatro acciones que la aplicación instalada no habría podido ejecutar: borrar un gasto, borrar un traspaso, revertir un abono y borrar un ingreso informal.**

### 🔌 El defecto
* Desde que borrar un movimiento abre un caso de auditoría, los cuatro comandos de Rust exigen un `motivo`. La interfaz lo pedía al usuario y se lo pasaba al wrapper de `api.js`, pero **el wrapper no lo reenviaba**. Tauri rechaza una llamada a la que le falta una clave obligatoria, así que las cuatro habrían fallado en cuanto la aplicación se instalara.
* Ninguna prueba lo veía: las de Rust llaman a los comandos directamente, sin pasar por `invoke`, y las de JavaScript no leían `api.js` contra los comandos. Compilaba y pasaba todo.
* Solo `eliminarIngreso` reenviaba el motivo.
* No afecta a la aplicación en uso: sigue en una versión anterior a los casos de auditoría. Habría aparecido al instalar la nueva.

### 📏 La regla
* `src/js/contrato/ipc.test.js` lee `api.js` y `main.rs` como texto y exige tres cosas: que cada wrapper envíe todo lo que su comando exige, que no envíe claves que el comando no conoce, y que no invoque un comando inexistente. Sin dependencias, y `herramientas/revisar.py` ya la ejecuta con el resto.
* No sustituye a ejecutar la aplicación, pero cierra una clase de error que ninguna otra prueba cubría. Se comprobó con cuatro mutaciones: el defecto original, una clave inventada, un comando que no existe y descartar de nuevo el motivo. Las cuatro fallan.

---

## 🚀 Versión 1.30.0 - 2026-09-23
**Arranca la Fase 6 — Capital y préstamos. Un valor calculado que se guardaba como declarado, corregido y confirmado contra la base real.**

### 🏦 Préstamos, documentado en retrospectiva
* El saldo vivo, el desglose de cuota y la conciliación de la versión 1.6.0 quedan registrados en `fase_6_capital_y_prestamos.md`: no había trabajo pendiente, solo el documento de la fase.

### 📉 Capital: el defecto de fondo
* `obtener_capital` calculaba la alerta de vencimiento **sobre el mismo objeto** que devolvía. Las seis acciones de la vista de capital —añadir o quitar un certificado, una inversión, un bien— leen el capital completo, mutan una colección y guardan el objeto entero de vuelta, así que ese cálculo **se grababa en el archivo** como si el titular lo hubiera declarado.
* No era teórico: la base real ya tenía `alerta_vencimiento` y `dias_restantes` guardados en disco para una inversión de bolsa, sin actualizarse desde el día en que se escribieron.
* `dominio::capital::calcular` centraliza la fórmula —antes duplicada entre certificados y bolsa— y `guardar_capital` retira los tres campos calculados **antes de persistir**, en el único punto de escritura, sin depender de que cada acción de la interfaz recuerde omitirlos.
* La base real, ya contaminada, se limpió a mano con un respaldo tomado antes de escribir.

### 🔍 Lo que queda declarado y sin resolver
* El capital no pasa por `Dinero`: un monto negativo, con fracción de céntimo o una fecha ilegible se acepta tal cual. Es la vía de dinero menos vigilada del proyecto, y se deja fuera por ser justo el motivo de que esta fase tenga menor densidad de reglas.

---

## 🚀 Versión 1.29.0 - 2026-09-23
**Registra el método de respaldos previsto para la versión 2027. No cambia el mecanismo vigente.**

### 📄 Documentación
* `METODO_RESPALDOS_VERSION_2027.md` fija la cadencia acordada para el rediseño: diarios cifrados en carpeta sincronizada (366 × 24 horas de retención mínima), mensuales locales, y un cierre anual del 31 de diciembre sin borrado automático.
* Los componentes de captura, cifrado, custodia, entrega local y calendario ya existen —probados con datos sintéticos— en una rama aislada de otro repositorio. Aún no están conectados al arranque ni a una carpeta real de nube.
* **El respaldo operativo de `main` no cambia**: sigue siendo el de [`src-tauri/src/respaldo.rs`](src-tauri/src/respaldo.rs), que copia antes de tocar el esquema y conserva diez.
* Cherry-pick de un commit hecho directamente sobre el `main` de otra copia local, para que el registro quede también en el historial versionado del repositorio.

---

## 🚀 Versión 1.28.0 - 2026-09-23
**Cierra la Fase 5: sin «Suscripciones» ni «Otros», la categoría se crea en vez de improvisar con la posición 1.**

### 🐛 El último de los seis defectos
* `categoria_de_suscripciones` buscaba por nombre, y si el titular renombraba o borraba las dos categorías que reconoce, el cargo caía en el **identificador 1 literal**. Un gasto de suscripción podía archivarse como alquiler o gasolina sin ningún error que lo dijera.
* Agotadas las dos búsquedas, ahora se **crea** `Suscripciones` en el momento. Es idempotente y no toca nada cuando la categoría ya existe.

Con esto se cierran los seis defectos que abrió la caracterización de la Fase 5.

---

## 🚀 Versión 1.27.0 - 2026-09-22
**La fecha manda: una suscripción guarda cuándo vence su próximo cobro, y ningún período se pierde en silencio.**

### 🕳️ La ventana que se cerró
* La marca de idempotencia guardaba **cuándo se ejecutó** el cargo, no **qué período saldó**. Eso daba a cada período una ventana —de su día de facturación al fin de mes— y perderla lo borraba: al llegar el mes siguiente, la marca pasaba a leerse como «ya atendido».
* La ventana era de **un solo día** para una suscripción del día 30. Costó un cargo real: **Netflix, agosto de 2026, USD 13,99**. Al migrar, ese período reaparece y se asienta con su fecha.
* Ahora la suscripción guarda `fecha_proximo_cobro`. Una fecha que ya pasó sigue pasada. `fecha_renovacion` se funde con ella: eran el mismo hecho con dos nombres.
* **`dia_facturacion` sobrevive como ancla**, y no por compatibilidad: el recorte a fin de mes no puede persistirse, o una del día 30 cobrada el 28 de febrero quedaría anclada al 28 para siempre.

### 📋 Uno se cobra solo, varios se preguntan
* Con un período vencido no hay ambigüedad. Con varios, la aplicación no sabe si el proveedor los cobró ni si la suscripción siguió activa: **fabricar cargos que quizá no ocurrieron es peor que señalarlos**.
* «Sí se cobró» asienta con la fecha del vencimiento. «No se cobró» avanza sin cobrar y **exige un motivo**, que queda como caso de auditoría.
* La lista tiene tope de doce: más atrás, nadie va a conciliar uno a uno.

### 🐛 Lo que arregla de paso
* El asiento lleva **la fecha del vencimiento, no la de ejecución**. En la base real un cargo de Google One —que factura el día 9— figuraba asentado el 11.

### ↩️ Lo que deja sin efecto
* El impedimento por **marca de cobro ilegible** de 1.26.0. Desde que la decisión lee una fecha, la marca no decide nada, así que ya no impide y señalarla sería un aviso que miente. Se conserva la restricción de esquema.

### 🗄️ Base de Datos
* **Migración 14**: funde `fecha_renovacion` en `fecha_proximo_cobro` y la deriva para las mensuales. Con último cobro, el período siguiente —que para Netflix queda vencido, y por eso reaparece—; sin él, su día en el mes en curso o el siguiente. No se inventa un pasado.

---

## 🚀 Versión 1.26.0 - 2026-09-22
**Una marca de cobro ilegible deja de cobrar en cada arranque. Era el defecto que invertía la idempotencia.**

### ⛔ No cobrar, y decirlo
* Si la fecha del último cobro no tenía tres partes separadas por `/`, la decisión hacía `requiere_cargo = true` sin más: **el mecanismo de idempotencia se volvía un duplicador**.
* Ahora no cobra. No se puede saber cuándo se cobró por última vez, y entre un cargo de más y uno de menos, el de menos se corrige mirando el estado de cuenta.
* **Pero no en silencio.** Una suscripción parada sin avisar es peor que una que cobra de más: el cargo indebido sale en el estado, la parada no sale en ninguna parte. `Impedimento` responde «¿llegará a cobrarse?», que es distinto de «¿toca hoy?», y unifica los dos estados parados.
* En una **anual** con marca ilegible no hay impedimento: decide con su fecha de renovación y no mira la marca. Señalarla sería decir que algo está parado cuando no lo está, y un aviso que miente se aprende a ignorar.

### 🔧 La salida
* `corregir_ultimo_cobro` es la única vía para escribir la fecha a mano, y existe solo para esto: la edición normal la conserva a propósito (`s7`), y esa preservación dejaba sin salida a una suscripción con la fecha rota.
* **Pide la fecha en lugar de limpiarla.** Borrarla la dejaría como «nunca cobrada» y volvería a cobrar este mes, que es el duplicado del que `s7` protege.

### 🗄️ Base de Datos
* **Migración 13**: un `CHECK` con `GLOB` obliga a que `fecha_ultimo_pago` y `fecha_renovacion` tengan forma `dd/mm/aaaa`. No es teórico: en la base real hay un `gastos.fecha` con valor `1009/2026`, un `10/09/2026` sin la primera barra.
* El `CHECK` comprueba la **forma**, no que la fecha exista. Por eso el analizador pasó a validar con `from_ymd_opt`: antes parseaba tres partes y usaba dos, de modo que un `31/02/2026` pasaba como febrero y un `13/13/2026` como mes 13.
* La migración **no toca `gastos.fecha`**: corregir un dato del titular es decisión suya.

---

## 🚀 Versión 1.25.0 - 2026-09-22
**El día de facturación se recorta a los días que tiene el mes: febrero deja de perder su cargo.**

### 📅 La regla
* Una mensual del **día 30** se cobraba once veces al año y una del **31**, siete: la condición comparaba contra el día crudo, y `hoy.day()` nunca llega a 31 en un mes de 30. Ahora el día se recorta al último del mes.
* **La fecha sale del estado de cuenta.** Un cargo del día 29 se generó el **28 de febrero** y se liquidó el 1 de marzo: son dos fechas distintas. Esta aplicación asienta el *consumo* contra la tarjeta, así que el asiento lleva la de generación; la liquidación entra por el ciclo de pago, que se lleva aparte.
* El recorte **mueve el día, no añade vencimientos**: cada mes sigue teniendo uno, y la marca de idempotencia no cambia.

### 💵 Efecto sobre datos reales
* Dos suscripciones facturan los días 29 y 30. Entre las dos, la aplicación dejaba de asentar **USD 30,94 al año** que el proveedor sí cobraba.
* Simulando 2027 sobre una copia de la base real: 98 cargos frente a 96, y los dos que faltaban aparecen fechados el **28/02/2027**.

### 🔍 Lo que se descartó al medirlo
* Una formulación más general —«cobra si pasó cualquier vencimiento posterior al último cobro»— también arreglaba febrero, y **de paso recuperaba un período atrasado** que antes se perdía: dieciocho cargos en enero donde había ocho, sobre datos reales. Recuperar períodos vencidos es una decisión abierta (`s13`) y no se toma de rebote.
## 🚀 Versión 1.24.0 - 2026-09-22
**Las suscripciones anuales anotan cuándo renuevan, en vez de deducirlo, y avisan una semana antes.**

### 📅 La fecha de renovación
* La condición anterior era `anio_actual > p_anio` y **no miraba el mes**: una anual cobrada en julio volvía a cobrar el 5 de enero, seis meses antes. No era una condición mal escrita, era que **intentaba deducir un vencimiento con un dato que no bastaba**.
* Ahora la fecha se anota y la decisión la lee. **Una anual sin fecha no se cobra**: entre un cargo de más y uno de menos, el de menos es el que se corrige mirando el estado de cuenta.
* Cierra de paso los otros dos agujeros **en las anuales**: ni el día 31 ni una marca de cobro ilegible las desvían ya. En las mensuales siguen abiertos, porque allí la marca es el único dato.
* Una fecha de renovación ilegible **se rechaza al guardarla**, en vez de dejar una anual que aparenta estar configurada y no cobra.

### 🔔 El aviso
* Siete días antes del cargo y hasta el propio día; pasada la fecha se apaga, porque ya no es un aviso sino un cobro pendiente.
* **Solo las anuales.** Una mensual tendría que predecir su próximo cobro, y esa predicción arrastraría el defecto del día 31: anunciar una fecha que el sistema luego no respeta es peor que no anunciar nada.
* La regla vive en el núcleo y llega a la vista como un `bool`. Una regla en el HTML es una regla sin pruebas.

### 🗄️ Base de Datos
* **Migración 12**: añade `suscripciones.fecha_renovacion` y la deriva para las anuales existentes, tomando el **día de facturación y no el día en que se ejecutó el cargo** — sobre datos reales esa diferencia era de un día. Sin marca previa, o con una ilegible, la deja vacía.

---

## 🚀 Versión 1.23.0 - 2026-09-22
**Fase 5 — Suscripciones. El puerto `Reloj` entra en servicio, y con él aparecen seis defectos que la red anterior no podía ver.**

### ⏰ El reloj, que existía sin usarse
* `procesar_suscripciones` leía la fecha del sistema por dentro. Eso **hacía imposible escribir la prueba que más falta hacía**: que una suscripción *no* se cobre antes de su día. Todas las pruebas anteriores usan el día 1 de facturación porque es el único siempre alcanzado, de modo que la red cubría el cobro y no la abstención.
* El puerto `Reloj` y su adaptador estaban construidos desde la Fase 0 y no los usaba nadie. El comando delega ahora en `procesar_suscripciones_con(reloj)`.

### 🔍 Seis defectos, reproducidos y **sin corregir**
* Una mensual del **día 31** se cobra **7 de 12 meses**: febrero, abril, junio, septiembre y noviembre se saltan enteros. La del día 30 pierde febrero.
* Una **anual** se recobra al cambiar el año aunque no haya pasado uno: cobrada en julio, vuelve a cobrar el 5 de enero.
* **Tres meses sin abrir la aplicación generan un solo cargo**, no tres.
* Una marca de cobro **ilegible** cobra en cada arranque, incluso antes de su día. Es el más grave: **invierte la propiedad que da nombre a la fase**.
* Sin las categorías «Suscripciones» ni «Otros», el cargo cae en el identificador **1 literal**, que hoy es «Alimentación».
* Los seis cambian importes que un proveedor ya cobró de verdad, y tres no tienen respuesta obvia. Quedan documentados y fijados por pruebas, a la espera de decisión.

### 🧩 `dominio::suscripcion`
* La regla sale del bucle que mueve el dinero y pasa a ser un tipo con una sola pregunta: `corresponde_cobrar(hoy)`.
* **`MarcaDeCobro::Ilegible` es distinto de `Ninguna`**: no haber cobrado nunca y no entender la marca son estados opuestos, y confundirlos era lo que escondía el defecto.
* La marca **no guarda el día**, porque la regla no lo mira; conservarlo sugeriría una precisión que la decisión no tiene.
## 🚀 Versión 1.22.0 - 2026-09-22
**El esquema rechaza una fracción de céntimo al escribir. El tramo a `INTEGER` se descarta: midiendo, la premisa era falsa.**

### 📌 Por qué no se convirtió a `INTEGER`
* Estaba previsto guardar centavos enteros. **En SQLite la afinidad `INTEGER` no restringe nada**: una columna declarada `INTEGER` acepta `75.005` y lo guarda como `real`, porque solo convierte cuando no pierde. El cambio de tipo no impedía lo que se quería impedir.
* Tampoco compraba nada por otro lado: `REAL` representa centavos exactos hasta 2,5·10¹⁶, y sumar un millón de filas mezclando magnitudes se desvió **0,000000 ¢**.
* Lo único que cambiaba era el modo de fallo, y a peor: leer un `INTEGER` como `f64` devuelve el entero crudo **sin error** —cien veces el importe, en silencio— y escribir un `i64` en una columna `REAL` hace lo mismo. Solo la dirección contraria falla en voz alta.
* Se cierra como se cerró el tramo 3: **declarándolo innecesario**, con las mediciones convertidas en pruebas.

### 🧮 La restricción de céntimo
* Cada columna de dinero vive bajo `CHECK (ROUND(v, 2) = v)`. La garantía que faltaba —rechazar la fracción **al escribir**, no solo al migrar— sin tocar la unidad de almacenamiento.
* **La expresión no es intercambiable.** Las variantes con `* 100` rechazan céntimos legítimos —once en un barrido, empezando por 4,77— porque multiplicar introduce el error que se pretendía detectar. La elegida no rechazó ninguno en 200 000 valores densos ni por magnitudes hasta 10¹⁴.

### 🐛 Lo que la prueba contra datos reales destapó
* **La aplicación habría dejado de funcionar.** `SET v = v + ?` suma en coma flotante y el 13 % de esas escrituras produce un valor que el `CHECK` rechaza; la quinta ya fallaba. Las nueve acumulaciones en SQL pasaron a `ROUND(v ± ?, 2)`, que corrige como mucho 1,5·10⁻⁵ unidades y **nunca decide un céntimo**. Una prueba recorre las fuentes para que no reaparezca ninguna sin redondear.
* **Doce importes reales no pasaban**: no eran fracciones sino ruido de representación de ~10⁻¹², que las migraciones 3 y 9 saltaban por usar una tolerancia de `1e-6`.
* **`legacy_alter_table` no surte efecto con las claves ajenas encendidas.** El `PRAGMA` se lee como activo y `RENAME` reescribe igualmente las cláusulas `REFERENCES` de las otras tablas. Renombrar `cuentas_ahorro` dejaba a `gastos` apuntando a una tabla temporal.

### 🗄️ Base de Datos
* **Migración 10 — el céntimo exacto, sin tolerancia**: redondea por exactitud, con el mismo criterio que impone el `CHECK`.
* **Migración 11 — la fracción se rechaza al escribir**: reconstruye las 12 tablas con sus restricciones. Idempotente, comprueba el recuento de filas y se detiene si encontrara un índice o disparador propio que la reconstrucción perdería.
* **Las migraciones corren con las claves ajenas apagadas**, como SQLite prescribe para rehacer una tabla, y cada una termina con `foreign_key_check` dentro de su transacción: la comprobación pasa de ser por sentencia a ser por migración.

---

## 🚀 Versión 1.21.0 - 2026-09-22
**Los importes que el usuario teclea llegan al núcleo como dígitos, no como coma flotante.**

### 🧮 Núcleo monetario
* `Dinero::desde_texto` decide el céntimo **sobre los dígitos escritos**. `Dinero::nuevo` recibía un `f64` que ya no valía lo tecleado: `1.005` se almacena como 1.00499… y bajaba a 1.00. Por texto sube a 1.01, que es la regla del sistema.
* `ImporteDecimal`, en la frontera IPC, acepta **texto o número**. El número se admite durante la transición para migrar pantalla a pantalla; cuando no quede ninguna llamada que lo mande, esa rama se retira.

### 📌 De dónde viene esta decisión
* Salió de investigar el tramo 4 de la política de redondeo, y el argumento no estaba en el planteamiento original: **los tramos 1 y 2 no eliminaron la conversión desde coma flotante, la centralizaron**. El defecto que `dividir_redondeando` documenta vivía también en la puerta de entrada.
* Antes de aplicarlo **se midió, y la medida acotó el trabajo**: 52 de 53 campos de importe llevan `step="0.01"`, que el navegador valida al enviar el formulario; y el viaje `texto → Number → texto` no pierde nada con dos decimales —cero divergencias en 28 571 muestras—.
* Quedan **tres importes fuera de esa validación**: la comisión por pago de impuestos, el saldo declarado de un préstamo y el cobro parcial de una factura. Los tres entran sin pasar por el formulario. Son los tres que se migran ahora.
* Se anota así, y no como «se hizo todo», porque **la diferencia entre una corrección que cambia importes y una que ordena el código debería poder leerse**. Migrar los cuarenta y un parámetros restantes es mecánico y no urge: el `step` ya los cubre.

### 🖥️ Interfaz
* Los tres campos mandan **el texto tal cual**, y se validan por **forma** —una expresión regular— en vez de por conversión. Convertir para comprobar devolvería al punto de partida.
* La cota del cobro parcial sigue comparando números, que es lo que es: una comparación, no una decisión de céntimo.

### 🐛 Correcciones
* `revertir_abono_tarjeta` calculaba el número de caso de auditoría y **no lo devolvía**. La reversión quedaba registrada, pero el usuario no veía con qué número.

---

## 🚀 Versión 1.20.0 - 2026-09-22
**Cuatro columnas de dinero estaban fuera de la red del redondeo, y una dejaba entrar fracciones de céntimo.**

### 🧮 Núcleo monetario
* `cuentas_ahorro.comision_pago_impuestos` se escribía **sin pasar por `Dinero`**: solo se comprobaba que el número fuera finito y no negativo. Una comisión de 75.005 entraba con su tercer decimal. Era la única vía por la que un importe llegaba a la base sin que el núcleo decidiera su céntimo.
* Otras tres —`pagos_tarjeta.monto_debitado`, `pagos_tarjeta.comision` y `correcciones.importe`— quedaron fuera de `COLUMNAS_DE_DINERO` por haberse añadido en migraciones **posteriores** a la 3, que es la que redondea y verifica.

### 📏 La regla
* **En un esquema migrado, toda columna `REAL` tiene que estar clasificada** como dinero o como tasa. Una prueba lo exige y nombra la que falte.
* La guardiana anterior protegía en un solo sentido: impedía meter una tasa entre el dinero, pero no impedía **olvidar** una columna de dinero. Por ahí se colaron las cuatro.
* El coste de cumplirla es una línea en una lista. El de no tenerla ya se pagó.

### 🗄️ Base de Datos
* **Migración 9 — las columnas que nacieron fuera de la red**: redondea al céntimo las cuatro tardías, con la misma verificación que la 3.

---

## 🚀 Versión 1.19.1 - 2026-09-22
**El tramo 3 de la política de redondeo se cierra declarándolo innecesario.**

### 🧮 Núcleo monetario
* Estaba previsto sacar la coma flotante también de las **columnas de tasa**, que SQLite guarda como `REAL`. Se comprobó antes de implementarlo y **no compra nada**: una tasa sobrevive intacta al viaje `entero → REAL → entero`.
* A escala de mil-millonésimas, una tasa del rango plausible necesita unos doce dígitos significativos y `f64` ofrece quince. La holgura es amplia, y lo mismo vale para los porcentajes de interés.
* Lo que sí hacía falta era **proteger esa holgura**, porque es la razón de que el tramo sobre. Dos pruebas de barrido lo fijan: si algún día se subiera la escala hasta agotarla, el viaje empezaría a perder y se sabría ahí.
* Una migración de tres columnas, su relleno y su verificación, **sustituidos por dos pruebas**.

### 🧪 Pruebas
* Las pruebas de caracterización usaban una **ruta temporal fija**, de modo que dos ejecuciones simultáneas de `cargo test` se pisaban la misma base y fallaban con «attempt to write a readonly database» — un mensaje que no dice en absoluto lo que pasa. Ahora la raíz lleva el identificador del proceso.
* Aparece en cuanto una herramienta lanza las pruebas mientras hay otra corriendo, que es justo lo que hace `herramientas/revisar.py`.

### 📄 Documentación
* `politica_redondeo.md` recoge los cuatro tramos, su estado, y las decisiones que cambiaron sobre la marcha.

---

## 🚀 Versión 1.19.0 - 2026-09-20
**Borrar un movimiento abre un caso de auditoría y exige explicarlo.**

> **Medida temporal.** La solución definitiva son los asientos de compensación de la Fase 8: un movimiento anulado deja su contrario y el original sobrevive. Mientras eso no exista, borrar destruye el rastro, y esto deja constancia de qué se destruyó y por qué.

### 🛠️ Corrección de transacciones
* Los cinco borrados de movimiento —gasto, factura, ingreso informal, traspaso y abono— **exigen un motivo escrito** y devuelven un **número de caso** (`COR-2026-0001`).
* **La exigencia es fricción deliberada, no un trámite.** El propósito de esta pantalla no es facilitar el borrado sino reducir cuántas veces hace falta: una corrección que cuesta una frase se piensa dos veces. Un motivo de menos de quince caracteres se rechaza, porque «error» pasa cualquier comprobación de «no vacío» y no explica nada.
* **Una línea junto al botón dice qué se pierde**: que no queda asiento que anule el movimiento, solo el caso, y que conviene corregir antes que borrar cuando la operación lo permita.
* El caso guarda **cómo se identificaba el movimiento** —descripción, importe y divisa— porque después de borrarlo no habría de dónde sacarlo.
* Los casos se consultan desde la misma pantalla. **No hay comando para borrarlos**: un rastro que se puede borrar no es un rastro.

### 🧾 Corregir una factura cobrada también abre caso
* Corregir el importe de una factura ya cobrada **mueve un saldo**, igual que borrarla, y hasta ahora no dejaba rastro. El registro se había construido para los borrados y esta corrección se quedó fuera.
* **El motivo se exige solo cuando mueve dinero.** Cambiar la fecha o el número de una factura cobrada no toca ningún saldo y no lo pide: exigir explicación donde no hay riesgo enseña a escribirla sin pensar, que es el modo en que un control de este tipo deja de servir.
* El diálogo dice **el ajuste exacto** que va a sufrir la cuenta antes de pedir la explicación.
* Si falta el motivo, la operación se deshace entera: ni caso, ni corrección, ni saldo movido.

### 🗄️ Base de Datos
* **Migración 8 — casos de corrección**: tabla `correcciones`.
* Limitación declarada y fijada por prueba: la numeración sale del mayor caso vivo del año, de modo que **borrar el último reutilizaría su número**. Por eso no existe forma de borrarlos; si algún día la hubiera, habría que rehacer la numeración primero.

---

## 🚀 Versión 1.18.0 - 2026-09-20
**Fase 4.2: el dominio de ingresos cierra los tres defectos que quedaban.**

### 🧾 Ingresos — H17, H18 y H19 resueltos
* **H17 — la cuenta deja de referenciarse por su nombre.** El cobro se aplicaba con `UPDATE ... WHERE nombre = ?` descartando el resultado: si el nombre no coincidía, el ingreso quedaba cobrado y ningún saldo se movía. Ahora se referencia por identificador y su ausencia es un error. Es el mismo arreglo que cerró H3 con la caja de efectivo.
* **H18 — cobrar una factura inexistente ya falla.** El `UPDATE` afectaba a cero filas y devolvía éxito; el sistema no distinguía entre haber cobrado y no haber encontrado nada que cobrar. La condición exige además que la factura esté pendiente, de modo que cobrar dos veces tampoco pasa inadvertido.
* **H19 — el importe entra en la divisa de su cuenta.** El tipo `Deposito` no se construye si el importe y la cuenta no coinciden, así que sumar pesos a un saldo en dólares deja de ser representable. Es el camino por el que se cerró H2.

### 🗄️ Base de Datos
* **Migración 7 — el cobro apunta a una cuenta, no a un nombre**: `ingresos` e `ingresos_informales` ganan `cuenta_ahorro_id`, rellenado desde el nombre guardado.
  * El nombre se conserva: sirve para leer el histórico y para los cobros registrados contra una cuenta que ya no existe, donde no hay identificador que poner.

### 🖥️ Interfaz
* Los selectores de cuenta de los cobros mandan el **identificador** en lugar del nombre.

---

## 🚀 Versión 1.17.0 - 2026-09-20
**H20 resuelto, y el cobro parcial pasa a ser una afirmación explícita.**

### ↩️ Ingresos — H20 resuelto
* Borrar una factura o un ingreso informal cobrado **deja de recortar el saldo en cero**. Si lo cobrado ya se gastó, la cuenta queda en negativo: eso es el estado verdadero, y el recorte hacía desaparecer la diferencia sin registro.
* Se resuelve **en las mismas condiciones que H5 y H10**, que retiraron el mismo recorte en tarjetas y transferencias. Con esto, el recorte a cero ya no existe en ningún vertical.

### 🧾 Corregir una factura cobrada
* **La regla pasa a ser el cobro completo.** Corregir el monto de una factura la da por cobrada por su neto nuevo, que es lo normal cuando se corrige un importe mal anotado.
* **Se confirma con las cifras delante**, no con una advertencia genérica: el diálogo dice por cuánto pasará a constar cobrada y que la cuenta se ajustará.
* **El cobro parcial es la excepción, y hay que declararlo.** Una casilla despliega el importe realmente cobrado; la diferencia con el neto queda como pendiente en vez de darse por saldada.
  * Está oculto por defecto a propósito: un campo siempre visible invitaría a rellenarlo y convertiría la excepción en costumbre.
  * Un «parcial» mayor que el neto **se rechaza**. Admitirlo dejaría la factura diciendo que se cobró más de lo facturado, sin nada que lo explicara.
* **La cuenta sigue siempre a lo recibido**, sea completo o parcial. Una sola regla, sin dos caminos que puedan divergir.

---

## 🚀 Versión 1.16.0 - 2026-09-20
**H16 resuelto, y una factura ya cobrada se puede corregir.**

### 🧾 Ingresos
* **H16 resuelto — la retención se decide al céntimo.** Antes `(monto × tasa).round()` redondeaba a unidades: el 15 % de 1 234.56 —185.184— quedaba en 185.00. Se perdían céntimos en cada factura, siempre en la misma dirección.
  * Pasa a usar el **mismo núcleo** que el resto del sistema. No por uniformidad: una regla que existe dos veces se corrige una vez y sigue mal en la otra, que es literalmente lo que había pasado con H8.
  * La retención y el neto **suman siempre el total exacto**, porque el neto se obtiene restando en vez de calcularse con un segundo porcentaje que volvería a redondear.
* **Corregir una factura ya cobrada**:
  * La interfaz solo ofrecía corregir mientras la factura estuviera emitida. Una vez cobrada, un error de importe quedaba congelado.
  * Corregirla **no es reescribir cifras**: hay dinero en una cuenta que dependía de ellas. La corrección ajusta esa cuenta por la diferencia del neto, y avisa antes de hacerlo.
  * El ajuste **se suma** a lo recibido en lugar de sustituirlo por el neto nuevo. Así una diferencia deliberada entre lo facturado y lo que de verdad entró —un cobro parcial— sobrevive a la corrección en vez de borrarse.
  * Si la cuenta de depósito ya no existe, la corrección **falla diciéndolo** en lugar de dejar la factura y el saldo descuadrados.

---

## 🚀 Versión 1.15.0 - 2026-09-19
**Una reversión devuelve lo que salió, no lo que hoy se calcularía.**

### ↩️ Reversión de abonos
* El abono **guarda** el importe que debitó de la cuenta y la comisión que cobró. La reversión los lee en vez de reconstruirlos.
* **Corrige una decisión anterior de este mismo proyecto.** La primera versión recalculaba, con el argumento de que el cálculo es el mismo que al registrar. Era el argumento equivocado: deshacer una operación es reponer *lo que ocurrió*, no lo que hoy creemos que debió ocurrir. Si la regla de redondeo cambia —como acaba de cambiar— o si un importe se corrigió a mano, recalcular deja un residuo que nadie ve.
* Un abono con cuenta pero **sin débito guardado ya no se revierte a ciegas**: se dice qué falta, porque devolver una cifra inventada a una cuenta real es peor que no devolver nada.

### 🗄️ Base de Datos
* **Migración 6 — el abono guarda lo que debitó**: `pagos_tarjeta` gana `monto_debitado` y `comision`.
  * El relleno de los abonos anteriores usa **el importe y la tasa**, no la comisión. Derivar el débito dividiendo la comisión entre 0.002 solo es exacto mientras la comisión conserve sus decimales: una vez redondeada al céntimo la división se desvía — 42.77 / 0.002 da 21 385.00 para un débito real de 21 384.61.
  * Para las filas anteriores a la columna el valor es una **reconstrucción, no un registro**, y queda dicho en la migración.

---

## 🚀 Versión 1.14.0 - 2026-09-16
**El céntimo se decide en aritmética entera, no en coma flotante.**

### 🧮 Núcleo monetario
* **`dividir_redondeando` es ahora el único lugar del sistema donde se decide un céntimo.** Redondea la mitad alejándose de cero, en enteros.
* **La regla anterior no era la que decía ser.** `f64::round()` aplica «mitad alejándose de cero» sobre el valor binario, que no es el decimal escrito: `1.005` se guarda como 1.00499… y bajaba a 1.00, mientras que `2.675` —cuyo error se cancela al multiplicar por 100— subía a 2.68. Dos importes de la misma forma, en direcciones opuestas, por un accidente de representación.
* **`Porcentaje`**: tipo nuevo, entero en millonésimas de la fracción. La retención pasa a declararse como `Porcentaje::puntos_basicos(20)` en vez de `0.002`, que no existe exactamente en binario.
* **`TasaCambio`** pasa a mil-millonésimas enteras. Necesita más escala que un porcentaje porque una tasa y su recíproca viven en órdenes de magnitud distintos: 60 pesos por dólar es 0.0166… dólares por peso, un decimal periódico.
* `porcentaje()` y `convertir()` calculan con `i128` y redondean una sola vez, al final.

### 📏 Tolerancia de representación: 0.01 centavos
* Se declara y se **hace cumplir por prueba** una desviación máxima de 0.01 centavos atribuible a representar una tasa, medida sobre un importe de referencia de 10 000 unidades.
* **Acota una de las dos fuentes de desviación, no las dos.** El redondeo final al céntimo no es un error sino una decisión: el 0.20 % de 8 967.90 son 1 793.58 centavos, y hay que cobrar 1 793 o 1 794. Ese residuo llega a medio centavo por definición y ningún límite lo reduce.
* **La tolerancia obligó a subir la escala de los porcentajes** de millonésimas a mil-millonésimas: a la escala anterior, una tasa cuantizada desviaba hasta 0.5 centavos sobre el importe de referencia, cincuenta veces el límite.
* Las tasas que el sistema declara en puntos básicos se representan **exactas**, sin residuo: toda la desviación que queda en ellas es la del redondeo final.

### 📌 Alcance, dicho con precisión
* Esto retira la coma flotante de la **generación** de importes derivados —retenciones, comisiones, conversiones, intereses—, que es donde estaba el riesgo real.
* **No la retira de la entrada ni de la persistencia**: `Dinero::nuevo` sigue recibiendo `f64`, y SQLite sigue guardando `REAL`. Cerrar esa frontera es trabajo posterior.
* Ningún importe ya calculado cambia de valor. Las pruebas de caracterización pasan sin tocarse.

---

## 🚀 Versión 1.13.1 - 2026-09-16
**Caracterización del vertical de Ingresos: quince pruebas y cinco defectos documentados.**

### 🧪 Pruebas
* **Fase 4.1** fija la conducta vigente de facturas, cobros e ingresos informales antes de extraer nada.
* Cinco de las quince pruebas **documentan defectos, no aciertos**. Existen para que la extracción no los corrija por accidente y para que corregirlos sea después una decisión visible.

### 🔍 Defectos documentados (sin corregir)
* **H16** — La retención se redondea a **unidades**, no a céntimos: el 15 % de 1 234.56 queda en 185.00 en lugar de 185.18.
* **H17** — La cuenta de depósito se localiza por su nombre literal y el resultado se descarta: si no coincide, la factura queda cobrada y ningún saldo se mueve, sin aviso.
* **H18** — Cobrar una factura inexistente devuelve éxito.
* **H19** — El importe se acredita sin mirar la divisa de la cuenta: `ingresos` no tiene columna de divisa, y el destino puede ser una cuenta en dólares.
* **H20** — El borrado revierte con recorte a cero: si lo cobrado ya se gastó, la diferencia desaparece sin registro.

### 📌 Observación de fondo
* **Tres de los cinco son reapariciones** de defectos ya resueltos en otros verticales: H16 repite H8, H17 repite H3, y H20 repite H5 y H10.
* Que el mismo error viva en cuatro sitios distintos es más informativo que cualquiera por separado: no había nada compartido que impidiera repetirlo. Es el argumento del plan para extraer un dominio — que una regla exista **una sola vez** y su corrección alcance a todos.

---

## 🚀 Versión 1.13.0 - 2026-09-16
**El abono a tarjeta pasa a ser caso de uso, y llega la prueba Gherkin del encargo inicial.**

### 💳 Tarjetas de Crédito
* **Registro de abono extraído a caso de uso**:
  * El comando de Tauri queda reducido a traducción: las reglas viven en el dominio y el caso de uso, no en 86 líneas de SQL.
  * **La tasa de cambio se exige solo cuando las divisas difieren**, y se rechaza nombrando la causa. Antes, abonar en dólares desde una cuenta en pesos sin tasa llegaba hasta el adaptador y fallaba con un error de divisas incompatibles, que no dice qué falta.
  * El abono se guarda **completo, con su vínculo a la comisión**. El comando anterior lo insertaba y lo enlazaba después con un `UPDATE`: entre las dos sentencias existía una fila sin vínculo.

### ⚠️ Cambio de conducta declarado (H15)
* El comando aplicaba la tasa **aunque no hubiera cambio de divisa**: un abono en pesos desde una cuenta en pesos con una tasa de 60 debitaba sesenta veces el importe.
* Era conducta conocida y conservada a propósito en la Fase 1, pero **ninguna prueba la cubría** y el campo de tasa está siempre visible en la interfaz, así que era alcanzable tecleando.
* Deja de ocurrir. Queda declarado y fijado por prueba en lugar de pasar inadvertido.

### 🧪 Pruebas
* **Prueba Gherkin de caso de uso**, pedida en el encargo inicial y pendiente desde entonces: cuatro escenarios en español sobre el abono multidivisa.
  * Es la única prueba del proyecto **legible sin saber Rust**, para que quien decide si una regla es correcta pueda comprobarlo sin fiarse de la traducción.
  * **Intérprete propio en lugar de la caja `cucumber`**: el proyecto mantiene cero dependencias nuevas desde la Fase 0, y cuatro escenarios caben en un archivo auditable de cabo a rabo.
  * Tres salvaguardas contra una prueba que pase por no ejecutar nada: el intérprete tiene sus propias pruebas, un paso no reconocido entra en pánico, y una prueba estructural exige que los cuatro escenarios existan con sus pasos.

---

## 🚀 Versión 1.12.0 - 2026-09-14
**Los abonos a tarjeta se pueden deshacer.**

### 💳 Tarjetas de Crédito
* **Reversión de abonos**:
  * Cada tarjeta muestra sus abonos registrados y permite deshacer uno. La operación repone la deuda, devuelve a la cuenta el importe con su comisión y elimina el gasto que la recogía.
  * La confirmación **enumera los tres efectos**, porque un abono no es una fila: deshacerlo mueve tres cosas a la vez.
  * **Sin recorte en ninguno de los dos extremos.** Si el abono había dejado saldo a favor, deshacerlo lo devuelve exactamente a cero; si deja la tarjeta por encima de su límite, ese es el estado verdadero. Es la misma resolución que cerró H5 y H10.
  * Un abono registrado sin cuenta no movió ningún saldo de ahorro, y la interfaz lo dice en lugar de dejar esperando una devolución que no va a llegar.

### 🗄️ Base de Datos
* **Migración 5 — vínculo del abono con lo que lo pagó**: `pagos_tarjeta` gana `cuenta_ahorro_id`, `tasa_cambio` y `gasto_comision_id`.
  * Un abono guardaba solo la tarjeta, la fecha, el importe y la divisa. Todo lo demás que movía quedaba fuera, y por eso **no existía reversión: no había forma de saber qué deshacer**.
  * Los abonos anteriores se vinculan con su comisión mediante una reconstrucción **exacta**, no aproximada: la comisión es el 0.20 % del importe ya convertido, de modo que conociendo la tasa se reconstruye el céntimo. La migración **verifica que ninguna comisión quedó atada a dos abonos** y falla si no puede garantizarlo.
  * La tasa vivía dentro del texto de la descripción de la comisión —el mismo defecto que se corrigió en los gastos (H9)—. Se lee una sola vez, para rellenar la columna, y nunca más.

---

## 🚀 Versión 1.11.0 - 2026-09-14
**Las cuentas declaran su entidad y su tarifa por pago de impuestos; la exención alcanza a la DGII.**

### 🏦 Cuentas de Ahorro
* **Entidad y tarifa declarables**:
  * Cada cuenta puede indicar con qué entidad se mantiene y qué comisión fija cobra esa entidad por el servicio de pago de impuestos.
  * Ambos datos son opcionales y nacen vacíos. Dejar la tarifa en blanco significa **que no hay ninguna pactada**, que no es lo mismo que declarar cero: la distinción se conserva de extremo a extremo y decide si la operación paga el porcentaje ordinario o el precio fijo del servicio.
* **Edición de cuentas**:
  * Se añadió la corrección de nombre, entidad y tarifa sobre cuentas ya registradas. Sin ella los datos nuevos habrían quedado fuera del alcance de las cuentas existentes, que son todas.
  * La edición **no ofrece el balance**: moverlo sin un asiento detrás sería la única vía por la que un saldo cambiaría sin dejar rastro.

### 🏛️ Cargos e Impuestos
* **Comisión fija por pago de impuestos**:
  * Cuando la cuenta de origen declara una tarifa y el gasto pertenece a la categoría de impuestos, se cobra ese importe fijo en lugar de que la operación quede sin comisión.
  * Es un **precio de servicio**, no un impuesto: se acumula con la comisión del carril LBTR si la operación además lo usa, y no crece con el monto.
  * La tarifa concreta no está escrita en el programa. El sistema expresa que esa clase de tarifa existe y cuándo se aplica; cuánto cobra cada entidad es un dato del titular.
* **Exención ampliada a la DGII**:
  * La exención de la retención del 0.20 % pasa a reconocer tanto a la TSS como a la DGII. Sigue exigiendo las dos condiciones a la vez —categoría de impuestos y mención del organismo— porque la categoría por sí sola incluiría pagos a terceros que sí retienen.

### 🗄️ Base de Datos
* **Migración 4 — identidad y comisiones de las cuentas**: añade `entidad` y `comision_pago_impuestos` a `cuentas_ahorro`, ambas nulas.
  * No se rellenan automáticamente **a propósito**: deducir la entidad a partir del nombre que el titular dio a cada cuenta exigiría escribir en el repositorio el mapa de con qué bancos opera.

---

## 🚀 Versión 1.10.0 - 2026-09-13
**Todos los importes caen en un centavo exacto, y la conversión se verifica en vez de darse por buena.**

### 💰 Precisión de los importes
* **Los importes se normalizan al centavo**:
  * El núcleo trabaja en centavos enteros desde la reestructuración, pero la base seguía guardándolos como números con coma. Eso permitió que se colara un tercer decimal —residuo de cuando la retención se calculaba en varios sitios con criterios de redondeo distintos—: el gasto quedaba con fracción de centavo y el saldo de la cuenta heredaba la deriva al debitarse.
* **La conversión se acepta solo cuando es verificable**:
  * Al terminar, la migración comprueba que no queda ni un importe fuera de centavo y falla si lo hay. Una diferencia sin explicar detiene el proceso en lugar de quedar absorbida por el redondeo.
* **Las tasas conservan su precisión**:
  * Tasas de cambio, tasas de interés y porcentajes de retención quedan fuera de la normalización. No son importes, y redondearlos a dos decimales los inutilizaría para reconstruir la operación que documentan.

---

## 🚀 Versión 1.9.0 - 2026-09-13
**Migraciones versionadas: la preparación del esquema deja de declarar éxito cuando falla.**

### 🗄️ Preparación del almacenamiento
* **La base lleva su versión de esquema**:
  * Se registra dentro del propio archivo. Sin ella no había forma de distinguir «este cambio ya se aplicó» de «este cambio no se pudo aplicar», que era la ambigüedad que hacía razonable descartar los errores.
* **Los errores dejan de descartarse**:
  * Antes los cambios de esquema se aplicaban ignorando cualquier fallo. El patrón nació para tolerar una condición esperada —la columna ya existe— pero descartaba también los fallos reales, de modo que la aplicación podía arrancar contra un esquema incompleto y el problema aparecía después, en la primera consulta que tocara una columna ausente.
  * Ahora la condición esperada se comprueba de antemano y cualquier otro fallo se propaga indicando qué migración y qué etapa lo produjeron.
* **Cada migración se aplica entera o no se aplica**:
  * Los cambios y el registro de la nueva versión ocurren juntos. Un fallo a mitad deja la base en la última versión completa, no en un estado intermedio sin nombre.
* **Una base más nueva que la aplicación se rechaza sin tocarla**, con un aviso que indica actualizar la aplicación.
* **Las semillas se separan de las migraciones**: los registros que el sistema necesita para operar dejan de ir mezclados con los cambios de estructura y las transformaciones de datos históricos.

---

## 🚀 Versión 1.8.0 - 2026-09-13
**Revertir una transferencia vuelve a ser exacto, y el dinero deja de poder aparecer o desaparecer entre cuentas.**

### 🔁 Transferencias
* **La reversión deshace exactamente lo que hizo la transferencia**:
  * Antes el origen recuperaba el importe completo pero al destino solo se le descontaba hasta dejarlo en cero. Si el destino ya había gastado parte de lo recibido, la diferencia no se descontaba y **el patrimonio quedaba inflado**.
  * Revertir significa que la transferencia nunca ocurrió, de modo que ambas cuentas vuelven al estado que tenían. Si el destino queda en negativo, eso informa de que faltan movimientos por registrar en esa cuenta, y ocultarlo tras un cero borraba justamente esa señal.
* **Los importes deben cuadrar cuando no hay cambio de divisa**:
  * Salir un importe y entrar otro distinto hacía aparecer o desaparecer dinero entre las dos cuentas sin que nada lo advirtiera. La diferencia solo puede ser una comisión, y para eso existe el campo de cargo; el mensaje de error lo indica.

### 🏦 Cuentas
* **Una cuenta que participa en transferencias no se elimina**:
  * Antes se eliminaba y su historial de transferencias se iba con ella en silencio, dejando además a la contraparte con el dinero recibido sin constancia de dónde había salido.

---

## 🚀 Versión 1.7.0 - 2026-09-13
**Transferencias entre cuentas con invariantes propias, y aviso de divisa en el formulario.**

### 🔁 Transferencias
* **Una transferencia a la misma cuenta deja de aceptarse**:
  * Antes se registraba: restaba el importe más el cargo y sumaba el importe sobre la misma fila, dejando el saldo alterado por el cargo y un asiento que no representaba ningún movimiento. Ahora no se puede ni construir.
* **Los importes van en la divisa de su cuenta, por construcción**:
  * Ningún camino del código puede acreditar un importe en una divisa que no sea la de la cuenta que lo recibe.
* **Aviso al cruzar divisas**:
  * El formulario rotula cada importe con la divisa de su cuenta y advierte cuando el débito y el crédito van en divisas distintas. Es la mitad del problema que el código no puede resolver: los importes se teclean como números sueltos y su divisa se deduce de la cuenta elegida, así que no hay ninguna declaración que el sistema pueda contradecir.
* **La caja de efectivo sigue sin poder eliminarse**, ahora comprobado antes que la guarda de gastos para que el rechazo explique el motivo real.

---

## 🚀 Versión 1.6.1 - 2026-09-13
**Respaldo consistente y verificado antes de modificar el esquema.**

### 🛟 Respaldo de la base
* **Copia consistente en lugar de copia del archivo**:
  * El respaldo se toma con el mecanismo propio de SQLite, que escribe una base nueva y completa desde una vista coherente de la actual. Copiar el archivo directamente solo produce un respaldo válido si nadie está escribiendo, y eso no se puede garantizar desde fuera.
* **Verificación antes de darlo por bueno**:
  * Cada respaldo se abre y se comprueban su integridad y sus relaciones. Si no las supera, se descarta: una copia que nunca se ha abierto no es un respaldo, es un archivo.
* **Automático antes de tocar el esquema**:
  * Al preparar el almacenamiento se respalda primero, y **si el respaldo no se puede tomar la preparación se detiene** sin haber modificado nada. Una instalación nueva no genera respaldos, porque todavía no hay nada que perder, y una base que no ha cambiado desde el último tampoco.
* **Respaldo bajo demanda** desde Ajustes, para antes de conciliar saldos o cargar un estado a mano.
* Los respaldos se guardan fuera del directorio de la base, con marca de tiempo y motivo en el nombre, conservando los diez más recientes.

---

## 🚀 Versión 1.6.0 - 2026-09-13
**Financiamientos con saldo vivo, vinculados a la tarjeta que los cobra, y vista de pasivos agrupada por acreedor.**

### 🏦 Financiamientos
* **El pasivo deja de deducirse y pasa a llevarse**:
  * El saldo de cada financiamiento se registra y se actualiza, en lugar de calcularse a partir del monto original y la fracción de cuotas pendientes.
  * Esa fórmula suponía amortización lineal —falsa en todo préstamo real, donde las primeras cuotas son casi todo interés— y en una línea de crédito ni siquiera se aplicaba: al no tener cuotas contadas, el pasivo quedaba fijado en el monto desembolsado y ningún pago lo movía.
* **Reparto de la cuota entre interés y capital**:
  * Al abonar, el saldo baja por el capital que la cuota amortiza, no por su importe íntegro. El interés del período se calcula sobre el saldo vigente y se redondea al centavo.
  * Se admite el capital negativo —cuota que no cubre el interés, deuda que crece— y el que excede el saldo. Ninguno se recorta: un saldo que se mueve en la dirección incómoda debe verse.
* **Libro de movimientos**:
  * Cada cuota queda asentada con su desglose y el saldo resultante, de modo que el saldo pueda reconstruirse en lugar de ser un número que muta sin rastro.
* **Conciliación contra el estado de cuenta**:
  * Nueva acción para fijar el saldo al que declara el acreedor. La estimación cuota a cuota nunca cuadra al centavo —comisiones, seguros y días de gracia quedan fuera de la fórmula—, y la corrección se asienta con la diferencia que introdujo en vez de aplicarse en silencio.
* **Edición de condiciones**:
  * Tasa, cuota, día de pago, límite y tarjeta vinculada son corregibles sin borrar y recrear el registro, que destruía el libro de movimientos.
  * El monto original no se reescribe por ser un hecho histórico, y el saldo solo se mueve por su vía propia.
* **Líneas de crédito revolventes**:
  * Registro del límite aprobado y cálculo del cupo disponible como la diferencia con el saldo, que es lo que hace visible que abonar libera capacidad de disponer.

### 💳 Facilidades acopladas a una tarjeta
* **Vínculo entre un financiamiento y la tarjeta que lo cobra**:
  * Algunas facilidades no son productos independientes: viven bajo una tarjeta, comparten su ciclo y se cobran dentro de su pago.
  * Al vincularlas, el día de corte y el de vencimiento se derivan de la tarjeta en lugar de guardarse por duplicado, porque dos copias mantenidas a mano se desincronizan.
* **Aviso de doble conteo**:
  * El saldo de una facilidad y el balance de su tarjeta pueden solaparse. La aplicación lo advierte en el punto donde se decide qué cifra registrar, junto al abono y a la conciliación.

### 🎨 Vista de pasivos
* **Agrupación por acreedor**:
  * Las facilidades se dibujan dentro de la tarjeta que las cobra, sangradas y unidas por una guía; los financiamientos sueltos van en su propio grupo. El vínculo pasa a ser posición en vez de una línea de texto.
* **Resumen de cabecera**:
  * Pasivo total, carga mensual, cupo disponible y próximo vencimiento, con el desglose por naturaleza del pasivo en una sola barra.
* **Menú de acciones por fila**, que releva a los cuatro botones que ocupaban cada línea del listado.

### 🛠️ Correcciones de conducta
* **La caja de efectivo se identifica por su papel, no por su nombre**:
  * Se localizaba con una comparación de texto literal. Si la fila se renombraba o se eliminaba —cosa que la guarda de borrado no impedía, porque ningún gasto la referenciaba por identificador—, el gasto quedaba registrado sin mover ningún saldo y la operación devolvía éxito.
  * Los gastos en efectivo históricos reciben la referencia real que les corresponde y su ausencia pasa a ser un error visible.
* **Un pago con tarjeta exige indicar la tarjeta**:
  * Antes, un gasto con método «tarjeta» y sin tarjeta indicada se registraba igual y ninguna deuda se incrementaba.

---

## 🚀 Versión 1.5.0 - 2026-09-09
**Conversión de divisa con tasa declarada, bonificaciones, edición de suscripciones y saldo a favor en tarjetas.**

### 💱 Conversión de divisa y liquidación
* **Gasto en divisa con tasa declarada**:
  * Un consumo en una divisa distinta a la de la cuenta que lo paga se registra indicando la tasa aplicable, y la tasa deja de vivir dentro del texto de la descripción para ser un dato propio.
* **Consumos pendientes de liquidación**:
  * Un consumo cuyo importe definitivo aún no ha fijado el emisor se registra como pendiente y se cierra después indicando el importe cargado; la tasa se deduce de él, porque el emisor comunica cuánto cargó y nunca a qué tasa lo hizo.
  * Se contemplan dos políticas de liquidación según el emisor: la que mantiene el cargo en la divisa de origen y la que lo traduce a moneda local.

### 🎁 Bonificaciones
* **Registro de cashback, promociones y recompensas** como crédito independiente aplicado a una tarjeta, que es la forma en que los emisores los acreditan: no como un descuento sobre el consumo sino como un abono posterior.

### 💳 Tarjetas
* **Límite ajustado por el titular**:
  * Algunas emisoras permiten fijar un límite propio por debajo del aprobado. El cupo efectivo pasa a ser el menor de los dos.
* **El balance admite saldo a favor**:
  * Se retira el recorte a cero que impedía que la deuda bajara de cero, tanto al revertir un gasto como al registrar un abono.
  * Ese recorte descartaba en silencio el exceso: bastaba abonar más que el balance —al pagar el balance del corte mientras entran consumos nuevos— para que la diferencia desapareciera sin registro. Un balance negativo significa ahora lo que significa en la realidad, que el titular pagó de más.
  * Registrar y revertir vuelven a ser operaciones inversas exactas desde cualquier balance de partida.

### 🔄 Suscripciones
* **Edición de suscripciones** conservando la marca de idempotencia, de modo que corregir una no provoque un cargo duplicado en el ciclo.

### 🔒 Privacidad del repositorio
* Retirada de rutas locales, detalles de configuración de acceso y datos de autoría del repositorio público; los importes de las pruebas pasan a ser sintéticos, conservando la propiedad matemática que cada prueba verifica.

---

## 🚀 Versión 1.4.0 - 2026-09-08
**Pago al corte, unificación del redondeo al centavo y núcleo de dominio con pruebas de caracterización.**

### 💳 Tarjetas
* **Pago al corte y pago del balance actual**:
  * El abono a una tarjeta puede fijarse al balance del corte o al balance vigente, sin teclear la cifra.

### 🏛️ Cargos y retenciones
* **Una sola política de redondeo para la retención del 0.20 %**:
  * La retención se calculaba en tres lugares distintos con dos criterios de redondeo, que era el origen de una deriva de fracciones de centavo en los saldos. Pasa a cobrarse al centavo desde un único punto.
* **La exención impositiva de la TSS no exime de la comisión de servicio**:
  * Se ratificó que la retención por transferencia es un tributo del que la TSS está exenta, mientras que la comisión LBTR es un cargo por servicio que sí aplica. Son dos conceptos con reglas distintas y así quedan separados.

### 🏗️ Arquitectura y pruebas
* **Núcleo de dominio con importes en centavos enteros**, que elimina la aritmética en coma flotante de los saldos y concentra en un solo punto el único cálculo donde puede perderse precisión.
* **Puertos de repositorio, adaptador SQLite y suite de contrato compartida**, verificada contra la implementación real y contra un doble en memoria.
* **Pruebas de caracterización** que fijan la conducta vigente antes de modificarla, para que ningún cambio de comportamiento pase inadvertido durante la reestructuración.

---

## 🚀 Versión 1.3.5 - 2026-08-17
**Alta de tarjetas de crédito con límites bimoneda y respaldos automáticos pre-migración.**

### 💳 Módulo de Tarjetas y Registros
* **Registro de Tarjeta con Configuración Bimoneda**:
  * Soporte para dar de alta tarjetas de crédito indicando entidad emisora y nombre del producto.
  * Configuración de límites de crédito independientes por divisa (DOP y USD) sobre un mismo plástico.
  * Parametrización del ciclo de facturación: día de corte mensual y día límite de pago en el mes posterior.
* **Seguridad de Base de Datos**:
  * Ejecución de respaldo atómico de seguridad previo a toda modificación del esquema SQLite.

---

## 🚀 Versión 1.3.4 - 2026-07-29
**Conversión de divisa con tasa de cambio en abonos a tarjetas y normalización de visualización en USD.**

### 💳 Módulo de Tarjetas y Conversión
* **Tasa de Cambio en Abonos**:
  * Al realizar un abono en Dólares (USD) a una tarjeta de crédito debitando desde una cuenta en Pesos (DOP), el sistema ahora solicita al usuario la tasa de cambio aplicable.
  * Realiza la conversión de forma atómica: debita el equivalente exacto en DOP (`monto * tasa_cambio`) de la cuenta de ahorros y acredita la suma en USD a la tarjeta de crédito.
  * Registra la comisión bancaria del 0.20% calculada sobre el total en Pesos (DOP) de la transacción, guardándolo como un gasto bancario local.

### 📊 Análisis de Egresos
* **Normalización de Visualización en Dólares**:
  * Se corrigió la mezcla de divisas en la tarjeta de resumen mensual de la vista de Gastos.
  * Ahora calcula y muestra de forma independiente las métricas en Pesos (DOP) y Dólares (USD) (Monto Neto, Comisiones y Total Debitado) evitando sumas incorrectas multitasa.

---

## 🚀 Versión 1.3.3 - 2026-07-28
**Filtro histórico mensual y desglose de gastos por categoría.**

### 📊 Análisis de Egresos
* **Desglose de Gastos por Categoría**:
  * Se implementó un panel visual premium e interactivo dentro de la vista de Gastos para mostrar la distribución porcentual y total de egresos por categoría.
  * Integra barras de progreso de color dinámico según la naturaleza de la categoría (comida, impuestos, comisiones bancarias, etc.).
* **Filtro Temporal Dinámico**:
  * Incorporación de un selector desplegable de mes/año que se alimenta dinámicamente de todas las transacciones históricas registradas.
  * Permite auditar y realizar el seguimiento retroactivo del comportamiento del gasto a lo largo del tiempo de manera rápida y legible.

---

## 🚀 Versión 1.3.2 - 2026-07-15
**Selector de cuenta para cobros y flujos de caja automáticos en ingresos informales.**

### 📈 Ingresos e Interfaz
* **Selección de Cuenta de Recepción**:
  * Se reemplazó el campo de texto de entrada libre para "Banco de Depósito" en los modales de cobro (formal e informal) por un selector desplegable dinámico que muestra todas las cuentas de ahorro y efectivo registradas en el sistema.
* **Flujo de Caja Automatizado en Ingresos**:
  * Al marcar un ingreso (formal o informal) como pagado/cobrado, el balance de la cuenta de ahorro o de efectivo seleccionada se incrementa automáticamente con el monto recibido.
* **Ajuste de Retenciones**:
  * Se ratificó que bajo el flujo de cobros de la categoría informal no se aplican retenciones automáticas de impuestos, recibiendo el monto neto de forma íntegra.

---

## 🚀 Versión 1.3.1 - 2026-07-14
**Corrección responsiva de grillas y KPI boxes.**

### 🎨 Diseño y Usabilidad (Hotfixes)
* **Grillas Responsivas Dinámicas:**
  * Se reemplazaron las columnas fijas `repeat(X, 1fr)` en las clases `.grid-3`, `.grid-2` y `.kpi-row` por estructuras dinámicas `repeat(auto-fit, minmax(..., 1fr))`.
  * Esto corrige el comportamiento estático de los layouts y los desbordamientos visuales de datos y KPI boxes en pantallas más pequeñas o compactas.

---

## 🚀 Versión 1.3.0 - 2026-07-14
**Gestión de caja y efectivo, excepciones impositivas de transferencias y herramientas de corrección de transacciones.**

### 💵 Módulo de Caja y Efectivo
* **Nueva Pestaña de Efectivo (💵):**
  * Visualización dedicada de balances de caja en DOP (`Efectivo DOP`) y USD (`Efectivo USD`).
  * **Carga de Saldo de Caja:**
    * **Entrada Informal:** Permite registrar un flujo informal cobrado directamente en efectivo, incrementando de inmediato el balance en caja.
    * **Retiro desde Cuenta:** Permite retirar fondos de una cuenta bancaria y cargarlos a efectivo, debitando el banco y sumándolo a la caja mediante una transferencia entre cuentas.
  * **Descuento de Gastos en Efectivo:** Los gastos registrados con el método de pago *Efectivo* ahora se descuentan automáticamente del balance de la caja en su respectiva divisa.

### 🏛️ Excepción Impositiva (TSS)
* **Gastos de Impuestos Exentos:**
  * Se implementó una excepción nativa para transacciones de la categoría "Impuestos" con descripción "TSS".
  * Los pagos vía transferencia para este tipo de gastos ya no cargan la comisión bancaria automática del 0.20%.

### 🛠️ Corrección de Transacciones
* **Reversión y Ajustes de Balance:**
  * Nuevo panel integrado en la pestaña de **Ajustes** para auditar y revertir transacciones mal registradas.
  * Admite la eliminación y reversión automática de saldos para:
    * **Gastos:** Restaura el balance de la cuenta de ahorro (transferencias), tarjeta de crédito (cargos) o efectivo (caja).
    * **Transferencias bancarias:** Devuelve el monto original y cargos a la cuenta de origen y debita la cuenta destino.
    * **Ingresos (Formales e Informales):** Descuenta el saldo si ya había sido cobrado/depositado.

### 🏗️ Backend y API Nativas
* **Comandos Tauri/Rust:**
  * Nuevos endpoints nativos: `crear_cobro_efectivo_informal`, `eliminar_gasto`, `eliminar_transaccion_cuenta`, `eliminar_ingreso_informal` y `eliminar_ingreso` para procesamiento atómico de reversiones y flujos de caja.

---

## 🚀 Versión 1.2.1 - 2026-07-13
**Corrección de superposición y optimización de diseño de Tarjetas de Crédito.**

### 🔧 Mejoras y Ajustes de Usabilidad
* **Reubicación de Formulario de Tarjetas:**
  * Remoción total del formulario "Registrar Nueva Tarjeta" de la pestaña de Tarjetas para evitar superposición.
  * Reubicación del formulario en la sección de **Ajustes** como una tarjeta de configuración estándar.
  * Redirección automática de Ajustes a Tarjetas al registrar exitosamente una nueva tarjeta.
* **Optimización Responsiva Global:**
  * Ajuste de anchos mínimos de formulario a `300px` y corrección de desbordamientos con `min-width: 0`.
  * Incremento del breakpoint responsivo del grid a `1200px` para apilar secciones verticalmente y evitar colisiones visuales en la ventana por defecto de Tauri (1100px).
* **Empaquetado Limpio:**
  * Configuración del build para generar un único instalador `.dmg` en macOS.

---

## 🚀 Versión 1.2.0 - 2026-07-13
**Nuevas características de control automatizado, multidivisa y simplificación de flujos.**

### 📈 Facturación e Ingresos
* **Integración del Catálogo de Clientes:**
  * Reemplazo del campo de texto libre por un selector dinámico de clientes registrados.
  * Formulario de alta, consulta y eliminación de clientes integrado en la sección de **Ajustes**.
* **Auto-incremento Inteligente:**
  * El número de factura lee el último registro en base de datos e incrementa el contador automáticamente (ej. `FAC-0023` ➔ `FAC-0024`).
  * Autocompletado de la fecha actual en formato standard (`dd/mm/aaaa`).
* **Edición y Corrección:**
  * Botón de edición (✏️) en el listado de facturas emitidas para corregir montos, fecha, número o cliente, recalculando comisiones y retenciones del 15% automáticamente.

### 🏦 Cuentas de Ahorro
* **Nueva Pestaña de Cuentas (🏦):**
  * Resumen gráfico de balances totales agregados por divisa (Pesos DOP y Dólares USD).
  * Panel de gestión de cuentas (alta y baja) bajo **Ajustes**.
  * Formulario de transferencias entre cuentas y cambio de divisas (Pesos <-> Dólares) registrando tasa de cambio y comisiones de forma histórica.
* **Integración con Gastos:**
  * Al registrar un gasto como *Transferencia*, el sistema requiere elegir la cuenta de ahorro de origen y debita el balance real de forma automática.
  * Cálculo e inclusión automatizada de comisiones LBTR (+100.00 DOP) o transferencias estándar (0.2%).

### 💳 Tarjetas de Crédito Normalizadas (Multidivisa)
* **Doble Límite y Sobregiro:**
  * Soporte en base de datos para límites de crédito y límites de sobregiro (*overdraft*) independientes para DOP y USD.
  * Visualización separada de balances actuales, disponibles totales (`límite + sobregiro - uso`) y porcentajes de uso.
* **Balance al Corte:**
  * Registro y visualización del balance al corte en ambas divisas.
  * Si el saldo al corte es cero, se muestra un indicador visual `✔️ Tarjeta al día`.
* **Abonos Rápidos Multidivisa:**
  * Permite registrar abonos rápidos seleccionando si el pago se realiza en Pesos (DOP) o Dólares (USD).
* **Modal de Configuración:**
  * Opción dedicada para actualizar límites, sobregiros y balances de corte de cualquier tarjeta.

### 🔄 Suscripciones Recurrentes Automatizadas
* **Facturación al Inicio:**
  * Rutina asíncrona nativa `procesar_suscripciones()` ejecutada en segundo plano al abrir la aplicación.
  * Carga automática a la tarjeta de crédito asignada y registro del gasto en la fecha correspondiente según el día de facturación configurado.
  * Soporte multidivisa (DOP/USD) y notificaciones interactivas en pantalla (*toasts*) al procesar cargos con éxito.

### 🏗️ Base de Datos y Backend
* **Migración Automática:**
  * Script en `db_sql.rs` para realizar la migración transparente de la base de datos SQLite sin pérdida de información de las versiones anteriores.
  * Nuevas tablas `cuentas_ahorro` y `transacciones_cuentas`. Modificaciones en las tablas `tarjetas`, `gastos`, `pagos_tarjeta` y `suscripciones`.

### 🔧 Mejoras y Ajustes de Usabilidad (Hotfixes)
* **Flujo de Caja (Balance Mensual):**
  * Cambio del criterio del balance mensual y el dashboard principal para basarse en el monto efectivamente cobrado (flujo de caja real), manteniendo los montos facturados solo como referencia fiscal/impositiva.
* **Grillas Responsivas:**
  * Reemplazo de las grillas estáticas por la clase CSS responsiva `.responsive-split-grid` para evitar el colapso de los campos de entrada de datos en pantallas de tamaño reducido o con el menú expandido.
* **Resumen de Tarjetas de Crédito:**
  * Corrección del cálculo del balance global de tarjetas y de la vista destacada en el Dashboard para mostrar los saldos en Pesos y Dólares de manera independiente y alineada.
* **Inversiones y Certificados:**
  * Añadida opción para seleccionar el Tipo de Pago (*A cuenta* / *Reinversión* / *Reinversión compuesta*) al registrar certificados y activos de bolsa, visualizándolo en su respectivo listado.
* **Sección de Tarjetas Mejorada:**
  * El formulario de creación se reposicionó al final (a la derecha en desktop, abajo en móvil) para permitir revisar primero los saldos existentes antes de dar de alta nuevos productos.
  * Se añadió un banner interactivo que sugiere dinámicamente qué tarjeta de crédito usar hoy según cuál fecha de corte está más lejana en el futuro, maximizando el período de financiamiento sin intereses.
* **Abonos a Tarjetas desde Cuentas:**
  * Se rediseñó el formulario de abonos rápidos de tarjetas de crédito permitiendo seleccionar una cuenta de ahorro de débito origen.
  * El sistema descuenta de forma automática el importe del abono de la cuenta seleccionada más una comisión por transferencia bancaria del 0.20%, registrando la comisión como un gasto bancario.

---

## 🚀 Versión 1.1.0 - 2026-06-15
**Módulo de inversiones en bolsa, capital físico e institucional, y financiamientos de largo plazo.**

### 🏢 Capital y Activos (NoSQL)
* **Capital Físico:**
  * Registro de bienes inmuebles, vehículos y maquinaria corporativa con valores estimados.
* **Inversiones Financieras:**
  * Registro de certificados de depósito e inversiones en bolsa de valores.
  * Alertas visuales inteligentes de vencimiento (10 días de anticipación) calculadas de forma dinámica.

### 🏷️ Financiamientos y Préstamos
* **Préstamos Institucionales:**
  * Seguimiento de préstamos con cuotas fijas y flexibles.
  * Control del número de cuotas totales, pendientes e importe de la cuota.
  * Botón rápido para abonar / pagar cuota mensual reduciendo las cuotas pendientes automáticamente.

---

## 🚀 Versión 1.0.0 - 2026-05-10
**Lanzamiento inicial de MiChelitosTauri.**

### 📊 Dashboard e Interfaz macOS
* **Diseño macOS Native:**
  * SPA diseñada con tema oscuro por defecto y soporte para modo claro/oscuro.
  * Panel de control principal con indicadores de gastos totales del mes, ingresos esperados y balance general.
  * Gráficos simplificados de distribución presupuestaria por categoría de egresos.

### 📉 Control de Egresos
* **Registro de Gastos:**
  * Formulario de egresos clasificando por categoría y método de pago (efectivo, tarjeta).
  * Catálogo dinámico de categorías administrable desde Ajustes.

### 📈 Ingresos Formales e Informales
* **Ingresos Formales:** Registro de facturas con cálculo automático de retenciones de impuestos.
* **Ingresos Informales:** Registro rápido de flujos de efectivo informales.
* **Mecanismo de Cobro:** Formulario modal para registrar el banco receptor y la fecha efectiva de cobro.
