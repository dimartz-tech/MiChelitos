// Punto único de carga de la interfaz para las pruebas de interacción.
//
// Compone la interfaz con **la misma función que ejecuta la aplicación**
// (`componerInterfaz`, de `ui/componer.ts`), pero sobre dobles: un DOM falso, una API
// falsa que registra cada llamada, diálogos que responden lo que la prueba decide,
// ventanas modales y menús que se registran, y un enrutador que solo anota la ruta
// (`renderReal: true` lo deja real). Del DOM real solo se evalúa `ui/dom.js`
// (`elemento` y `buscar`), como lo carga el navegador, sobre el `document` falso.
//
// Las pruebas llaman siempre a `interfaz.appUI.<método>(...)`, que es lo mismo que
// hacen los atributos `onclick="appUI.<método>()"`: el puente que cuelga de
// `window.appUI`. **Añadir una vista no obliga a tocar este archivo ni las pruebas**:
// la registra `vistas/registro.ts`.

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { crearDomFalso, AUSENTE } from './dom_falso.js';
// Lo mismo que ejecuta la aplicación (`composicion.ts`).
import { componerInterfaz } from '../../../src/js/ui/componer.js';
import { crearAvisos } from '../../../src/js/ui/servicios.js';

export { AUSENTE };
export { crearEvento, crearElemento } from './dom_falso.js';

const JS = join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..', 'src', 'js');

/** Los scripts clásicos que sigue cargando `index.html` y las pruebas necesitan, relativos a `src/js`. */
const SCRIPTS = ['ui/dom.js'];

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
    const confirmaciones = [];
    const preguntas = [];
    const temporizadores = [];
    const modalesAbiertos = [];
    const menusAbiertos = [];
    const respuestas = new Map(Object.entries(api));

    // Los contenedores que la composición exige; la prueba puede pisarlos.
    const { document, registro, declarar } = crearDomFalso({
        'app-content': {}, 'notification-container': {}, ...campos,
    });
    const omisiones = registro.omisiones;

    const confirmFalso = crearDialogo('confirm', confirm, confirmaciones, omisiones);
    const promptFalso = crearDialogo('prompt', prompt, preguntas, omisiones);
    const AppAPI = crearApiFalsa(llamadas, respuestas);

    const evaluar = new Function('document', `${SCRIPTS.map(leer).join('\n')}\nreturn { elemento, buscar };`);
    const { elemento: elementoReal, buscar: buscarReal } = evaluar(document);

    // Cada aviso queda registrado, y además se dibuja con el código real de los avisos
    // sobre el DOM falso (los temporizadores no corren: se guardan en `temporizadores`).
    const avisosReales = crearAvisos({
        contenedor: elementoReal('notification-container'),
        crear: () => document.createElement('div'),
        programar: (fn, ms) => { temporizadores.push({ fn, ms }); return temporizadores.length; },
    });
    const avisosObservados = {
        mostrar(mensaje, tipo = 'success') {
            avisos.push({ tipo, mensaje: String(mensaje) });
            avisosReales.mostrar(mensaje, tipo);
        },
    };
    const enrutadorObservado = { mostrar: async ruta => { renders.push(ruta); } };

    const { appUI: puente, servicios, enrutador, vistas } = componerInterfaz(
        {
            dom: { elemento: elementoReal, buscar: buscarReal },
            contenido: elementoReal('app-content'),
            avisos: { contenedor: elementoReal('notification-container') },
            dialogos: { confirmar: m => confirmFalso(m), preguntar: (m, d) => promptFalso(m, d) },
            // Una ventana modal abierta queda registrada y existe en el DOM falso con su
            // contenido, para que la vista (o la prueba) la encuentre y la cierre por id.
            modales: { abrir: (id, html) => { modalesAbiertos.push({ id, html }); declarar(id, { innerHTML: html }); } },
            // Un menú flotante abierto queda registrado: `alElegir(accion)` hace lo que el clic en su renglón.
            menus: {
                abrir: opciones => { menusAbiertos.push({ ...opciones, abierto: true }); },
                cerrar: () => { for (const m of menusAbiertos) m.abierto = false; },
            },
            sustituir: { avisos: avisosObservados, ...(renderReal ? {} : { enrutador: enrutadorObservado }) },
        },
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
    const appUI = new Proxy(puente, {
        get(destino, nombre, receptor) {
            const valor = Reflect.get(destino, nombre, receptor);
            if (typeof valor !== 'function') return valor;
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
        /** El puente de manejadores, como lo ve el HTML (`window.appUI`). */
        appUI,
        /** Los servicios que ven las vistas (con los dobles de esta carga). */
        servicios,
        /** El enrutador **real**, aunque las vistas vean uno que solo anota la ruta. */
        enrutador,
        /** Las vistas registradas, por ruta. */
        vistas,
        /** El doble de `AppAPI` que ve la interfaz (solo los métodos reales de api.ts). */
        api: AppAPI,
        llamadas,
        avisos,
        renders,
        confirmaciones,
        preguntas,
        temporizadores,
        /** Las ventanas modales que abrieron las vistas: `{ id, html }`. */
        modalesAbiertos,
        /** Los menús flotantes que abrieron las vistas: `{ id, html, ancla, alElegir, abierto }`. */
        menusAbiertos,
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
