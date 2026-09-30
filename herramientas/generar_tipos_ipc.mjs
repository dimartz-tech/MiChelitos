// Genera `src/js/tipos-ipc.js` —los tipos del contrato entre la interfaz y
// Rust— a partir de `src-tauri/src/main.rs`.
//
//   node herramientas/generar_tipos_ipc.mjs            escribe el archivo
//   node herramientas/generar_tipos_ipc.mjs --comprobar  falla si está desactualizado
//
// Rust es la fuente: una estructura o un comando que cambian sin regenerar
// hacen fallar `src/js/contrato/tipos.test.js`. Sin dependencias.

import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), '..');
const FUENTE = join(RAIZ, 'src-tauri', 'src', 'main.rs');
const DESTINO = join(RAIZ, 'src', 'js', 'tipos-ipc.js');

const PRIMITIVOS = { i64: 'number', i32: 'number', u32: 'number', f64: 'number', String: 'string', bool: 'boolean' };

/** Tipo de Rust → tipo de JSDoc. Falla ante uno desconocido: mejor romper que mentir. */
export function tipoJs(rust) {
    const t = rust.replace(/\s+/g, ' ').trim();
    let m;
    if ((m = /^Option<(.+)>$/.exec(t))) return `${tipoJs(m[1])} | null`;
    if ((m = /^Vec<(.+)>$/.exec(t))) {
        const interno = tipoJs(m[1]);
        return interno.includes(' ') ? `(${interno})[]` : `${interno}[]`;
    }
    if (t === '()') return 'null';
    if (t === 'Value' || t === 'serde_json::Value') return 'any';
    // El importe viaja como texto o como número; el texto decide el céntimo.
    if (t === 'ipc::ImporteDecimal') return 'string | number';
    if (t in PRIMITIVOS) return PRIMITIVOS[t];
    if (/^[A-Z]\w*$/.test(t)) return t; // estructura declarada en main.rs
    throw new Error(`tipo de Rust sin traducción: ${t}`);
}

/** Separa por comas de primer nivel (las hay dentro de `<...>`). */
function partirArgumentos(texto) {
    const partes = [];
    let profundidad = 0;
    let actual = '';
    for (const c of texto) {
        if ('<('.includes(c)) profundidad++;
        if ('>)'.includes(c)) profundidad--;
        if (c === ',' && profundidad === 0) { partes.push(actual); actual = ''; } else actual += c;
    }
    partes.push(actual);
    return partes.map(p => p.trim()).filter(Boolean);
}

const camelCase = s => s.replace(/_([a-z0-9])/g, (_, c) => c.toUpperCase());
const sinComentarios = s => s.replace(/\/\/[^\n]*/g, '');

function estructuras(rust, derivar) {
    const salida = [];
    const re = new RegExp(`#\\[derive\\(([^)]*${derivar}[^)]*)\\)\\]\\s*(?:#\\[[^\\]]*\\]\\s*)*(?:pub )?struct (\\w+)\\s*\\{([\\s\\S]*?)\\n\\}`, 'g');
    for (const m of rust.matchAll(re)) {
        const campos = [...sinComentarios(m[3]).matchAll(/^\s*(?:pub\s+)?(\w+):\s*([^,\n]+),/gm)]
            .map(c => [c[1], tipoJs(c[2])]);
        salida.push([m[2], campos]);
    }
    return salida;
}

/** Devuelve los datos que alimentan el archivo generado. */
export function analizar(rust) {
    const respuestas = estructuras(rust, 'Serialize');
    const entradas = estructuras(rust, 'Deserialize').filter(([n]) => !respuestas.some(([r]) => r === n));

    const comandos = [];
    for (const m of rust.matchAll(/#\[tauri::command\]\s*(?:async\s+)?fn\s+(\w+)\s*\(([\s\S]*?)\)\s*->\s*([\s\S]*?)\s*\{/g)) {
        const argumentos = partirArgumentos(sinComentarios(m[2])).map(p => {
            const i = p.indexOf(':');
            const nombre = p.slice(0, i).trim();
            const tipo = p.slice(i + 1).trim();
            return { clave: camelCase(nombre), opcional: tipo.startsWith('Option<'), tipo: tipoJs(tipo) };
        });
        const crudo = m[3].replace(/\s+/g, ' ').trim();
        const interno = /^Result<(.+), String>$/.exec(crudo)?.[1] ?? crudo;
        comandos.push({ nombre: m[1], argumentos, retorno: tipoJs(interno) });
    }
    return { respuestas, entradas, comandos };
}

export function generar(rust) {
    const { respuestas, entradas, comandos } = analizar(rust);
    const campos = cs => cs.map(([n, t]) => `${n}: ${t}`).join(', ');
    const lineas = [
        '// GENERADO por herramientas/generar_tipos_ipc.mjs desde src-tauri/src/main.rs. No editar a mano:',
        '// se regenera con `node herramientas/generar_tipos_ipc.mjs` y una prueba exige que esté al día.',
        '// Solo contiene tipos de JSDoc; no hay código que se ejecute.',
        '// @ts-check',
        '',
        '// --- Lo que Rust devuelve ---',
        ...respuestas.map(([n, cs]) => `/** @typedef {{ ${campos(cs)} }} ${n} */`),
        '',
        '// --- Lo que Rust recibe en estructuras ---',
        ...entradas.map(([n, cs]) => `/** @typedef {{ ${campos(cs)} }} ${n} */`),
        '',
        '// --- Cada comando: sus argumentos (claves en camelCase, como las envía Tauri) y su respuesta ---',
        '/**',
        ' * @typedef {{',
        ...comandos.map(c => {
            const args = c.argumentos.map(a => `${a.clave}${a.opcional ? '?' : ''}: ${a.tipo}`).join(', ');
            return ` *   ${c.nombre}: { args: { ${args} }, ret: ${c.retorno} },`;
        }),
        ' * }} Comandos',
        ' */',
        '',
        'export {};',
        '',
    ];
    return lineas.join('\n');
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
    const texto = generar(readFileSync(FUENTE, 'utf8'));
    if (process.argv.includes('--comprobar')) {
        if (readFileSync(DESTINO, 'utf8') !== texto) {
            console.error('src/js/tipos-ipc.js está desactualizado: ejecuta `node herramientas/generar_tipos_ipc.mjs`.');
            process.exit(1);
        }
    } else {
        writeFileSync(DESTINO, texto);
        console.log(`escrito ${DESTINO}`);
    }
}
