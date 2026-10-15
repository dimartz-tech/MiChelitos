// Lee los manejadores delegados de un HTML **como los leería el oyente de la aplicación**
// (`ui/delegacion.ts`), para comprobar con qué argumentos se llamaría a la acción sin depender de
// cómo esté escrito el atributo.
//
// Un `data-args="[&quot;a&amp;b&quot;]"` llega al navegador como texto HTML: primero se deshacen
// las entidades (`&quot;` → `"`) y el resultado se interpreta como JSON. Una prueba que mire la
// **cadena** (`/data-args="\[1\]"/`) fija un formato; esta mira el **efecto**: qué argumentos
// recibe la acción. Es lo que protege de que un `'` o un `\` dentro de un nombre rompan el
// atributo o lleguen alterados.
//
// Antes (hasta la 1.106.0) los manejadores eran atributos `onclick="appUI.f(…)"` que esta ayuda
// ejecutaba como JavaScript; la interfaz ya no tiene ninguno.

import { ATRIBUTO_DE_ACCION } from '../../../src/js/nucleo/html.js';
import { accionesGenerales } from '../../../src/js/ui/delegacion.js';

const ENTIDADES = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'", '#39': "'" };

/** Deshace las entidades HTML de un valor de atributo (las que escribe `escaparHtml`). */
export function decodificarAtributo(valor) {
    return valor.replace(/&(#\d+|#x[0-9a-f]+|[a-z]+);/gi, (m, e) => {
        if (e in ENTIDADES) return ENTIDADES[e];
        if (e[0] === '#') return String.fromCodePoint(e[1] === 'x' ? parseInt(e.slice(2), 16) : parseInt(e.slice(1), 10));
        return m;
    });
}

/** Qué recibe la acción antes de sus argumentos, en las llamadas que devuelve `llamadasDelManejador`. */
const MARCA = { evento: 'EVENTO', elemento: 'ELEMENTO', valor: 'VALOR' };

/** Etiquetas (`<…>`) del HTML, con sus atributos sin interpretar; un `>` dentro de un valor entrecomillado no la cierra. */
const ETIQUETAS = /<[A-Za-z][^\s>/]*(?:[^>"']|"[^"]*"|'[^']*')*>/g;
const ATRIBUTOS = /([a-zA-Z][\w:-]*)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'))?/g;

/** Los atributos de una etiqueta, ya sin entidades. */
function atributosDe(etiqueta) {
    const resultado = new Map();
    for (const m of etiqueta.replace(/^<[^\s>]+/, '').matchAll(ATRIBUTOS)) {
        resultado.set(m[1], decodificarAtributo(m[2] ?? m[3] ?? ''));
    }
    return resultado;
}

/**
 * Todos los manejadores delegados del HTML: `{ evento, nombre, args, pasa }`, con los argumentos
 * ya interpretados. **Lanza** si `data-args` no es un arreglo JSON: es justo lo que pasaría en la aplicación.
 */
export function accionesDe(html) {
    const hallazgos = [];
    for (const [etiqueta] of html.matchAll(ETIQUETAS)) {
        const atributos = atributosDe(etiqueta);
        for (const [evento, atributo] of Object.entries(ATRIBUTO_DE_ACCION)) {
            if (!atributos.has(atributo)) continue;
            const args = atributos.has('data-args') ? JSON.parse(atributos.get('data-args')) : [];
            if (!Array.isArray(args)) throw new Error(`data-args no es un arreglo: ${atributos.get('data-args')}`);
            hallazgos.push({ evento, nombre: atributos.get(atributo), args, pasa: atributos.get('data-pasa') ?? (evento === 'submit' ? 'evento' : null) });
        }
    }
    return hallazgos;
}

/** Las acciones que no son de ninguna vista (navegar, cerrar una ventana, alternar pestañas). */
export const ACCIONES_GENERALES = new Set(Object.keys(accionesGenerales({ dom: {}, navegar() {} })));

/** Los nombres de acción distintos que escribe el HTML. */
export const nombresDeAccion = html => new Set(accionesDe(html).map(a => a.nombre));

/** Los nombres de acción **de las vistas** (los que tiene que exponer el puente `appUI`): sin las generales. */
export const nombresDelPuente = html => new Set([...nombresDeAccion(html)].filter(n => !ACCIONES_GENERALES.has(n)));

/**
 * Los argumentos de cada llamada a `metodo` que declara el HTML (un arreglo por llamada). Si la
 * acción recibe antes el evento, el elemento o el valor, va primero como `'EVENTO'`, `'ELEMENTO'`
 * o `'VALOR'`.
 */
export function llamadasDelManejador(html, metodo) {
    return accionesDe(html)
        .filter(a => a.nombre === metodo)
        .map(a => [...(a.pasa ? [MARCA[a.pasa]] : []), ...a.args]);
}
