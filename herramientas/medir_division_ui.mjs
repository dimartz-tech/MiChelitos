// Mide `src/js/ui.ts` para la división por pestañas (ver division_de_ui_limpia.md).
//
// Uso:  node herramientas/medir_division_ui.mjs [ruta/ui.ts] [--json]
//
// Usa el compilador de TypeScript (devDependency) para leer la clase `AppUI`,
// construye el grafo de llamadas entre métodos (`this.metodo(...)` y los
// `appUI.metodo(...)` de los manejadores en línea que el método escribe) y
// asigna cada método a la pestaña cuyo `render*` lo alcanza. No ejecuta nada
// de la aplicación ni lee datos: solo el texto del fuente.

import ts from 'typescript';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const ruta = args.find(a => !a.startsWith('--')) ?? join(RAIZ, 'src', 'js', 'ui.ts');
const comoJson = args.includes('--json');

const VISTAS = ['dashboard', 'ingresos', 'gastos', 'tarjetas', 'cuentas', 'efectivo',
    'suscripciones', 'capital', 'prestamos', 'resumen', 'ajustes'];
const COMPARTIDOS = ['render', 'showToast', 'formatMoney', 'pedirMotivoDeCorreccion'];
const ESTADO = ['selectedGastosMonth', 'contentContainer', 'notifContainer', '_menuPasivoAbort'];
const raiz = v => 'render' + v[0].toUpperCase() + v.slice(1);

const texto = readFileSync(ruta, 'utf8');
const sf = ts.createSourceFile(ruta, texto, ts.ScriptTarget.ES2022, true);
const clase = sf.statements.find(s => ts.isClassDeclaration(s) && s.name?.text === 'AppUI');
const metodos = new Map(); // nombre -> { nombre, ini, fin, lineas, texto, vista }
for (const m of clase.members) {
    if (!ts.isMethodDeclaration(m)) continue;
    // Las líneas incluyen los comentarios y la línea en blanco que preceden al método
    // (de ahí salen las cifras de division_de_ui.md); `lineasCodigo` es solo el método.
    const ini = m.getStart(sf, true);
    const l0 = sf.getLineAndCharacterOfPosition(ini).line;
    const l1 = sf.getLineAndCharacterOfPosition(m.end).line;
    const lf = sf.getLineAndCharacterOfPosition(m.getFullStart()).line;
    metodos.set(m.name.getText(sf), { nombre: m.name.getText(sf), nodo: m, lineas: l1 - lf, lineasCodigo: l1 - l0 + 1, l0, texto: texto.slice(ini, m.end) });
}

// --- referencias `this.x` (por AST) y `appUI.x` (por texto, dentro de plantillas) ---
function refsThis(nodo) {
    const r = [];
    (function visitar(n) {
        if (ts.isPropertyAccessExpression(n) && n.expression.kind === ts.SyntaxKind.ThisKeyword) r.push(n.name.text);
        ts.forEachChild(n, visitar);
    })(nodo);
    return r;
}
for (const m of metodos.values()) {
    m.this = refsThis(m.nodo);
    m.appUI = [...m.texto.matchAll(/\bappUI\.(\w+)/g)].map(x => x[1]);
}

// --- asignación por alcanzabilidad desde cada render*, sin atravesar lo compartido ---
const alcanza = new Map();
for (const v of VISTAS) {
    const visto = new Set();
    const pila = [raiz(v)];
    while (pila.length) {
        const n = pila.pop();
        if (visto.has(n) || COMPARTIDOS.includes(n) || !metodos.has(n)) continue;
        visto.add(n);
        const m = metodos.get(n);
        for (const d of [...m.this, ...m.appUI]) pila.push(d);
    }
    alcanza.set(v, visto);
}
const duenos = new Map();
for (const [v, s] of alcanza) for (const n of s) duenos.set(n, [...(duenos.get(n) ?? []), v]);
const compartidosAjenos = [...metodos.keys()].filter(n => !duenos.has(n) && !COMPARTIDOS.includes(n));
const multiples = [...duenos].filter(([, vs]) => vs.length > 1);
for (const m of metodos.values()) m.vista = duenos.get(m.nombre)?.[0] ?? (COMPARTIDOS.includes(m.nombre) ? '(compartido)' : '(huérfano)');

