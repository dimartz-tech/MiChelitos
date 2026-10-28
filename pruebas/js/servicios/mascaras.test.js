// El oyente de máscaras (`ui/mascaras.ts`) con un campo de juguete que se comporta como el
// de un navegador (valor, cursor, `setSelectionRange`), y con un campo sacado de la plantilla
// real de la vista de Gastos (las demás plantillas las cubre `contrato/fechas.test.js`). Datos inventados.

import test from 'node:test';
import assert from 'node:assert/strict';
import { enmascarar } from '../../../src/js/ui/mascaras.js';
import { ATRIBUTOS_DE_FECHA } from '../../../src/js/nucleo/html.js';
import { VistaGastos } from '../../../src/js/vistas/gastos.js';

/** Los atributos de una etiqueta `<input …>` como un objeto. */
function atributosDe(etiqueta) {
    return Object.fromEntries([...etiqueta.matchAll(/([\w-]+)="([^"]*)"/g)].map(m => [m[1], m[2]]));
}

/**
 * Un `<input type="text">` que se comporta como el del navegador: guarda valor y cursor, y
 * `escribir`/`retroceso`/`pegar` aplican la edición y disparan `input` en la raíz con el
 * `inputType` que corresponde.
 */
function crearCampo(raiz, atributos = {}, { valor = '', readOnly = false, disabled = false } = {}) {
    const campo = {
        value: valor, readOnly, disabled, selectionStart: valor.length, escrituras: 0, selecciones: [],
        getAttribute: nombre => atributos[nombre] ?? null,
        setSelectionRange(a, b) { this.selecciones.push([a, b]); this.selectionStart = a; },
    };
    let v = valor;
    Object.defineProperty(campo, 'value', {
        get: () => v,
        set: nuevo => { v = nuevo; campo.escrituras++; campo.selectionStart = nuevo.length; },
    });
    const disparar = (inputType, extra = {}) => raiz.disparar({ target: campo, inputType, isComposing: false, ...extra });
    campo.escribir = cadena => {
        for (const ch of cadena) {
            if (v.length >= Number(atributos.maxlength ?? Infinity)) continue;
            const c = campo.selectionStart;
            v = v.slice(0, c) + ch + v.slice(c);
            campo.selectionStart = c + 1;
            disparar('insertText');
        }
    };
    campo.retroceso = () => {
        const c = campo.selectionStart;
        if (c === 0) return;
        v = v.slice(0, c - 1) + v.slice(c);
        campo.selectionStart = c - 1;
        disparar('deleteContentBackward');
    };
    campo.pegar = cadena => {
        const c = campo.selectionStart;
        v = v.slice(0, c) + cadena + v.slice(c);
        campo.selectionStart = c + cadena.length;
        disparar('insertFromPaste');
    };
    campo.dispararSinTecla = (inputType, extra) => disparar(inputType, extra);
    return campo;
}

function raiz() {
    const oyentes = [];
    return { addEventListener: (tipo, fn) => oyentes.push([tipo, fn]), oyentes, disparar(evento) { for (const [tipo, fn] of oyentes) if (tipo === 'input') fn(evento); } };
}

const FECHA = atributosDe(`<input ${ATRIBUTOS_DE_FECHA}>`);
const preparar = (atributos, opciones) => { const r = raiz(); enmascarar(r); return { r, campo: crearCampo(r, atributos, opciones) }; };

test('engancha un solo oyente, el de «input»', () => {
    const r = raiz();
    enmascarar(r);
    assert.deepEqual(r.oyentes.map(([tipo]) => tipo), ['input']);
});

test('ATRIBUTOS_DE_FECHA pide la máscara, el teclado numérico, 10 caracteres y sin autocompletado', () => {
    assert.deepEqual(FECHA, { 'data-mascara': 'fecha', inputmode: 'numeric', maxlength: '10', autocomplete: 'off' });
});

test('teclear una fecha en un campo vacío pone las barras y deja el cursor al final', () => {
    const { campo } = preparar(FECHA);
    campo.escribir('28102026');
    assert.equal(campo.value, '28/10/2026');
    assert.equal(campo.selectionStart, 10);
});

test('teclear una fecha con barras no las duplica', () => {
    const { campo } = preparar(FECHA);
    campo.escribir('28/10/2026');
    assert.equal(campo.value, '28/10/2026');
});

test('sólo corrige cuando hace falta: un dígito que ya queda bien no reasigna el valor', () => {
    const { campo } = preparar(FECHA);
    campo.escribir('2');
    const antes = campo.escrituras;
    campo.escribir('8');            // «28» → «28/»: hay que corregir
    assert.equal(campo.escrituras, antes + 1);
    campo.escribir('/');            // «28//» → «28/»: hay que corregir
    campo.escribir('1');            // «28/1»: ya estaba bien, no se toca
    const despues = campo.escrituras;
    assert.equal(campo.value, '28/1');
    campo.escribir('0');
    assert.equal(campo.escrituras, despues + 1, 'solo «28/10» → «28/10/» reasigna');
});

test('Retroceso recorre la fecha entera sin atascarse', () => {
    const { campo } = preparar(FECHA, { valor: '28/10/2026' });
    const vistos = [];
    for (let i = 0; i < 10; i++) { campo.retroceso(); vistos.push(campo.value); }
    assert.deepEqual(vistos, ['28/10/202', '28/10/20', '28/10/2', '28/10/', '28/10', '28/1', '28/', '28', '2', '']);
});

test('después de borrar la barra automática se puede seguir tecleando', () => {
    const { campo } = preparar(FECHA);
    campo.escribir('28');
    assert.equal(campo.value, '28/');
    campo.retroceso();
    assert.equal(campo.value, '28');
    campo.escribir('10');
    assert.equal(campo.value, '28/10/');
});

test('pegar con distintos separadores, y un año de dos dígitos sin expandir', () => {
    for (const [pegado, esperado] of [['28102026', '28/10/2026'], ['28/10/2026', '28/10/2026'], ['28-10-2026', '28/10/2026'], ['28/10/26', '28/10/26'], ['hola 28', '28/']]) {
        const { campo } = preparar(FECHA);
        campo.pegar(pegado);
        assert.equal(campo.value, esperado, `pegado «${pegado}»`);
    }
});

test('editar una fecha ya cargada: teclear en medio y borrar en medio dejan el cursor en su sitio', () => {
    const { campo } = preparar(FECHA, { valor: '05/03/2027' });
    // Borrar el «3» del mes: «05/|3/2027» con el cursor tras el «3» → «05/0» no: se prueba el caso real.
    campo.selectionStart = 5;           // «05/03|/2027»
    campo.retroceso();                  // → «05/0/2027», cursor 4
    assert.equal(campo.value, '05/0/2027');
    assert.equal(campo.selectionStart, 4);
    campo.escribir('4');                // «05/04|/2027»: ya cabe, no hay nada que corregir
    assert.equal(campo.value, '05/04/2027');
    assert.equal(campo.selectionStart, 5);
});

test('un valor ya correcto cargado en el campo no se toca hasta que el usuario edita', () => {
    const { campo } = preparar(FECHA, { valor: '05/03/2027' });
    assert.equal(campo.escrituras, 0);
    assert.equal(campo.value, '05/03/2027');
});

test('un campo sin data-mascara no se toca', () => {
    const { campo } = preparar({ placeholder: 'dd/mm/aaaa' });
    campo.escribir('28102026');
    assert.equal(campo.value, '28102026');
    const { campo: otra } = preparar({ 'data-mascara': 'otra' });
    otra.escribir('2810');
    assert.equal(otra.value, '2810');
});

test('un campo de solo lectura o deshabilitado no se reformatea', () => {
    const { campo } = preparar(FECHA, { valor: '2810', readOnly: true });
    campo.dispararSinTecla('insertText');
    assert.equal(campo.value, '2810');
    const { campo: apagado } = preparar(FECHA, { valor: '2810', disabled: true });
    apagado.dispararSinTecla('insertText');
    assert.equal(apagado.value, '2810');
});

test('durante una composición (IME) no se interfiere', () => {
    const { campo } = preparar(FECHA, { valor: '2810' });
    campo.dispararSinTecla('insertCompositionText', { isComposing: true });
    assert.equal(campo.value, '2810');
});

test('un evento cuyo objetivo no es un elemento (document, texto) se ignora sin fallar', () => {
    const r = raiz();
    enmascarar(r);
    r.disparar({ target: null });
    r.disparar({ target: {} });
    r.disparar({ target: { getAttribute: () => null } });
});

test('un navegador sin setSelectionRange o que lo rechaza no rompe la máscara', () => {
    const r = raiz();
    enmascarar(r);
    const campo = { value: '28', selectionStart: 2, getAttribute: n => FECHA[n] ?? null };
    r.disparar({ target: campo, inputType: 'insertText' });
    assert.equal(campo.value, '28/');
    const malo = { value: '28', selectionStart: 2, getAttribute: n => FECHA[n] ?? null, setSelectionRange() { throw new Error('InvalidStateError'); } };
    r.disparar({ target: malo, inputType: 'insertText' });
    assert.equal(malo.value, '28/');
});

test('si el navegador no informa el cursor se asume que está al final', () => {
    const r = raiz();
    enmascarar(r);
    const campo = { value: '2810', selectionStart: null, getAttribute: n => FECHA[n] ?? null };
    r.disparar({ target: campo, inputType: 'insertText' });
    assert.equal(campo.value, '28/10/');
});

// --- Campos de las plantillas reales ---------------------------------------------------------

const ETIQUETA = (html, id) => html.match(new RegExp(`<input[^>]*id="${id}"[^>]*>`))?.[0];

async function dibujarGastos() {
    const pantalla = { contenido: { innerHTML: '' } };
    const vista = new VistaGastos({
        api: { obtenerGastos: async () => [], obtenerCategorias: async () => [{ id: 1, nombre: 'Comida' }], obtenerCuentas: async () => [], obtenerTarjetas: async () => [] },
        avisos: { mostrar() {} }, formato: { importe: v => `#${v}#` }, enrutador: { mostrar: async () => {} }, pantalla,
        dom: { elemento: () => { throw new Error('no se usa'); }, buscar: () => null },
        modales: { abrir() {} }, ahora: () => new Date(2027, 2, 5),
    });
    await vista.render();
    return pantalla.contenido.innerHTML;
}

test('el campo de fecha de Gastos, tal como lo dibuja la vista, funciona con la máscara (con valor inicial)', async () => {
    const etiqueta = ETIQUETA(await dibujarGastos(), 'gas_fec');
    assert.ok(etiqueta, 'la vista dibuja #gas_fec');
    const atributos = atributosDe(etiqueta);
    assert.equal(atributos['data-mascara'], 'fecha');
    assert.equal(atributos.inputmode, 'numeric');
    assert.equal(atributos.maxlength, '10');
    assert.equal(atributos.autocomplete, 'off');
    assert.equal(atributos.placeholder, 'dd/mm/aaaa');
    assert.match(etiqueta, /\srequired[\s>]/);
    // Ya viene con la fecha de hoy: se edita el día sin perder el resto.
    assert.equal(atributos.value, '05/03/2027');
    const { campo } = preparar(atributos, { valor: atributos.value });
    campo.selectionStart = 2;               // «05|/03/2027»
    campo.retroceso();                      // «0/03/2027»
    campo.escribir('7');                    // «07/03/2027»
    assert.equal(campo.value, '07/03/2027');
});
