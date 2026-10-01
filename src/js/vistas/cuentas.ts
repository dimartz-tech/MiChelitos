// Pestaña «Cuentas de Ahorro» (diseño «B»: clase con dependencias inyectadas).
//
// No usa `this` de ningún objeto mezclado, ni `AppAPI`, `elemento()`, `buscar()`
// ni `appUI` globales: todo entra por el constructor, así que se puede probar en
// Node con dobles (`pruebas/js/vistas/cuentas.test.js`). Los cuerpos son los de
// `ui.ts`; solo cambian los accesos a lo compartido (ver `division_de_ui_limpia.md`).

import type { ApiDe, Avisos, Dom, Enrutador, Formato, Pantalla, Reloj, Vista } from '../ui/servicios';

import { escaparHtml } from '../nucleo/html.js';

type ApiCuentas = ApiDe<'obtenerCuentas' | 'obtenerTransaccionesCuentas' | 'transferirEntreCuentas'>;

export interface DependenciasCuentas {
    api: ApiCuentas;
    avisos: Avisos;
    formato: Formato;
    enrutador: Enrutador;
    pantalla: Pantalla;
    dom: Dom;
    ahora: Reloj;
}

/** Los manejadores en línea (`onsubmit`/`oninput`/`onchange` → `appUI.…`) que la plantilla escribe. */
export const MANEJADORES_CUENTAS = ['handleTransferirCuentas', 'rotularDivisasTransferencia'] as const;
export type ManejadorCuentas = (typeof MANEJADORES_CUENTAS)[number];

/** Un formulario envía un evento con `preventDefault`; solo eso se usa. */
interface EventoDeFormulario {
    preventDefault(): void;
}

export class VistaCuentas implements Vista {
    constructor(private readonly dep: DependenciasCuentas) {}

