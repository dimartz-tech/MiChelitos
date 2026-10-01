// Pestaña «Ingresos» (diseño «B»: clase con dependencias inyectadas).
//
// Facturas formales y registro informal de flujos: alta, corrección de una
// factura (que, si ya está cobrada, mueve un saldo y pide motivo), cobro de
// facturas y de ingresos informales. No usa `AppAPI`, `elemento()`, `document`,
// `confirm` ni `Date` globales: todo entra por el constructor
// (`pruebas/js/vistas/ingresos.test.js`). Los cuerpos son los de `ui.ts`.

import type { Ingreso } from '../tipos-ipc';
import type { ApiDe, Avisos, Dialogos, Dom, Enrutador, Formato, Modales, Motivo, Pantalla, Reloj, Vista } from '../ui/servicios';

import { argumentoJs, escaparHtml } from '../nucleo/html.js';

type ApiIngresos = ApiDe<
    | 'obtenerIngresos'
    | 'obtenerIngresosInformales'
    | 'obtenerClientes'
    | 'obtenerCuentas'
    | 'crearIngreso'
    | 'crearIngresoInformal'
    | 'actualizarIngreso'
    | 'marcarIngresoPagado'
    | 'marcarInformalPagado'
>;

export interface DependenciasIngresos {
    api: ApiIngresos;
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

/** Los manejadores en línea (`onsubmit`/`onchange`/`onclick` → `appUI.…`) que la plantilla escribe. */
export const MANEJADORES_INGRESOS = [
    'handleAgregarIngreso', 'handleAgregarIngresoInformal', 'handleSelectCliente',
    'abrirEdicionFormal', 'alternarCobroParcial', 'handleEdicionFormalSubmit',
    'abrirCobroFormal', 'handleCobroFormalSubmit', 'abrirCobroInformal', 'handleCobroInformalSubmit',
] as const;
export type ManejadorIngresos = (typeof MANEJADORES_INGRESOS)[number];

/** Un formulario envía un evento con `preventDefault`; solo eso se usa. */
interface EventoDeFormulario {
    preventDefault(): void;
}

export class VistaIngresos implements Vista {
    constructor(private readonly dep: DependenciasIngresos) {}

