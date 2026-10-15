// La delegación de eventos (`ui/delegacion.ts`): lo que sustituye a los `onclick`/`onsubmit`/
// `onchange`/`oninput` en línea. Se prueba con elementos de juguete que solo tienen lo que el
// oyente lee (`getAttribute`, `parentElement`, `value`), y con el HTML que escriben los ayudantes
// de `nucleo/html.ts` de punta a punta: lo que una plantilla escribe es lo que la acción recibe.

import test from 'node:test';
import assert from 'node:assert/strict';
import { accionesGenerales, delegar } from '../../../src/js/ui/delegacion.js';
import { accion, alCambiar, alEnviar, alEscribir, manejador } from '../../../src/js/nucleo/html.js';
import { accionesDe } from '../ayudas/manejadores_html.js';

/** Un elemento con sus atributos (`{ 'data-accion': 'f' }`), su padre y, si lo tiene, su valor. */
const el = (atributos = {}, padre = null, extra = {}) => ({
    getAttribute: nombre => (nombre in atributos ? atributos[nombre] : null),
    parentElement: padre,
    ...extra,
});

/** Una raíz que guarda los oyentes y un `disparar` que simula el evento que llega a `document`. */
function raiz() {
    const oyentes = {};
    return {
        addEventListener: (tipo, fn) => { oyentes[tipo] = fn; },
        oyentes,
        disparar(tipo, objetivo, extra = {}) {
            const evento = { target: objetivo, cancelBubble: false, prevenido: false, preventDefault() { this.prevenido = true; }, ...extra };
            oyentes[tipo](evento);
            return evento;
        },
    };
}

