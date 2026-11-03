// «Gasto por categoría» de la pestaña Resumen: las funciones PURAS que dan forma a lo que calculó Rust
// (`obtener_resumen_gastos`). Sin DOM, sin API, sin reloj: se prueban en Node (`pruebas/js/vistas/resumen_gastos.test.js`).
//
// Rust ya hizo toda la aritmética de dinero; aquí solo se rotula. Los importes llegan como TEXTO exacto y se
// formatean sobre el texto (`importeDeTexto`), sin `Number`. Las variaciones llegan en puntos básicos (100 = 1 %) y
// las partes del total en milésimas (1000 = 100 %): enteros, así que el texto sale por división entera.

import type { Categoria, FilaResumenGasto, MedidasResumen, MesDisponible, ResumenGastos } from '../tipos-ipc';

import { accion, escaparHtml, manejador } from '../nucleo/html.js';
import { importeDeTexto, porTasaEnTexto } from '../nucleo/importe_texto.js';

/** Los promedios que ofrece el selector («Promedio de N meses») y el que se usa por defecto. */
export const PROMEDIOS_OFRECIDOS: readonly number[] = [2, 3, 6];
export const PROMEDIO_POR_DEFECTO = 3;

/** Las categorías de inversión y obligaciones que el conmutador deja fuera (comparadas sin tildes ni mayúsculas). */
export const CATEGORIAS_DE_INVERSION_Y_OBLIGACIONES: readonly string[] = ['Impuestos', 'Maquinaria/Equipos', 'Prestamo', 'Renta', 'Alquiler'];

/** El id (sintético) de la fila «Cargos derivados» que usa el núcleo. */
export const ID_CARGOS_DERIVADOS = -1;

const MESES = ['enero', 'febrero', 'marzo', 'abril', 'mayo', 'junio', 'julio', 'agosto', 'septiembre', 'octubre', 'noviembre', 'diciembre'];

/** Minúsculas y sin tildes, para comparar nombres de categoría. */
function normalizar(nombre: string): string {
    return nombre.normalize('NFD').replace(/[̀-ͯ]/g, '').trim().toLowerCase();
}

/**
 * Los ids de las categorías de inversión y obligaciones que EXISTEN en el catálogo (se buscan por nombre; nunca se
 * escriben ids a mano). Si alguna no existe, simplemente no se excluye.
 */
export function idsDeInversionYObligaciones(categorias: readonly Categoria[]): number[] {
    const buscadas = new Set(CATEGORIAS_DE_INVERSION_Y_OBLIGACIONES.map(normalizar));
    return categorias.filter(c => buscadas.has(normalizar(c.nombre))).map(c => c.id);
}

/** «marzo 2027». */
export function etiquetaDeMes(anio: number, mes: number): string {
    return `${MESES[mes - 1] ?? `mes ${mes}`} ${anio}`;
}

/** El valor de una opción del selector de mes: «2027-03». */
export function valorDeMes(m: MesDisponible): string {
    return `${m.anio}-${String(m.mes).padStart(2, '0')}`;
}

/** Lo inverso de `valorDeMes`; `null` si el texto no es un mes. */
export function mesDeValor(valor: string): MesDisponible | null {
    const m = /^(\d{4})-(\d{2})$/.exec(valor);
    if (!m) return null;
    const mes = Number(m[2]);
    return mes >= 1 && mes <= 12 ? { anio: Number(m[1]), mes } : null;
}

/** Los meses del selector, de más antiguo a más reciente: los que tienen datos y, siempre, el elegido. */
export function mesesDelSelector(disponibles: readonly MesDisponible[], elegido: MesDisponible): MesDisponible[] {
    const porValor = new Map<string, MesDisponible>();
    for (const m of [...disponibles, elegido]) porValor.set(valorDeMes(m), m);
    return [...porValor.values()].sort((a, b) => a.anio - b.anio || a.mes - b.mes);
}

/** Partes del total en milésimas (1000 = 100 %) como «33.3 %». */
export function porcentajeDeMilesimas(milesimas: number): string {
    const neg = milesimas < 0 ? '-' : '';
    const abs = Math.abs(Math.trunc(milesimas));
    return `${neg}${Math.trunc(abs / 10)}.${abs % 10} %`;
}

export type SentidoDeVariacion = 'sube' | 'baja' | 'igual' | 'sin-referencia';