    async render(): Promise<void> {
        const { api, formato, pantalla, ahora } = this.dep;
        const ingresos = await api.obtenerIngresos();
        const informales = await api.obtenerIngresosInformales();
        const clientes = await api.obtenerClientes();

        const hoy = ahora();
        const hoyStr = hoy.getDate().toString().padStart(2, '0') + '/' + (hoy.getMonth() + 1).toString().padStart(2, '0') + '/' + hoy.getFullYear();

        const lastInvoice = ingresos.length > 0 ? ingresos[0].numero_factura : "";
        const nextInvoice = (() => {
            if (!lastInvoice) return "FAC-0001";
            const match = lastInvoice.match(/^(.*?)(\d+)$/);
            if (match) {
                const prefix = match[1];
                const numStr = match[2];
                const nextVal = parseInt(numStr, 10) + 1;
                return prefix + nextVal.toString().padStart(numStr.length, '0');
            }
            return lastInvoice + "-1";
        })();

        pantalla.contenido.innerHTML = `
            <div class="section-title">
                <h1>Ingresos</h1>
                <span class="subtitle">Facturas formales y registro informal de flujos</span>
            </div>

            <div class="responsive-split-grid">
                <!-- Formulario -->
                <div class="card" style="height: fit-content;">
                    <div style="display: flex; gap: 0.4rem; margin-bottom: 1.2rem; background: rgba(255,255,255,0.02); padding: 3px; border-radius: var(--radius-sm); border: 1px solid var(--border-color);">
                        <button onclick="elemento('tauri-form-formal').style.display='block'; elemento('tauri-form-informal').style.display='none'; this.className='btn'; elemento('btn-tab-inf').className='btn btn-secondary';" id="btn-tab-for" class="btn" style="flex:1; padding: 0.4rem; font-size: 0.8rem;">📄 Formal</button>
                        <button onclick="elemento('tauri-form-formal').style.display='none'; elemento('tauri-form-informal').style.display='block'; this.className='btn'; elemento('btn-tab-for').className='btn btn-secondary';" id="btn-tab-inf" class="btn btn-secondary" style="flex:1; padding: 0.4rem; font-size: 0.8rem; border:none;">💸 Informal</button>
                    </div>

                    <!-- FORMULARIO FORMAL -->
                    <div id="tauri-form-formal">
                        <form id="form-add-formal" onsubmit="appUI.handleAgregarIngreso(event)">
                            <div class="form-group">
                                <label for="num_fac">Número de Factura *</label>
                                <input type="text" id="num_fac" class="form-control" value="${escaparHtml(nextInvoice)}" required>
                            </div>
                            <div class="form-group">
                                <label for="fec_em">Fecha de Emisión *</label>
                                <input type="text" id="fec_em" class="form-control" value="${escaparHtml(hoyStr)}" placeholder="dd/mm/aaaa" required>
                            </div>
                            <div class="form-group">
                                <label for="cli_select">Cliente *</label>
                                <select id="cli_select" class="form-control" onchange="appUI.handleSelectCliente(this.value)" required>
                                    <option value="" disabled selected>Seleccione cliente...</option>
                                    ${clientes.map(c => `<option value="${c.id}" data-rnc="${escaparHtml(c.rnc)}" data-nombre="${escaparHtml(c.nombre)}">${escaparHtml(c.nombre)} (RNC: ${escaparHtml(c.rnc)})</option>`).join('')}
                                </select>
                                <input type="hidden" id="cli_nom">
                                <input type="hidden" id="cli_rnc">
                                <div style="margin-top:0.4rem; text-align:right;">
                                    <a href="#" onclick="navigate('ajustes')" style="font-size:0.75rem; color:var(--accent-primary); text-decoration:none;">⚙️ Crear nuevo cliente</a>
                                </div>
                            </div>
                            <div class="form-row">
                                <div class="form-group">
                                    <label for="mon_tot">Monto DOP *</label>
                                    <input type="number" id="mon_tot" step="0.01" class="form-control" placeholder="0.00" required>
                                </div>
                                <div class="form-group">
                                    <label for="ret_por">Retención %</label>
                                    <input type="number" id="ret_por" step="0.01" value="15.00" class="form-control">
                                </div>
                            </div>
                            <button type="submit" class="btn" style="width:100%; margin-top:0.5rem;">🚀 Registrar Factura</button>
                        </form>
                    </div>

                    <!-- FORMULARIO INFORMAL -->
                    <div id="tauri-form-informal" style="display: none;">
                        <form id="form-add-informal" onsubmit="appUI.handleAgregarIngresoInformal(event)">
                            <div class="form-group">
                                <label for="fecha_inf">Fecha *</label>
                                <input type="text" id="fecha_inf" class="form-control" value="${escaparHtml(hoyStr)}" placeholder="dd/mm/aaaa" required>
                            </div>
                            <div class="form-group">
                                <label for="monto_inf">Monto DOP *</label>
                                <input type="number" id="monto_inf" step="0.01" class="form-control" placeholder="0.00" required>
                            </div>
                            <div class="form-group">
                                <label for="desc_inf">Descripción *</label>
                                <input type="text" id="desc_inf" class="form-control" placeholder="Consultoría externa, ventas..." required>
                            </div>
                            <button type="submit" class="btn" style="width:100%; background: linear-gradient(135deg, #10b981, #059669); color: white; margin-top:0.5rem;">🚀 Registrar Ingreso</button>
                        </form>
                    </div>
                </div>

                <!-- Historial e Ingresos -->
                <div style="display: flex; flex-direction: column; gap: 1.5rem;">
                    <!-- Tabla Formales -->
                    <div class="card">
                        <h3 style="font-family: var(--font-heading); font-size: 1.1rem; margin-bottom: 1rem;">📄 Facturas Emitidas</h3>
                        ${ingresos.length > 0 ? `
                            <div class="table-responsive">
                                <table class="table-modern">
                                    <thead>
                                        <tr>
                                            <th>Factura</th>
                                            <th>Cliente</th>
                                            <th>Emisión</th>
                                            <th>Monto</th>
                                            <th>Retención</th>
                                            <th>Estado</th>
                                            <th>Acciones</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        ${ingresos.map(i => {
                                            return `
                                                <tr>
                                                    <td><strong>${escaparHtml(i.numero_factura)}</strong></td>
                                                    <td>${escaparHtml(i.cliente_nombre)}<br><span style="font-size:0.75rem; color:var(--text-muted);">RNC: ${escaparHtml(i.cliente_rnc)}</span></td>
                                                    <td>${escaparHtml(i.fecha_emision)}</td>
                                                    <td class="amount">DOP ${formato.importe(i.monto_total)}</td>
                                                    <td class="amount expense">DOP ${formato.importe(i.monto_retenido)}</td>
                                                    <td><span class="badge ${escaparHtml(i.estatus)}">${escaparHtml(i.estatus)}</span></td>
                                                    <td>
                                                        <div style="display:flex; gap:0.3rem;">
                                                            <!-- Corregir se ofrece también cobrada: un error de importe no
                                                                 debería quedar congelado porque el dinero ya entró. El ajuste
                                                                 de la cuenta lo resuelve el comando. -->
                                                            <button onclick="appUI.abrirEdicionFormal(${argumentoJs(JSON.stringify(i))})" class="btn btn-secondary" style="padding: 0.3rem 0.5rem; font-size:0.75rem; border:none; background:rgba(255,255,255,0.05);" title="${i.estatus === 'pagada' ? 'Corregir factura cobrada: ajustará la cuenta por la diferencia' : 'Corregir factura'}">✏️</button>
                                                            ${i.estatus === 'emitida' ? `
                                                                <button onclick="appUI.abrirCobroFormal(${i.id}, ${i.monto_total - i.monto_retenido})" class="btn" style="padding: 0.3rem 0.5rem; font-size:0.75rem;">💵 Cobrar</button>
                                                            ` : `
                                                                <span style="font-size:0.75rem; color:var(--text-muted); font-style:italic;">Dep: ${escaparHtml(i.institucion_deposito)}</span>
                                                            `}
                                                        </div>
                                                    </td>
                                                </tr>
                                            `;
                                        }).join('')}
                                    </tbody>
                                </table>
                            </div>
                        ` : `
                            <p style="color:var(--text-muted); text-align:center; padding:1rem;">No hay facturas registradas.</p>
                        `}
                    </div>

                    <!-- Tabla Informales -->
                    <div class="card">
                        <h3 style="font-family: var(--font-heading); font-size: 1.1rem; margin-bottom: 1rem; color: #10b981;">💸 Ingresos Informales</h3>
                        ${informales.length > 0 ? `
                            <div class="table-responsive">
                                <table class="table-modern">
                                    <thead>
                                        <tr>
                                            <th>Descripción</th>
                                            <th>Fecha</th>
                                            <th>Monto Esperado</th>
                                            <th>Estado</th>
                                            <th>Depósito</th>
                                            <th>Acciones</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        ${informales.map(inf => `
                                            <tr>
                                                <td><strong>${escaparHtml(inf.descripcion)}</strong></td>
                                                <td>${escaparHtml(inf.fecha)}</td>
                                                <td class="amount income">DOP ${formato.importe(inf.monto)}</td>
                                                <td><span class="badge ${inf.estatus === 'pagado' ? 'pagada' : 'emitida'}">${escaparHtml(inf.estatus)}</span></td>
                                                <td>
                                                    ${inf.estatus === 'pagado' ? `
                                                        <span style="font-size:0.75rem; color:var(--text-muted);">En ${escaparHtml(inf.institucion_deposito)} el ${escaparHtml(inf.fecha_pago)}</span>
                                                    ` : '-'}
                                                </td>
                                                <td>
                                                    ${inf.estatus === 'pendiente' ? `
                                                        <button onclick="appUI.abrirCobroInformal(${inf.id}, ${inf.monto})" class="btn" style="padding: 0.3rem 0.6rem; font-size:0.75rem; background: linear-gradient(135deg, #10b981, #059669); color: white;">💵 Cobrar</button>
                                                    ` : '-'}
                                                </td>
                                            </tr>
                                        `).join('')}
                                    </tbody>
                                </table>
                            </div>
                        ` : `
                            <p style="color:var(--text-muted); text-align:center; padding:1rem;">No hay ingresos informales registrados.</p>
                        `}
                    </div>
                </div>
            </div>
        `;
    }

