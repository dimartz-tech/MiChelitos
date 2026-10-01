// Pestaña «Tarjetas de Crédito» (diseño «B»: clase con dependencias inyectadas).
//
// La última vista que salía de `ui.ts`, y la que más mueve dinero: abonos
// (con la tasa de cambio cuando se paga en dólares desde una cuenta en pesos),
// avances de efectivo (que se **simulan en el núcleo** antes de pedir la
// confirmación, para que la cifra que se confirma sea la que se asienta), las
// bonificaciones, la edición de límites y de la política de liquidación, y los
// dos historiales desde los que se deshace un abono o un avance con motivo.
// Recomienda además qué tarjeta conviene usar hoy: la que tiene más días hasta su
// próximo corte, contando desde el reloj inyectado. No usa `AppAPI`, `elemento()`,
// `confirm`, `prompt`, `document` ni `Date` globales: todo entra por el
// constructor (`pruebas/js/vistas/tarjetas.test.js`). Los cuerpos son los de `ui.ts`.

import type { Tarjeta } from '../tipos-ipc';
import type { ApiDe, Avisos, Dialogos, Dom, Enrutador, Formato, Modales, Motivo, Pantalla, Reloj, Vista } from '../ui/servicios';

import { argumentoJs, escaparHtml } from '../nucleo/html.js';

type ApiTarjetas = ApiDe<
    | 'obtenerTarjetas'
    | 'obtenerCuentas'
    | 'obtenerBonificaciones'
    | 'obtenerPrestamos'
    | 'crearBonificacion'
    | 'eliminarBonificacion'
    | 'obtenerAbonosTarjeta'
    | 'revertirAbonoTarjeta'
    | 'registrarPagoTarjeta'
    | 'simularAvanceEfectivo'
    | 'registrarAvanceEfectivo'
    | 'obtenerAvancesTarjeta'
    | 'revertirAvanceEfectivo'
    | 'actualizarLimitesTarjeta'
>;

export interface DependenciasTarjetas {
    api: ApiTarjetas;
    avisos: Avisos;
    formato: Formato;
    enrutador: Enrutador;
    pantalla: Pantalla;
    dom: Dom;
    dialogos: Dialogos;
    motivo: Motivo;
    modales: Modales;
    ahora: Reloj;
}

/** Los manejadores en línea (`onsubmit`/`onclick`/`onchange` → `appUI.…`) que la plantilla escribe. */
export const MANEJADORES_TARJETAS = [
    'handleAbonoTarjeta', 'aplicarTipoAbono',
    'alternarAvance', 'aplicarTipoAvance', 'handleAvanceEfectivo',
    'alternarAbonos', 'handleRevertirAbono', 'alternarAvances', 'handleRevertirAvance',
    'handleAgregarBonificacion', 'handleEliminarBonificacion',
    'abrirEdicionLimitesTarjeta', 'handleEdicionLimitesTarjetaSubmit',
] as const;
export type ManejadorTarjetas = (typeof MANEJADORES_TARJETAS)[number];

/** Un formulario envía un evento con `preventDefault`; solo eso se usa. */
interface EventoDeFormulario {
    preventDefault(): void;
}

/** Cómo se rotula y se colorea el balance de una tarjeta. */
interface EtiquetaDeBalance {
    texto: string;
    color: string;
}

export class VistaTarjetas implements Vista {
    constructor(private readonly dep: DependenciasTarjetas) {}

