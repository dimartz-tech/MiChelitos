// Pestaña «Suscripciones» (diseño «B»: clase con dependencias inyectadas).
//
// Cobros recurrentes con tarjeta. Es la primera vista que **abre una ventana
// modal** (el editor de una suscripción) y que pide **un motivo** para una
// corrección (descartar un período): por eso estrena los servicios `Modales` y
// `Motivo`, además de `Dialogos`. No usa `this` de ningún objeto mezclado, ni
// `AppAPI`, `elemento()`, `confirm()`, `prompt()` ni `document` globales: todo
// entra por el constructor (`pruebas/js/vistas/suscripciones.test.js`). Los
// cuerpos son los de `ui.ts`.

import type { Suscripcion } from '../tipos-ipc';
import type { ApiDe, Avisos, Dialogos, Dom, Enrutador, Formato, Modales, Motivo, Pantalla, Vista } from '../ui/servicios';

type ApiSuscripciones = ApiDe<
    | 'obtenerSuscripciones'
    | 'obtenerTarjetas'
    | 'crearSuscripcion'
    | 'actualizarSuscripcion'
    | 'eliminarSuscripcion'
    | 'corregirProximoCobro'
    | 'asentarPeriodoPendiente'
    | 'descartarPeriodoPendiente'
>;

export interface DependenciasSuscripciones {
    api: ApiSuscripciones;
    avisos: Avisos;
    formato: Formato;
    enrutador: Enrutador;
    pantalla: Pantalla;
    dom: Dom;
    dialogos: Dialogos;
    motivo: Motivo;
    modales: Modales;
}

/** Los manejadores en línea (`onsubmit`/`onclick` → `appUI.…`) que la plantilla escribe. */
export const MANEJADORES_SUSCRIPCIONES = [
    'handleAgregarSuscripcion', 'abrirEdicionSuscripcion', 'handleEdicionSuscripcionSubmit',
    'handleEliminarSuscripcion', 'handleCorregirProximoCobro', 'handleAsentarPendiente', 'handleDescartarPendiente',
] as const;
export type ManejadorSuscripciones = (typeof MANEJADORES_SUSCRIPCIONES)[number];

/** Un formulario envía un evento con `preventDefault`; solo eso se usa. */
interface EventoDeFormulario {
    preventDefault(): void;
}

export class VistaSuscripciones implements Vista {
    constructor(private readonly dep: DependenciasSuscripciones) {}

