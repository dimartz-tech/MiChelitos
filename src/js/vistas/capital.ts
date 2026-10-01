// Pestaña «Capital» (diseño «B»: clase con dependencias inyectadas).
//
// Inventario de certificados, inversiones en bolsa y bienes. Es la primera vista
// que **guarda**: cada alta o baja lee el documento de capital entero, lo
// modifica y lo devuelve a Rust, que valida los importes con `Dinero` (positivos
// y exactos al céntimo) y los guarda. La vista **no suma ni redondea nada**:
// manda los importes como **texto**, tal cual se escribieron, y solo los formatea
// para mostrarlos. Los totales del capital los calcula Rust en centavos
// (`obtener_capital` los añade como `totales`) y los muestran el Dashboard y el
// Resumen, no esta pantalla. Ver `division_de_ui_limpia.md`, §9.
//
// No usa `this` de ningún objeto mezclado, ni `AppAPI`, `elemento()`, `confirm()`
// ni `Date` globales: todo entra por el constructor
// (`pruebas/js/vistas/capital.test.js`). Los cuerpos son los de `ui.ts`.

import type { ApiDe, Avisos, Dialogos, Dom, Enrutador, Formato, Pantalla, Reloj, Vista } from '../ui/servicios';

type ApiCapital = ApiDe<'obtenerCapital' | 'guardarCapital'>;

/**
 * Lo que la vista lee de cada elemento del capital. Rust devuelve el documento
 * como JSON libre (`any`); aquí se declara **solo lo que se usa**, no un modelo
 * del capital. `monto` y `valor_estimado` llegan como número (así los guarda el
 * archivo) y salen como texto (así los manda la vista).
 */
interface Certificado {
    banco: string;
    monto: number | string;
    tasa: number;
    vencimiento: string;
    tipo_pago: string;
    alerta_vencimiento?: boolean;
    alerta_msg?: string;
}
interface InversionDeBolsa extends Omit<Certificado, 'banco'> {
    emisor: string;
}
interface Bien {
    id: string;
    nombre: string;
    subtipo?: string;
    valor_estimado: number | string;
}

export interface DependenciasCapital {
    api: ApiCapital;
    avisos: Avisos;
    formato: Formato;
    enrutador: Enrutador;
    pantalla: Pantalla;
    dom: Dom;
    dialogos: Dialogos;
    /** Solo para el identificador de un bien nuevo (`Date.now()` antes). */
    ahora: Reloj;
}

/** Los manejadores en línea (`onsubmit`/`onclick` → `appUI.…`) que la plantilla escribe. */
export const MANEJADORES_CAPITAL = [
    'handleAgregarCertificado', 'handleEliminarCertificado',
    'handleAgregarBolsa', 'handleEliminarBolsa',
    'handleAgregarPropiedad', 'handleEliminarPropiedad',
] as const;
export type ManejadorCapital = (typeof MANEJADORES_CAPITAL)[number];

/** Un formulario envía un evento con `preventDefault`; solo eso se usa. */
interface EventoDeFormulario {
    preventDefault(): void;
}

export class VistaCapital implements Vista {
    constructor(private readonly dep: DependenciasCapital) {}

