// Punto único de carga de la interfaz para las pruebas de interacción.
//
// Evalúa el JavaScript **compilado** de la interfaz (`npm test` compila antes)
// como lo hace el navegador, con `document`, `window`, `AppAPI`, `confirm`,
// `prompt`, `localStorage`, `navigate` y `setTimeout` falsos, y devuelve el
// `appUI` resultante más los registros de lo ocurrido.
//
// Las pruebas llaman siempre a `interfaz.appUI.<método>(...)`, que es lo mismo
// que hacen los atributos `onclick="appUI.<método>()"`. Las vistas extraídas de
// `ui.ts` (`src/js/vistas/`) se registran aquí con `registrarVistas`, la misma
// función que usa `composicion.ts`, así que **añadir una vista no obliga a tocar
// este archivo ni las pruebas**.

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { crearDomFalso, AUSENTE } from './dom_falso.js';
// Lo mismo que ejecuta la aplicación (`composicion.ts`): los servicios reales
// sobre dobles, y el registro de las vistas extraídas.
import { serviciosDesdeAppUI } from '../../../src/js/ui/servicios.js';
import { registrarVistas } from '../../../src/js/vistas/registro.js';

export { AUSENTE };
export { crearEvento, crearElemento } from './dom_falso.js';

const JS = join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..', 'src', 'js');

/** Scripts clásicos, en el orden de `index.html`, relativos a `src/js`. */
const SCRIPTS = ['nucleo/respaldos.js', 'ui/dom.js', 'ui.js'];

const leer = ruta => readFileSync(join(JS, ruta), 'utf8');

/** Los métodos que de verdad expone `AppAPI` (se evalúa `api.js` con una ventana vacía). */
function metodosDeLaApi() {
    const AppAPI = new Function('window', `${leer('api.js')}\nreturn AppAPI;`)({});
    return Object.keys(AppAPI);
}

/**
 * Doble de `AppAPI`: solo existen los métodos reales (llamar a uno que no
 * existe falla como en la aplicación), cada llamada queda registrada y la
 * respuesta la decide la prueba.
 */
function crearApiFalsa(llamadas, respuestas) {
    const api = {};
    for (const metodo of metodosDeLaApi()) {
        api[metodo] = async (...args) => {
            llamadas.push({ metodo, args });
            const r = respuestas.get(metodo);
            return typeof r === 'function' ? r(...args) : r;
        };
    }
    return api;
}

/** Normaliza una respuesta de diálogo (valor, función o cola de valores). */
function crearDialogo(nombre, config, registro, omisiones) {
    const cola = Array.isArray(config) ? [...config] : null;
    return (texto, ...resto) => {
        registro.push({ texto: String(texto), resto });
        let r;
        if (cola) {
            if (cola.length === 0) r = undefined;
            else r = cola.shift();
        } else r = config;
        if (typeof r === 'function') r = r(String(texto), ...resto);
        if (r === undefined) {
            const msg = `La prueba no configuró ${nombre}(), pero el manejador lo invoca. ` +
                `Pásalo en cargarInterfaz({ ${nombre}: ... }).`;
            omisiones.push(msg);
            throw new Error(msg);
        }
        return r;
    };
}

/**
 * @param {object} [opciones]
 * @param {Record<string, any>} [opciones.campos]  elementos del DOM por id (ver `dom_falso.js`)
 * @param {Record<string, any>} [opciones.api]     respuestas de AppAPI por método (valor o función)
 * @param {boolean|Function|any[]} [opciones.confirm]  respuesta de `confirm()`
 * @param {string|null|Function|any[]} [opciones.prompt] respuesta de `prompt()` (`null` = cancelar)
 * @param {boolean} [opciones.renderReal]  no sustituir `render()` (por defecto solo se registra la ruta)
 */
