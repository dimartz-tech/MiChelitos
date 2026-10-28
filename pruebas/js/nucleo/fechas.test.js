// La máscara de fechas (`nucleo/fechas.ts`): función pura, sin DOM. Datos inventados.
//
// Para no probar solo la función aislada se simula lo que hace el navegador alrededor de ella:
// aplicar la edición (insertar, Retroceso, Supr, pegar) y llamar a la máscara con el texto
// resultante y el cursor donde lo dejó el navegador; el `maxlength="10"` del campo corta el tecleo.
// Esa simulación es lo que hace que «el Retroceso no se queda atascado» sea una afirmación real.

import test from 'node:test';
import assert from 'node:assert/strict';
import { enmascararFecha } from '../../../src/js/nucleo/fechas.js';

/** Un campo simulado: texto y cursor (sin selección). */
const campo = (texto = '', cursor = texto.length) => ({ texto, cursor });

/** Teclea `cadena` carácter a carácter; devuelve el campo y cada estado intermedio. */
function teclear(inicio, cadena) {
    let c = inicio;
    const pasos = [];
    for (const ch of cadena) {
        if (c.texto.length >= 10) { pasos.push(c.texto); continue; }   // maxlength="10"
        const texto = c.texto.slice(0, c.cursor) + ch + c.texto.slice(c.cursor);
        c = enmascararFecha(texto, c.cursor + 1, false);
        pasos.push(c.texto);
    }
    return { campo: c, pasos };
}

/** Retroceso `veces` veces. */
function retroceso(inicio, veces = 1) {
    let c = inicio;
    const pasos = [];
    for (let i = 0; i < veces; i++) {
        if (c.cursor === 0) { pasos.push(c.texto); continue; }
        const texto = c.texto.slice(0, c.cursor - 1) + c.texto.slice(c.cursor);
        c = enmascararFecha(texto, c.cursor - 1, true);
        pasos.push(c.texto);
    }
    return { campo: c, pasos };
}

/** Supr (borrar hacia delante). */
function suprimir(inicio) {
    const texto = inicio.texto.slice(0, inicio.cursor) + inicio.texto.slice(inicio.cursor + 1);
    return enmascararFecha(texto, inicio.cursor, true);
}

/** Pegar `cadena` en el cursor (el navegador pone el cursor tras lo pegado). */
function pegar(inicio, cadena) {
    const texto = inicio.texto.slice(0, inicio.cursor) + cadena + inicio.texto.slice(inicio.cursor);
    return enmascararFecha(texto, inicio.cursor + cadena.length, false);
}

test('tecleando solo dígitos la barra aparece sola tras el día y tras el mes', () => {
    const { pasos } = teclear(campo(), '28102026');
    assert.deepEqual(pasos, ['2', '28/', '28/1', '28/10/', '28/10/2', '28/10/20', '28/10/202', '28/10/2026']);
});

test('el cursor siempre queda al final mientras se teclea hacia delante, también tras la barra automática', () => {
    let c = campo();
    for (const ch of '28102026') {
        c = enmascararFecha(c.texto.slice(0, c.cursor) + ch + c.texto.slice(c.cursor), c.cursor + 1, false);
        assert.equal(c.cursor, c.texto.length, `tras «${ch}» quedó «${c.texto}» con el cursor en ${c.cursor}`);
    }
});

test('si el usuario teclea la barra él mismo no se duplica', () => {
    assert.deepEqual(teclear(campo(), '28/10/2026').pasos,
        ['2', '28/', '28/', '28/1', '28/10/', '28/10/', '28/10/2', '28/10/20', '28/10/202', '28/10/2026']);
    assert.equal(enmascararFecha('28//', 4).texto, '28/');
    assert.equal(enmascararFecha('28/10//', 7).texto, '28/10/');
});

test('una barra, guion o punto escritos en el lugar del separador se convierten en «/»', () => {
    assert.equal(teclear(campo(), '28-10-2026').campo.texto, '28/10/2026');
    assert.equal(teclear(campo(), '28.10.2026').campo.texto, '28/10/2026');
    assert.equal(teclear(campo(), '28-10/2026').campo.texto, '28/10/2026');
});

test('los separadores que no ocupan el lugar de uno se descartan: al inicio, repetidos o dentro del año', () => {
    assert.equal(enmascararFecha('/', 1).texto, '');
    assert.equal(enmascararFecha('-', 1).texto, '');
    assert.equal(enmascararFecha('--', 2).texto, '');
    assert.equal(enmascararFecha('/28', 3).texto, '28/');
    assert.equal(enmascararFecha('28/10/20-26', 11).texto, '28/10/2026');
    assert.equal(enmascararFecha('28/10/2026/', 11).texto, '28/10/2026');
    assert.equal(enmascararFecha('28/10/2026.', 11).texto, '28/10/2026');
});

