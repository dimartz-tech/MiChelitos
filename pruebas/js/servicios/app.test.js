// El arranque de la aplicación (`app.ts`): navegar entre pestañas, procesar las
// suscripciones pendientes al iniciar, el tema claro/oscuro y el menú lateral.
//
// Era un script clásico que llamaba al global `appUI`; ahora es un módulo que recibe
// el enrutador, los avisos, el DOM y la API. Sigue colgando `navigate` de `window`
// porque lo llaman los manejadores en línea del HTML. Se prueba con un DOM de juguete.

import test from 'node:test';
import assert from 'node:assert/strict';
import { iniciarAplicacion } from '../../../src/js/app.js';

/** Un elemento con `classList` y los oyentes que registra. */
const elemento = (extra = {}) => {
    const clases = new Set(extra.clases ?? []);
    const oyentes = {};
    return {
        id: extra.id ?? '', textContent: '',
        classList: { add: c => clases.add(c), remove: c => clases.delete(c), toggle: c => (clases.has(c) ? clases.delete(c) : clases.add(c)), contains: c => clases.has(c) },
        addEventListener: (tipo, fn) => { oyentes[tipo] = fn; },
        pulsar() { oyentes.click(); },
        clases,
    };
};

function montar({ mensajes = [], falla = false, tema = null, colapsado = null, listo = true } = {}) {
    const navs = ['dashboard', 'gastos', 'ajustes'].map(r => elemento({ id: `nav-btn-${r}`, clases: r === 'ajustes' ? ['nav-item', 'active'] : ['nav-item'] }));
    const botones = { 'theme-toggle-btn': elemento(), 'sidebar-collapse-btn': elemento() };
    const sidebar = elemento();
    const cuerpo = elemento();
    const almacen = new Map();
    if (tema) almacen.set('desktop-theme', tema);
    if (colapsado !== null) almacen.set('sidebar-collapsed', colapsado);
    const programados = [], mostrados = [], avisos = [], escuchas = {};
    const previos = { document: globalThis.document, window: globalThis.window, localStorage: globalThis.localStorage, setTimeout: globalThis.setTimeout };
    globalThis.document = {
        readyState: listo ? 'complete' : 'loading', body: cuerpo,
        addEventListener: (tipo, fn) => { escuchas[tipo] = fn; },
        querySelectorAll: () => navs,
        querySelector: sel => (sel === '.sidebar-nav .nav-item.active' ? navs.find(n => n.clases.has('active')) ?? null : sel === '.app-sidebar' ? sidebar : null),
    };
    globalThis.window = {};
    globalThis.localStorage = { getItem: k => (almacen.has(k) ? almacen.get(k) : null), setItem: (k, v) => { almacen.set(k, String(v)); } };
    globalThis.setTimeout = (fn, ms) => { programados.push({ fn, ms }); return programados.length; };
    iniciarAplicacion({
        enrutador: { mostrar: async ruta => { mostrados.push(ruta); } },
        avisos: { mostrar: (mensaje, tipo) => avisos.push({ mensaje, tipo }) },
        dom: { elemento: id => botones[id], buscar: id => botones[id] ?? navs.find(n => n.id === id) ?? null },
        api: { procesarSuscripciones: () => (falla ? Promise.reject(new Error('sin red')) : Promise.resolve(mensajes)) },
    });
    return {
        navs, botones, sidebar, cuerpo, almacen, programados, mostrados, avisos, escuchas, navigate: globalThis.window.navigate,
        restaurar: () => { Object.assign(globalThis, previos); },
    };
}
async function conApp(opciones, prueba) {
    const t = montar(opciones);
    try { await prueba(t); } finally { t.restaurar(); }
}
const asentar = () => new Promise(r => setImmediate(r));

test('al arrancar muestra el Dashboard, y navigate queda como global para los manejadores en línea', () => conApp({}, t => {
    assert.deepEqual(t.mostrados, ['dashboard']);
    assert.equal(typeof t.navigate, 'function');
}));

test('navigate marca la pestaña activa y pide dibujarla', () => conApp({}, t => {
    t.navigate('gastos');
    assert.deepEqual(t.navs.map(n => n.clases.has('active')), [false, true, false]);
    assert.deepEqual(t.mostrados, ['dashboard', 'gastos']);
}));