    // --- RENDER: TARJETAS ---
    async render(): Promise<void> {
        const { api, formato, pantalla, ahora } = this.dep;
        const tarjetas = await api.obtenerTarjetas();
        const cuentas = await api.obtenerCuentas();
        const bonificaciones = await api.obtenerBonificaciones();
        const prestamos = await api.obtenerPrestamos();

        const hoy = ahora();
        const hoyStr = hoy.getDate().toString().padStart(2, '0') + '/' + (hoy.getMonth() + 1).toString().padStart(2, '0') + '/' + hoy.getFullYear();

        const diaActual = hoy.getDate();
        const mesActual = hoy.getMonth();
        const anioActual = hoy.getFullYear();

        let mejorTarjeta = null as import('../tipos-ipc').Tarjeta | null;
        let maxDiasRestantes = -1;

        tarjetas.forEach(t => {
            let diaCorte = t.fecha_corte;
            let fechaCorteEsteMes = new Date(anioActual, mesActual, diaCorte);
            let fechaProximoCorte;
            
            if (diaActual <= diaCorte) {
                fechaProximoCorte = fechaCorteEsteMes;
            } else {
                fechaProximoCorte = new Date(anioActual, mesActual + 1, diaCorte);
            }
            
            const diffMs = fechaProximoCorte.getTime() - hoy.getTime();
            const diffDays = Math.ceil(diffMs / (1000 * 60 * 60 * 24));
            
            (t as import('../tipos-ipc').Tarjeta & { dias_para_corte: number }).dias_para_corte = diffDays;
            
            if (diffDays > maxDiasRestantes) {
                maxDiasRestantes = diffDays;
                mejorTarjeta = t;
            }
        });

        pantalla.contenido.innerHTML = `
            <div class="section-title" style="display:flex; justify-content:space-between; align-items:center; gap:1rem;">
                <div>
                    <h1>Tarjetas de Crédito</h1>
                    <span class="subtitle">Monitoreo de consumos, abonos y alertas de corte (Multidivisa)</span>
                </div>
                <button onclick="navigate('ajustes')" class="btn" style="padding: 0.5rem 1rem; font-size: 0.85rem;">➕ Registrar Tarjeta</button>
            </div>

            <div style="display:flex; flex-direction:column; gap:2rem; width:100%;">
                <!-- Tarjeta Recomendada Hoy -->
                ${mejorTarjeta ? `
                    <div class="card" style="border-left: 5px solid #10b981; background: rgba(16, 185, 129, 0.02); padding: 1rem 1.2rem; width:100%;">
                        <div style="display:flex; justify-content:space-between; align-items:center; gap: 1rem;">
                            <div>
                                <span style="font-size:0.75rem; text-transform:uppercase; color:#10b981; font-weight:bold; letter-spacing:0.05em; display:block; margin-bottom:0.15rem;">💡 Tarjeta Recomendada Hoy</span>
                                <strong style="font-size:1.05rem; font-family:var(--font-heading); display:block;">${escaparHtml(mejorTarjeta.entidad)} - ${escaparHtml(mejorTarjeta.nombre_tarjeta)}</strong>
                                <p style="font-size:0.75rem; color:var(--text-secondary); margin-top:0.2rem; line-height:1.3;">
                                    Próximo corte en <strong>${maxDiasRestantes} días</strong> (Día ${mejorTarjeta.fecha_corte}). Usarla hoy te otorga la mayor cantidad de días de financiamiento sin intereses.
                                </p>
                            </div>
                            <span style="font-size: 1.8rem; opacity: 0.85;">💡</span>
                        </div>
                    </div>
                ` : ''}

                <!-- Grilla de Tarjetas (Ancho Completo) -->
                ${tarjetas.length > 0 ? `
                    <div style="display:grid; grid-template-columns: repeat(auto-fill, minmax(340px, 1fr)); gap:1.5rem; width:100%;">
                        ${tarjetas.map(t => {
                            const limiteTotalDop = t.limite_efectivo_pesos + t.limite_sobregiro_pesos;
                            const disponibleDop = t.disponible_pesos;
                            const pctDop = this.porcentajeUso(t.balance_pesos, limiteTotalDop);

                            const limiteTotalUsd = t.limite_efectivo_dolares + t.limite_sobregiro_dolares;
                            const disponibleUsd = t.disponible_dolares;
                            const pctUsd = this.porcentajeUso(t.balance_dolares, limiteTotalUsd);

                            const balDop = this.etiquetaBalanceTarjeta(t.balance_pesos, 'DOP');
                            const balUsd = this.etiquetaBalanceTarjeta(t.balance_dolares, 'USD');

                            const tarjetaAlDia = (t.balance_corte_pesos <= 0 && t.balance_corte_dolares <= 0);

                            // Una facilidad que cuelga de esta tarjeta se cobra dentro de su
                            // pago. Su saldo se contabiliza aparte en el patrimonio, así que
                            // si además entrara en el balance de la tarjeta se contaría dos
                            // veces. El aviso está aquí, junto al abono, porque es el momento
                            // en que se decide qué cifra registrar.
                            const facilidades = prestamos.filter(p => p.tarjeta_id === t.id);

                            return `
                                <div class="card" style="display:flex; flex-direction:column; gap:0.8rem;">
                                    <div style="display:flex; justify-content:space-between; border-bottom:1px solid var(--border-color); padding-bottom:0.5rem; align-items:center;">
                                        <div>
                                            <strong>${escaparHtml(t.entidad)}</strong>
                                            <div style="font-size:0.75rem; color:var(--text-muted);">${escaparHtml(t.nombre_tarjeta)}</div>
                                        </div>
                                        <div>
                                            ${tarjetaAlDia ? `
                                                <span class="badge pagada" style="font-size:0.7rem;">✔️ Tarjeta al día</span>
                                            ` : `
                                                <span class="badge" style="font-size:0.7rem; background:rgba(255, 69, 58, 0.15); color:#ff453a;">Corte: DOP ${formato.importe(t.balance_corte_pesos)} / USD ${formato.importe(t.balance_corte_dolares)}</span>
                                            `}
                                        </div>
                                    </div>

                                    ${facilidades.length > 0 ? `
                                        <div style="background:rgba(255,193,7,0.08); border:1px solid rgba(255,193,7,0.3); border-radius:var(--radius-sm); padding:0.5rem 0.6rem; font-size:0.7rem; color:var(--text-secondary);">
                                            ⚠️ Esta tarjeta cobra
                                            ${facilidades.map(f => `<strong>${escaparHtml(f.institucion_financiera)}</strong> (cuota DOP ${formato.importe(f.monto_cuota)})`).join(', ')}.
                                            Al conciliar el balance, no incluyas la cuota si ya cuenta como saldo de la facilidad:
                                            se duplicaría en el patrimonio.
                                        </div>
                                    ` : ''}

                                    <!-- Pesos Section -->
                                    <div>
                                        <div style="display:flex; justify-content:space-between; font-size:0.75rem; color:var(--text-secondary); margin-bottom:0.2rem;">
                                            <span>🇩🇴 DOP (Uso: ${pctDop.toFixed(1)}%)</span>
                                            <span style="color:${escaparHtml(balDop.color)};">${escaparHtml(balDop.texto)}</span>
                                        </div>
                                        <div style="width:100%; height:6px; background:rgba(255,255,255,0.05); border-radius:3px; overflow:hidden; margin-bottom:0.3rem;">
                                            <div style="width: ${pctDop}%; height:100%; background: ${pctDop > 85 ? '#ff453a' : 'var(--accent-primary)'};"></div>
                                        </div>
                                        <div style="display:flex; justify-content:space-between; font-size:0.75rem; color:var(--text-muted);">
                                            <span>Disp: DOP ${formato.importe(disponibleDop)}</span>
                                            <span title="${t.limite_ajustado_pesos != null ? `Aprobado por el banco: DOP ${formato.importe(t.limite_pesos)}` : ''}">${t.limite_ajustado_pesos != null ? '🔒 ' : ''}Lím: DOP ${formato.importe(limiteTotalDop)}</span>
                                        </div>
                                    </div>

                                    <!-- Dólares Section -->
                                    <div>
                                        <div style="display:flex; justify-content:space-between; font-size:0.75rem; color:var(--text-secondary); margin-bottom:0.2rem;">
                                            <span>🇺🇸 USD (Uso: ${pctUsd.toFixed(1)}%)</span>
                                            <span style="color:${escaparHtml(balUsd.color)};">${escaparHtml(balUsd.texto)}</span>
                                        </div>
                                        <div style="width:100%; height:6px; background:rgba(255,255,255,0.05); border-radius:3px; overflow:hidden; margin-bottom:0.3rem;">
                                            <div style="width: ${pctUsd}%; height:100%; background: ${pctUsd > 85 ? '#ff453a' : '#10b981'};"></div>
                                        </div>
                                        <div style="display:flex; justify-content:space-between; font-size:0.75rem; color:var(--text-muted);">
                                            <span>Disp: USD ${formato.importe(disponibleUsd)}</span>
                                            <span title="${t.limite_ajustado_dolares != null ? `Aprobado por el banco: USD ${formato.importe(t.limite_dolares)}` : ''}">${t.limite_ajustado_dolares != null ? '🔒 ' : ''}Lím: USD ${formato.importe(limiteTotalUsd)}</span>
                                        </div>
                                    </div>

                                    <!-- Alertas -->
                                    <div style="display:flex; flex-direction:column; gap:0.3rem;">
                                        <div class="alert-banner ${t.alerta_corte ? 'warning' : 'info'}" style="padding:0.4rem 0.6rem; font-size:0.75rem;">
                                            <span>📅</span>
                                            <div>${escaparHtml(t.dias_corte_msg)}</div>
                                        </div>
                                        <div class="alert-banner ${t.alerta_pago ? 'danger' : 'info'}" style="padding:0.4rem 0.6rem; font-size:0.75rem;">
                                            <span>🚨</span>
                                            <div>${escaparHtml(t.dias_pago_msg)}</div>
                                        </div>
                                    </div>

                                    <!-- Abono rápido -->
                                    <form onsubmit="appUI.handleAbonoTarjeta(event, ${t.id})" style="display:flex; flex-direction:column; gap:0.5rem; border-top:1px dashed var(--border-color); padding-top:0.8rem; margin-top:0.3rem;">
                                        <div style="display:flex; gap:0.4rem; align-items:flex-end; width:100%;">
                                            <div style="flex:1.2;">
                                                <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Fecha</label>
                                                <input type="text" id="pag_fecha_${t.id}" value="${escaparHtml(hoyStr)}" placeholder="dd/mm/aaaa" class="form-control" style="padding:0.4rem; font-size:0.75rem;" required>
                                            </div>
                                            <div style="flex:1;">
                                                <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Divisa</label>
                                                <select id="pag_div_${t.id}" class="form-control" style="padding:0.4rem; font-size:0.75rem;" onchange="appUI.aplicarTipoAbono(${t.id}, ${t.balance_corte_pesos}, ${t.balance_corte_dolares}, ${t.balance_pesos}, ${t.balance_dolares})">
                                                    <option value="DOP">DOP</option>
                                                    <option value="USD">USD</option>
                                                </select>
                                            </div>
                                            <div style="flex:1.2;">
                                                <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Monto</label>
                                                <input type="number" step="0.01" id="pag_monto_${t.id}" placeholder="0.00" class="form-control" style="padding:0.4rem; font-size:0.75rem;" required>
                                            </div>
                                        </div>
                                        <div style="width:100%;">
                                            <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Tipo de abono</label>
                                            <select id="pag_tipo_${t.id}" class="form-control" style="padding:0.4rem; font-size:0.75rem; width:100%;" onchange="appUI.aplicarTipoAbono(${t.id}, ${t.balance_corte_pesos}, ${t.balance_corte_dolares}, ${t.balance_pesos}, ${t.balance_dolares})">
                                                <option value="personalizado">Personalizado</option>
                                                <option value="corte">Pago al corte</option>
                                                <option value="actual">Saldo del balance actual</option>
                                            </select>
                                        </div>
                                        <div style="display:flex; gap:0.4rem; align-items:flex-end; width:100%;">
                                            <div style="flex:2;">
                                                <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Cuenta Débito (Opcional)</label>
                                                <select id="pag_cuenta_${t.id}" class="form-control" style="padding:0.4rem; font-size:0.75rem; width:100%;">
                                                    <option value="">-- Ninguna (Efectivo/Otro) --</option>
                                                    ${cuentas.map(c => `<option value="${c.id}" data-divisa="${escaparHtml(c.divisa)}">${escaparHtml(c.nombre)} (${escaparHtml(c.divisa)}) - Bal: ${escaparHtml(c.divisa)} ${formato.importe(c.balance_actual)}</option>`).join('')}
                                                </select>
                                            </div>
                                            <div style="flex:1;">
                                                <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Tasa Cambio</label>
                                                <input type="number" step="0.01" id="pag_tasa_${t.id}" value="0.00" placeholder="0.00" class="form-control" style="padding:0.4rem; font-size:0.75rem;">
                                            </div>
                                            <button type="submit" class="btn" style="padding:0.4rem 0.6rem; font-size:0.75rem; height:fit-content; background: linear-gradient(135deg, #10b981, #059669); color:white; flex:1;">Abonar</button>
                                        </div>
                                    </form>

                                    <!-- Abonos registrados -->
                                    <button onclick="appUI.alternarAbonos(${t.id})" class="btn" style="padding:0.3rem; font-size:0.75rem; background:rgba(255,255,255,0.02); border:1px solid var(--border-color); color:var(--text-secondary); width:100%;">🧾 Abonos registrados</button>
                                    <div id="abonos_${t.id}" hidden style="font-size:0.75rem;"></div>

                                    <!-- Avance de efectivo -->
                                    <button onclick="appUI.alternarAvance(${t.id})" class="btn" style="padding:0.3rem; font-size:0.75rem; background:rgba(255,255,255,0.02); border:1px solid var(--border-color); color:var(--text-secondary); width:100%;">💵 Avance de efectivo</button>
                                    <form id="avance_${t.id}" hidden onsubmit="appUI.handleAvanceEfectivo(event, ${t.id})" style="font-size:0.75rem;">
                                        <div style="display:flex; flex-direction:column; gap:0.5rem;">
                                            <p style="font-size:0.7rem; color:var(--text-secondary); margin:0;">
                                                La tarjeta pone el dinero en una cuenta de ahorro. La deuda sube por el monto <strong>y</strong> su cargo; la cuenta recibe el monto sin el cargo.
                                            </p>
                                            <div style="display:flex; gap:0.4rem; align-items:flex-end;">
                                                <div style="flex:1.2;">
                                                    <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Fecha</label>
                                                    <input type="text" id="avc_fecha_${t.id}" value="${escaparHtml(hoyStr)}" placeholder="dd/mm/aaaa" class="form-control" style="padding:0.4rem; font-size:0.75rem;" required>
                                                </div>
                                                <div style="flex:1;">
                                                    <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Divisa</label>
                                                    <select id="avc_div_${t.id}" class="form-control" style="padding:0.4rem; font-size:0.75rem;" onchange="appUI.aplicarTipoAvance(${t.id})">
                                                        <option value="DOP">DOP</option>
                                                        <option value="USD">USD</option>
                                                    </select>
                                                </div>
                                                <div style="flex:1.2;">
                                                    <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Monto</label>
                                                    <input type="number" step="0.01" id="avc_monto_${t.id}" placeholder="0.00" class="form-control" style="padding:0.4rem; font-size:0.75rem;" required>
                                                </div>
                                            </div>
                                            <div>
                                                <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Cuenta que recibe *</label>
                                                <select id="avc_cuenta_${t.id}" class="form-control" style="padding:0.4rem; font-size:0.75rem; width:100%;" required>
                                                    <option value="">-- Selecciona --</option>
                                                    ${cuentas.map(c => `<option value="${c.id}" data-divisa="${escaparHtml(c.divisa)}">${escaparHtml(c.nombre)} (${escaparHtml(c.divisa)}) - Bal: ${escaparHtml(c.divisa)} ${formato.importe(c.balance_actual)}</option>`).join('')}
                                                </select>
                                            </div>
                                            <div style="display:flex; gap:0.4rem; align-items:flex-end;">
                                                <div style="flex:1.3;">
                                                    <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Cargo</label>
                                                    <select id="avc_tipo_${t.id}" class="form-control" style="padding:0.4rem; font-size:0.75rem; width:100%;" onchange="appUI.aplicarTipoAvance(${t.id})">
                                                        <option value="porcentaje">Porcentaje</option>
                                                        <option value="fijo">Monto fijo</option>
                                                        <option value="exonerado">Exonerado</option>
                                                    </select>
                                                </div>
                                                <div id="avc_valor_caja_${t.id}" style="flex:1;">
                                                    <label id="avc_valor_et_${t.id}" style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Porcentaje (%)</label>
                                                    <input type="number" step="0.01" id="avc_valor_${t.id}" placeholder="6.25" class="form-control" style="padding:0.4rem; font-size:0.75rem;">
                                                </div>
                                            </div>
                                            <div>
                                                <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Nota (opcional; por qué se exoneró, por ejemplo)</label>
                                                <input type="text" id="avc_nota_${t.id}" class="form-control" style="padding:0.4rem; font-size:0.75rem;">
                                            </div>
                                            <button type="submit" class="btn" style="padding:0.4rem 0.6rem; font-size:0.75rem; background: linear-gradient(135deg, #6366f1, #4f46e5); color:white;">Registrar avance</button>
                                        </div>
                                    </form>
                                    <button onclick="appUI.alternarAvances(${t.id})" class="btn" style="padding:0.3rem; font-size:0.75rem; background:rgba(255,255,255,0.02); border:1px solid var(--border-color); color:var(--text-secondary); width:100%;">🧾 Avances registrados</button>
                                    <div id="avances_${t.id}" hidden style="font-size:0.75rem;"></div>

                                    <!-- Configurar límites -->
                                    <button onclick="appUI.abrirEdicionLimitesTarjeta(${argumentoJs(JSON.stringify(t))})" class="btn" style="padding:0.3rem; font-size:0.75rem; background:rgba(255,255,255,0.02); border:1px solid var(--border-color); color:var(--text-secondary); width:100%;">⚙️ Configurar Límites / Corte</button>
                                </div>
                            `;
                        }).join('')}
                    </div>
                ` : `
                    <p style="color:var(--text-muted); text-align:center; padding:2rem; background:var(--bg-surface); border:1px solid var(--border-color); border-radius:var(--radius-md);">No hay tarjetas registradas.</p>
                `}

                ${tarjetas.length > 0 ? `
                <div class="card" style="margin-top:1.2rem;">
                    <h3 style="font-family:var(--font-heading); margin-bottom:0.3rem;">🎁 Bonificaciones recibidas</h3>
                    <p style="font-size:0.75rem; color:var(--text-secondary); margin-bottom:1rem;">
                        Cashback, devoluciones promocionales y recompensas. Se registran como crédito aparte y reducen la deuda de la tarjeta, sin alterar el consumo que las generó.
                        Un mismo consumo puede recibir varias.
                    </p>

                    <form onsubmit="appUI.handleAgregarBonificacion(event)" style="display:flex; gap:0.5rem; flex-wrap:wrap; align-items:flex-end; margin-bottom:1rem;">
                        <div style="flex:1; min-width:110px;">
                            <label style="font-size:0.7rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Fecha</label>
                            <input type="text" id="bon_fecha" value="${escaparHtml(hoyStr)}" placeholder="dd/mm/aaaa" class="form-control" style="padding:0.4rem; font-size:0.78rem;" required>
                        </div>
                        <div style="flex:1.6; min-width:170px;">
                            <label style="font-size:0.7rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Tarjeta</label>
                            <select id="bon_tarjeta" class="form-control" style="padding:0.4rem; font-size:0.78rem;" required>
                                ${tarjetas.map(t => `<option value="${t.id}">${escaparHtml(t.entidad)} - ${escaparHtml(t.nombre_tarjeta)}</option>`).join('')}
                            </select>
                        </div>
                        <div style="flex:0.8; min-width:80px;">
                            <label style="font-size:0.7rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Divisa</label>
                            <select id="bon_divisa" class="form-control" style="padding:0.4rem; font-size:0.78rem;">
                                <option value="DOP">DOP</option><option value="USD">USD</option>
                            </select>
                        </div>
                        <div style="flex:1; min-width:100px;">
                            <label style="font-size:0.7rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Monto</label>
                            <input type="number" step="0.01" id="bon_monto" placeholder="0.00" class="form-control" style="padding:0.4rem; font-size:0.78rem;" required>
                        </div>
                        <div style="flex:2; min-width:190px;">
                            <label style="font-size:0.7rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Concepto</label>
                            <input type="text" id="bon_concepto" list="conceptos_bonificacion" placeholder="Cashback compra por internet..." class="form-control" style="padding:0.4rem; font-size:0.78rem;" required>
                            <datalist id="conceptos_bonificacion">
                                <option value="Cashback compra por internet"></option>
                                <option value="Cashback Personalizado"></option>
                                <option value="Cashback Promocional"></option>
                                <option value="Recompensas Qik Rebate"></option>
                                <option value="Devolución promocional"></option>
                            </datalist>
                        </div>
                        <button type="submit" class="btn" style="padding:0.45rem 0.9rem; font-size:0.78rem; background:linear-gradient(135deg, #10b981, #059669); color:white;">Registrar</button>
                    </form>

                    ${bonificaciones.length > 0 ? `
                        <div style="max-height:240px; overflow-y:auto;">
                        <table class="data-table" style="font-size:0.78rem;">
                            <thead><tr><th>Fecha</th><th>Tarjeta</th><th>Concepto</th><th style="text-align:right;">Monto</th><th></th></tr></thead>
                            <tbody>
                                ${bonificaciones.map(b => `
                                    <tr>
                                        <td>${escaparHtml(b.fecha)}</td>
                                        <td>${escaparHtml(b.entidad)} (${escaparHtml(b.nombre_tarjeta)})</td>
                                        <td>${escaparHtml(b.concepto)}</td>
                                        <td class="amount" style="text-align:right; color:var(--color-success);">+${escaparHtml(b.divisa)} ${formato.importe(b.monto)}</td>
                                        <td><button onclick="appUI.handleEliminarBonificacion(${b.id})" class="btn btn-danger" style="padding:0.2rem 0.4rem; font-size:0.7rem;">🗑️</button></td>
                                    </tr>
                                `).join('')}
                            </tbody>
                        </table>
                        </div>
                        <p style="font-size:0.75rem; color:var(--text-secondary); margin-top:0.6rem;">
                            Total acreditado: ${['DOP','USD'].map(d => {
                                const suma = bonificaciones.filter(b => b.divisa === d).reduce((s,b) => s + b.monto, 0);
                                return suma > 0 ? `<strong>${escaparHtml(d)} ${formato.importe(suma)}</strong>` : '';
                            }).filter(Boolean).join(' · ') || '—'}
                        </p>
                    ` : `
                        <p style="color:var(--text-muted); font-size:0.8rem; text-align:center; padding:0.8rem;">Aún no has registrado bonificaciones.</p>
                    `}
                </div>
                ` : ''}
            </div>
        `;
    }

