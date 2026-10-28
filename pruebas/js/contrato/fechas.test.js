// Contrato de los campos de fecha escritos a mano (`<input type="text" … placeholder="dd/mm/aaaa">`):
// todos llevan la máscara que pone la `/` sola (`ATRIBUTOS_DE_FECHA` de `nucleo/html.ts`, aplicada
// por el oyente de `ui/mascaras.ts`). Recorre TODAS las plantillas de la interfaz y el index.html,
// de modo que un campo de fecha nuevo que se olvide de la máscara hace fallar esta prueba.
//
// Un campo se considera «de fecha» si lleva el placeholder dd/mm/aaaa **o** su id dice que lo es
// (`…fec`, `…fecha…`, `…_ven`). Los `<input type="date">` usan el selector nativo y no se tocan.

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fuentesDeLaInterfaz, leerHtml } from '../ayudas/fuentes_interfaz.js';
import { ATRIBUTOS_DE_FECHA } from '../../../src/js/nucleo/html.js';

const FUENTES = [...fuentesDeLaInterfaz().map(f => ({ nombre: f.replace(/^.*\/src\//, 'src/'), texto: readFileSync(f, 'utf8') })), { nombre: 'index.html', texto: leerHtml() }];

/** Una etiqueta `<input …>`; admite `${ … }` con un `>` dentro. */
const ETIQUETA = /<input(?:[^>$]|\$\{[^}]*\}|\$)*>/g;
const ID_DE_FECHA = /^(?:[a-z]+_)*(?:fec|fecha|ven)(?:_|\$|$)/;

function camposDeFecha() {
    const campos = [];
    for (const { nombre, texto } of FUENTES) {
        for (const m of texto.matchAll(ETIQUETA)) {
            const etiqueta = m[0];
            const id = etiqueta.match(/\sid="([^"]*)"/)?.[1] ?? '';
            const tipo = etiqueta.match(/\stype="([^"]*)"/)?.[1] ?? 'text';
            if (tipo === 'date') continue;
            if (/placeholder="dd\/mm\/aaaa"/.test(etiqueta) || (tipo === 'text' && ID_DE_FECHA.test(id))) {
                campos.push({ nombre, id, etiqueta, linea: texto.slice(0, m.index).split('\n').length });
            }
        }
    }
    return campos;
}

const LLEVA_MASCARA = etiqueta => etiqueta.includes('${ATRIBUTOS_DE_FECHA}') || /\sdata-mascara="fecha"/.test(etiqueta);

test('el recorrido encuentra los campos de fecha de las plantillas (la prueba no está vacía)', () => {
    const ids = camposDeFecha().map(c => c.id.replace(/\$\{[^}]*\}/g, '${…}'));
    for (const esperado of ['pag_fecha_${…}', 'avc_fecha_${…}', 'bon_fecha', 'gas_fec', 'fecha_inf', 'fec_em', 'cob_fec_${…}', 'cob_fec_inf_${…}', 'cer_ven', 'bol_ven', 'efe_inf_fec', 'efe_ret_fec', 'tra_fec', 'edit_fec_em_${…}']) {
        assert.ok(ids.includes(esperado), `no se encontró el campo de fecha #${esperado}`);
    }
});

test('todo campo de fecha de texto lleva la máscara (ATRIBUTOS_DE_FECHA)', () => {
    const sin = camposDeFecha().filter(c => !LLEVA_MASCARA(c.etiqueta)).map(c => `${c.nombre}:${c.linea} #${c.id}`);
    assert.deepEqual(sin, [], 'añade ${ATRIBUTOS_DE_FECHA} (de nucleo/html.ts) a la etiqueta para que la «/» se ponga sola');
});

test('todo campo con máscara de fecha conserva su placeholder dd/mm/aaaa y no es un <input type="date">', () => {
    const mal = camposDeFecha().filter(c => LLEVA_MASCARA(c.etiqueta) && (!/placeholder="dd\/mm\/aaaa"/.test(c.etiqueta) || /type="date"/.test(c.etiqueta)))
        .map(c => `${c.nombre}:${c.linea} #${c.id}`);
    assert.deepEqual(mal, []);
});

test('ningún campo de fecha repite ni contradice los atributos de la máscara (maxlength, inputmode, autocomplete)', () => {
    const repetidos = camposDeFecha().filter(c => LLEVA_MASCARA(c.etiqueta))
        .filter(c => /\s(?:maxlength|inputmode|autocomplete)=/.test(c.etiqueta.replace('${ATRIBUTOS_DE_FECHA}', '')))
        .map(c => `${c.nombre}:${c.linea} #${c.id}`);
    assert.deepEqual(repetidos, []);
});

test('la plantilla de los atributos: máscara de fecha, teclado numérico, 10 caracteres y sin autocompletado', () => {
    assert.equal(ATRIBUTOS_DE_FECHA, 'data-mascara="fecha" inputmode="numeric" maxlength="10" autocomplete="off"');
});

test('ningún campo con máscara lleva un manejador en línea (la arquitectura es declarativa)', () => {
    for (const c of camposDeFecha()) assert.doesNotMatch(c.etiqueta, /\son[a-z]+=/, `${c.nombre}:${c.linea}`);
});