test('navigate a una ruta sin botón en el menú igualmente la dibuja', () => conApp({}, t => {
    t.navigate('sin-boton');
    assert.deepEqual(t.mostrados.at(-1), 'sin-boton');
    assert.deepEqual(t.navs.map(n => n.clases.has('active')), [false, false, false]);
}));

test('si el documento aún se está leyendo, espera a DOMContentLoaded para arrancar', () => conApp({ listo: false }, t => {
    assert.deepEqual(t.mostrados, []);
    t.escuchas.DOMContentLoaded();
    assert.deepEqual(t.mostrados, ['dashboard']);
}));

test('a los 1,5 s procesa las suscripciones: avisa cada cargo y redibuja la pestaña activa', () => conApp({ mensajes: ['Cargo de Netflix', 'Cargo de Spotify'] }, async t => {
    assert.equal(t.programados[0].ms, 1500);
    t.navigate('ajustes');                  // el titular ya está en otra pestaña cuando llegan los cargos
    t.programados[0].fn();
    await asentar();
    assert.deepEqual(t.avisos, [{ mensaje: 'Cargo de Netflix', tipo: 'success' }, { mensaje: 'Cargo de Spotify', tipo: 'success' }]);
    assert.deepEqual(t.mostrados, ['dashboard', 'ajustes', 'ajustes'], 'recarga la pestaña activa para reflejar los balances');
}));

test('sin cargos pendientes no avisa ni redibuja; y si falla, no rompe nada', () => conApp({ mensajes: [] }, async t => {
    t.programados[0].fn();
    await asentar();
    assert.deepEqual([t.avisos, t.mostrados], [[], ['dashboard']]);
    const previo = console.error; const errores = []; console.error = (...a) => errores.push(a);
    try {
        const roto = montar({ falla: true });
        roto.programados[0].fn();
        await asentar();
        roto.restaurar();
        assert.equal(errores.length, 1);
    } finally { console.error = previo; }
}));

test('el tema por defecto es oscuro; el guardado «light» lo arranca claro', () => {
    return conApp({}, oscuro => {
        assert.equal(oscuro.botones['theme-toggle-btn'].textContent, '🌙');
        assert.equal(oscuro.cuerpo.clases.has('light-theme'), false);
    }).then(() => conApp({ tema: 'light' }, claro => {
        assert.equal(claro.botones['theme-toggle-btn'].textContent, '☀️');
        assert.equal(claro.cuerpo.clases.has('light-theme'), true);
    }));
}

);

test('cambiar de tema lo guarda, lo avisa y redibuja la pestaña activa', () => conApp({}, t => {
    t.navigate('ajustes');
    t.botones['theme-toggle-btn'].pulsar();
    assert.equal(t.cuerpo.clases.has('light-theme'), true);
    assert.equal(t.almacen.get('desktop-theme'), 'light');
    assert.deepEqual(t.avisos.at(-1), { mensaje: 'Modo Claro activado.', tipo: undefined });
    assert.equal(t.mostrados.at(-1), 'ajustes');
    t.botones['theme-toggle-btn'].pulsar();
    assert.equal(t.almacen.get('desktop-theme'), 'dark');
    assert.equal(t.botones['theme-toggle-btn'].textContent, '🌙');
    assert.deepEqual(t.avisos.at(-1), { mensaje: 'Modo Oscuro activado.', tipo: undefined });
}));

test('el menú lateral recuerda si estaba colapsado, y al pulsar se pliega y se guarda', () => conApp({ colapsado: 'true' }, t => {
    assert.equal(t.sidebar.clases.has('collapsed'), true);
    assert.equal(t.botones['sidebar-collapse-btn'].textContent, '▶️');
    t.botones['sidebar-collapse-btn'].pulsar();
    assert.equal(t.sidebar.clases.has('collapsed'), false);
    assert.equal(t.almacen.get('sidebar-collapsed'), 'false');
    assert.equal(t.botones['sidebar-collapse-btn'].textContent, '◀️');
}));

test('el menú lateral parte desplegado si no hay nada guardado', () => conApp({}, t => {
    assert.equal(t.sidebar.clases.has('collapsed'), false);
    assert.equal(t.botones['sidebar-collapse-btn'].textContent, '◀️');
}));
