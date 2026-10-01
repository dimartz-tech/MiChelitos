// DOM falso mínimo para probar los manejadores de `ui.ts` sin navegador.
//
// No es jsdom ni se le parece: es lo justo para que `elemento('id').value`,
// `buscar('id')?.checked`, `.remove()`, `.hidden`, `.dataset`, `.style`… hagan
// lo que el manejador espera. No hay CSS, ni layout, ni eventos que viajen, y
// `innerHTML` es una cadena opaca.
//
// Regla central: **un id que el manejador lea y la prueba no declaró no da un
// valor vacío silencioso: falla** con un mensaje que nombra el id. Así cada
// prueba documenta lo que lee su manejador. Los ids que deben estar ausentes
// (porque el manejador usa `buscar` y la ausencia es un caso real) se
// declaran con `AUSENTE`.

/** Marca un id como «no existe en la página» (`getElementById` devuelve null). */
export const AUSENTE = Symbol('ausente');

class ClassListFalsa {
    #clases = new Set();
    add(...c) { c.forEach(x => this.#clases.add(x)); }
    remove(...c) { c.forEach(x => this.#clases.delete(x)); }
    contains(c) { return this.#clases.has(c); }
    toggle(c, forzar) {
        const poner = forzar ?? !this.#clases.has(c);
        if (poner) this.#clases.add(c); else this.#clases.delete(c);
        return poner;
    }
    toString() { return [...this.#clases].join(' '); }
}

/** Una `<option>`: texto, o `{ value, text, dataset, attrs }`. */
function crearOpcion(spec) {
    const o = typeof spec === 'string' ? { value: spec, text: spec } : spec;
    const opcion = crearElemento('option');
    opcion.value = String(o.value ?? '');
    opcion.text = o.text ?? opcion.value;
    opcion.textContent = opcion.text;
    Object.assign(opcion.dataset, o.dataset ?? {});
    opcion.attrs = { ...(o.attrs ?? {}) };
    return opcion;
}

/**
 * Un elemento suelto. Las propiedades de `props` (`value`, `checked`,
 * `hidden`, `dataset`, `options`, `selectedIndex`…) se aplican encima.
 * Con `options`, `value` sigue a la opción elegida, como un `<select>`.
 */
export function crearElemento(etiqueta = 'div', props = {}, registro = null) {
    const hijos = [];
    let valor = '';
    let valorFijado = false;
    const el = {
        tagName: String(etiqueta).toUpperCase(),
        id: '',
        className: '',
        textContent: '',
        innerHTML: '',
        hidden: false,
        disabled: false,
        readOnly: false,
        checked: false,
        text: '',
        options: [],
        selectedIndex: -1,
        style: {},
        dataset: {},
        classList: new ClassListFalsa(),
        attrs: {},
        eliminado: false,
        enfocado: false,
        oyentes: [],
        hijos,
        get value() {
            if (!valorFijado && this.options.length > 0 && this.selectedIndex >= 0) {
                return this.options[this.selectedIndex]?.value ?? '';
            }
            return valor;
        },
        set value(v) {
            valor = String(v);
            valorFijado = true;
            // Un <select> real, al recibir un valor, elige la opción que lo tiene.
            const i = this.options.findIndex(o => o.value === valor);
            if (i >= 0) { this.selectedIndex = i; valorFijado = false; }
        },
        get selectedOptions() {
            const o = this.options[this.selectedIndex];
            return o ? [o] : [];
        },
        getAttribute(nombre) { return this.attrs[nombre] ?? null; },
        setAttribute(nombre, v) { this.attrs[nombre] = String(v); },
        remove() { this.eliminado = true; registro?.alEliminar(this); },
        focus() { this.enfocado = true; },
        addEventListener(tipo, fn) { this.oyentes.push({ tipo, fn }); },
        removeEventListener() {},
        appendChild(hijo) { hijos.push(hijo); registro?.alAnadir(hijo); return hijo; },
        querySelector(sel) { return registro?.buscarPorSelector(sel) ?? null; },
        querySelectorAll(sel) { const r = registro?.buscarPorSelector(sel); return r ? [r] : []; },
    };
    const { options, value, dataset, selectedIndex, ...otros } = props;
    if (options) {
        el.options = options.map(crearOpcion);
        el.selectedIndex = selectedIndex ?? (el.options.length > 0 ? 0 : -1);
    }
    if (value !== undefined) el.value = value;
    Object.assign(el.dataset, dataset ?? {});
    Object.assign(el, otros);
    return el;
}

/**
 * Construye `document` a partir de `campos`: `{ id: spec }` donde `spec` es
 *   - texto o número → `{ value }`
 *   - booleano       → `{ checked }`
 *   - objeto         → propiedades del elemento (`value`, `checked`, `options`…)
 *   - `AUSENTE`      → el id no existe
 * Devuelve `{ document, registro, declarar }`; `registro.omisiones` lista cada
 * id leído sin declarar (y cada diálogo sin configurar, ver `cargar_interfaz.js`).
 */
export function crearDomFalso(campos = {}) {
    const ids = new Map();      // id → elemento | null (ausente)
    const eliminados = [];
    const anadidos = [];
    const omisiones = [];

    const registro = {
        omisiones, eliminados, anadidos,
        alEliminar(el) { if (el.id) eliminados.push(el.id); },
        alAnadir(hijo) { anadidos.push(hijo); if (hijo.id) ids.set(hijo.id, hijo); },
        buscarPorSelector(sel) {
            const m = /^#([\w-]+)$/.exec(sel);
            return m ? (ids.get(m[1]) ?? null) : null;
        },
    };

    function declarar(id, spec) {
        if (spec === AUSENTE) { ids.set(id, null); return; }
        let props;
        if (spec !== null && typeof spec === 'object') props = spec;
        else if (typeof spec === 'boolean') props = { checked: spec };
        else props = { value: String(spec) };
        const el = crearElemento('div', props, registro);
        el.id = id;
        ids.set(id, el);
    }

    for (const [id, spec] of Object.entries(campos)) declarar(id, spec);

    const body = crearElemento('body', {}, registro);
    const documento = {
        body,
        getElementById(id) {
            if (!ids.has(id)) {
                const msg = `La prueba no declaró el elemento #${id}, pero el manejador lo lee. ` +
                    `Decláralo en «campos» (con su valor) o como AUSENTE si debe no existir.`;
                omisiones.push(msg);
                throw new Error(msg);
            }
            return ids.get(id);
        },
        createElement: etiqueta => crearElemento(etiqueta, {}, registro),
        querySelector: sel => registro.buscarPorSelector(sel),
        querySelectorAll: sel => { const r = registro.buscarPorSelector(sel); return r ? [r] : []; },
        addEventListener() {},
    };
    return { document: documento, registro, declarar };
}

/** El `evento` que reciben los `onsubmit`: solo importa que se llame a `preventDefault`. */
export function crearEvento() {
    return { prevenido: false, preventDefault() { this.prevenido = true; } };
}
