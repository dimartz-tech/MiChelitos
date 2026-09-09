# 💱 Modelo de conversión de divisa

**Hallazgo asociado:** H9 — la tasa de cambio recibe tres tratamientos distintos
**Fecha:** 2026-09-09
**Estado:** MODELADO — no implementado

---

## 1. El problema, en dos capas

### 1.1 La tasa no se guarda de forma uniforme

| Operación | ¿Convierte? | ¿Guarda la tasa? |
|---|---|---|
| Transferencia entre cuentas | Sí | **Sí**, columna `tasa_cambio` |
| Abono a tarjeta multidivisa | Sí | **No** — embebida en el texto de la descripción |
| Gasto pagado desde cuenta de otra divisa | No | No existe |

La evidencia está en los datos: la tasa de un abono vive dentro de una cadena del tipo `"Comisión 0.20% Pago Tarjeta (Tasa 59.8)"`. No se puede consultar, ni agregar, ni auditar, y un cambio de formato del texto la volvería irrecuperable.

### 1.2 Cada emisor liquida las compras en divisa de forma distinta

Aquí está el matiz que obliga a rehacer el modelo. Una compra en dólares en un comercio extranjero no se comporta igual según la tarjeta:

**Política A — liquidación en la divisa de origen.**
El consumo entra en dólares y permanece en dólares. El saldo en divisa extranjera de la tarjeta lo refleja directamente. No hay conversión que registrar.

**Política B — traducción a moneda local.**
El consumo entra en dólares y el saldo lo refleja **temporalmente** en dólares. Días después, el emisor lo traduce a moneda local aplicando **una tasa de referencia propia que no se conoce en el momento de la compra**, y el importe se traslada al saldo en pesos.

> **La consecuencia práctica:** un consumo bajo la política B tiene, durante un intervalo, un importe conocido en dólares y un importe **desconocido** en pesos. Si se contrasta un estado de cuenta durante ese intervalo, o si el estado ya liquidó algo que la aplicación aún tiene pendiente, aparecerá una discrepancia. El modelo debe hacer que esa discrepancia sea **esperada y explicable**, en lugar de un descuadre sin causa aparente.

---

## 2. Modelo de dominio

### 2.1 La política pertenece a la tarjeta, no a la transacción

Es un atributo del producto financiero. Todos los consumos en divisa de una misma tarjeta se comportan igual, así que vive en el agregado `Tarjeta`.

```rust
/// Cómo liquida un emisor los consumos hechos en divisa extranjera.
pub enum PoliticaLiquidacion {
    /// El consumo permanece en su divisa de origen.
    EnDivisaDeOrigen,

    /// El emisor traduce el consumo a moneda local en un momento posterior,
    /// a una tasa de referencia propia que no se conoce al comprar.
    TraduceAMonedaLocal { moneda_local: Divisa },
}
```

### 2.2 La conversión como hecho, no como cálculo

```rust
pub struct Conversion {
    origen: Dinero,
    destino: Dinero,
    tasa: TasaCambio,
    fecha: String,        // dd/mm/aaaa — cuándo ocurrió la conversión
    fuente: FuenteTasa,
}

pub enum FuenteTasa {
    /// La declaró el titular al operar. Es el caso de los abonos a tarjeta.
    Declarada,
    /// Se dedujo del estado de cuenta del emisor. Es el caso de las compras
    /// bajo la política B.
    ReferenciaDelEmisor,
}
```

**Dos constructores, según qué se conoce primero:**

```rust
/// Se conoce la tasa y se calcula el destino. Abonos: el titular la teclea.
Conversion::con_tasa(origen: Dinero, destino: Divisa, tasa: TasaCambio, fecha)

/// Se conocen ambos importes y se deduce la tasa. Estados de cuenta: el
/// emisor informa "USD 100.00" y "RD$ 6,050.00" sin decir a qué tasa.
Conversion::desde_importes(origen: Dinero, destino: Dinero, fecha)
```

**Invariante:** los tres valores son siempre coherentes entre sí. Un `Conversion` no puede existir con una tasa que no cuadre con sus importes, lo que descarta el descuadre silencioso que hoy nadie detectaría.

### 2.3 Principio rector: los importes mandan, la tasa se deriva

Cuando la conversión la hace el emisor, **el dato autoritativo son los dos importes**, no la tasa.

El motivo es aritmético: la tasa real del banco puede tener más decimales de los que muestra el estado. Si se guardara solo `origen` y `tasa` y se recalculara `destino`, el resultado podría desviarse en centavos del importe que el banco realmente cargó. Guardando ambos importes tal como los informa el emisor, el saldo reproduce el estado de cuenta al centavo y la tasa queda como información derivada y descriptiva.

Para las conversiones que declara el titular (abonos) el orden se invierte: ahí la tasa es el dato de entrada y el destino se calcula con `Dinero::convertir`, que ya redondea a centavos.

### 2.4 Ciclo de vida de un consumo en divisa

```rust
pub enum EstadoConversion {
    /// El consumo se liquida en su propia divisa. Política A, o compra en
    /// moneda local. No hay nada que convertir.
    NoAplica,

    /// Política B, aún sin traducir. El importe en moneda local todavía
    /// NO SE CONOCE: es el emisor quien lo fijará.
    PendienteDeLiquidacion,

    /// El emisor ya tradujo el consumo.
    Liquidado(Conversion),
}
```