    // --- RENDER: CUENTAS DE AHORRO ---
    async render(): Promise<void> {
        const { api, formato, pantalla, ahora } = this.dep;
        const cuentas = await api.obtenerCuentas();
        const transacciones = await api.obtenerTransaccionesCuentas();

        const hoy = ahora();
        const hoyStr = hoy.getDate().toString().padStart(2, '0') + '/' + (hoy.getMonth() + 1).toString().padStart(2, '0') + '/' + hoy.getFullYear();

        pantalla.contenido.innerHTML = `
            <div class="section-title">
                <h1>Cuentas de Ahorro</h1>
                <span class="subtitle">Gestión de saldos bancarios y transferencias entre divisas</span>
            </div>

            <div class="responsive-split-grid">
                <!-- Acciones rápidas (Transferencias / Cambio Divisas) -->
                <div style="display:flex; flex-direction:column; gap:1.5rem;">
                    <!-- TARJETAS DE SALDOS RESUMEN -->
                    <div class="card" style="height: fit-content;">
                        <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 1rem;">💰 Saldos Totales</h3>
                        <div style="display:flex; flex-direction:column; gap:0.6rem;">
                            <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.6rem; border-radius:var(--radius-sm); display:flex; justify-content:space-between; align-items:center;">
                                <span style="font-size:0.8rem; color:var(--text-secondary);">Total DOP</span>
                                <strong style="font-size:1.1rem; color:var(--accent-primary);">DOP ${formato.importe(cuentas.filter(c => c.divisa === 'DOP').reduce((sum, c) => sum + c.balance_actual, 0))}</strong>
                            </div>
                            <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.6rem; border-radius:var(--radius-sm); display:flex; justify-content:space-between; align-items:center;">
                                <span style="font-size:0.8rem; color:var(--text-secondary);">Total USD</span>
                                <strong style="font-size:1.1rem; color: #10b981;">USD ${formato.importe(cuentas.filter(c => c.divisa === 'USD').reduce((sum, c) => sum + c.balance_actual, 0))}</strong>
                            </div>
                        </div>
                    </div>

                    <!-- FORMULARIO DE TRANSFERENCIA / CAMBIO DIVISAS -->
                    <div class="card" style="height: fit-content;">
                        <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 0.5rem;">🔄 Transferencia / Cambio</h3>
                        <p style="font-size:0.75rem; color:var(--text-secondary); margin-bottom:1rem;">Permite transferir fondos y registrar el cambio de divisas (Pesos <-> Dólares).</p>
                        <form id="form-transfer-cuentas" onsubmit="appUI.handleTransferirCuentas(event)">
                            <div class="form-group">
                                <label for="tra_fec">Fecha *</label>
                                <input type="text" id="tra_fec" class="form-control" value="${escaparHtml(hoyStr)}" required>
                            </div>
                            <div class="form-group">
                                <label for="tra_ori">Cuenta Origen *</label>
                                <select id="tra_ori" class="form-control" onchange="appUI.rotularDivisasTransferencia()" required>
                                    <option value="" disabled selected>Seleccione...</option>
                                    ${cuentas.map(c => `<option value="${c.id}" data-divisa="${escaparHtml(c.divisa)}">${escaparHtml(c.nombre)} (${escaparHtml(c.divisa)}) - Bal: ${escaparHtml(c.divisa)} ${formato.importe(c.balance_actual)}</option>`).join('')}
                                </select>
                            </div>
                            <div class="form-group">
                                <label for="tra_des">Cuenta Destino *</label>
                                <select id="tra_des" class="form-control" onchange="appUI.rotularDivisasTransferencia()" required>
                                    <option value="" disabled selected>Seleccione...</option>
                                    ${cuentas.map(c => `<option value="${c.id}" data-divisa="${escaparHtml(c.divisa)}">${escaparHtml(c.nombre)} (${escaparHtml(c.divisa)}) - Bal: ${escaparHtml(c.divisa)} ${formato.importe(c.balance_actual)}</option>`).join('')}
                                </select>
                            </div>
                            <div id="tra_aviso_divisas" style="display:none; background:rgba(245,158,11,0.08); border:1px solid rgba(245,158,11,0.25); border-radius:var(--radius-sm); padding:0.6rem 0.75rem; font-size:0.72rem; color:var(--text-secondary); margin-bottom:1rem; line-height:1.5;"></div>

                            <div class="form-row">
                                <div class="form-group">
                                    <label for="tra_mon_ori">Monto Débito (Origen) <span id="tra_div_ori" style="color:var(--accent-primary);"></span> *</label>
                                    <input type="number" id="tra_mon_ori" step="0.01" class="form-control" placeholder="0.00" required>
                                </div>
                                <div class="form-group">
                                    <label for="tra_mon_des">Monto Crédito (Destino) <span id="tra_div_des" style="color:var(--accent-primary);"></span> *</label>
                                    <input type="number" id="tra_mon_des" step="0.01" class="form-control" placeholder="0.00" required>
                                </div>
                            </div>
                            <div class="form-group">
                                <label for="tra_car">Comisión / Cargo (Divisa Origen)</label>
                                <input type="number" id="tra_car" step="0.01" value="0.00" class="form-control">
                            </div>
                            <div class="form-group">
                                <label for="tra_des_txt">Descripción *</label>
                                <input type="text" id="tra_des_txt" class="form-control" placeholder="Compra USD, transferencia interna..." required>
                            </div>
                            <button type="submit" class="btn" style="width:100%; margin-top:0.5rem; background: linear-gradient(135deg, var(--accent-primary), #00cdac); color:white;">🚀 Ejecutar Transacción</button>
                        </form>
                    </div>
                </div>

                <!-- Historial de Transacciones y Listas -->
                <div style="display:flex; flex-direction:column; gap:1.5rem;">
                    <!-- Cuentas Actuales -->
                    <div class="card">
                        <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 1rem;">🏦 Cuentas Registradas</h3>
                        <div style="display:grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap:1rem;">
                            ${cuentas.length > 0 ? cuentas.map(c => `
                                <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:1rem; border-radius:var(--radius-md); position:relative;">
                                    <div style="font-size:0.75rem; color:var(--text-muted); font-weight:bold; letter-spacing:1px; margin-bottom:0.3rem;">AHORRO - ${escaparHtml(c.divisa)}</div>
                                    <div style="font-family: var(--font-heading); font-size:1.1rem; font-weight:bold; margin-bottom:0.5rem; white-space:nowrap; overflow:hidden; text-overflow:ellipsis;" title="${escaparHtml(c.nombre)}">${escaparHtml(c.nombre)}</div>
                                    <div style="font-size:1.25rem; font-weight:bold; color: ${c.divisa === 'USD' ? '#10b981' : 'var(--accent-primary)'};">${escaparHtml(c.divisa)} ${formato.importe(c.balance_actual)}</div>
                                </div>
                            `).join('') : `
                                <p style="color:var(--text-muted); grid-column: 1/-1; text-align:center; padding:1rem;">No hay cuentas de ahorro registradas. Ve a Ajustes para agregarlas.</p>
                            `}
                        </div>
                    </div>

                    <!-- Historial -->
                    <div class="card">
                        <h3 style="font-family: var(--font-heading); font-size: 1.1rem; margin-bottom: 1rem;">📋 Historial de Transferencias</h3>
                        ${transacciones.length > 0 ? `
                            <div class="table-responsive">
                                <table class="table-modern">
                                    <thead>
                                        <tr>
                                            <th>Fecha</th>
                                            <th>Descripción</th>
                                            <th>Origen</th>
                                            <th>Débito</th>
                                            <th>Destino</th>
                                            <th>Crédito</th>
                                            <th>Cargo</th>
                                            <th>Tasa</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        ${transacciones.map(t => `
                                            <tr>
                                                <td>${escaparHtml(t.fecha)}</td>
                                                <td><strong>${escaparHtml(t.descripcion || '-')}</strong></td>
                                                <td>${escaparHtml(t.cuenta_origen_nombre)}</td>
                                                <td class="amount expense">${t.monto_origen > 0 ? `- ${t.monto_origen.toLocaleString('es-DO', {minimumFractionDigits: 2, maximumFractionDigits: 2})}` : '-'}</td>
                                                <td>${escaparHtml(t.cuenta_destino_nombre)}</td>
                                                <td class="amount income">${t.monto_destino > 0 ? `+ ${t.monto_destino.toLocaleString('es-DO', {minimumFractionDigits: 2, maximumFractionDigits: 2})}` : '-'}</td>
                                                <td class="amount expense">${t.cargo > 0 ? `${t.cargo.toLocaleString('es-DO', {minimumFractionDigits: 2, maximumFractionDigits: 2})}` : '-'}</td>
                                                <td style="font-family:monospace; font-size:0.8rem;">${t.tasa_cambio.toFixed(4)}</td>
                                            </tr>
                                        `).join('')}
                                    </tbody>
                                </table>
                            </div>
                        ` : `
                            <p style="color:var(--text-muted); text-align:center; padding:1rem;">No hay transferencias entre cuentas registradas.</p>
                        `}
                    </div>
                </div>
            </div>
        `;
    }

