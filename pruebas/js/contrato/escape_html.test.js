// Ninguna interpolación de texto libre llega al HTML sin escapar.
//
// Las vistas construyen su HTML con plantillas y lo asignan a `innerHTML`. Un nombre, una
// descripción, un concepto o un mensaje de Rust con `<` o `"` se interpretaría como marcado, y
// un `<img src=x onerror=…>` ejecutaría código en el WebView (cuyo puente con Rust admite todos
// los comandos). En la 1.67.0 se corrigió el panel de casos de corrección y se anotó que el
// problema era general: unos 560 puntos de interpolación en las once vistas. En la 1.68.0 se
// corrigieron todos, y esta prueba impide que vuelva.
//
// El análisis lo hace el compilador de TypeScript (`ayudas/analizar_plantillas.js`): para cada
// interpolación de una plantilla HTML mira su **tipo** (texto libre, no un número ni un literal)
// y **dónde cae** (el texto de un elemento, un atributo, una cadena JavaScript dentro de un
// manejador, un JSON metido en un atributo). Por eso encuentra también las variables
// intermedias (`const origen = c.nombre …; ${origen}`), que una búsqueda por nombre de campo no
// vería.

import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { RAIZ } from '../ayudas/fuentes_interfaz.js';
import { analizarPlantillas } from '../ayudas/analizar_plantillas.js';

const resumen = hallazgos => hallazgos.map(h => `${h.archivo}:${h.linea} ${h.texto} (${h.motivo})`);

test('ninguna interpolación de texto libre llega al HTML sin escapar', () => {
    const { hallazgos, interpolaciones } = analizarPlantillas(RAIZ);
    assert.ok(interpolaciones.length > 500, `el análisis no encuentra las plantillas (${interpolaciones.length})`);
    assert.deepEqual(resumen(hallazgos), [], 'usa escaparHtml(...) en el texto o argumentoJs(...) en un manejador');
});

// --- el analizador, con código de ejemplo: si dejara de ver un caso, la prueba de arriba no valdría nada ---

function analizarEjemplo(cuerpo) {
    const dir = mkdtempSync(join(tmpdir(), 'escape-html-'));
    try {
        mkdirSync(join(dir, 'src', 'js', 'vistas'), { recursive: true });
        writeFileSync(join(dir, 'tsconfig.build.json'), JSON.stringify({ compilerOptions: { target: 'es2022', module: 'es2022', moduleResolution: 'bundler', lib: ['es2022', 'dom'], strict: true, skipLibCheck: true }, include: ['src/js/vistas/*.ts'] }));
        writeFileSync(join(dir, 'src', 'js', 'vistas', 'ejemplo.ts'), `
declare function escaparHtml(t: unknown): string;
declare function argumentoJs(v: unknown): string;
declare const formato: { importe(v: number): string };
interface Cuenta { id: number; nombre: string; entidad: string | null; saldo: number; estado: 'a' | 'b' }
declare const c: Cuenta;
declare const cuentas: Cuenta[];
declare const sinTipo: any;
export const html = (() => { ${cuerpo} })();
`);
        return analizarPlantillas(dir).hallazgos.map(h => h.motivo.split(':')[0]);
    } finally { rmSync(dir, { recursive: true, force: true }); }
}

test('el analizador detecta un texto libre sin escapar, en un elemento y en un atributo', () => {
    assert.deepEqual(analizarEjemplo('return `<td>${c.nombre}</td>`;'), ['texto libre sin escapar']);
    assert.deepEqual(analizarEjemplo('return `<input value="${c.nombre}" title="${c.entidad}">`;'), ['texto libre sin escapar', 'texto libre sin escapar']);
    assert.deepEqual(analizarEjemplo('return `<td>${c.entidad || "-"}</td>`;'), ['texto libre sin escapar']);
});

test('el analizador detecta variables intermedias y plantillas anidadas sin etiquetas', () => {
    assert.deepEqual(analizarEjemplo('const origen = c.nombre; return `<td>${origen}</td>`;'), ['texto libre sin escapar']);
    assert.deepEqual(analizarEjemplo('return `<td>${c.saldo > 0 ? `${c.nombre}` : ""}</td>`;'), ['texto libre sin escapar']);
    assert.deepEqual(analizarEjemplo('return `<ul>${cuentas.map(x => `<li>${x.nombre}</li>`).join("")}</ul>`;'), ['texto libre sin escapar']);
});

test('el analizador detecta una cadena JS en un manejador, un JSON en un atributo y un escape a mano', () => {
    assert.deepEqual(analizarEjemplo(`return \`<button onclick="f('\${c.nombre}')">x</button>\`;`).length, 1);
    assert.deepEqual(analizarEjemplo('return `<button onclick="f(${JSON.stringify(c)})">x</button>`;').length, 1);
    assert.ok(analizarEjemplo(`return \`<button onclick='f(\${JSON.stringify(c).replace(/'/g, "&#39;")})'>x</button>\`;`).length >= 1);
});

test('el analizador trata como texto lo que no tiene tipo (el resultado de un JSON.parse): puede serlo', () => {
    assert.deepEqual(analizarEjemplo('return `<td>${sinTipo.nombre}</td>`;'), ['texto libre sin escapar']);
    assert.deepEqual(analizarEjemplo('return `<td>${escaparHtml(sinTipo.nombre)}</td>`;'), []);
    assert.deepEqual(analizarEjemplo('const o: Cuenta = JSON.parse("{}"); return `<td>${o.id} ${escaparHtml(o.nombre)}</td>`;'), [], 'con su tipo, los números no cuentan');
});

test('el analizador no se queja de lo que es seguro: escapado, números, literales y fragmentos', () => {
    assert.deepEqual(analizarEjemplo('return `<td>${escaparHtml(c.nombre)}</td>`;'), []);
    assert.deepEqual(analizarEjemplo('return `<button onclick="f(${argumentoJs(c)}, ${c.id})">x</button>`;'), []);
    assert.deepEqual(analizarEjemplo('return `<td>${c.id} ${c.saldo} ${formato.importe(c.saldo)}</td>`;'), []);
    assert.deepEqual(analizarEjemplo('return `<option ${c.estado === "a" ? "selected" : ""} class="${c.estado}">x</option>`;'), []);
    assert.deepEqual(analizarEjemplo('return `<ul>${cuentas.map(x => `<li>${escaparHtml(x.nombre)}</li>`).join("")}</ul>`;'), []);
    assert.deepEqual(analizarEjemplo('const fila = `<tr><td>${escaparHtml(c.nombre)}</td></tr>`; return `<table>${fila}</table>`;'), []);
    assert.deepEqual(analizarEjemplo('const t = `Hola ${c.nombre}`; return t;'), [], 'una plantilla que no es HTML no se analiza');
});
