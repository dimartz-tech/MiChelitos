import test from 'node:test';
import assert from 'node:assert/strict';
import {
    DIVISAS,
    ErrorDivisa,
    normalizarDivisa,
    esDivisaValida,
    formatear,
    sumar,
    totalizarPorDivisa,
    divisasPresentes,
} from './dinero.js';

// --- Divisa ---

test('las divisas admitidas coinciden con las del esquema SQLite', () => {
    assert.deepEqual([...DIVISAS], ['DOP', 'USD']);
});

test('el código se interpreta sin distinguir mayúsculas ni espacios', () => {
    assert.equal(normalizarDivisa('dop'), 'DOP');
    assert.equal(normalizarDivisa('  usd '), 'USD');
});

test('una divisa no admitida es rechazada', () => {
    assert.throws(() => normalizarDivisa('EUR'), ErrorDivisa);
    assert.equal(esDivisaValida('EUR'), false);
    assert.equal(esDivisaValida('DOP'), true);
});

// --- Formato ---

test('formatea pesos y dólares con dos decimales y separador de miles', () => {
    assert.equal(formatear(1234.5, 'DOP'), 'RD$1,234.50');
    assert.equal(formatear(1234.5, 'USD'), 'US$1,234.50');
});

test('formatea cero y negativos sin perder la divisa', () => {
    assert.equal(formatear(0, 'DOP'), 'RD$0.00');
    assert.equal(formatear(-50, 'DOP'), '-RD$50.00');
});

test('un monto no numérico no se formatea en silencio', () => {
    assert.throws(() => formatear(NaN, 'DOP'), ErrorDivisa);
    assert.throws(() => formatear('abc', 'DOP'), ErrorDivisa);
});

// --- Regla central: no se mezclan divisas ---

test('sumar pesos con dólares lanza error en vez de dar un total falso', () => {
    assert.throws(
        () => sumar({ monto: 1000, divisa: 'DOP' }, { monto: 50, divisa: 'USD' }),
        ErrorDivisa,
    );
});

test('sumar en la misma divisa opera con normalidad', () => {
    assert.deepEqual(
        sumar({ monto: 1000, divisa: 'DOP' }, { monto: 250.5, divisa: 'DOP' }),
        { monto: 1250.5, divisa: 'DOP' },
    );
});

// --- Totales por divisa ---

test('los totales se agrupan por divisa y nunca se mezclan', () => {
    const gastos = [
        { monto: 1000, divisa: 'DOP' },
        { monto: 500, divisa: 'DOP' },
        { monto: 25, divisa: 'USD' },
    ];
    assert.deepEqual(totalizarPorDivisa(gastos), { DOP: 1500, USD: 25 });
});

test('una lista vacía devuelve ceros en ambas divisas, no un objeto vacío', () => {
    assert.deepEqual(totalizarPorDivisa([]), { DOP: 0, USD: 0 });
    assert.deepEqual(totalizarPorDivisa(undefined), { DOP: 0, USD: 0 });
});

test('acepta un extractor para totalizar campos derivados', () => {
    const gastos = [
        { monto: 1000, costo_adicional: 2, divisa: 'DOP' },
        { monto: 500, costo_adicional: 1, divisa: 'DOP' },
    ];
    assert.deepEqual(
        totalizarPorDivisa(gastos, (g) => g.costo_adicional),
        { DOP: 3, USD: 0 },
    );
});

test('divisasPresentes omite las divisas sin movimiento', () => {
    assert.deepEqual(divisasPresentes({ DOP: 1500, USD: 0 }), ['DOP']);
    assert.deepEqual(divisasPresentes({ DOP: 1500, USD: 25 }), ['DOP', 'USD']);
    assert.deepEqual(divisasPresentes({ DOP: 0, USD: 0 }), []);
});
