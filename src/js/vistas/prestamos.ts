// Pestaña «Financiamientos y Deudas» (diseño «B»: clase con dependencias inyectadas).
//
// Préstamos, líneas de crédito y las tarjetas que los cobran, vistos como un solo
// pasivo: cabecera con el total, la carga mensual y el próximo vencimiento; un
// grupo por tarjeta con sus facilidades colgando de ella, y los independientes.
// Aquí viven las cifras que se calculan en la vista (`resumirPasivos`,
// `proximoVencimiento`, `agruparPasivosPorAcreedor`), que ahora se pueden probar
// sin dibujar. El menú de acciones de cada fila usa el servicio `MenuFlotante`
// (antes `document`, `window`, `setTimeout` y un `AbortController` sueltos en la
// clase). No usa `AppAPI`, `elemento()`, `document`, `confirm`, `prompt` ni
// `Date` globales: todo entra por el constructor (`pruebas/js/vistas/prestamos.test.js`).
// Los cuerpos son los de `ui.ts`.

import type { Prestamo, Tarjeta } from '../tipos-ipc';
import type { ApiDe, Avisos, Dialogos, Dom, Enrutador, Formato, MenuFlotante, Modales, Pantalla, Referencias, Reloj, Vista } from '../ui/servicios';

import { argumentoJs, escaparHtml } from '../nucleo/html.js';

type ApiPrestamos = ApiDe<
    | 'obtenerPrestamos'
    | 'obtenerTarjetas'
    | 'crearPrestamo'
    | 'actualizarPrestamo'
    | 'declararSaldoPrestamo'
    | 'pagarCuotaPrestamo'
    | 'eliminarPrestamo'
>;

export interface DependenciasPrestamos {
    api: ApiPrestamos;
    avisos: Avisos;
    formato: Formato;
    enrutador: Enrutador;
    pantalla: Pantalla;
    dom: Dom;
    dialogos: Dialogos;
    modales: Modales;
    menus: MenuFlotante;
    referencias: Referencias;
    ahora: Reloj;
}

/** Los manejadores que `window.appUI` expone de esta vista: los de la plantilla y los del menú de acciones. */
export const MANEJADORES_PRESTAMOS = [
    'handleAgregarPrestamo', 'toggleCamposPrestamos', 'abrirMenuPasivo',
    'handleEdicionPrestamo', 'avisarFechasDerivadas',
    'handlePagarCuota', 'handleDeclararSaldo', 'handleEliminarPrestamo',
] as const;
export type ManejadorPrestamos = (typeof MANEJADORES_PRESTAMOS)[number];

/** Un formulario envía un evento con `preventDefault`; solo eso se usa. */
interface EventoDeFormulario {
    preventDefault(): void;
}

/** El clic en el botón de acciones: se usa para no cerrar el menú al abrirlo y para colocarlo. */
interface EventoDeMenu {
    stopPropagation(): void;
    currentTarget: { getBoundingClientRect(): Pick<DOMRect, 'right' | 'top' | 'bottom'> } | null;
}

interface TramoDeComposicion {
    etiqueta: string;
    color: string;
    monto: number;
}

interface ResumenPasivos {
    total: number;
    cargaMensual: number;
    cupoDisponible: number;
    proximoVencimiento: { dia: number; enDias: number } | null;
    composicion: (TramoDeComposicion & { porcentaje: number })[];
}

interface GrupoDeTarjeta {
    tarjeta: Tarjeta;
    facilidades: Prestamo[];
}

export class VistaPrestamos implements Vista {
    constructor(private readonly dep: DependenciasPrestamos) {}

