// Analiza las plantillas HTML de las vistas con el compilador de TypeScript y dice qué
// interpolaciones (`${...}`) llegan al HTML **sin escapar**.
//
// Una plantilla es HTML si lleva etiquetas, si cuelga de una interpolación de otra que
// lo es, o si se asigna a un `innerHTML`. En ella, una interpolación de **texto libre**
// (tipo `string`, no un literal ni un número) es un riesgo salvo que:
//   * ya pase por `escaparHtml(...)` o `argumentoJs(...)`,
//   * sea un fragmento de HTML construido en otra parte (una variable que se inicializa con
//     una plantilla con etiquetas o con un `.join`), o una llamada a un constructor de
//     fragmentos (`.join`, `this.plantilla…`, `formato.importe`, `.toFixed`…).
// Dos contextos más, que `escaparHtml` por sí solo no resuelve:
//   * una **cadena JS dentro de un manejador** (`onclick="f('${x}')"`): el valor tiene que ir
//     como literal con `argumentoJs(x)`, que escapa para JS y para el atributo;
//   * un `JSON.stringify(...)` metido en un atributo, o un escape a mano (`.replace(/'/g…`):
//     tiene que ir con `argumentoJs(...)`.
//
// Lo usa `contrato/escape_html.test.js`. Necesita `typescript` (devDependency) y tarda un par
// de segundos: compila las vistas para conocer el tipo de cada interpolación.

import ts from 'typescript';
import { join } from 'node:path';

const FRAGMENTOS = /^(formato\.importe|escaparHtml|argumentoJs|Number|this\.plantilla\w*|this\.formatMonthYearStr|this\.fechaAIso)$/;
const DEVUELVE_NUMERO_O_FRAGMENTO = /\.(toFixed|toLocaleString|join|reduce|length)$/;

