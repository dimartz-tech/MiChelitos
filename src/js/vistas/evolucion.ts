// Pestaña «Evolución» (diseño «B»: clase con dependencias inyectadas).
//
// Muestra el historial del patrimonio que ya guardan las fotos mensuales
// (`obtener_fotos_patrimonio`): una gráfica SVG propia, una tabla equivalente para quien no
// ve la gráfica, el resumen del último mes y el desglose por partidas de la foto elegida. Lo
// único que escribe es «Tomar foto ahora» (`tomar_foto_patrimonio`, origen `manual`).
//
// Dinero: los importes llegan como **texto** y aquí no pasan por `Number`: la aritmética
// (variaciones, escala del eje) es en centavos enteros (`nucleo/evolucion.ts`) y la gráfica la
// dibuja una función pura (`nucleo/grafica.ts`). Sin reconstrucción del pasado ni recomendaciones.

import type { ApiDe, Avisos, Dom, Enrutador, Pantalla, Vista } from '../ui/servicios';

import { accion, escaparHtml, manejador } from '../nucleo/html.js';
import {
    enCentavos, etiquetaDeSerie, formatearConSigno, formatearPorcentaje, formatearTexto,
    nombreDeMes, ordenarFotos, variacion,
    type Serie, type Variacion,
} from '../nucleo/evolucion.js';
import { generarGrafica } from '../nucleo/grafica.js';

type ApiEvolucion = ApiDe<'obtenerFotosPatrimonio' | 'tomarFotoPatrimonio'>;
type Foto = Awaited<ReturnType<ApiEvolucion['obtenerFotosPatrimonio']>>[number];
type Campo = HTMLInputElement;

export interface DependenciasEvolucion {
    api: ApiEvolucion;
    avisos: Avisos;
    pantalla: Pantalla;
    dom: Dom;
    enrutador: Enrutador;
}

/** Los manejadores que el HTML llama como `appUI.<nombre>`. */
export const MANEJADORES_EVOLUCION = ['handleCambiarSerieEvolucion', 'handleCambiarMesEvolucion', 'handleTomarFotoEvolucion'] as const;

/** La tasa USD→DOP que el titular sugiere para la foto (la misma que usa Rust si no se escribe ninguna). */
export const TASA_SUGERIDA = '60.00';

const ORIGENES: Record<string, string> = {
    real: 'Automática',
    manual: 'Manual',
    capital: 'Al guardar el capital',
    reconstruida: 'Reconstruida',
};
const CALIDADES: Record<string, string> = {
    completa: 'Completa',
    con_aviso: 'Con aviso',
    estimada: 'Estimada',
};

const claveDeMes = (f: { anio: number; mes: number }): string => `${f.anio}-${String(f.mes).padStart(2, '0')}`;

/** Lo que significa un cambio, con signo, flecha y texto (nunca solo color ni solo flecha). */
function celdaDeVariacion(v: Variacion | null): string {
    if (!v) return '<span style="color:var(--text-muted);">—</span>';
    const flecha = v.direccion === 'sube' ? '▲' : v.direccion === 'baja' ? '▼' : '■';
    const texto = v.direccion === 'sube' ? 'sube' : v.direccion === 'baja' ? 'baja' : 'sin cambio';
    const color = v.direccion === 'sube' ? 'var(--color-success)' : v.direccion === 'baja' ? 'var(--color-danger)' : 'var(--text-secondary)';
    const pct = v.centesimas === null ? 'sin base para el %' : formatearPorcentaje(v.centesimas);
    return `<span style="color:${color}; font-weight:600;">${flecha} ${escaparHtml(formatearConSigno(v.delta))} (${escaparHtml(pct)}) <span style="font-weight:400;">· ${texto}</span></span>`;
}

/** Un importe de texto para mostrar; si no se puede leer, el texto tal cual, escapado. */
const importe = (texto: string): string => escaparHtml(formatearTexto(texto));

export class VistaEvolucion implements Vista {
    private serie: Serie = 'total';
    private mesElegido: string | null = null;

