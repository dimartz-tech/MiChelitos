// Gráfica SVG de la evolución del patrimonio: una FUNCIÓN PURA que devuelve texto SVG.
//
// Sin DOM, sin Tauri y sin librerías: se prueba en Node comparando el texto. Los colores son
// variables CSS de la aplicación (`--accent-primary`, `--color-success`, `--color-danger`, …),
// así que sirve igual en modo claro y oscuro.
//
// Dinero: las coordenadas se calculan con **centavos enteros** (`bigint`, ver `evolucion.ts`):
// la posición vertical es `(tope - valor) * alto / (tope - base)` en enteros, con una décima de
// píxel de resolución; solo el resultado (una coordenada) pasa a `number`. Las etiquetas del
// eje y la descripción se formatean sobre los centavos, no sobre un flotante.
//
// Marcadores: círculo relleno = foto real; rombo hueco = foto estimada o reconstruida, y el
// tramo que llega a una foto así se dibuja punteado. Hoy todas las fotos son reales; el
// estimado existe para cuando se reconstruya el pasado.
//
// Eje horizontal: proporcional a los meses (un mes sin foto deja un hueco, no se aprieta).
// Con menos de dos fotos no hay nada que dibujar: devuelve la cadena vacía.

import { escaparHtml } from './html.js';
import {
    esEstimada, etiquetaDeSerie, formatearCentavos, nombreDeMes, ordenarFotos, valorDeSerie,
    type FotoEvolucion, type Serie,
} from './evolucion.js';

export interface OpcionesGrafica {
    serie: Serie;
    /** Prefijo de los ids del título y la descripción (único por gráfica en la página). */
    id?: string;
    ancho?: number;
    alto?: number;
}

const MARGEN = { izq: 92, der: 24, arriba: 20, abajo: 44 };
const MARCAS = 5;

const px = (n: number): string => (Math.round(n * 10) / 10).toString();

/** Descripción accesible: «Patrimonio total de A en mar 2027 a B en jun 2027», con el mínimo y el máximo. */
export function describirGrafica(fotos: readonly FotoEvolucion[], serie: Serie): string {
    const orden = ordenarFotos(fotos);
    if (orden.length === 0) return '';
    const valores = orden.map(f => valorDeSerie(f, serie));
    const mes = (i: number): string => nombreDeMes(orden[i].anio, orden[i].mes);
    let imin = 0, imax = 0;
    valores.forEach((v, i) => { if (v < valores[imin]) imin = i; if (v > valores[imax]) imax = i; });
    const ultimo = orden.length - 1;
    return `${etiquetaDeSerie(serie)} de ${formatearCentavos(valores[0])} en ${mes(0)} a ${formatearCentavos(valores[ultimo])} en ${mes(ultimo)}. ` +
        `Mínimo ${formatearCentavos(valores[imin])} en ${mes(imin)}; máximo ${formatearCentavos(valores[imax])} en ${mes(imax)}. Importes en pesos dominicanos.`;
}

