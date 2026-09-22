# 📜 Historial de Versiones - MiChelitosTauri

Este archivo detalla la evolución de la aplicación de escritorio nativa macOS **MiChelitosTauri**, incluyendo características integradas, correcciones y cambios en la arquitectura de base de datos.

---

## 🚀 Versión 1.19.1 (Versión Actual) - 2026-09-22
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