test('un día o un mes de un dígito cerrado con barra se respeta (no se rellena con cero: lo juzga el backend)', () => {
    assert.equal(teclear(campo(), '5/3/2026').campo.texto, '5/3/2026');
    assert.equal(enmascararFecha('5/', 2).texto, '5/');
});

test('letras, espacios y símbolos se descartan', () => {
    assert.equal(enmascararFecha('abc', 3).texto, '');
    assert.equal(enmascararFecha('ab12cd', 6).texto, '12/');
    assert.equal(enmascararFecha('28 10 2026', 10).texto, '28/10/2026');
    assert.equal(enmascararFecha('  ', 2).texto, '');
    assert.equal(enmascararFecha('1e5', 3).texto, '15/');
    assert.equal(enmascararFecha('２８', 2).texto, '');   // dígitos de ancho completo: no son 0-9
    assert.equal(enmascararFecha('😀28', 4).texto, '28/');
    assert.deepEqual(teclear(campo(), '2a8').pasos, ['2', '2', '28/']);
});

test('el texto nunca pasa de 10 caracteres: lo que sobra del año se descarta', () => {
    assert.equal(enmascararFecha('281020261234', 12).texto, '28/10/2026');
    assert.equal(enmascararFecha('28/10/202612', 12).texto, '28/10/2026');
    assert.equal(teclear(campo('28/10/2026'), '7').campo.texto, '28/10/2026');
});

test('un año de dos dígitos NO se expande: queda tal cual para que lo rechace el backend', () => {
    assert.equal(pegar(campo(), '28/10/26').texto, '28/10/26');
    assert.equal(teclear(campo(), '281026').campo.texto, '28/10/26');
});

test('texto vacío y valores raros del cursor', () => {
    assert.deepEqual(enmascararFecha('', 0), { texto: '', cursor: 0 });
    assert.deepEqual(enmascararFecha('', 5), { texto: '', cursor: 0 });
    assert.deepEqual(enmascararFecha('281', -3), { texto: '28/1', cursor: 0 });
    assert.deepEqual(enmascararFecha('281', Number.NaN), { texto: '28/1', cursor: 4 });
    assert.deepEqual(enmascararFecha('281', 99), { texto: '28/1', cursor: 4 });
    assert.deepEqual(enmascararFecha('281', 1.9), { texto: '28/1', cursor: 1 });
});

// --- Pegado ---------------------------------------------------------------------------------

test('pegar una fecha completa, con cualquier separador, en un campo vacío', () => {
    for (const pegado of ['28102026', '28/10/2026', '28-10-2026', '28.10.2026', ' 28/10/2026 ', '28 10 2026', '28/10/2026\n']) {
        assert.deepEqual(pegar(campo(), pegado), { texto: '28/10/2026', cursor: 10 }, `pegado «${pegado}»`);
    }
});

test('pegar un trozo deja la barra automática y el cursor al final', () => {
    assert.deepEqual(pegar(campo(), '2810'), { texto: '28/10/', cursor: 6 });
    assert.deepEqual(pegar(campo(), '28'), { texto: '28/', cursor: 3 });
    assert.deepEqual(pegar(campo(), '28/10'), { texto: '28/10/', cursor: 6 });
});

test('pegar sobre una fecha ya escrita (selección reemplazada) la sustituye', () => {
    // El navegador deja en el campo solo lo pegado cuando se había seleccionado todo.
    assert.equal(pegar(campo(), '01/02/2027').texto, '01/02/2027');
});

// --- Borrado --------------------------------------------------------------------------------

test('Retroceso desde el final recorre la fecha sin atascarse en las barras', () => {
    const { pasos } = retroceso(campo('28/10/2026'), 12);
    assert.deepEqual(pasos, ['28/10/202', '28/10/20', '28/10/2', '28/10/', '28/10', '28/1', '28/', '28', '2', '', '', '']);
});

test('tras borrar la barra automática, escribir el dígito siguiente vuelve a ponerla', () => {
    const tras = retroceso(campo('28/'), 1).campo;
    assert.deepEqual(tras, { texto: '28', cursor: 2 });
    assert.equal(teclear(tras, '1').campo.texto, '28/1');
    const trasMes = retroceso(campo('28/10/'), 1).campo;
    assert.equal(trasMes.texto, '28/10');
    assert.equal(teclear(trasMes, '2').campo.texto, '28/10/2');
});

test('borrar el dígito que sigue a una barra deja la barra y un segundo Retroceso la quita', () => {
    const uno = retroceso(campo('28/1'), 1).campo;
    assert.equal(uno.texto, '28/');
    assert.equal(retroceso(uno, 1).campo.texto, '28');
});