export interface Variacion {
    sentido: SentidoDeVariacion;
    /** ▲ ▼ ● — (decorativa: el texto dice lo mismo). */
    flecha: string;
    /** «sube 12.34 %», «baja 20.00 %», «igual», «—». */
    texto: string;
}

/**
 * Una variación en puntos básicos como flecha Y texto: el color nunca es la única señal. `null` es «sin
 * referencia» (el mes de comparación no tenía gasto o no existía), no un cero.
 */
export function variacionDePb(pb: number | null): Variacion {
    if (pb === null) return { sentido: 'sin-referencia', flecha: '', texto: '—' };
    if (pb === 0) return { sentido: 'igual', flecha: '=', texto: 'igual' };
    const abs = Math.abs(Math.trunc(pb));
    const cifra = `${Math.trunc(abs / 100)}.${String(abs % 100).padStart(2, '0')} %`;
    return pb > 0
        ? { sentido: 'sube', flecha: '▲', texto: `sube ${cifra}` }
        : { sentido: 'baja', flecha: '▼', texto: `baja ${cifra}` };
}

/** Gastar más es la señal de alerta (rojo); gastar menos, la buena (verde). Solo refuerza: la flecha y el texto ya lo dicen. */
const COLOR_DE_SENTIDO: Record<SentidoDeVariacion, string> = {
    'sube': 'var(--color-danger, #ef4444)',
    'baja': 'var(--color-success, #22c55e)',
    'igual': 'var(--text-muted)',
    'sin-referencia': 'var(--text-muted)',
};

function htmlDeVariacion(pb: number | null): string {
    const v = variacionDePb(pb);
    const flecha = v.flecha ? `<span aria-hidden="true">${escaparHtml(v.flecha)}</span> ` : '';
    const titulo = v.sentido === 'sin-referencia' ? ' title="Sin referencia con la que comparar"' : '';
    return `<span class="rg-variacion rg-${v.sentido}" style="color:${escaparHtml(COLOR_DE_SENTIDO[v.sentido])}; white-space:nowrap;"${titulo}>${flecha}${escaparHtml(v.texto)}</span>`;
}

/** La parte en dólares de un importe, o `''` si no hay (`0.00`). */
function usdAparte(m: MedidasResumen): string {
    return /^-?0+(\.0+)?$/.test(m.usd_aparte) ? '' : m.usd_aparte;
}

/** Orden de la tabla: las categorías como llegan y la fila sintética «Cargos derivados» siempre al final. */
export function filasOrdenadas(filas: readonly FilaResumenGasto[]): FilaResumenGasto[] {
    return [...filas.filter(f => !f.derivado), ...filas.filter(f => f.derivado)];
}

/**
 * «incluye USD 12.34 ≈ DOP 740.40 a tasa 60.00 · estimado». Los dólares ya están contados en el total en pesos
 * (la parte liquidada con lo que fijó el emisor; el resto, con la tasa de referencia): es una nota, no un sumando.
 */
export function notaDeDolares(usd: string, tasa: string): string {
    const en = porTasaEnTexto(usd, tasa);
    const tasaTexto = importeDeTexto(tasa);
    return en === null
        ? `incluye USD ${importeDeTexto(usd)} · estimado`
        : `incluye USD ${importeDeTexto(usd)} ≈ DOP ${importeDeTexto(en)} a tasa ${tasaTexto} · estimado`;
}

