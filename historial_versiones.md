# 📜 Historial de Versiones - MiChelitosTauri

Este archivo detalla la evolución de la aplicación de escritorio nativa macOS **MiChelitosTauri**, incluyendo características integradas, correcciones y cambios en la arquitectura de base de datos.

---

## 🚀 Versión 1.6.0 (Versión Actual) - 2026-09-13
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
