// El menú flotante real (`crearMenuFlotanteDelNavegador`), con un DOM de juguete.
//
// Es lo único de `ui/servicios.ts` con lógica propia: solo hay un menú abierto a
// la vez, el clic que lo abre no lo cierra, se cierra con el siguiente clic o con
// Escape, cada cierre retira los dos listeners de `document`, y se coloca encima
// del botón si debajo no cabe. Los listeners los guarda un `AbortController`, que
// aquí se respeta como en el navegador (`{ signal }`).

import test from 'node:test';
import assert from 'node:assert/strict';
import { crearMenuFlotanteDelNavegador } from '../../../src/js/ui/servicios.js';

/** Un `document` y un `window` mínimos, con listeners que obedecen a `signal`. */
function instalarDom({ altoVentana = 800, altoMenu = 120, anchoMenu = 190 } = {}) {
    const listeners = [];
    const cuerpo = { hijos: [], appendChild(h) { h.padre = cuerpo; this.hijos.push(h); } };
    const renglones = [];
    const document = {
        body: cuerpo,
        createElement: () => ({
            style: {}, dataset: {}, offsetHeight: altoMenu, offsetWidth: anchoMenu,
            remove() { cuerpo.hijos = cuerpo.hijos.filter(h => h !== this); this.quitado = true; },
            set innerHTML(html) { this._html = html; },
            get innerHTML() { return this._html; },
            querySelectorAll() { return renglones; },
        }),
        addEventListener(tipo, fn, { signal } = {}) {
            const reg = { tipo, fn, activo: true };
            listeners.push(reg);
            signal?.addEventListener('abort', () => { reg.activo = false; });
        },
    };
    const temporizadores = [];
    const previos = { document: globalThis.document, window: globalThis.window, setTimeout: globalThis.setTimeout };
    globalThis.document = document;
    globalThis.window = { innerHeight: altoVentana };
    globalThis.setTimeout = fn => { temporizadores.push(fn); return temporizadores.length; };
    return {
        cuerpo, renglones, listeners,
        activos: tipo => listeners.filter(l => l.activo && l.tipo === tipo).length,
        despachar: (tipo, ev = {}) => listeners.filter(l => l.activo && l.tipo === tipo).forEach(l => l.fn(ev)),
        pasarUnTick: () => { while (temporizadores.length) temporizadores.shift()(); },
        restaurar: () => { Object.assign(globalThis, previos); },
    };
}

/** Un renglón del menú: las acciones se enganchan con `onclick`. */
const renglon = accion => ({ dataset: { accion }, style: {} });

async function conDom(opciones, prueba) {
    const dom = instalarDom(opciones);
    try { await prueba(dom); } finally { dom.restaurar(); }
}

const abrir = (menu, extra = {}) => menu.abrir({ id: 'menu-x', html: '<div data-accion="a"></div>', ancla: { right: 300, top: 50, bottom: 80 }, alElegir: () => {}, ...extra });

test('abre el menú en <body> con su identificador y su contenido', () => conDom({}, dom => {
    const menu = crearMenuFlotanteDelNavegador();
    abrir(menu);
    assert.equal(dom.cuerpo.hijos.length, 1);
    assert.equal(dom.cuerpo.hijos[0].id, 'menu-x');
    assert.match(dom.cuerpo.hijos[0].innerHTML, /data-accion="a"/);
}));

test('solo hay uno abierto: abrir otro retira el anterior', () => conDom({}, dom => {
    const menu = crearMenuFlotanteDelNavegador();
    abrir(menu, { id: 'uno' });
    abrir(menu, { id: 'dos' });
    assert.deepEqual(dom.cuerpo.hijos.map(h => h.id), ['dos']);
}));

test('se coloca debajo del botón, alineado a su borde derecho, si cabe', () => conDom({ altoVentana: 800, altoMenu: 120, anchoMenu: 190 }, dom => {
    abrir(crearMenuFlotanteDelNavegador());
    const { style } = dom.cuerpo.hijos[0];
    assert.equal(style.left, '110px');   // 300 - 190
    assert.equal(style.top, '84px');     // 80 + 4
}));

