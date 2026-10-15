// `escaparHtml` y `argumentoJs` (`nucleo/html.ts`): lo único que separa un texto del titular de
// un `<img onerror=…>` en una plantilla, o de una cadena de manejador que se cierra antes de
// tiempo. Se prueba el efecto, no la cadena: lo que el navegador entiende tras deshacer las
// entidades del atributo.

import test from 'node:test';
import assert from 'node:assert/strict';
import { escaparHtml, argumentoJs, manejador, accion, alEnviar } from '../../../src/js/nucleo/html.js';
import { accionesDe, decodificarAtributo } from '../ayudas/manejadores_html.js';

const HOSTILES = [
    `<img src=x onerror="alert(1)">`, `'); alert(1); //`, `"\\`, `a & b`, `&amp; &lt;b&gt;`, `</script><script>alert(1)</script>`,
    `línea 1\nlínea 2\r\n\ttab`, `  `, `'''"""\\\\''`, ``, `Tom & Jerry, S.A. — año nuevo «cita»`, `\u0000\u001f`,
];

test('escaparHtml escapa los cinco caracteres que cambian el significado de un texto o de un atributo', () => {
    assert.equal(escaparHtml(`<a href="x" title='y'>&</a>`), '&lt;a href=&quot;x&quot; title=&#39;y&#39;&gt;&amp;&lt;/a&gt;');
});

test('escaparHtml deja intacto el texto corriente: tildes, comas, números y símbolos', () => {
    for (const texto of ['Cuenta de Ahorro', 'Año nuevo, señor Núñez', 'Cafetería «La 5ª»', '12.50 → 13%', 'DOP', '']) assert.equal(escaparHtml(texto), texto);
});

test('escaparHtml acepta lo que no es texto: números, nulos y objetos', () => {
    assert.equal(escaparHtml(42), '42');
    assert.equal(escaparHtml(null), 'null');
    assert.equal(escaparHtml(undefined), 'undefined');
});

test('escaparHtml: el texto escapado, dentro de un elemento, no contiene ninguna etiqueta', () => {
    for (const h of HOSTILES) assert.doesNotMatch(escaparHtml(h), /[<>"']/, JSON.stringify(h));
});

test('escaparHtml es reversible por el navegador: deshacer las entidades devuelve el texto original', () => {
    for (const h of HOSTILES) assert.equal(decodificarAtributo(escaparHtml(h)), h, JSON.stringify(h));
});

/** Ejecuta `onclick="f(${argumentoJs(valor)})"` como el navegador y devuelve lo que recibió `f`. */
function recibido(valor) {
    const atributo = `f(${argumentoJs(valor)})`;
    let args = null;
    new Function('f', decodificarAtributo(atributo))((...a) => { args = a; });
    return args;
}

test('argumentoJs: una cadena llega íntegra al manejador, sean cuales sean sus caracteres', () => {
    for (const h of HOSTILES) assert.deepEqual(recibido(h), [h], JSON.stringify(h));
});

test('argumentoJs: números, nulos, booleanos y objetos llegan con su tipo', () => {
    assert.deepEqual(recibido(42), [42]);
    assert.deepEqual(recibido(null), [null]);
    assert.deepEqual(recibido(true), [true]);
    assert.deepEqual(recibido(undefined), [null], 'lo indefinido no es JSON: llega como nulo, no como código');
    const objeto = { id: 6, nombre: `O'Brien "x" & <b>`, notas: [`'`, `\\`], anidado: { a: null } };
    assert.deepEqual(recibido(objeto), [objeto]);
});

test('argumentoJs: JSON en texto (lo que esperan los manejadores que hacen JSON.parse) llega como cadena exacta', () => {
    const objeto = { nombre: `x'); alert(1); //` };
    const [texto] = recibido(JSON.stringify(objeto));
    assert.equal(typeof texto, 'string');
    assert.deepEqual(JSON.parse(texto), objeto);
});

test('argumentoJs: nada de lo que lleve el valor puede salirse del argumento y ejecutarse', () => {
    let ejecutado = 0;
    const hostil = `'); globalThis.__ejecutado = 1; ('`;
    globalThis.__ejecutado = 0;
    new Function('f', decodificarAtributo(`f(${argumentoJs(hostil)})`))(() => {});
    ejecutado = globalThis.__ejecutado;
    delete globalThis.__ejecutado;
    assert.equal(ejecutado, 0);
});

test('argumentoJs: el atributo resultante no puede contener comillas dobles sin escapar (no cierra el atributo)', () => {
    for (const h of HOSTILES) assert.doesNotMatch(argumentoJs(h), /["<>]/, JSON.stringify(h));
    assert.doesNotMatch(argumentoJs({ a: `"`, b: [`'`] }), /["<>]/);
});

// --- manejadores delegados (`manejador`, `accion`, `alEnviar`…) ---

test('manejador: escribe el evento, el nombre, los argumentos y lo que se pasa, y nada más', () => {
    assert.equal(accion('f'), 'data-accion="f"');
    assert.equal(accion('f', 1, 'a'), 'data-accion="f" data-args="[1,&quot;a&quot;]"');
    assert.equal(alEnviar('g', 7), 'data-accion-enviar="g" data-args="[7]"');
    assert.equal(manejador('change', 'h', [], 'valor'), 'data-accion-cambiar="h" data-pasa="valor"');
    assert.equal(manejador('input', 'i', [2], 'elemento'), 'data-accion-escribir="i" data-args="[2]" data-pasa="elemento"');
});

test('manejador: ningún texto de los argumentos puede salirse del atributo ni crear otro', () => {
    for (const h of HOSTILES) {
        const etiqueta = `<button ${accion('f', h, { n: h })} class="x">`;
        const atributos = accionesDe(etiqueta);
        assert.equal(atributos.length, 1, JSON.stringify(h));
        assert.deepEqual(atributos[0].args, [h, { n: h }], JSON.stringify(h));
        assert.equal(atributos[0].nombre, 'f');
        // Dentro de los valores no queda ninguna comilla ni ángulo sin escapar.
        assert.doesNotMatch(accion('f', h).replace(/="[^"]*"/g, m => m.slice(2, -1).replace(/&(quot|amp|lt|gt|#39);/g, '')), /["'<>]/, JSON.stringify(h));
    }
});

test('manejador: un nombre hostil tampoco rompe el atributo', () => {
    const nombre = `f" onfocus="alert(1)`;
    const [a] = accionesDe(`<button ${accion(nombre)} class="x">`);
    assert.equal(a.nombre, nombre);
    assert.doesNotMatch(accion(nombre), /onfocus="/);
});

test('manejador: lo indefinido llega como nulo, no como código', () => {
    assert.deepEqual(accionesDe(`<b ${accion('f', undefined, null)}>`)[0].args, [null, null]);
});