    // --- RENDER: CAPITAL ---
    async render(): Promise<void> {
        const { api, formato, pantalla } = this.dep;
        const capital = await api.obtenerCapital();
        const inmobiliario = capital?.propiedades?.inmobiliario || [];
        const vehiculos = capital?.propiedades?.vehiculos || [];
        const maquinaria = capital?.propiedades?.maquinaria || [];

        pantalla.contenido.innerHTML = `
            <div class="section-title">
                <h1>Capital y Activos</h1>
                <span class="subtitle">Inversiones financieras y propiedades físicas</span>
            </div>

            <div class="grid-3">
                <!-- Certificados -->
                <div class="card">
                    <h3 style="font-family: var(--font-heading); font-size: 1.1rem; margin-bottom: 1rem;">🏦 Certificados Financieros</h3>
                    <form id="form-add-certificado" onsubmit="appUI.handleAgregarCertificado(event)" style="border-bottom:1px dashed var(--border-color); padding-bottom:1rem; margin-bottom:1rem;">
                        <div class="form-row">
                            <div class="form-group">
                                <label>Banco *</label>
                                <input type="text" id="cer_ban" class="form-control" style="padding:0.5rem;" required>
                            </div>
                            <div class="form-group">
                                <label>Monto DOP *</label>
                                <input type="number" step="0.01" id="cer_mon" class="form-control" style="padding:0.5rem;" required>
                            </div>
                        </div>
                        <div class="form-row">
                            <div class="form-group">
                                <label>Tasa (%) *</label>
                                <input type="number" step="0.01" id="cer_tas" class="form-control" style="padding:0.5rem;" required>
                            </div>
                            <div class="form-group">
                                <label>Vence *</label>
                                <input type="text" id="cer_ven" class="form-control" placeholder="dd/mm/aaaa" style="padding:0.5rem;" required>
                            </div>
                        </div>
                        <div class="form-group">
                            <label>Tipo de Pago *</label>
                            <select id="cer_pag" class="form-control" style="padding:0.5rem;" required>
                                <option value="A cuenta" selected>A cuenta</option>
                                <option value="Reinversión compuesta">Reinversión compuesta</option>
                            </select>
                        </div>
                        <button type="submit" class="btn" style="width:100%; padding:0.5rem;">➕ Agregar Certificado</button>
                    </form>

                    <div style="display:flex; flex-direction:column; gap:0.5rem; max-height:220px; overflow-y:auto;">
                        ${(capital.certificados || []).map((c: Certificado, i: number) => `
                            <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.6rem; border-radius:4px; font-size:0.75rem;">
                                <div style="display:flex; justify-content:space-between; font-weight:bold;">
                                    <span>${c.banco}</span>
                                    <span>DOP ${formato.importe(c.monto)}</span>
                                </div>
                                <div style="display:flex; justify-content:space-between; color:var(--text-secondary); margin-top:0.2rem; align-items:center;">
                                    <span>Tasa: ${c.tasa}% | Vence: ${c.vencimiento}</span>
                                    <span style="font-size:0.7rem; color:var(--accent-primary); font-style:italic;">${c.tipo_pago || 'A cuenta'}</span>
                                </div>
                                <div style="text-align:right; margin-top:0.2rem;">
                                    <button onclick="appUI.handleEliminarCertificado(${i})" style="background:none; border:none; color:var(--color-danger); cursor:pointer; font-size:0.7rem;">Eliminar</button>
                                </div>
                                ${c.alerta_vencimiento ? `
                                    <div class="badge danger" style="width:100%; text-align:center; font-size:0.65rem; margin-top:0.3rem; padding:2px;">${c.alerta_msg}</div>
                                ` : ''}
                            </div>
                        `).join('')}
                    </div>
                </div>

                <!-- Bolsa -->
                <div class="card">
                    <h3 style="font-family: var(--font-heading); font-size: 1.1rem; margin-bottom: 1rem;">📈 Bolsa de Valores</h3>
                    <form id="form-add-bolsa" onsubmit="appUI.handleAgregarBolsa(event)" style="border-bottom:1px dashed var(--border-color); padding-bottom:1rem; margin-bottom:1rem;">
                        <div class="form-row">
                            <div class="form-group">
                                <label>Emisor *</label>
                                <input type="text" id="bol_emi" class="form-control" style="padding:0.5rem;" required>
                            </div>
                            <div class="form-group">
                                <label>Monto DOP *</label>
                                <input type="number" step="0.01" id="bol_mon" class="form-control" style="padding:0.5rem;" required>
                            </div>
                        </div>
                        <div class="form-row">
                            <div class="form-group">
                                <label>Tasa (%) *</label>
                                <input type="number" step="0.01" id="bol_tas" class="form-control" style="padding:0.5rem;" required>
                            </div>
                            <div class="form-group">
                                <label>Vence *</label>
                                <input type="text" id="bol_ven" class="form-control" placeholder="dd/mm/aaaa" style="padding:0.5rem;" required>
                            </div>
                        </div>
                        <div class="form-group">
                            <label>Tipo de Pago *</label>
                            <select id="bol_pag" class="form-control" style="padding:0.5rem;" required>
                                <option value="A cuenta" selected>A cuenta</option>
                                <option value="Reinversión">Reinversión</option>
                            </select>
                        </div>
                        <button type="submit" class="btn" style="width:100%; padding:0.5rem;">➕ Agregar Inversión</button>
                    </form>

                    <div style="display:flex; flex-direction:column; gap:0.5rem; max-height:220px; overflow-y:auto;">
                        ${(capital.bolsa || []).map((b: InversionDeBolsa, i: number) => `
                            <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.6rem; border-radius:4px; font-size:0.75rem;">
                                <div style="display:flex; justify-content:space-between; font-weight:bold;">
                                    <span>${b.emisor}</span>
                                    <span>DOP ${formato.importe(b.monto)}</span>
                                </div>
                                <div style="display:flex; justify-content:space-between; color:var(--text-secondary); margin-top:0.2rem; align-items:center;">
                                    <span>Tasa: ${b.tasa}% | Vence: ${b.vencimiento}</span>
                                    <span style="font-size:0.7rem; color:#10b981; font-style:italic;">${b.tipo_pago || 'A cuenta'}</span>
                                </div>
                                <div style="text-align:right; margin-top:0.2rem;">
                                    <button onclick="appUI.handleEliminarBolsa(${i})" style="background:none; border:none; color:var(--color-danger); cursor:pointer; font-size:0.7rem;">Eliminar</button>
                                </div>
                                ${b.alerta_vencimiento ? `
                                    <div class="badge danger" style="width:100%; text-align:center; font-size:0.65rem; margin-top:0.3rem; padding:2px;">${b.alerta_msg}</div>
                                ` : ''}
                            </div>
                        `).join('')}
                    </div>
                </div>

                <!-- Bienes Físicos -->
                <div class="card">
                    <h3 style="font-family: var(--font-heading); font-size: 1.1rem; margin-bottom: 1rem;">🏢 Propiedades</h3>
                    <form id="form-add-propiedad" onsubmit="appUI.handleAgregarPropiedad(event)" style="border-bottom:1px dashed var(--border-color); padding-bottom:1rem; margin-bottom:1rem;">
                        <div class="form-row">
                            <div class="form-group">
                                <label>Tipo *</label>
                                <select id="pro_tip" class="form-control" style="padding:0.5rem;" required>
                                    <option value="inmobiliario" selected>Bienes Raíces</option>
                                    <option value="vehiculos">Vehículos</option>
                                    <option value="maquinaria">Maquinaria/Equipos</option>
                                </select>
                            </div>
                            <div class="form-group">
                                <label>Subtipo *</label>
                                <input type="text" id="pro_sub" class="form-control" placeholder="Apto, SUV..." style="padding:0.5rem;" required>
                            </div>
                        </div>
                        <div class="form-row">
                            <div class="form-group">
                                <label>Nombre/ID *</label>
                                <input type="text" id="pro_nom" class="form-control" style="padding:0.5rem;" required>
                            </div>
                            <div class="form-group">
                                <label>Valor Est. *</label>
                                <input type="number" step="0.01" id="pro_val" class="form-control" style="padding:0.5rem;" required>
                            </div>
                        </div>
                        <button type="submit" class="btn" style="width:100%; padding:0.5rem;">➕ Agregar Propiedad</button>
                    </form>

                    <div style="display:flex; flex-direction:column; gap:0.5rem; max-height:220px; overflow-y:auto; font-size:0.75rem;">
                        ${inmobiliario.length > 0 ? `
                            <div style="font-weight:bold; color:var(--accent-primary);">Inmuebles:</div>
                            ${inmobiliario.map((p: Bien) => `
                                <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.4rem; border-radius:4px; margin-bottom:0.2rem;">
                                    <span>${p.nombre}</span>
                                    <span>DOP ${formato.importe(p.valor_estimado)} <button onclick="appUI.handleEliminarPropiedad('inmobiliario', '${p.id}')" style="background:none; border:none; color:var(--color-danger); cursor:pointer; margin-left:0.4rem;">🗑️</button></span>
                                </div>
                            `).join('')}
                        ` : ''}

                        ${vehiculos.length > 0 ? `
                            <div style="font-weight:bold; color:var(--accent-primary); margin-top:0.4rem;">Vehículos:</div>
                            ${vehiculos.map((p: Bien) => `
                                <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.4rem; border-radius:4px; margin-bottom:0.2rem;">
                                    <span>${p.nombre}</span>
                                    <span>DOP ${formato.importe(p.valor_estimado)} <button onclick="appUI.handleEliminarPropiedad('vehiculos', '${p.id}')" style="background:none; border:none; color:var(--color-danger); cursor:pointer; margin-left:0.4rem;">🗑️</button></span>
                                </div>
                            `).join('')}
                        ` : ''}

                        ${maquinaria.length > 0 ? `
                            <div style="font-weight:bold; color:var(--accent-primary); margin-top:0.4rem;">Equipos:</div>
                            ${maquinaria.map((p: Bien) => `
                                <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.4rem; border-radius:4px; margin-bottom:0.2rem;">
                                    <span>${p.nombre}</span>
                                    <span>DOP ${formato.importe(p.valor_estimado)} <button onclick="appUI.handleEliminarPropiedad('maquinaria', '${p.id}')" style="background:none; border:none; color:var(--color-danger); cursor:pointer; margin-left:0.4rem;">🗑️</button></span>
                                </div>
                            `).join('')}
                        ` : ''}
                    </div>
                </div>
            </div>
        `;
    }