    // --- RENDER: PRESTAMOS ---
    async render(): Promise<void> {
        const { api, pantalla, dom } = this.dep;
        const prestamos = await api.obtenerPrestamos();
        const tarjetas = await api.obtenerTarjetas();

        const resumen = this.resumirPasivos(prestamos, tarjetas);
        const grupos = this.agruparPasivosPorAcreedor(prestamos, tarjetas);

        pantalla.contenido.innerHTML = `
            <div class="section-title">
                <h1>Financiamientos y Deudas</h1>
                <span class="subtitle">Monitoreo de deudas, cuotas de préstamos y vencimientos mensuales</span>
            </div>

            ${this.plantillaResumenPasivos(resumen)}

            <div class="responsive-split-grid">
                <!-- Formulario -->
                <div class="card" style="height:fit-content;">
                    <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 1rem;">📝 Registrar Financiamiento</h3>
                    <form id="form-add-prestamo" onsubmit="appUI.handleAgregarPrestamo(event)">
                        <div class="form-group">
                            <label for="pre_tip">Tipo de Préstamo *</label>
                            <select id="pre_tip" class="form-control" onchange="appUI.toggleCamposPrestamos(this.value)" required>
                                <option value="consumo" selected>Préstamo de Consumo</option>
                                <option value="hipotecario">Préstamo Hipotecario</option>
                                <option value="vehiculo">Préstamo de Vehículo</option>
                                <option value="flexible">Préstamo Flexible (Línea)</option>
                            </select>
                        </div>
                        <div class="form-group">
                            <label for="pre_ins">Institución Financiera *</label>
                            <input type="text" id="pre_ins" class="form-control" placeholder="Banco BHD, Popular..." required>
                        </div>
                        <div class="form-row">
                            <div class="form-group">
                                <label for="pre_mon">Monto Préstamo *</label>
                                <input type="number" step="0.01" id="pre_mon" class="form-control" placeholder="0.00" required>
                            </div>
                            <div class="form-group">
                                <label for="pre_tas">Tasa Actual (%) *</label>
                                <input type="number" step="0.01" id="pre_tas" class="form-control" placeholder="14.5" required>
                            </div>
                        </div>

                        <!-- Campos condicionales -->
                        <div id="pre_campos_cuotas" class="form-row">
                            <div class="form-group">
                                <label for="pre_tot">Cuotas Totales *</label>
                                <input type="number" id="pre_tot" class="form-control" placeholder="36" required>
                            </div>
                            <div class="form-group">
                                <label for="pre_pen">Cuotas Pendientes *</label>
                                <input type="number" id="pre_pen" class="form-control" placeholder="24" required>
                            </div>
                        </div>

                        <div class="form-row">
                            <div class="form-group">
                                <label for="pre_sal" title="Capital que se debe hoy. En blanco si el financiamiento acaba de desembolsarse.">Saldo actual</label>
                                <input type="number" step="0.01" id="pre_sal" class="form-control" placeholder="Igual al monto">
                            </div>
                            <div id="pre_grupo_limite" class="form-group" style="display:none;">
                                <label for="pre_lim" title="Cupo aprobado. Al pagar, la diferencia con el saldo vuelve a quedar disponible.">Límite de crédito</label>
                                <input type="number" step="0.01" id="pre_lim" class="form-control" placeholder="0.00">
                            </div>
                        </div>

                        <div class="form-row">
                            <div class="form-group">
                                <label for="pre_cuo">Monto Cuota *</label>
                                <input type="number" step="0.01" id="pre_cuo" class="form-control" placeholder="0.00" required>
                            </div>
                            <div class="form-group">
                                <label for="pre_dia">Día del mes Vence *</label>
                                <input type="number" min="1" max="31" id="pre_dia" class="form-control" placeholder="15" required>
                            </div>
                        </div>

                        <button type="submit" class="btn" style="width:100%; margin-top:0.5rem;">🚀 Registrar Deuda</button>
                    </form>
                </div>

                <!-- Listado agrupado por acreedor -->
                <div style="display:flex; flex-direction:column; gap:1.25rem;">
                    ${grupos.tarjetas.map(g => this.plantillaGrupoTarjeta(g)).join('')}
                    ${grupos.independientes.length > 0 ? this.plantillaGrupoIndependientes(grupos.independientes) : ''}
                    ${prestamos.length === 0 && grupos.tarjetas.length === 0 ? `
                        <div class="card"><p style="color:var(--text-muted); text-align:center; padding:2rem; margin:0;">No hay deudas registradas.</p></div>
                    ` : ''}
                </div>
            </div>
        `;

        // El formulario se pinta con un tipo ya seleccionado, así que el
        // `onchange` no llega a dispararse. Sin esta llamada, los campos
        // condicionales se quedan en un estado que no corresponde al tipo
        // mostrado — y el límite de la línea resultaba imposible de registrar.
        this.toggleCamposPrestamos(dom.buscar<Campo>('pre_tip')?.value ?? 'consumo');
    }

    /**
     * Cifras de cabecera: cuánto se debe, cuánto sale al mes y cuánto queda.
     *
     * Suma los saldos tal como están registrados. Cuando una facilidad cuelga
     * de una tarjeta, su saldo y el balance de esa tarjeta **pueden solaparse**
     * —si el emisor carga la cuota al corte—, y entonces el total estaría
     * contando de más. No se corrige aquí: la corrección depende de una
     * observación que aún no está confirmada, y restar a ciegas produciría el
     * error contrario. El grupo lo advierte donde se decide qué cifra usar.
     */
    private resumirPasivos(prestamos: Prestamo[], tarjetas: Tarjeta[]): ResumenPasivos {
        const { referencias } = this.dep;
        const enPesos = (t: Tarjeta) => t.balance_pesos + (t.balance_dolares * referencias.tasaUsdADop);

        const totalFinanciamientos = prestamos.reduce((s, p) => s + p.saldo_actual, 0);
        const totalTarjetas = tarjetas.reduce((s, t) => s + enPesos(t), 0);

        const cargaMensual = prestamos.reduce(
            (s, p) => s + (p.es_revolvente || (p.cuotas_pendientes ?? 0) > 0 ? p.monto_cuota : 0), 0);

        const cupoLineas = prestamos.reduce((s, p) => s + (p.disponible ?? 0), 0);
        const cupoTarjetas = tarjetas.reduce(
            (s, t) => s + t.disponible_pesos + (t.disponible_dolares * referencias.tasaUsdADop), 0);

        // Composición por naturaleza del pasivo, no por acreedor: es la
        // pregunta que responde un desglose de una sola barra.
        const porTipo = new Map();
        const acumular = (etiqueta: string, color: string, monto: number) => {
            if (!(monto > 0)) return;
            const previo = porTipo.get(etiqueta);
            if (previo) previo.monto += monto;
            else porTipo.set(etiqueta, { etiqueta, color, monto });
        };
        const COLORES: Record<string, string> = {
            hipotecario: '#3b82f6', vehiculo: '#6366f1',
            consumo: '#8b5cf6', flexible: 'var(--color-warning)',
        };
        prestamos.forEach(p => acumular(
            p.es_revolvente ? 'Líneas' : this.nombreTipo(p.tipo_prestamo),
            COLORES[p.tipo_prestamo] ?? '#6366f1',
            p.saldo_actual));
        acumular('Tarjetas', 'var(--accent-primary)', totalTarjetas);

        const composicion = [...porTipo.values()].sort((a, b) => b.monto - a.monto);
        const total = totalFinanciamientos + totalTarjetas;

        return {
            total,
            cargaMensual,
            cupoDisponible: cupoLineas + cupoTarjetas,
            proximoVencimiento: this.proximoVencimiento(prestamos, tarjetas),
            composicion: composicion.map(c => ({ ...c, porcentaje: total > 0 ? (c.monto / total) * 100 : 0 })),
        };
    }

