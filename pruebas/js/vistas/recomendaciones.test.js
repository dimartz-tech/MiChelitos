// Recomendaciones del historial del patrimonio: las funciones puras que rotulan lo que calculó Rust y el bloque de
// Evolución. Datos inventados. Se importa el JavaScript compilado (`npm test` compila antes).

import test from 'node:test';
import assert from 'node:assert/strict';
import {
    AVISO_NO_ASESORIA, etiquetaDeDestino, etiquetaDeSeveridad, htmlDeRecomendacion, htmlRecomendaciones, nombreDeRegla, textoDeCifra,
} from '../../../src/js/vistas/recomendaciones.js';

const cifra = (etiqueta, tipo, valor) => ({ etiqueta, tipo, valor });
const reco = (extra = {}) => ({
    id: 'tendencia_gasto:7', regla: 'tendencia_gasto', severidad: 'atencion', titulo: 'El gasto variable en «Salidas inventadas» viene subiendo',
    explicacion: 'Hasta 10/2026 el promedio sube.', regla_texto: 'Sube si pasa del 10 % y de DOP 500.00.',
    cifras: [cifra('Promedio de los 3 últimos meses', 'importe', '1700.16'), cifra('Cambio', 'porcentaje', '70.0'), cifra('Mes', 'texto', '10/2026')],
    destino: 'gastos', ...extra,
});
const resultado = (extra = {}) => ({
    aviso: 'Reglas descriptivas, no asesoría financiera: muestran qué dicen tus propios datos.',
    anio_gastos: 2026, mes_gastos: 10, anio_presupuestos: 2026, mes_presupuestos: 11,
    recomendaciones: [reco()], omitidas: 0, maximo_por_severidad: 5, reglas_evaluadas: 3, sin_datos: [], usd_excluido: [], ...extra,
});

test('cada tipo de cifra se rotula sobre el texto exacto, sin pasar por Number', () => {
    assert.equal(textoDeCifra(cifra('x', 'importe', '1234567.89')), 'DOP 1,234,567.89');
    assert.equal(textoDeCifra(cifra('x', 'importe', '-562.51')), 'DOP -562.51');
    assert.equal(textoDeCifra(cifra('x', 'importe_usd', '12.34')), 'USD 12.34');
    assert.equal(textoDeCifra(cifra('x', 'porcentaje', '81.5')), '81.5 %');
    assert.equal(textoDeCifra(cifra('x', 'tasa', '58.75')), '58.75');
    assert.equal(textoDeCifra(cifra('x', 'texto', '10/2026')), '10/2026');
    // Un importe enorme conserva todos sus dígitos (Number los perdería).
    assert.equal(textoDeCifra(cifra('x', 'importe', '99999999999999999.99')), 'DOP 99,999,999,999,999,999.99');
});

test('la severidad y las reglas tienen nombre en palabras', () => {
    assert.equal(etiquetaDeSeveridad('atencion'), 'Atención');
    assert.equal(etiquetaDeSeveridad('info'), 'Informativa');
    assert.equal(nombreDeRegla('deuda_y_ahorro'), 'Deuda y ahorro');
    assert.equal(nombreDeRegla('desconocida'), 'desconocida');
});

test('solo hay enlace a las pestañas conocidas; «evolucion» (la actual) y lo raro no se enlazan', () => {
    assert.equal(etiquetaDeDestino('gastos'), 'Gastos');
    assert.equal(etiquetaDeDestino('prestamos'), 'Deudas');
    assert.equal(etiquetaDeDestino('evolucion'), null);
    assert.equal(etiquetaDeDestino('javascript:alert(1)'), null);
    assert.doesNotMatch(htmlDeRecomendacion(reco({ destino: 'evolucion' })), /data-accion="navegar"/);
    assert.doesNotMatch(htmlDeRecomendacion(reco({ destino: '"><script>' })), /data-accion|<script/);
});

