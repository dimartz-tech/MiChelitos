// Los servicios reales que salieron de la clase `AppUI` (retirada en la 1.65.0):
// los avisos, el formato, el motivo de una corrección y el enrutador. Antes eran
// métodos de una clase de 100 métodos que solo se podía probar entera; ahora cada
// uno se prueba solo, con dobles.

import test from 'node:test';
import assert from 'node:assert/strict';
import { crearAvisos, formatoDelNavegador, crearMotivo, crearEnrutador, referenciasDelNavegador, TASA_USD_A_DOP } from '../../../src/js/ui/servicios.js';

// --- Avisos ---

/** Un elemento mínimo con `classList`, que anota lo que le pasa. */
function elementoDeAviso() {
    const clases = new Set();
    return {
        className: '', innerHTML: '', quitado: false,
        classList: { add: c => clases.add(c), remove: c => clases.delete(c), contiene: c => clases.has(c) },
        remove() { this.quitado = true; },
    };
}

function montarAvisos() {
    const hijos = [], programados = [];
    const avisos = crearAvisos({
        contenedor: { appendChild: h => hijos.push(h) },
        crear: elementoDeAviso,
        programar: (accion, ms) => { programados.push({ accion, ms }); return programados.length; },
    });
    return { avisos, hijos, programados };
}

test('un aviso de éxito lleva ✅, su mensaje y la clase toast-success', () => {
    const t = montarAvisos();
    t.avisos.mostrar('Hecho');
    assert.equal(t.hijos.length, 1);
    assert.equal(t.hijos[0].className, 'toast toast-success');
    assert.match(t.hijos[0].innerHTML, /✅/);
    assert.match(t.hijos[0].innerHTML, /<span>Hecho<\/span>/);
});

test('un error o un aviso informativo llevan ⚠️ y su clase', () => {
    for (const tipo of ['error', 'info']) {
        const t = montarAvisos();
        t.avisos.mostrar('Ojo', tipo);
        assert.equal(t.hijos[0].className, `toast toast-${tipo}`);
        assert.match(t.hijos[0].innerHTML, /⚠️/);
        assert.doesNotMatch(t.hijos[0].innerHTML, /✅/);
    }
});

test('el aviso aparece a los 100 ms, se quita de la vista a los 4 s y se retira 400 ms después', () => {
    const t = montarAvisos();
    t.avisos.mostrar('Hecho');
    assert.deepEqual(t.programados.map(p => p.ms), [100, 4000]);
    t.programados[0].accion();
    assert.equal(t.hijos[0].classList.contiene('show'), true);
    t.programados[1].accion();
    assert.equal(t.hijos[0].classList.contiene('show'), false);
    assert.equal(t.programados[2].ms, 400);
    assert.equal(t.hijos[0].quitado, false);
    t.programados[2].accion();
    assert.equal(t.hijos[0].quitado, true);
});

test('varios avisos se apilan, cada uno con sus propios temporizadores', () => {
    const t = montarAvisos();
    t.avisos.mostrar('Uno');
    t.avisos.mostrar('Dos', 'error');
    assert.equal(t.hijos.length, 2);
    assert.equal(t.programados.length, 4);
});

// --- Formato y referencias ---

test('los importes salen como el resto de la aplicación: separador de millar y dos decimales', () => {
    assert.equal(formatoDelNavegador.importe(1234.5), '1,234.50');
    assert.equal(formatoDelNavegador.importe(0), '0.00');
    assert.equal(formatoDelNavegador.importe(-5), '-5.00');
    assert.equal(formatoDelNavegador.importe('99.999'), '100.00');
    assert.equal(formatoDelNavegador.importe('12'), '12.00');
});

test('la tasa de referencia del dólar vive en un solo sitio', () => {
    assert.equal(TASA_USD_A_DOP, 60);
    assert.equal(referenciasDelNavegador.tasaUsdADop, TASA_USD_A_DOP);
});

// --- Motivo de una corrección ---

const MOTIVO_BUENO = 'Registrado dos veces por error';