    /**
     * Día del mes que vence antes, contando desde hoy.
     *
     * Se compara la distancia en días y no el número del día: el 3 vence antes
     * que el 25 si hoy es 28, y ordenar por el número diría lo contrario.
     */
    private proximoVencimiento(prestamos: Prestamo[], tarjetas: Tarjeta[]): { dia: number; enDias: number } | null {
        const { ahora } = this.dep;
        const hoy = ahora().getDate();
        const distancia = (dia: number) => (dia >= hoy ? dia - hoy : (30 - hoy) + dia);

        const dias = [
            ...prestamos.filter(p => p.es_revolvente || (p.cuotas_pendientes ?? 0) > 0).map(p => p.dia_pago),
            ...tarjetas.map(t => t.fecha_limite_pago),
        ].filter(d => Number.isFinite(d) && d > 0);

        if (dias.length === 0) return null;
        const dia = dias.reduce((a, b) => (distancia(a) <= distancia(b) ? a : b));
        return { dia, enDias: distancia(dia) };
    }

    /**
     * Reparte los pasivos entre las tarjetas que los cobran y los sueltos.
     *
     * Es la estructura de la vista: una facilidad vinculada no se lista aparte
     * con una nota que diga de quién cuelga — se coloca dentro de su tarjeta,
     * que es lo que hace imposible mirar una sin ver la otra.
     */
    private agruparPasivosPorAcreedor(prestamos: Prestamo[], tarjetas: Tarjeta[]): { tarjetas: GrupoDeTarjeta[]; independientes: Prestamo[] } {
        const conFacilidades = tarjetas
            .map(t => ({ tarjeta: t, facilidades: prestamos.filter(p => p.tarjeta_id === t.id) }))
            .filter(g => g.facilidades.length > 0);

        return {
            tarjetas: conFacilidades,
            independientes: prestamos.filter(p => p.tarjeta_id == null),
        };
    }

    /**
     * Nombre presentable de un tipo de financiamiento.
     *
     * Los códigos de la tabla van sin tilde porque son identificadores; la
     * interfaz no debe heredar esa restricción y mostrar «Vehiculo».
     */
    private nombreTipo(codigo: string): string {
        return ({
            consumo: 'Consumo',
            hipotecario: 'Hipotecario',
            vehiculo: 'Vehículo',
            flexible: 'Línea de crédito',
        })[codigo] ?? String(codigo).charAt(0).toUpperCase() + String(codigo).slice(1);
    }

    /**
     * Nombre completo del financiamiento, con su preposición.
     *
     * No se compone como «Préstamo de » + tipo: «hipotecario» es un adjetivo y
     * daría «Préstamo de hipotecario». Cada nombre se escribe entero.
     */
    private nombreFinanciamiento(codigo: string): string {
        return ({
            consumo: 'Préstamo de consumo',
            hipotecario: 'Préstamo hipotecario',
            vehiculo: 'Préstamo de vehículo',
            flexible: 'Línea de crédito',
        })[codigo] ?? `Préstamo (${codigo})`;
    }

