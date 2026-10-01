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

test('index.html carga los scripts clásicos que hacen falta y la composición como módulo, y nada más', () => {
    // `api.js` (AppAPI) y `ui/dom.js` (`elemento`/`buscar`, que algunos manejadores en línea
    // llaman) son scripts clásicos con globales; todo lo demás entra por `composicion.js`,
    // que es un módulo y se ejecuta después de ellos aunque vaya al final. Desde la 1.65.0
    // no hay `ui.js` ni `app.js`: un `<script>` que reapareciera cargaría código que ya no existe.
    const html = readFileSync(join(RAIZ, 'src', 'index.html'), 'utf8');
    const scripts = [...html.matchAll(/<script\b([^>]*)>/g)].map(m => m[1].trim());
    assert.deepEqual(scripts, ['src="js/api.js"', 'src="js/ui/dom.js"', 'type="module" src="js/composicion.js"']);
});
