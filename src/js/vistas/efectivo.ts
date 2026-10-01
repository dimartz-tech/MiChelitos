// Pestaña «Caja y Efectivo» (diseño «B»: clase con dependencias inyectadas).
//
// No usa `this` de ningún objeto mezclado, ni `AppAPI`, `elemento()` ni
// `appUI` globales: todo entra por el constructor. Por eso se puede probar en
// Node con dobles (`pruebas/js/vistas/efectivo.test.js`). Los cuerpos son los
// de `ui.ts`; solo cambian los accesos a lo compartido (ver
// `division_de_ui_limpia.md`, §6).

import type { CuentaAhorro } from '../tipos-ipc';
import type { ApiDe, Avisos, Dom, Enrutador, Formato, Pantalla, Reloj, Vista } from '../ui/servicios';

type ApiEfectivo = ApiDe<'obtenerCuentas' | 'crearCobroEfectivoInformal' | 'transferirEntreCuentas'>;

export interface DependenciasEfectivo {
    api: ApiEfectivo;
    avisos: Avisos;
    formato: Formato;
    enrutador: Enrutador;
    pantalla: Pantalla;
    dom: Dom;
    ahora: Reloj;
}

/** Los manejadores en línea (`onsubmit="appUI.…"`) que la plantilla escribe. */
export const MANEJADORES_EFECTIVO = ['handleAgregarEfectivoInformal', 'handleRetirarAEfectivo'] as const;
export type ManejadorEfectivo = (typeof MANEJADORES_EFECTIVO)[number];

/** Un formulario envía un evento con `preventDefault`; solo eso se usa. */
interface EventoDeFormulario {
    preventDefault(): void;
}

export class VistaEfectivo implements Vista {
    constructor(private readonly dep: DependenciasEfectivo) {}

    // --- RENDER: EFECTIVO ---
    async render(): Promise<void> {
        const { api, formato, pantalla, ahora } = this.dep;
        const cuentas: CuentaAhorro[] = await api.obtenerCuentas();
        const cuentasAhorro = cuentas.filter(c => c.nombre !== 'Efectivo DOP' && c.nombre !== 'Efectivo USD');
        const efectivoDop = cuentas.find(c => c.nombre === 'Efectivo DOP') || { balance_actual: 0 };
        const efectivoUsd = cuentas.find(c => c.nombre === 'Efectivo USD') || { balance_actual: 0 };

        const hoy = ahora();
        const hoyStr = hoy.getDate().toString().padStart(2, '0') + '/' + (hoy.getMonth() + 1).toString().padStart(2, '0') + '/' + hoy.getFullYear();

        pantalla.contenido.innerHTML = `
            <div class="section-title">
                <h1>Caja y Efectivo</h1>
                <span class="subtitle">Gestión de dinero físico y flujo de caja</span>
            </div>

            <div class="responsive-split-grid">
                <!-- PANEL IZQUIERDO: BALANCES Y AGREGAR SALDO -->
                <div style="display:flex; flex-direction:column; gap:1.5rem;">
                    <!-- TARJETAS DE SALDOS EFECTIVO -->
                    <div class="card" style="height: fit-content;">
                        <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 1rem;">💵 Balance en Efectivo</h3>
                        <div style="display:flex; flex-direction:column; gap:0.6rem;">
                            <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.8rem; border-radius:var(--radius-md); display:flex; justify-content:space-between; align-items:center;">
                                <span style="font-size:0.85rem; color:var(--text-secondary);">Efectivo DOP</span>
                                <strong style="font-size:1.3rem; color:var(--accent-primary);">DOP ${formato.importe(efectivoDop.balance_actual)}</strong>
                            </div>
                            <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.8rem; border-radius:var(--radius-md); display:flex; justify-content:space-between; align-items:center;">
                                <span style="font-size:0.85rem; color:var(--text-secondary);">Efectivo USD</span>
                                <strong style="font-size:1.3rem; color: #10b981;">USD ${formato.importe(efectivoUsd.balance_actual)}</strong>
                            </div>
                        </div>
                    </div>

                    <!-- AGREGAR SALDO VIA INGRESO INFORMAL -->
                    <div class="card" style="height: fit-content;">
                        <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 0.5rem;">💸 Entrada Informal a Efectivo</h3>
                        <p style="font-size:0.75rem; color:var(--text-secondary); margin-bottom:1rem;">Registra una entrada de dinero informal directamente en caja.</p>
                        <form id="form-add-efectivo-informal" onsubmit="appUI.handleAgregarEfectivoInformal(event)">
                            <div class="form-group">
                                <label for="efe_inf_fec">Fecha *</label>
                                <input type="text" id="efe_inf_fec" class="form-control" value="${hoyStr}" required>
                            </div>
                            <div class="form-row">
                                <div class="form-group">
                                    <label for="efe_inf_mon">Monto *</label>
                                    <input type="number" id="efe_inf_mon" step="0.01" class="form-control" placeholder="0.00" required>
                                </div>
                                <div class="form-group">
                                    <label for="efe_inf_div">Divisa *</label>
                                    <select id="efe_inf_div" class="form-control">
                                        <option value="DOP" selected>DOP</option>
                                        <option value="USD">USD</option>
                                    </select>
                                </div>
                            </div>
                            <div class="form-group">
                                <label for="efe_inf_des">Descripción *</label>
                                <input type="text" id="efe_inf_des" class="form-control" placeholder="Cobro servicio informal..." required>
                            </div>
                            <button type="submit" class="btn" style="width:100%; margin-top:0.5rem; background: linear-gradient(135deg, #10b981, #059669); color:white;">💵 Registrar Entrada</button>
                        </form>
                    </div>
                </div>

                <!-- PANEL DERECHO: RETIRO DE CUENTA A EFECTIVO -->
                <div style="display:flex; flex-direction:column; gap:1.5rem;">
                    <div class="card" style="height: fit-content;">
                        <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 0.5rem;">🏦 Retiro desde Cuenta Bancaria</h3>
                        <p style="font-size:0.75rem; color:var(--text-secondary); margin-bottom:1rem;">Transfiere fondos de una cuenta bancaria a efectivo.</p>
                        <form id="form-retirar-a-efectivo" onsubmit="appUI.handleRetirarAEfectivo(event)">
                            <div class="form-group">
                                <label for="efe_ret_fec">Fecha *</label>
                                <input type="text" id="efe_ret_fec" class="form-control" value="${hoyStr}" required>
                            </div>
                            <div class="form-group">
                                <label for="efe_ret_ori">Cuenta Origen (Banco) *</label>
                                <select id="efe_ret_ori" class="form-control" required>
                                    <option value="" disabled selected>Seleccione cuenta...</option>
                                    ${cuentasAhorro.map(c => `<option value="${c.id}" data-divisa="${c.divisa}">${c.nombre} (${c.divisa}) - Bal: ${c.divisa} ${formato.importe(c.balance_actual)}</option>`).join('')}
                                </select>
                            </div>
                            <div class="form-row">
                                <div class="form-group">
                                    <label for="efe_ret_mon">Monto Retiro *</label>
                                    <input type="number" id="efe_ret_mon" step="0.01" class="form-control" placeholder="0.00" required>
                                </div>
                                <div class="form-group">
                                    <label for="efe_ret_car">Cargo/Comisión Retiro</label>
                                    <input type="number" id="efe_ret_car" step="0.01" value="0.00" class="form-control">
                                </div>
                            </div>
                            <div class="form-group">
                                <label for="efe_ret_des_txt">Descripción *</label>
                                <input type="text" id="efe_ret_des_txt" class="form-control" value="Retiro de efectivo" required>
                            </div>
                            <button type="submit" class="btn" style="width:100%; margin-top:0.5rem; background: linear-gradient(135deg, var(--accent-primary), #00cdac); color:white;">🚀 Registrar Retiro</button>
                        </form>
                    </div>
                </div>
            </div>
        `;
    }

