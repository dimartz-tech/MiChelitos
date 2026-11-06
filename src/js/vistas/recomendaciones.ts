// Recomendaciones del historial del patrimonio: las funciones PURAS que dan forma a lo que calculó Rust
// (`obtener_recomendaciones`). Sin DOM, sin API, sin reloj: se prueban en Node (`pruebas/js/vistas/recomendaciones.test.js`).
//
// Rust ya hizo toda la aritmética y decidió qué reglas se cumplen; aquí solo se rotula. Los importes llegan como TEXTO
// exacto y se formatean sobre el texto (`importeDeTexto`), sin `Number`. Son reglas DESCRIPTIVAS: cada tarjeta muestra el
// dato que la dispara y la regla en palabras. Nunca se presenta como asesoría financiera y nunca se inventa una cifra:
// sin datos suficientes se dice.

import type { CifraRecomendacion, RecomendacionPatrimonio, ResultadoRecomendaciones } from '../tipos-ipc';

import { accion, escaparHtml } from '../nucleo/html.js';
import { importeDeTexto } from '../nucleo/importe_texto.js';

/** El aviso que acompaña siempre al bloque, venga o no la respuesta de Rust. */
export const AVISO_NO_ASESORIA = 'Reglas descriptivas, no asesoría financiera.';

/** Las pestañas a las que una recomendación puede llevar (las rutas del menú). Cualquier otro destino no se enlaza. */
const DESTINOS: Readonly<Record<string, string>> = {
    gastos: 'Gastos',
    tarjetas: 'Tarjetas',
    prestamos: 'Deudas',
    resumen: 'Resumen',
    cuentas: 'Cuentas',
};

const REGLAS: Readonly<Record<string, string>> = {
    presupuesto: 'Presupuestos',
    tendencia_gasto: 'Tendencia del gasto',
    variacion_patrimonio: 'Variación del patrimonio',
    deuda_y_ahorro: 'Deuda y ahorro',
    cargos_derivados: 'Cargos derivados',
    fijo_variable: 'Gasto fijo y variable',
};

const ORIGENES_USD: Readonly<Record<string, string>> = {
    gastos: 'gastos del último mes completo',
    tarjetas: 'deuda de tarjetas',
    cuentas: 'saldo de cuentas',
};

/** La severidad en palabras. Nunca se comunica solo por color. */
export function etiquetaDeSeveridad(severidad: string): string {
    return severidad === 'atencion' ? 'Atención' : 'Informativa';
}

/** El nombre de una regla para el listado de reglas sin datos. */
export function nombreDeRegla(regla: string): string {
    return REGLAS[regla] ?? regla;
}

/** Una cifra de respaldo lista para mostrar: pesos, dólares sin convertir, porcentaje, tasa o texto. */
export function textoDeCifra(c: Pick<CifraRecomendacion, 'tipo' | 'valor'>): string {
    if (c.tipo === 'importe') return `DOP ${importeDeTexto(c.valor)}`;
    if (c.tipo === 'importe_usd') return `USD ${importeDeTexto(c.valor)}`;
    if (c.tipo === 'porcentaje') return `${c.valor} %`;
    return c.valor;
}

/** El nombre de la pestaña a la que lleva el destino, o `null` si no es una pestaña enlazable. */
export function etiquetaDeDestino(destino: string): string | null {
    return DESTINOS[destino] ?? null;
}

function htmlDeCifras(cifras: readonly CifraRecomendacion[]): string {
    if (cifras.length === 0) return '';
    const filas = cifras
        .map(c => `<div style="display:flex; justify-content:space-between; gap:0.75rem;"><dt style="color:var(--text-secondary);">${escaparHtml(c.etiqueta)}</dt><dd style="margin:0; font-weight:600; text-align:right;">${escaparHtml(textoDeCifra(c))}</dd></div>`)
        .join('');
    return `<dl class="reco-cifras" style="margin:0.5rem 0; font-size:0.8rem; display:flex; flex-direction:column; gap:0.15rem;">${filas}</dl>`;
}

/** Una recomendación: severidad en palabras, título, qué dicen los datos, las cifras, la regla y el enlace. */
export function htmlDeRecomendacion(r: RecomendacionPatrimonio): string {
    const atencion = r.severidad === 'atencion';
    const simbolo = atencion ? '⚠' : 'ℹ';
    const borde = atencion ? 'var(--color-warning)' : 'var(--border-color)';
    const insignia = atencion ? 'badge pendiente' : 'badge';
    const destino = etiquetaDeDestino(r.destino);
    const enlace = destino === null
        ? ''
        : `<button type="button" ${accion('navegar', r.destino)} class="btn btn-secondary" style="padding:0.2rem 0.7rem; font-size:0.75rem;">Ver en ${escaparHtml(destino)}</button>`;
    return `
        <article class="reco" data-reco-id="${escaparHtml(r.id)}" data-severidad="${escaparHtml(r.severidad)}" style="border-left:3px solid ${borde}; padding:0.5rem 0.8rem; margin-bottom:0.8rem;">
            <h4 style="margin:0 0 0.3rem 0; font-size:0.95rem;"><span class="${insignia}" style="margin-right:0.4rem;"><span aria-hidden="true">${simbolo}</span> ${escaparHtml(etiquetaDeSeveridad(r.severidad))}</span>${escaparHtml(r.titulo)}</h4>
            <p style="margin:0; font-size:0.85rem;">${escaparHtml(r.explicacion)}</p>
            ${htmlDeCifras(r.cifras)}
            <p class="reco-regla" style="margin:0 0 0.5rem 0; font-size:0.72rem; color:var(--text-secondary);"><strong>Regla:</strong> ${escaparHtml(r.regla_texto)}</p>
            ${enlace}
        </article>`;
}

