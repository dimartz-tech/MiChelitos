# Fase 5 — Suscripciones

La fase de la **idempotencia**: que abrir la aplicación dos veces el mismo día
no cobre dos veces.

Lo que había era una red de nueve pruebas (`s1`–`s9`) que cubría esa propiedad
y poco más, y una regla de decisión escrita dentro del bucle que mueve el
dinero. Lo que sigue es lo que se hizo y, sobre todo, **lo que se encontró al
poder mirarla**.

## El puerto `Reloj`, que existía sin usarse

`procesar_suscripciones` leía `Local::now()` por dentro. Eso no es un detalle
de estilo: **hacía imposible escribir la prueba que más falta hacía**.

Todas las pruebas anteriores usan el día 1 de facturación, porque es el único
que está siempre alcanzado sea cual sea la fecha real. De modo que la red
cubría el cobro y **no cubría la abstención**: no había forma de afirmar que
una suscripción del día 15 no se cobra el día 3.

El puerto `Reloj` y su adaptador estaban construidos desde la Fase 0 y no los
usaba nadie. Ahora el comando delega en `procesar_suscripciones_con(reloj)`, y
`s15` afirma lo que faltaba: catorce días seguidos sin cargo, y cargo el
quince.

## Seis defectos, medidos antes de escribirlos

Se replicó la condición exacta y se barrieron fechas, en vez de deducirlos
leyendo el código. Todos quedan **reproducidos y sin corregir**, cada uno
sujeto por una prueba que muere si alguien cambia la conducta:

| | Lo que hace hoy | Prueba |
|---|---|---|
| Mensual el **día 31** | ~~7 cargos en 12 meses~~ — **corregido**: el día se recorta al último del mes | `s10b` |
| Mensual el **día 30** | ~~11 cargos: se pierde febrero~~ — **corregido** | `s11b` |
| **Anual** | ~~Se recobra al cambiar el año~~ — **corregido**: la fecha se anota, no se deduce | `s12b` |
| **Períodos vencidos** | Tres meses sin abrir la aplicación generan **un** cargo, no tres | `s13` |
| Marca de cobro **ilegible** | ~~Cobra en cada arranque~~ — **corregido**, ver abajo | `s14b` |
| Sin la categoría **«Suscripciones»** ni **«Otros»** | El gasto cae en el identificador 1 literal, que hoy es «Alimentación» | `s17` |

**Tres de los seis quedan cerrados**, cada uno por decisión del titular y con
el estado de cuenta delante; los capítulos del final cuentan cómo. Siguen
abiertos los **períodos vencidos** (`s13`) y la **categoría de respaldo**
(`s17`).

### Por qué no se corrigen aquí

Los seis cambian importes que un proveedor ya cobró de verdad, y tres de ellos
no tienen una respuesta obvia:

* **¿Un mes saltado se recupera o se pierde?** Hoy se pierde. Recuperarlo
  cargaría varios meses de golpe sin que nadie lo pida.
* **¿Qué hace el día 31 en un mes de 30?** Cobrar el último día del mes es lo
  natural, y es una regla nueva, no un arreglo.
* **¿Qué se hace con una marca ilegible?** Lo prudente es lo contrario de lo
  actual —no cobrar y avisar—, porque un cargo de más cuesta más de deshacer
  que uno de menos.

Son decisiones del titular. Documentarlas y fijarlas con pruebas es lo que
permite tomarlas más tarde sin volver a descubrirlas.

## `dominio::suscripcion`

La regla sale del bucle y pasa a ser un tipo:

```rust
Suscripcion { frecuencia, dia_de_facturacion, ultimo_cobro }
    .corresponde_cobrar(hoy) -> bool
```

Tres decisiones de modelado que el código anterior no permitía expresar:

* **`MarcaDeCobro::Ilegible` es distinto de `Ninguna`.** No haber cobrado
  nunca y no entender la marca son estados opuestos —uno espera a su día, el
  otro no espera—, y confundirlos era lo que escondía el defecto.
* **La marca no guarda el día.** La regla no lo mira, y conservarlo sugeriría
  una precisión que la decisión no tiene. Que las tres partes se parseen y
  solo se usen dos es donde el defecto de los períodos vencidos se hace
  visible.
* **Una frecuencia desconocida no se interpreta.** `Frecuencia::desde_codigo`
  devuelve `None` y quien llama decide; hoy decide no cobrar, que es lo que
  hacía la cadena de `if` al no coincidir con ninguna rama.

