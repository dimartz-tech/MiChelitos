// Las dos cifras del patrimonio, rotuladas, para Resumen y Dashboard.
//
// Las compone el núcleo (`obtener_patrimonio`, en centavos enteros); aquí solo se muestran. Durante una versión
// conviven la cifra vigente («capital − deudas») y la nueva («incluyendo cuentas y efectivo») para que el titular
// las compare. La cifra que cada pantalla ya tenía **no se toca**: esto se añade debajo.
//
// Todo texto que viene de los datos pasa por `escaparHtml`; no hay manejadores en línea.

import type { Patrimonio } from '../tipos-ipc';
import type { Formato } from '../ui/servicios';

import { escaparHtml } from '../nucleo/html.js';

/** Consulta tolerante: si el comando falla, la pantalla sigue mostrando todo lo demás. */
export async function consultarPatrimonio(api: { obtenerPatrimonio?: () => Promise<Patrimonio> }): Promise<Patrimonio | null> {
    try {
        return api.obtenerPatrimonio ? await api.obtenerPatrimonio() : null;
    } catch {
        return null;
    }
}

export function htmlCifrasDePatrimonio(p: Patrimonio | null, formato: Formato): string {
    if (!p) {
        return `
            <div class="card" data-patrimonio="no-disponible" style="margin-bottom:1rem; font-size:0.8rem; color:var(--text-muted);">
                No se pudo calcular el patrimonio con cuentas y efectivo.
            </div>`;
    }
    const dolares = (p.liquidez_usd !== '0.00' || p.tarjetas_usd !== '0.00')
        ? ` Incluye USD ${formato.importe(p.liquidez_usd)} en cuentas y USD ${formato.importe(p.tarjetas_usd)} de tarjetas, a ${p.tasa_usd_a_dop} pesos por dólar (tasa estimada, no una cotización).`
        : '';
    return `
            <div class="card" data-patrimonio="cifras" style="margin-bottom:1rem;">
                <div style="display:flex; justify-content:space-between; align-items:flex-start; flex-wrap:wrap; gap:1rem;">
                    <div>
                        <span style="font-size:0.8rem; color:var(--text-secondary);">Patrimonio neto (capital − deudas)</span>
                        <div class="amount" style="font-size:1.4rem;">DOP ${escaparHtml(formato.importe(p.neto_actual))}</div>
                    </div>
                    <div>
                        <span style="font-size:0.8rem; color:var(--text-secondary);">Patrimonio neto incluyendo cuentas y efectivo</span>
                        <div class="amount" style="font-size:1.4rem;">DOP ${escaparHtml(formato.importe(p.neto_con_liquidez))}</div>
                    </div>
                </div>
                <p style="margin:0.6rem 0 0; font-size:0.75rem; color:var(--text-muted);">
                    La segunda cifra suma además cuentas de ahorro (DOP ${escaparHtml(formato.importe(p.liquidez_dop))}, de ellas efectivo DOP ${escaparHtml(formato.importe(p.efectivo_dop))}) y los dólares de cuentas convertidos (DOP ${escaparHtml(formato.importe(p.liquidez_usd_en_dop))}).${escaparHtml(dolares)}
                </p>
            </div>`;
}
