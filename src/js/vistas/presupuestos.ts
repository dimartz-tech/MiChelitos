// Presupuestos mensuales por categoría: las funciones PURAS que dan forma a lo que calculó Rust
// (`obtener_presupuestos_mes`). Sin DOM, sin API, sin reloj: se prueban en Node (`pruebas/js/vistas/presupuestos.test.js`).
//
// Rust ya hizo toda la aritmética de dinero; aquí solo se rotula. Los importes llegan como TEXTO exacto y se formatean
// sobre el texto (`importeDeTexto`), sin `Number`. El porcentaje llega en milésimas (1000 = 100 %), truncado: aquí se
// muestra por división entera. El texto es descriptivo: dice dónde va el gasto, nunca qué hacer.

import type { EstadoPresupuestos, FilaPresupuesto } from '../tipos-ipc';

import { accion, alEnviar, escaparHtml } from '../nucleo/html.js';
import { importeDeTexto } from '../nucleo/importe_texto.js';

/** La nota que acompaña siempre a los presupuestos: los dólares no se convierten ni cuentan. */
export const NOTA_USD =
    'Solo cuentan los gastos en DOP. Los gastos en USD no se convierten (no hay una tasa que valga) y quedan fuera del cálculo; si los hay, se indican aparte.';

/** El estado en palabras. Nunca se comunica solo por color. */
export function etiquetaDeEstado(estado: string): string {
    if (estado === 'excedido') return 'Excedido';
    if (estado === 'cerca') return 'Cerca del límite (desde el 80 %)';
    return 'Dentro del presupuesto';
}

/** El porcentaje de milésimas, por división entera: 815 → «81.5 %». */
export function porcentajeDeMilesimas(milesimas: number): string {
    const m = Math.max(0, Math.trunc(milesimas));
    return `${Math.floor(m / 10)}.${m % 10} %`;
}

/** El ancho de la barra: nunca más del 100 %. */
export function anchoDeBarra(milesimas: number): number {
    return Math.min(100, Math.max(0, Math.floor(milesimas / 10)));
}

/** Lo que resta o lo que se pasó, en palabras, sobre el texto exacto de Rust. */
export function textoDeRestante(fila: Pick<FilaPresupuesto, 'restante'>): string {
    if (fila.restante.startsWith('-')) return `Excedido por DOP ${importeDeTexto(fila.restante.slice(1))}`;
    return `Restan DOP ${importeDeTexto(fila.restante)}`;
}

/** Hay dólares del mes fuera del cálculo. */
export function tieneDolares(fila: Pick<FilaPresupuesto, 'usd_aparte'>): boolean {
    return fila.usd_aparte !== '0.00';
}

/** Las filas que merecen un aviso: cerca o excedidas. */
export function filasEnAlerta(estado: EstadoPresupuestos): FilaPresupuesto[] {
    return estado.filas.filter(f => f.estado === 'cerca' || f.estado === 'excedido');
}

export interface AlertaDePresupuesto {
    tipo: string;
    mensaje: string;
    nivel: 'danger' | 'warning';
}

/** Los avisos del Inicio para el mes en curso: descriptivos, sin consejos. */
export function alertasDePresupuesto(estado: EstadoPresupuestos): AlertaDePresupuesto[] {
    return filasEnAlerta(estado).map(f => {
        const excedido = f.estado === 'excedido';
        const dolares = tieneDolares(f) ? ` (más USD ${importeDeTexto(f.usd_aparte)} fuera del cálculo)` : '';
        return {
            tipo: 'Presupuesto',
            nivel: excedido ? 'danger' : 'warning',
            mensaje: `${f.categoria_nombre}: ${excedido ? 'excedido' : 'cerca del límite'}, DOP ${importeDeTexto(f.gastado)} de DOP ${importeDeTexto(f.presupuesto)} (${porcentajeDeMilesimas(f.porcentaje_milesimas)}). ${textoDeRestante(f)}.${dolares}`,
        };
    });
}

/** Una categoría para el selector. */
export interface CategoriaParaPresupuesto {
    id: number;
    nombre: string;
}

