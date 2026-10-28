// El arranque de la aplicación (`app.ts`): navegar entre pestañas, procesar las
// suscripciones pendientes al iniciar, el tema claro/oscuro y la navegación superior.
//
// Era un script clásico que llamaba al global `appUI`; ahora es un módulo que recibe
// el enrutador, los avisos, el DOM y la API. `navigate` ya no cuelga de `window`: lo llama la
// acción general `navegar` de la delegación de eventos (`data-accion="navegar"`). Se prueba
// con un DOM de juguete.

import test from 'node:test';
import assert from 'node:assert/strict';
import { iniciarAplicacion } from '../../../src/js/app.js';

/** Un elemento con `classList` y los oyentes que registra. */
const elemento = (extra = {}) => {
    const clases = new Set(extra.clases ?? []);
    const oyentes = {};
    const atributos = new Map();
    return {
        id: extra.id ?? '', textContent: '',
        classList: { add: c => clases.add(c), remove: c => clases.delete(c), toggle: c => (clases.has(c) ? clases.delete(c) : clases.add(c)), contains: c => clases.has(c) },
        addEventListener: (tipo, fn) => { oyentes[tipo] = fn; },
        setAttribute: (nombre, valor) => atributos.set(nombre, valor),
        removeAttribute: nombre => atributos.delete(nombre),
        getAttribute: nombre => atributos.get(nombre) ?? null,
        pulsar() { oyentes.click(); },
        clases,
    };
};

function montar({ cargos = [], fallos = [], falla = false, tema = null, colapsado = null, listo = true, foto = { creada: false, foto: null }, fallaFoto = false } = {}) {
    const navs = ['dashboard', 'gastos', 'ajustes'].map(r => elemento({ id: `nav-btn-${r}`, clases: r === 'ajustes' ? ['nav-item', 'active'] : ['nav-item'] }));
    const botones = { 'theme-toggle-btn': elemento() };
    const sidebar = elemento();
    const cuerpo = elemento();
    const almacen = new Map();
    if (tema) almacen.set('desktop-theme', tema);
    if (colapsado !== null) almacen.set('sidebar-collapsed', colapsado);
    const programados = [], mostrados = [], avisos = [], escuchas = {}, todas = {}, llamadasDeVista = [];
    const previos = { document: globalThis.document, window: globalThis.window, localStorage: globalThis.localStorage, setTimeout: globalThis.setTimeout };
    globalThis.document = {
        readyState: listo ? 'complete' : 'loading', body: cuerpo,
        addEventListener: (tipo, fn) => { escuchas[tipo] = fn; (todas[tipo] ??= []).push(fn); },
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
        api: {
            asegurarFotoDelMes: () => (fallaFoto ? Promise.reject(new Error('base ocupada')) : Promise.resolve(foto)),
            procesarSuscripciones: () => (falla ? Promise.reject(new Error('sin red')) : Promise.resolve({ cargos, fallos })) },
        acciones: { accionDeVista: (...args) => llamadasDeVista.push(args) },
    });
    // Pulsar un elemento con `data-accion="navegar"`: lo que hace un botón del menú.
    const navigate = ruta => escuchas.click({
        target: { getAttribute: n => (n === 'data-accion' ? 'navegar' : n === 'data-args' ? JSON.stringify([ruta]) : null), parentElement: null },
        cancelBubble: false, preventDefault() {},
    });
    return {
        navs, botones, sidebar, cuerpo, almacen, programados, mostrados, avisos, escuchas, todas, navigate, llamadasDeVista,
        restaurar: () => { Object.assign(globalThis, previos); },
    };
}
async function conApp(opciones, prueba) {
    const t = montar(opciones);
    try { await prueba(t); } finally { t.restaurar(); }
}
const asentar = () => new Promise(r => setImmediate(r));

test('al arrancar muestra el Dashboard, y deja enganchada la delegación de eventos', () => conApp({}, t => {
    assert.deepEqual(t.mostrados, ['dashboard']);
    for (const tipo of ['click', 'submit', 'change', 'input']) assert.equal(typeof t.escuchas[tipo], 'function', tipo);
    assert.equal(globalThis.window.navigate, undefined, 'ya no hay manejadores en línea que necesiten un global');
}));