function htmlDeFila(f: FilaResumenGasto, r: ResumenGastos): string {
    const m = f.medidas;
    const usd = usdAparte(m);
    const etiquetaDeBarra = `${f.categoria_nombre}: ${porcentajeDeMilesimas(m.porcentaje_milesimas)} del total`;
    const barra = Math.max(0, Math.min(100, m.porcentaje_milesimas / 10));
    const nota = f.derivado
        ? `<small style="display:block; color:var(--text-muted); font-weight:400;">Comisiones de abonos y cargos de avances de efectivo, agrupados aparte (no son una categoría que elijas).</small>`
        : '';
    const tramo = r.parcial && r.hasta_dia !== null
        ? `<small style="display:block; color:var(--text-muted);">al día ${r.hasta_dia} del mes anterior: ${m.anterior_mismo_dia === null ? 'sin dato' : `DOP ${escaparHtml(importeDeTexto(m.anterior_mismo_dia))}`} · ${htmlDeVariacion(m.variacion_mismo_dia_pb)}</small>`
        : '';
    return `
                <tr${f.derivado ? ' class="rg-derivado"' : ''}>
                    <th scope="row" style="text-align:left;">${escaparHtml(f.categoria_nombre)}${nota}</th>
                    <td style="text-align:right;">${escaparHtml(importeDeTexto(m.importe))}${usd ? `<small style="display:block; color:var(--text-muted);">USD ${escaparHtml(importeDeTexto(usd))}</small>` : ''}</td>
                    <td>
                        ${escaparHtml(porcentajeDeMilesimas(m.porcentaje_milesimas))}
                        <div class="rg-barra" role="img" aria-label="${escaparHtml(etiquetaDeBarra)}" style="height:6px; min-width:80px; background:rgba(255,255,255,0.08); border-radius:3px; overflow:hidden; margin-top:2px;">
                            <div style="width:${barra}%; height:100%; background:var(--accent-primary);"></div>
                        </div>
                    </td>
                    <td>${htmlDeVariacion(m.variacion_anterior_pb)}<small style="display:block; color:var(--text-muted);">mes anterior: DOP ${escaparHtml(importeDeTexto(m.anterior))}</small>${tramo}</td>
                    <td>${htmlDeVariacion(m.variacion_promedio_pb)}<small style="display:block; color:var(--text-muted);">${m.promedio === null ? 'sin promedio' : `promedio: DOP ${escaparHtml(importeDeTexto(m.promedio))}`}</small></td>
                </tr>`;
}

/** Lo que el titular tiene elegido en el bloque. */
export interface EleccionDeGasto {
    anio: number;
    mes: number;
    nPromedio: number;
    /** `true`: todo incluido; `false`: sin inversión ni obligaciones. */
    incluirTodo: boolean;
    /** Los meses con datos (de la última respuesta buena). */
    mesesDisponibles: readonly MesDisponible[];
    /** Los nombres de las categorías que el conmutador deja fuera, cuando está apagado. */
    excluidas: readonly string[];
}

export type EstadoDeGasto =
    | { tipo: 'cargando' }
    | { tipo: 'error'; mensaje: string }
    | { tipo: 'ok'; resumen: ResumenGastos };

/** Los ids de los controles (la vista los usa para devolver el foco tras repintar). */
export const ID_BLOQUE_GASTO = 'resumen-gasto-categoria';
export const ID_MES = 'rg-mes';
export const ID_PROMEDIO = 'rg-promedio';
export const ID_CONMUTADOR = 'rg-incluir-todo';
export const ID_ANTERIOR = 'rg-mes-anterior';
export const ID_SIGUIENTE = 'rg-mes-siguiente';

function htmlDeControles(e: EleccionDeGasto): string {
    const meses = mesesDelSelector(e.mesesDisponibles, { anio: e.anio, mes: e.mes });
    const actual = valorDeMes({ anio: e.anio, mes: e.mes });
    const indice = meses.findIndex(m => valorDeMes(m) === actual);
    const opcionesMes = [...meses].reverse()
        .map(m => `<option value="${valorDeMes(m)}"${valorDeMes(m) === actual ? ' selected' : ''}>${escaparHtml(etiquetaDeMes(m.anio, m.mes))}</option>`).join('');
    const opcionesN = PROMEDIOS_OFRECIDOS
        .map(n => `<option value="${n}"${n === e.nPromedio ? ' selected' : ''}>${n} meses</option>`).join('');
    return `
            <div style="display:flex; flex-wrap:wrap; gap:0.75rem; align-items:flex-end; margin-bottom:0.75rem;">
                <button type="button" class="btn btn-secondary" id="${ID_ANTERIOR}" ${accion('resumenGastoMover', -1)}${indice <= 0 ? ' disabled' : ''} aria-label="Mes anterior">‹ Anterior</button>
                <label style="display:flex; flex-direction:column; font-size:0.75rem; gap:0.2rem;">Mes
                    <select id="${ID_MES}" ${manejador('change', 'resumenGastoMes', [], 'valor')}>${opcionesMes}</select>
                </label>
                <button type="button" class="btn btn-secondary" id="${ID_SIGUIENTE}" ${accion('resumenGastoMover', 1)}${indice >= meses.length - 1 ? ' disabled' : ''} aria-label="Mes siguiente">Siguiente ›</button>
                <label style="display:flex; flex-direction:column; font-size:0.75rem; gap:0.2rem;">Promedio de
                    <select id="${ID_PROMEDIO}" ${manejador('change', 'resumenGastoPromedio', [], 'valor')}>${opcionesN}</select>
                </label>
                <label style="display:flex; align-items:center; gap:0.4rem; font-size:0.85rem;">
                    <input type="checkbox" id="${ID_CONMUTADOR}" ${manejador('change', 'resumenGastoConmutar', [], 'elemento')}${e.incluirTodo ? ' checked' : ''}>
                    Incluir inversión y obligaciones
                </label>
            </div>`;
}