    // --- RENDER: SUSCRIPCIONES ---
    async render(): Promise<void> {
        const { api, formato, pantalla } = this.dep;
        const suscripciones = await api.obtenerSuscripciones();
        const tarjetas = await api.obtenerTarjetas();

        pantalla.contenido.innerHTML = `
            <div class="section-title">
                <h1>Suscripciones Recurrentes</h1>
                ${(() => {
                    const conHuecos = suscripciones.filter(s => s.pendientes.length > 0);
                    if (conHuecos.length === 0) return '';
                    return `<div class="card" style="border-left:3px solid var(--warning, #e0a020); margin-bottom:1rem;">
                        <strong>📋 Períodos por confirmar</strong>
                        <p style="font-size:0.78rem; color:var(--text-secondary); margin:0.4rem 0 0.6rem;">
                            Vencieron varios sin que la aplicación estuviera abierta. No se cobran solos:
                            confirma contra el estado de cuenta cuáles ocurrieron.
                        </p>
                        ${conHuecos.map(s => `
                            <div style="display:flex; align-items:center; gap:0.5rem; margin-bottom:0.4rem; font-size:0.85rem;">
                                <span style="flex:1;"><strong>${s.plataforma}</strong> — ${s.divisa} ${formato.importe(s.monto)},
                                    el más antiguo: <strong>${s.pendientes[0]}</strong>
                                    ${s.pendientes.length > 1 ? `<span style="color:var(--text-muted);">(y ${s.pendientes.length - 1} más)</span>` : ''}</span>
                                <button onclick="appUI.handleAsentarPendiente(${s.id}, '${s.pendientes[0]}')" class="btn" style="padding:0.25rem 0.5rem; font-size:0.78rem;">Sí se cobró</button>
                                <button onclick="appUI.handleDescartarPendiente(${s.id}, '${s.pendientes[0]}')" class="btn" style="padding:0.25rem 0.5rem; font-size:0.78rem; background:rgba(255,255,255,0.05); border:1px solid var(--border-color);">No se cobró</button>
                            </div>`).join('')}
                    </div>`;
                })()}
                ${(() => {
                    const paradas = suscripciones.filter(s => s.impedimento);
                    if (paradas.length === 0) return '';
                    return `<div class="card" style="border-left:3px solid var(--danger, #e05260); margin-bottom:1rem;">
                        <strong>⛔ No se cobrarán</strong>
                        <ul style="margin:0.5rem 0 0 1rem; font-size:0.85rem;">
                            ${paradas.map(s => `<li><strong>${s.plataforma}</strong> — ${s.impedimento}</li>`).join('')}
                        </ul>
                    </div>`;
                })()}
                ${(() => {
                    const avisan = suscripciones.filter(s => s.avisa);
                    if (avisan.length === 0) return '';
                    return `<div class="card" style="border-left:3px solid var(--warning, #e0a020); margin-bottom:1rem;">
                        <strong>🔔 Cobro próximo</strong>
                        <ul style="margin:0.5rem 0 0 1rem; font-size:0.85rem;">
                            ${avisan.map(s => `<li><strong>${s.plataforma}</strong> — ${s.divisa} ${formato.importe(s.monto)} el ${s.fecha_proximo_cobro}</li>`).join('')}
                        </ul>
                    </div>`;
                })()}
                <span class="subtitle">Monitoreo de membresías mensuales y anuales debidamente cargadas</span>
            </div>

            <div class="responsive-split-grid">
                <div class="card" style="height:fit-content;">
                    <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 1rem;">🔄 Nueva Suscripción</h3>
                    ${tarjetas.length > 0 ? `
                        <form id="form-add-suscripcion" onsubmit="appUI.handleAgregarSuscripcion(event)">
                            <div class="form-group">
                                <label for="sus_pla">Servicio / Plataforma *</label>
                                <input type="text" id="sus_pla" class="form-control" placeholder="Netflix, Spotify, AWS..." required>
                            </div>
                            <div class="form-row">
                                <div class="form-group" style="flex:2;">
                                    <label for="sus_mon">Monto *</label>
                                    <input type="number" id="sus_mon" step="0.01" class="form-control" placeholder="0.00" required>
                                </div>
                                <div class="form-group" style="flex:1;">
                                    <label for="sus_div">Divisa</label>
                                    <select id="sus_div" class="form-control">
                                        <option value="DOP" selected>DOP</option>
                                        <option value="USD">USD</option>
                                    </select>
                                </div>
                            </div>
                            <div class="form-row">
                                <div class="form-group">
                                    <label for="sus_dia">Día Facturación (1-31) *</label>
                                    <input type="number" id="sus_dia" min="1" max="31" value="1" class="form-control" required>
                                </div>
                                <div class="form-group">
                                    <label for="sus_fre">Frecuencia</label>
                                    <select id="sus_fre" class="form-control">
                                        <option value="mensual" selected>Mensual</option>
                                        <option value="anual">Anual</option>
                                    </select>
                                </div>
                            </div>
                            <div class="form-group">
                                <label for="sus_ren">Fecha del próximo cobro *</label>
                                <input type="date" id="sus_ren" class="form-control" required>
                                <small style="color:var(--text-muted); font-size:0.7rem;">
                                    Sin ella la suscripción no se cobra. El día de facturación solo sirve de ancla
                                    para calcular los siguientes.
                                </small>
                            </div>
                            <div class="form-group">
                                <label for="sus_tar">Tarjeta de Cargo *</label>
                                <select id="sus_tar" class="form-control" required>
                                    <option value="" disabled selected>Seleccione...</option>
                                    ${tarjetas.map(t => `<option value="${t.id}">${t.entidad} - ${t.nombre_tarjeta}</option>`).join('')}
                                </select>
                            </div>
                            <button type="submit" class="btn" style="width:100%; margin-top:0.5rem;">🚀 Registrar Cargo</button>
                        </form>
                    ` : `
                        <p style="color:var(--text-muted); text-align:center; padding:1rem 0;">
                            Registra primero una tarjeta de crédito para poder asociar suscripciones.
                        </p>
                    `}
                </div>

                <div class="card">
                    <h3 style="font-family: var(--font-heading); font-size: 1.1rem; margin-bottom: 1rem;">📋 Cargos Activos</h3>
                    ${suscripciones.length > 0 ? `
                        <div class="table-responsive">
                            <table class="table-modern">
                                <thead>
                                    <tr>
                                        <th>Servicio</th>
                                        <th>Frecuencia</th>
                                        <th>Día Pago</th>
                                        <th>Próximo Cobro</th>
                                        <th>Último Pago</th>
                                        <th>Tarjeta Cargo</th>
                                        <th>Monto</th>
                                        <th>Acciones</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    ${suscripciones.map(s => `
                                        <tr>
                                            <td><strong>${s.plataforma}</strong></td>
                                            <td style="text-transform:capitalize;">${s.frecuencia}</td>
                                            <td>Día ${s.dia_facturacion}</td>
                                            <td>${
                                                s.fecha_proximo_cobro
                                                    ? `${s.avisa ? '🔔 ' : ''}${s.fecha_proximo_cobro}` +
                                                      (s.pendientes.length > 1 ? ` <span style="color:var(--danger, #e05260);">+${s.pendientes.length - 1}</span>` : '')
                                                    : '<span style="color:var(--danger, #e05260);">Sin fecha: no se cobrará</span>'
                                            }</td>
                                            <td>${
                                                s.impedimento
                                                    ? `<span style="color:var(--danger, #e05260);" title="${s.impedimento}">⛔ ${s.fecha_ultimo_pago || 'sin fecha'}</span>`
                                                    : (s.fecha_ultimo_pago || '<span style="font-style:italic;color:var(--text-muted);">Pendiente</span>')
                                            }</td>
                                            <td>${s.entidad} (${s.nombre_tarjeta})</td>
                                            <td class="amount expense">${s.divisa} ${formato.importe(s.monto)}</td>
                                            <td>
                                                ${s.impedimento ? `<button onclick="appUI.handleCorregirProximoCobro(${s.id})" class="btn" style="padding: 0.3rem 0.5rem; font-size:0.8rem; background:rgba(224,82,96,0.15); border:1px solid var(--danger, #e05260);" title="Corregir la fecha del último cobro">🔧</button>` : ''}
                                                <button onclick='appUI.abrirEdicionSuscripcion(${JSON.stringify(s).replace(/'/g, "&apos;")})' class="btn" style="padding: 0.3rem 0.5rem; font-size:0.8rem; background:rgba(255,255,255,0.05); border:1px solid var(--border-color);" title="Editar">✏️</button>
                                                <button onclick="appUI.handleEliminarSuscripcion(${s.id})" class="btn btn-danger" style="padding: 0.3rem 0.5rem; font-size:0.8rem;">🗑️</button>
                                            </td>
                                        </tr>
                                    `).join('')}
                                </tbody>
                            </table>
                        </div>
                    ` : `
                        <p style="color:var(--text-muted); text-align:center; padding:2rem;">No hay suscripciones registradas.</p>
                    `}
                </div>
            </div>
        `;
    }