    async handleAgregarBonificacion(e: EventoDeFormulario): Promise<void> {
        e.preventDefault();
        const { api, avisos, enrutador, dom } = this.dep;
        const fecha = dom.elemento<Campo>('bon_fecha').value.trim();
        const tarjeta = Number(dom.elemento<Campo>('bon_tarjeta').value);
        const divisa = dom.elemento<Campo>('bon_divisa').value;
        // Texto, tal cual se escribió: el céntimo lo deciden los dígitos (convención de 1.21.0).
        const monto = dom.elemento<Campo>('bon_monto').value.trim();
        const concepto = dom.elemento<Campo>('bon_concepto').value.trim();

        // El número solo sirve para comparar: lo que viaja son los dígitos escritos.
        if (!(Number(monto) > 0)) { avisos.mostrar("El monto de la bonificación debe ser mayor que cero.", "error"); return; }
        if (!concepto) { avisos.mostrar("Indica el concepto: distingue un cashback de una promoción o recompensa.", "error"); return; }

        try {
            await api.crearBonificacion(fecha, tarjeta, monto, divisa, concepto);
            avisos.mostrar("Bonificación registrada. La deuda de la tarjeta se redujo.");
            await enrutador.mostrar('tarjetas');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    async handleEliminarBonificacion(id: number): Promise<void> {
        const { api, avisos, enrutador } = this.dep;
        try {
            await api.eliminarBonificacion(id);
            avisos.mostrar("Bonificación revertida. La deuda vuelve a su valor anterior.");
            await enrutador.mostrar('tarjetas');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    /**
     * Rellena el monto del abono según el tipo elegido y la divisa activa.
     * El importe queda visible antes de confirmar, en lugar de resolverse de
     * forma opaca al enviar el formulario.
     */
    aplicarTipoAbono(id: number, cortePesos: number, corteDolares: number, balPesos: number, balDolares: number): void {
        const { avisos, formato, dom } = this.dep;
        const tipo = dom.elemento<Campo>(`pag_tipo_${id}`).value;
        const divisa = dom.elemento<Campo>(`pag_div_${id}`).value;
        const campoMonto = dom.elemento<HTMLInputElement>(`pag_monto_${id}`);

        if (tipo === 'personalizado') {
            campoMonto.readOnly = false;
            return;
        }

        const esDolares = divisa === 'USD';
        const saldo = tipo === 'corte'
            ? (esDolares ? corteDolares : cortePesos)
            : (esDolares ? balDolares : balPesos);

        const etiqueta = tipo === 'corte' ? 'al corte' : 'actual';

        // Con saldo a favor no hay nada que abonar: prefijar el negativo solo
        // conseguiría que el envío fallara con "el monto debe ser mayor que
        // cero", que no explica lo que de verdad ocurre.
        if (Number(saldo) < 0) {
            campoMonto.value = '0.00';
            campoMonto.readOnly = true;
            avisos.mostrar(
                `La tarjeta tiene ${divisa} ${formato.importe(Math.abs(saldo))} a favor. No hay saldo ${etiqueta} que abonar.`,
                'info'
            );
            return;
        }

        campoMonto.value = Number(saldo).toFixed(2);
        campoMonto.readOnly = true;

        if (Number(saldo) === 0) {
            avisos.mostrar(`No hay saldo ${etiqueta} en ${divisa}.`, 'info');
        }
    }

    /**
     * Despliega los abonos de una tarjeta para poder deshacer uno.
     *
     * Se cargan al abrir y no al pintar la vista: son un histórico que casi
     * nunca se mira, y traerlos para las siete tarjetas a la vez sería pagar
     * siempre por lo que se usa de vez en cuando.
     */
    async alternarAbonos(id: number): Promise<void> {
        const { api, formato, dom } = this.dep;
        const caja = dom.buscar(`abonos_${id}`);
        if (!caja) return;

        if (!caja.hidden) {
            caja.hidden = true;
            return;
        }

        caja.hidden = false;
        caja.innerHTML = `<p style="color:var(--text-muted); padding:0.5rem;">Cargando…</p>`;

        try {
            const abonos = await api.obtenerAbonosTarjeta(id);
            if (abonos.length === 0) {
                caja.innerHTML = `<p style="color:var(--text-muted); padding:0.5rem;">Sin abonos registrados.</p>`;
                return;
            }

            caja.innerHTML = abonos.map(a => {
                // Un abono sin cuenta no movió ningún saldo de ahorro, y
                // deshacerlo tampoco lo hará. Decirlo evita que alguien
                // espere una devolución que no va a llegar.
                const origen = a.cuenta_nombre
                    ? a.cuenta_nombre
                    : 'sin cuenta asociada';
                const tasa = a.tasa_cambio && a.tasa_cambio !== 1
                    ? ` · tasa ${a.tasa_cambio}`
                    : '';
                return `
                    <div style="display:flex; justify-content:space-between; align-items:center; gap:0.5rem; padding:0.4rem 0.5rem; border-bottom:1px solid var(--border-color);">
                        <div>
                            <strong>${escaparHtml(a.divisa)} ${formato.importe(a.monto_pagado)}</strong>
                            <div style="font-size:0.7rem; color:var(--text-muted);">${escaparHtml(a.fecha_pago)} · ${escaparHtml(origen)}${escaparHtml(tasa)}</div>
                        </div>
                        <button onclick="appUI.handleRevertirAbono(${a.id}, ${id})" class="btn btn-danger" style="padding:0.2rem 0.45rem; font-size:0.7rem;" title="Deshacer este abono">↩︎</button>
                    </div>`;
            }).join('');
        } catch (err) {
            caja.innerHTML = `<p style="color:var(--color-danger); padding:0.5rem;">${escaparHtml(String(err))}</p>`;
        }
    }

    /**
     * Deshace un abono, avisando de todo lo que va a mover.
     *
     * La confirmación enumera los tres efectos porque un abono no es una
     * fila: revertirlo repone deuda, devuelve dinero y borra la comisión.
     */
    async handleRevertirAbono(abonoId: number, tarjetaId: number): Promise<void> {
        const { api, avisos, enrutador, dialogos, motivo: pedidorDeMotivo } = this.dep;
        const confirmado = await dialogos.confirmar(
            "¿Deshacer este abono?\n\n" +
            "Se repondrá la deuda de la tarjeta, volverá a la cuenta el importe con su comisión, " +
            "y se eliminará el gasto que la recogía.\n\n" +
            "El registro del abono desaparece."
        );
        if (!confirmado) return;

        try {
            const motivo = await pedidorDeMotivo.pedir(
                    `Vas a borrar este abono a tarjeta`,
                    'Esto **destruye el movimiento**: no queda un asiento que lo anule, solo el caso de auditoría que estás a punto de abrir.'
                );
            if (motivo === null) return;
            const resumen = await api.revertirAbonoTarjeta(abonoId, motivo);
            avisos.mostrar(resumen);
            await enrutador.mostrar('tarjetas');
            // Se vuelve a abrir el desplegable para que se vea el resultado
            // en lugar de dejar al usuario frente a una tarjeta cerrada.
            await this.alternarAbonos(tarjetaId);
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    /// Despliega el formulario de avance de una tarjeta.
    alternarAvance(id: number): void {
        const { dom } = this.dep;
        const forma = dom.buscar(`avance_${id}`);
        if (!forma) return;
        forma.hidden = !forma.hidden;
        if (!forma.hidden) this.aplicarTipoAvance(id);
    }

    /// Ajusta el formulario a la divisa y al tipo de cargo elegidos.
    ///
    /// Solo se ofrecen las cuentas de la divisa del avance: el núcleo lo exige
    /// y es mejor no dejar elegir lo que se va a rechazar. Que la cuenta y el
    /// avance coincidan en divisa es una regla; aquí solo se evita el
    /// tropiezo, y el núcleo la sigue comprobando.
    aplicarTipoAvance(id: number): void {
        const { dom } = this.dep;
        const divisa = dom.elemento<Campo>(`avc_div_${id}`).value;
        const tipo = dom.elemento<Campo>(`avc_tipo_${id}`).value;
        const cuenta = dom.elemento<HTMLSelectElement>(`avc_cuenta_${id}`);

        for (const opcion of cuenta.options) {
            if (!opcion.value) continue;
            const coincide = opcion.dataset.divisa === divisa;
            opcion.hidden = !coincide;
            opcion.disabled = !coincide;
        }
        if (cuenta.selectedOptions[0]?.disabled) cuenta.value = '';

        const caja = dom.elemento(`avc_valor_caja_${id}`);
        const etiqueta = dom.elemento(`avc_valor_et_${id}`);
        const valor = dom.elemento<HTMLInputElement>(`avc_valor_${id}`);
        caja.hidden = tipo === 'exonerado';
        valor.required = tipo !== 'exonerado';
        if (tipo === 'porcentaje') {
            etiqueta.textContent = 'Porcentaje (%)';
            valor.placeholder = '6.25';
        } else if (tipo === 'fijo') {
            etiqueta.textContent = `Cargo fijo (${divisa})`;
            valor.placeholder = '0.00';
        } else {
            valor.value = '';
        }
    }

    /// Lo que el formulario dice del cargo, en la forma que el núcleo espera.
    ///
    /// El monto y el cargo fijo se mandan **como texto**, tal cual se
    /// escribieron; el porcentaje, que es una tasa y no un importe, como
    /// número.
    private valoresDeAvance(id: number): { tipo: string; porcentaje: number | null; fijo: string | null } {
        const { dom } = this.dep;
        const tipo = dom.elemento<Campo>(`avc_tipo_${id}`).value;
        const bruto = dom.elemento<Campo>(`avc_valor_${id}`).value.trim();
        return {
            tipo,
            porcentaje: tipo === 'porcentaje' && bruto !== '' ? Number(bruto) : null,
            fijo: tipo === 'fijo' && bruto !== '' ? bruto : null,
        };
    }

    /// Registra un avance, enseñando antes lo que va a mover.
    ///
    /// El cargo se calcula en el núcleo —`simularAvanceEfectivo`—, no aquí:
    /// una regla en el HTML es una regla sin pruebas, y la cifra que se
    /// confirma tiene que ser exactamente la que se va a asentar.
    async handleAvanceEfectivo(e: EventoDeFormulario, id: number): Promise<void> {
        e.preventDefault();
        const { api, avisos, formato, enrutador, dom, dialogos } = this.dep;
        const fecha = dom.elemento<Campo>(`avc_fecha_${id}`).value.trim();
        const divisa = dom.elemento<Campo>(`avc_div_${id}`).value;
        const monto = dom.elemento<Campo>(`avc_monto_${id}`).value.trim();
        const cuentaSel = dom.elemento<HTMLSelectElement>(`avc_cuenta_${id}`);
        const cuentaId = cuentaSel.value;
        const nota = dom.elemento<Campo>(`avc_nota_${id}`).value.trim();
        const { tipo, porcentaje, fijo } = this.valoresDeAvance(id);

        if (!cuentaId) {
            avisos.mostrar("Elige la cuenta que recibe el avance.", "error");
            return;
        }

        try {
            const sim = await api.simularAvanceEfectivo(monto, divisa, tipo, porcentaje, fijo);
            const cargoTxt = tipo === 'porcentaje' ? `Cargo (${porcentaje}%)`
                : tipo === 'fijo' ? 'Cargo fijo'
                : 'Cargo (exonerado)';
            const ok = await dialogos.confirmar(
                `Avance de efectivo\n\n` +
                `Monto: ${divisa} ${formato.importe(sim.monto)} → ${cuentaSel.selectedOptions[0].text.split(' - ')[0]}\n` +
                `${cargoTxt}: ${divisa} ${formato.importe(sim.cargo)}\n` +
                `La deuda de la tarjeta sube: ${divisa} ${formato.importe(sim.a_la_tarjeta)}\n\n` +
                `¿Registrar?`
            );
            if (!ok) return;

            const resumen = await api.registrarAvanceEfectivo(
                id, cuentaId, fecha, monto, divisa, tipo, porcentaje, fijo, nota
            );
            avisos.mostrar(resumen);
            await enrutador.mostrar('tarjetas');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    /// Despliega los avances de una tarjeta para poder deshacer uno.
    ///
    /// Se cargan al abrir y no al pintar la vista, igual que los abonos: es un
    /// histórico que casi nunca se mira.
    async alternarAvances(id: number): Promise<void> {
        const { api, formato, dom } = this.dep;
        const caja = dom.buscar(`avances_${id}`);
        if (!caja) return;
        if (!caja.hidden) {
            caja.hidden = true;
            return;
        }
        caja.hidden = false;
        caja.innerHTML = `<p style="color:var(--text-muted); padding:0.5rem;">Cargando…</p>`;

        try {
            const avances = await api.obtenerAvancesTarjeta(id);
            if (avances.length === 0) {
                caja.innerHTML = `<p style="color:var(--text-muted); padding:0.5rem;">Sin avances registrados.</p>`;
                return;
            }
            caja.innerHTML = avances.map(a => {
                const cargo = a.tipo_cargo === 'porcentaje' ? `cargo ${a.tasa}% = ${a.divisa} ${formato.importe(a.cargo)}`
                    : a.tipo_cargo === 'fijo' ? `cargo fijo ${a.divisa} ${formato.importe(a.cargo)}`
                    : 'exonerado';
                const nota = a.nota ? `<div style="font-size:0.7rem; color:var(--text-muted); font-style:italic;">${escaparHtml(a.nota)}</div>` : '';
                return `
                    <div style="display:flex; justify-content:space-between; align-items:center; gap:0.5rem; padding:0.4rem 0.5rem; border-bottom:1px solid var(--border-color);">
                        <div>
                            <strong>${escaparHtml(a.divisa)} ${formato.importe(a.monto)}</strong>
                            <div style="font-size:0.7rem; color:var(--text-muted);">${escaparHtml(a.fecha)} · ${escaparHtml(a.cuenta_nombre)} · ${escaparHtml(cargo)}</div>
                            ${nota}
                        </div>
                        <button onclick="appUI.handleRevertirAvance(${a.id}, ${id})" class="btn btn-danger" style="padding:0.2rem 0.45rem; font-size:0.7rem;" title="Deshacer este avance">↩︎</button>
                    </div>`;
            }).join('');
        } catch (err) {
            caja.innerHTML = `<p style="color:var(--color-danger); padding:0.5rem;">${escaparHtml(String(err))}</p>`;
        }
    }

    /// Deshace un avance, avisando de todo lo que va a mover.
    async handleRevertirAvance(avanceId: number, tarjetaId: number): Promise<void> {
        const { api, avisos, enrutador, dialogos, motivo: pedidorDeMotivo } = this.dep;
        const confirmado = await dialogos.confirmar(
            "¿Deshacer este avance de efectivo?\n\n" +
            "La deuda de la tarjeta bajará por el monto y su cargo, la cuenta devolverá el monto, " +
            "y se eliminará el gasto que recogía el cargo.\n\n" +
            "Si ya gastaste ese dinero, la cuenta quedará en negativo: es el estado verdadero, y no se recorta."
        );
        if (!confirmado) return;

        try {
            const motivo = await pedidorDeMotivo.pedir(
                `Vas a borrar este avance de efectivo`,
                'Esto **destruye el movimiento**: no queda un asiento que lo anule, solo el caso de auditoría que estás a punto de abrir.'
            );
            if (motivo === null) return;
            const resumen = await api.revertirAvanceEfectivo(avanceId, motivo);
            avisos.mostrar(resumen);
            await enrutador.mostrar('tarjetas');
            await this.alternarAvances(tarjetaId);
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    async handleAbonoTarjeta(e: EventoDeFormulario, id: number): Promise<void> {
        e.preventDefault();
        const { api, avisos, enrutador, dom, dialogos } = this.dep;

        const fec = dom.elemento<Campo>(`pag_fecha_${id}`).value;
        const div = dom.elemento<Campo>(`pag_div_${id}`).value;
        const mon = Number(dom.elemento<Campo>(`pag_monto_${id}`).value);
        const cueId = dom.elemento<Campo>(`pag_cuenta_${id}`).value;

        if (!(mon > 0)) {
            avisos.mostrar("El monto del abono debe ser mayor que cero.", "error");
            return;
        }

        let tasaCambio = Number(dom.elemento<Campo>(`pag_tasa_${id}`).value) || 0;
        if (cueId) {
            try {
                const cuentas = await api.obtenerCuentas();
                const cuenta = cuentas.find(c => c.id === Number(cueId));
                if (cuenta && cuenta.divisa === "DOP" && div === "USD") {
                    if (tasaCambio <= 0) {
                        const promptVal = await dialogos.preguntar(`Estás realizando un abono de USD ${mon} desde la cuenta en Pesos "${cuenta.nombre}".\nPor favor, ingresa la tasa de cambio (DOP por 1 USD):`, "60.0");
                        if (promptVal === null) {
                            avisos.mostrar("Operación cancelada.", "info");
                            return;
                        }
                        tasaCambio = Number(promptVal);
                        if (isNaN(tasaCambio) || tasaCambio <= 0) {
                            avisos.mostrar("Tasa de cambio inválida.", "error");
                            return;
                        }
                    }
                }
            } catch (err) {
                avisos.mostrar("Error al validar cuenta: " + String(err), 'error');
                return;
            }
        }

        try {
            await api.registrarPagoTarjeta(id, fec, mon, div, cueId ? Number(cueId) : null, tasaCambio);
            avisos.mostrar("Abono a tarjeta guardado.");
            await enrutador.mostrar('tarjetas');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    abrirEdicionLimitesTarjeta(tJsonStr: string): void {
        const { modales } = this.dep;
        const t: Tarjeta = JSON.parse(tJsonStr);
        const html = `
            <div class="card" style="width: 450px; background: var(--bg-surface-opaque); max-height:90vh; overflow-y:auto;">
                <h3 style="font-family: var(--font-heading); margin-bottom: 0.6rem;">⚙️ Configurar Límites / Corte</h3>
                <p style="font-size:0.75rem; color:var(--text-secondary); margin-bottom:1rem;">${escaparHtml(t.entidad)} - ${escaparHtml(t.nombre_tarjeta)}</p>
                <form onsubmit="appUI.handleEdicionLimitesTarjetaSubmit(event, ${t.id})">
                    <h4 style="font-size:0.85rem; color:var(--accent-primary); margin-bottom:0.5rem; border-bottom:1px solid var(--border-color); padding-bottom:0.2rem;">Pesos (DOP)</h4>
                    <div class="form-row">
                        <div class="form-group">
                            <label>Límite DOP</label>
                            <input type="number" step="0.01" id="edit_lim_dop_${t.id}" class="form-control" value="${t.limite_pesos}">
                        </div>
                        <div class="form-group">
                            <label>Sobregiro DOP</label>
                            <input type="number" step="0.01" id="edit_sob_dop_${t.id}" class="form-control" value="${t.limite_sobregiro_pesos}">
                        </div>
                    </div>
                    <div class="form-group">
                        <label>Límite ajustado DOP <span style="font-weight:400; color:var(--text-muted);">— opcional</span></label>
                        <input type="number" step="0.01" id="edit_aju_dop_${t.id}" class="form-control" placeholder="Sin ajuste" value="${t.limite_ajustado_pesos ?? ''}">
                        <span style="font-size:0.68rem; color:var(--text-muted);">Tope que te impones por debajo del aprobado. Déjalo vacío si no lo usas.</span>
                    </div>
                    <div class="form-group">
                        <label>Balance Corte DOP</label>
                        <input type="number" step="0.01" id="edit_cor_dop_${t.id}" class="form-control" value="${t.balance_corte_pesos}">
                    </div>

                    <h4 style="font-size:0.85rem; color:#10b981; margin-top:1rem; margin-bottom:0.5rem; border-bottom:1px solid var(--border-color); padding-bottom:0.2rem;">Dólares (USD)</h4>
                    <div class="form-row">
                        <div class="form-group">
                            <label>Límite USD</label>
                            <input type="number" step="0.01" id="edit_lim_usd_${t.id}" class="form-control" value="${t.limite_dolares}">
                        </div>
                        <div class="form-group">
                            <label>Sobregiro USD</label>
                            <input type="number" step="0.01" id="edit_sob_usd_${t.id}" class="form-control" value="${t.limite_sobregiro_dolares}">
                        </div>
                    </div>
                    <div class="form-group">
                        <label>Límite ajustado USD <span style="font-weight:400; color:var(--text-muted);">— opcional</span></label>
                        <input type="number" step="0.01" id="edit_aju_usd_${t.id}" class="form-control" placeholder="Sin ajuste" value="${t.limite_ajustado_dolares ?? ''}">
                    </div>
                    <div class="form-group">
                        <label>Balance Corte USD</label>
                        <input type="number" step="0.01" id="edit_cor_usd_${t.id}" class="form-control" value="${t.balance_corte_dolares}">
                    </div>

                    <h4 style="font-size:0.85rem; color:var(--text-secondary); margin-top:1rem; margin-bottom:0.5rem; border-bottom:1px solid var(--border-color); padding-bottom:0.2rem;">Compras en divisa</h4>
                    <div class="form-group">
                        <label>¿Cómo liquida el emisor las compras en dólares?</label>
                        <select id="edit_pol_${t.id}" class="form-control">
                            <option value="origen" ${t.politica_liquidacion !== 'traduce' ? 'selected' : ''}>Se quedan en dólares</option>
                            <option value="traduce" ${t.politica_liquidacion === 'traduce' ? 'selected' : ''}>Las traduce a pesos días después</option>
                        </select>
                        <span style="font-size:0.68rem; color:var(--text-muted);">Si las traduce, el consumo queda pendiente hasta que indiques el importe en pesos que aparezca en tu estado.</span>
                    </div>

                    <div style="display:flex; justify-content:flex-end; gap:0.5rem; margin-top:1.2rem;">
                        <button type="button" onclick="elemento('modal-edit-tar-${t.id}').remove()" class="btn btn-secondary">Cancelar</button>
                        <button type="submit" class="btn">Guardar Parámetros</button>
                    </div>
                </form>
            </div>
        `;
        modales.abrir(`modal-edit-tar-${t.id}`, html);
    }

    async handleEdicionLimitesTarjetaSubmit(e: EventoDeFormulario, id: number): Promise<void> {
        e.preventDefault();
        const { api, avisos, enrutador, dom } = this.dep;
        const limDop = Number(dom.elemento<Campo>(`edit_lim_dop_${id}`).value);
        const sobDop = Number(dom.elemento<Campo>(`edit_sob_dop_${id}`).value);
        const corDop = Number(dom.elemento<Campo>(`edit_cor_dop_${id}`).value);
        const limUsd = Number(dom.elemento<Campo>(`edit_lim_usd_${id}`).value);
        const sobUsd = Number(dom.elemento<Campo>(`edit_sob_usd_${id}`).value);
        const corUsd = Number(dom.elemento<Campo>(`edit_cor_usd_${id}`).value);

        // Vacío es "sin ajuste"; cero es un tope deliberado. Se leen como texto
        // para no confundir ambos casos.
        const ajuDopTexto = dom.elemento<Campo>(`edit_aju_dop_${id}`).value.trim();
        const ajuUsdTexto = dom.elemento<Campo>(`edit_aju_usd_${id}`).value.trim();
        const ajuDop = ajuDopTexto === '' ? null : Number(ajuDopTexto);
        const ajuUsd = ajuUsdTexto === '' ? null : Number(ajuUsdTexto);

        if ((ajuDop !== null && ajuDop > limDop) || (ajuUsd !== null && ajuUsd > limUsd)) {
            avisos.mostrar("El límite ajustado no puede superar al aprobado.", "error");
            return;
        }

        try {
            const politica = dom.buscar<Campo>(`edit_pol_${id}`)?.value || 'origen';
            await api.actualizarLimitesTarjeta(id, limDop, limUsd, sobDop, sobUsd, corDop, corUsd, ajuDop, ajuUsd, politica);
            avisos.mostrar("Parámetros actualizados correctamente.");
            dom.elemento(`modal-edit-tar-${id}`).remove();
            await enrutador.mostrar('tarjetas');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    /// Un balance de tarjeta negativo es saldo a favor del titular, no un uso
    /// negativo. Rotularlo "Uso: DOP -100.00" invita a leerlo como un error.
    private etiquetaBalanceTarjeta(balance: number, divisa: string): EtiquetaDeBalance {
        const { formato } = this.dep;
        const esAFavor = Number(balance) < 0;
        return {
            texto: `${esAFavor ? 'A favor' : 'Uso'}: ${divisa} ${formato.importe(Math.abs(balance))}`,
            color: esAFavor ? '#10b981' : 'inherit',
        };
    }

    /// Porcentaje de uso acotado a [0, 100] para dibujar la barra. Un saldo a
    /// favor da negativo y una barra con anchura negativa no se renderiza.
    private porcentajeUso(balance: number, limiteTotal: number): number {
        if (!(limiteTotal > 0)) return 0;
        return Math.min(Math.max((balance / limiteTotal) * 100, 0), 100);
    }
}

/**
 * Lo que `window.appUI` expone de esta vista: un puente que delega en la
 * instancia. Los atributos en línea no cambian.
 */
export function puenteTarjetas(vista: VistaTarjetas): {
    handleAbonoTarjeta: (e: EventoDeFormulario, id: number) => Promise<void>;
    aplicarTipoAbono: (id: number, cortePesos: number, corteDolares: number, balPesos: number, balDolares: number) => void;
    alternarAvance: (id: number) => void;
    aplicarTipoAvance: (id: number) => void;
    handleAvanceEfectivo: (e: EventoDeFormulario, id: number) => Promise<void>;
    alternarAbonos: (id: number) => Promise<void>;
    handleRevertirAbono: (abonoId: number, tarjetaId: number) => Promise<void>;
    alternarAvances: (id: number) => Promise<void>;
    handleRevertirAvance: (avanceId: number, tarjetaId: number) => Promise<void>;
    handleAgregarBonificacion: (e: EventoDeFormulario) => Promise<void>;
    handleEliminarBonificacion: (id: number) => Promise<void>;
    abrirEdicionLimitesTarjeta: (tJsonStr: string) => void;
    handleEdicionLimitesTarjetaSubmit: (e: EventoDeFormulario, id: number) => Promise<void>;
} {
    return {
        handleAbonoTarjeta: (e, id) => vista.handleAbonoTarjeta(e, id),
        aplicarTipoAbono: (id, cortePesos, corteDolares, balPesos, balDolares) => vista.aplicarTipoAbono(id, cortePesos, corteDolares, balPesos, balDolares),
        alternarAvance: id => vista.alternarAvance(id),
        aplicarTipoAvance: id => vista.aplicarTipoAvance(id),
        handleAvanceEfectivo: (e, id) => vista.handleAvanceEfectivo(e, id),
        alternarAbonos: id => vista.alternarAbonos(id),
        handleRevertirAbono: (abonoId, tarjetaId) => vista.handleRevertirAbono(abonoId, tarjetaId),
        alternarAvances: id => vista.alternarAvances(id),
        handleRevertirAvance: (avanceId, tarjetaId) => vista.handleRevertirAvance(avanceId, tarjetaId),
        handleAgregarBonificacion: e => vista.handleAgregarBonificacion(e),
        handleEliminarBonificacion: id => vista.handleEliminarBonificacion(id),
        abrirEdicionLimitesTarjeta: tJsonStr => vista.abrirEdicionLimitesTarjeta(tJsonStr),
        handleEdicionLimitesTarjetaSubmit: (e, id) => vista.handleEdicionLimitesTarjetaSubmit(e, id),
    };
}