function montarMotivo(respuesta) {
    const preguntas = [], avisos = [];
    const motivo = crearMotivo(
        { confirmar: async () => true, preguntar: async m => { preguntas.push(m); return new Promise(r => setTimeout(() => r(respuesta), 2)); } },
        { mostrar: (mensaje, tipo) => avisos.push({ mensaje, tipo }) },
    );
    return { motivo, preguntas, avisos };
}

test('el motivo se pide con lo que ocurre, su consecuencia y la instrucción, y se devuelve tal cual', async () => {
    const t = montarMotivo(MOTIVO_BUENO);
    const motivo = await t.motivo.pedir('Vas a borrar este gasto', 'Esto **destruye el movimiento**.');
    assert.equal(motivo, MOTIVO_BUENO);
    assert.match(t.preguntas[0], /^Vas a borrar este gasto\.\n\nEsto \*\*destruye el movimiento\*\*\.\n\n/);
    assert.match(t.preguntas[0], /una frase que siga teniendo sentido dentro de seis meses/);
    assert.deepEqual(t.avisos, []);
});

test('cancelar devuelve null sin avisar; un motivo corto devuelve null y lo explica', async () => {
    const cancelado = montarMotivo(null);
    assert.equal(await cancelado.motivo.pedir('x', 'y'), null);
    assert.deepEqual(cancelado.avisos, []);
    for (const corto of ['', '   ', 'corto', '12345678901234']) {
        const t = montarMotivo(corto);
        assert.equal(await t.motivo.pedir('x', 'y'), null, JSON.stringify(corto));
        assert.deepEqual(t.avisos, [{ mensaje: 'El motivo es demasiado corto: explica qué pasó, no solo que pasó.', tipo: 'error' }]);
    }
});

test('quince caracteres, sin contar los espacios de los extremos, bastan', async () => {
    const t = montarMotivo('  123456789012345  ');
    assert.equal(await t.motivo.pedir('x', 'y'), '  123456789012345  ');
});

// --- Enrutador ---

function montarEnrutador({ vistas = {} } = {}) {
    const pantalla = { contenido: { innerHTML: '' } };
    const dibujos = [];
    const mapa = new Map(Object.entries(vistas).map(([ruta, f]) => [ruta, { render: async () => { dibujos.push(ruta); if (f) await f(pantalla); } }]));
    return { enrutador: crearEnrutador(pantalla, mapa), pantalla, dibujos };
}

test('muestra «Cargando» mientras la vista dibuja, y deja el resultado de la vista', async () => {
    let durante;
    const t = montarEnrutador({ vistas: { gastos: pantalla => { durante = pantalla.contenido.innerHTML; pantalla.contenido.innerHTML = '<h1>Gastos</h1>'; } } });
    await t.enrutador.mostrar('gastos');
    assert.match(durante, /Cargando módulo nativo/);
    assert.equal(t.pantalla.contenido.innerHTML, '<h1>Gastos</h1>');
    assert.deepEqual(t.dibujos, ['gastos']);
});

test('una ruta que nadie reconoce cae en el Dashboard, y no dibuja ninguna otra vista', async () => {
    const t = montarEnrutador({ vistas: { dashboard: null, gastos: null } });
    await t.enrutador.mostrar('una-ruta-que-no-existe');
    assert.deepEqual(t.dibujos, ['dashboard']);
});

test('si la vista falla, se pinta el error en lugar de dejar la pantalla a medias', async () => {
    const t = montarEnrutador({ vistas: { gastos: () => { throw new Error('Rust no responde'); } } });
    await t.enrutador.mostrar('gastos');
    assert.match(t.pantalla.contenido.innerHTML, /Error al renderizar el módulo/);
    assert.match(t.pantalla.contenido.innerHTML, /Rust no responde/);
    assert.doesNotMatch(t.pantalla.contenido.innerHTML, /Cargando/);
});

test('un fallo de una pestaña no impide dibujar la siguiente', async () => {
    const t = montarEnrutador({ vistas: { gastos: () => { throw new Error('mal'); }, ingresos: pantalla => { pantalla.contenido.innerHTML = '<h1>Ingresos</h1>'; } } });
    await t.enrutador.mostrar('gastos');
    await t.enrutador.mostrar('ingresos');
    assert.equal(t.pantalla.contenido.innerHTML, '<h1>Ingresos</h1>');
});
