// Pruebas de la vista «Resumen» en Node, sin navegador.
//
// Es una pantalla de solo lectura, pero **calcula**: patrimonio neto, carga de
// endeudamiento, flujos fijos y balance del mes. Esas reglas viven en la vista,
// y hasta ahora ninguna prueba las ejercía. Con la vista recibiendo sus
// dependencias, aquí se le dan datos inventados, un formato que enseña el valor
// sin redondear, un reloj fijo y una tasa de referencia propia.
//
// Se importa el JavaScript compilado de la vista (`npm test` compila antes).

import test from 'node:test';
import assert from 'node:assert/strict';
import { VistaResumen, puenteResumen } from '../../../src/js/vistas/resumen.js';

/** Datos inventados con resultados fáciles de comprobar a mano. */
function datos(extra = {}) {
    return {
        capital: { totales: { certificados: 1000, bolsa: 500, inmobiliario: 8000, vehiculos: 2000, maquinaria: 0, patrimonio: 11500 } },
        ingresos: [
            { fecha_emision: '10/03/2027', monto_recibido: 200, monto_total: 300 },
            { fecha_emision: '10/02/2027', monto_recibido: 999, monto_total: 999 }, // otro mes: no cuenta
        ],
        informales: [{ fecha: '11/03/2027', monto_recibido: 50, monto: 80 }],
        gastos: [
            { fecha: '12/03/2027', monto: 100, costo_adicional: 5 },
            { fecha: '12/02/2027', monto: 999, costo_adicional: 0 }, // otro mes: no cuenta
        ],
        tarjetas: [{ balance_pesos: 100, balance_dolares: 2 }],
        suscripciones: [
            { frecuencia: 'mensual', monto: 10 },
            { frecuencia: 'anual', monto: 120 }, // 120 / 12 = 10 al mes
        ],
        prestamos: [
            { saldo_actual: 300, tipo_prestamo: 'amortizable', cuotas_pendientes: 5, monto_cuota: 25 },
            { saldo_actual: 100, tipo_prestamo: 'flexible', cuotas_pendientes: null, monto_cuota: 10 },
            { saldo_actual: 50, tipo_prestamo: 'amortizable', cuotas_pendientes: 0, monto_cuota: 999 }, // ya sin cuotas: no suma cuota
        ],
        ...extra,
    };
}

async function dibujar({ d = datos(), tasa = 50 } = {}) {
    const pantalla = { contenido: { innerHTML: '' } };
    const llamadas = [];
    const responde = (nombre, valor) => async () => { llamadas.push(nombre); return valor; };
    const vista = new VistaResumen({
        api: {
            obtenerCapital: responde('obtenerCapital', d.capital),
            obtenerIngresos: responde('obtenerIngresos', d.ingresos),
            obtenerIngresosInformales: responde('obtenerIngresosInformales', d.informales),
            obtenerGastos: responde('obtenerGastos', d.gastos),
            obtenerTarjetas: responde('obtenerTarjetas', d.tarjetas),
            obtenerSuscripciones: responde('obtenerSuscripciones', d.suscripciones),
            obtenerPrestamos: responde('obtenerPrestamos', d.prestamos),
        },
        formato: { importe: v => `#${Number(v).toFixed(2)}#` },
        pantalla,
        ahora: () => new Date(2027, 2, 5),
        referencias: { tasaUsdADop: tasa },
    });
    await vista.render();
    return { html: pantalla.contenido.innerHTML, llamadas };
}

/** El importe `DOP #…#` que sigue a una etiqueta de la pantalla. */
function importeDe(html, etiqueta) {
    const i = html.indexOf(etiqueta);
    assert.ok(i >= 0, `no aparece «${etiqueta}»`);
    const m = /DOP #(-?[\d.]+)#/.exec(html.slice(i));
    assert.ok(m, `no hay importe tras «${etiqueta}»`);
    return Number(m[1]);
}

test('consulta las siete fuentes y dibuja el título', async () => {
    const { html, llamadas } = await dibujar();
    assert.match(html, /Resumen Ejecutivo/);
    assert.deepEqual([...llamadas].sort(), [
        'obtenerCapital', 'obtenerGastos', 'obtenerIngresos', 'obtenerIngresosInformales',
        'obtenerPrestamos', 'obtenerSuscripciones', 'obtenerTarjetas',
    ]);
});

