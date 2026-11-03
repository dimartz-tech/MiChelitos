// Presentación del estado de un traspaso (modelo de asientos de compensación, PROTOTIPO).
//
// Un traspaso anulado no desaparece: sobrevive marcado `anulada` y su contrario es una fila `compensacion` que apunta
// al original (`compensa_a`). Estas funciones solo dibujan esa distinción; los números salen del listado tal cual.

import { escaparHtml } from './html.js';

/** Lo mínimo del listado que hace falta para rotular un traspaso. */
export interface EstadoDeTraspaso {
    estado: string;
    compensa_a: number | null;
}

/** La insignia de un traspaso: vacía si está vigente. Todo texto pasa por `escaparHtml`. */
export function insigniaDeTraspaso(t: EstadoDeTraspaso): string {
    if (t.estado === 'anulada') {
        return '<span class="badge danger" data-estado-traspaso="anulada">Anulado</span>';
    }
    if (t.estado === 'compensacion') {
        return `<span class="badge emitida" data-estado-traspaso="compensacion">Compensación de #${escaparHtml(t.compensa_a ?? '?')}</span>`;
    }
    return '';
}

/** Solo un traspaso vigente se puede anular (el original anulado ya lo está; una compensación no se anula). */
export function sePuedeAnular(t: EstadoDeTraspaso): boolean {
    return t.estado === 'vigente';
}

/** Atributos de una fila que ya no cuenta: atenuada. Vacío si está vigente. */
export function atributosDeFilaDeTraspaso(t: EstadoDeTraspaso): string {
    return t.estado === 'vigente' ? '' : ' class="traspaso-no-vigente" style="opacity:0.6;"';
}
