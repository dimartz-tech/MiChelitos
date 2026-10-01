// Los manejadores en línea (`onclick="appUI.guardar()"`) son el hilo que une el
// HTML que dibuja `ui.ts` con el código que lo atiende. Un manejador que apunta
// a algo que no existe **falla al pulsar**, no al cargar: ninguna otra prueba
// ni el compilador lo ven, porque el nombre vive dentro de una cadena.
//
// Existe para la división de `ui.ts` por pestañas (`division_de_ui.md`): mover
// un método a otro archivo es justo la operación que deja un `onclick` colgado.
// Esta prueba comprueba, antes de fusionar cada movimiento, que cada manejador
// sigue teniendo destino.

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..');
const JS = join(RAIZ, 'src', 'js');

function fuentes(dir) {
    return readdirSync(dir).flatMap(n => {
        const ruta = join(dir, n);
        if (statSync(ruta).isDirectory()) return fuentes(ruta);
        return n.endsWith('.ts') && !n.endsWith('.d.ts') ? [ruta] : [];
    });
}

const TS = fuentes(JS);
const HTML = readFileSync(join(RAIZ, 'src', 'index.html'), 'utf8');
// Todo el HTML que se escribe: las plantillas de las vistas y el index.html.
const MARCADO = [...TS.map(f => readFileSync(f, 'utf8')), HTML].join('\n');

/**
 * Cuerpos de todos los atributos `on...="..."`, sin las interpolaciones `${...}`:
 * esas se evalúan al **dibujar** la vista (`${String(id)}`), no al pulsar, y no
 * son destinos del manejador.
 */
const manejadores = () =>
    [...MARCADO.matchAll(/\bon(?:click|submit|input|change|keyup|keydown|blur|focus|mouseenter|mouseleave)\s*=\s*"([^"]*)"/g)]
        .map(m => m[1].replace(/\$\{[^}]*\}/g, '0'));

/** Métodos definidos en las vistas (clase u objeto de métodos); `api.ts` no cuenta. */
const metodosDeLaInterfaz = () => new Set(
    TS.filter(f => !f.endsWith('api.ts'))
        .flatMap(f => [...readFileSync(f, 'utf8').matchAll(/^    (?:async )?(\w+)\([^)]*\)[^{;]*\{\s*$/gm)].map(m => m[1])),
);

test('hay decenas de manejadores en línea (la prueba no está vacía)', () => {
    assert.ok(manejadores().length > 80, `se esperaban decenas, hay ${manejadores().length}`);
});

test('todo appUI.metodo() de un manejador existe como método de la interfaz', () => {
    const definidos = metodosDeLaInterfaz();
    const llamados = new Set(manejadores().flatMap(c => [...c.matchAll(/\bappUI\.(\w+)/g)].map(m => m[1])));
    assert.ok(llamados.size > 50, `se esperaban decenas de métodos distintos, hay ${llamados.size}`);
    const colgados = [...llamados].filter(n => !definidos.has(n));
    assert.deepEqual(colgados, [], 'manejadores que llaman a un método que no existe');
});

test('navigate() existe como función global en app.ts', () => {
    assert.ok(manejadores().some(c => /\bnavigate\(/.test(c)), 'ya ningún manejador usa navigate');
    const app = readFileSync(join(JS, 'app.ts'), 'utf8');
    assert.match(app, /^function navigate\(/m, 'navigate dejó de ser una función global de app.ts');
});

test('los manejadores solo llaman a destinos globales conocidos: cualquier otro se declara aquí', () => {
    // Un destino nuevo (otro objeto global, una función suelta) tiene que
    // existir en el ámbito global del navegador. Si una vista pasa a ser un
    // módulo, sus nombres dejan de ser globales y hay que exponerlos.
    //
    // `elemento` y `buscar` (ui/dom.ts) están porque algunos manejadores
    // alternan paneles sin pasar por `appUI`: si `dom` dejara de ser un script
    // global habría que colgarlos de `window`. `String` y `Number` son del
    // propio navegador.
    const PERMITIDOS = new Set(['appUI', 'navigate', 'elemento', 'buscar', 'String', 'Number']);
    const destinos = new Set();
    for (const c of manejadores()) {
        for (const m of c.matchAll(/\b([A-Za-z_]\w*)\s*\.\s*\w+\s*\(/g)) destinos.add(m[1]);
        for (const m of c.matchAll(/(?<![.\w])([A-Za-z_]\w*)\s*\(/g)) destinos.add(m[1]);
    }
    const ajenos = [...destinos].filter(d => !PERMITIDOS.has(d));
    assert.deepEqual(ajenos, [], 'manejadores que llaman a un destino que no está declarado como global');
});