test('una recomendación muestra severidad en palabras y símbolo, cifras, regla y enlace; sin on*=', () => {
    const html = htmlDeRecomendacion(reco());
    assert.match(html, /data-severidad="atencion"/);
    assert.match(html, /<span aria-hidden="true">⚠<\/span> Atención/);
    assert.match(html, /DOP 1,700\.16/);
    assert.match(html, /70\.0 %/);
    assert.match(html, /<strong>Regla:<\/strong> Sube si pasa del 10 %/);
    assert.match(html, /data-accion="navegar" data-args="\[&quot;gastos&quot;\]"/);
    assert.match(html, /Ver en Gastos/);
    assert.doesNotMatch(html, /\son[a-z]+\s*=\s*["']/i);
    assert.match(htmlDeRecomendacion(reco({ severidad: 'info' })), /ℹ<\/span> Informativa/);
});

test('el texto de Rust (título, explicación, cifras, regla, id) se escapa', () => {
    const hostil = '<img src=x onerror=alert(1)>"\'';
    const html = htmlDeRecomendacion(reco({
        id: hostil, titulo: hostil, explicacion: hostil, regla_texto: hostil, cifras: [cifra(hostil, 'texto', hostil)],
    }));
    assert.doesNotMatch(html, /<img/);
    assert.match(html, /&lt;img src=x onerror=alert\(1\)&gt;&quot;&#39;/);
});

test('el bloque siempre lleva el aviso de que no es asesoría financiera, con o sin respuesta', () => {
    assert.equal(AVISO_NO_ASESORIA, 'Reglas descriptivas, no asesoría financiera.');
    for (const html of [htmlRecomendaciones(resultado()), htmlRecomendaciones(null, 'x'), htmlRecomendaciones(null)]) {
        assert.match(html, /<strong>Reglas descriptivas, no asesoría financiera\.<\/strong>/);
        assert.match(html, /Cómo aumentar tu patrimonio/);
        assert.match(html, /aria-labelledby="evo-reco-titulo"/);
    }
});

test('con recomendaciones: periodos, tarjetas en orden, y nada de «sin datos» si no hay', () => {
    const html = htmlRecomendaciones(resultado({ recomendaciones: [reco({ id: 'a:1', titulo: 'Primera' }), reco({ id: 'b:2', titulo: 'Segunda', severidad: 'info' })] }));
    assert.match(html, /último mes completo \(10\/2026\)/);
    assert.match(html, /mes en curso \(11\/2026\)/);
    assert.ok(html.indexOf('Primera') < html.indexOf('Segunda'));
    assert.equal([...html.matchAll(/<article class="reco"/g)].length, 2);
    assert.doesNotMatch(html, /Reglas sin datos suficientes/);
    assert.doesNotMatch(html, /más que no se muestran/);
});

test('sin datos suficientes lo dice en vez de inventar y lista los motivos de cada regla', () => {
    const html = htmlRecomendaciones(resultado({
        recomendaciones: [], reglas_evaluadas: 0,
        sin_datos: [{ regla: 'variacion_patrimonio', motivo: 'Se necesitan al menos 2 fotos.' }, { regla: 'presupuesto', motivo: 'No hay <b>presupuestos</b>.' }],
    }));
    assert.match(html, /Aún no hay datos suficientes para recomendar nada/);
    assert.doesNotMatch(html, /<article/);
    assert.match(html, /Reglas sin datos suficientes \(2\)/);
    assert.match(html, /<strong>Variación del patrimonio:<\/strong> Se necesitan al menos 2 fotos\./);
    assert.match(html, /No hay &lt;b&gt;presupuestos&lt;\/b&gt;/);
});

test('con reglas evaluadas pero ninguna activa, dice que no hay nada que señalar (no «faltan datos»)', () => {
    const html = htmlRecomendaciones(resultado({ recomendaciones: [], reglas_evaluadas: 4 }));
    assert.match(html, /ninguna regla se activa/);
    assert.doesNotMatch(html, /Aún no hay datos suficientes/);
});

test('las recomendaciones omitidas por el límite se cuentan, con el máximo', () => {
    const html = htmlRecomendaciones(resultado({ omitidas: 3 }));
    assert.match(html, /Hay 3 recomendación\(es\) más que no se muestran \(como máximo 5 de cada severidad\)/);
});

test('los dólares excluidos se informan aparte, sin convertir', () => {
    const html = htmlRecomendaciones(resultado({ usd_excluido: [{ origen: 'gastos', importe: '45.17' }, { origen: 'tarjetas', importe: '1025.50' }, { origen: 'raro<x>', importe: '1.00' }] }));
    assert.match(html, /Fuera del análisis, sin convertir/);
    assert.match(html, /USD 45\.17 en gastos del último mes completo/);
    assert.match(html, /USD 1,025\.50 en deuda de tarjetas/);
    assert.match(html, /USD 1\.00 en raro&lt;x&gt;/);
    assert.doesNotMatch(htmlRecomendaciones(resultado()), /reco-usd/);
});

test('si Rust no responde: alerta accesible con el motivo escapado', () => {
    const html = htmlRecomendaciones(null, 'falló <script>');
    assert.match(html, /<p role="alert"[^>]*>No se pudieron calcular las recomendaciones: falló &lt;script&gt;<\/p>/);
    assert.doesNotMatch(html, /<script>/);
    assert.match(htmlRecomendaciones(null), /No se pudieron calcular las recomendaciones\./);
});
