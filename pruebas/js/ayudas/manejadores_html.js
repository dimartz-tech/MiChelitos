// Ejecuta los manejadores en línea de un HTML **como lo haría el navegador**, para comprobar
// con qué argumentos se llamaría a `appUI.<método>(…)` sin depender de cómo esté escrito el
// atributo.
//
// Un atributo `onclick="appUI.abrir(&quot;a&amp;b&quot;)"` llega al navegador como texto HTML:
// primero se deshacen las entidades (`&quot;` → `"`) y el resultado se ejecuta como JavaScript.
// Una prueba que mire la **cadena** (`/abrir\("a&b"\)/`) fija un formato; esta mira el
// **efecto**: qué argumentos recibe el método. Es lo que protege de que un `'` o un `\` dentro
// de un nombre cierren una cadena del manejador y ejecuten lo que venga detrás.

const ENTIDADES = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'", '#39': "'" };

/** Deshace las entidades HTML de un valor de atributo (las que escribe `escaparHtml`). */
export function decodificarAtributo(valor) {
    return valor.replace(/&(#\d+|#x[0-9a-f]+|[a-z]+);/gi, (m, e) => {
        if (e in ENTIDADES) return ENTIDADES[e];
        if (e[0] === '#') return String.fromCodePoint(e[1] === 'x' ? parseInt(e.slice(2), 16) : parseInt(e.slice(1), 10));
        return m;
    });
}

/** Todos los atributos `on…="…"` del HTML, ya decodificados, como el navegador los ejecutaría. */
export function manejadoresDe(html) {
    return [...html.matchAll(/\bon[a-z]+="([^"]*)"/g)].map(m => decodificarAtributo(m[1]));
}

/**
 * Ejecuta cada manejador que llame a `appUI.<metodo>(…)` con un `appUI` que solo anota las
 * llamadas, y devuelve los argumentos de cada una (un array por llamada). Si un manejador no es
 * JavaScript válido tras decodificarlo, **lanza**: es justo lo que pasaría en el navegador.
 */
export function llamadasDelManejador(html, metodo) {
    const llamadas = [];
    for (const codigo of manejadoresDe(html)) {
        if (!new RegExp(`\\bappUI\\.${metodo}\\(`).test(codigo)) continue;
        const appUI = new Proxy({}, { get: (_, nombre) => (...args) => { if (nombre === metodo) llamadas.push(args); } });
        new Function('appUI', 'event', 'elemento', codigo)(appUI, { stopPropagation() {}, currentTarget: null }, () => ({ remove() {} }));
    }
    return llamadas;
}