test('el patrimonio neto es el patrimonio del núcleo menos los pasivos', async () => {
    // pasivos = tarjetas (100 + 2 × 50) + préstamos (300 + 100 + 50) = 650; 11 500 − 650
    const { html } = await dibujar();
    assert.equal(importeDe(html, 'Patrimonio Neto'), 10850);
    assert.equal(importeDe(html, 'Balances Tarjetas de Crédito'), 200);
});

test('los dólares de las tarjetas se pasan a pesos con la tasa de referencia inyectada, no con una escrita en la vista', async () => {
    const con50 = await dibujar({ tasa: 50 });
    const con70 = await dibujar({ tasa: 70 });
    assert.equal(importeDe(con50.html, 'Balances Tarjetas de Crédito'), 200);
    assert.equal(importeDe(con70.html, 'Balances Tarjetas de Crédito'), 240);
});

test('el ratio de endeudamiento sale de pasivos / activos y rotula Saludable, Moderado o Alto Riesgo', async () => {
    const saludable = await dibujar(); // 650 / 11 500 = 5,7 %
    assert.match(saludable.html, /5\.7%/);
    assert.match(saludable.html, /Saludable/);

    const moderado = await dibujar({ d: datos({ prestamos: [{ saldo_actual: 4000, tipo_prestamo: 'flexible', cuotas_pendientes: null, monto_cuota: 0 }] }) });
    assert.match(moderado.html, /36\.5%/); // (200 + 4000) / 11 500
    assert.match(moderado.html, /Moderado/);

    const alto = await dibujar({ d: datos({ prestamos: [{ saldo_actual: 9000, tipo_prestamo: 'flexible', cuotas_pendientes: null, monto_cuota: 0 }] }) });
    assert.match(alto.html, /Alto Riesgo/);
});

test('sin activos el ratio es 0 y no divide por cero', async () => {
    const { html } = await dibujar({ d: datos({ capital: { totales: {} } }) });
    assert.match(html, /0\.0%/);
    assert.doesNotMatch(html, /NaN|Infinity/);
});

test('la carga fija suma las cuotas que corren (flexibles y con cuotas pendientes) y las suscripciones al mes', async () => {
    // préstamos: 25 + 10 (el tercero ya no tiene cuotas pendientes: no suma); suscripciones: 10 + 120 / 12
    const { html } = await dibujar();
    assert.equal(importeDe(html, 'Cuotas de Préstamos'), 35);
    assert.equal(importeDe(html, 'Suscripciones Recurrentes'), 20);
    assert.equal(importeDe(html, 'Carga Fija Mensual'), 55);
});

test('el balance del mes cuenta solo lo del mes del reloj inyectado', async () => {
    // cobrado 200 + 50; gastos 100 + 5; el resto es de febrero
    const { html } = await dibujar();
    assert.equal(importeDe(html, 'Ingresos Cobrados (Flujo)'), 250);
    assert.equal(importeDe(html, 'Ingresos Facturados (Impuestos)'), 380);
    assert.equal(importeDe(html, 'Gastos y Cargos (Este Mes)'), 105);
    assert.equal(importeDe(html, 'Balance Mensual (Cobrado)'), 145);
});

test('un balance negativo se marca como gasto', async () => {
    const { html } = await dibujar({ d: datos({ gastos: [{ fecha: '12/03/2027', monto: 900, costo_adicional: 0 }] }) });
    assert.equal(importeDe(html, 'Balance Mensual (Cobrado)'), -650);
    assert.match(html, /class="amount expense">DOP #-650\.00#/);
});

test('sin capital que devolver (null) la vista no falla', async () => {
    const { html } = await dibujar({ d: datos({ capital: null }) });
    assert.match(html, /Resumen Ejecutivo/);
    assert.equal(importeDe(html, 'Patrimonio Neto'), -650);
});

test('no tiene manejadores en línea: el puente no cuelga nada de appUI', async () => {
    const { html } = await dibujar();
    assert.doesNotMatch(html, /appUI\./);
    const vista = new VistaResumen({ api: {}, formato: {}, pantalla: {}, ahora: () => new Date(), referencias: { tasaUsdADop: 1 } });
    assert.deepEqual(puenteResumen(vista), {});
});