    constructor(private readonly dep: DependenciasEvolucion) {}

    async render(): Promise<void> {
        const { api, pantalla } = this.dep;
        const cabecera = `
            <div class="section-title">
                <h1>Evolución del patrimonio</h1>
                <span class="subtitle">Una foto por mes de lo que tienes y lo que debes</span>
            </div>`;
        let fotos: Foto[];
        try {
            fotos = ordenarFotos(await api.obtenerFotosPatrimonio());
        } catch (err) {
            pantalla.contenido.innerHTML = `${cabecera}
                <div class="card" role="alert" style="border-left: 4px solid var(--color-danger);">
                    <h3 style="color: var(--color-danger); margin-bottom: 0.5rem;">No se pudo leer el historial del patrimonio</h3>
                    <p style="font-size: 0.9rem;">${escaparHtml(String(err))}</p>
                </div>`;
            return;
        }

        const elegida = fotos.find(f => claveDeMes(f) === this.mesElegido) ?? fotos[fotos.length - 1] ?? null;
        this.mesElegido = elegida ? claveDeMes(elegida) : null;

        let cuerpo: string;
        if (fotos.length === 0) {
            cuerpo = `<div class="card"><p>Aún no hay fotos del patrimonio. Toma la primera con el botón de abajo; después se toma una sola vez al mes, al abrir la aplicación.</p></div>`;
        } else if (fotos.length === 1) {
            cuerpo = `<div class="card"><p>Hay una sola foto (${escaparHtml(nombreDeMes(fotos[0].anio, fotos[0].mes))}). Se necesita al menos otra foto para mostrar variación, así que todavía no hay gráfica.</p></div>
                ${this.htmlDesglose(fotos, elegida)}`;
        } else {
            cuerpo = `${this.htmlResumen(fotos)}${this.htmlGrafica(fotos)}${this.htmlTabla(fotos)}${this.htmlDesglose(fotos, elegida)}`;
        }

        pantalla.contenido.innerHTML = `${cabecera}${cuerpo}${this.htmlTomarFoto()}`;
    }

    // --- piezas ---

    private htmlResumen(fotos: Foto[]): string {
        const ultima = fotos[fotos.length - 1], anterior = fotos[fotos.length - 2];
        const v = variacion(enCentavos(ultima.patrimonio), enCentavos(anterior.patrimonio));
        return `
            <div class="card" id="evo-resumen" style="margin-bottom:1.5rem;">
                <h3 style="font-family: var(--font-heading); font-size: 1.1rem; margin-bottom: 0.6rem;">Último mes: ${escaparHtml(nombreDeMes(ultima.anio, ultima.mes))}</h3>
                <p style="margin:0 0 0.3rem 0;">Patrimonio total: <strong>${importe(ultima.patrimonio)}</strong> DOP</p>
                <p style="margin:0;">Variación frente a ${escaparHtml(nombreDeMes(anterior.anio, anterior.mes))}: ${celdaDeVariacion(v)}</p>
            </div>`;
    }

    private htmlGrafica(fotos: Foto[]): string {
        const opcion = (valor: Serie): string => `<option value="${valor}"${valor === this.serie ? ' selected' : ''}>${escaparHtml(etiquetaDeSerie(valor))}</option>`;
        return `
            <div class="card" style="margin-bottom:1.5rem;">
                <div style="display:flex; justify-content:space-between; align-items:center; gap:1rem; flex-wrap:wrap; margin-bottom:0.8rem;">
                    <h3 style="font-family: var(--font-heading); font-size: 1.1rem; margin:0;">Gráfica</h3>
                    <label for="evo_serie" style="font-size:0.85rem;">Serie
                        <select id="evo_serie" class="form-control" style="width:auto; display:inline-block; padding:0.3rem 0.6rem; font-size:0.85rem;" ${manejador('change', 'handleCambiarSerieEvolucion', [], 'valor')}>
                            ${opcion('total')}${opcion('liquido')}
                        </select>
                    </label>
                </div>
                ${generarGrafica(fotos, { serie: this.serie })}
                <p style="font-size:0.75rem; color:var(--text-secondary); margin:0.6rem 0 0 0;">● foto real &nbsp; ◇ foto estimada o reconstruida (tramo punteado). Importes en DOP.</p>
            </div>`;
    }

