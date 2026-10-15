// Los documentos históricos viven en `docs/historico/`. Esta prueba falla si alguien los cita con una ruta que ya no
// existe: ni los nombres sueltos de la raíz (se movieron), ni un `docs/historico/<archivo>` inexistente.

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..');
const HIST = join(RAIZ, 'docs', 'historico');
const movidos = readdirSync(HIST).filter(n => n !== 'README.md');

const IGNORADOS = ['node_modules', 'target', 'gen', '.claude'];
const fuentes = [];
const recorre = (dir, prof = 0) => {
    for (const e of readdirSync(dir, { withFileTypes: true })) {
        if (IGNORADOS.includes(e.name) || e.name.startsWith('.git') && e.name !== '.gitignore') continue;
        const ruta = join(dir, e.name);
        if (e.isDirectory()) { if (prof < 6) recorre(ruta, prof + 1); }
        else if (/\.(md|ts|rs|mjs|json|yml|toml)$/.test(e.name) || e.name === '.gitignore') fuentes.push(ruta);
    }
};
recorre(RAIZ);
// Los .js de src/js son generados (y se ignoran); los de pruebas sí cuentan.
fuentes.push(...readdirSync(join(RAIZ, 'pruebas', 'js'), { recursive: true })
    .filter(n => n.endsWith('.js')).map(n => join(RAIZ, 'pruebas', 'js', n)));

test('ninguna referencia a un documento histórico apunta a la raíz ni a un archivo inexistente', () => {
    const fallos = [];
    for (const f of fuentes) {
        if (f.startsWith(HIST) || f === join(RAIZ, 'historial_versiones.md') && false) continue;
        const texto = readFileSync(f, 'utf8');
        for (const m of texto.matchAll(/docs\/historico\/([A-Za-z0-9_.]+)/g)) {
            const nombre = m[1].replace(/\.$/, '');
            if (nombre !== 'README.md' && !movidos.includes(nombre)) fallos.push(`${f}: ${m[0]}`);
        }
        for (const n of movidos) {
            const re = new RegExp(`(^|[^/A-Za-z0-9_])${n.replace('.', '\\.')}`);
            if (re.test(texto)) fallos.push(`${f}: cita ${n} sin la ruta docs/historico/`);
        }
    }
    assert.deepEqual(fallos, []);
});
