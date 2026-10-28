// La gráfica SVG del patrimonio: función pura, se prueba sobre el texto. Datos inventados.

import test from 'node:test';
import assert from 'node:assert/strict';
import { generarGrafica, describirGrafica } from '../../../src/js/nucleo/grafica.js';

const foto = (anio, mes, patrimonio, liquido = patrimonio, extra = {}) => ({
    anio, mes, patrimonio, liquido, activos: patrimonio, pasivos: '0.00', origen: 'real', calidad: 'completa', ...extra,
});

const circulos = svg => [...svg.matchAll(/<circle class="marcador-real" cx="([\d.-]+)" cy="([\d.-]+)"/g)].map(m => ({ x: Number(m[1]), y: Number(m[2]) }));
const rombos = svg => [...svg.matchAll(/<path class="marcador-estimado"/g)].length;
const etiquetasY = svg => [...svg.matchAll(/class="eje-y"[^>]*>([^<]*)</g)].map(m => m[1]);

test('con 0 o 1 foto no hay gráfica (cadena vacía)', () => {
    assert.equal(generarGrafica([], { serie: 'total' }), '');
    assert.equal(generarGrafica([foto(2027, 1, '100.07')], { serie: 'total' }), '');
});

test('con 2 fotos: svg accesible con role="img", título y descripción enlazados', () => {
    const svg = generarGrafica([foto(2027, 1, '1234.57'), foto(2027, 2, '2345.68')], { serie: 'total' });
    assert.match(svg, /^<svg [^>]*role="img"/);
    assert.match(svg, /aria-labelledby="grafica-evolucion-titulo grafica-evolucion-desc"/);
    assert.match(svg, /<title id="grafica-evolucion-titulo">Patrimonio total: evolución mensual<\/title>/);
    assert.match(svg, /<desc id="grafica-evolucion-desc">Patrimonio total de 1,234\.57 en ene 2027 a 2,345\.68 en feb 2027\./);
    assert.equal(circulos(svg).length, 2);
    assert.equal(rombos(svg), 0);
    assert.match(svg, /<\/svg>$/);
});

test('escala exacta: el máximo y el mínimo caen donde dicen los enteros (arriba, más pequeño)', () => {
    // 100.00 y 200.00 -> dominio [95.00, 205.00]; área de 256 px desde y = 20.
    const svg = generarGrafica([foto(2027, 1, '100.00'), foto(2027, 2, '200.00')], { serie: 'total' });
    const [a, b] = circulos(svg);
    assert.equal(a.y, 264.3);
    assert.equal(b.y, 31.6);
    assert.equal(a.x, 92);
    assert.equal(b.x, 696);
});

test('la escala no pierde céntimos con importes de más de 2^53 centavos', () => {
    const svg = generarGrafica([foto(2027, 1, '9007199254740993.17'), foto(2027, 2, '9007199254740993.18')], { serie: 'total' });
    const [a, b] = circulos(svg);
    assert.ok(a.y > b.y, 'un céntimo más debe subir el punto');
    assert.match(svg, /9,007,199,254,740,993\.17 en ene 2027 a 9,007,199,254,740,993\.18 en feb 2027/);
    assert.doesNotMatch(svg, /NaN|Infinity/);
});

test('N fotos: se ordenan por año y mes aunque lleguen desordenadas', () => {
    const fotos = [foto(2027, 2, '300.03'), foto(2026, 12, '100.01'), foto(2027, 1, '200.02')];
    const svg = generarGrafica(fotos, { serie: 'total' });
    const puntos = circulos(svg);
    assert.equal(puntos.length, 3);
    assert.ok(puntos[0].x < puntos[1].x && puntos[1].x < puntos[2].x);
    assert.ok(puntos[0].y > puntos[1].y && puntos[1].y > puntos[2].y, 'dic 2026 es el más bajo');
    assert.match(svg, /de 100\.01 en dic 2026 a 300\.03 en feb 2027/);
    assert.equal(fotos[0].anio, 2027, 'la entrada no se altera');
});

test('el eje horizontal es proporcional a los meses: un mes sin foto deja un hueco', () => {
    const svg = generarGrafica([foto(2027, 1, '10.01'), foto(2027, 2, '20.02'), foto(2027, 4, '30.03')], { serie: 'total' });
    const [a, b, c] = circulos(svg);
    assert.ok(Math.abs((c.x - b.x) - 2 * (b.x - a.x)) < 0.2, `${a.x} ${b.x} ${c.x}`);
});

test('valores negativos: línea del cero y etiquetas con signo', () => {
    const svg = generarGrafica([foto(2027, 1, '-500.05'), foto(2027, 2, '300.30')], { serie: 'total' });
    assert.match(svg, /class="cero"/);
    assert.ok(etiquetasY(svg).some(e => e.startsWith('-')), etiquetasY(svg).join());
    assert.match(svg, /de -500\.05 en ene 2027 a 300\.30 en feb 2027/);
    const sinCero = generarGrafica([foto(2027, 1, '500.05'), foto(2027, 2, '300.30')], { serie: 'total' });
    assert.doesNotMatch(sinCero, /class="cero"/);
});

test('todos los valores iguales: sin NaN ni división por cero, y a media altura', () => {
    const svg = generarGrafica([foto(2027, 1, '777.77'), foto(2027, 2, '777.77'), foto(2027, 3, '777.77')], { serie: 'total' });
    assert.doesNotMatch(svg, /NaN|Infinity|undefined/);
    const ys = new Set(circulos(svg).map(p => p.y));
    assert.equal(ys.size, 1);
    assert.equal([...ys][0], 148);
    const ceros = generarGrafica([foto(2027, 1, '0.00'), foto(2027, 2, '0.00')], { serie: 'total' });
    assert.doesNotMatch(ceros, /NaN|Infinity|undefined/);
});