    /**
     * Rotula cada importe con la divisa de su cuenta, y avisa si cruzan.
     *
     * El formulario pide dos números sueltos: la divisa de cada uno se deduce
     * de la cuenta elegida, no se declara. El backend no puede detectar que
     * quien teclea 6 000 pensando en pesos ha elegido una cuenta en dólares,
     * porque no hay ninguna declaración que contradecir — acreditará seis mil
     * dólares. Decir la divisa junto al campo es lo único que ataja ese error
     * donde se comete.
     */
    rotularDivisasTransferencia(): void {
        const { dom } = this.dep;
        const divisaDe = (id: string) => dom.buscar<HTMLSelectElement>(id)?.selectedOptions?.[0]?.dataset?.divisa ?? '';
        const origen = divisaDe('tra_ori');
        const destino = divisaDe('tra_des');

        const rotulo = (id: string, divisa: string) => {
            const el = dom.buscar(id);
            if (el) el.textContent = divisa ? `en ${divisa}` : '';
        };
        rotulo('tra_div_ori', origen);
        rotulo('tra_div_des', destino);

        const aviso = dom.buscar('tra_aviso_divisas');
        if (!aviso) return;

        if (origen && destino && origen !== destino) {
            aviso.style.display = 'block';
            aviso.innerHTML = `⚠️ Esta transferencia cruza divisas: el débito va en <strong>${escaparHtml(origen)}</strong> y el crédito en <strong>${escaparHtml(destino)}</strong>. Comprueba que cada importe esté en su divisa — la tasa se deduce de los dos.`;
        } else {
            aviso.style.display = 'none';
            aviso.innerHTML = '';
        }
    }

    async handleTransferirCuentas(e: EventoDeFormulario): Promise<void> {
        e.preventDefault();
        const { api, avisos, enrutador, dom } = this.dep;
        const fec = dom.elemento<Campo>('tra_fec').value;
        const ori = Number(dom.elemento<Campo>('tra_ori').value);
        const des = Number(dom.elemento<Campo>('tra_des').value);
        const monOri = Number(dom.elemento<Campo>('tra_mon_ori').value);
        const monDes = Number(dom.elemento<Campo>('tra_mon_des').value);
        const car = Number(dom.elemento<Campo>('tra_car').value);
        const txt = dom.elemento<Campo>('tra_des_txt').value;
        try {
            await api.transferirEntreCuentas(fec, ori, des, monOri, monDes, car, txt);
            avisos.mostrar("Transacción ejecutada con éxito.");
            await enrutador.mostrar('cuentas');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }
}

/**
 * Lo que `window.appUI` expone de esta vista: un puente que delega en la
 * instancia. Los atributos `onsubmit`/`oninput` no cambian.
 */
export function puenteCuentas(vista: VistaCuentas): {
    handleTransferirCuentas: (e: EventoDeFormulario) => Promise<void>;
    rotularDivisasTransferencia: () => void;
} {
    return {
        handleTransferirCuentas: e => vista.handleTransferirCuentas(e),
        rotularDivisasTransferencia: () => vista.rotularDivisasTransferencia(),
    };
}
