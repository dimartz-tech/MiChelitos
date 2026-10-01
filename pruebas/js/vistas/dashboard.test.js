// Pruebas de la vista «Dashboard» en Node, sin navegador.
//
// Es la pantalla de inicio: de solo lectura, pero **filtra por el mes en curso,
// suma y decide qué avisos mostrar**. Con la vista recibiendo sus dependencias
// se le dan datos inventados, un formato que enseña el valor y un reloj fijo.
//
// Se importa el JavaScript compilado de la vista (`npm test` compila antes).

import test from 'node:test';
import assert from 'node:assert/strict';
import { VistaDashboard, puenteDashboard } from '../../../src/js/vistas/dashboard.js';

function datos(extra = {}) {
    return {
        capital: { totales: { certificados: 1000, bolsa: 500, inmobiliario: 8000, vehiculos: 2000, maquinaria: 0, patrimonio: 11500 }, certificados: [], bolsa: [] },
        gastos: [
            { fecha: '12/03/2027', monto: 100, costo_adicional: 5 },
            { fecha: '12/02/2027', monto: 999, costo_adicional: 0 }, // otro mes
        ],
        ingresos: [
            { fecha_emision: '10/03/2027', monto_total: 300, monto_recibido: 200, monto_retenido: 30 },
            { fecha_emision: '10/02/2027', monto_total: 999, monto_recibido: 999, monto_retenido: 99 }, // otro mes
        ],
        informales: [{ fecha: '11/03/2027', monto: 80, monto_recibido: 50 }],
        tarjetas: [],
        prestamos: [],
        ...extra,
    };
}

async function dibujar({ d = datos() } = {}) {
    const pantalla = { contenido: { innerHTML: '' } };
    const llamadas = [];
    const responde = (nombre, valor) => async () => { llamadas.push(nombre); return valor; };
    const vista = new VistaDashboard({
        api: {
            obtenerCapital: responde('obtenerCapital', d.capital),
            obtenerGastos: responde('obtenerGastos', d.gastos),
            obtenerIngresos: responde('obtenerIngresos', d.ingresos),
            obtenerIngresosInformales: responde('obtenerIngresosInformales', d.informales),
            obtenerTarjetas: responde('obtenerTarjetas', d.tarjetas),
            obtenerPrestamos: responde('obtenerPrestamos', d.prestamos),
        },
        formato: { importe: v => `#${Number(v).toFixed(2)}#` },
        pantalla,
        ahora: () => new Date(2027, 2, 5),
    });
    await vista.render();
    return { html: pantalla.contenido.innerHTML, llamadas };
}

const importeTras = (html, etiqueta) => {
    const i = html.indexOf(etiqueta);
    assert.ok(i >= 0, `no aparece «${etiqueta}»`);
    const m = /DOP #(-?[\d.]+)#/.exec(html.slice(i));
    assert.ok(m, `no hay importe tras «${etiqueta}»`);
    return Number(m[1]);
};

test('consulta las seis fuentes y dibuja el título', async () => {
    const { html, llamadas } = await dibujar();
    assert.match(html, /Dashboard General/);
    assert.deepEqual([...llamadas].sort(), [
        'obtenerCapital', 'obtenerGastos', 'obtenerIngresos', 'obtenerIngresosInformales', 'obtenerPrestamos', 'obtenerTarjetas',
    ]);
});

test('el patrimonio total es el que calcula el núcleo, sin sumarlo de nuevo en la vista', async () => {
    const { html } = await dibujar();
    assert.equal(importeTras(html, 'Patrimonio Total'), 11500);
});