function htmlDeFila(f: FilaPresupuesto): string {
    const color = f.estado === 'excedido' ? 'var(--color-danger)' : f.estado === 'cerca' ? 'var(--color-warning)' : 'var(--color-success)';
    const ancho = anchoDeBarra(f.porcentaje_milesimas);
    const simbolo = f.estado === 'excedido' ? '🚨' : f.estado === 'cerca' ? '⚠️' : '✔';
    return `
        <div class="presupuesto-fila" data-estado="${escaparHtml(f.estado)}">
            <div style="display:flex; justify-content:space-between; gap:0.5rem; font-size:0.8rem; margin-bottom:0.25rem;">
                <span style="font-weight:600;">${escaparHtml(f.categoria_nombre)}${f.fija ? ' <span style="font-size:0.7rem; color:var(--text-muted);">↻ fija / recurrente</span>' : ''}</span>
                <span style="font-family:var(--font-heading); font-weight:700;">DOP ${escaparHtml(importeDeTexto(f.gastado))} de DOP ${escaparHtml(importeDeTexto(f.presupuesto))}
                    <span style="font-size:0.7rem; color:var(--text-secondary); font-weight:normal;">(${escaparHtml(porcentajeDeMilesimas(f.porcentaje_milesimas))})</span></span>
            </div>
            <div role="progressbar" aria-label="Presupuesto de ${escaparHtml(f.categoria_nombre)}" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${ancho}"
                 aria-valuetext="${escaparHtml(`${porcentajeDeMilesimas(f.porcentaje_milesimas)}: ${etiquetaDeEstado(f.estado)}`)}"
                 style="background:rgba(255,255,255,0.03); height:8px; border-radius:4px; overflow:hidden; border:1px solid var(--border-color);">
                <div style="background:${color}; width:${ancho}%; height:100%;"></div>
            </div>
            <div style="display:flex; justify-content:space-between; align-items:center; gap:0.5rem; font-size:0.72rem; color:var(--text-secondary); margin-top:0.25rem;">
                <span><span aria-hidden="true">${simbolo}</span> ${escaparHtml(etiquetaDeEstado(f.estado))} · ${escaparHtml(textoDeRestante(f))}${tieneDolares(f) ? ` · USD ${escaparHtml(importeDeTexto(f.usd_aparte))} aparte (no cuenta)` : ''}</span>
                <button type="button" ${accion('handleQuitarPresupuesto', f.categoria_id)} class="btn btn-secondary" style="padding:0.1rem 0.5rem; font-size:0.7rem;"
                        aria-label="Quitar el presupuesto de ${escaparHtml(f.categoria_nombre)}">Quitar</button>
            </div>
        </div>`;
}

/**
 * El bloque «Presupuestos del mes» de Gastos: las barras con su estado, la nota de los dólares y el formulario para
 * fijar o cambiar el presupuesto de una categoría. `estado` es `null` si Rust no pudo responder.
 */
export function htmlPresupuestos(estado: EstadoPresupuestos | null, categorias: readonly CategoriaParaPresupuesto[]): string {
    const filas = estado?.filas ?? [];
    const totales = estado !== null && filas.length > 0
        ? `<p style="font-size:0.72rem; color:var(--text-secondary); margin:0.2rem 0 0;">Con presupuesto: DOP ${escaparHtml(importeDeTexto(estado.total_gastado))} de DOP ${escaparHtml(importeDeTexto(estado.total_presupuesto))}.</p>`
        : '';
    return `
        <div data-presupuestos style="margin-top: 1.5rem; border-top: 1px solid var(--border-color); padding-top: 1.2rem;">
            <h4 style="font-family: var(--font-heading); font-size: 1rem; margin-bottom: 0.4rem; color: var(--text-secondary);">Presupuestos del mes</h4>
            <p class="nota-presupuestos" style="font-size:0.72rem; color:var(--text-muted); margin:0 0 0.8rem;">${escaparHtml(NOTA_USD)}</p>
            ${estado === null ? `<p style="color:var(--text-muted); font-size:0.8rem;">No se pudieron leer los presupuestos.</p>` : filas.length === 0
                ? `<p style="color:var(--text-muted); font-size:0.8rem; margin:0 0 0.8rem;">Aún no hay presupuestos fijados. Elige una categoría y un monto mensual en pesos.</p>`
                : `<div style="display:flex; flex-direction:column; gap:0.9rem; margin-bottom:0.6rem;">${filas.map(htmlDeFila).join('')}</div>${totales}`}
            <form id="form-presupuesto" ${alEnviar('handleFijarPresupuesto')} style="display:flex; gap:0.5rem; align-items:flex-end; flex-wrap:wrap; margin-top:0.8rem;">
                <div class="form-group" style="margin:0;">
                    <label for="pre_cat">Categoría</label>
                    <select id="pre_cat" class="form-control">
                        ${categorias.map(c => `<option value="${escaparHtml(c.id)}">${escaparHtml(c.nombre)}</option>`).join('')}
                    </select>
                </div>
                <div class="form-group" style="margin:0;">
                    <label for="pre_monto">Presupuesto mensual (DOP)</label>
                    <input type="text" id="pre_monto" class="form-control" inputmode="decimal" autocomplete="off" placeholder="0.00" required>
                </div>
                <button type="submit" class="btn">Fijar</button>
            </form>
        </div>`;
}
