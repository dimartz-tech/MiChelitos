// El contrato entre la interfaz y el núcleo: que cada comando de Rust reciba
// todo lo que exige.
//
// Existe por un defecto que las pruebas de Rust no podían ver. Estas llaman a
// los comandos directamente, sin pasar por `invoke`, de modo que un wrapper de
// `api.js` que **descarta un argumento** compilaba, pasaba todas las pruebas y
// fallaba en la aplicación: Tauri rechaza la llamada si falta una clave
// obligatoria.
//
// Así ocurrió con `motivo`. La interfaz lo pedía al usuario, se lo pasaba al
// wrapper, y el wrapper no lo reenviaba: cuatro acciones —borrar un gasto,
// borrar un traspaso, revertir un abono, borrar un ingreso informal— habrían
// fallado en cuanto la aplicación se instalara.
//
// La prueba lee los dos archivos como texto, sin dependencias. No sustituye a
// ejecutar la aplicación, pero cierra la clase de error que ninguna otra
// prueba del proyecto cubría.

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..');
const rust = readFileSync(join(RAIZ, 'src-tauri', 'src', 'main.rs'), 'utf8');
const js = readFileSync(join(RAIZ, 'src', 'js', 'api.js'), 'utf8');

const aCamello = (s) => s.replace(/_([a-z0-9])/g, (_, c) => c.toUpperCase());

/** Quita comentarios de línea y de bloque, respetando las cadenas. */
function sinComentarios(texto) {
    let salida = '';
    for (let i = 0; i < texto.length; i++) {
        const c = texto[i];
        if (c === '"' || c === "'" || c === '`') {
            const cierre = c;
            salida += c;
            for (i++; i < texto.length && texto[i] !== cierre; i++) {
                if (texto[i] === '\\') { salida += texto[i++]; }
                salida += texto[i];
            }
            salida += cierre;
        } else if (c === '/' && texto[i + 1] === '/') {
            while (i < texto.length && texto[i] !== '\n') i++;
            salida += '\n';
        } else if (c === '/' && texto[i + 1] === '*') {
            i += 2;
            while (i < texto.length && !(texto[i] === '*' && texto[i + 1] === '/')) i++;
            i++;
        } else {
            salida += c;
        }
    }
    return salida;
}

/** Devuelve el contenido entre el delimitador que abre en `desde` y su pareja. */
function entreDelimitadores(texto, desde) {
    const pares = { '(': ')', '{': '}', '[': ']' };
    const abre = texto[desde];
    let nivel = 0;
    for (let i = desde; i < texto.length; i++) {
        const c = texto[i];
        if (c === '"' || c === "'" || c === '`') {
            for (i++; i < texto.length && texto[i] !== c; i++) if (texto[i] === '\\') i++;
            continue;
        }
        if (c in pares) nivel++;
        if (c === ')' || c === '}' || c === ']') {
            nivel--;
            if (nivel === 0) return texto.slice(desde + 1, i);
        }
    }
    throw new Error(`delimitador ${abre} sin cerrar`);
}

/** Parte por las comas de primer nivel, ignorando las de dentro de pares. */
function partirPorComas(texto) {
    const partes = [];
    let nivel = 0;
    let actual = '';
    for (let i = 0; i < texto.length; i++) {
        const c = texto[i];
        if (c === '"' || c === "'" || c === '`') {
            actual += c;
            for (i++; i < texto.length && texto[i] !== c; i++) {
                if (texto[i] === '\\') actual += texto[i++];
                actual += texto[i];
            }
            actual += c;
            continue;
        }
        if ('({[<'.includes(c)) nivel++;
        if (')}]>'.includes(c)) nivel--;
        if (c === ',' && nivel === 0) { partes.push(actual); actual = ''; }
        else actual += c;
    }
    if (actual.trim()) partes.push(actual);
    return partes.map((p) => p.trim()).filter(Boolean);
}

/** Comandos Rust: nombre → { obligatorias, opcionales } en la forma que ve JS. */
function comandosDeRust() {
    const fuente = sinComentarios(rust);
    const comandos = new Map();
    const patron = /#\[tauri::command\]\s*(?:pub\s+)?fn\s+(\w+)\s*\(/g;
    let m;
    while ((m = patron.exec(fuente))) {
        const cuerpo = entreDelimitadores(fuente, m.index + m[0].length - 1);
        const obligatorias = new Set();
        const opcionales = new Set();
        for (const parametro of partirPorComas(cuerpo)) {
            const dos = parametro.indexOf(':');
            if (dos < 0) continue;
            const nombre = parametro.slice(0, dos).replace(/\bmut\b/, '').trim();
            const tipo = parametro.slice(dos + 1).trim();
            (tipo.startsWith('Option<') ? opcionales : obligatorias).add(aCamello(nombre));
        }
        comandos.set(m[1], { obligatorias, opcionales });
    }
    return comandos;
}