function htmlDeCuerpo(estado: EstadoDeGasto, e: EleccionDeGasto): string {
    if (estado.tipo === 'cargando') {
        return `<p role="status" style="color:var(--text-muted);">Cargando el gasto de ${escaparHtml(etiquetaDeMes(e.anio, e.mes))}…</p>`;
    }
    if (estado.tipo === 'error') {
        return `<div class="alert-banner warning" role="alert"><span>⚠️</span><div>No se pudo calcular el gasto por categoría (${escaparHtml(estado.mensaje)}). El resto del Resumen no se ve afectado; prueba otro mes o vuelve a abrir la pestaña.</div></div>`;
    }
    const r = estado.resumen;
    const mes = etiquetaDeMes(r.anio, r.mes);
    if (r.filas.length === 0) {
        return `<p role="status" style="color:var(--text-muted);">Sin gastos en ${escaparHtml(mes)}${e.incluirTodo ? '' : ' (con la inversión y las obligaciones fuera)'}.</p>`;
    }
    const parcial = r.parcial ? `, parcial, hasta el día ${r.hasta_dia ?? '?'}` : '';
    const usd = usdAparte(r.totales);
    const exclusion = e.incluirTodo || e.excluidas.length === 0 ? ''
        : `<p style="margin:0.25rem 0; font-size:0.75rem; color:var(--text-muted);">Fuera de la tabla: ${escaparHtml(e.excluidas.join(', '))}.</p>`;
    const etiquetaParcial = r.parcial
        ? `<span class="badge emitida" style="font-size:0.7rem;">parcial, hasta el día ${r.hasta_dia ?? '?'}</span>` : '';
    return `
            <p style="margin:0.25rem 0;">
                <strong style="font-size:1.25rem;">Total DOP ${escaparHtml(importeDeTexto(r.totales.importe))}</strong>
                ${etiquetaParcial}
                ${usd ? `<small style="display:block; color:var(--text-muted);">${escaparHtml(notaDeDolares(usd, r.tasa_usd))}</small>` : ''}
            </p>
            ${exclusion}
            <div style="overflow-x:auto;">
            <table class="rg-tabla" style="width:100%; border-collapse:collapse; font-size:0.85rem;">
                <caption style="text-align:left; padding:0.4rem 0; color:var(--text-secondary);">Gasto por categoría de ${escaparHtml(mes + parcial)} (DOP). Comparado con el mes anterior y con el promedio de ${r.meses_promedio} meses completos.</caption>
                <thead>
                    <tr>
                        <th scope="col" style="text-align:left;">Categoría</th>
                        <th scope="col" style="text-align:right;">Importe</th>
                        <th scope="col">% del total</th>
                        <th scope="col">Contra el mes anterior</th>
                        <th scope="col">Contra el promedio de ${r.meses_promedio} meses</th>
                    </tr>
                </thead>
                <tbody>${filasOrdenadas(r.filas).map(f => htmlDeFila(f, r)).join('')}
                </tbody>
                <tfoot>
                    <tr>
                        <th scope="row" style="text-align:left;">Total</th>
                        <td style="text-align:right;"><strong>${escaparHtml(importeDeTexto(r.totales.importe))}</strong></td>
                        <td>100 %</td>
                        <td>${htmlDeVariacion(r.totales.variacion_anterior_pb)}</td>
                        <td>${htmlDeVariacion(r.totales.variacion_promedio_pb)}</td>
                    </tr>
                </tfoot>
            </table>
            </div>`;
}

/** El bloque entero (controles + estado), que va dentro de `#resumen-gasto-categoria`. */
export function htmlDeGastoPorCategoria(estado: EstadoDeGasto, eleccion: EleccionDeGasto): string {
    return `
            <h3 style="font-family:var(--font-heading); font-size:1.15rem; border-bottom:1px solid var(--border-color); padding-bottom:0.5rem; margin-bottom:0.75rem;">📂 Gasto por categoría</h3>${htmlDeControles(eleccion)}
            ${htmlDeCuerpo(estado, eleccion)}`;
}