test('Retroceso con el cursor justo detrás de una barra en medio del texto: el cursor se pone antes de la barra y no se atasca', () => {
    // «28/|10/2026» + Retroceso → el navegador deja «2810/2026» con el cursor en 2.
    const a = enmascararFecha('2810/2026', 2, true);
    assert.deepEqual(a, { texto: '28/10/2026', cursor: 2 });
    // El siguiente Retroceso borra el «8», no vuelve a tropezar con la barra.
    assert.deepEqual(retroceso(a, 1).campo, { texto: '2/10/2026', cursor: 1 });
});

test('borrar un dígito en medio no desplaza el resto de la fecha', () => {
    // «28/1|0/2026» + Retroceso → «28/0/2026» con el cursor en 3.
    assert.deepEqual(retroceso(campo('28/10/2026', 4), 1).campo, { texto: '28/0/2026', cursor: 3 });
    assert.deepEqual(enmascararFecha('28/0/2026', 3, true), { texto: '28/0/2026', cursor: 3 });
});

test('Supr (borrado hacia delante) sobre la barra automática no la atasca', () => {
    // «28|/10/2026» + Supr → «2810/2026» con el cursor en 2: la máscara la devuelve y el cursor sigue en 2.
    assert.deepEqual(suprimir(campo('28/10/2026', 2)), { texto: '28/10/2026', cursor: 2 });
    // «2|8/10/2026» + Supr → «2/10/2026».
    assert.deepEqual(suprimir(campo('28/10/2026', 1)), { texto: '2/10/2026', cursor: 1 });
});

test('vaciar un campo con valor inicial borrando carácter a carácter llega a vacío', () => {
    let c = campo('05/03/2027');
    for (let i = 0; i < 10; i++) c = retroceso(c, 1).campo;
    assert.deepEqual(c, { texto: '', cursor: 0 });
});

// --- Edición en medio -----------------------------------------------------------------------

test('insertar un dígito en medio reparte los dígitos y deja el cursor tras lo insertado', () => {
    // «2|8/10/20» → se teclea «5» → «25|8/10/20» (cursor 2).
    const r = enmascararFecha('258/10/20', 2, false);
    assert.equal(r.texto, '25/81/020');
    assert.equal(r.cursor, 3, 'el cursor salta la barra automática y cae delante del «8» desplazado');
});

test('teclear en medio de una fecha corta no mueve el resto', () => {
    // «28/|» (cursor tras la barra) → «1» → «28/1».
    assert.deepEqual(teclear(campo('28/'), '1').campo, { texto: '28/1', cursor: 4 });
    // «|» al inicio de «10/2026»: se teclea «2» → «210/2026» con el cursor en 1.
    const r = enmascararFecha('210/2026', 1, false);
    assert.equal(r.texto, '21/02/026');
    assert.equal(r.cursor, 1);
});

test('el cursor en medio se queda tras el mismo carácter que tenía a su izquierda', () => {
    assert.deepEqual(enmascararFecha('28/10/2026', 4, false), { texto: '28/10/2026', cursor: 4 });
    assert.deepEqual(enmascararFecha('28/10/2026', 0, false), { texto: '28/10/2026', cursor: 0 });
    // Letra tecleada en medio: se descarta y el cursor vuelve donde estaba.
    assert.deepEqual(enmascararFecha('28x/10/2026', 3, false), { texto: '28/10/2026', cursor: 3 });
    assert.deepEqual(enmascararFecha('2x8/10/2026', 2, false), { texto: '28/10/2026', cursor: 1 });
});

// --- Propiedades ------------------------------------------------------------------------------

test('propiedades sobre entradas pseudoaleatorias: forma, longitud, cursor válido e idempotencia', () => {
    let semilla = 12345;
    const azar = n => { semilla = (semilla * 1103515245 + 12345) & 0x7fffffff; return semilla % n; };
    const alfabeto = '0123456789012345678/-. aZ';
    const FORMA = /^(\d{1,2}(\/(\d{1,2}(\/\d{0,4})?)?)?)?$/;
    for (let i = 0; i < 3000; i++) {
        const largo = azar(16);
        let texto = '';
        for (let j = 0; j < largo; j++) texto += alfabeto[azar(alfabeto.length)];
        const cursor = azar(largo + 1);
        for (const borrando of [false, true]) {
            const r = enmascararFecha(texto, cursor, borrando);
            const ctx = `«${texto}» cursor ${cursor} borrando ${borrando} → «${r.texto}» ${r.cursor}`;
            assert.match(r.texto, FORMA, ctx);
            assert.ok(r.texto.length <= 10, ctx);
            assert.ok(r.cursor >= 0 && r.cursor <= r.texto.length, ctx);
            // Reaplicarla borrando no cambia nada (un texto ya formateado es estable)…
            assert.equal(enmascararFecha(r.texto, r.texto.length, true).texto, r.texto, ctx);
            // …y los dígitos que quedan son un prefijo de los que se escribieron.
            const quedan = r.texto.replace(/\D/g, '');
            assert.ok(texto.replace(/\D/g, '').startsWith(quedan), ctx);
        }
    }
});
