// Delegación de eventos: lo que sustituye a los atributos `onclick`/`onsubmit`/`onchange`/
// `oninput` en línea.
//
// Una plantilla declara datos (`data-accion`, `data-args`…, ver `nucleo/html.ts`) y
// **un solo oyente por tipo de evento**, colgado de `document`, busca hacia arriba desde el
// elemento tocado el que lleve el atributo y llama a la acción con ese nombre. Así la interfaz
// no tiene código dentro del marcado y la política de contenido puede prohibirlo; además los
// elementos que las vistas dibujan después (pestañas, ventanas modales) funcionan sin volver a
// enganchar nada.
//
// Se conserva el comportamiento del atributo en línea:
// * el evento **burbujea**: si dos elementos anidados llevan acción, se llaman de dentro a fuera,
//   salvo que una acción detenga la propagación (`stopPropagation`);
// * una acción que no existe falla **en voz alta** (`console.error`), no en silencio;
// * `submit` llama a `preventDefault` antes de la acción, que recibe el evento como primer argumento.

import { ATRIBUTO_DE_ACCION, type EventoDelegado } from '../nucleo/html.js';
import type { Dom } from './servicios.js';

/** Una acción del puente: lo que `window.appUI` (u otra tabla) expone con un nombre. */
export type Accion = (...args: never[]) => unknown;
export type Acciones = Readonly<Record<string, Accion>>;

/** Lo único que se necesita de `document`: poder escuchar eventos. */
export interface RaizDeEventos {
    addEventListener(tipo: string, oyente: (evento: Event) => void): void;
}

/** Lo que se lee de un elemento: sus atributos, su valor y su padre. */
interface ElementoDelegable {
    getAttribute(nombre: string): string | null;
    parentElement: ElementoDelegable | null;
    value?: unknown;
}

type Consola = Pick<Console, 'error'>;

/** Los argumentos de `data-args`: un arreglo JSON. Otra cosa es un error de la plantilla. */
function leerArgumentos(texto: string | null): unknown[] {
    if (texto === null) return [];
    const valor: unknown = JSON.parse(texto);
    if (!Array.isArray(valor)) throw new Error('data-args debe ser un arreglo JSON');
    return valor;
}

function atender(evento: Event, tipo: EventoDelegado, acciones: Acciones, consola: Consola): void {
    const atributo = ATRIBUTO_DE_ACCION[tipo];
    let el = evento.target as unknown as ElementoDelegable | null;
    for (; el && typeof el.getAttribute === 'function'; el = el.parentElement) {
        const nombre = el.getAttribute(atributo);
        if (nombre === null) continue;
        if (tipo === 'submit') evento.preventDefault();
        try {
            const accion = Object.hasOwn(acciones, nombre) ? acciones[nombre] : undefined;
            if (!accion) throw new Error(`no existe la acción «${nombre}»`);
            const pasa = el.getAttribute('data-pasa') ?? (tipo === 'submit' ? 'evento' : null);
            const previos: unknown[] = pasa === 'evento' ? [evento] : pasa === 'elemento' ? [el] : pasa === 'valor' ? [el.value] : [];
            (accion as (...args: unknown[]) => unknown)(...previos, ...leerArgumentos(el.getAttribute('data-args')));
        } catch (error) {
            consola.error(`Manejador «${nombre}» (${tipo}):`, error);
        }
        if (evento.cancelBubble) break;
    }
}

/**
 * Engancha los oyentes delegados a `raiz` (en la aplicación, `document`). Devuelve nada: viven
 * lo que viva la página.
 */
export function delegar(raiz: RaizDeEventos, acciones: Acciones, consola: Consola = console): void {
    for (const tipo of Object.keys(ATRIBUTO_DE_ACCION) as EventoDelegado[]) {
        raiz.addEventListener(tipo, evento => atender(evento, tipo, acciones, consola));
    }
}

/**
 * Las acciones que no son de ninguna vista: navegar, cerrar una ventana y alternar pestañas.
 * Antes eran fragmentos de JavaScript repetidos dentro de los atributos (`navigate('x')`,
 * `elemento('modal-…').remove()`, el cambio de pestañas con `this.className=…`).
 */
export function accionesGenerales({ dom, navegar }: { dom: Dom; navegar: (ruta: string) => void }): Record<string, Accion> {
    return {
        navegar: ((ruta: string) => navegar(ruta)) as Accion,
        /** Cierra (quita del documento) la ventana o el panel con ese id. */
        quitar: ((id: string) => { dom.elemento(id).remove(); }) as Accion,
        /**
         * Pestañas: muestra `panel`, oculta los demás `paneles`, y deja el `boton` pulsado como
         * activo (`btn`) y el resto de `botones` como secundarios (`btn btn-secondary`).
         */
        activarPestana: ((boton: HTMLElement, panel: string, paneles: string[], botones: string[]) => {
            for (const id of paneles) dom.elemento(id).style.display = id === panel ? 'block' : 'none';
            for (const id of botones) dom.elemento(id).className = id === boton.id ? 'btn' : 'btn btn-secondary';
        }) as Accion,
    };
}