    async handleAgregarEfectivoInformal(e: EventoDeFormulario): Promise<void> {
        e.preventDefault();
        const { api, avisos, enrutador, dom } = this.dep;
        const fec = dom.elemento<Campo>('efe_inf_fec').value;
        const mon = Number(dom.elemento<Campo>('efe_inf_mon').value);
        const div = dom.elemento<Campo>('efe_inf_div').value;
        const des = dom.elemento<Campo>('efe_inf_des').value;
        try {
            await api.crearCobroEfectivoInformal(fec, des, mon, div);
            avisos.mostrar("Entrada en efectivo registrada correctamente.");
            await enrutador.mostrar('efectivo');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    async handleRetirarAEfectivo(e: EventoDeFormulario): Promise<void> {
        e.preventDefault();
        const { api, avisos, enrutador, dom } = this.dep;
        const fec = dom.elemento<Campo>('efe_ret_fec').value;
        const oriId = Number(dom.elemento<Campo>('efe_ret_ori').value);
        const mon = Number(dom.elemento<Campo>('efe_ret_mon').value);
        const car = Number(dom.elemento<Campo>('efe_ret_car').value);
        const desTxt = dom.elemento<Campo>('efe_ret_des_txt').value;

        try {
            const cuentas = await api.obtenerCuentas();
            const ori = cuentas.find(c => c.id === oriId);
            if (!ori) throw new Error("Cuenta origen no encontrada");

            const cashName = ori.divisa === 'USD' ? 'Efectivo USD' : 'Efectivo DOP';
            const des = cuentas.find(c => c.nombre === cashName);
            if (!des) throw new Error(`Cuenta destino ${cashName} no encontrada`);

            await api.transferirEntreCuentas(fec, ori.id, des.id, mon, mon, car, desTxt);
            avisos.mostrar("Retiro de efectivo ejecutado exitosamente.");
            await enrutador.mostrar('efectivo');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }
}

/**
 * Lo que `window.appUI` expone de esta vista: un puente que delega en la
 * instancia. Los atributos `onsubmit="appUI.…"` no cambian.
 */
export function puenteEfectivo(vista: VistaEfectivo): Record<ManejadorEfectivo, (e: EventoDeFormulario) => Promise<void>> {
    return {
        handleAgregarEfectivoInformal: e => vista.handleAgregarEfectivoInformal(e),
        handleRetirarAEfectivo: e => vista.handleRetirarAEfectivo(e),
    };
}