function htmlSinDatos(r: ResultadoRecomendaciones): string {
    if (r.sin_datos.length === 0) return '';
    const items = r.sin_datos
        .map(s => `<li><strong>${escaparHtml(nombreDeRegla(s.regla))}:</strong> ${escaparHtml(s.motivo)}</li>`)
        .join('');
    return `
        <details class="reco-sin-datos" style="margin-top:0.6rem; font-size:0.8rem;">
            <summary>Reglas sin datos suficientes (${r.sin_datos.length})</summary>
            <ul style="margin:0.4rem 0 0 1.1rem; padding:0;">${items}</ul>
        </details>`;
}

function htmlDolares(r: ResultadoRecomendaciones): string {
    if (r.usd_excluido.length === 0) return '';
    const partes = r.usd_excluido
        .map(u => `USD ${escaparHtml(importeDeTexto(u.importe))} en ${escaparHtml(ORIGENES_USD[u.origen] ?? u.origen)}`)
        .join('; ');
    return `<p class="reco-usd" style="margin:0.6rem 0 0; font-size:0.72rem; color:var(--text-muted);">Fuera del análisis, sin convertir (no hay tasa que valga): ${partes}.</p>`;
}

function nombreDeMes(anio: number, mes: number): string {
    return `${String(mes).padStart(2, '0')}/${anio}`;
}

/**
 * El bloque «Cómo aumentar tu patrimonio» de la pestaña Evolución. `resultado` es `null` si Rust no pudo responder
 * (`error` lleva el motivo). Con datos insuficientes lo dice en vez de inventar.
 */
export function htmlRecomendaciones(resultado: ResultadoRecomendaciones | null, error: string | null = null): string {
    const cabecera = `
            <h3 id="evo-reco-titulo" style="font-family: var(--font-heading); font-size: 1.1rem; margin-bottom: 0.4rem;">Cómo aumentar tu patrimonio</h3>
            <p class="reco-aviso" role="note" style="font-size:0.8rem; margin:0 0 0.8rem 0; color:var(--text-secondary);"><strong>${escaparHtml(AVISO_NO_ASESORIA)}</strong> ${escaparHtml(resultado?.aviso ?? '')}</p>`;
    let cuerpo = '';
    if (resultado === null) {
        cuerpo = `<p role="alert" style="font-size:0.85rem; color:var(--color-danger);">No se pudieron calcular las recomendaciones${error ? `: ${escaparHtml(error)}` : '.'}</p>`;
    } else {
        const periodos = `<p class="reco-periodos" style="font-size:0.72rem; color:var(--text-muted); margin:0 0 0.6rem 0;">Gastos leídos del último mes completo (${escaparHtml(nombreDeMes(resultado.anio_gastos, resultado.mes_gastos))}); presupuestos, del mes en curso (${escaparHtml(nombreDeMes(resultado.anio_presupuestos, resultado.mes_presupuestos))}); patrimonio, de las dos últimas fotos mensuales. Solo pesos.</p>`;
        const pie = [htmlSinDatos(resultado), htmlDolares(resultado)].join('');
        if (resultado.recomendaciones.length === 0) {
            const mensaje = resultado.reglas_evaluadas === 0
                ? 'Aún no hay datos suficientes para recomendar nada: faltan gastos de meses completos, presupuestos, fotos del patrimonio o tarjetas y cuentas con saldo.'
                : 'Con tus datos de hoy ninguna regla se activa: no hay nada que señalar.';
            cuerpo = `${periodos}<p class="reco-vacio" style="font-size:0.9rem;">${escaparHtml(mensaje)}</p>${pie}`;
        } else {
            const omitidas = resultado.omitidas > 0
                ? `<p class="reco-omitidas" style="font-size:0.78rem; color:var(--text-secondary);">Hay ${resultado.omitidas} recomendación(es) más que no se muestran (como máximo ${resultado.maximo_por_severidad} de cada severidad).</p>`
                : '';
            cuerpo = `${periodos}${resultado.recomendaciones.map(htmlDeRecomendacion).join('')}${omitidas}${pie}`;
        }
    }
    return `<section class="card" id="evo-recomendaciones" aria-labelledby="evo-reco-titulo" style="margin-bottom:1.5rem;">${cabecera}${cuerpo}</section>`;
}