test('navigate marca la pestaña activa y pide dibujarla', () => conApp({}, t => {
    t.navigate('gastos');
    assert.deepEqual(t.navs.map(n => n.clases.has('active')), [false, true, false]);
    assert.equal(t.navs[1].getAttribute('aria-current'), 'page');
    assert.equal(t.navs[0].getAttribute('aria-current'), null);
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

test('al arrancar asegura la foto del patrimonio: si ya existía no avisa nada', () => conApp({}, async t => {
    await asentar();
    assert.deepEqual(t.avisos, []);
}));

test('si hubo que tomar la foto del mes lo avisa, y si quedó con aviso lo dice como error', async () => {
    await conApp({ foto: { creada: true, foto: { calidad: 'completa' } } }, async t => {
        await asentar();
        assert.deepEqual(t.avisos, [{ mensaje: 'Se tomó la foto del patrimonio de este mes.', tipo: undefined }]);
    });
    await conApp({ foto: { creada: true, foto: { calidad: 'con_aviso' } } }, async t => {
        await asentar();
        assert.equal(t.avisos.length, 1);
        assert.equal(t.avisos[0].tipo, 'error');
        assert.match(t.avisos[0].mensaje, /con aviso/);
    });
});

test('si la foto del patrimonio falla, se avisa sin bloquear el arranque ni los cobros', () => conApp({ fallaFoto: true, cargos: ['Cargo de Netflix'] }, async t => {
    await asentar();
    assert.deepEqual(t.mostrados, ['dashboard'], 'la pantalla inicial se mostró igual');
    assert.equal(t.avisos.length, 1);
    assert.match(t.avisos[0].mensaje, /No se pudo tomar la foto del patrimonio.*base ocupada/);
    t.programados[0].fn();
    await asentar();
    assert.ok(t.avisos.some(a => a.mensaje === 'Cargo de Netflix'), 'las suscripciones se procesan igual');
}));

test('a los 1,5 s procesa las suscripciones: avisa cada cargo y redibuja la pestaña activa', () => conApp({ cargos: ['Cargo de Netflix', 'Cargo de Spotify'] }, async t => {
    assert.equal(t.programados[0].ms, 1500);
    t.navigate('ajustes');                  // el titular ya está en otra pestaña cuando llegan los cargos
    t.programados[0].fn();
    await asentar();
    assert.deepEqual(t.avisos, [{ mensaje: 'Cargo de Netflix', tipo: 'success' }, { mensaje: 'Cargo de Spotify', tipo: 'success' }]);
    assert.deepEqual(t.mostrados, ['dashboard', 'ajustes', 'ajustes'], 'recarga la pestaña activa para reflejar los balances');
}));

test('un cobro que falló se avisa como error, sin impedir los cargos de las demás ni redibujar por él', () => conApp({ cargos: ['Cargo de Netflix'], fallos: ['No se pudo cobrar «Rota»: boom. Se reintentará al abrir la aplicación.'] }, async t => {
    t.programados[0].fn();
    await asentar();
    assert.deepEqual(t.avisos, [
        { mensaje: 'No se pudo cobrar «Rota»: boom. Se reintentará al abrir la aplicación.', tipo: 'error' },
        { mensaje: 'Cargo de Netflix', tipo: 'success' },
    ]);
    assert.deepEqual(t.mostrados, ['dashboard', 'dashboard'], 'hubo un cargo: se redibuja la pestaña activa');
}));

test('si solo hubo fallos se avisan, pero no se redibuja: ningún saldo cambió', () => conApp({ fallos: ['No se pudo cobrar «Rota»: boom.'] }, async t => {
    t.programados[0].fn();
    await asentar();
    assert.deepEqual(t.avisos, [{ mensaje: 'No se pudo cobrar «Rota»: boom.', tipo: 'error' }]);
    assert.deepEqual(t.mostrados, ['dashboard']);
}));

test('sin cargos pendientes no avisa ni redibuja; y si falla, no rompe nada', () => conApp({}, async t => {
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

test('el tema por defecto es claro; se respeta el oscuro guardado', () => {
    return conApp({}, claro => {
        assert.equal(claro.botones['theme-toggle-btn'].textContent, '☀️');
        assert.equal(claro.cuerpo.clases.has('light-theme'), true);
    }).then(() => conApp({ tema: 'dark' }, oscuro => {
        assert.equal(oscuro.botones['theme-toggle-btn'].textContent, '🌙');
        assert.equal(oscuro.cuerpo.clases.has('light-theme'), false);
    }));
}

);

test('cambiar de tema lo guarda, lo avisa y redibuja la pestaña activa', () => conApp({}, t => {
    t.navigate('ajustes');
    t.botones['theme-toggle-btn'].pulsar();
    assert.equal(t.cuerpo.clases.has('light-theme'), false);
    assert.equal(t.almacen.get('desktop-theme'), 'dark');
    assert.deepEqual(t.avisos.at(-1), { mensaje: 'Modo Oscuro activado.', tipo: undefined });
    assert.equal(t.mostrados.at(-1), 'ajustes');
    t.botones['theme-toggle-btn'].pulsar();
    assert.equal(t.almacen.get('desktop-theme'), 'light');
    assert.equal(t.botones['theme-toggle-btn'].textContent, '☀️');
    assert.deepEqual(t.avisos.at(-1), { mensaje: 'Modo Claro activado.', tipo: undefined });
}));

test('la navegación superior ignora la preferencia obsoleta de menú colapsado', () => conApp({ colapsado: 'true' }, t => {
    assert.equal(t.sidebar.clases.has('collapsed'), false);
    assert.equal(t.botones['sidebar-collapse-btn'], undefined);
    assert.equal(t.almacen.get('sidebar-collapsed'), 'true');
}));

test('al arrancar engancha además la máscara de fechas: un campo con data-mascara="fecha" recibe la «/» sola', () => conApp({}, t => {
    assert.equal(t.todas.input.length, 2, 'la delegación de eventos y la máscara de fechas escuchan «input»');
    const atributos = { 'data-mascara': 'fecha' };
    const campo = { value: '2810', selectionStart: 4, getAttribute: n => atributos[n] ?? null, parentElement: null, setSelectionRange() {} };
    for (const oyente of t.todas.input) oyente({ target: campo, inputType: 'insertText', cancelBubble: false });
    assert.equal(campo.value, '28/10/');
}));