test('si debajo no cabe, se coloca encima del botón; nunca fuera de la pantalla', () => conDom({ altoVentana: 150, altoMenu: 120, anchoMenu: 190 }, dom => {
    abrir(crearMenuFlotanteDelNavegador(), { ancla: { right: 100, top: 130, bottom: 160 } });
    const { style } = dom.cuerpo.hijos[0];
    assert.equal(style.top, '8px');      // max(8, 130 - 120 - 4)
    assert.equal(style.left, '8px');     // max(8, 100 - 190)
}));

test('los listeners del documento se enganchan un tick después, para que el clic que abre no lo cierre', () => conDom({}, dom => {
    const menu = crearMenuFlotanteDelNavegador();
    abrir(menu);
    assert.equal(dom.activos('click') + dom.activos('keydown'), 0);
    dom.despachar('click');                                    // el clic que abre sigue burbujeando
    assert.equal(dom.cuerpo.hijos.length, 1);
    dom.pasarUnTick();
    assert.equal(dom.activos('click'), 1);
    assert.equal(dom.activos('keydown'), 1);
}));

test('el siguiente clic en cualquier sitio lo cierra y retira los dos listeners', () => conDom({}, dom => {
    abrir(crearMenuFlotanteDelNavegador());
    dom.pasarUnTick();
    dom.despachar('click');
    assert.equal(dom.cuerpo.hijos.length, 0);
    assert.equal(dom.activos('click') + dom.activos('keydown'), 0);
}));

test('Escape lo cierra; cualquier otra tecla no', () => conDom({}, dom => {
    abrir(crearMenuFlotanteDelNavegador());
    dom.pasarUnTick();
    dom.despachar('keydown', { key: 'Enter' });
    assert.equal(dom.cuerpo.hijos.length, 1);
    dom.despachar('keydown', { key: 'Escape' });
    assert.equal(dom.cuerpo.hijos.length, 0);
    assert.equal(dom.activos('click') + dom.activos('keydown'), 0);
}));

test('cerrar a mano retira el menú y evita que un tick pendiente enganche listeners', () => conDom({}, dom => {
    const menu = crearMenuFlotanteDelNavegador();
    abrir(menu);
    menu.cerrar();
    assert.equal(dom.cuerpo.hijos.length, 0);
    dom.pasarUnTick();                                          // el tick diferido llega tarde: no engancha nada
    assert.equal(dom.activos('click') + dom.activos('keydown'), 0);
    menu.cerrar();                                              // cerrar de nuevo no falla
}));

test('abrir y cerrar muchas veces no acumula listeners', () => conDom({}, dom => {
    const menu = crearMenuFlotanteDelNavegador();
    for (let i = 0; i < 5; i++) { abrir(menu); dom.pasarUnTick(); }
    assert.equal(dom.activos('click'), 1);
    assert.equal(dom.activos('keydown'), 1);
    menu.cerrar();
    assert.equal(dom.activos('click') + dom.activos('keydown'), 0);
}));

test('elegir un renglón cierra el menú primero y luego avisa con su acción', () => conDom({}, dom => {
    const menu = crearMenuFlotanteDelNavegador();
    const abonar = renglon('abonar');
    dom.renglones.push(abonar, renglon('eliminar'));
    const elegidas = [];
    let abiertoAlElegir = null;
    abrir(menu, { alElegir: accion => { elegidas.push(accion); abiertoAlElegir = dom.cuerpo.hijos.length; } });
    abonar.onclick();
    assert.deepEqual(elegidas, ['abonar']);
    assert.equal(abiertoAlElegir, 0);           // ya estaba cerrado cuando se ejecutó la acción
}));

test('el renglón se resalta al pasar el ratón y se apaga al salir', () => conDom({}, dom => {
    const r = renglon('a');
    dom.renglones.push(r);
    abrir(crearMenuFlotanteDelNavegador());
    r.onmouseenter();
    assert.equal(r.style.background, 'rgba(255,255,255,0.06)');
    r.onmouseleave();
    assert.equal(r.style.background, 'none');
}));