    private plantillaResumenPasivos(r: ResumenPasivos): string {
        const { formato } = this.dep;
        if (!(r.total > 0)) return '';
        return `
            <div class="card" style="margin-bottom:1.5rem;">
                <div style="display:flex; justify-content:space-between; align-items:flex-end; flex-wrap:wrap; gap:1.5rem; margin-bottom:1.2rem;">
                    <div>
                        <div style="font-size:0.8rem; color:var(--text-secondary); margin-bottom:0.25rem;">Pasivo total</div>
                        <div style="font-family:var(--font-heading); font-size:2.2rem; font-weight:800; letter-spacing:-0.02em; line-height:1;">DOP ${formato.importe(r.total)}</div>
                    </div>
                    <div style="display:flex; gap:2rem; text-align:right;">
                        <div>
                            <div style="font-size:0.72rem; color:var(--text-muted);">Carga mensual</div>
                            <div style="font-family:var(--font-heading); font-size:1.15rem; font-weight:700; color:#fca5a5;">${formato.importe(r.cargaMensual)}</div>
                        </div>
                        <div>
                            <div style="font-size:0.72rem; color:var(--text-muted);">Cupo disponible</div>
                            <div style="font-family:var(--font-heading); font-size:1.15rem; font-weight:700; color:var(--color-success);">${formato.importe(r.cupoDisponible)}</div>
                        </div>
                        ${r.proximoVencimiento ? `
                            <div>
                                <div style="font-size:0.72rem; color:var(--text-muted);">Próximo vence</div>
                                <div style="font-family:var(--font-heading); font-size:1.15rem; font-weight:700; color:#fcd34d;">Día ${r.proximoVencimiento.dia}</div>
                            </div>
                        ` : ''}
                    </div>
                </div>

                <div style="display:flex; height:12px; border-radius:6px; overflow:hidden; gap:2px; margin-bottom:0.75rem;">
                    ${r.composicion.map(c => `
                        <div title="${escaparHtml(c.etiqueta)}: DOP ${formato.importe(c.monto)}" style="width:${c.porcentaje}%; background:${escaparHtml(c.color)}; transition:filter var(--transition-fast);"></div>
                    `).join('')}
                </div>
                <div style="display:flex; gap:1.5rem; flex-wrap:wrap; font-size:0.72rem; color:var(--text-secondary);">
                    ${r.composicion.map(c => `
                        <div style="display:flex; align-items:center; gap:0.4rem;">
                            <span style="width:8px; height:8px; border-radius:2px; background:${escaparHtml(c.color)};"></span>
                            ${escaparHtml(c.etiqueta)} · ${formato.importe(c.monto)}
                        </div>
                    `).join('')}
                </div>
            </div>
        `;
    }

    private plantillaGrupoTarjeta(g: GrupoDeTarjeta): string {
        const { formato, referencias } = this.dep;
        const t = g.tarjeta;
        const suma = t.balance_pesos + (t.balance_dolares * referencias.tasaUsdADop)
                   + g.facilidades.reduce((s, f) => s + f.saldo_actual, 0);

        return `
            <div class="card">
                <div style="display:flex; justify-content:space-between; align-items:center; padding-bottom:0.9rem; border-bottom:1px solid var(--border-color);">
                    <div style="display:flex; align-items:center; gap:0.75rem;">
                        <div style="width:38px; height:26px; border-radius:5px; background:rgba(0,242,254,0.12); border:1px solid rgba(0,242,254,0.28); display:flex; align-items:center; justify-content:center;">
                            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--accent-primary)" stroke-width="1.7"><rect x="2" y="5" width="20" height="14" rx="2"/><path d="M2 10h20"/></svg>
                        </div>
                        <div>
                            <div style="font-family:var(--font-heading); font-size:1rem; font-weight:700;">${escaparHtml(t.nombre_tarjeta)}</div>
                            <div style="font-size:0.72rem; color:var(--text-muted);">${escaparHtml(t.entidad)} · Corte día ${t.fecha_corte} · Pago día ${t.fecha_limite_pago}</div>
                        </div>
                    </div>
                    <div style="text-align:right;">
                        <div style="font-size:0.72rem; color:var(--text-muted);">Suma de lo que cuelga aquí</div>
                        <div style="font-family:var(--font-heading); font-size:1.15rem; font-weight:700;">DOP ${formato.importe(suma)}</div>
                    </div>
                </div>

                <div style="display:flex; align-items:center; gap:1rem; padding:0.8rem 0.25rem; border-bottom:1px solid rgba(255,255,255,0.05);">
                    <div style="width:26px;"></div>
                    <div style="flex:1.5;">
                        <div style="font-size:0.85rem; font-weight:600;">Balance de la tarjeta</div>
                        <div style="font-size:0.72rem; color:var(--text-muted);">Consumos del ciclo</div>
                    </div>
                    <div style="flex:1; text-align:right;">
                        <div style="font-size:0.9rem; font-weight:600;">DOP ${formato.importe(t.balance_pesos)}</div>
                        <div style="font-size:0.72rem; color:var(--color-success);">Disponible ${formato.importe(t.disponible_pesos)}</div>
                    </div>
                    <div style="flex:0.7; text-align:right; font-size:0.78rem; color:var(--text-muted);">Día ${t.fecha_limite_pago}</div>
                    <div style="width:30px;"></div>
                </div>

                ${g.facilidades.map(f => this.plantillaFilaFinanciamiento(f, true)).join('')}

                <div style="background:rgba(245,158,11,0.07); border:1px solid rgba(245,158,11,0.2); border-radius:var(--radius-sm); padding:0.6rem 0.75rem; font-size:0.72rem; color:var(--text-secondary); margin-top:0.75rem; display:flex; align-items:flex-start; gap:0.5rem; line-height:1.5;">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--color-warning)" stroke-width="1.8" stroke-linecap="round" style="flex-shrink:0; margin-top:1px;"><path d="M12 9v4M12 17h.01"/><path d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z"/></svg>
                    <span>Al conciliar el balance de esta tarjeta, no incluyas la cuota de ${g.facilidades.length > 1 ? 'sus facilidades' : 'su facilidad'}: se contaría dos veces en el patrimonio.</span>
                </div>
            </div>
        `;
    }

