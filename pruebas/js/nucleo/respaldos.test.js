import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// El script se carga en la aplicación como script clásico, no como módulo: aquí
// se evalúa su texto para probar exactamente lo que recibe el navegador.
// Se lee el JavaScript **compilado** (`npm test` compila antes).
const fuente = readFileSync(
    join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..', 'src', 'js', 'nucleo', 'respaldos.js'),
    'utf8',
);
const { describirRespaldo, escaparHtml } = new Function(
    `${fuente}; return { describirRespaldo, escaparHtml };`,
)();

test('un respaldo con el nombre de la aplicación se muestra con fecha dd/mm/aaaa y motivo legible', () => {
    assert.equal(
        describirRespaldo('michelitos_2026-09-29T19-03-25_antes-de-instalar.db'),
        '29/09/2026 19:03:25 · antes de instalar',
    );
});

test('un respaldo repetido en el mismo segundo conserva su sufijo', () => {
    assert.equal(
        describirRespaldo('michelitos_2026-09-29T19-03-25_manual-2.db'),
        '29/09/2026 19:03:25 · manual 2',
    );
});

test('un nombre antiguo que no sigue el patrón se muestra tal cual, sin extensión', () => {
    assert.equal(
        describirRespaldo('michelitos-antes-de-corregir-fecha-305-20260923T220651Z.db'),
        'michelitos-antes-de-corregir-fecha-305-20260923T220651Z',
    );
});

test('un nombre con marcas de HTML no puede salirse del atributo ni abrir una etiqueta', () => {
    assert.equal(escaparHtml('a"><img src=x onerror=1>&\''), 'a&quot;&gt;&lt;img src=x onerror=1&gt;&amp;&#39;');
});
