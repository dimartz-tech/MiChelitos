// Pruebas de la vista «Evolución» en Node, con dobles y datos inventados.

import test from 'node:test';
import assert from 'node:assert/strict';
import { VistaEvolucion } from '../../../src/js/vistas/evolucion.js';

const partida = (rubro, referencia, divisa, importe, importe_dop) => ({ rubro, referencia, divisa, importe, importe_dop });

const foto = (anio, mes, patrimonio, liquido, extra = {}) => ({
    id: anio * 100 + mes, anio, mes, fecha: `15/${String(mes).padStart(2, '0')}/${anio}`, origen: 'real', sellada: false,
    tasa_usd_dop: '60.00', activos: patrimonio, pasivos: '0.00', patrimonio, liquido, calidad: 'completa', partidas: [], ...extra,
});

function crear({ fotos = [], error = null } = {}) {
    const pantalla = { contenido: { innerHTML: '' } };
    const llamadas = [];
    const avisos = [];
    const vista = new VistaEvolucion({
        api: {
            obtenerFotosPatrimonio: async () => { llamadas.push('obtenerFotosPatrimonio'); if (error) throw error; return fotos; },
            tomarFotoPatrimonio: async (...a) => { llamadas.push(['tomar', ...a]); return fotos[0]; },
        },
        avisos: { mostrar: (m, t) => avisos.push({ m, t }) },
        pantalla,
        dom: { elemento: () => ({ value: '' }), buscar: () => null },
        enrutador: { mostrar: async () => {} },
    });
    return { vista, pantalla, llamadas, avisos, html: () => pantalla.contenido.innerHTML };
}

test('sin fotos: lo dice, no dibuja gráfica ni tabla y deja tomar la primera', async () => {
    const t = crear();
    await t.vista.render();
    assert.match(t.html(), /Aún no hay fotos del patrimonio/);
    assert.doesNotMatch(t.html(), /<svg|<table/);
    assert.match(t.html(), /data-accion="handleTomarFotoEvolucion"/);
});

test('una sola foto: pide otra para mostrar variación, sin gráfica vacía', async () => {
    const t = crear({ fotos: [foto(2027, 3, '1500.37', '700.01', { partidas: [partida('efectivo', 'Caja', 'DOP', '700.01', '700.01')] })] });
    await t.vista.render();
    assert.match(t.html(), /Se necesita al menos otra foto para mostrar variación/);
    assert.doesNotMatch(t.html(), /<svg/);
    assert.doesNotMatch(t.html(), /id="evo-tabla"/);
    assert.match(t.html(), /id="evo-partidas"/);
});

test('error del comando: lo muestra escapado, con role="alert", y sigue viva la pantalla', async () => {
    const t = crear({ error: 'falló <b>algo</b> "grave"' });
    await t.vista.render();
    assert.match(t.html(), /role="alert"/);
    assert.match(t.html(), /No se pudo leer el historial/);
    assert.match(t.html(), /falló &lt;b&gt;algo&lt;\/b&gt; &quot;grave&quot;/);
    assert.doesNotMatch(t.html(), /<b>algo/);
});

const FOTOS = [
    // desordenadas a propósito
    foto(2027, 3, '1500.37', '700.07', { origen: 'capital', calidad: 'con_aviso', partidas: [
        partida('efectivo', 'Caja <img src=x onerror=alert(1)>', 'DOP', '700.07', '700.07'),
        partida('tarjeta', 'Visa', 'USD', '-12.34', '-740.40'),
    ] }),
    foto(2027, 1, '1000.01', '500.05', { origen: 'real', pasivos: '20.02' }),
    foto(2027, 2, '1200.50', '600.06'),
];

test('con fotos: resumen del último mes con monto, porcentaje, signo, flecha y texto', async () => {
    const t = crear({ fotos: FOTOS });
    await t.vista.render();
    const resumen = t.html().match(/id="evo-resumen"[\s\S]*?<\/div>/)[0];
    assert.match(resumen, /mar 2027/);
    assert.match(resumen, /1,500\.37/);
    assert.match(resumen, /frente a feb 2027/);
    assert.match(resumen, /▲ \+299\.87 \(\+24\.98 %\)/);
    assert.match(resumen, /sube/);
});

test('tabla accesible: caption, th scope, orden más reciente primero y variación por fila', async () => {
    const t = crear({ fotos: FOTOS });
    await t.vista.render();
    const tabla = t.html().match(/<table class="table-modern" id="evo-tabla">[\s\S]*?<\/table>/)[0];
    assert.match(tabla, /<caption/);
    assert.equal([...tabla.matchAll(/<th scope="col"/g)].length, 8);
    const filas = [...tabla.matchAll(/<th scope="row">([^<]*)<\/th>/g)].map(m => m[1]);
    assert.deepEqual(filas, ['mar 2027', 'feb 2027', 'ene 2027']);
    // la más antigua no tiene anterior
    const ultima = tabla.slice(tabla.lastIndexOf('<tr>'));
    assert.match(ultima, /—/);
    assert.match(tabla, /▲ \+200\.49 \(\+20\.05 %\)/);
    assert.match(tabla, /20\.02/); // pasivos de ene
    assert.match(tabla, /Al guardar el capital/);
    assert.match(tabla, /15\/03\/2027/);
});