    private plantillaGrupoIndependientes(prestamos: Prestamo[]): string {
        const { formato } = this.dep;
        const suma = prestamos.reduce((s, p) => s + p.saldo_actual, 0);
        return `
            <div class="card">
                <div style="display:flex; justify-content:space-between; align-items:center; padding-bottom:0.9rem; border-bottom:1px solid var(--border-color);">
                    <div style="font-family:var(--font-heading); font-size:0.95rem; font-weight:700; color:var(--text-secondary);">Financiamientos independientes</div>
                    <div style="text-align:right;">
                        <div style="font-size:0.72rem; color:var(--text-muted);">Suma</div>
                        <div style="font-family:var(--font-heading); font-size:1.15rem; font-weight:700;">DOP ${formato.importe(suma)}</div>
                    </div>
                </div>
                ${prestamos.map(p => this.plantillaFilaFinanciamiento(p, false)).join('')}
            </div>
        `;
    }

    /**
     * Una fila de financiamiento. `anidada` la dibuja colgando de su tarjeta.
     *
     * El progreso significa cosas distintas según el tipo, y por eso la barra
     * cambia de color y de referencia: en un amortizable mide capital
     * amortizado; en una línea, cupo consumido. Dibujar la misma barra para
     * ambos sugeriría que se leen igual.
     */
    private plantillaFilaFinanciamiento(p: Prestamo, anidada: boolean): string {
        const { formato } = this.dep;
        const usa = p.es_revolvente && p.limite_credito !== null && p.limite_credito > 0
            ? Math.min(Math.max((p.saldo_actual / p.limite_credito) * 100, 0), 100)
            : (p.monto_prestamo > 0
                ? Math.min(Math.max((p.saldo_actual / p.monto_prestamo) * 100, 0), 100)
                : 0);
        const color = p.es_revolvente ? 'var(--color-warning)' : '#3b82f6';

        const etiqueta = p.es_revolvente
            ? { texto: anidada ? 'Facilidad' : 'Revolvente', color: '#fcd34d', fondo: 'rgba(245,158,11,0.12)', borde: 'rgba(245,158,11,0.25)' }
            : { texto: `${p.cuotas_pendientes ?? '—'}/${p.cuotas_totales ?? '—'}`, color: '#93c5fd', fondo: 'rgba(59,130,246,0.12)', borde: 'rgba(59,130,246,0.25)',
                titulo: `Quedan ${p.cuotas_pendientes ?? '—'} cuotas de ${p.cuotas_totales ?? '—'}` };

        const nombre = this.nombreFinanciamiento(p.tipo_prestamo);
        const contexto = anidada
            ? `Cuota DOP ${formato.importe(p.monto_cuota)} · ${p.tasa_actual} % · dentro del pago de la tarjeta`
            : `${p.institucion_financiera} · Cuota DOP ${formato.importe(p.monto_cuota)} · ${p.tasa_actual} % · día ${p.dia_pago}`;

        return `
            <div class="fila-pasivo" style="display:flex; align-items:center; gap:1rem; padding:0.8rem 0.25rem; border-bottom:1px solid rgba(255,255,255,0.05); transition:background var(--transition-fast);">
                <div style="width:26px; display:flex; justify-content:center; align-self:stretch; align-items:center;">
                    ${anidada ? `
                        <svg width="18" height="34" viewBox="0 0 18 34" fill="none" stroke="rgba(245,158,11,0.5)" stroke-width="1.4"><path d="M4 0v14a5 5 0 0 0 5 5h6"/></svg>
                    ` : ''}
                </div>
                <div style="flex:1.5;">
                    <div style="display:flex; align-items:center; gap:0.5rem; flex-wrap:wrap;">
                        <span style="font-size:0.85rem; font-weight:600;">${escaparHtml(nombre)}</span>
                        <span title="${escaparHtml(etiqueta.titulo ?? '')}" style="font-size:0.62rem; font-weight:700; text-transform:uppercase; letter-spacing:0.04em; color:${escaparHtml(etiqueta.color)}; background:${escaparHtml(etiqueta.fondo)}; border:1px solid ${escaparHtml(etiqueta.borde)}; border-radius:20px; padding:2px 8px;">${escaparHtml(etiqueta.texto)}</span>
                        ${p.alerta_pago ? `<span class="badge danger" style="font-size:0.6rem; padding:1px 7px;">${escaparHtml(p.dias_pago_msg)}</span>` : ''}
                    </div>
                    <div style="font-size:0.72rem; color:var(--text-muted); margin-top:2px;">${escaparHtml(contexto)}</div>
                </div>
                <div style="flex:1; text-align:right;">
                    <div style="font-size:0.9rem; font-weight:600;">DOP ${formato.importe(p.saldo_actual)}</div>
                    <div style="height:4px; background:rgba(255,255,255,0.06); border-radius:3px; margin:0.3rem 0 0.25rem; overflow:hidden;">
                        <div style="width:${usa}%; height:100%; background:${color}; border-radius:3px; transition:width var(--transition-normal);"></div>
                    </div>
                    <div style="font-size:0.72rem;">
                        ${p.disponible != null
                            ? `<span style="color:var(--color-success);">Disponible ${formato.importe(p.disponible)}</span> <span style="color:var(--text-muted);">de ${formato.importe(p.limite_credito!)}</span>`
                            : p.es_revolvente
                                ? `<span style="color:var(--text-muted); font-style:italic;">Sin límite declarado</span>`
                                : `<span style="color:var(--text-muted);">de ${formato.importe(p.monto_prestamo)}</span>`}
                    </div>
                </div>
                <div style="flex:0.7; text-align:right; font-size:0.78rem; color:var(--text-muted);">
                    ${anidada ? 'Hereda<br>las fechas' : `Día ${p.dia_pago}`}
                </div>
                <div style="width:30px; text-align:right;">
                    <button onclick="appUI.abrirMenuPasivo(event, ${argumentoJs(p)})" title="Acciones"
                            style="background:none; border:none; color:var(--text-muted); font-size:1.05rem; cursor:pointer; padding:0.2rem 0.4rem; border-radius:var(--radius-sm); transition:color var(--transition-fast), background var(--transition-fast);">⋯</button>
                </div>
            </div>
        `;
    }