export function cargarInterfaz({ campos = {}, api = {}, confirm, prompt, renderReal = false } = {}) {
    const llamadas = [];
    const avisos = [];
    const renders = [];
    const navegaciones = [];
    const confirmaciones = [];
    const preguntas = [];
    const temporizadores = [];
    const modalesAbiertos = [];
    const respuestas = new Map(Object.entries(api));

    // Los contenedores que el constructor exige; la prueba puede pisarlos.
    const { document, registro, declarar } = crearDomFalso({
        'app-content': {}, 'notification-container': {}, ...campos,
    });
    const omisiones = registro.omisiones;

    const almacen = new Map();
    const localStorage = {
        getItem: k => (almacen.has(k) ? almacen.get(k) : null),
        setItem: (k, v) => { almacen.set(k, String(v)); },
        removeItem: k => { almacen.delete(k); },
    };
    const confirmFalso = crearDialogo('confirm', confirm, confirmaciones, omisiones);
    const promptFalso = crearDialogo('prompt', prompt, preguntas, omisiones);
    const setTimeoutFalso = (fn, ms) => { temporizadores.push({ fn, ms }); return temporizadores.length; };
    const window = { localStorage, confirm: confirmFalso, prompt: promptFalso, __TAURI__: undefined };
    const AppAPI = crearApiFalsa(llamadas, respuestas);
    const navigate = ruta => { navegaciones.push(ruta); };

    const fuente = SCRIPTS.map(leer).join('\n');
    const evaluar = new Function(
        'document', 'window', 'AppAPI', 'confirm', 'prompt', 'localStorage', 'navigate', 'setTimeout',
        `${fuente}\nreturn { appUI, elemento, buscar };`,
    );
    const { appUI: objetivo, elemento: elementoReal, buscar: buscarReal } =
        evaluar(document, window, AppAPI, confirmFalso, promptFalso, localStorage, navigate, setTimeoutFalso);

    // Cada aviso queda registrado, y además se muestra con el código real.
    const showToastReal = objetivo.showToast;
    objetivo.showToast = function (mensaje, tipo = 'success') {
        avisos.push({ tipo, mensaje: String(mensaje) });
        return showToastReal.call(this, mensaje, tipo);
    };
    if (!renderReal) {
        objetivo.render = async function (ruta) { renders.push(ruta); };
    }

    // Las vistas extraídas, con los servicios reales sobre los dobles de esta carga.
    // `showToast` y `render` ya están observados, y los servicios los llaman a
    // través de `objetivo`, así que cada aviso y cada ruta quedan registrados.
    registrarVistas(
        objetivo,
        serviciosDesdeAppUI(
            objetivo,
            { elemento: elementoReal, buscar: buscarReal },
            () => new Date(),
            { confirmar: m => confirmFalso(m), preguntar: (m, d) => promptFalso(m, d) },
            // Una ventana modal abierta queda registrada y existe en el DOM falso con su
            // contenido, para que la vista (o la prueba) la encuentre y la cierre por id.
            { abrir: (id, html) => { modalesAbiertos.push({ id, html }); declarar(id, { innerHTML: html }); } },
        ),
        AppAPI,
    );

    // Un id leído sin declarar lo atraparía el `try/catch` del manejador y lo
    // convertiría en un aviso de error: el proxy lo convierte en fallo de la
    // prueba al terminar la llamada, con el mensaje que nombra el id.
    const comprobarOmisiones = () => {
        if (omisiones.length > 0) {
            throw new Error(`Prueba mal configurada:\n  - ${[...new Set(omisiones)].join('\n  - ')}`);
        }
    };
    const appUI = new Proxy(objetivo, {
        get(destino, nombre, receptor) {
            const valor = Reflect.get(destino, nombre, receptor);
            if (typeof valor !== 'function' || nombre === 'constructor') return valor;
            return (...args) => {
                const resultado = valor.apply(receptor, args);
                if (resultado && typeof resultado.then === 'function') {
                    return resultado.then(
                        v => { comprobarOmisiones(); return v; },
                        e => { comprobarOmisiones(); throw e; },
                    );
                }
                comprobarOmisiones();
                return resultado;
            };
        },
    });

    return {
        appUI,
        /** El doble de `AppAPI` que ve la interfaz (solo los métodos reales de api.ts). */
        api: AppAPI,
        llamadas,
        avisos,
        renders,
        navegaciones,
        confirmaciones,
        preguntas,
        temporizadores,
        /** Las ventanas modales que abrieron las vistas extraídas: `{ id, html }`. */
        modalesAbiertos,
        dom: { document, registro, declarar },
        /** Los elementos que `remove()` quitó, por id (modales cerrados). */
        get eliminados() { return registro.eliminados; },
        /** Fija o cambia el valor de un campo después de cargar. */
        fijar(id, spec) { declarar(id, spec); },
        /** Elemento por id, tal como lo ve el manejador. */
        elemento(id) { return document.getElementById(id); },
        /** Cambia la respuesta de un método de la API. */
        responder(metodo, valorOFuncion) { respuestas.set(metodo, valorOFuncion); },
        /** Hace que el método rechace con `error` (la API de Tauri rechaza con texto). */
        rechazar(metodo, error) { respuestas.set(metodo, () => Promise.reject(error)); },
        llamadasA(metodo) { return llamadas.filter(l => l.metodo === metodo); },
        /** Resumen `metodo(args)` de todo lo enviado, para mensajes de fallo. */
        get nombresLlamados() { return llamadas.map(l => l.metodo); },
        avisosDeError() { return avisos.filter(a => a.tipo === 'error'); },
    };
}