// --- métricas por vista ---
// Un «sitio a reescribir» en B es una LÍNEA del cuerpo que toca algo que B
// inyecta. B-mínimo: lo compartido y el estado (`this.showToast`, `this.render`,
// `this.contentContainer`…). B-total: además la API y el DOM
// (`AppAPI.x`, `elemento()`, `buscar()`, `document.x`) y `navigate(`.
const RE_COMP = new RegExp(`\\bthis\\.(?:${[...COMPARTIDOS, ...ESTADO].join('|')})\\b`);
const RE_EXT = /\bAppAPI\.|\belemento(?:<[^>]*>)?\(|\bbuscar(?:<[^>]*>)?\(|\bdocument\.|\bnavigate\(|\blocalStorage\b|\bwindow\./;
const filas = [];
for (const v of [...VISTAS, '(compartido)']) {
    const ms = [...metodos.values()].filter(m => m.vista === v);
    const f = { vista: v, metodos: ms.length, lineas: ms.reduce((s, m) => s + m.lineas, 0),
        a: 0, b: 0, c: 0, d: 0, e: 0, dialogos: 0, fechas: 0, docGlobal: 0, json: 0, handlers: 0, handlerRefs: 0, handlersDOM: 0, dom: 0, api: 0, navigate: 0,
        bMinLineas: 0, bMinCuerpos: 0, bTotLineas: 0, bTotCuerpos: 0, detalleD: [], detalleE: new Set(),
        detalleEstado: {}, detalleCompartido: {} };
    const propios = new Set(ms.map(m => m.nombre));
    for (const m of ms) {
        for (const r of m.this) {
            if (propios.has(r)) f.a++;
            else if (COMPARTIDOS.includes(r)) { f.b++; f.detalleCompartido[r] = (f.detalleCompartido[r] ?? 0) + 1; }
            else if (ESTADO.includes(r)) { f.c++; f.detalleEstado[r] = (f.detalleEstado[r] ?? 0) + 1; }
            else if (metodos.has(r)) { f.d++; f.detalleD.push(`${m.nombre}->${r}`); }
            else { f.e++; f.detalleE.add(r); }
        }
        const atrs = [...m.texto.matchAll(/\bon(?:click|submit|input|change|keyup|keydown|blur|focus|mouseenter|mouseleave)\s*=\s*(?:"([^"]*)"|'([^']*)')/g)].map(x => [x[0], x[1] ?? x[2]]);
        f.handlers += atrs.length;
        f.handlerRefs += atrs.reduce((s, x) => s + [...x[1].matchAll(/\bappUI\./g)].length, 0);
        f.handlersDOM += atrs.filter(x => /\b(elemento|buscar)\(/.test(x[1])).length;
        f.dom += [...m.texto.matchAll(/\b(?:elemento|buscar)(?:<[^>]*>)?\(/g)].length;
        f.api += [...m.texto.matchAll(/\bAppAPI\./g)].length;
        f.dialogos += [...m.texto.matchAll(/\b(?:confirm|prompt)\(/g)].length;
        f.fechas += [...m.texto.matchAll(/\bnew Date\b/g)].length;
        f.docGlobal += [...m.texto.matchAll(/\b(?:document|window)\./g)].length;
        f.json += [...m.texto.matchAll(/on\w+='[^']*JSON\.stringify/g)].length;
        f.navigate += [...m.texto.matchAll(/\bnavigate\(/g)].length;
        // líneas a reescribir: solo se cuentan las del código, no las de las plantillas
        // de atributos (esas son del cableado, se miden aparte).
        const lineas = m.texto.split('\n');
        let min = 0, tot = 0;
        for (const l of lineas) {
            const c = RE_COMP.test(l);
            const x = RE_EXT.test(l.replace(/\bon\w+="[^"]*"/g, ''));
            if (c) min++;
            if (c || x) tot++;
        }
        f.bMinLineas += min; f.bTotLineas += tot;
        if (min > 0) f.bMinCuerpos++;
        if (tot > 0) f.bTotCuerpos++;
    }
    f.detalleE = [...f.detalleE];
    filas.push(f);
}
const suma = k => filas.reduce((s, f) => s + f[k], 0);

if (comoJson) {
    console.log(JSON.stringify({ filas, multiples, compartidosAjenos, totalMetodos: metodos.size }, null, 2));
} else {
    console.log(`Archivo: ${ruta}`);
    console.log(`Métodos: ${metodos.size}; asignados a una vista: ${duenos.size}; compartidos: ${COMPARTIDOS.length}; huérfanos: ${compartidosAjenos.length} ${compartidosAjenos.join(',')}`);
    console.log(`Métodos alcanzados desde más de una vista: ${multiples.length} ${multiples.map(([n, v]) => `${n}[${v}]`).join(' ')}`);
    console.log('');
    const cols = ['vista', 'metodos', 'lineas', 'a', 'b', 'c', 'd', 'e', 'handlers', 'handlerRefs', 'handlersDOM', 'dom', 'api', 'navigate', 'dialogos', 'fechas', 'docGlobal', 'json', 'bMinCuerpos', 'bMinLineas', 'bTotCuerpos', 'bTotLineas'];
    console.log(cols.join('\t'));
    for (const f of filas) console.log(cols.map(c => f[c]).join('\t'));
    console.log(['TOTAL', ...cols.slice(1).map(suma)].join('\t'));
    console.log('');
    for (const f of filas) {
        if (f.d || f.e) console.log(`${f.vista}: d=${f.detalleD.join(',')} e=${f.detalleE.join(',')}`);
    }
    console.log('');
    for (const f of filas) console.log(`${f.vista}: compartido ${JSON.stringify(f.detalleCompartido)} estado ${JSON.stringify(f.detalleEstado)}`);
}
