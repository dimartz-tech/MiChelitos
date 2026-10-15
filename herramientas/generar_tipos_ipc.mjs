// Genera `src/js/tipos-ipc.d.ts` —los tipos del contrato entre la interfaz y
// Rust— a partir de `src-tauri/src/main.rs` y, si existe, de los módulos de
// comandos en `src-tauri/src/comandos/*.rs`.
//
//   node herramientas/generar_tipos_ipc.mjs            escribe el archivo
//   node herramientas/generar_tipos_ipc.mjs --comprobar  falla si está desactualizado
//
// Rust es la fuente: una estructura o un comando que cambian sin regenerar
// hacen fallar `src/js/contrato/tipos.test.js`. Sin dependencias.
//
// Falla en voz alta, nunca en silencio: si el número de `#[tauri::command]` no
// coincide con los comandos que entiende, o con los registrados en
// `generate_handler!`, se niega a generar.

import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), '..');
const DIR_FUENTES = join(RAIZ, 'src-tauri', 'src');
const DESTINO = join(RAIZ, 'src', 'js', 'tipos-ipc.d.ts');

const PRIMITIVOS = { i64: 'number', i32: 'number', u32: 'number', f64: 'number', String: 'string', bool: 'boolean' };

/** Todo el Rust del contrato: `main.rs` y los módulos de `comandos/`, en orden estable. */
export function leerFuentesRust(directorio = DIR_FUENTES) {
    const archivos = [join(directorio, 'main.rs')];
    const dirComandos = join(directorio, 'comandos');
    if (existsSync(dirComandos)) {
        const extra = readdirSync(dirComandos).filter(f => f.endsWith('.rs')).sort();
        archivos.push(...extra.map(f => join(dirComandos, f)));
    }
    return archivos.map(a => readFileSync(a, 'utf8')).join('\n');
}

/** Tipo de Rust → tipo de TypeScript. Falla ante uno desconocido: mejor romper que mentir. */
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
    // El importe viaja solo como texto: los dígitos escritos deciden el céntimo.
    if (t === 'ipc::ImporteDecimal') return 'string';
    if (t in PRIMITIVOS) return PRIMITIVOS[t];
    if (/^(?:\w+::)*[A-Z]\w*$/.test(t)) return t.split('::').pop(); // estructura del contrato, con o sin ruta de módulo
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
    const re = new RegExp(`#\\[derive\\(([^)]*${derivar}[^)]*)\\)\\]\\s*(?:#\\[[^\\]]*\\]\\s*)*(?:pub(?:\\([^)]*\\))? )?struct (\\w+)\\s*\\{([\\s\\S]*?)\\n\\}`, 'g');
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
    for (const m of rust.matchAll(/#\[tauri::command\]\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(\w+)\s*\(([\s\S]*?)\)\s*->\s*([\s\S]*?)\s*\{/g)) {
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
    verificarCoherencia(rust, comandos);
    return { respuestas, entradas, comandos };
}

/** Los tres recuentos tienen que coincidir: atributos, comandos entendidos y registrados. */
function verificarCoherencia(rust, comandos) {
    const fuente = sinComentarios(rust);
    const marcados = (fuente.match(/#\[tauri::command\]/g) ?? []).length;
    if (marcados !== comandos.length) {
        throw new Error(`hay ${marcados} #[tauri::command] y el generador solo entiende ${comandos.length}: ` +
            'revisa que cada uno tenga la forma `[pub] [async] fn nombre(...) -> Result<T, String>`');
    }
    const bloque = /generate_handler!\s*\[([\s\S]*?)\]/.exec(fuente);
    if (!bloque) throw new Error('no se encontró generate_handler![...]');
    const registrados = bloque[1].split(',').map(x => x.trim().split('::').pop()).filter(Boolean);
    const nombres = new Set(comandos.map(c => c.nombre));
    const sinRegistrar = [...nombres].filter(n => !registrados.includes(n));
    const sinComando = registrados.filter(n => !nombres.has(n));
    if (sinRegistrar.length || sinComando.length) {
        throw new Error(`generate_handler! no coincide con los comandos. Sin registrar: [${sinRegistrar}]. Registrados sin comando: [${sinComando}]`);
    }
}

export function generar(rust) {
    const { respuestas, entradas, comandos } = analizar(rust);
    const campos = cs => cs.map(([n, t]) => `${n}: ${t}`).join('; ');
    const lineas = [
        '// GENERADO por herramientas/generar_tipos_ipc.mjs desde el Rust de src-tauri/src (main.rs y comandos/). No editar a mano:',
        '// se regenera con `npm run tipos:generar` y una prueba exige que esté al día.',
        '// Solo contiene tipos; no hay código que se ejecute.',
        '',
        '// --- Lo que Rust devuelve ---',
        ...respuestas.map(([n, cs]) => `export type ${n} = { ${campos(cs)} };`),
        '',
        '// --- Lo que Rust recibe en estructuras ---',
        ...entradas.map(([n, cs]) => `export type ${n} = { ${campos(cs)} };`),
        '',
        '// --- Cada comando: sus argumentos (claves en camelCase, como las envía Tauri) y su respuesta ---',
        'export type Comandos = {',
        ...comandos.map(c => {
            const args = c.argumentos.map(a => `${a.clave}${a.opcional ? '?' : ''}: ${a.tipo}`).join('; ');
            return `    ${c.nombre}: { args: { ${args} }; ret: ${c.retorno} };`;
        }),
        '};',
        '',
    ];
    return lineas.join('\n');
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
    const texto = generar(leerFuentesRust());
    if (process.argv.includes('--comprobar')) {
        if (readFileSync(DESTINO, 'utf8') !== texto) {
            console.error('src/js/tipos-ipc.d.ts está desactualizado: ejecuta `npm run tipos:generar`.');
            process.exit(1);
        }
    } else {
        writeFileSync(DESTINO, texto);
        console.log(`escrito ${DESTINO}`);
    }
}