    async handleAgregarSuscripcion(e: EventoDeFormulario): Promise<void> {
        e.preventDefault();
        const { api, avisos, enrutador, dom } = this.dep;
        const pla = dom.elemento<Campo>('sus_pla').value;
        const mon = dom.elemento<Campo>('sus_mon').value.trim();
        const div = dom.elemento<Campo>('sus_div').value;
        const dia = Number(dom.elemento<Campo>('sus_dia').value);
        const fre = dom.elemento<Campo>('sus_fre').value;
        const tar = Number(dom.elemento<Campo>('sus_tar').value);
        const ren = this.fechaDeEntrada('sus_ren');
        if (ren === null) {
            avisos.mostrar("Indica la fecha del próximo cobro.", "error");
            return;
        }

        try {
            await api.crearSuscripcion(pla, mon, tar, fre, dia, div, ren);
            avisos.mostrar("Suscripción recurrente guardada.");
            await enrutador.mostrar('suscripciones');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    async abrirEdicionSuscripcion(s: Suscripcion): Promise<void> {
        const { api, modales } = this.dep;
        const tarjetas = await api.obtenerTarjetas();
        modales.abrir(`modal-edit-sus-${s.id}`, `
            <div class="card" style="width: 420px; background: var(--bg-surface-opaque); max-height:90vh; overflow-y:auto;">
                <h3 style="font-family: var(--font-heading); margin-bottom: 0.4rem;">✏️ Editar Suscripción</h3>
                <p style="font-size:0.72rem; color:var(--text-secondary); margin-bottom:1rem;">
                    Se conserva el último pago registrado (${s.fecha_ultimo_pago || 'ninguno'}), de modo que editar no provoca un cobro repetido este período.
                </p>
                <form onsubmit="appUI.handleEdicionSuscripcionSubmit(event, ${s.id})">
                    <div class="form-group">
                        <label>Servicio / Plataforma</label>
                        <input type="text" id="es_pla_${s.id}" class="form-control" value="${s.plataforma}" required>
                    </div>
                    <div class="form-row">
                        <div class="form-group">
                            <label>Monto</label>
                            <input type="number" step="0.01" id="es_mon_${s.id}" class="form-control" value="${s.monto}" required>
                        </div>
                        <div class="form-group">
                            <label>Divisa</label>
                            <select id="es_div_${s.id}" class="form-control">
                                <option value="DOP" ${s.divisa === 'DOP' ? 'selected' : ''}>DOP</option>
                                <option value="USD" ${s.divisa === 'USD' ? 'selected' : ''}>USD</option>
                            </select>
                        </div>
                    </div>
                    <div class="form-row">
                        <div class="form-group">
                            <label>Día de facturación</label>
                            <input type="number" min="1" max="31" id="es_dia_${s.id}" class="form-control" value="${s.dia_facturacion}" required>
                        </div>
                        <div class="form-group">
                            <label>Frecuencia</label>
                            <select id="es_fre_${s.id}" class="form-control">
                                <option value="mensual" ${s.frecuencia === 'mensual' ? 'selected' : ''}>Mensual</option>
                                <option value="anual" ${s.frecuencia === 'anual' ? 'selected' : ''}>Anual</option>
                            </select>
                        </div>
                    </div>
                    <div class="form-group">
                        <label>Tarjeta de cargo</label>
                        <div class="form-group">
                            <label for="es_ren_${s.id}">Fecha del próximo cobro *</label>
                            <input type="date" id="es_ren_${s.id}" class="form-control" value="${this.fechaAIso(s.fecha_proximo_cobro)}">
                        </div>
                        <select id="es_tar_${s.id}" class="form-control" required>
                            ${tarjetas.map(t => `<option value="${t.id}" ${t.id === s.tarjeta_id ? 'selected' : ''}>${t.entidad} - ${t.nombre_tarjeta}</option>`).join('')}
                        </select>
                    </div>
                    <div style="display:flex; justify-content:flex-end; gap:0.5rem; margin-top:1.2rem;">
                        <button type="button" onclick="elemento('modal-edit-sus-${s.id}').remove()" class="btn btn-secondary">Cancelar</button>
                        <button type="submit" class="btn">Guardar Cambios</button>
                    </div>
                </form>
            </div>
        `);
    }

    async handleEdicionSuscripcionSubmit(e: EventoDeFormulario, id: number): Promise<void> {
        e.preventDefault();
        const { api, avisos, enrutador, dom } = this.dep;
        const pla = dom.elemento<Campo>(`es_pla_${id}`).value.trim();
        const mon = dom.elemento<Campo>(`es_mon_${id}`).value.trim();
        const div = dom.elemento<Campo>(`es_div_${id}`).value;
        const dia = Number(dom.elemento<Campo>(`es_dia_${id}`).value);
        const fre = dom.elemento<Campo>(`es_fre_${id}`).value;
        const tar = Number(dom.elemento<Campo>(`es_tar_${id}`).value);

        if (!pla) { avisos.mostrar("El nombre del servicio no puede estar vacío.", "error"); return; }
        if (!(Number(mon) > 0)) { avisos.mostrar("El monto debe ser mayor que cero.", "error"); return; }
        if (!(dia >= 1 && dia <= 31)) { avisos.mostrar("El día de facturación debe estar entre 1 y 31.", "error"); return; }

        try {
            const ren = this.fechaDeEntrada(`es_ren_${id}`);
            if (ren === null) {
                avisos.mostrar("Indica la fecha del próximo cobro.", "error");
                return;
            }
            await api.actualizarSuscripcion(id, pla, mon, tar, fre, dia, div, ren);
            avisos.mostrar("Suscripción actualizada. Los cargos ya realizados no se alteran.");
            dom.elemento(`modal-edit-sus-${id}`).remove();
            await enrutador.mostrar('suscripciones');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    async handleEliminarSuscripcion(id: number): Promise<void> {
        const { api, avisos, enrutador, dialogos } = this.dep;
        if (await dialogos.confirmar("¿Deseas dar de baja esta suscripción?")) {
            try {
                await api.eliminarSuscripcion(id);
                avisos.mostrar("Suscripción eliminada.");
                await enrutador.mostrar('suscripciones');
            } catch (err) {
                avisos.mostrar(String(err), 'error');
            }
        }
    }

    /// Un `<input type="date">` entrega ISO; la aplicación guarda dd/mm/aaaa.
    ///
    /// La conversión se hace aquí, en el borde, y no en el núcleo: son dos
    /// formatos con dos públicos, y mezclarlos es lo que produce fechas que
    /// nadie sabe leer.
    private fechaDeEntrada(elementoId: string): string | null {
        const { dom } = this.dep;
        const iso = dom.buscar<Campo>(elementoId)?.value;
        if (!iso) return null;
        const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
        return m ? `${m[3]}/${m[2]}/${m[1]}` : null;
    }

    /// El camino inverso, para rellenar el formulario de edición.
    private fechaAIso(ddmmaaaa: string | null | undefined): string {
        if (!ddmmaaaa) return '';
        const m = /^(\d{2})\/(\d{2})\/(\d{4})$/.exec(ddmmaaaa);
        return m ? `${m[3]}-${m[2]}-${m[1]}` : '';
    }

    /// Pone a mano la fecha del próximo cobro de una suscripción parada.
    ///
    /// Pide la fecha en lugar de deducirla, por lo mismo que la anual:
    /// deducir un vencimiento con datos que no bastan es lo que produjo los
    /// defectos de esta fase.
    async handleCorregirProximoCobro(id: number): Promise<void> {
        const { api, avisos, enrutador, dialogos } = this.dep;
        const fecha = await dialogos.preguntar(
            `Esta suscripción no tiene fecha de próximo cobro, así que no se cobrará.\n\n` +
            `Escríbela en formato dd/mm/aaaa.`,
            ''
        );
        if (fecha === null) return;

        // Se valida la forma aquí y también en el núcleo. Aquí para no hacer
        // ir y volver un texto que ya se ve mal; allí porque es donde la
        // garantía tiene que vivir.
        const limpia = fecha.trim();
        if (!/^\d{2}\/\d{2}\/\d{4}$/.test(limpia)) {
            avisos.mostrar("La fecha debe escribirse como dd/mm/aaaa.", "error");
            return;
        }

        try {
            await api.corregirProximoCobro(id, limpia);
            avisos.mostrar("Fecha puesta. La suscripción vuelve a su ciclo.");
            await enrutador.mostrar('suscripciones');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    /// Confirma que el período más antiguo sí lo cobró el proveedor.
    async handleAsentarPendiente(id: number, fecha: string): Promise<void> {
        const { api, avisos, enrutador, dialogos } = this.dep;
        if (!await dialogos.confirmar(`Se asentará el cargo con fecha ${fecha}, y la deuda de la tarjeta subirá.\n\n¿El proveedor cobró ese período?`)) return;
        try {
            avisos.mostrar(await api.asentarPeriodoPendiente(id));
            await enrutador.mostrar('suscripciones');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    /// Da por no cobrado el período más antiguo.
    ///
    /// Pide un motivo por lo mismo que las correcciones: descartar es
    /// afirmar que el proveedor no cobró, y esa afirmación se hace mirando un
    /// estado de cuenta. Si la cifra anual no cuadra dentro de seis meses,
    /// esto dirá por qué.
    async handleDescartarPendiente(id: number, fecha: string): Promise<void> {
        const { api, avisos, enrutador, motivo: pedidorDeMotivo } = this.dep;
        const motivo = await pedidorDeMotivo.pedir(
            `Dar por no cobrado el período del ${fecha}`,
            `No se asentará ningún cargo y la suscripción pasará al período siguiente. ` +
            `Quedará un caso de auditoría con lo que escribas.`
        );
        if (motivo === null) return;
        try {
            avisos.mostrar(await api.descartarPeriodoPendiente(id, motivo));
            await enrutador.mostrar('suscripciones');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }
}

/**
 * Lo que `window.appUI` expone de esta vista: un puente que delega en la
 * instancia. Los atributos `onsubmit`/`onclick` no cambian.
 */
export function puenteSuscripciones(vista: VistaSuscripciones): {
    handleAgregarSuscripcion: (e: EventoDeFormulario) => Promise<void>;
    abrirEdicionSuscripcion: (s: Suscripcion) => Promise<void>;
    handleEdicionSuscripcionSubmit: (e: EventoDeFormulario, id: number) => Promise<void>;
    handleEliminarSuscripcion: (id: number) => Promise<void>;
    handleCorregirProximoCobro: (id: number) => Promise<void>;
    handleAsentarPendiente: (id: number, fecha: string) => Promise<void>;
    handleDescartarPendiente: (id: number, fecha: string) => Promise<void>;
} {
    return {
        handleAgregarSuscripcion: e => vista.handleAgregarSuscripcion(e),
        abrirEdicionSuscripcion: s => vista.abrirEdicionSuscripcion(s),
        handleEdicionSuscripcionSubmit: (e, id) => vista.handleEdicionSuscripcionSubmit(e, id),
        handleEliminarSuscripcion: id => vista.handleEliminarSuscripcion(id),
        handleCorregirProximoCobro: id => vista.handleCorregirProximoCobro(id),
        handleAsentarPendiente: (id, fecha) => vista.handleAsentarPendiente(id, fecha),
        handleDescartarPendiente: (id, fecha) => vista.handleDescartarPendiente(id, fecha),
    };
}
