// Cobertura declarada: cada método `handle*` de la interfaz tiene que estar
// ejercitado por una prueba de interacción (llamado como `appUI.<método>(`) o
// figurar aquí con la razón por la que no se prueba. Un manejador nuevo sin
// prueba ni declaración rompe esta prueba: es el aviso de que hay un botón sin
// red de seguridad.
//
// Lee los fuentes `.ts` como texto (igual que `manejadores.test.js`), así que
// sigue valiendo cuando `ui.ts` se divida en vistas.

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const AQUI = dirname(fileURLToPath(import.meta.url));
const JS = join(AQUI, '..', '..', '..', 'src', 'js');

/** Manejadores sin prueba de interacción, con su razón. Hoy no hay ninguno. */
const NO_CUBIERTOS = new Map([
    // ['handleEjemplo', 'razón por la que no se puede probar con DOM falso'],
]);

function fuentes(dir) {
    return readdirSync(dir).flatMap(n => {
        const ruta = join(dir, n);
        if (statSync(ruta).isDirectory()) return fuentes(ruta);
        return n.endsWith('.ts') && !n.endsWith('.d.ts') && n !== 'api.ts' ? [ruta] : [];
    });
}

const definidos = [...new Set(
    fuentes(JS).flatMap(f => [...readFileSync(f, 'utf8').matchAll(/^    (?:async )?(handle\w+)\(/gm)].map(m => m[1])),
)].sort();

const pruebas = readdirSync(AQUI)
    .filter(n => n.endsWith('.test.js') && n !== 'cobertura.test.js')
    .map(n => readFileSync(join(AQUI, n), 'utf8'))
    .join('\n');

test('hay decenas de manejadores handle* (la prueba no está vacía)', () => {
    assert.ok(definidos.length >= 45, `se esperaban ~49, hay ${definidos.length}`);
});

test('todo manejador handle* está cubierto por una prueba de interacción o declarado sin cubrir', () => {
    const sinPrueba = definidos.filter(
        m => !new RegExp(`\\bappUI\\.${m}\\(`).test(pruebas) && !NO_CUBIERTOS.has(m),
    );
    assert.deepEqual(sinPrueba, [], 'manejadores sin prueba de interacción ni declaración en NO_CUBIERTOS');
});

test('la lista de no cubiertos no tiene entradas obsoletas', () => {
    const obsoletas = [...NO_CUBIERTOS.keys()].filter(
        m => !definidos.includes(m) || new RegExp(`\\bappUI\\.${m}\\(`).test(pruebas),
    );
    assert.deepEqual(obsoletas, [], 'declarados como no cubiertos pero ya cubiertos, o ya inexistentes');
});
