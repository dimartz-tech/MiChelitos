# 📝 Archivo de Control y Aprendizajes - MiChelitosTauri

Este archivo sirve como registro de control para documentar los cambios realizados, las lecciones aprendidas y las reglas de diseño para evitar regresiones o repetir errores en futuras actualizaciones del proyecto.

---

## 🛠️ Registro de Cambios (Versión 1.3.5)

### 1. Alta de Tarjeta de Crédito con Límites Bimoneda
* **Objetivo**: Permitir el registro de tarjetas de crédito con límites independientes por divisa y ciclo de facturación personalizado.
* **Parámetros de configuración soportados**:
  * Entidad emisora — texto libre
  * Nombre / producto de la tarjeta — texto libre
  * Límite DOP y Límite USD — independientes entre sí
  * Fecha de corte — día del mes (1–31)
  * Fecha límite de pago — día del mes siguiente (1–31)
* **Protocolo de Respaldo**: Creación de una copia de seguridad atómica de la base SQLite, con marca de tiempo, previa a toda modificación de esquema o alta de producto financiero.

> **Nota de privacidad**: este archivo documenta *capacidades* del sistema. Los valores concretos de entidades, límites y ciclos de facturación son datos del usuario y residen únicamente en la base de datos local (`~/.michelitos/`), nunca en el repositorio.

---

## 🛠️ Registro de Cambios (Versión 1.3.4)

### 1. Excepción Impositiva (TSS)
* **Objetivo**: Evitar el cargo del 0.20% por transferencias bancarias en gastos de la categoría "Impuestos" si la descripción incluye la palabra "TSS".
* **Solución**: Modificación en el backend Rust (`crear_gasto`). Se consulta dinámicamente si la categoría asociada al gasto es "Impuestos" (case-insensitive) y si el campo de descripción contiene "TSS" (case-insensitive). Si se cumplen ambas condiciones, la comisión se establece en `0.0`.
* **Fórmula de cálculo**:
  ```rust
  let is_tss_tax = cat_nom.to_lowercase() == "impuestos" && input.descripcion.to_uppercase().contains("TSS");
  if !is_tss_tax {
      costo_adicional = (input.monto * 0.002).round();
  }
  ```

### 2. Módulo de Caja y Efectivo
* **Objetivo**: Controlar el saldo físico de caja (Pesos y Dólares) sin alterar el esquema de base de datos.
* **Solución**: 
  * Se aprovechan los registros `'Efectivo DOP'` y `'Efectivo USD'` en la tabla `cuentas_ahorro`.
  * Se creó una vista dedicada de **Efectivo** en la barra lateral.
  * Los gastos registrados en "Efectivo" ahora descuentan del balance de caja de la divisa correspondiente en `cuentas_ahorro`.
  * **Carga de Saldo**:
    1. **Entrada Informal**: Registra un ingreso informal directamente en la caja con estatus "pagado" (suma a la cuenta de efectivo correspondiente).
    2. **Retiro desde Cuenta**: Transfiere saldo de una cuenta bancaria a la caja correspondiente utilizando el comando nativo `transferir_entre_cuentas`.

### 3. Modales de Cobro e Ingresos Automáticos
* **Objetivo**: Reemplazar la entrada de texto libre por una selección segura de cuentas bancarias y de efectivo al cobrar facturas e ingresos informales.
* **Solución**:
  * Se modificaron los modales de cobro formal e informal en `src/js/ui.js` para que sean asíncronos y carguen un desplegable `<select>` dinámico de `cuentas_ahorro`.
  * Al marcar un ingreso (tanto formal como informal) como pagado/cobrado, se actualiza automáticamente el balance de la cuenta seleccionada incrementándolo con el monto recibido.
  * Se modificó `marcar_ingreso_pagado` y `marcar_informal_pagado` en Rust para realizar esta operación de forma atómica dentro de una transacción SQLite.
  * Se ratificó que bajo el flujo de cobros de la categoría informal no se aplican retenciones impositivas automáticas.

### 4. Corrección y Reversión de Transacciones
* **Objetivo**: Posibilitar ajustes mediante la eliminación o reversión de transacciones mal registradas, reestableciendo los saldos previos.
* **Solución**:
  * Panel en **Ajustes** que lista gastos, ingresos (formales e informales) y transferencias recientes con botones para revertir.
  * Comandos nativos en Rust (`eliminar_gasto`, `eliminar_transaccion_cuenta`, `eliminar_ingreso_informal`, `eliminar_ingreso`) que ejecutan de forma atómica la reversión en balance y la remoción del registro.