## Verificación

447 pruebas en verde. La extracción no cambió una sola conducta: las nueve
pruebas anteriores y las siete nuevas pasan igual antes y después.

| Mutación | Resultado |
|---|---|
| El umbral del día pasa a estricto (`>`) | mueren 9 pruebas, entre ellas `s15` |
| La anual mira también el mes | mueren `s12` y su prueba de dominio |
| La marca ilegible deja de cobrar | muere `s14` |
| El respaldo de categoría cambia de 1 a 0 | muere `s17` |

La primera importa por lo que dice de la red anterior: **antes de inyectar el
reloj, esa mutación no habría matado nada**, porque ninguna prueba podía
distinguir un día de otro.

## Lo que queda

Las tres decisiones de arriba, y con ellas la corrección de los seis defectos.
Mientras tanto el cobro sigue sin pasar por `Dinero` —se lee como `f64` y se
carga a la tarjeta— y la divisa se decide con `if divisa == "USD"`, de modo
que cualquier otra cosa es pesos: el mismo patrón que `c16` declaró en gastos.


---

# El día que no existe en el mes

Los dos primeros defectos **se cierran**, por decisión del titular y con el
estado de cuenta delante.

## La regla, y las dos veces que se corrigió

El día de facturación se **recorta a los días que tiene el mes**. Una del 30
se cobra el 28 de febrero; una del 31, el 30 de abril.

Llegar ahí costó tres versiones, y las dos correcciones vinieron del titular:

1. **«El último día del mes»** — la suposición razonable de partida.
2. **«El 1 de marzo»** — al mirar el estado de cuenta. Se implementó así, y
   obligaba a rehacer la marca de idempotencia: si el cargo de febrero cae en
   marzo, marzo lleva dos vencimientos y «ya cobré este mes» los funde en uno.
3. **«El cargo se generó el 28/02 y se cobró el 1 de marzo»** — lo que
   reconcilia las dos anteriores. Son dos fechas distintas: **generación y
   liquidación**.

Esta aplicación asienta el **consumo** contra la tarjeta, no su liquidación,
así que el asiento lleva la fecha de generación: el 28. La liquidación entra
por el ciclo de pago de la tarjeta, que se lleva aparte.

La tercera versión resultó además la más pequeña: sin cambiar la marca de
idempotencia, porque el recorte **mueve el día, no añade vencimientos**. Cada
mes sigue teniendo uno.

## Lo que cambia en los datos reales

Dos suscripciones del titular facturan los días 29 y 30. Entre las dos, la
aplicación dejaba de asentar **USD 30,94 al año** que el proveedor sí cobraba.

Simulando 2027 sobre una copia de la base real: 98 cargos donde antes había
96, y febrero pasa a tener los dos que le faltaban, ambos fechados el
**28/02/2027**.

## Lo que sigue abierto

De los seis defectos quedan **dos**:

* **Varios períodos vencidos generan un solo cargo** (`s13`). Se comprobó al
  implementar: una formulación más general de la regla de febrero también
  recuperaba un período atrasado de rebote —dieciocho cargos en enero donde
  había ocho, sobre datos reales— y se descartó por eso. Recuperar períodos
  vencidos es una decisión, y no se toma de lado.
* **Sin la categoría esperada, el gasto cae en el identificador 1** (`s17`).

Y sigue pendiente que el cobro pase por `Dinero`: hoy se lee como `f64` y se
carga a la tarjeta directamente.

---

# La renovación anual, anotada

El defecto de la anual **se cierra**, por decisión del titular: en lugar de
deducir el vencimiento, se anota.

## Por qué el defecto existía

La condición era `anio_actual > p_anio && dia >= dia_facturacion`. No miraba
el mes, y no podía: del último cobro solo se guardaba una fecha que la regla
usaba a medias. **Intentaba deducir un vencimiento anual con un dato que no
bastaba**, y de ahí salía el cargo seis meses antes.

La corrección no es afinar la condición: es dejar de deducir. Una anual sabe
cuándo renueva porque el proveedor lo dice, y eso es un dato que se anota.

## Lo que cierra de paso

Una fecha es una fecha, así que en las anuales desaparecen también los otros
dos agujeros:

* El día 31 ya no las desvía.
* Una **marca de cobro ilegible** ya no las arrastra a cobrar. El agujero
  queda abierto solo en las mensuales, donde la marca sigue siendo el único
  dato.

## Las que ya existían

La migración 12 deriva la fecha del último cobro más un año, tomando el **día
de facturación y no el día en que se ejecutó el cargo**. Son dos cosas
distintas: el cargo se anota el día en que se abrió la aplicación, que puede
ser posterior. Sobre datos reales esa diferencia era de un día.

Sin último cobro, o con una marca ilegible, la columna queda vacía. **Una
anual sin fecha no se cobra.** Inventar una para poder cobrar sería el mismo
error que la migración corrige, y entre un cargo de más y uno de menos, el de
menos se corrige mirando el estado de cuenta.

## El aviso

Siete días antes, y hasta el propio día del cargo. Pasada la fecha deja de ser
un aviso y pasa a ser un cobro pendiente, de modo que se apaga.

**Solo las anuales avisan.** Una mensual tendría que predecir su próximo cobro,
y esa predicción arrastraría hoy el defecto del día 31: anunciar una fecha que
el sistema luego no respeta es peor que no anunciar nada. La columna «Próximo
Cobro» muestra un guion en las mensuales por esa razón, no por olvido.

El aviso se resuelve en el núcleo y llega a la vista como un `bool`. Una regla
en el HTML es una regla sin pruebas.


---

# La marca de cobro ilegible

El defecto que **invertía la propiedad que da nombre a la fase**: si la fecha
del último cobro no tenía tres partes separadas por `/`, la rama que decidía
hacía `requiere_cargo = true` sin más. El mecanismo de idempotencia se volvía
un duplicador, y cobraba en cada arranque.

## No cobrar, y decirlo

No se puede saber cuándo se cobró por última vez. Entre arriesgar un cargo de
más y uno de menos, **el de menos se corrige mirando el estado de cuenta**; el
de más hay que deshacerlo. Es el mismo criterio que la anual sin fecha de
renovación.

Pero no cobrar, a secas, cambia un defecto por otro: **una suscripción parada
en silencio es peor que una que cobra de más**, porque el cargo indebido
aparece en el estado de cuenta y la parada no aparece en ninguna parte.

De ahí `Impedimento`, que responde a una pregunta distinta de
`corresponde_cobrar`: no «¿toca hoy?» sino «¿llegará a cobrarse alguna vez?».
Unifica los dos estados parados —marca ilegible y anual sin renovación— y
lleva su propio texto, porque si cambia la regla el texto que la explica tiene
que cambiar con ella.

Un detalle que costó una corrección a media implementación: en una **anual**
con marca ilegible no hay impedimento. La anual decide con su fecha de
renovación y no mira la marca, así que señalarla sería decir que algo está
parado cuando no lo está. **Un aviso que miente se aprende a ignorar.**

## La salida

`s7` conserva `fecha_ultimo_pago` a propósito, porque reiniciarla provoca un
cobro duplicado. Esa misma preservación dejaba al titular sin salida cuando la
fecha guardada no se entiende.

`corregir_ultimo_cobro` es la única vía para escribirla a mano, y existe solo
por esto. **Pide la fecha en lugar de limpiarla**: borrarla la dejaría como
«nunca cobrada» y volvería a cobrar este mes, que es justo el duplicado del
que `s7` protege. Quien tiene el estado de cuenta delante sabe cuál es la
buena.

## Dos capas, y ninguna sobra

El dominio impide el daño. La **migración 13** cierra la puerta: un `CHECK`
con `GLOB` obliga a que las dos fechas de una suscripción tengan forma
`dd/mm/aaaa`.

Que hacía falta no es teórico. En la base real no había ninguna marca rota en
`suscripciones`, **pero sí una en `gastos.fecha`**: un `10/09/2026` tecleado
sin la primera barra. El estado es alcanzable; simplemente no había tocado aún
esta tabla.

Y el `CHECK` no basta solo: comprueba la **forma**, no que la fecha exista. Un
`31/02/2026` la tiene y no es un día. Por eso el analizador pasó a validar con
`from_ymd_opt` en vez de parsear tres partes y usar dos — así un `13/13/2026`
deja de leerse como «mes 13».

La migración **no toca `gastos.fecha`**: ahí hay un valor real que no cumple,
y corregir un dato del titular es decisión suya, no de una migración.