export function generarGrafica(fotos: readonly FotoEvolucion[], { serie, id = 'grafica-evolucion', ancho = 720, alto = 320 }: OpcionesGrafica): string {
    const orden = ordenarFotos(fotos);
    if (orden.length < 2) return '';

    const valores = orden.map(f => valorDeSerie(f, serie));
    let min = valores[0], max = valores[0];
    for (const v of valores) { if (v < min) min = v; if (v > max) max = v; }

    // Dominio con un 5 % de aire; si todos los valores son iguales, un 10 % del valor (o 100.00 si es cero).
    let colchon = (max - min) / 20n;
    if (max === min) {
        const abs = min < 0n ? -min : min;
        colchon = abs === 0n ? 10000n : abs / 10n;
    }
    if (colchon === 0n) colchon = 1n;
    const base = min - colchon, tope = max + colchon;
    const rango = tope - base;

    const areaAncho = ancho - MARGEN.izq - MARGEN.der;
    const areaAlto = alto - MARGEN.arriba - MARGEN.abajo;
    const escala = BigInt(areaAlto * 10);
    const yDe = (v: bigint): number => MARGEN.arriba + Number(((tope - v) * escala) / rango) / 10;

    const meses = orden.map(f => f.anio * 12 + f.mes);
    const m0 = meses[0], span = meses[meses.length - 1] - m0 || 1;
    const xDe = (i: number): number => MARGEN.izq + ((meses[i] - m0) * areaAncho) / span;

    const color = serie === 'liquido' ? 'var(--color-success)' : 'var(--accent-primary)';
    const idTitulo = `${id}-titulo`, idDesc = `${id}-desc`;
    const titulo = `${etiquetaDeSerie(serie)}: evolución mensual`;
    const partes: string[] = [];

    // Marcas del eje vertical (con decimales solo si el rango es de pocas unidades).
    const conDecimales = rango < 1000n;
    const vistas = new Set<string>();
    for (let i = 0; i < MARCAS; i++) {
        const v = base + (rango * BigInt(i)) / BigInt(MARCAS - 1);
        const etiqueta = formatearCentavos(v, conDecimales);
        if (vistas.has(etiqueta)) continue;
        vistas.add(etiqueta);
        const y = yDe(v);
        partes.push(`<line class="rejilla" x1="${MARGEN.izq}" y1="${px(y)}" x2="${ancho - MARGEN.der}" y2="${px(y)}" stroke="var(--border-color)" stroke-width="1"/>`);
        partes.push(`<text class="eje-y" x="${MARGEN.izq - 8}" y="${px(y + 4)}" text-anchor="end" font-size="11" fill="var(--text-secondary)">${escaparHtml(etiqueta)}</text>`);
    }
    if (base < 0n && tope > 0n) {
        const y0 = yDe(0n);
        partes.push(`<line class="cero" x1="${MARGEN.izq}" y1="${px(y0)}" x2="${ancho - MARGEN.der}" y2="${px(y0)}" stroke="var(--color-danger)" stroke-width="1" stroke-dasharray="2 3"/>`);
    }

    // Eje horizontal: una etiqueta cada `salto` fotos, y siempre la última.
    const salto = Math.ceil(orden.length / 6);
    orden.forEach((f, i) => {
        if (i % salto !== 0 && i !== orden.length - 1) return;
        partes.push(`<text class="eje-x" x="${px(xDe(i))}" y="${alto - MARGEN.abajo + 20}" text-anchor="middle" font-size="11" fill="var(--text-secondary)">${escaparHtml(nombreDeMes(f.anio, f.mes))}</text>`);
    });

    // Tramos: punteados si cualquiera de sus extremos es estimado.
    for (let i = 1; i < orden.length; i++) {
        const estimado = esEstimada(orden[i - 1]) || esEstimada(orden[i]);
        partes.push(`<path class="${estimado ? 'tramo-estimado' : 'tramo-real'}" d="M${px(xDe(i - 1))},${px(yDe(valores[i - 1]))} L${px(xDe(i))},${px(yDe(valores[i]))}" fill="none" stroke="${color}" stroke-width="2"${estimado ? ' stroke-dasharray="6 4"' : ''}/>`);
    }

    // Marcadores.
    orden.forEach((f, i) => {
        const x = xDe(i), y = yDe(valores[i]);
        const pie = `${nombreDeMes(f.anio, f.mes)}: ${formatearCentavos(valores[i])}`;
        if (esEstimada(f)) {
            partes.push(`<path class="marcador-estimado" d="M${px(x)},${px(y - 6)} L${px(x + 6)},${px(y)} L${px(x)},${px(y + 6)} L${px(x - 6)},${px(y)} Z" fill="var(--bg-surface-opaque)" stroke="${color}" stroke-width="2"><title>${escaparHtml(pie)} (estimado)</title></path>`);
        } else {
            partes.push(`<circle class="marcador-real" cx="${px(x)}" cy="${px(y)}" r="4.5" fill="${color}"><title>${escaparHtml(pie)}</title></circle>`);
        }
    });

    return `<svg class="grafica-evolucion" role="img" aria-labelledby="${escaparHtml(idTitulo)} ${escaparHtml(idDesc)}" viewBox="0 0 ${ancho} ${alto}" width="100%" style="max-width:${ancho}px;height:auto" xmlns="http://www.w3.org/2000/svg">` +
        `<title id="${escaparHtml(idTitulo)}">${escaparHtml(titulo)}</title>` +
        `<desc id="${escaparHtml(idDesc)}">${escaparHtml(describirGrafica(orden, serie))}</desc>` +
        partes.join('') +
        `</svg>`;
}