export function analizarPlantillas(raiz) {
    const cfg = ts.readConfigFile(join(raiz, 'tsconfig.build.json'), ts.sys.readFile);
    const parsed = ts.parseJsonConfigFileContent(cfg.config, ts.sys, raiz);
    const programa = ts.createProgram(parsed.fileNames, { ...parsed.options, noEmit: true });
    const checker = programa.getTypeChecker();
    const hallazgos = [];
    const interpolaciones = [];

    /** `string` a secas (texto libre), no un literal como `'selected'` ni un número; o un `any`. */
    const textoLibre = t => {
        // Un `any` (lo que devuelve `JSON.parse`) puede ser texto: se trata como tal, por prudencia.
        if ((t.flags & (ts.TypeFlags.Any | ts.TypeFlags.Unknown)) !== 0) return true;
        const partes = t.isUnion() ? t.types : [t];
        const solo = partes.every(u => (u.flags & (ts.TypeFlags.StringLike | ts.TypeFlags.Null | ts.TypeFlags.Undefined)) !== 0);
        return solo && partes.some(u => (u.flags & ts.TypeFlags.String) !== 0);
    };
    const conEtiquetas = texto => /<[A-Za-z\/!]/.test(texto);
    const estaticoDe = t => [t.head.text, ...t.templateSpans.map(s => s.literal.text)].join('');

    const memo = new Map();
    function esHtml(plantilla) {
        if (memo.has(plantilla)) return memo.get(plantilla);
        let r = conEtiquetas(estaticoDe(plantilla));
        if (!r) {
            // ¿cuelga de una interpolación de una plantilla que sí es HTML?
            for (let n = plantilla.parent; n && !r; n = n.parent) {
                if (ts.isTemplateSpan(n)) r = esHtml(n.parent);
                if (ts.isFunctionLike(n) && !ts.isArrowFunction(n)) break;
            }
        }
        if (!r) {
            // ¿se asigna a un `innerHTML`?
            const p = plantilla.parent;
            r = !!p && ts.isBinaryExpression(p) && p.operatorToken.kind === ts.SyntaxKind.EqualsToken && /\.innerHTML$/.test(p.left.getText());
        }
        memo.set(plantilla, r);
        return r;
    }

    /** ¿Es una variable que contiene un fragmento de HTML ya construido? */
    function esFragmentoVariable(id) {
        const simbolo = checker.getSymbolAtLocation(id);
        const decl = simbolo?.valueDeclaration;
        if (!decl || !ts.isVariableDeclaration(decl)) return false;
        const pareceHtml = texto => conEtiquetas(texto) || /\.join\(/.test(texto);
        if (decl.initializer && pareceHtml(decl.initializer.getText())) return true;
        // Una variable que se va rellenando (`let html = ''; … html = \`<div>…\``) también es un fragmento.
        let hay = false;
        const buscar = n => {
            if (ts.isBinaryExpression(n) && (n.operatorToken.kind === ts.SyntaxKind.EqualsToken || n.operatorToken.kind === ts.SyntaxKind.PlusEqualsToken)
                && ts.isIdentifier(n.left) && checker.getSymbolAtLocation(n.left) === simbolo && pareceHtml(n.right.getText())) hay = true;
            if (!hay) ts.forEachChild(n, buscar);
        };
        buscar(decl.getSourceFile());
        return hay;
    }

    function sinParentesis(e) { while (ts.isParenthesizedExpression(e) || ts.isNonNullExpression(e)) e = e.expression; return e; }

    function clasificar(e) {
        e = sinParentesis(e);
        if (ts.isCallExpression(e)) {
            const callee = e.expression.getText();
            if (FRAGMENTOS.test(callee) || DEVUELVE_NUMERO_O_FRAGMENTO.test(callee)) return 'seguro';
            if (callee === 'JSON.stringify') return 'json';
            if (callee === 'String' || /\.(trim|slice|replace|toUpperCase|toLowerCase|split|padStart|toString)$/.test(callee)) return 'dato';
            return 'fragmento';              // una IIFE u otra llamada que devuelve HTML: sus interpolaciones se miran aparte
        }
        if (ts.isIdentifier(e)) return esFragmentoVariable(e) ? 'fragmento' : 'dato';
        if (ts.isPropertyAccessExpression(e) || ts.isElementAccessExpression(e)) return 'dato';
        if (ts.isBinaryExpression(e)) {
            const op = e.operatorToken.kind;
            if (op === ts.SyntaxKind.BarBarToken || op === ts.SyntaxKind.QuestionQuestionToken) return clasificar(e.left) === 'dato' || clasificar(e.right) === 'dato' ? 'dato' : 'fragmento';
            return 'fragmento';
        }
        if (ts.isConditionalExpression(e)) {
            // Un condicional es un fragmento, salvo que una rama sea un dato suelto.
            const ramas = [e.whenTrue, e.whenFalse].map(sinParentesis);
            return ramas.some(r => !ts.isTemplateExpression(r) && !ts.isNoSubstitutionTemplateLiteral(r) && !ts.isStringLiteral(r) && !ts.isCallExpression(r) && clasificar(r) === 'dato') ? 'dato' : 'fragmento';
        }
        return 'fragmento';
    }

    for (const sf of programa.getSourceFiles()) {
        // Las vistas y los servicios que escriben HTML (avisos, enrutador, diálogos, menús).
        const analizable = sf.fileName.includes('/src/js/vistas/') || sf.fileName.endsWith('/src/js/ui/servicios.ts');
        if (!analizable || sf.fileName.endsWith('registro.ts')) continue;
        const archivo = sf.fileName.split('/').pop();
        const visitar = nodo => {
            if (ts.isTemplateExpression(nodo) && esHtml(nodo)) {
                nodo.templateSpans.forEach((span, i) => {
                    const e = span.expression;
                    const tipo = checker.getTypeAtLocation(e);
                    const antes = i === 0 ? nodo.head.text : nodo.templateSpans[i - 1].literal.text;
                    const despues = span.literal.text;
                    const linea = sf.getLineAndCharacterOfPosition(e.getStart()).line + 1;
                    const enManejador = /\bon\w+="[^"]*$/.test(antes) || /\bon\w+='[^']*$/.test(antes);
                    const enCadenaJs = enManejador && /'$/.test(antes) && /^'/.test(despues);
                    const clase = clasificar(e);
                    const dato = (clase === 'dato' || clase === 'json') && (textoLibre(tipo) || clase === 'json');
                    const info = { archivo, linea, texto: e.getText().replace(/\s+/g, ' ').slice(0, 100), clase, enCadenaJs, inicio: e.getStart(), fin: e.getEnd() };
                    interpolaciones.push(info);
                    if (enCadenaJs && clase !== 'seguro') hallazgos.push({ ...info, motivo: 'cadena JS en un manejador: usa argumentoJs(...) sin las comillas' });
                    else if (clase === 'json') hallazgos.push({ ...info, motivo: 'JSON.stringify en un atributo: usa argumentoJs(...)' });
                    else if (dato && textoLibre(tipo)) hallazgos.push({ ...info, motivo: 'texto libre sin escapar: usa escaparHtml(...)' });
                });
            }
            if (ts.isCallExpression(nodo) && /\.replace$/.test(nodo.expression.getText()) && /&#39;|&apos;|&quot;/.test(nodo.getText())) {
                hallazgos.push({ archivo, linea: sf.getLineAndCharacterOfPosition(nodo.getStart()).line + 1, texto: nodo.getText().slice(0, 100), clase: 'escape-a-mano', motivo: 'escape a mano de comillas: usa escaparHtml/argumentoJs' });
            }
            ts.forEachChild(nodo, visitar);
        };
        visitar(sf);
    }
    return { hallazgos, interpolaciones };
}