    /**
     * Menú de acciones de un financiamiento.
     *
     * Las cuatro acciones dejan de ocupar sitio en cada fila. Se abre junto al
     * botón y se cierra con el siguiente clic o con Escape; solo hay uno
     * abierto a la vez, porque dos menús flotando compiten por el mismo sitio.
     */
    abrirMenuPasivo(evento: EventoDeMenu, p: Prestamo): void {
        const { menus } = this.dep;
        evento.stopPropagation();

        const puedeAbonar = p.es_revolvente || (p.cuotas_pendientes ?? 0) > 0;
        const html = `
            ${puedeAbonar ? `<div data-accion="abonar" style="padding:0.5rem 0.7rem; border-radius:6px; cursor:pointer;">Abonar cuota</div>` : ''}
            <div data-accion="conciliar" style="padding:0.5rem 0.7rem; border-radius:6px; cursor:pointer;">Conciliar saldo</div>
            <div data-accion="editar" style="padding:0.5rem 0.7rem; border-radius:6px; cursor:pointer;">Editar condiciones</div>
            <div style="height:1px; background:rgba(255,255,255,0.07); margin:0.25rem 0.5rem;"></div>
            <div data-accion="eliminar" style="padding:0.5rem 0.7rem; border-radius:6px; cursor:pointer; color:#fca5a5;">Eliminar</div>
        `;

        menus.abrir({
            id: 'menu-pasivo',
            html,
            ancla: (evento.currentTarget as NonNullable<EventoDeMenu['currentTarget']>).getBoundingClientRect(),
            alElegir: accion => {
                if (accion === 'abonar') this.handlePagarCuota(p.id);
                if (accion === 'conciliar') this.handleDeclararSaldo(p.id, p.saldo_actual);
                if (accion === 'editar') this.abrirEdicionPrestamo(p);
                if (accion === 'eliminar') this.handleEliminarPrestamo(p.id);
            },
        });
    }

    toggleCamposPrestamos(val: string): void {
        const { dom } = this.dep;
        // Se llama también al pintar el formulario, no solo desde el
        // `onchange`: si el tipo llegara ya seleccionado, el campo de límite
        // se quedaba oculto y no había forma de registrarlo.
        const container = dom.elemento('pre_campos_cuotas');
        const tot = dom.elemento<HTMLInputElement>('pre_tot');
        const pen = dom.elemento<HTMLInputElement>('pre_pen');

        // El límite solo significa algo donde hay cupo que reponer; el backend
        // rechaza un amortizable que lo traiga, así que la vista no lo ofrece.
        const grupoLimite = dom.buscar('pre_grupo_limite');
        const lim = dom.buscar<HTMLInputElement>('pre_lim');

        if (val === 'flexible') {
            container.style.display = 'none';
            tot.required = false;
            pen.required = false;
            tot.value = '';
            pen.value = '';
            if (grupoLimite) grupoLimite.style.display = 'block';
        } else {
            container.style.display = 'grid';
            tot.required = true;
            pen.required = true;
            if (grupoLimite) grupoLimite.style.display = 'none';
            if (lim) lim.value = '';
        }
    }

