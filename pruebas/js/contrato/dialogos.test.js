// Los diálogos de confirmación y de texto: el contrato que los protege.
//
// En el WebView de Tauri 1.x `window.confirm()` NO devuelve un booleano: Tauri lo
// sustituye por una versión asíncrona que devuelve una **promesa**, y una promesa
// es siempre «verdadera» en un `if (confirm(...))`: la acción se ejecutaba sin
// esperar la respuesta, de modo que Cancelar no cancelaba. Y `window.prompt()`
// devuelve `null` al instante sin mostrar nada (`wry` 0.24 no lo implementa en
// macOS), así que toda corrección que pedía un motivo se abandonaba en silencio.
// Se comprobó en la aplicación empaquetada. La interfaz, por tanto, solo usa el
// servicio `Dialogos` y siempre lo **espera**.
//
// Un doble de pruebas con `confirm: true` no lo habría descubierto nunca:
// devuelve un booleano de verdad. Por eso hay dos tipos de prueba aquí: las que
// leen el código y las que responden **tarde**, como el WebView real.

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fuentesDeLaInterfaz, RAIZ } from '../ayudas/fuentes_interfaz.js';
import { cargarInterfaz } from '../ayudas/cargar_interfaz.js';

/** Las líneas de código de un fuente (sin comentarios de línea ni de bloque). */
const lineasDeCodigo = ruta => readFileSync(ruta, 'utf8').split('\n')
    .map((l, i) => ({ n: i + 1, l }))
    .filter(({ l }) => !/^\s*(\/\/|\*|\/\*)/.test(l));

const SERVICIOS = join(RAIZ, 'src', 'js', 'ui', 'servicios.ts');
const interfaz = fuentesDeLaInterfaz().filter(f => f !== SERVICIOS);

test('la interfaz no llama a confirm(), prompt() ni alert() nativos', () => {
    const malas = [];
    for (const f of interfaz) {
        for (const { n, l } of lineasDeCodigo(f)) {
            if (/(?<![\w.$])(confirm|prompt|alert)\(/.test(l)) malas.push(`${f.replace(RAIZ, '')}:${n}: ${l.trim().slice(0, 70)}`);
        }
    }
    assert.deepEqual(malas, [], 'usa el servicio Dialogos (`dialogos.confirmar`/`preguntar`) y espéralo con await');
});

test('toda petición de diálogo o de motivo se espera con await', () => {
    const sinEsperar = [];
    const peticion = /(dialogos\.(confirmar|preguntar)|\.pedir|pedidorDeMotivo\.pedir|pedirMotivoDeCorreccion)\(/;
    for (const f of interfaz) {
        for (const { n, l } of lineasDeCodigo(f)) {
            const m = peticion.exec(l);
            if (!m) continue;
            if (/\basync\s+pedirMotivoDeCorreccion\(/.test(l)) continue;                    // la definición
            if (!new RegExp(`await\\s+(this\\.)?${m[0].replace(/[.()]/g, '\\$&').replace(/^\\\./, '')}`).test(l)
                && !/await\s+[\w.]*(confirmar|preguntar|pedir|pedirMotivoDeCorreccion)\(/.test(l)) {
                sinEsperar.push(`${f.replace(RAIZ, '')}:${n}: ${l.trim().slice(0, 70)}`);
            }
        }
    }
    assert.deepEqual(sinEsperar, [], 'una promesa sin esperar es siempre «verdadera»: Cancelar no cancelaría');
});

/** Responde después, como el WebView real, y no antes. */
const tarde = valor => () => new Promise(resolver => setTimeout(() => resolver(valor), 5));

test('un borrado con motivo espera la confirmación: no toca la API hasta que se acepta, y Cancelar no borra', async () => {
    const MOTIVO = 'Registrado por error en la cuenta equivocada';
    const aceptada = cargarInterfaz({ confirm: tarde(true), prompt: tarde(MOTIVO) });
    const espera = aceptada.appUI.handleEliminarGastoCorr(7);
    assert.deepEqual(aceptada.nombresLlamados, [], 'no debe llamar a la API antes de que el titular responda');
    await espera;
    assert.deepEqual(aceptada.llamadasA('eliminarGasto').map(l => l.args), [[7, MOTIVO]]);

    const cancelada = cargarInterfaz({ confirm: tarde(false), prompt: tarde(MOTIVO) });
    await cancelada.appUI.handleEliminarGastoCorr(7);
    assert.deepEqual(cancelada.nombresLlamados, []);
    assert.equal(cancelada.preguntas.length, 0, 'tras cancelar no se pide el motivo');
});

test('un motivo cancelado, vacío o corto no borra aunque la respuesta llegue tarde', async () => {
    for (const respuesta of [null, '', 'corto']) {
        const ui = cargarInterfaz({ confirm: tarde(true), prompt: tarde(respuesta) });
        await ui.appUI.handleEliminarTransaccionCuentaCorr(3);
        assert.deepEqual(ui.nombresLlamados, [], String(respuesta));
    }
});

test('una vista extraída también espera la confirmación', async () => {
    const aceptada = cargarInterfaz({ confirm: tarde(true) });
    const espera = aceptada.appUI.handleEliminarPrestamo(5);
    assert.deepEqual(aceptada.nombresLlamados, []);
    await espera;
    assert.deepEqual(aceptada.llamadasA('eliminarPrestamo').map(l => l.args), [[5]]);

    const cancelada = cargarInterfaz({ confirm: tarde(false) });
    await cancelada.appUI.handleEliminarPrestamo(5);
    assert.deepEqual(cancelada.nombresLlamados, []);
});
