import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// `dom` se carga como script clásico: se evalúa su JavaScript compilado con un
// `document` de mentira, para probar lo que recibe el navegador.
const fuente = readFileSync(
    join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..', 'src', 'js', 'ui', 'dom.js'),
    'utf8',
);
function cargar(elementos) {
    const documento = { getElementById: id => elementos[id] ?? null };
    return new Function('document', `${fuente}; return { elemento, buscar };`)(documento);
}

test('elemento devuelve el elemento que existe', () => {
    const caja = { value: '12' };
    assert.equal(cargar({ monto: caja }).elemento('monto'), caja);
});

test('elemento falla diciendo cuál falta, en lugar de un «Cannot read properties of null»', () => {
    const { elemento } = cargar({});
    assert.throws(() => elemento('gas_mon'), /No existe el elemento #gas_mon/);
});

test('buscar conserva el null donde la ausencia es legítima', () => {
    const { buscar } = cargar({});
    assert.equal(buscar('modal-abierto'), null);
});

test('buscar devuelve el elemento cuando existe', () => {
    const caja = {};
    assert.equal(cargar({ x: caja }).buscar('x'), caja);
});
