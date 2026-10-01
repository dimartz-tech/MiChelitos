# El capital y el dinero

Pregunta del titular (2026-10-01): *¿se podría integrar `capital` con `Dinero`?* Respuesta corta: **ya está integrado en el núcleo de Rust; lo que `capital` no puede ser es un movimiento de dinero como los demás flujos.** Este documento explica qué es y qué no es, y dónde quedan huecos.

## Qué es el capital
Un **inventario de patrimonio**: certificados financieros, inversiones en bolsa y bienes (inmuebles, vehículos, maquinaria). No es un registro de transacciones: no hay saldo que se mueva, ni cuenta que se debite o se acredite. Es **solo pesos** (`divisa_del_capital()` devuelve DOP), de modo que las reglas multidivisa de `Dinero` (nunca sumar DOP con USD) no se ejercen aquí.

## Cómo se relaciona con `Dinero` hoy

| Capa | Qué hace con los importes |
|---|---|
| **Rust, dominio y aplicación** (desde 1.34.0, #66) | **Es el único sitio que decide dinero.** `leer_monto` convierte cada importe a `Dinero` —desde **texto** tal cual se escribió, o desde número—; `validar_monto` exige que sea positivo y exacto al céntimo; `preparar_para_guardar` valida **solo lo nuevo o lo cambiado** frente a lo ya guardado; `totales_de` suma en **centavos enteros**; `obtener_capital` devuelve el documento con los `totales` añadidos. |
| **Almacén** | Un **archivo JSON** (`capital.json`), no SQLite. Los importes se guardan como **números** al céntimo, porque la aplicación instalada lee el mismo archivo. El respaldo lo incluye desde 1.36.0. |
| **Interfaz (la vista)** | **No hace ninguna operación de dinero.** Manda el importe como **texto** (convención de 1.21.0), la tasa como número (una tasa no es dinero), y lo formatea para mostrarlo. Los totales **no** se ven en esta pantalla: los muestran el Dashboard y el Resumen, tal como los calculó Rust. |
| **`nucleo/dinero.ts`** (formatear, sumar, totalizar por divisa) | **Ninguna vista lo usa.** Existe y tiene pruebas, pero la interfaz formatea con `formatMoney`. |

Por eso, en la interfaz, **no hay nada de `Dinero` que integrar**: el cálculo ya vive donde debe, y la vista solo transporta texto y muestra cifras. Integrar `nucleo/dinero.ts` en `capital` solo cambiaría quién formatea, sin ganar ninguna garantía.

## Lo que `capital` no puede integrar (y por qué no es un defecto)
* **No mueve saldos.** Un certificado no sale de ninguna cuenta ni se refleja en ella; por eso no pasa por `registrar_gasto`, ni por una transferencia, ni por los casos de uso con su puerto y su almacén en memoria.
* **No es transaccional con el resto.** Está fuera de SQLite: no entra en una transacción con cuentas, ni lleva `CHECK (ROUND(col, 2) = col)`, ni migraciones versionadas. La garantía de céntimo la da Rust al validar antes de escribir el JSON.

## Huecos que sí hay (hallados al extraer la vista; ninguno corregido aquí)

1. **Lectura-modificación-escritura del documento entero desde la vista.** Cada alta o baja hace `obtenerCapital()`, cambia el documento y lo devuelve con `guardarCapital()`. Es una **ventana de «gana la última escritura»**: dos ventanas o dos pulsaciones muy seguidas pueden pisarse. Rust valida lo que cambia, pero no puede saber que el documento que recibe ya está viejo.
2. **Los certificados y la bolsa se borran por posición, los bienes por identificador.** `splice(idx, 1)` con el índice con que se dibujó la lista: si el documento cambió entre dibujar y pulsar, **se puede retirar otro certificado**. Solo los bienes tienen `id`.
3. **Las tres bajas (`certificado`, `bolsa` y `bien`) no tienen `try/catch`.** Si Rust rechaza el guardado no hay aviso y la pantalla no se redibuja. Ya figuraba como hallazgo de 1.49.0 (pruebas `todo`).
4. **El identificador de un bien es `Date.now()`.** Dos bienes creados en el mismo milisegundo colisionarían; con una persona pulsando no ocurre, pero no es una garantía. Desde esta extracción el reloj es inyectable.

## Si se quisiera integrar más
La integración «a fondo» sería llevar las operaciones al núcleo como **comandos propios** (`agregar_certificado`, `retirar_certificado`, …) que reciben un `ImporteDecimal` y hacen la lectura-modificación-escritura **dentro de Rust**, con identificadores para todo. Cierra los huecos 1 a 3 de una vez y deja la vista sin documento que manipular. Cuesta: tres o seis comandos nuevos con sus pruebas de caracterización, y cambiar la vista (cuerpos que hoy construyen el documento). **No hace falta para dividir `ui.ts`** y conviene hacerlo **después**, con la vista ya aislada y probada, que es justo lo que esta extracción deja listo.

Una opción más ambiciosa, mover el capital a una tabla SQLite con importes en centavos enteros, solo se justificaría si se quisiera atomicidad con las cuentas; hoy no hay ninguna razón de dominio para ello.
