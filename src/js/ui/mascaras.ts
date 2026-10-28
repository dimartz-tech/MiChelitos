// Máscaras de entrada declarativas: un campo con `data-mascara="fecha"` recibe la `/` sola al
// teclear la fecha (`nucleo/fechas.ts`). Como la delegación de eventos, es **un solo oyente** de
// `input` colgado de `document`: los formularios que se dibujan después funcionan sin enganchar
// nada y las plantillas no llevan código, solo datos (`ATRIBUTOS_DE_FECHA`, en `nucleo/html.ts`).
//
// Solo corrige cuando hace falta: si el texto ya está bien no se reasigna el valor (no se
// interrumpe a un lector de pantalla ni se mueve el cursor). El pegado y el autocompletado del
// navegador pasan por el mismo camino, porque también disparan `input`.

import { enmascararFecha } from '../nucleo/fechas.js';
import { ATRIBUTO_DE_MASCARA } from '../nucleo/html.js';
import type { RaizDeEventos } from './delegacion.js';

/** Lo que se lee y escribe de un campo de texto. */
interface CampoEnmascarable {
    getAttribute(nombre: string): string | null;
    value: string;
    readOnly?: boolean;
    disabled?: boolean;
    selectionStart?: number | null;
    setSelectionRange?(inicio: number, fin: number): void;
}

interface EventoDeEntrada {
    target: unknown;
    inputType?: string;
    isComposing?: boolean;
}

function atender(evento: EventoDeEntrada): void {
    const campo = evento.target as CampoEnmascarable | null;
    if (!campo || typeof campo.getAttribute !== 'function') return;
    if (campo.getAttribute(ATRIBUTO_DE_MASCARA) !== 'fecha') return;
    if (campo.readOnly || campo.disabled || evento.isComposing) return;
    const borrando = (evento.inputType ?? '').startsWith('delete');
    const actual = campo.value;
    const { texto, cursor } = enmascararFecha(actual, campo.selectionStart ?? actual.length, borrando);
    if (texto === actual) return;
    campo.value = texto;
    try { campo.setSelectionRange?.(cursor, cursor); } catch { /* un campo sin selección: el valor ya está bien */ }
}

/** Engancha el oyente de las máscaras a `raiz` (en la aplicación, `document`). */
export function enmascarar(raiz: RaizDeEventos): void {
    raiz.addEventListener('input', evento => atender(evento as unknown as EventoDeEntrada));
}