    async handleAgregarIngreso(e: EventoDeFormulario): Promise<void> {
        e.preventDefault();
        const { api, avisos, enrutador, dom } = this.dep;
        const fac = dom.elemento<Campo>('num_fac').value;
        const fec = dom.elemento<Campo>('fec_em').value;
        const cli = dom.elemento<Campo>('cli_nom').value;
        const rnc = dom.elemento<Campo>('cli_rnc').value;
        const mon = Number(dom.elemento<Campo>('mon_tot').value);
        const ret = Number(dom.elemento<Campo>('ret_por').value);

        try {
            await api.crearIngreso({
                numero_factura: fac,
                rnc_cliente: rnc,
                nombre_cliente: cli,
                fecha_emision: fec,
                monto_total: mon,
                porcentaje_retencion: ret
            });
            avisos.mostrar("Factura registrada exitosamente.");
            await enrutador.mostrar('ingresos');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    async handleAgregarIngresoInformal(e: EventoDeFormulario): Promise<void> {
        e.preventDefault();
        const { api, avisos, enrutador, dom } = this.dep;
        const fec = dom.elemento<Campo>('fecha_inf').value;
        // Texto, tal cual se escribió: el céntimo lo deciden los dígitos (convención de 1.21.0).
        const mon = dom.elemento<Campo>('monto_inf').value.trim();
        const des = dom.elemento<Campo>('desc_inf').value;

        try {
            await api.crearIngresoInformal(fec, des, mon);
            avisos.mostrar("Ingreso informal guardado.");
            await enrutador.mostrar('ingresos');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    handleSelectCliente(val: string): void {
        const { dom } = this.dep;
        const select = dom.elemento<HTMLSelectElement>('cli_select');
        const option = select.options[select.selectedIndex];
        if (option) {
            dom.elemento<Campo>('cli_nom').value = option.getAttribute('data-nombre') || "";
            dom.elemento<Campo>('cli_rnc').value = option.getAttribute('data-rnc') || "";
        }
    }

    abrirEdicionFormal(iJsonStr: string): void {
        const { api, avisos, dom, modales } = this.dep;
        const i: Ingreso = JSON.parse(iJsonStr);
        api.obtenerClientes().then(clientes => {
            const html = `
                <div class="card" style="width: 450px; background: var(--bg-surface-opaque);">
                    <h3 style="font-family: var(--font-heading); margin-bottom: 0.6rem;">✏️ Corregir / Editar Factura</h3>
                    ${i.estatus === 'pagada' ? `
                        <p style="font-size:0.75rem; color:var(--color-warning); margin-bottom:0.8rem; line-height:1.4;">
                            Esta factura ya está cobrada en <strong>${escaparHtml(i.institucion_deposito || 'ninguna cuenta')}</strong>.
                            Al corregirla se dará por cobrada <strong>por su neto completo</strong>,
                            y esa cuenta se ajustará por la diferencia.
                        </p>
                        <div style="border:1px solid var(--border-color); border-radius:var(--radius-sm); padding:0.6rem; margin-bottom:0.8rem;">
                            <label style="display:flex; align-items:center; gap:0.4rem; font-size:0.8rem; cursor:pointer;">
                                <input type="checkbox" id="edit_parcial_chk_${i.id}" onchange="appUI.alternarCobroParcial(${i.id})">
                                Entró solo una parte del neto
                            </label>
                            <div id="edit_parcial_caja_${i.id}" hidden style="margin-top:0.5rem;">
                                <label style="font-size:0.7rem; color:var(--text-muted);">Importe realmente cobrado</label>
                                <input type="number" step="0.01" min="0" id="edit_parcial_mon_${i.id}" class="form-control" value="${i.monto_recibido ?? ''}" style="padding:0.4rem;">
                                <p style="font-size:0.7rem; color:var(--text-muted); margin-top:0.3rem; line-height:1.3;">
                                    La diferencia con el neto queda como pendiente de cobro en vez de darse por saldada.
                                </p>
                            </div>
                        </div>
                    ` : ''}
                    <form onsubmit="appUI.handleEdicionFormalSubmit(event, ${i.id})">
                        <div class="form-group">
                            <label>Número de Factura *</label>
                            <input type="text" id="edit_num_fac_${i.id}" class="form-control" value="${escaparHtml(i.numero_factura)}" required>
                        </div>
                        <div class="form-group">
                            <label>Fecha de Emisión *</label>
                            <input type="text" id="edit_fec_em_${i.id}" class="form-control" value="${escaparHtml(i.fecha_emision)}" required>
                        </div>
                        <div class="form-group">
                            <label>Cliente *</label>
                            <select id="edit_cli_select_${i.id}" class="form-control" required>
                                ${clientes.map(c => `<option value="${c.id}" ${c.id === i.cliente_id ? 'selected' : ''}>${escaparHtml(c.nombre)} (RNC: ${escaparHtml(c.rnc)})</option>`).join('')}
                            </select>
                        </div>
                        <div class="form-row">
                            <div class="form-group">
                                <label>Monto DOP *</label>
                                <input type="number" step="0.01" id="edit_mon_tot_${i.id}" class="form-control" value="${i.monto_total}" required>
                            </div>
                            <div class="form-group">
                                <label>Retención % *</label>
                                <input type="number" step="0.01" id="edit_ret_por_${i.id}" class="form-control" value="${i.porcentaje_retencion}" required>
                            </div>
                        </div>
                        <div style="display:flex; justify-content:flex-end; gap:0.5rem; margin-top:1.2rem;">
                            <button type="button" onclick="elemento('modal-edit-for-${i.id}').remove()" class="btn btn-secondary">Cancelar</button>
                            <button type="submit" class="btn">Guardar Cambios</button>
                        </div>
                    </form>
                </div>
            `;
            modales.abrir(`modal-edit-for-${i.id}`, html);
            // El estado viaja con el modal para que el envío sepa si mover
            // dinero exige avisar antes.
            const overlay = dom.elemento(`modal-edit-for-${i.id}`);
            overlay.dataset.cobrada = i.estatus === 'pagada' ? 'si' : 'no';
            overlay.dataset.recibido = String(i.monto_recibido ?? 0);
        }).catch(err => avisos.mostrar(String(err), 'error'));
    }

    /// Despliega el importe del cobro parcial solo cuando se declara.
    ///
    /// Oculto por defecto porque lo normal es el cobro completo: un campo
    /// siempre visible invitaría a rellenarlo y convertiría la excepción en
    /// costumbre.
    alternarCobroParcial(id: number): void {
        const { dom } = this.dep;
        const caja = dom.buscar(`edit_parcial_caja_${id}`);
        const marca = dom.buscar<HTMLInputElement>(`edit_parcial_chk_${id}`);
        if (caja && marca) {
            caja.hidden = !marca.checked;
        }
    }

    async handleEdicionFormalSubmit(e: EventoDeFormulario, id: number): Promise<void> {
        e.preventDefault();
        const { api, avisos, formato, enrutador, dom, dialogos, motivo: pedidorDeMotivo } = this.dep;
        const fac = dom.elemento<Campo>(`edit_num_fac_${id}`).value;
        const fec = dom.elemento<Campo>(`edit_fec_em_${id}`).value;
        const cliId = Number(dom.elemento<Campo>(`edit_cli_select_${id}`).value);
        const mon = Number(dom.elemento<Campo>(`edit_mon_tot_${id}`).value);
        const ret = Number(dom.elemento<Campo>(`edit_ret_por_${id}`).value);

        // Corregir una factura cobrada mueve dinero, así que se confirma con
        // las cifras delante en vez de con una advertencia genérica.
        const cobrada = dom.buscar(`modal-edit-for-${id}`)?.dataset?.cobrada === 'si';
        let parcial: string | null = null;
        let motivo: string | null = null;

        if (cobrada) {
            const quiereParcial = dom.buscar<HTMLInputElement>(`edit_parcial_chk_${id}`)?.checked;
            const neto = Math.round((mon - mon * (ret / 100)) * 100) / 100;

            if (quiereParcial) {
                // Va como texto, igual que los otros importes que no pasan
                // por la validación del formulario: este campo se lee con
                // `.value` directamente, así que el `step="0.01"` nunca llega
                // a comprobarse. Se valida la **forma**, y el céntimo lo
                // decide el núcleo.
                const texto = dom.elemento<Campo>(`edit_parcial_mon_${id}`).value;
                parcial = texto.trim() === '' ? null : texto.trim();
                if (parcial === null || !/^\d*\.?\d+$/.test(parcial.replace(/,/g, ''))) {
                    avisos.mostrar("Indica cuánto se cobró de verdad, o desmarca el cobro parcial.", "error");
                    return;
                }
                // La cota sí es una comparación, no una decisión de céntimo:
                // convertir aquí no compromete el importe que se envía.
                if (Number(parcial.replace(/,/g, '')) > neto) {
                    avisos.mostrar(`Un cobro parcial no puede superar el neto (${formato.importe(neto)}).`, "error");
                    return;
                }
            }

            const cobrado = parcial === null ? neto : Number(parcial.replace(/,/g, ''));
            const recibidoAntes = Number(
                dom.buscar(`modal-edit-for-${id}`)?.dataset?.recibido ?? 0
            );
            const ajuste = Math.round((cobrado - recibidoAntes) * 100) / 100;

            // El motivo se pide **solo si se mueve un saldo**. Exigirlo para
            // corregir una fecha sería fricción sin riesgo, y la fricción que
            // no protege enseña a escribir motivos de trámite.
            if (ajuste !== 0) {
                motivo = await pedidorDeMotivo.pedir(
                    `Vas a corregir una factura ya cobrada`,
                    `Esto **mueve un saldo real**: la cuenta de depósito se ajustará en ` +
                    `DOP ${formato.importe(ajuste)}, y solo quedará el caso de auditoría que abras.`
                );
                if (motivo === null) return;
            } else {
                const sigue = await dialogos.confirmar(
                    "Esta factura ya está cobrada.\n\n" +
                    `Pasará a constar cobrada por DOP ${formato.importe(cobrado)}` +
                    (parcial !== null ? ` de un neto de ${formato.importe(neto)}.` : " (neto completo).") +
                    "\n\nNingún saldo cambia.\n\n¿Continuar?"
                );
                if (!sigue) return;
            }
        }

        try {
            const resumen = await api.actualizarIngreso(id, fac, cliId, fec, mon, ret, parcial, motivo);
            avisos.mostrar(resumen || "Factura corregida.");
            dom.elemento(`modal-edit-for-${id}`).remove();
            await enrutador.mostrar('ingresos');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    // --- COBROS DE INGRESOS (MODALES) ---
    async abrirCobroFormal(id: number, sugerido: number): Promise<void> {
        const { api, avisos, formato, modales } = this.dep;
        try {
            const cuentas = await api.obtenerCuentas();
            const html = `
                <div class="card" style="width: 400px; background: var(--bg-surface-opaque);">
                    <h3 style="font-family: var(--font-heading); margin-bottom: 0.6rem;">💰 Registrar Cobro Factura</h3>
                    <p style="font-size: 0.8rem; color: var(--text-secondary); margin-bottom: 1.2rem;">
                        Monto neto sugerido a recibir: <strong>DOP ${formato.importe(sugerido)}</strong>
                    </p>
                    <form onsubmit="appUI.handleCobroFormalSubmit(event, ${id})">
                        <div class="form-group">
                            <label>Cuenta de Depósito *</label>
                            <select id="cob_ban_${id}" class="form-control" required>
                                <option value="" disabled selected>Seleccione cuenta...</option>
                                ${cuentas.map(c => `<option value="${c.id}" data-divisa="${escaparHtml(c.divisa)}">${escaparHtml(c.nombre)} (${escaparHtml(c.divisa)}) - Bal: ${escaparHtml(c.divisa)} ${formato.importe(c.balance_actual)}</option>`).join('')}
                            </select>
                        </div>
                        <div class="form-row">
                            <div class="form-group">
                                <label>Fecha *</label>
                                <input type="text" id="cob_fec_${id}" class="form-control" placeholder="dd/mm/aaaa" required>
                            </div>
                            <div class="form-group">
                                <label>Monto Recibido *</label>
                                <input type="number" step="0.01" id="cob_mon_${id}" class="form-control" value="${sugerido.toFixed(2)}" required>
                            </div>
                        </div>
                        <div style="display:flex; justify-content:flex-end; gap:0.5rem; margin-top:1.2rem;">
                            <button type="button" onclick="elemento('modal-cobro-for-${id}').remove()" class="btn btn-secondary">Cancelar</button>
                            <button type="submit" class="btn">Cobrar</button>
                        </div>
                    </form>
                </div>
            `;
            modales.abrir(`modal-cobro-for-${id}`, html);
        } catch (err) {
            avisos.mostrar("Error al obtener cuentas: " + String(err), 'error');
        }
    }

    async handleCobroFormalSubmit(e: EventoDeFormulario, id: number): Promise<void> {
        e.preventDefault();
        const { api, avisos, enrutador, dom } = this.dep;
        const ban = dom.elemento<Campo>(`cob_ban_${id}`).value;
        const fec = dom.elemento<Campo>(`cob_fec_${id}`).value;
        const mon = Number(dom.elemento<Campo>(`cob_mon_${id}`).value);

        try {
            await api.marcarIngresoPagado(id, ban, fec, mon);
            avisos.mostrar("Factura marcada como pagada.");
            dom.elemento(`modal-cobro-for-${id}`).remove();
            await enrutador.mostrar('ingresos');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    async abrirCobroInformal(id: number, sugerido: number): Promise<void> {
        const { api, avisos, formato, modales } = this.dep;
        try {
            const cuentas = await api.obtenerCuentas();
            const html = `
                <div class="card" style="width: 400px; background: var(--bg-surface-opaque);">
                    <h3 style="font-family: var(--font-heading); margin-bottom: 0.6rem; color: #10b981;">💰 Registrar Cobro Informal</h3>
                    <p style="font-size: 0.8rem; color: var(--text-secondary); margin-bottom: 1.2rem;">
                        Monto esperado: <strong>DOP ${formato.importe(sugerido)}</strong>
                    </p>
                    <form onsubmit="appUI.handleCobroInformalSubmit(event, ${id})">
                        <div class="form-group">
                            <label>Cuenta de Depósito *</label>
                            <select id="cob_ban_inf_${id}" class="form-control" required>
                                <option value="" disabled selected>Seleccione cuenta...</option>
                                ${cuentas.map(c => `<option value="${c.id}" data-divisa="${escaparHtml(c.divisa)}">${escaparHtml(c.nombre)} (${escaparHtml(c.divisa)}) - Bal: ${escaparHtml(c.divisa)} ${formato.importe(c.balance_actual)}</option>`).join('')}
                            </select>
                        </div>
                        <div class="form-row">
                            <div class="form-group">
                                <label>Fecha *</label>
                                <input type="text" id="cob_fec_inf_${id}" class="form-control" placeholder="dd/mm/aaaa" required>
                            </div>
                            <div class="form-group">
                                <label>Monto Recibido *</label>
                                <input type="number" step="0.01" id="cob_mon_inf_${id}" class="form-control" value="${sugerido.toFixed(2)}" required>
                            </div>
                        </div>
                        <div style="display:flex; justify-content:flex-end; gap:0.5rem; margin-top:1.2rem;">
                            <button type="button" onclick="elemento('modal-cobro-inf-${id}').remove()" class="btn btn-secondary">Cancelar</button>
                            <button type="submit" class="btn" style="background: linear-gradient(135deg, #10b981, #059669); color:white;">Cobrar</button>
                        </div>
                    </form>
                </div>
            `;
            modales.abrir(`modal-cobro-inf-${id}`, html);
        } catch (err) {
            avisos.mostrar("Error al obtener cuentas: " + String(err), 'error');
        }
    }

    async handleCobroInformalSubmit(e: EventoDeFormulario, id: number): Promise<void> {
        e.preventDefault();
        const { api, avisos, enrutador, dom } = this.dep;
        const ban = dom.elemento<Campo>(`cob_ban_inf_${id}`).value;
        const fec = dom.elemento<Campo>(`cob_fec_inf_${id}`).value;
        const mon = Number(dom.elemento<Campo>(`cob_mon_inf_${id}`).value);

        try {
            await api.marcarInformalPagado(id, ban, fec, mon);
            avisos.mostrar("Ingreso informal registrado como pagado.");
            dom.elemento(`modal-cobro-inf-${id}`).remove();
            await enrutador.mostrar('ingresos');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }
}

/**
 * Lo que `window.appUI` expone de esta vista: un puente que delega en la
 * instancia. Los atributos en línea no cambian.
 */
export function puenteIngresos(vista: VistaIngresos): {
    handleAgregarIngreso: (e: EventoDeFormulario) => Promise<void>;
    handleAgregarIngresoInformal: (e: EventoDeFormulario) => Promise<void>;
    handleSelectCliente: (val: string) => void;
    abrirEdicionFormal: (iJsonStr: string) => void;
    alternarCobroParcial: (id: number) => void;
    handleEdicionFormalSubmit: (e: EventoDeFormulario, id: number) => Promise<void>;
    abrirCobroFormal: (id: number, sugerido: number) => Promise<void>;
    handleCobroFormalSubmit: (e: EventoDeFormulario, id: number) => Promise<void>;
    abrirCobroInformal: (id: number, sugerido: number) => Promise<void>;
    handleCobroInformalSubmit: (e: EventoDeFormulario, id: number) => Promise<void>;
} {
    return {
        handleAgregarIngreso: e => vista.handleAgregarIngreso(e),
        handleAgregarIngresoInformal: e => vista.handleAgregarIngresoInformal(e),
        handleSelectCliente: val => vista.handleSelectCliente(val),
        abrirEdicionFormal: iJsonStr => vista.abrirEdicionFormal(iJsonStr),
        alternarCobroParcial: id => vista.alternarCobroParcial(id),
        handleEdicionFormalSubmit: (e, id) => vista.handleEdicionFormalSubmit(e, id),
        abrirCobroFormal: (id, sugerido) => vista.abrirCobroFormal(id, sugerido),
        handleCobroFormalSubmit: (e, id) => vista.handleCobroFormalSubmit(e, id),
        abrirCobroInformal: (id, sugerido) => vista.abrirCobroInformal(id, sugerido),
        handleCobroInformalSubmit: (e, id) => vista.handleCobroInformalSubmit(e, id),
    };
}