    /**
     * Corrige las condiciones de un financiamiento ya registrado.
     *
     * Sin esta vía, cambiar la tasa, la cuota o el límite obligaba a borrar el
     * registro y crearlo de nuevo — lo que se lleva por delante el libro de
     * movimientos. El saldo no se edita aquí a propósito: tiene su propia vía
     * en «Conciliar», que deja asiento de la diferencia.
     */
    private async abrirEdicionPrestamo(p: Prestamo): Promise<void> {
        const { api, modales } = this.dep;
        const tarjetas = await api.obtenerTarjetas();

        const html = `
            <div class="card" style="width: 460px; background: var(--bg-surface-opaque);">
                <h3 style="font-family: var(--font-heading); margin-bottom:0.4rem;">✏️ Condiciones del financiamiento</h3>
                <p style="font-size:0.75rem; color:var(--text-secondary); margin-bottom:1rem;">
                    ${escaparHtml(p.institucion_financiera)} — <strong style="text-transform:capitalize;">${escaparHtml(p.tipo_prestamo)}</strong><br>
                    El saldo no se edita aquí: usa «Conciliar», que deja constancia de la diferencia.
                </p>
                <form onsubmit="appUI.handleEdicionPrestamo(event, ${p.id})">
                    <div class="form-row">
                        <div class="form-group">
                            <label for="edp_tas_${p.id}">Tasa anual (%)</label>
                            <input type="number" step="0.01" id="edp_tas_${p.id}" class="form-control" value="${p.tasa_actual}" required>
                        </div>
                        <div class="form-group">
                            <label for="edp_cuo_${p.id}">Monto cuota</label>
                            <input type="number" step="0.01" id="edp_cuo_${p.id}" class="form-control" value="${p.monto_cuota}" required>
                        </div>
                    </div>
                    ${p.es_revolvente ? `
                        <div class="form-group">
                            <label for="edp_lim_${p.id}">Límite de crédito</label>
                            <input type="number" step="0.01" id="edp_lim_${p.id}" class="form-control"
                                   value="${p.limite_credito ?? ''}" placeholder="Sin declarar">
                            <small style="font-size:0.7rem; color:var(--text-muted);">Al pagar, lo amortizado vuelve a quedar disponible.</small>
                        </div>
                    ` : ''}
                    <div class="form-group">
                        <label for="edp_tar_${p.id}">Tarjeta que lo cobra</label>
                        <select id="edp_tar_${p.id}" class="form-control" onchange="appUI.avisarFechasDerivadas(${p.id})">
                            <option value="">Ninguna — se paga por su cuenta</option>
                            ${tarjetas.map(t => `
                                <option value="${t.id}" data-pago="${t.fecha_limite_pago}" data-corte="${t.fecha_corte}" ${p.tarjeta_id === t.id ? 'selected' : ''}>
                                    ${escaparHtml(t.entidad)} — ${escaparHtml(t.nombre_tarjeta)}
                                </option>
                            `).join('')}
                        </select>
                        <div id="edp_aviso_${p.id}" style="font-size:0.7rem; color:var(--text-secondary); margin-top:0.4rem;"></div>
                    </div>
                    <div class="form-group">
                        <label for="edp_dia_${p.id}">Día de pago propio</label>
                        <input type="number" min="1" max="31" id="edp_dia_${p.id}" class="form-control" value="${p.dia_pago}" required>
                    </div>
                    <div style="display:flex; justify-content:flex-end; gap:0.5rem; margin-top:1.2rem;">
                        <button type="button" onclick="elemento('modal-pre-${p.id}').remove()" class="btn btn-secondary">Cancelar</button>
                        <button type="submit" class="btn">Guardar</button>
                    </div>
                </form>
            </div>
        `;
        modales.abrir(`modal-pre-${p.id}`, html);
        this.avisarFechasDerivadas(p.id);
    }

    /**
     * Explica que vincular una facilidad a una tarjeta le cede las fechas.
     *
     * Sin el aviso, el día de pago propio del financiamiento seguiría visible
     * y editable mientras deja de tener efecto, que es la forma más fiable de
     * que alguien lo corrija y no entienda por qué no cambia nada.
     */
    avisarFechasDerivadas(id: number): void {
        const { dom } = this.dep;
        const select = dom.buscar<HTMLSelectElement>(`edp_tar_${id}`);
        const aviso = dom.buscar(`edp_aviso_${id}`);
        const dia = dom.buscar<HTMLInputElement>(`edp_dia_${id}`);
        if (!select || !aviso) return;

        const opcion = select.selectedOptions[0];
        const vinculada = select.value !== '';

        if (vinculada) {
            aviso.innerHTML = `Toma las fechas de la tarjeta: <strong>corte día ${escaparHtml(opcion.dataset.corte)}</strong>,
                               <strong>pago día ${escaparHtml(opcion.dataset.pago)}</strong>. El día propio de abajo queda sin efecto.`;
            if (dia) dia.disabled = true;
        } else {
            aviso.textContent = '';
            if (dia) dia.disabled = false;
        }
    }

