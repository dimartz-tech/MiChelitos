// Presupuestos mensuales por categoría: las funciones puras que rotulan lo que calculó Rust y el bloque de Gastos.
// Datos inventados. Se importa el JavaScript compilado (`npm test` compila antes).

import test from 'node:test';
import assert from 'node:assert/strict';
import {
    NOTA_USD, alertasDePresupuesto, anchoDeBarra, etiquetaDeEstado, filasEnAlerta, htmlPresupuestos, porcentajeDeMilesimas,
    textoDeRestante, tieneDolares,
} from '../../../src/js/vistas/presupuestos.js';

const fila = (extra = {}) => ({
    categoria_id: 1, categoria_nombre: 'Comida', fija: false, presupuesto: '1500.01', gastado: '1223.35', restante: '276.66',
    porcentaje_milesimas: 815, estado: 'cerca', usd_aparte: '0.00', ...extra,
});
const estado = (filas) => ({ anio: 2026, mes: 9, filas, total_presupuesto: '1500.01', total_gastado: '1223.35', total_restante: '276.66' });
const CATS = [{ id: 1, nombre: 'Comida' }, { id: 2, nombre: 'Ocio' }];

test('el porcentaje sale de las milésimas por división entera, sin redondear hacia arriba', () => {
    assert.equal(porcentajeDeMilesimas(815), '81.5 %');
    assert.equal(porcentajeDeMilesimas(999), '99.9 %');
    assert.equal(porcentajeDeMilesimas(1061), '106.1 %');
    assert.equal(porcentajeDeMilesimas(0), '0.0 %');
    assert.equal(porcentajeDeMilesimas(-4), '0.0 %');
});

test('la barra nunca pasa del 100 % ni baja de 0', () => {
    assert.equal(anchoDeBarra(1500), 100);
    assert.equal(anchoDeBarra(815), 81);
    assert.equal(anchoDeBarra(-1), 0);
});

test('las etiquetas dicen el estado con palabras', () => {
    assert.equal(etiquetaDeEstado('ok'), 'Dentro del presupuesto');
    assert.match(etiquetaDeEstado('cerca'), /Cerca del límite \(desde el 80 %\)/);
    assert.equal(etiquetaDeEstado('excedido'), 'Excedido');
});

test('restante: lo que queda o lo que se pasó, sobre el texto exacto', () => {
    assert.equal(textoDeRestante({ restante: '1234567.89' }), 'Restan DOP 1,234,567.89');
    assert.equal(textoDeRestante({ restante: '-183.64' }), 'Excedido por DOP 183.64');
    assert.equal(textoDeRestante({ restante: '0.00' }), 'Restan DOP 0.00');
});

test('los dólares solo se señalan si hay', () => {
    assert.equal(tieneDolares({ usd_aparte: '0.00' }), false);
    assert.equal(tieneDolares({ usd_aparte: '65.45' }), true);
});

test('solo cerca y excedido están en alerta', () => {
    const e = estado([fila(), fila({ categoria_id: 2, estado: 'ok' }), fila({ categoria_id: 3, estado: 'excedido' })]);
    assert.deepEqual(filasEnAlerta(e).map(f => f.categoria_id), [1, 3]);
    assert.equal(alertasDePresupuesto(estado([])).length, 0);
    assert.deepEqual(alertasDePresupuesto(e).map(a => a.nivel), ['warning', 'danger']);
});

test('el bloque lleva la nota de los dólares, la barra con su texto, el estado en palabras y el formulario', () => {
    const html = htmlPresupuestos(estado([fila({ usd_aparte: '65.45' })]), CATS);
    assert.ok(html.includes('Solo cuentan los gastos en DOP'), NOTA_USD);
    assert.match(html, /role="progressbar"[^>]*aria-valuenow="81"/);
    assert.match(html, /81\.5 %: Cerca del límite/);
    assert.match(html, /Cerca del límite \(desde el 80 %\) · Restan DOP 276\.66/);
    assert.match(html, /USD 65\.45 aparte \(no cuenta\)/);
    assert.match(html, /id="form-presupuesto"/);
    assert.match(html, /<option value="2">Ocio<\/option>/);
    assert.match(html, /data-accion="handleQuitarPresupuesto"/);
    assert.match(html, /handleFijarPresupuesto/);
    assert.doesNotMatch(html, /\bon[a-z]+=/i, 'sin atributos on*=');
    assert.doesNotMatch(html, /undefined|NaN/);
});

test('un excedido se rotula como excedido con lo que se pasó, y la fija conserva su marca', () => {
    const html = htmlPresupuestos(estado([fila({ estado: 'excedido', restante: '-183.64', porcentaje_milesimas: 1061, fija: true })]), CATS);
    assert.match(html, /Excedido · Excedido por DOP 183\.64/);
    assert.match(html, /↻ fija \/ recurrente/);
    assert.match(html, /aria-valuenow="100"/);
});

test('sin presupuestos: lo dice, sin barras, y el formulario sigue ahí', () => {
    const html = htmlPresupuestos(estado([]), CATS);
    assert.match(html, /Aún no hay presupuestos fijados/);
    assert.doesNotMatch(html, /progressbar/);
    assert.match(html, /id="pre_monto"/);
});

test('si Rust no respondió: aviso y la nota, sin romper', () => {
    const html = htmlPresupuestos(null, CATS);
    assert.match(html, /No se pudieron leer los presupuestos/);
    assert.match(html, /Solo cuentan los gastos en DOP/);
});

test('los nombres de categoría se escapan en barras, botones y selector', () => {
    const malo = '<img src=x onerror=alert(1)>"\'';
    const html = htmlPresupuestos(estado([fila({ categoria_nombre: malo })]), [{ id: 9, nombre: malo }]);
    assert.doesNotMatch(html, /<img/);
    assert.match(html, /&lt;img/);
});