test('calidad: con_aviso se ve con texto y símbolo, no solo con color', async () => {
    const t = crear({ fotos: FOTOS });
    await t.vista.render();
    assert.match(t.html(), /<span class="badge danger">⚠ Con aviso<\/span>/);
    assert.match(t.html(), /Completa/);
});

test('variación a la baja: signo, flecha ▼ y texto «baja»', async () => {
    const t = crear({ fotos: [foto(2027, 1, '1000.01', '1.00'), foto(2027, 2, '900.01', '1.00')] });
    await t.vista.render();
    assert.match(t.html(), /▼ -100\.00 \(-10\.00 %\)/);
    assert.match(t.html(), /baja/);
});

test('variación sin cambio: «sin cambio», no «sube» ni «baja»', async () => {
    const t = crear({ fotos: [foto(2027, 1, '1000.01', '1.00'), foto(2027, 2, '1000.01', '1.00')] });
    await t.vista.render();
    const resumen = t.html().match(/id="evo-resumen"[\s\S]*?<\/div>/)[0];
    assert.match(resumen, /■ 0\.00 \(0\.00 %\)/);
    assert.match(resumen, /sin cambio/);
});

test('desglose: la foto más reciente por defecto; rubro, referencia escapada, divisa, importe e importe en DOP', async () => {
    const t = crear({ fotos: FOTOS });
    await t.vista.render();
    const tabla = t.html().match(/<table class="table-modern" id="evo-partidas">[\s\S]*?<\/table>/)[0];
    assert.match(tabla, /<th scope="row">efectivo<\/th>/);
    assert.match(tabla, /Caja &lt;img src=x onerror=alert\(1\)&gt;/);
    assert.doesNotMatch(t.html(), /<img src=x/);
    assert.match(tabla, /<td>USD<\/td>/);
    assert.match(tabla, /-12\.34/);
    assert.match(tabla, /-740\.40/);
    assert.match(t.html(), /<option value="2027-03" selected>mar 2027<\/option>/);
    assert.match(t.html(), /Tasa USD→DOP de esta foto: 60\.00/);
});

test('cambiar de mes muestra las partidas de esa foto; un mes que ya no existe vuelve a la más reciente', async () => {
    const t = crear({ fotos: FOTOS });
    await t.vista.handleCambiarMesEvolucion('2027-01');
    assert.match(t.html(), /<option value="2027-01" selected>ene 2027<\/option>/);
    assert.match(t.html(), /Esta foto no tiene partidas/);
    await t.vista.handleCambiarMesEvolucion('1999-09');
    assert.match(t.html(), /<option value="2027-03" selected>mar 2027<\/option>/);
});

test('cambiar de serie: la gráfica pasa a líquido (y una serie desconocida se queda en total)', async () => {
    const t = crear({ fotos: FOTOS });
    await t.vista.render();
    assert.match(t.html(), /Patrimonio total de 1,000\.01 en ene 2027 a 1,500\.37 en mar 2027/);
    assert.match(t.html(), /<option value="total" selected>/);
    await t.vista.handleCambiarSerieEvolucion('liquido');
    assert.match(t.html(), /Patrimonio líquido de 500\.05 en ene 2027 a 700\.07 en mar 2027/);
    assert.match(t.html(), /<option value="liquido" selected>/);
    await t.vista.handleCambiarSerieEvolucion('<raro>');
    assert.match(t.html(), /Patrimonio total de/);
});

test('gráfica y tabla conviven: el svg es accesible y la tabla es la alternativa', async () => {
    const t = crear({ fotos: FOTOS });
    await t.vista.render();
    assert.match(t.html(), /<svg [^>]*role="img"[^>]*aria-labelledby="grafica-evolucion-titulo grafica-evolucion-desc"/);
    assert.match(t.html(), /<label for="evo_serie"/);
    assert.match(t.html(), /<label for="evo_mes"/);
    assert.match(t.html(), /<label for="evo_tasa"/);
});

test('el campo de la tasa sugiere 60.00, es de texto y no lleva máscara de fecha', async () => {
    const t = crear();
    await t.vista.render();
    const campo = t.html().match(/<input [^>]*id="evo_tasa"[^>]*>/)[0];
    assert.match(campo, /type="text"/);
    assert.match(campo, /placeholder="60\.00"/);
    assert.doesNotMatch(campo, /data-mascara/);
});

test('ningún atributo on*= en la plantilla: solo data-accion*', async () => {
    const t = crear({ fotos: [foto(2027, 1, '1000.01', '500.05'), foto(2027, 2, '1200.50', '600.06')] });
    await t.vista.render();
    assert.doesNotMatch(t.html(), /\son[a-z]+\s*=\s*["']/i);
    assert.match(t.html(), /data-accion-cambiar="handleCambiarSerieEvolucion"[^>]*data-pasa="valor"/);
});