/** Convierte lo que escribe un ayudante (`data-accion="f" data-args="[1]"`) en los atributos de un elemento. */
const atributosDe = (cadena, extra = {}) => ({ ...leer(cadena), ...extra });
function leer(cadena) {
    const decod = v => v.replaceAll('&quot;', '"').replaceAll('&#39;', "'").replaceAll('&lt;', '<').replaceAll('&gt;', '>').replaceAll('&amp;', '&');
    return Object.fromEntries([...cadena.matchAll(/([\w-]+)="([^"]*)"/g)].map(m => [m[1], decod(m[2])]));
}

const sinConsola = () => { const errores = []; return { error: (...a) => errores.push(a), errores }; };

test('engancha un oyente por cada tipo de evento: clic, envío, cambio y entrada', () => {
    const r = raiz();
    delegar(r, {});
    assert.deepEqual(Object.keys(r.oyentes).sort(), ['change', 'click', 'input', 'submit']);
});

test('un clic llama a la acción con los argumentos de data-args', () => {
    const r = raiz(); const llamadas = [];
    delegar(r, { eliminar: (...a) => llamadas.push(a) });
    r.disparar('click', el(atributosDe(accion('eliminar', 7, 'x'))));
    assert.deepEqual(llamadas, [[7, 'x']]);
});

test('el clic en un hijo del botón (el icono, el texto) llama a la acción del botón', () => {
    const r = raiz(); const llamadas = [];
    delegar(r, { eliminar: (...a) => llamadas.push(a) });
    const boton = el(atributosDe(accion('eliminar', 3)));
    r.disparar('click', el({}, el({}, boton)));
    assert.deepEqual(llamadas, [[3]]);
});

test('un clic fuera de todo elemento con acción no hace nada', () => {
    const r = raiz(); const consola = sinConsola();
    delegar(r, {}, consola);
    r.disparar('click', el({}, el({})));
    r.disparar('click', null);
    assert.deepEqual(consola.errores, []);
});

test('cada evento lee su propio atributo: un clic no dispara la acción de un cambio', () => {
    const r = raiz(); const llamadas = [];
    delegar(r, { alCambiar: () => llamadas.push('cambio'), alPulsar: () => llamadas.push('clic') });
    const campo = el(atributosDe(alCambiar('alCambiar')));
    r.disparar('click', campo);
    assert.deepEqual(llamadas, []);
    r.disparar('change', campo);
    assert.deepEqual(llamadas, ['cambio']);
});

test('un envío llama a preventDefault y pasa el evento como primer argumento', () => {
    const r = raiz(); const llamadas = [];
    delegar(r, { guardar: (...a) => llamadas.push(a) });
    const evento = r.disparar('submit', el(atributosDe(alEnviar('guardar', 5, 100))));
    assert.equal(evento.prevenido, true);
    assert.deepEqual(llamadas, [[evento, 5, 100]]);
});

test('data-pasa: «valor» pasa el valor del campo; «elemento», el elemento; «evento», el evento', () => {
    const r = raiz(); const llamadas = [];
    delegar(r, { f: (...a) => llamadas.push(a) });
    const campo = el(atributosDe(manejador('change', 'f', ['a'], 'valor')), null, { value: 'DOP' });
    r.disparar('change', campo);
    const boton = el(atributosDe(manejador('click', 'f', [], 'elemento')));
    r.disparar('click', boton);
    const menu = el(atributosDe(manejador('click', 'f', [{ id: 1 }], 'evento')));
    const evento = r.disparar('click', menu);
    assert.deepEqual(llamadas, [['DOP', 'a'], [boton], [evento, { id: 1 }]]);
});

test('un envío sin acción no cancela nada: solo se cancela donde hay una acción', () => {
    const r = raiz();
    delegar(r, {});
    assert.equal(r.disparar('submit', el({})).prevenido, false);
});

test('el evento burbujea de dentro a fuera, como los atributos en línea, hasta que una acción lo detiene', () => {
    const r = raiz(); const orden = [];
    delegar(r, { dentro: () => orden.push('dentro'), fuera: () => orden.push('fuera') });
    const exterior = el(atributosDe(accion('fuera')));
    const interior = el(atributosDe(accion('dentro')), exterior);
    r.disparar('click', interior);
    assert.deepEqual(orden, ['dentro', 'fuera']);

    orden.length = 0;
    delegar(r, { dentro: evento => { orden.push('dentro'); evento.cancelBubble = true; }, fuera: () => orden.push('fuera') });
    const evento = { cancelBubble: false };
    // `dentro` recibe el evento solo si se pide: con data-pasa="evento".
    const conEvento = el(atributosDe(manejador('click', 'dentro', [], 'evento')), exterior);
    r.disparar('click', conEvento, evento);
    assert.deepEqual(orden, ['dentro'], 'stopPropagation en la acción corta la subida');
});

test('una acción que no existe falla en voz alta (console.error) y no rompe las demás', () => {
    const r = raiz(); const consola = sinConsola(); const llamadas = [];
    delegar(r, { buena: () => llamadas.push('buena') }, consola);
    const buena = el(atributosDe(accion('buena')));
    r.disparar('click', el(atributosDe(accion('noExiste')), buena));
    assert.equal(consola.errores.length, 1);
    assert.match(String(consola.errores[0][1]), /noExiste/);
    assert.deepEqual(llamadas, ['buena'], 'la de más arriba sigue su curso');
});

test('el nombre de una acción no puede ser algo heredado del prototipo (constructor, toString…)', () => {
    const r = raiz(); const consola = sinConsola();
    delegar(r, {}, consola);
    r.disparar('click', el({ 'data-accion': 'constructor' }));
    r.disparar('click', el({ 'data-accion': '__proto__' }));
    assert.equal(consola.errores.length, 2);
});

test('data-args que no es un arreglo JSON se registra como error y la acción no se llama', () => {
    const r = raiz(); const consola = sinConsola(); const llamadas = [];
    delegar(r, { f: () => llamadas.push(1) }, consola);
    r.disparar('click', el({ 'data-accion': 'f', 'data-args': '{"a":1}' }));
    r.disparar('click', el({ 'data-accion': 'f', 'data-args': 'no es json' }));
    assert.deepEqual(llamadas, []);
    assert.equal(consola.errores.length, 2);
});

test('de punta a punta: un texto hostil llega a la acción íntegro, como dato, sea cual sea', () => {
    const hostil = `x'); alert(1); //" \\ <img src=x onerror="alert(2)"> &amp; \n`;
    const r = raiz(); const llamadas = [];
    delegar(r, { f: (...a) => llamadas.push(a) });
    const objeto = { nombre: hostil, notas: [hostil] };
    r.disparar('click', el(atributosDe(accion('f', hostil, 7, objeto, null))));
    assert.deepEqual(llamadas, [[hostil, 7, objeto, null]]);
});

test('el HTML que escriben los ayudantes se lee igual con el analizador de las pruebas (mismo formato)', () => {
    const html = `<button ${accion('a', 1)}></button><form ${alEnviar('b')}></form><select ${alCambiar('c', 'x')}></select><input ${alEscribir('d')}>`;
    assert.deepEqual(accionesDe(html).map(a => [a.evento, a.nombre, a.args, a.pasa]), [
        ['click', 'a', [1], null], ['submit', 'b', [], 'evento'], ['change', 'c', ['x'], null], ['input', 'd', [], null],
    ]);
});

// --- acciones generales ---

function dom() {
    const mapa = {};
    const crear = id => (mapa[id] ??= { id, style: {}, className: '', quitado: false, remove() { this.quitado = true; } });
    return { elemento: crear, buscar: id => mapa[id] ?? null, mapa };
}

test('navegar llama a la función de navegación con la ruta', () => {
    const rutas = [];
    const g = accionesGenerales({ dom: dom(), navegar: ruta => rutas.push(ruta) });
    g.navegar('ajustes');
    assert.deepEqual(rutas, ['ajustes']);
});

test('quitar retira del documento el elemento con ese id', () => {
    const d = dom();
    accionesGenerales({ dom: d, navegar() {} }).quitar('modal-x-1');
    assert.equal(d.mapa['modal-x-1'].quitado, true);
});

test('quitar un elemento que no existe falla (como lo hacía elemento(id).remove() en el atributo)', () => {
    const g = accionesGenerales({ dom: { elemento: () => { throw new Error('no existe'); }, buscar: () => null }, navegar() {} });
    assert.throws(() => g.quitar('modal-x-1'), /no existe/);
});

test('activarPestana muestra un panel, oculta los demás y marca el botón pulsado como activo', () => {
    const d = dom();
    const g = accionesGenerales({ dom: d, navegar() {} });
    const paneles = ['p-a', 'p-b', 'p-c']; const botones = ['b-a', 'b-b', 'b-c'];
    g.activarPestana(d.elemento('b-b'), 'p-b', paneles, botones);
    assert.deepEqual(paneles.map(p => d.mapa[p].style.display), ['none', 'block', 'none']);
    assert.deepEqual(botones.map(b => d.mapa[b].className), ['btn btn-secondary', 'btn', 'btn btn-secondary']);
    g.activarPestana(d.elemento('b-a'), 'p-a', paneles, botones);
    assert.deepEqual(paneles.map(p => d.mapa[p].style.display), ['block', 'none', 'none']);
    assert.deepEqual(botones.map(b => d.mapa[b].className), ['btn', 'btn btn-secondary', 'btn btn-secondary']);
});

test('de punta a punta con las acciones generales: un botón de menú y una pestaña', () => {
    const d = dom(); const rutas = [];
    const r = raiz();
    delegar(r, accionesGenerales({ dom: d, navegar: ruta => rutas.push(ruta) }));
    r.disparar('click', el(atributosDe(accion('navegar', 'gastos'))));
    assert.deepEqual(rutas, ['gastos']);
    const boton = d.elemento('b-2');
    r.disparar('click', el(atributosDe(manejador('click', 'activarPestana', ['p-2', ['p-1', 'p-2'], ['b-1', 'b-2']], 'elemento')), null, {}), {});
    // El elemento que llega es el de juguete (sin id): ningún botón coincide y todos quedan secundarios.
    assert.equal(d.mapa['p-2'].style.display, 'block');
    assert.equal(d.mapa['p-1'].style.display, 'none');
    assert.equal(boton.className, 'btn btn-secondary');
});
