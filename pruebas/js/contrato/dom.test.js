// Las reglas que conservan lo ganado con el ayudante de DOM.
//
// `ui.ts` tuvo `@ts-nocheck` mientras se migraba a TypeScript: 472 errores,
// casi todos por acceder al DOM sin aceptar el `null`. Al quitarlo aparecieron
// además dos defectos reales. Estas pruebas impiden volver atrás sin darse cuenta.

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..');
const JS = join(RAIZ, 'src', 'js');

function fuentesTs(dir) {
    return readdirSync(dir).flatMap(n => {
        const ruta = join(dir, n);
        if (statSync(ruta).isDirectory()) return fuentesTs(ruta);
        return n.endsWith('.ts') && !n.endsWith('.d.ts') ? [ruta] : [];
    });
}

test('ningún fuente del frontend desactiva la comprobación de tipos', () => {
    const con = fuentesTs(JS).filter(f => /@ts-(nocheck|ignore|expect-error)/.test(readFileSync(f, 'utf8')));
    assert.deepEqual(con.map(f => f.replace(RAIZ, '')), [], 'quita la supresión y arregla el tipo');
});

test('la interfaz no vuelve a llamar a document.getElementById directamente', () => {
    // Solo `ui/dom.ts` lo hace: devuelve el elemento tipado o falla diciendo cuál falta.
    const directos = fuentesTs(JS)
        .filter(f => !f.endsWith(join('ui', 'dom.ts')))
        .filter(f => readFileSync(f, 'utf8').includes('document.getElementById('));
    assert.deepEqual(directos.map(f => f.replace(RAIZ, '')), [], 'usa elemento() o buscar()');
});

test('index.html carga el ayudante de DOM antes que la interfaz', () => {
    const html = readFileSync(join(RAIZ, 'src', 'index.html'), 'utf8');
    const dom = html.indexOf('js/ui/dom.js');
    const ui = html.indexOf('js/ui.js');
    assert.ok(dom !== -1 && ui !== -1 && dom < ui, 'dom.js debe ir antes de ui.js');
});
