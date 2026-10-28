// Aritmética de la pestaña «Evolución»: centavos enteros, formato, variación y orden. Datos inventados.

import test from 'node:test';
import assert from 'node:assert/strict';
import {
    enCentavos, formatearCentavos, formatearConSigno, formatearTexto, formatearPorcentaje,
    nombreDeMes, ordenarFotos, variacion, valorDeSerie, esEstimada, etiquetaDeSerie,
} from '../../../src/js/nucleo/evolucion.js';

test('enCentavos: texto exacto a centavos enteros, con signo y sin pasar por Number', () => {
    assert.equal(enCentavos('1234.56'), 123456n);
    assert.equal(enCentavos('-80.10'), -8010n);
    assert.equal(enCentavos('+7'), 700n);
    assert.equal(enCentavos('0.5'), 50n);
    assert.equal(enCentavos('0'), 0n);
    assert.equal(enCentavos(' 12.30 '), 1230n);
    // Más allá de 2^53 un Number ya no distingue el último céntimo: los enteros sí.
    assert.equal(enCentavos('9007199254740993.17'), 900719925474099317n);
    assert.equal(enCentavos('-9007199254740993.17'), -900719925474099317n);
});

test('enCentavos: más de dos decimales se redondean mitad lejos de cero mirando el tercero', () => {
    assert.equal(enCentavos('1.004'), 100n);
    assert.equal(enCentavos('1.005'), 101n);
    assert.equal(enCentavos('-1.005'), -101n);
    assert.equal(enCentavos('1.0049999'), 100n);
});

test('enCentavos: lo que no es un importe lanza', () => {
    for (const malo of ['', 'abc', '1,5', '1.2.3', '--1', '.5', '1e3']) assert.throws(() => enCentavos(malo), /ilegible/, malo);
});

test('formatearCentavos: separador de miles, dos decimales y signo', () => {
    assert.equal(formatearCentavos(123456n), '1,234.56');
    assert.equal(formatearCentavos(0n), '0.00');
    assert.equal(formatearCentavos(5n), '0.05');
    assert.equal(formatearCentavos(-5n), '-0.05');
    assert.equal(formatearCentavos(-123456789n), '-1,234,567.89');
    assert.equal(formatearCentavos(900719925474099317n), '9,007,199,254,740,993.17');
});

test('formatearCentavos sin decimales redondea a unidades (y no deja «-0»)', () => {
    assert.equal(formatearCentavos(123456n, false), '1,235');
    assert.equal(formatearCentavos(-123456n, false), '-1,235');
    assert.equal(formatearCentavos(-20n, false), '0');
    assert.equal(formatearCentavos(50n, false), '1');
});

test('formatearConSigno y formatearTexto', () => {
    assert.equal(formatearConSigno(1234n), '+12.34');
    assert.equal(formatearConSigno(-1234n), '-12.34');
    assert.equal(formatearConSigno(0n), '0.00');
    assert.equal(formatearTexto('1500.37'), '1,500.37');
    assert.equal(formatearTexto('no es importe'), 'no es importe');
});

test('variación: delta, dirección y porcentaje redondeado a centésimas', () => {
    assert.deepEqual(variacion(10500n, 10000n), { delta: 500n, centesimas: 500n, direccion: 'sube' });
    assert.deepEqual(variacion(9900n, 10000n), { delta: -100n, centesimas: -100n, direccion: 'baja' });
    assert.deepEqual(variacion(7n, 7n), { delta: 0n, centesimas: 0n, direccion: 'igual' });
    // 1 / 3 = 33.33 %; 2 / 3 = 66.67 % (redondeo mitad lejos de cero)
    assert.equal(variacion(400n, 300n).centesimas, 3333n);
    assert.equal(variacion(500n, 300n).centesimas, 6667n);
    assert.equal(variacion(200n, 300n).centesimas, -3333n);
});

test('variación: base cero no tiene porcentaje; base negativa usa su valor absoluto', () => {
    assert.equal(variacion(500n, 0n).centesimas, null);
    assert.equal(variacion(500n, 0n).direccion, 'sube');
    // de -200 a -100: mejora 100 sobre |-200| = 50 %
    assert.deepEqual(variacion(-10000n, -20000n), { delta: 10000n, centesimas: 5000n, direccion: 'sube' });
    assert.equal(variacion(-30000n, -20000n).centesimas, -5000n);
});

test('formatearPorcentaje', () => {
    assert.equal(formatearPorcentaje(123n), '+1.23 %');
    assert.equal(formatearPorcentaje(-5n), '-0.05 %');
    assert.equal(formatearPorcentaje(0n), '0.00 %');
    assert.equal(formatearPorcentaje(1000012n), '+10000.12 %');
});

test('nombreDeMes y ordenarFotos: por año y luego mes, sin tocar el original', () => {
    assert.equal(nombreDeMes(2027, 3), 'mar 2027');
    assert.equal(nombreDeMes(2026, 12), 'dic 2026');
    const entrada = [{ anio: 2027, mes: 2 }, { anio: 2026, mes: 12 }, { anio: 2027, mes: 1 }, { anio: 2026, mes: 3 }];
    const copia = [...entrada];
    assert.deepEqual(ordenarFotos(entrada).map(f => `${f.anio}-${f.mes}`), ['2026-3', '2026-12', '2027-1', '2027-2']);
    assert.deepEqual(entrada, copia);
});

test('serie: total o líquido, y qué es una foto estimada', () => {
    const f = { patrimonio: '100.01', liquido: '40.02', origen: 'real', calidad: 'completa' };
    assert.equal(valorDeSerie(f, 'total'), 10001n);
    assert.equal(valorDeSerie(f, 'liquido'), 4002n);
    assert.equal(etiquetaDeSerie('liquido'), 'Patrimonio líquido');
    assert.equal(esEstimada(f), false);
    assert.equal(esEstimada({ origen: 'reconstruida', calidad: 'completa' }), true);
    assert.equal(esEstimada({ origen: 'real', calidad: 'estimada' }), true);
    assert.equal(esEstimada({ origen: 'manual', calidad: 'con_aviso' }), false);
});