test('ingresos y gastos son solo los del mes del reloj inyectado, con los cargos sumados al gasto', async () => {
    const { html } = await dibujar();
    // recibido: 200 formal + 50 informal; gastado: 100 + 5 de cargo; febrero no cuenta
    assert.match(html, /DOP #250\.00#/, 'recibido del mes');
    assert.match(html, /Facturado: DOP #380\.00# \| Retenido: DOP #30\.00#/);
    assert.match(html, /DOP #105\.00#/, 'gastado del mes');
});

test('sin movimientos del mes todo es cero y no hay recordatorios', async () => {
    const { html } = await dibujar({ d: datos({ gastos: [], ingresos: [], informales: [] }) });
    assert.match(html, /Facturado: DOP #0\.00# \| Retenido: DOP #0\.00#/);
    assert.doesNotMatch(html, /Recordatorios del Sistema/);
    assert.doesNotMatch(html, /NaN|undefined/);
});

test('los avisos salen de tarjetas, certificados, bolsa y préstamos, cada uno con su nivel', async () => {
    const d = datos({
        tarjetas: [{ entidad: 'Banco A', nombre_tarjeta: 'Oro', alerta_corte: true, dias_corte_msg: 'corta en 2 días', alerta_pago: true, dias_pago_msg: 'paga en 1 día', balance_pesos: 10, balance_dolares: 1 }],
        capital: { totales: {}, certificados: [{ banco: 'Banco C', alerta_vencimiento: true, alerta_msg: 'vence pronto' }, { banco: 'Sin alerta', alerta_vencimiento: false }], bolsa: [{ emisor: 'Emisor B', alerta_vencimiento: true, alerta_msg: 'vence mañana' }] },
        prestamos: [{ institucion_financiera: 'Financiera D', tipo_prestamo: 'consumo', alerta_pago: true, dias_pago_msg: 'cuota mañana', monto_cuota: 25 }],
    });
    const { html } = await dibujar({ d });
    assert.match(html, /Recordatorios del Sistema/);
    assert.match(html, /<span class="badge danger"[^>]*>5<\/span>/, 'cinco avisos');
    assert.match(html, /\[Tarjeta \(Corte\)\]<\/strong> Banco A Oro: corta en 2 días/);
    // El nivel decide el color y el icono: cada aviso debe llevar el suyo.
    const niveles = {};
    for (const m of html.matchAll(/alert-banner (\w+)">\s*<span>[^<]*<\/span>\s*<div><strong>\[([^\]]+\)?)\]/g)) niveles[m[2]] = m[1];
    assert.deepEqual(niveles, {
        'Tarjeta (Corte)': 'warning',
        'Tarjeta (Pago)': 'danger',
        'Certificado': 'info',
        'Bolsa': 'info',
        'Deuda / Préstamo': 'danger',
    });
    assert.match(html, /\[Tarjeta \(Pago\)\]<\/strong> Banco A Oro: paga en 1 día/);
    assert.match(html, /\[Certificado\]<\/strong> Banco: Banco C - vence pronto/);
    assert.doesNotMatch(html, /Sin alerta/, 'un certificado sin alerta no aparece');
    assert.match(html, /\[Bolsa\]<\/strong> Emisor: Emisor B - vence mañana/);
    assert.match(html, /\[Deuda \/ Préstamo\]<\/strong> Financiera D \(consumo\): cuota mañana/);
});

test('muestra solo las tres primeras tarjetas y préstamos, con sus importes por el formato inyectado', async () => {
    const t = n => ({ entidad: `Banco ${n}`, nombre_tarjeta: `Tarjeta ${n}`, balance_pesos: 100 * n, balance_dolares: n, alerta_corte: false, alerta_pago: false });
    const p = n => ({ institucion_financiera: `Financiera ${n}`, tipo_prestamo: 'consumo', monto_cuota: 10 * n, alerta_pago: false });
    const { html } = await dibujar({ d: datos({ tarjetas: [t(1), t(2), t(3), t(4)], prestamos: [p(1), p(2), p(3), p(4)] }) });
    assert.match(html, /Banco 3/);
    assert.doesNotMatch(html, /Banco 4/);
    assert.match(html, /DOP #300\.00#/);
    assert.match(html, /Financiera 3/);
    assert.doesNotMatch(html, /Financiera 4/);
    assert.match(html, /DOP #30\.00# \/ mes/);
});

test('sin tarjetas ni préstamos dibuja los mensajes vacíos', async () => {
    const { html } = await dibujar();
    assert.match(html, /No hay tarjetas registradas/);
    assert.match(html, /No hay préstamos registrados/);
});

test('sus enlaces son navigate(), no manejadores de appUI; el puente no cuelga nada', async () => {
    const { html } = await dibujar();
    assert.match(html, /onclick="navigate\('tarjetas'\)"/);
    assert.match(html, /onclick="navigate\('prestamos'\)"/);
    assert.doesNotMatch(html, /appUI\./);
    assert.deepEqual(puenteDashboard(new VistaDashboard({})), {});
});
