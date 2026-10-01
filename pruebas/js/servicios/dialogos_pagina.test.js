// Los diálogos reales (`crearDialogosDePagina`), con un DOM de juguete.
//
// Sustituyen a `confirm()` y `prompt()` del WebView, que en Tauri 1.x no sirven
// (ver `pruebas/js/contrato/dialogos.test.js`). Aquí se fija lo que prometen:
// una confirmación devuelve `true` solo si se acepta; una pregunta devuelve el
// texto o `null`; Escape cancela, Enter acepta; el mensaje se escapa y solo
// admite negrita; el diálogo se retira al responder; y en una confirmación el
// foco empieza en Cancelar, porque casi todas son borrados.

import test from 'node:test';
import assert from 'node:assert/strict';
import { crearDialogosDePagina } from '../../../src/js/ui/servicios.js';

function instalarDom() {
    const cuerpo = { hijos: [], appendChild(h) { this.hijos.push(h); } };
    const foco = { en: null };
    const elemento = nombre => ({ dataset: { dialogo: nombre }, value: '', focus() { foco.en = nombre; }, select() { this.seleccionado = true; } });
    const creados = [];
    const document = {
        body: cuerpo,
        createElement: () => {
            const capa = {
                style: {}, attrs: {}, partes: {},
                setAttribute(k, v) { this.attrs[k] = v; },
                remove() { cuerpo.hijos = cuerpo.hijos.filter(h => h !== capa); this.quitada = true; },
                set innerHTML(html) {
                    this._html = html;
                    this.partes = { aceptar: elemento('aceptar'), cancelar: elemento('cancelar') };
                    if (html.includes('data-dialogo="texto"')) this.partes.texto = elemento('texto');
                },
                get innerHTML() { return this._html; },
                querySelector(sel) { return this.partes[/"(\w+)"/.exec(sel)[1]] ?? null; },
            };
            creados.push(capa);
            return capa;
        },
    };
    const previo = globalThis.document;
    globalThis.document = document;
    return { cuerpo, foco, creados, restaurar: () => { globalThis.document = previo; } };
}

async function conDom(prueba) {
    const dom = instalarDom();
    try { await prueba(dom); } finally { dom.restaurar(); }
}
const tecla = (capa, key, objetivo) => capa.onkeydown({ key, target: objetivo, preventDefault() {} });

test('confirmar: queda pendiente hasta que se responde y entonces devuelve true al aceptar', () => conDom(async dom => {
    const dialogos = crearDialogosDePagina();
    const promesa = dialogos.confirmar('¿Seguro?');
    let resuelta = false;
    promesa.then(() => { resuelta = true; });
    await new Promise(r => setImmediate(r));
    assert.equal(resuelta, false, 'no responde por sí sola');
    assert.equal(dom.cuerpo.hijos.length, 1);
    dom.creados[0].partes.aceptar.onclick();
    assert.equal(await promesa, true);
    assert.equal(dom.cuerpo.hijos.length, 0, 'el diálogo se retira al responder');
}));

test('confirmar: Cancelar y Escape devuelven false; el foco empieza en Cancelar', () => conDom(async dom => {
    const dialogos = crearDialogosDePagina();
    const a = dialogos.confirmar('¿Seguro?');
    assert.equal(dom.foco.en, 'cancelar');
    dom.creados[0].partes.cancelar.onclick();
    assert.equal(await a, false);
    const b = dialogos.confirmar('¿Seguro?');
    tecla(dom.creados[1], 'Escape');
    assert.equal(await b, false);
}));

test('confirmar: Enter no acepta por sí solo (el foco está en Cancelar)', () => conDom(async dom => {
    const dialogos = crearDialogosDePagina();
    let resuelta = false;
    const p = dialogos.confirmar('¿Seguro?'); p.then(() => { resuelta = true; });
    tecla(dom.creados[0], 'Enter', dom.creados[0].partes.cancelar);
    await new Promise(r => setImmediate(r));
    assert.equal(resuelta, false);
    dom.creados[0].partes.cancelar.onclick();
    await p;
}));

test('preguntar: devuelve el texto escrito al aceptar y null al cancelar', () => conDom(async dom => {
    const dialogos = crearDialogosDePagina();
    const a = dialogos.preguntar('Motivo:');
    dom.creados[0].partes.texto.value = 'Registrado por error';
    dom.creados[0].partes.aceptar.onclick();
    assert.equal(await a, 'Registrado por error');
    const b = dialogos.preguntar('Motivo:');
    dom.creados[1].partes.texto.value = 'algo escrito';
    dom.creados[1].partes.cancelar.onclick();
    assert.equal(await b, null);
}));

test('preguntar: parte del valor por defecto, selecciona el campo y Enter acepta; Escape cancela', () => conDom(async dom => {
    const dialogos = crearDialogosDePagina();
    const a = dialogos.preguntar('Saldo:', '1250.00');
    const campo = dom.creados[0].partes.texto;
    assert.equal(campo.value, '1250.00');
    assert.equal(dom.foco.en, 'texto');
    assert.equal(campo.seleccionado, true);
    tecla(dom.creados[0], 'Enter', campo);
    assert.equal(await a, '1250.00');
    const b = dialogos.preguntar('Saldo:');
    assert.equal(dom.creados[1].partes.texto.value, '');
    tecla(dom.creados[1], 'Escape');
    assert.equal(await b, null);
}));

test('un texto vacío aceptado es una cadena vacía, no null (quien llama decide si vale)', () => conDom(async dom => {
    const p = crearDialogosDePagina().preguntar('Motivo:');
    dom.creados[0].partes.aceptar.onclick();
    assert.equal(await p, '');
}));

test('el mensaje se escapa y solo admite negrita con **…**', () => conDom(async dom => {
    const p = crearDialogosDePagina().confirmar('Vas a borrar <b>esto</b> & "aquello".\nEsto **destruye el movimiento**.');
    const html = dom.creados[0].innerHTML;
    assert.match(html, /Vas a borrar &lt;b&gt;esto&lt;\/b&gt; &amp; &quot;aquello&quot;\./);
    assert.match(html, /Esto <strong>destruye el movimiento<\/strong>\./);
    assert.doesNotMatch(html, /<b>esto/);
    assert.equal(dom.creados[0].attrs.role, 'dialog');
    assert.equal(dom.creados[0].style.zIndex, '2000');     // por encima de una ventana modal ya abierta
    dom.creados[0].partes.cancelar.onclick();
    await p;
}));