    async handleAgregarCertificado(e: EventoDeFormulario): Promise<void> {
        e.preventDefault();
        const { api, avisos, enrutador, dom } = this.dep;
        const ban = dom.elemento<Campo>('cer_ban').value;
        // Texto, tal cual se escribió: el céntimo lo deciden los dígitos.
        const mon = dom.elemento<Campo>('cer_mon').value.trim();
        const tas = Number(dom.elemento<Campo>('cer_tas').value);
        const ven = dom.elemento<Campo>('cer_ven').value;
        const pag = dom.elemento<Campo>('cer_pag').value;

        try {
            const capital = await api.obtenerCapital();
            if (!capital.certificados) capital.certificados = [];
            capital.certificados.push({ banco: ban, monto: mon, tasa: tas, vencimiento: ven, tipo_pago: pag });
            await api.guardarCapital(capital);
            avisos.mostrar("Certificado guardado.");
            await enrutador.mostrar('capital');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    async handleEliminarCertificado(idx: number): Promise<void> {
        const { api, avisos, enrutador, dialogos } = this.dep;
        if (dialogos.confirmar("¿Retirar este certificado financiero?")) {
            const capital = await api.obtenerCapital();
            capital.certificados.splice(idx, 1);
            await api.guardarCapital(capital);
            avisos.mostrar("Certificado retirado.");
            await enrutador.mostrar('capital');
        }
    }

    async handleAgregarBolsa(e: EventoDeFormulario): Promise<void> {
        e.preventDefault();
        const { api, avisos, enrutador, dom } = this.dep;
        const emi = dom.elemento<Campo>('bol_emi').value;
        // Texto, tal cual se escribió: el céntimo lo deciden los dígitos.
        const mon = dom.elemento<Campo>('bol_mon').value.trim();
        const tas = Number(dom.elemento<Campo>('bol_tas').value);
        const ven = dom.elemento<Campo>('bol_ven').value;
        const pag = dom.elemento<Campo>('bol_pag').value;

        try {
            const capital = await api.obtenerCapital();
            if (!capital.bolsa) capital.bolsa = [];
            capital.bolsa.push({ emisor: emi, monto: mon, tasa: tas, vencimiento: ven, tipo_pago: pag });
            await api.guardarCapital(capital);
            avisos.mostrar("Inversión de bolsa guardada.");
            await enrutador.mostrar('capital');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    async handleEliminarBolsa(idx: number): Promise<void> {
        const { api, avisos, enrutador, dialogos } = this.dep;
        if (dialogos.confirmar("¿Liquidar esta inversión de bolsa?")) {
            const capital = await api.obtenerCapital();
            capital.bolsa.splice(idx, 1);
            await api.guardarCapital(capital);
            avisos.mostrar("Inversión liquidada.");
            await enrutador.mostrar('capital');
        }
    }

    async handleAgregarPropiedad(e: EventoDeFormulario): Promise<void> {
        e.preventDefault();
        const { api, avisos, enrutador, dom, ahora } = this.dep;
        const tip = dom.elemento<Campo>('pro_tip').value;
        const sub = dom.elemento<Campo>('pro_sub').value;
        const nom = dom.elemento<Campo>('pro_nom').value;
        // Texto, tal cual se escribió: el céntimo lo deciden los dígitos.
        const val = dom.elemento<Campo>('pro_val').value.trim();

        try {
            const capital = await api.obtenerCapital();
            if (!capital.propiedades) capital.propiedades = { inmobiliario: [], vehiculos: [], maquinaria: [] };
            if (!capital.propiedades[tip]) capital.propiedades[tip] = [];
            
            capital.propiedades[tip].push({
                id: ahora().getTime().toString(),
                subtipo: sub,
                nombre: nom,
                valor_estimado: val
            });
            await api.guardarCapital(capital);
            avisos.mostrar("Propiedad registrada.");
            await enrutador.mostrar('capital');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    async handleEliminarPropiedad(tipo: string, id: string): Promise<void> {
        const { api, avisos, enrutador, dialogos } = this.dep;
        if (dialogos.confirmar("¿Eliminar este bien del capital?")) {
            const capital = await api.obtenerCapital();
            if (capital.propiedades && capital.propiedades[tipo]) {
                capital.propiedades[tipo] = capital.propiedades[tipo].filter((p: Bien) => p.id !== id);
                await api.guardarCapital(capital);
                avisos.mostrar("Bien eliminado.");
                await enrutador.mostrar('capital');
            }
        }
    }
}

/**
 * Lo que `window.appUI` expone de esta vista: un puente que delega en la
 * instancia. Los atributos `onsubmit`/`onclick` no cambian.
 */
export function puenteCapital(vista: VistaCapital): {
    handleAgregarCertificado: (e: EventoDeFormulario) => Promise<void>;
    handleEliminarCertificado: (idx: number) => Promise<void>;
    handleAgregarBolsa: (e: EventoDeFormulario) => Promise<void>;
    handleEliminarBolsa: (idx: number) => Promise<void>;
    handleAgregarPropiedad: (e: EventoDeFormulario) => Promise<void>;
    handleEliminarPropiedad: (tipo: string, id: string) => Promise<void>;
} {
    return {
        handleAgregarCertificado: e => vista.handleAgregarCertificado(e),
        handleEliminarCertificado: idx => vista.handleEliminarCertificado(idx),
        handleAgregarBolsa: e => vista.handleAgregarBolsa(e),
        handleEliminarBolsa: idx => vista.handleEliminarBolsa(idx),
        handleAgregarPropiedad: e => vista.handleAgregarPropiedad(e),
        handleEliminarPropiedad: (tipo, id) => vista.handleEliminarPropiedad(tipo, id),
    };
}
