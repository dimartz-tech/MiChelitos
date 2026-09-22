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
| Mensual el **día 31** | 7 cargos en 12 meses: febrero, abril, junio, septiembre y noviembre se saltan enteros | `s10` |
| Mensual el **día 30** | 11 cargos: se pierde febrero | `s11` |
| **Anual** | ~~Se recobra al cambiar el año~~ — **corregido**, ver abajo | `s12b` |
| **Períodos vencidos** | Tres meses sin abrir la aplicación generan **un** cargo, no tres | `s13` |
| Marca de cobro **ilegible** | Cobra en cada arranque, incluso antes de su día | `s14` |
| Sin la categoría **«Suscripciones»** ni **«Otros»** | El gasto cae en el identificador 1 literal, que hoy es «Alimentación» | `s17` |

El quinto es el más grave, porque **invierte la propiedad que da nombre a la
fase**: una fecha mal formada convierte el mecanismo de idempotencia en un
duplicador.

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