    private htmlTabla(fotos: Foto[]): string {
        const filas = fotos.map((f, i) => {
            const v = i === 0 ? null : variacion(enCentavos(f.patrimonio), enCentavos(fotos[i - 1].patrimonio));
            const aviso = f.calidad === 'con_aviso';
            return `
                <tr>
                    <th scope="row">${escaparHtml(nombreDeMes(f.anio, f.mes))}</th>
                    <td>${escaparHtml(f.fecha)}</td>
                    <td>${escaparHtml(ORIGENES[f.origen] ?? f.origen)}</td>
                    <td style="text-align:right;">${importe(f.activos)}</td>
                    <td style="text-align:right;">${importe(f.pasivos)}</td>
                    <td style="text-align:right;"><strong>${importe(f.patrimonio)}</strong></td>
                    <td>${celdaDeVariacion(v)}</td>
                    <td>${aviso ? '<span class="badge danger">⚠ Con aviso</span>' : escaparHtml(CALIDADES[f.calidad] ?? f.calidad)}</td>
                </tr>`;
        }).reverse().join('');
        return `
            <div class="card" style="margin-bottom:1.5rem;">
                <div class="table-responsive">
                    <table class="table-modern" id="evo-tabla">
                        <caption style="text-align:left; font-family: var(--font-heading); font-size:1.1rem; margin-bottom:0.6rem;">Fotos mensuales del patrimonio (la más reciente primero; importes en DOP)</caption>
                        <thead>
                            <tr>
                                <th scope="col">Mes</th>
                                <th scope="col">Fecha de la foto</th>
                                <th scope="col">Origen</th>
                                <th scope="col" style="text-align:right;">Activos</th>
                                <th scope="col" style="text-align:right;">Pasivos</th>
                                <th scope="col" style="text-align:right;">Patrimonio</th>
                                <th scope="col">Variación vs. foto anterior</th>
                                <th scope="col">Calidad</th>
                            </tr>
                        </thead>
                        <tbody>${filas}</tbody>
                    </table>
                </div>
            </div>`;
    }

    private htmlDesglose(fotos: Foto[], elegida: Foto | null): string {
        if (!elegida) return '';
        const opciones = [...fotos].reverse().map(f =>
            `<option value="${escaparHtml(claveDeMes(f))}"${f === elegida ? ' selected' : ''}>${escaparHtml(nombreDeMes(f.anio, f.mes))}</option>`).join('');
        const filas = elegida.partidas.map(p => `
                <tr>
                    <th scope="row">${escaparHtml(p.rubro)}</th>
                    <td>${escaparHtml(p.referencia)}</td>
                    <td>${escaparHtml(p.divisa)}</td>
                    <td style="text-align:right;">${importe(p.importe)}</td>
                    <td style="text-align:right;">${importe(p.importe_dop)}</td>
                </tr>`).join('');
        const tabla = elegida.partidas.length === 0
            ? '<p>Esta foto no tiene partidas.</p>'
            : `<div class="table-responsive">
                    <table class="table-modern" id="evo-partidas">
                        <caption style="text-align:left; font-size:0.85rem; color:var(--text-secondary); margin-bottom:0.4rem;">Partidas de la foto de ${escaparHtml(nombreDeMes(elegida.anio, elegida.mes))}</caption>
                        <thead><tr>
                            <th scope="col">Rubro</th><th scope="col">Referencia</th><th scope="col">Divisa</th>
                            <th scope="col" style="text-align:right;">Importe</th><th scope="col" style="text-align:right;">Importe en DOP</th>
                        </tr></thead>
                        <tbody>${filas}</tbody>
                    </table>
                </div>`;
        return `
            <div class="card" style="margin-bottom:1.5rem;">
                <div style="display:flex; justify-content:space-between; align-items:center; gap:1rem; flex-wrap:wrap; margin-bottom:0.8rem;">
                    <h3 style="font-family: var(--font-heading); font-size: 1.1rem; margin:0;">Desglose por partidas</h3>
                    <label for="evo_mes" style="font-size:0.85rem;">Mes
                        <select id="evo_mes" class="form-control" style="width:auto; display:inline-block; padding:0.3rem 0.6rem; font-size:0.85rem;" ${manejador('change', 'handleCambiarMesEvolucion', [], 'valor')}>${opciones}</select>
                    </label>
                </div>
                <p style="font-size:0.8rem; color:var(--text-secondary); margin:0 0 0.6rem 0;">Tasa USD→DOP de esta foto: ${escaparHtml(elegida.tasa_usd_dop)}${elegida.sellada ? ' · mes sellado' : ''}</p>
                ${tabla}
            </div>`;
    }