/** Invocaciones JS: comando → claves que el wrapper envía. */
function invocacionesDeJs() {
    const fuente = sinComentarios(js);
    const llamadas = new Map();
    const patron = /invoke\(\s*'(\w+)'\s*(,\s*)?/g;
    let m;
    while ((m = patron.exec(fuente))) {
        const claves = new Set();
        if (m[2]) {
            const inicio = m.index + m[0].length;
            if (fuente[inicio] === '{') {
                for (const par of partirPorComas(entreDelimitadores(fuente, inicio))) {
                    const clave = par.split(':')[0].trim();
                    if (/^\w+$/.test(clave)) claves.add(clave);
                }
            }
        }
        llamadas.set(m[1], claves);
    }
    return llamadas;
}

const comandos = comandosDeRust();
const llamadas = invocacionesDeJs();

test('el analizador encuentra los comandos y las invocaciones', () => {
    assert.ok(comandos.size > 40, `solo encontró ${comandos.size} comandos de Rust`);
    assert.ok(llamadas.size > 40, `solo encontró ${llamadas.size} invocaciones de JS`);
    // Un caso conocido con su forma exacta, para que un analizador roto que
    // no encuentre nada no pase por una interfaz sin errores.
    const eliminar = comandos.get('eliminar_gasto');
    assert.deepEqual([...eliminar.obligatorias].sort(), ['id', 'motivo']);
});

test('cada wrapper envía todo lo que su comando de Rust exige', () => {
    const faltan = [];
    for (const [nombre, { obligatorias }] of comandos) {
        const enviadas = llamadas.get(nombre);
        if (!enviadas) continue; // comando sin wrapper: lo cubre la prueba de abajo
        const ausentes = [...obligatorias].filter((k) => !enviadas.has(k));
        if (ausentes.length) faltan.push(`${nombre}: falta ${ausentes.join(', ')}`);
    }
    assert.deepEqual(faltan, [], `Tauri rechaza estas llamadas:\n  ${faltan.join('\n  ')}`);
});

test('ningún wrapper envía una clave que su comando no conoce', () => {
    const sobran = [];
    for (const [nombre, enviadas] of llamadas) {
        const comando = comandos.get(nombre);
        if (!comando) { sobran.push(`${nombre}: el comando no existe en Rust`); continue; }
        const conocidas = new Set([...comando.obligatorias, ...comando.opcionales]);
        const extra = [...enviadas].filter((k) => !conocidas.has(k));
        if (extra.length) sobran.push(`${nombre}: ${extra.join(', ')}`);
    }
    assert.deepEqual(sobran, [], `Claves que se enviarían al vacío:\n  ${sobran.join('\n  ')}`);
});

test('toda llamada de la interfaz a AppAPI tiene su envoltorio en api.ts', () => {
    // El panel «Casos de corrección» llamaba a `AppAPI.obtenerCorrecciones`,
    // que nadie escribió nunca: el comando existía en Rust, pero el envoltorio
    // no. La prueba de arriba solo mira de `api` hacia Rust; esta cierra el
    // otro tramo, de la interfaz hacia `api`.
    const fuentesDeLaInterfaz = ['ui.ts', 'app.ts', join('ui', 'dom.ts')]
        .map(f => { try { return readFileSync(join(RAIZ, 'src', 'js', f), 'utf8'); } catch { return ''; } })
        .join('\n');
    const api = readFileSync(join(RAIZ, 'src', 'js', 'api.ts'), 'utf8');
    const definidos = new Set([...api.matchAll(/^    async (\w+)\(/gm)].map(m => m[1]));
    const usados = new Set([...fuentesDeLaInterfaz.matchAll(/\bAppAPI\.(\w+)/g)].map(m => m[1]));

    assert.ok(usados.size > 40, `se esperaban decenas de llamadas, hay ${usados.size}`);
    const sinEnvoltorio = [...usados].filter(n => !definidos.has(n));
    assert.deepEqual(sinEnvoltorio, [], 'la interfaz llama a métodos de AppAPI que no existen');
});
