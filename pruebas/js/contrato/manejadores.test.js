// Los manejadores delegados (`data-accion="handleGuardar"`) son el hilo que une el HTML que
// dibujan las vistas con el código que lo atiende. Una acción que no existe **falla al pulsar**,
// no al cargar: ninguna otra prueba ni el compilador lo ven, porque el nombre vive dentro de una
// cadena.
//
// Hasta la 1.106.0 los manejadores eran atributos en línea (`onclick="appUI.guardar()"`) y la
// política de contenido (CSP) tenía que admitir `'unsafe-inline'` en `script-src-attr` por ellos.
// Ahora la interfaz declara **datos** (`data-accion`, `data-accion-enviar`, `data-accion-cambiar`,
// `data-accion-escribir`, `data-args`, `data-pasa`) y un oyente delegado (`ui/delegacion.ts`) los
// atiende. Esta prueba exige las dos mitades: que no vuelva a aparecer ningún `on…=` en el marcado,
// y que cada acción que se escribe exista en el puente real que cuelga de `window.appUI`.

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fuentesDeLaInterfaz, leerHtml } from '../ayudas/fuentes_interfaz.js';
import { cargarInterfaz } from '../ayudas/cargar_interfaz.js';
import { ACCIONES_GENERALES, accionesDe } from '../ayudas/manejadores_html.js';

const TS = fuentesDeLaInterfaz();
const HTML = leerHtml();
const FUENTES = [...TS.map(f => ({ nombre: f, texto: readFileSync(f, 'utf8') })), { nombre: 'index.html', texto: HTML }];

/** Una llamada a los ayudantes de `nucleo/html.ts` desde una plantilla: `${accion('nombre', …)}`. */
const AYUDANTES = /\$\{(accion|alEnviar|alCambiar|alEscribir)\(\s*(['"`])([^'"`]*)\2/g;
const MANEJADOR = /\$\{manejador\(\s*'(click|submit|change|input)'\s*,\s*(['"`])([^'"`]*)\2/g;
const LLAMADAS = /\$\{(?:accion|alEnviar|alCambiar|alEscribir|manejador)\(/g;

/** Los nombres de acción que escriben las plantillas de las vistas y el index.html, con su archivo. */
function nombresEscritos() {
    const nombres = [];
    for (const { nombre: archivo, texto } of FUENTES) {
        for (const m of texto.matchAll(AYUDANTES)) nombres.push({ archivo, nombre: m[3] });
        for (const m of texto.matchAll(MANEJADOR)) nombres.push({ archivo, nombre: m[3] });
        for (const m of texto.matchAll(/\bdata-accion(?:-enviar|-cambiar|-escribir)?="([\w]+)"/g)) nombres.push({ archivo, nombre: m[1] });
    }
    return nombres;
}

test('ningún archivo de la interfaz escribe un manejador en línea (on…="…"): lo exige la CSP', () => {
    // Un atributo `onclick="…"`, `onsubmit='…'`… en una plantilla o en el index.html. Las propiedades
    // asignadas desde TypeScript (`el.onclick = () => …`) no son atributos del marcado: no cuentan.
    const hallazgos = [];
    for (const { nombre, texto } of FUENTES) {
        for (const m of texto.matchAll(/(^|[\s"'`])on[a-z]+\s*=\s*["']/gm)) {
            const linea = texto.slice(0, m.index).split('\n').length;
            hallazgos.push(`${nombre}:${linea}`);
        }
    }
    assert.deepEqual(hallazgos, [], 'un manejador en línea obliga a admitir script-src-attr \'unsafe-inline\'; usa accion(…)/alEnviar(…)/alCambiar(…)/alEscribir(…)');
});

test('hay decenas de manejadores delegados (la prueba no está vacía)', () => {
    assert.ok(nombresEscritos().length > 100, `se esperaban más de cien, hay ${nombresEscritos().length}`);
});

test('toda llamada a un ayudante de manejador lleva el nombre como literal (si no, no se podría comprobar)', () => {
    let llamadas = 0, literales = 0;
    for (const { texto } of FUENTES) {
        llamadas += [...texto.matchAll(LLAMADAS)].length;
        literales += [...texto.matchAll(AYUDANTES)].length + [...texto.matchAll(MANEJADOR)].length;
    }
    assert.ok(llamadas > 90, `se esperaban decenas de llamadas, hay ${llamadas}`);
    assert.equal(literales, llamadas, 'un manejador cuyo nombre no es un literal no puede verificarse contra el puente');
});

test('toda acción escrita existe en el puente real que cuelga de window.appUI, o es una acción general', () => {
    // Lo que **de verdad se compone**: si una vista se queda sin registrar, o su puente no expone la
    // acción que su plantilla escribe, el botón falla al pulsarlo y no al cargar.
    const puente = new Set(Object.keys(cargarInterfaz().appUI));
    const conocidas = new Set([...puente, ...ACCIONES_GENERALES]);
    const escritas = nombresEscritos();
    assert.ok(new Set(escritas.map(e => e.nombre)).size > 50, 'se esperaban decenas de acciones distintas');
    const colgadas = escritas.filter(e => !conocidas.has(e.nombre)).map(e => `${e.archivo}: ${e.nombre}`);
    assert.deepEqual(colgadas, [], 'acciones escritas en el HTML que ninguna vista expone en el puente');
});

test('las acciones generales no pisan ningún método de una vista (la composición lo rechazaría al arrancar)', () => {
    const puente = Object.keys(cargarInterfaz().appUI);
    assert.deepEqual(puente.filter(n => ACCIONES_GENERALES.has(n)), []);
});

test('el index.html: el menú navega por la acción general «navegar» y no lleva script en línea', () => {
    const acciones = accionesDe(HTML);
    assert.equal(acciones.length, 12);
    assert.ok(acciones.every(a => a.evento === 'click' && a.nombre === 'navegar' && a.args.length === 1), 'cada botón del menú navega a una ruta');
    assert.doesNotMatch(HTML, /<script(?![^>]*\bsrc=)/, 'un <script> sin src es código en línea');
});

test('el puente que se compone no expone nada que ninguna plantilla use (sin acciones muertas)', () => {
    const escritas = new Set(nombresEscritos().map(e => e.nombre));
    // Algunas acciones se llaman desde el código de la vista (no desde una plantilla) y están en el puente
    // para las pruebas y la consola: se listan aquí con su razón en vez de ocultarlas.
    const SOLO_DESDE_CODIGO = new Set([
        'toggleMetodoPago',        // lo llama `cambiarMetodoDePago`, la acción que escribe la plantilla
        'handlePagarCuota',        // las tres, desde el menú de acciones de un financiamiento (`abrirMenuPasivo`)
        'handleDeclararSaldo',
        'handleEliminarPrestamo',
    ]);
    const muertas = Object.keys(cargarInterfaz().appUI).filter(n => !escritas.has(n) && !SOLO_DESDE_CODIGO.has(n));
    assert.deepEqual(muertas, [], 'métodos del puente que ninguna plantilla escribe: quítalos o decláralos en SOLO_DESDE_CODIGO');
});