### 5. Grillas Responsivas Dinámicas
* **Objetivo**: Resolver el error de diseño donde los datos y KPI boxes en el Dashboard, Gastos y Ajustes quedaban estáticos o se solapaban.
* **Solución**: Reemplazo de columnas rígidas en `src/css/style.css`.
  * Modificación de `.grid-3`, `.grid-2` y `.kpi-row` para autoajustarse:
    ```css
    .grid-3 { grid-template-columns: repeat(auto-fit, minmax(320px, 1fr)); }
    .grid-2 { grid-template-columns: repeat(auto-fit, minmax(350px, 1fr)); }
    .kpi-row { grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); }
    ```

### 6. Desglose de Gastos por Categoría y Filtro Mensual (Versión 1.3.3)
* **Objetivo**: Proveer una visualización analítica, clara y legible del comportamiento del gasto a lo largo del tiempo agrupado por categoría.
* **Solución**:
  * Se implementó un panel de desglose porcentual y numérico por categorías de gasto en la vista de **Gastos y Egresos**.
  * Incorpora barras de progreso que indican la proporción del gasto por categoría en el mes, con colores dinámicos inteligentes para destacar categorías críticas (impuestos, comida, comisiones bancarias, etc.).
  * Agrega un selector dinámico de Mes/Año que extrae cronológicamente todos los meses con transacciones de la base de datos para realizar búsquedas históricas y retroactivas de manera fluida.

### 7. Tasa de Cambio en Abonos y Separación de Resúmenes Multidivisa (Versión 1.3.4)
* **Objetivo**: Evitar descuadres financieros al abonar a tarjetas de crédito en Dólares (USD) desde cuentas corrientes o de ahorro en Pesos (DOP), y evitar la sumatoria directa de divisas distintas en los resúmenes mensuales.
* **Solución**:
  * **Abonos Multi-moneda en UI**: Se incorporó un campo de entrada visual ("Tasa Cambio") en el formulario de abono rápido de las Tarjetas para que el usuario ingrese la tasa de conversión directamente en la interfaz.
  * **Mecanismo de Respaldo (Fallback)**: Si se realiza un pago de USD con una cuenta DOP y el usuario deja el campo de tasa en `0.00` o vacío, el frontend dispara interactivamente un prompt de validación para requerir la tasa aplicable.
  * **Tipado de Backend Robusto**: Se modificó la firma en Rust para procesar `tasa_cambio` como un tipo primitivo `f64` (donde valores `> 0.0` indican conversión activa), previniendo los errores de deserialización de tipo `null` comunes en Tauri IPC.
  * **Visualización de Gastos**: El panel de resumen mensual de la sección de Gastos separa los totales (Neto, Comisiones y Debitado) en secciones independientes para DOP y USD si existen gastos en ambas monedas, solucionando el error que mezclaba importes de diferentes divisas.

---

## 💡 Lecciones Aprendidas y Reglas de Control

### 🚨 1. Bloqueos al compilar el instalador `.dmg` en macOS (Tauri)
* **Error presentado**: `failed to bundle project: error running bundle_dmg.sh`.
* **Causa**: Al empaquetar una aplicación con Tauri en macOS, el script `bundle_dmg.sh` monta imágenes de disco temporalmente. Si un archivo `.dmg` anterior está montado en `/Volumes/MiChelitos, o si el proceso falló previamente dejando la imagen virtual bloqueada, el sistema de archivos de macOS impide sobreescribirla.
* **Acción de control**: Antes de realizar cualquier compilación de producción (`tauri build`), se deben expulsar los volúmenes correspondientes en la terminal:
  ```bash
  hdiutil detach "/Volumes/MiChelitos" && hdiutil detach "/Volumes/MiChelitos 1" || true
  ```

### 🗄️ 2. Consistencia en Transacciones y Balances de SQLite
* **Regla**: Toda inserción o eliminación de transacciones que afecte los saldos de cuentas de ahorro o tarjetas debe ocurrir dentro de una **transacción SQL** (`conn.transaction()`).
* **Razón**: Si el guardado del gasto tiene éxito pero el descuento del balance de la cuenta de ahorros falla (o viceversa), la base de datos quedará en un estado inconsistente y los saldos descuadrados. Las transacciones SQLite garantizan que ambos ocurren o ninguno.

### 🎨 3. Diseño Responsivo en Ventanas Nativas (Tauri)
* **Regla**: Evitar el uso de `repeat(X, 1fr)` en layouts principales. Usar siempre `repeat(auto-fit, minmax(width, 1fr))`.
* **Razón**: La ventana por defecto de la aplicación en escritorio tiene un ancho fijo (ej. 1100px). Forzar varias columnas fijas causa colapso de los campos de texto e inputs de datos al colapsar o expandir el menú lateral. El uso de `auto-fit` permite que las columnas se apilen automáticamente de forma fluida.