```
   compra en divisa
          │
          ▼
   ┌──────────────┐   política A    ┌──────────────┐
   │  registrada  │────────────────►│   NoAplica   │  saldo USD, definitivo
   └──────┬───────┘                 └──────────────┘
          │ política B
          ▼
  ┌───────────────────────┐   se carga el estado    ┌──────────────────┐
  │ PendienteDeLiquidacion│────────────────────────►│    Liquidado     │
  │  saldo USD, temporal  │   con importe en pesos  │  saldo DOP, firme│
  └───────────────────────┘                         └──────────────────┘
```

**La liquidación mueve saldo entre divisas de la misma tarjeta:** resta del saldo en dólares y suma al saldo en pesos. No es solo un cambio de etiqueta.

---

## 3. El estado de cuenta como fuente de la tasa

De aquí sale la pieza más útil del modelo. Bajo la política B, el emisor **nunca comunica su tasa de referencia por adelantado**. La única forma de conocerla es el estado de cuenta.

Eso convierte la carga del estado en la operación que **cierra** las conversiones pendientes:

1. Se localiza el consumo pendiente que corresponde a la línea del estado.
2. Se lee el importe en pesos que el emisor asignó.
3. Se construye `Conversion::desde_importes(...)`, que deduce la tasa aplicada.
4. El consumo pasa a `Liquidado` y el saldo se traslada de dólares a pesos.

Como efecto secundario se obtiene el **histórico de tasas de referencia reales** de cada emisor, que hoy no existe en ninguna parte.

### 3.1 Qué discrepancias serán esperables

Al contrastar un estado, estas diferencias **no son errores** y el informe debe presentarlas como tales:

| Situación | Efecto en el contraste |
|---|---|
| Consumo pendiente en la aplicación, ya liquidado en el estado | La aplicación lo tiene en USD; el estado en DOP. Se resuelve liquidándolo |
| Consumo pendiente en ambos | Coinciden en USD. Sin discrepancia |
| Consumo posterior al corte | Está en la aplicación y no en el estado. Ya observado en el cruce anterior |
| Bonificación aplicada por el emisor | Falta en la aplicación hasta que exista el módulo de cashback |

Solo el resto merece investigarse.

---

## 4. Persistencia

Columnas nuevas, todas nullable para no alterar la conducta de las filas existentes:

**`tarjetas`**
```
politica_liquidacion   TEXT   -- 'origen' | 'traduce'; NULL = 'origen'
```

**`gastos`**
```
estado_conversion      TEXT   -- NULL | 'pendiente' | 'liquidado'
monto_liquidado        REAL   -- importe en moneda local que fijó el emisor
divisa_liquidada       TEXT
tasa_conversion        REAL   -- derivada; descriptiva, no autoritativa
fecha_liquidacion      TEXT   -- dd/mm/aaaa
```

**`pagos_tarjeta`**
```
tasa_cambio            REAL   -- rescata del texto la tasa de los abonos
monto_equivalente      REAL   -- importe realmente debitado en la otra divisa
divisa_equivalente     TEXT
```

> Las tasas históricas embebidas en descripciones del tipo `"(Tasa 59.8)"` pueden recuperarse con una migración de lectura única. Es una extracción con expresión regular sobre texto que la aplicación generó ella misma, así que el formato es conocido y estable. Debe hacerse **sobre una copia** y contrastar el total antes de escribir.

---

## 5. Reglas que quedan fijadas

1. **La política de liquidación es de la tarjeta**, no de la transacción.
2. **Convertir primero, retener después.** La retención del 0,20 % se calcula sobre el importe en moneda local, que es el que sale de la cuenta. Ya es la conducta vigente en abonos desde la unificación de H8.
3. **Un consumo pendiente no tiene importe en moneda local.** No se estima ni se interpola con una tasa de mercado: se deja pendiente hasta que el emisor lo fije. Inventar una cifra produciría un saldo que nunca cuadrará con el estado.
4. **Los importes del emisor son autoritativos; la tasa es derivada.**
5. **Un `Conversion` siempre es internamente coherente**, se construya por el camino que se construya.

---

## 6. Preguntas abiertas

Se resolverán al contrastar estados de cuenta de tarjetas con ambas políticas:

* **¿Hay comisión por transacción internacional?** Es habitual que el emisor cobre un porcentaje adicional sobre compras en el extranjero. Los estados contrastados hasta ahora no mostraban ninguna, pero corresponden a consumos locales. Si existe, es un tercer concepto de cargo junto a retención y comisión de servicio.
* **¿Con qué fecha se liquida?** ¿La de la compra, la de entrada al sistema, o la de la traducción? Determina en qué período de facturación cae el consumo.
* **¿La tasa es uniforme por día o por transacción?** Si varias compras del mismo día liquidan a la misma tasa, se puede validar la coherencia entre ellas.

---

## 7. Impacto en la Fase 1.7

El caso de uso `registrar_gasto` de la Fase 1.5 **rechaza** debitar una cuenta de una divisa distinta a la del gasto. Hoy el código en producción no compara divisas y resta igual, que es el hallazgo H2.

Cablearlo en la Fase 1.7 convertiría un fallo silencioso en un error visible. Con este modelo aparece una tercera vía, preferible a las dos anteriores:

| Opción | Efecto |
|---|---|
| Rechazar | Seguro, pero bloquea un flujo legítimo |
| Convertir con tasa declarada | Correcto para pagos donde el titular conoce la tasa |
| **Registrar como pendiente** | Correcto para consumos bajo política B, donde la tasa aún no existe |

La elección depende del tipo de operación, y el modelo ya distingue los tres casos.