    async handleEdicionPrestamo(e: EventoDeFormulario, id: number): Promise<void> {
        e.preventDefault();
        const { api, avisos, enrutador, dom } = this.dep;
        const limiteCampo = dom.buscar<Campo>(`edp_lim_${id}`);
        const limiteTexto = limiteCampo ? limiteCampo.value : '';
        const tarjeta = dom.elemento<Campo>(`edp_tar_${id}`).value;

        try {
            await api.actualizarPrestamo({
                id: Number(id),
                tasa_actual: Number(dom.elemento<Campo>(`edp_tas_${id}`).value),
                monto_cuota: Number(dom.elemento<Campo>(`edp_cuo_${id}`).value),
                dia_pago: Number(dom.elemento<Campo>(`edp_dia_${id}`).value),
                // En blanco significa «no declarado», que no es lo mismo que cero.
                limite_credito: limiteTexto === '' ? null : Number(limiteTexto),
                tarjeta_id: tarjeta === '' ? null : Number(tarjeta),
            });
            avisos.mostrar("Condiciones actualizadas.");
            dom.buscar(`modal-pre-${id}`)?.remove();
            await enrutador.mostrar('prestamos');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    async handleAgregarPrestamo(e: EventoDeFormulario): Promise<void> {
        e.preventDefault();
        const { api, avisos, enrutador, dom } = this.dep;
        const tip = dom.elemento<Campo>('pre_tip').value;
        const ins = dom.elemento<Campo>('pre_ins').value;
        const mon = Number(dom.elemento<Campo>('pre_mon').value);
        const tas = Number(dom.elemento<Campo>('pre_tas').value);
        const tot = dom.buscar('pre_tot') && dom.elemento<Campo>('pre_tot').value ? Number(dom.elemento<Campo>('pre_tot').value) : null;
        const pen = dom.buscar('pre_pen') && dom.elemento<Campo>('pre_pen').value ? Number(dom.elemento<Campo>('pre_pen').value) : null;
        const cuo = Number(dom.elemento<Campo>('pre_cuo').value);
        const dia = Number(dom.elemento<Campo>('pre_dia').value);
        const salTexto = dom.buscar<Campo>('pre_sal')?.value ?? '';
        const limTexto = dom.buscar<Campo>('pre_lim')?.value ?? '';
        // En blanco significa "no declarado", que no es lo mismo que cero.
        const sal = salTexto === '' ? null : Number(salTexto);
        const lim = tip === 'flexible' && limTexto !== '' ? Number(limTexto) : null;

        try {
            await api.crearPrestamo({
                tipo_prestamo: tip,
                monto_prestamo: mon,
                institucion_financiera: ins,
                tasa_actual: tas,
                cuotas_totales: tot,
                cuotas_pendientes: pen,
                monto_cuota: cuo,
                dia_pago: dia,
                saldo_actual: sal,
                limite_credito: lim
            });
            avisos.mostrar("Financiamiento registrado con éxito.");
            await enrutador.mostrar('prestamos');
        } catch (err) {
            // Captura y presentación de la excepción del backend de Rust
            avisos.mostrar(String(err), 'error');
        }
    }

    /**
     * Fija el saldo al del estado de cuenta.
     *
     * La aplicación estima el saldo cuota a cuota, y ninguna estimación cuadra
     * al centavo con el acreedor: comisiones, seguros y días de gracia no
     * entran en la fórmula. Esta es la vía para corregirlo, y la diferencia
     * queda asentada como un movimiento propio en lugar de aplicarse a ciegas.
     */
    async handleDeclararSaldo(id: number, saldoActual: number): Promise<void> {
        const { api, avisos, formato, enrutador, dialogos } = this.dep;
        const respuesta = await dialogos.preguntar(
            `Saldo que muestra el estado de cuenta (la aplicación estima DOP ${formato.importe(saldoActual)}):`,
            Number(saldoActual).toFixed(2)
        );
        if (respuesta === null) return;

        // Mismo motivo: entra por `prompt`, así que va como texto.
        const saldo = respuesta.trim();
        // Se valida la **forma** del texto, no el número: convertirlo aquí
        // para comprobarlo devolvería al punto de partida.
        if (!/^-?\d*\.?\d+$/.test(saldo.replace(/,/g, ''))) {
            avisos.mostrar("El saldo debe ser un importe válido.", 'error');
            return;
        }

        try {
            await api.declararSaldoPrestamo(id, saldo);
            avisos.mostrar("Saldo conciliado con el estado de cuenta.");
            await enrutador.mostrar('prestamos');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    async handlePagarCuota(id: number): Promise<void> {
        const { api, avisos, enrutador } = this.dep;
        try {
            await api.pagarCuotaPrestamo(id);
            avisos.mostrar("Abono de cuota registrado.");
            await enrutador.mostrar('prestamos');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    async handleEliminarPrestamo(id: number): Promise<void> {
        const { api, avisos, enrutador, dialogos } = this.dep;
        if (await dialogos.confirmar("¿Deseas eliminar este registro de deuda?")) {
            try {
                await api.eliminarPrestamo(id);
                avisos.mostrar("Registro eliminado.");
                await enrutador.mostrar('prestamos');
            } catch (err) {
                avisos.mostrar(String(err), 'error');
            }
        }
    }
}

/**
 * Lo que `window.appUI` expone de esta vista: un puente que delega en la
 * instancia. Los atributos en línea no cambian.
 */
export function puentePrestamos(vista: VistaPrestamos): {
    handleAgregarPrestamo: (e: EventoDeFormulario) => Promise<void>;
    toggleCamposPrestamos: (val: string) => void;
    abrirMenuPasivo: (evento: EventoDeMenu, p: Prestamo) => void;
    handleEdicionPrestamo: (e: EventoDeFormulario, id: number) => Promise<void>;
    avisarFechasDerivadas: (id: number) => void;
    handlePagarCuota: (id: number) => Promise<void>;
    handleDeclararSaldo: (id: number, saldoActual: number) => Promise<void>;
    handleEliminarPrestamo: (id: number) => Promise<void>;
} {
    return {
        handleAgregarPrestamo: e => vista.handleAgregarPrestamo(e),
        toggleCamposPrestamos: val => vista.toggleCamposPrestamos(val),
        abrirMenuPasivo: (evento, p) => vista.abrirMenuPasivo(evento, p),
        handleEdicionPrestamo: (e, id) => vista.handleEdicionPrestamo(e, id),
        avisarFechasDerivadas: id => vista.avisarFechasDerivadas(id),
        handlePagarCuota: id => vista.handlePagarCuota(id),
        handleDeclararSaldo: (id, saldoActual) => vista.handleDeclararSaldo(id, saldoActual),
        handleEliminarPrestamo: id => vista.handleEliminarPrestamo(id),
    };
}