    private htmlTomarFoto(): string {
        return `
            <div class="card">
                <h3 style="font-family: var(--font-heading); font-size: 1.1rem; margin-bottom: 0.6rem;">Tomar foto ahora</h3>
                <p style="font-size:0.85rem; color:var(--text-secondary); margin:0 0 0.8rem 0;">Guarda el estado de hoy como la foto de este mes. Si el mes ya está sellado, no se puede cambiar.</p>
                <div style="display:flex; gap:1rem; align-items:flex-end; flex-wrap:wrap;">
                    <div class="form-group" style="margin:0;">
                        <label for="evo_tasa">Tasa USD→DOP (opcional)</label>
                        <input type="text" id="evo_tasa" class="form-control" inputmode="decimal" autocomplete="off" placeholder="${TASA_SUGERIDA}" style="padding:0.5rem; width:10rem;">
                    </div>
                    <button type="button" class="btn" ${accion('handleTomarFotoEvolucion')}>📸 Tomar foto ahora</button>
                </div>
            </div>`;
    }

    // --- manejadores ---

    async handleCambiarSerieEvolucion(valor: string): Promise<void> {
        this.serie = valor === 'liquido' ? 'liquido' : 'total';
        await this.render();
    }

    async handleCambiarMesEvolucion(valor: string): Promise<void> {
        this.mesElegido = valor;
        await this.render();
    }

    async handleTomarFotoEvolucion(): Promise<void> {
        const { api, avisos, dom, enrutador } = this.dep;
        try {
            const escrita = dom.elemento<Campo>('evo_tasa').value.trim().replace(',', '.');
            if (escrita !== '' && !/^\d+(\.\d+)?$/.test(escrita)) {
                avisos.mostrar('La tasa debe ser un número positivo, por ejemplo 58,75.', 'error');
                return;
            }
            const foto = await api.tomarFotoPatrimonio('manual', escrita === '' ? undefined : escrita);
            this.mesElegido = claveDeMes(foto);
            avisos.mostrar(`Foto de ${nombreDeMes(foto.anio, foto.mes)} guardada: patrimonio ${formatearTexto(foto.patrimonio)} DOP.`);
            await enrutador.mostrar('evolucion');
        } catch (err) {
            const texto = String(err);
            const sellado = /sellad/i.test(texto);
            avisos.mostrar(sellado
                ? `Ese mes ya está sellado y su foto no se puede cambiar. ${texto}`
                : `No se pudo tomar la foto: ${texto}`, 'error');
        }
    }
}

/** Lo que `window.appUI` expone de esta vista. */
export function puenteEvolucion(vista: VistaEvolucion): {
    handleCambiarSerieEvolucion: (valor: string) => Promise<void>;
    handleCambiarMesEvolucion: (valor: string) => Promise<void>;
    handleTomarFotoEvolucion: () => Promise<void>;
} {
    return {
        handleCambiarSerieEvolucion: v => vista.handleCambiarSerieEvolucion(v),
        handleCambiarMesEvolucion: v => vista.handleCambiarMesEvolucion(v),
        handleTomarFotoEvolucion: () => vista.handleTomarFotoEvolucion(),
    };
}