test('rangos de pocos centavos: las marcas del eje no se repiten y enseñan los centavos', () => {
    const svg = generarGrafica([foto(2027, 1, '10.01'), foto(2027, 2, '10.02')], { serie: 'total' });
    const etiquetas = etiquetasY(svg);
    assert.equal(new Set(etiquetas).size, etiquetas.length);
    assert.ok(etiquetas.every(e => /\.\d\d$/.test(e)), etiquetas.join());
});

test('rangos grandes: etiquetas del eje en unidades con separador de miles', () => {
    const svg = generarGrafica([foto(2027, 1, '1000000.07'), foto(2027, 2, '2500000.99')], { serie: 'total' });
    const etiquetas = etiquetasY(svg);
    assert.ok(etiquetas.length >= 4);
    assert.ok(etiquetas.every(e => /^\d{1,3}(,\d{3})*$/.test(e)), etiquetas.join(' | '));
});

test('serie líquida: usa el líquido (valores, descripción, título y color distintos del total)', () => {
    const fotos = [foto(2027, 1, '1000.01', '100.10'), foto(2027, 2, '2000.02', '50.05')];
    const total = generarGrafica(fotos, { serie: 'total' });
    const liquido = generarGrafica(fotos, { serie: 'liquido' });
    assert.match(total, /Patrimonio total de 1,000\.01 en ene 2027 a 2,000\.02 en feb 2027/);
    assert.match(liquido, /Patrimonio líquido de 100\.10 en ene 2027 a 50\.05 en feb 2027/);
    assert.match(liquido, /Patrimonio líquido: evolución mensual/);
    assert.match(total, /var\(--accent-primary\)/);
    assert.match(liquido, /var\(--color-success\)/);
    assert.doesNotMatch(liquido, /var\(--accent-primary\)/);
    const [a, b] = circulos(liquido);
    assert.ok(b.y > a.y, 'el líquido baja: el punto baja');
});

test('foto estimada o reconstruida: rombo hueco y tramo punteado; las reales, círculo y tramo continuo', () => {
    const svg = generarGrafica([
        foto(2027, 1, '10.01'),
        foto(2027, 2, '20.02', '20.02', { origen: 'reconstruida' }),
        foto(2027, 3, '30.03', '30.03', { calidad: 'estimada' }),
        foto(2027, 4, '40.04'),
        foto(2027, 5, '50.05', '50.05', { origen: 'manual', calidad: 'con_aviso' }),
    ], { serie: 'total' });
    assert.equal(rombos(svg), 2);
    assert.equal(circulos(svg).length, 3);
    // tramos 1-2, 2-3 y 3-4 punteados; 4-5 continuo
    assert.equal([...svg.matchAll(/class="tramo-estimado"[^>]*stroke-dasharray="6 4"/g)].length, 3);
    assert.equal([...svg.matchAll(/class="tramo-real"/g)].length, 1);
    assert.doesNotMatch(svg.match(/<path class="tramo-real"[^>]*>/)[0], /dasharray/);
});

test('los colores son variables CSS de la aplicación, no valores fijos (sirve en claro y oscuro)', () => {
    const svg = generarGrafica([foto(2027, 1, '-5.01'), foto(2027, 2, '9.02')], { serie: 'total' });
    assert.doesNotMatch(svg, /#[0-9a-fA-F]{3,6}\b|rgb\(|rgba\(/);
    for (const v of ['--accent-primary', '--border-color', '--text-secondary', '--color-danger']) assert.ok(svg.includes(`var(${v})`), v);
});

test('etiquetas del eje horizontal: pocas, legibles y siempre la última', () => {
    const fotos = [];
    for (let m = 1; m <= 12; m++) fotos.push(foto(2027, m, `${100 + m}.0${m % 10}`));
    fotos.push(foto(2028, 1, '120.01'));
    const svg = generarGrafica(fotos, { serie: 'total' });
    const etiquetas = [...svg.matchAll(/class="eje-x"[^>]*>([^<]*)</g)].map(m => m[1]);
    assert.ok(etiquetas.length <= 8, etiquetas.join());
    assert.equal(etiquetas.at(-1), 'ene 2028');
    assert.equal(etiquetas[0], 'ene 2027');
});

test('descripción: mínimo y máximo con su mes', () => {
    const texto = describirGrafica([foto(2027, 1, '500.50'), foto(2027, 2, '100.10'), foto(2027, 3, '900.90'), foto(2027, 4, '700.70')], 'total');
    assert.match(texto, /Mínimo 100\.10 en feb 2027; máximo 900\.90 en mar 2027/);
    assert.match(texto, /de 500\.50 en ene 2027 a 700\.70 en abr 2027/);
    assert.equal(describirGrafica([], 'total'), '');
});

test('el id de la gráfica se escapa en el marcado', () => {
    const svg = generarGrafica([foto(2027, 1, '10.01'), foto(2027, 2, '20.02')], { serie: 'total', id: 'x"><script>alert(1)</script>' });
    assert.doesNotMatch(svg, /<script>/);
    assert.match(svg, /id="x&quot;&gt;&lt;script&gt;/);
});

test('un importe ilegible no se traga en silencio: lanza', () => {
    assert.throws(() => generarGrafica([foto(2027, 1, 'abc'), foto(2027, 2, '1.00')], { serie: 'total' }), /ilegible/);
});
