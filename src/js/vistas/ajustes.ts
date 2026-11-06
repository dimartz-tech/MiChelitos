// Pestaña «Ajustes» (diseño «B»: clase con dependencias inyectadas).
//
// Catálogos (categorías, clientes, cuentas de ahorro, el alta de tarjetas),
// respaldos de la base (crear y restaurar) y el panel de **correcciones**: los
// borrados que revierten un movimiento y piden un motivo para dejar un caso de
// auditoría. Es la vista con más diálogos de la interfaz, y por eso la que más
// ganó con el servicio `Dialogos` asíncrono: todo lo que pregunta lo **espera**
// (`pruebas/js/contrato/dialogos.test.js`). No usa `AppAPI`, `elemento()`,
// `confirm` ni `prompt` globales: todo entra por el constructor
// (`pruebas/js/vistas/ajustes.test.js`). Los cuerpos son los de `ui.ts`.

import type { CasoCorreccion, CuentaAhorro } from '../tipos-ipc';
import { accion, alEnviar, escaparHtml, manejador } from '../nucleo/html.js';
import { describirRespaldo } from '../nucleo/respaldos.js';
import { AYUDA_DE_FIJAS, esDeInversionYObligaciones } from './resumen_gastos.js';

/** Las pestañas del panel de correcciones: sus paneles y los botones que los alternan (acción `activarPestana`). */
const PANELES_CORRECCION = ['corr-gastos', 'corr-ingresos', 'corr-trans'];
const BOTONES_CORRECCION = ['btn-corr-gas', 'btn-corr-ing', 'btn-corr-tra'];
import type { ApiDe, Avisos, Dialogos, Dom, Enrutador, Formato, Modales, Motivo, Pantalla, Vista } from '../ui/servicios';

type ApiAjustes = ApiDe<
    | 'obtenerCategorias'
    | 'obtenerClientes'
    | 'obtenerCuentas'
    | 'obtenerGastos'
    | 'obtenerIngresosInformales'
    | 'obtenerTransaccionesCuentas'
    | 'obtenerIngresos'
    | 'listarRespaldos'
    | 'obtenerCorrecciones'
    | 'crearCategoria'
    | 'eliminarCategoria'
    | 'marcarCategoriaFija'
    | 'renombrarCategoria'
    | 'previsualizarFusionCategorias'
    | 'fusionarCategorias'
    | 'crearCliente'
    | 'eliminarCliente'
    | 'crearCuenta'
    | 'actualizarCuenta'
    | 'archivarCuenta'
    | 'reactivarCuenta'
    | 'obtenerCuentasArchivadas'
    | 'crearTarjeta'
    | 'eliminarGasto'
    | 'eliminarIngresoInformal'
    | 'eliminarIngreso'
    | 'eliminarTransaccionCuenta'
    | 'crearRespaldo'
    | 'restaurarRespaldo'
>;

export interface DependenciasAjustes {
    api: ApiAjustes;
    avisos: Avisos;
    formato: Formato;
    enrutador: Enrutador;
    pantalla: Pantalla;
    dom: Dom;
    dialogos: Dialogos;
    motivo: Motivo;
    modales: Modales;
}

/** Las acciones delegadas (`data-accion*` → `appUI.…`) que la plantilla escribe. */
export const MANEJADORES_AJUSTES = [
    'handleAgregarCategoria', 'handleEliminarCategoria', 'handleMarcarCategoriaFija',
    'handleRenombrarCategoria', 'handleFusionarCategoria', 'cambiarDestinoFusion', 'handleConfirmarFusion',
    'handleAgregarCliente', 'handleEliminarCliente',
    'handleAgregarCuenta', 'abrirEdicionCuenta', 'handleArchivarCuenta', 'handleReactivarCuenta',
    'handleAgregarTarjeta',
    'handleCrearRespaldo', 'handleRestaurarRespaldo',
    'alternarCasosDeCorreccion',
    'handleEliminarGastoCorr', 'handleEliminarIngresoInformalCorr',
    'handleEliminarIngresoCorr', 'handleEliminarTransaccionCuentaCorr',
] as const;
export type ManejadorAjustes = (typeof MANEJADORES_AJUSTES)[number];

/** Un formulario envía un evento con `preventDefault`; solo eso se usa. */
interface EventoDeFormulario {
    preventDefault(): void;
}

/** El botón que lanza una operación larga: se deshabilita y cambia de texto mientras corre. */
interface BotonDeAccion {
    disabled: boolean;
    textContent: string | null;
}

/** La forma del JSON que guarda `dominio::detalle_traspaso::DetalleDeTraspaso` (versión 1). Importes y tasa, en texto exacto. */
interface DetalleDeTraspaso {
    version: number;
    traspaso_id: number;
    fecha: string;
    descripcion: string | null;
    origen: { cuenta_id: number; cuenta_nombre: string; importe: { monto: string; divisa: string } };
    destino: { cuenta_id: number; cuenta_nombre: string; importe: { monto: string; divisa: string } };
    tasa: string | null;
    cargo: { monto: string; divisa: string };
}

/**
 * El detalle de un caso de traspaso (la foto completa que se guardó al borrarlo), o «sin detalle» si el caso es
 * anterior a que se guardara. Los demás tipos de caso no llevan detalle y no muestran nada. Todo texto va por
 * `escaparHtml`: los nombres de cuenta y la descripción los escribió el titular. Los importes ya llegan como texto
 * exacto en el JSON, así que se muestran tal cual, sin pasar por coma flotante.
 */
export function detalleDeTraspasoHtml(c: CasoCorreccion): string {
    if (c.tipo !== 'traspaso') return '';
    const estilo = 'margin-top:0.2rem; color:var(--text-secondary);';
    if (c.detalle == null) {
        return `<div class="caso-detalle caso-sin-detalle" style="${estilo}">Sin detalle (caso anterior)</div>`;
    }
    let d: DetalleDeTraspaso;
    try {
        d = JSON.parse(c.detalle) as DetalleDeTraspaso;
        if (!d || !d.origen || !d.destino || !d.cargo) throw new Error('forma inesperada');
    } catch {
        return `<div class="caso-detalle caso-sin-detalle" style="${estilo}">Sin detalle (ilegible)</div>`;
    }
    const lado = (e: DetalleDeTraspaso['origen']) =>
        `${escaparHtml(e.cuenta_nombre)} (${escaparHtml(e.importe.monto)} ${escaparHtml(e.importe.divisa)})`;
    const partes = [
        `${escaparHtml(d.fecha)}: ${lado(d.origen)} → ${lado(d.destino)}`,
        d.tasa != null ? `tasa ${escaparHtml(d.tasa)}` : '',
        `cargo ${escaparHtml(d.cargo.monto)} ${escaparHtml(d.cargo.divisa)}`,
        d.descripcion != null ? `«${escaparHtml(d.descripcion)}»` : '',
    ].filter(x => x !== '');
    return `<div class="caso-detalle" style="${estilo}">${partes.join(' · ')}</div>`;
}

/** Una categoría tal como la lista `obtenerCategorias`. */
type CategoriaDeLista = Awaited<ReturnType<ApiAjustes['obtenerCategorias']>>[number];
type VistaPreviaDeFusion = Awaited<ReturnType<ApiAjustes['previsualizarFusionCategorias']>>;

/** El identificador de la ventana de fusión (la acción `quitar` la cierra por él). */
const ID_MODAL_FUSION = 'modal-fusion-categoria';

export class VistaAjustes implements Vista {
    /** La vista previa que el titular tiene delante: «Confirmar» solo actúa sobre esa. */
    private fusionVigente: VistaPreviaDeFusion | null = null;

    constructor(private readonly dep: DependenciasAjustes) {}

    // --- RENDER: AJUSTES ---
    async render(): Promise<void> {
        const { api, formato, pantalla } = this.dep;
        const categorias = await api.obtenerCategorias();
        const clientes = await api.obtenerClientes();
        const cuentas = await api.obtenerCuentas();
        const cuentasArchivadas = await api.obtenerCuentasArchivadas();
        const gastos = await api.obtenerGastos();
        const informales = await api.obtenerIngresosInformales();
        const transacciones = await api.obtenerTransaccionesCuentas();
        const ingresos = await api.obtenerIngresos();
        // Sin respaldos legibles el resto de Ajustes debe seguir funcionando.
        const respaldos = await api.listarRespaldos().catch(() => []);

        pantalla.contenido.innerHTML = `
            <div class="section-title">
                <h1>Ajustes</h1>
                <span class="subtitle">Configuraciones de catálogos y entorno nativo</span>
            </div>

            <div style="display:grid; grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); gap:1.5rem; margin-bottom: 2rem;">
                <!-- Respaldo -->
                <div class="card" style="height:fit-content;">
                    <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 0.5rem;">🛟 Respaldo de la base</h3>
                    <p style="font-size:0.75rem; color:var(--text-secondary); margin-bottom:1rem;">
                        La aplicación respalda sola antes de modificar el esquema. Usa esto cuando vayas a hacer algo arriesgado a mano — conciliar varios saldos o cargar un estado — y quieras una red antes.
                    </p>
                    <button ${manejador('click', 'handleCrearRespaldo', [], 'elemento')} class="btn" style="width:100%;">Respaldar ahora</button>
                    <div id="respaldo_resultado" style="font-size:0.72rem; color:var(--text-muted); margin-top:0.75rem; word-break:break-all;"></div>

                    <hr style="border:none; border-top:1px solid var(--border-color, rgba(128,128,128,0.25)); margin:1.25rem 0;">
                    <h4 style="font-size:0.95rem; margin-bottom:0.4rem;">Restaurar un respaldo</h4>
                    <p style="font-size:0.75rem; color:var(--text-secondary); margin-bottom:0.75rem;">
                        Devuelve la base y el capital al estado del respaldo elegido. Antes se guarda una copia de lo que hay ahora, así que se puede deshacer.
                    </p>
                    ${respaldos.length === 0
                        ? `<p style="font-size:0.75rem; color:var(--text-muted);">Todavía no hay respaldos.</p>`
                        : `<select id="respaldo_elegido" class="form-control" style="margin-bottom:0.6rem;">
                               ${respaldos.map(n => `<option value="${escaparHtml(n)}">${escaparHtml(describirRespaldo(n))}</option>`).join('')}
                           </select>
                           <button ${manejador('click', 'handleRestaurarRespaldo', [], 'elemento')} class="btn" style="width:100%;">Restaurar este respaldo</button>`}
                    <div id="restauracion_resultado" style="font-size:0.72rem; color:var(--text-muted); margin-top:0.75rem; word-break:break-all;"></div>
                </div>

                <!-- Categorías -->
                <div class="card" style="height:fit-content;">
                    <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 0.5rem;">🏷️ Categorías de Gastos</h3>
                    <p style="font-size:0.75rem; color:var(--text-secondary); margin-bottom:1rem;">Añade nuevas etiquetas o remueve las que no utilices.</p>
                    <p style="font-size:0.75rem; color:var(--text-secondary); margin-bottom:1rem;">${escaparHtml(AYUDA_DE_FIJAS)}. Solo cambia cómo se agrupa el Resumen; no toca ningún gasto ni saldo.</p>
                    
                    <form id="form-add-categoria" ${alEnviar('handleAgregarCategoria')} style="display:flex; gap:0.4rem; margin-bottom: 1rem;">
                        <input type="text" id="cat_nom" class="form-control" placeholder="Nueva categoría..." style="padding:0.5rem 0.8rem;" required>
                        <button type="submit" class="btn" style="padding:0.5rem 0.8rem;">➕</button>
                    </form>

                    <div style="display:flex; flex-direction:column; gap:0.4rem; max-height:320px; overflow-y:auto; font-size:0.85rem;">
                        ${categorias.map(c => this.filaDeCategoria(c)).join('')}
                    </div>
                </div>

                <!-- Clientes -->
                <div class="card" style="height:fit-content;">
                    <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 0.5rem;">👥 Catálogo de Clientes</h3>
                    <p style="font-size:0.75rem; color:var(--text-secondary); margin-bottom:1rem;">Registra clientes para utilizarlos en facturación formal.</p>
                    
                    <form id="form-add-cliente" ${alEnviar('handleAgregarCliente')} style="display:flex; flex-direction:column; gap:0.4rem; margin-bottom: 1rem;">
                        <input type="text" id="cli_aj_nom" class="form-control" placeholder="Nombre (Ej. Empresa S.A.)..." style="padding:0.4rem 0.6rem;" required>
                        <input type="text" id="cli_aj_rnc" class="form-control" placeholder="RNC (Ej. 131XXXXXX)..." style="padding:0.4rem 0.6rem;" required>
                        <button type="submit" class="btn" style="padding:0.4rem; font-size:0.85rem;">🚀 Registrar Cliente</button>
                    </form>

                    <div style="display:flex; flex-direction:column; gap:0.4rem; max-height:220px; overflow-y:auto; font-size:0.85rem;">
                        ${clientes.length > 0 ? clientes.map(cl => `
                            <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.5rem 0.8rem; border-radius:var(--radius-sm); display:flex; justify-content:space-between; align-items:center;">
                                <div>
                                    <strong>${escaparHtml(cl.nombre)}</strong><br>
                                    <span style="font-size:0.7rem; color:var(--text-muted);">RNC: ${escaparHtml(cl.rnc)}</span>
                                </div>
                                <button ${accion('handleEliminarCliente', cl.id)} class="btn btn-danger" style="padding:0.2rem 0.4rem; font-size:0.75rem;">🗑️</button>
                            </div>
                        `).join('') : `
                            <p style="color:var(--text-muted); text-align:center; padding:1rem; font-size:0.75rem;">No hay clientes registrados.</p>
                        `}
                    </div>
                </div>

                <!-- Cuentas de Ahorro -->
                <div class="card" style="height:fit-content;">
                    <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 0.5rem;">🏦 Cuentas de Ahorro</h3>
                    <p style="font-size:0.75rem; color:var(--text-secondary); margin-bottom:1rem;">Registra tus cuentas bancarias en Pesos o Dólares.</p>
                    
                    <form id="form-add-cuenta" ${alEnviar('handleAgregarCuenta')} style="display:flex; flex-direction:column; gap:0.4rem; margin-bottom: 1rem;">
                        <input type="text" id="cue_aj_nom" class="form-control" placeholder="Nombre de la Cuenta..." required style="padding:0.4rem 0.6rem;">
                        <div style="display:flex; gap:0.4rem;">
                            <input type="text" id="cue_aj_ent" class="form-control" placeholder="Entidad..." style="flex:1.5; padding:0.4rem 0.6rem;">
                            <select id="cue_aj_div" class="form-control" style="flex:1; padding:0.4rem 0.6rem;">
                                <option value="DOP" selected>DOP</option>
                                <option value="USD">USD</option>
                            </select>
                        </div>
                        <div style="display:flex; gap:0.4rem;">
                            <input type="number" id="cue_aj_bal" class="form-control" placeholder="Balance Inicial..." step="0.01" value="0.00" required style="flex:1; padding:0.4rem 0.6rem;">
                            <input type="number" id="cue_aj_com" class="form-control" placeholder="Comisión impuestos..." step="0.01" min="0" title="Tarifa fija que cobra la entidad por pagar impuestos desde esta cuenta. En blanco si no hay ninguna pactada: no es lo mismo que cero." style="flex:1; padding:0.4rem 0.6rem;">
                        </div>
                        <button type="submit" class="btn" style="padding:0.4rem; font-size:0.85rem; background: linear-gradient(135deg, var(--accent-primary), #00cdac); color:white;">🚀 Registrar Cuenta</button>
                    </form>

                    <div style="display:flex; flex-direction:column; gap:0.4rem; max-height:220px; overflow-y:auto; font-size:0.85rem;">
                        ${cuentas.length > 0 ? cuentas.map(c => `
                            <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.5rem 0.8rem; border-radius:var(--radius-sm); display:flex; justify-content:space-between; align-items:center;">
                                <div>
                                    <strong>${escaparHtml(c.nombre)}</strong>
                                    ${c.entidad ? `<span style="font-size:0.7rem; color:var(--text-muted);"> · ${escaparHtml(c.entidad)}</span>` : ''}
                                    <br>
                                    <span style="font-size:0.75rem; color:var(--accent-primary); font-weight:bold;">${escaparHtml(c.divisa)} ${formato.importe(c.balance_actual)}</span>
                                    ${c.comision_pago_impuestos != null ? `<span style="font-size:0.7rem; color:var(--color-warning);" title="Tarifa fija por pagar impuestos desde esta cuenta"> · 🧾 ${formato.importe(c.comision_pago_impuestos)}</span>` : ''}
                                </div>
                                <div style="display:flex; gap:0.3rem;">
                                    <button ${accion('abrirEdicionCuenta', c)} class="btn" style="padding:0.2rem 0.4rem; font-size:0.75rem;">✏️</button>
                                    <button ${accion('handleArchivarCuenta', c.id)} class="btn btn-secondary" style="padding:0.2rem 0.4rem; font-size:0.75rem;" title="Archivar: conserva el historial y solo se permite con saldo cero" aria-label="Archivar la cuenta ${escaparHtml(c.nombre)}">📦 Archivar</button>
                                </div>
                            </div>
                        `).join('') : `
                            <p style="color:var(--text-muted); text-align:center; padding:1rem; font-size:0.75rem;">No hay cuentas registradas.</p>
                        `}
                    </div>

                    ${cuentasArchivadas.length > 0 ? `
                        <details style="margin-top:0.8rem;">
                            <summary style="cursor:pointer; font-size:0.8rem; color:var(--text-secondary);">Cuentas archivadas (${cuentasArchivadas.length})</summary>
                            <p style="font-size:0.7rem; color:var(--text-muted); margin:0.4rem 0;">No aparecen en selectores ni en saldos, y no reciben operaciones. Su historial se conserva.</p>
                            <div style="display:flex; flex-direction:column; gap:0.4rem; font-size:0.85rem;">
                                ${cuentasArchivadas.map(c => `
                                    <div style="border:1px dashed var(--border-color); padding:0.5rem 0.8rem; border-radius:var(--radius-sm); display:flex; justify-content:space-between; align-items:center;">
                                        <div>
                                            <strong>${escaparHtml(c.nombre)}</strong>
                                            <span style="font-size:0.7rem; color:var(--text-muted);"> · Archivada · ${escaparHtml(c.divisa)}${c.entidad ? ` · ${escaparHtml(c.entidad)}` : ''}</span>
                                        </div>
                                        <button ${accion('handleReactivarCuenta', c.id)} class="btn btn-secondary" style="padding:0.2rem 0.4rem; font-size:0.75rem;" aria-label="Reactivar la cuenta ${escaparHtml(c.nombre)}">♻️ Reactivar</button>
                                    </div>
                                `).join('')}
                            </div>
                        </details>
                    ` : ''}
                </div>

                <!-- Tarjetas de Crédito -->
                <div class="card" style="height:fit-content;">
                    <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 0.5rem;">💳 Tarjetas de Crédito</h3>
                    <p style="font-size:0.75rem; color:var(--text-secondary); margin-bottom:1rem;">Registra tarjetas de crédito para consumos y abonos.</p>
                    
                    <form id="form-add-tarjeta" ${alEnviar('handleAgregarTarjeta')} style="display:flex; flex-direction:column; gap:0.4rem;">
                        <div class="form-group" style="margin-bottom:0.6rem;">
                            <label for="tar_ent">Banco Emisor *</label>
                            <input type="text" id="tar_ent" class="form-control" placeholder="Banco Popular..." style="padding:0.4rem 0.6rem;" required>
                        </div>
                        <div class="form-group" style="margin-bottom:0.6rem;">
                            <label for="tar_nom">Nombre Tarjeta *</label>
                            <input type="text" id="tar_nom" class="form-control" placeholder="Visa Infinite..." style="padding:0.4rem 0.6rem;" required>
                        </div>
                        
                        <div style="border-top: 1px dashed var(--border-color); margin: 0.5rem 0; padding-top: 0.5rem;">
                            <h4 style="font-size:0.8rem; color:var(--accent-primary); margin-bottom:0.4rem;">🇩🇴 Saldo en Pesos (DOP)</h4>
                            <div class="form-row" style="gap:0.4rem; margin-bottom:0.4rem;">
                                <div class="form-group" style="margin-bottom:0;">
                                    <label for="tar_lim_dop" style="font-size:0.7rem; margin-bottom:0.2rem;">Límite DOP</label>
                                    <input type="number" id="tar_lim_dop" step="0.01" value="0.00" class="form-control" style="padding:0.4rem 0.6rem;">
                                </div>
                                <div class="form-group" style="margin-bottom:0;">
                                    <label for="tar_sob_dop" style="font-size:0.7rem; margin-bottom:0.2rem;">Sobregiro DOP</label>
                                    <input type="number" id="tar_sob_dop" step="0.01" value="0.00" class="form-control" style="padding:0.4rem 0.6rem;">
                                </div>
                            </div>
                            <div class="form-row" style="gap:0.4rem;">
                                <div class="form-group" style="margin-bottom:0;">
                                    <label for="tar_bal_dop" style="font-size:0.7rem; margin-bottom:0.2rem;">Bal. Actual DOP</label>
                                    <input type="number" id="tar_bal_dop" step="0.01" value="0.00" class="form-control" style="padding:0.4rem 0.6rem;">
                                </div>
                                <div class="form-group" style="margin-bottom:0;">
                                    <label for="tar_cor_dop" style="font-size:0.7rem; margin-bottom:0.2rem;">Bal. Corte DOP</label>
                                    <input type="number" id="tar_cor_dop" step="0.01" value="0.00" class="form-control" style="padding:0.4rem 0.6rem;">
                                </div>
                            </div>
                        </div>

                        <div style="border-top: 1px dashed var(--border-color); margin: 0.5rem 0; padding-top: 0.5rem;">
                            <h4 style="font-size:0.8rem; color:#10b981; margin-bottom:0.4rem;">🇺🇸 Saldo en Dólares (USD)</h4>
                            <div class="form-row" style="gap:0.4rem; margin-bottom:0.4rem;">
                                <div class="form-group" style="margin-bottom:0;">
                                    <label for="tar_lim_usd" style="font-size:0.7rem; margin-bottom:0.2rem;">Límite USD</label>
                                    <input type="number" id="tar_lim_usd" step="0.01" value="0.00" class="form-control" style="padding:0.4rem 0.6rem;">
                                </div>
                                <div class="form-group" style="margin-bottom:0;">
                                    <label for="tar_sob_usd" style="font-size:0.7rem; margin-bottom:0.2rem;">Sobregiro USD</label>
                                    <input type="number" id="tar_sob_usd" step="0.01" value="0.00" class="form-control" style="padding:0.4rem 0.6rem;">
                                </div>
                            </div>
                            <div class="form-row" style="gap:0.4rem;">
                                <div class="form-group" style="margin-bottom:0;">
                                    <label for="tar_bal_usd" style="font-size:0.7rem; margin-bottom:0.2rem;">Bal. Actual USD</label>
                                    <input type="number" id="tar_bal_usd" step="0.01" value="0.00" class="form-control" style="padding:0.4rem 0.6rem;">
                                </div>
                                <div class="form-group" style="margin-bottom:0;">
                                    <label for="tar_cor_usd" style="font-size:0.7rem; margin-bottom:0.2rem;">Bal. Corte USD</label>
                                    <input type="number" id="tar_cor_usd" step="0.01" value="0.00" class="form-control" style="padding:0.4rem 0.6rem;">
                                </div>
                            </div>
                        </div>

                        <div class="form-row" style="border-top: 1px dashed var(--border-color); margin-top: 0.5rem; padding-top: 0.5rem; gap:0.4rem; margin-bottom:0.6rem;">
                            <div class="form-group" style="margin-bottom:0;">
                                <label for="tar_cor" style="font-size:0.7rem; margin-bottom:0.2rem;">Día Corte (1-31) *</label>
                                <input type="number" id="tar_cor" min="1" max="31" class="form-control" placeholder="15" style="padding:0.4rem 0.6rem;" required>
                            </div>
                            <div class="form-group" style="margin-bottom:0;">
                                <label for="tar_pag" style="font-size:0.7rem; margin-bottom:0.2rem;">Día Vence (1-31) *</label>
                                <input type="number" id="tar_pag" min="1" max="31" class="form-control" placeholder="5" style="padding:0.4rem 0.6rem;" required>
                            </div>
                        </div>
                        <button type="submit" class="btn" style="width:100%; padding:0.5rem; font-size:0.85rem; background: linear-gradient(135deg, var(--accent-primary), var(--accent-primary-hover)); color:#0b0f19;">🚀 Registrar Tarjeta</button>
                    </form>
                </div>
            </div>

            <!-- Corrección de Transacciones -->
            <div class="card" style="width: 100%; margin-bottom: 1.5rem;">
                <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 0.5rem;">🛠️ Corrección de Transacciones (Ajustes de Balance)</h3>
                <p style="font-size:0.75rem; color:var(--text-secondary); margin-bottom:0.6rem;">
                    Revertir y eliminar transacciones mal registradas. El sistema ajustará automáticamente los saldos de cuentas, tarjetas y efectivo asociados.
                </p>
                <!-- La línea que acompaña al botón: dice qué se pierde y qué queda. -->
                <p style="font-size:0.75rem; color:var(--color-warning); margin-bottom:1.2rem; line-height:1.5; border-left:2px solid var(--color-warning); padding-left:0.6rem;">
                    <strong>Borrar destruye el movimiento.</strong> No queda un asiento que lo anule: solo el
                    <strong>caso de auditoría</strong> que se abre al hacerlo, con el motivo que escribas y un número
                    de referencia. Por eso se pide una explicación — y por eso conviene corregir antes que borrar
                    cuando la operación lo permita.
                </p>
                <div style="display:flex; justify-content:flex-end; margin-bottom:0.8rem;">
                    <button type="button" ${accion('alternarCasosDeCorreccion')} class="btn btn-secondary" style="padding:0.25rem 0.6rem; font-size:0.75rem;">📋 Ver casos abiertos</button>
                </div>
                <div id="casos-correccion" hidden style="margin-bottom:1.2rem;"></div>
                
                <div style="display:flex; gap:0.5rem; margin-bottom: 1rem; border-bottom: 1px solid var(--border-color); padding-bottom: 0.5rem;">
                    <button type="button" ${manejador('click', 'activarPestana', ['corr-gastos', PANELES_CORRECCION, BOTONES_CORRECCION], 'elemento')} id="btn-corr-gas" class="btn" style="padding: 0.35rem 0.75rem; font-size:0.8rem;">💸 Gastos</button>
                    <button type="button" ${manejador('click', 'activarPestana', ['corr-ingresos', PANELES_CORRECCION, BOTONES_CORRECCION], 'elemento')} id="btn-corr-ing" class="btn btn-secondary" style="padding: 0.35rem 0.75rem; font-size:0.8rem; border:none;">📈 Ingresos</button>
                    <button type="button" ${manejador('click', 'activarPestana', ['corr-trans', PANELES_CORRECCION, BOTONES_CORRECCION], 'elemento')} id="btn-corr-tra" class="btn btn-secondary" style="padding: 0.35rem 0.75rem; font-size:0.8rem; border:none;">🔄 Transferencias</button>
                </div>

                <!-- SECCIÓN GASTOS -->
                <div id="corr-gastos">
                    ${gastos.length > 0 ? `
                        <div class="table-responsive" style="max-height: 250px; overflow-y: auto;">
                            <table class="table-modern" style="font-size:0.8rem;">
                                <thead>
                                    <tr>
                                        <th>Fecha</th>
                                        <th>Descripción</th>
                                        <th>Categoría</th>
                                        <th>Método</th>
                                        <th>Total</th>
                                        <th>Acción</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    ${gastos.slice(0, 15).map(g => `
                                        <tr>
                                            <td>${escaparHtml(g.fecha)}</td>
                                            <td><strong>${escaparHtml(g.descripcion)}</strong></td>
                                            <td>${escaparHtml(g.categoria_nombre)}</td>
                                            <td style="text-transform:capitalize;">${escaparHtml(g.metodo_pago)}</td>
                                            <td class="amount expense">${escaparHtml(g.divisa)} ${formato.importe(g.monto + g.costo_adicional)}</td>
                                            <td>
                                                <button type="button" ${accion('handleEliminarGastoCorr', g.id)} class="btn btn-danger" style="padding: 0.2rem 0.4rem; font-size: 0.7rem;">🗑️ Revertir</button>
                                            </td>
                                        </tr>
                                    `).join('')}
                                </tbody>
                            </table>
                        </div>
                    ` : `<p style="color:var(--text-muted); font-size:0.8rem; text-align:center; padding:1rem;">No hay gastos registrados.</p>`}
                </div>

                <!-- SECCIÓN INGRESOS -->
                <div id="corr-ingresos" style="display:none;">
                    <h4 style="font-size:0.85rem; color:#10b981; margin-bottom:0.5rem;">💸 Ingresos Informales (Recientes)</h4>
                    ${informales.length > 0 ? `
                        <div class="table-responsive" style="max-height: 180px; overflow-y: auto; margin-bottom: 1.5rem;">
                            <table class="table-modern" style="font-size:0.8rem;">
                                <thead>
                                    <tr>
                                        <th>Fecha</th>
                                        <th>Descripción</th>
                                        <th>Monto</th>
                                        <th>Estatus</th>
                                        <th>Depósito</th>
                                        <th>Acción</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    ${informales.slice(0, 10).map(inf => `
                                        <tr>
                                            <td>${escaparHtml(inf.fecha)}</td>
                                            <td><strong>${escaparHtml(inf.descripcion)}</strong></td>
                                            <td class="amount income">DOP ${formato.importe(inf.monto)}</td>
                                            <td><span class="badge ${inf.estatus === 'pagado' ? 'pagada' : 'emitida'}">${escaparHtml(inf.estatus)}</span></td>
                                            <td>${escaparHtml(inf.institucion_deposito || '-')}</td>
                                            <td>
                                                <button type="button" ${accion('handleEliminarIngresoInformalCorr', inf.id)} class="btn btn-danger" style="padding: 0.2rem 0.4rem; font-size: 0.7rem;">🗑️ Revertir</button>
                                            </td>
                                        </tr>
                                    `).join('')}
                                </tbody>
                            </table>
                        </div>
                    ` : `<p style="color:var(--text-muted); font-size:0.8rem; text-align:center; padding:1rem; margin-bottom:1rem;">No hay ingresos informales registrados.</p>`}

                    <h4 style="font-size:0.85rem; color:var(--accent-primary); margin-bottom:0.5rem;">📄 Ingresos Formales (Facturas Recientes)</h4>
                    ${ingresos.length > 0 ? `
                        <div class="table-responsive" style="max-height: 180px; overflow-y: auto;">
                            <table class="table-modern" style="font-size:0.8rem;">
                                <thead>
                                    <tr>
                                        <th>Factura</th>
                                        <th>Cliente</th>
                                        <th>Fecha</th>
                                        <th>Total</th>
                                        <th>Estatus</th>
                                        <th>Acción</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    ${ingresos.slice(0, 10).map(i => `
                                        <tr>
                                            <td><strong>${escaparHtml(i.numero_factura)}</strong></td>
                                            <td>${escaparHtml(i.cliente_nombre)}</td>
                                            <td>${escaparHtml(i.fecha_emision)}</td>
                                            <td class="amount income">DOP ${formato.importe(i.monto_total)}</td>
                                            <td><span class="badge ${i.estatus === 'pagada' ? 'pagada' : 'emitida'}">${escaparHtml(i.estatus)}</span></td>
                                            <td>
                                                <button type="button" ${accion('handleEliminarIngresoCorr', i.id)} class="btn btn-danger" style="padding: 0.2rem 0.4rem; font-size: 0.7rem;">🗑️ Eliminar</button>
                                            </td>
                                        </tr>
                                    `).join('')}
                                </tbody>
                            </table>
                        </div>
                    ` : `<p style="color:var(--text-muted); font-size:0.8rem; text-align:center; padding:1rem;">No hay ingresos formales registrados.</p>`}
                </div>

                <!-- SECCIÓN TRANSFERENCIAS -->
                <div id="corr-trans" style="display:none;">
                    ${transacciones.length > 0 ? `
                        <div class="table-responsive" style="max-height: 250px; overflow-y: auto;">
                            <table class="table-modern" style="font-size:0.8rem;">
                                <thead>
                                    <tr>
                                        <th>Fecha</th>
                                        <th>Descripción</th>
                                        <th>Origen</th>
                                        <th>Monto</th>
                                        <th>Destino</th>
                                        <th>Cargo</th>
                                        <th>Acción</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    ${transacciones.slice(0, 15).map(t => `
                                        <tr>
                                            <td>${escaparHtml(t.fecha)}</td>
                                            <td><strong>${escaparHtml(t.descripcion || '-')}</strong></td>
                                            <td>${escaparHtml(t.cuenta_origen_nombre)}</td>
                                            <td class="amount expense">- ${formato.importe(t.monto_origen)}</td>
                                            <td>${escaparHtml(t.cuenta_destino_nombre)}</td>
                                            <td class="amount expense">${t.cargo > 0 ? formato.importe(t.cargo) : '-'}</td>
                                            <td>
                                                <button type="button" ${accion('handleEliminarTransaccionCuentaCorr', t.id)} class="btn btn-danger" style="padding: 0.2rem 0.4rem; font-size: 0.7rem;">🗑️ Revertir</button>
                                            </td>
                                        </tr>
                                    `).join('')}
                                </tbody>
                            </table>
                        </div>
                    ` : `<p style="color:var(--text-muted); font-size:0.8rem; text-align:center; padding:1rem;">No hay transferencias de cuenta registradas.</p>`}
                </div>
            </div>

            <div class="card" style="height:fit-content; width: 100%;">
                <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 0.8rem;">💻 Información del Sistema</h3>
                <div style="font-size:0.85rem; color:var(--text-secondary); display:flex; flex-direction:column; gap:0.5rem;">
                    <div>Entorno de Compilación: <span class="badge pagada" style="padding:2px 8px;">Tauri + Rust (macOS Native)</span></div>
                    <div>Persistencia Local SQL: <span class="badge pagada" style="padding:2px 8px;">SQLite 3.x (rusqlite)</span></div>
                    <div>Almacenamiento Físico: <code style="background:rgba(255,255,255,0.02); padding:2px 5px; border-radius:3px; font-size:0.75rem;">~/.michelitos/</code></div>
                    <div style="font-style:italic; font-size:0.75rem; color:var(--text-muted); margin-top:0.5rem; border-top:1px dashed var(--border-color); padding-top:0.5rem;">
                        * Toda la lógica de persistencia, retenciones automáticas del 15% y las comisiones bancarias se gestionan de forma nativa en el compilado de Rust para un rendimiento óptimo.
                    </div>
                </div>
            </div>
        `;
    }

    async handleAgregarTarjeta(e: EventoDeFormulario): Promise<void> {
        e.preventDefault();
        const { api, avisos, enrutador, dom } = this.dep;
        const ent = dom.elemento<Campo>('tar_ent').value;
        const nom = dom.elemento<Campo>('tar_nom').value;
        // Texto, tal cual se escribió (el céntimo lo deciden los dígitos); en blanco = 0, como antes.
        const importe = (id: string): string => dom.elemento<Campo>(id).value.trim() || '0';
        const limDop = importe('tar_lim_dop');
        const sobDop = importe('tar_sob_dop');
        const balDop = importe('tar_bal_dop');
        const corDop = importe('tar_cor_dop');
        const limUsd = importe('tar_lim_usd');
        const sobUsd = importe('tar_sob_usd');
        const balUsd = importe('tar_bal_usd');
        const corUsd = importe('tar_cor_usd');
        const cor = Number(dom.elemento<Campo>('tar_cor').value);
        const pag = Number(dom.elemento<Campo>('tar_pag').value);

        try {
            await api.crearTarjeta(ent, nom, limDop, limUsd, sobDop, sobUsd, balDop, balUsd, corDop, corUsd, cor, pag);
            avisos.mostrar("Tarjeta registrada.");
            await enrutador.mostrar('tarjetas');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    async handleAgregarCliente(e: EventoDeFormulario): Promise<void> {
        e.preventDefault();
        const { api, avisos, enrutador, dom } = this.dep;
        const nom = dom.elemento<Campo>('cli_aj_nom').value;
        const rnc = dom.elemento<Campo>('cli_aj_rnc').value;
        try {
            await api.crearCliente(rnc, nom);
            avisos.mostrar("Cliente registrado.");
            await enrutador.mostrar('ajustes');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    async handleEliminarCliente(id: number): Promise<void> {
        const { api, avisos, enrutador, dialogos } = this.dep;
        if (await dialogos.confirmar("¿Deseas eliminar este cliente?")) {
            try {
                await api.eliminarCliente(id);
                avisos.mostrar("Cliente eliminado.");
                await enrutador.mostrar('ajustes');
            } catch (err) {
                avisos.mostrar(String(err), 'error');
            }
        }
    }

    async handleAgregarCuenta(e: EventoDeFormulario): Promise<void> {
        e.preventDefault();
        const { api, avisos, enrutador, dom } = this.dep;
        const nom = dom.elemento<Campo>('cue_aj_nom').value;
        const div = dom.elemento<Campo>('cue_aj_div').value;
        // Texto, tal cual se escribió: el céntimo lo deciden los dígitos (convención de 1.21.0).
        const bal = dom.elemento<Campo>('cue_aj_bal').value.trim();
        const ent = dom.elemento<Campo>('cue_aj_ent').value;
        // Un campo en blanco es «no declarada», no cero: se envía nulo para
        // que la ausencia siga siendo distinguible de una tarifa gratuita.
        const comTexto = dom.elemento<Campo>('cue_aj_com').value;
        // Como texto, tal cual se escribió: el céntimo lo deciden los dígitos (convención de 1.21.0).
        const com = comTexto.trim() === '' ? null : comTexto.trim();
        try {
            await api.crearCuenta(nom, div, bal, ent, com);
            avisos.mostrar("Cuenta de ahorro registrada.");
            await enrutador.mostrar('ajustes');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    /**
     * Corrige los datos que el titular declara sobre una cuenta.
     *
     * No ofrece el balance a propósito: moverlo sin dejar rastro sería la
     * única forma de que un saldo cambiara sin un asiento detrás.
     */
    async abrirEdicionCuenta(cuenta: CuentaAhorro): Promise<void> {
        const { api, avisos, enrutador, dialogos } = this.dep;
        const nombre = await dialogos.preguntar(`Nombre de la cuenta:`, cuenta.nombre);
        if (nombre === null) return;

        const entidad = await dialogos.preguntar(
            `Entidad con la que se mantiene «${nombre.trim()}»:`,
            cuenta.entidad || ''
        );
        if (entidad === null) return;

        const comisionActual = cuenta.comision_pago_impuestos != null
            ? String(cuenta.comision_pago_impuestos)
            : '';
        const comision = await dialogos.preguntar(
            `Comisión fija por pago de impuestos desde esta cuenta.\n\nDéjalo vacío si la entidad no tiene una tarifa pactada; eso no es lo mismo que declarar cero.`,
            comisionActual
        );
        if (comision === null) return;

        // Se manda **el texto**, no un número. Este campo llega por `prompt`,
        // que no pasa por la validación de `step="0.01"` del navegador, así
        // que aquí sí puede aparecer un tercer decimal — y el céntimo debe
        // decidirlo el núcleo con su regla, no `Number()` sobre binario.
        const valor = comision.trim() === '' ? null : comision.trim();
        if (valor !== null && !/^-?\d*\.?\d+$/.test(valor.replace(/,/g, ''))) {
            avisos.mostrar("La comisión debe ser un importe válido.", "error");
            return;
        }
        if (valor !== null && valor.startsWith('-')) {
            avisos.mostrar("La comisión no puede ser negativa.", "error");
            return;
        }

        try {
            await api.actualizarCuenta(cuenta.id, nombre, entidad, valor);
            avisos.mostrar("Cuenta actualizada.");
            await enrutador.mostrar('ajustes');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    async handleArchivarCuenta(id: number): Promise<void> {
        const { api, avisos, enrutador, dialogos } = this.dep;
        if (await dialogos.confirmar("¿Archivar esta cuenta? Conserva todo su historial, deja de aparecer en selectores y saldos, y puedes reactivarla. Solo se permite con saldo exactamente cero.")) {
            try {
                await api.archivarCuenta(id);
                avisos.mostrar("Cuenta archivada.");
                await enrutador.mostrar('ajustes');
            } catch (err) {
                avisos.mostrar(String(err), 'error');
            }
        }
    }

    async handleReactivarCuenta(id: number): Promise<void> {
        const { api, avisos, enrutador } = this.dep;
        try {
            await api.reactivarCuenta(id);
            avisos.mostrar("Cuenta reactivada.");
            await enrutador.mostrar('ajustes');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    /**
     * Despliega los casos de corrección abiertos.
     *
     * Existe para que el rastro se pueda leer: un registro de auditoría que
     * nadie puede consultar no audita nada. Se carga al abrir porque es un
     * histórico que se mira de vez en cuando.
     */
    async alternarCasosDeCorreccion(): Promise<void> {
        const { api, formato, dom } = this.dep;
        const caja = dom.buscar('casos-correccion');
        if (!caja) return;
        if (!caja.hidden) { caja.hidden = true; return; }

        caja.hidden = false;
        caja.innerHTML = `<p style="color:var(--text-muted); font-size:0.75rem; padding:0.5rem;">Cargando…</p>`;

        try {
            const casos = await api.obtenerCorrecciones();
            if (casos.length === 0) {
                caja.innerHTML = `<p style="color:var(--text-muted); font-size:0.75rem; padding:0.5rem;">Ningún caso abierto. Es la mejor cifra posible.</p>`;
                return;
            }
            caja.innerHTML = `
                <div style="max-height:220px; overflow-y:auto; border:1px solid var(--border-color); border-radius:var(--radius-sm);">
                    ${casos.map(c => `
                        <div style="padding:0.5rem 0.7rem; border-bottom:1px solid var(--border-color); font-size:0.75rem;">
                            <div style="display:flex; justify-content:space-between; gap:0.5rem;">
                                <strong style="color:var(--accent-primary);">${escaparHtml(c.numero_caso)}</strong>
                                <span style="color:var(--text-muted);">${escaparHtml(c.fecha)} · ${escaparHtml(c.tipo)}</span>
                            </div>
                            <div style="margin-top:0.2rem;">
                                ${escaparHtml(c.descripcion)}${c.importe != null ? ` — ${escaparHtml(c.divisa || '')} ${formato.importe(c.importe)}` : ''}
                            </div>
                            ${detalleDeTraspasoHtml(c)}
                            <div style="margin-top:0.2rem; color:var(--text-secondary); font-style:italic;">${escaparHtml(c.motivo)}</div>
                        </div>
                    `).join('')}
                </div>`;
        } catch (err) {
            caja.innerHTML = `<p style="color:var(--color-danger); font-size:0.75rem; padding:0.5rem;">${escaparHtml(String(err))}</p>`;
        }
    }

    async handleEliminarGastoCorr(id: number): Promise<void> {
        const { api, avisos, enrutador, dialogos, motivo: pedidorDeMotivo } = this.dep;
        if (await dialogos.confirmar("¿Estás seguro de que deseas revertir y eliminar este gasto? Los balances asociados serán restaurados.")) {
            try {
                const motivo = await pedidorDeMotivo.pedir(
                    `Vas a borrar este gasto`,
                    'Esto **destruye el movimiento**: no queda un asiento que lo anule, solo el caso de auditoría que estás a punto de abrir.'
                );
                if (motivo === null) return;
                const caso = await api.eliminarGasto(id, motivo);
                avisos.mostrar(`Gasto revertido y eliminado. Caso ${caso}.`);
                await enrutador.mostrar('ajustes');
            } catch (err) {
                avisos.mostrar(String(err), 'error');
            }
        }
    }

    async handleEliminarIngresoInformalCorr(id: number): Promise<void> {
        const { api, avisos, enrutador, dialogos, motivo: pedidorDeMotivo } = this.dep;
        if (await dialogos.confirmar("¿Estás seguro de que deseas revertir y eliminar este ingreso informal? El balance asociado (si ya fue cobrado en efectivo) será descontado.")) {
            try {
                const motivo = await pedidorDeMotivo.pedir(
                    `Vas a borrar este ingreso informal`,
                    'Esto **destruye el movimiento**: no queda un asiento que lo anule, solo el caso de auditoría que estás a punto de abrir.'
                );
                if (motivo === null) return;
                const caso = await api.eliminarIngresoInformal(id, motivo);
                avisos.mostrar("Ingreso informal revertido y eliminado.");
                await enrutador.mostrar('ajustes');
            } catch (err) {
                avisos.mostrar(String(err), 'error');
            }
        }
    }

    async handleEliminarIngresoCorr(id: number): Promise<void> {
        const { api, avisos, enrutador, dialogos, motivo: pedidorDeMotivo } = this.dep;
        if (await dialogos.confirmar("¿Estás seguro de que deseas revertir y eliminar esta factura/ingreso formal?")) {
            try {
                const motivo = await pedidorDeMotivo.pedir(
                    `Vas a borrar esta factura`,
                    'Esto **destruye el movimiento**: no queda un asiento que lo anule, solo el caso de auditoría que estás a punto de abrir.'
                );
                if (motivo === null) return;
                const caso = await api.eliminarIngreso(id, motivo);
                avisos.mostrar("Ingreso formal eliminado.");
                await enrutador.mostrar('ajustes');
            } catch (err) {
                avisos.mostrar(String(err), 'error');
            }
        }
    }

    async handleEliminarTransaccionCuentaCorr(id: number): Promise<void> {
        const { api, avisos, enrutador, dialogos, motivo: pedidorDeMotivo } = this.dep;
        if (await dialogos.confirmar("¿Estás seguro de que deseas revertir y eliminar esta transferencia? Los saldos de las cuentas origen y destino serán restaurados.")) {
            try {
                const motivo = await pedidorDeMotivo.pedir(
                    `Vas a borrar este traspaso entre cuentas`,
                    'Esto **destruye el movimiento**: no queda un asiento que lo anule, solo el caso de auditoría que estás a punto de abrir.'
                );
                if (motivo === null) return;
                const caso = await api.eliminarTransaccionCuenta(id, motivo);
                avisos.mostrar("Transferencia revertida y eliminada.");
                await enrutador.mostrar('ajustes');
            } catch (err) {
                avisos.mostrar(String(err), 'error');
            }
        }
    }

    /**
     * Toma un respaldo bajo demanda.
     *
     * Se deshabilita el botón mientras corre: un `VACUUM INTO` sobre una base
     * grande tarda, y sin esto se acumularían copias por impaciencia.
     */
    async handleCrearRespaldo(boton: BotonDeAccion): Promise<void> {
        const { api, avisos, dom } = this.dep;
        const salida = dom.buscar('respaldo_resultado');
        boton.disabled = true;
        const textoOriginal = boton.textContent;
        boton.textContent = 'Respaldando…';
        try {
            const ruta = await api.crearRespaldo();
            avisos.mostrar('Respaldo creado y verificado.');
            if (salida) salida.textContent = ruta;
        } catch (err) {
            avisos.mostrar(String(err), 'error');
            if (salida) salida.textContent = '';
        } finally {
            boton.disabled = false;
            boton.textContent = textoOriginal;
        }
    }

    /**
     * Restaura un respaldo elegido de la lista.
     *
     * La confirmación dice qué se sustituye y dónde queda la red: es una
     * acción que reemplaza datos vivos y no se debe poder disparar sin leerlo.
     * Al terminar se vuelve a dibujar Ajustes; el resto de pantallas pide sus
     * datos al abrirse, así que no muestran nada anterior.
     */
    async handleRestaurarRespaldo(boton: BotonDeAccion): Promise<void> {
        const { api, avisos, enrutador, dom, dialogos } = this.dep;
        const elegido = dom.buscar<HTMLSelectElement>('respaldo_elegido');
        if (!elegido || !elegido.value) return;
        const etiqueta = elegido.options[elegido.selectedIndex].textContent;
        const nombre = elegido.value;

        if (!await dialogos.confirmar(
            `Se restaurará el respaldo:\n${etiqueta}\n\n` +
            `Todo lo registrado después de esa fecha se perderá de la vista (base y capital). ` +
            `Antes se guardará una copia del estado actual para poder deshacerlo.\n\n¿Restaurar?`
        )) return;

        boton.disabled = true;
        const textoOriginal = boton.textContent;
        boton.textContent = 'Restaurando…';
        try {
            const r = await api.restaurarRespaldo(nombre);
            avisos.mostrar('Respaldo restaurado.');
            await enrutador.mostrar('ajustes');
            const salida = dom.buscar('restauracion_resultado');
            if (salida) {
                salida.textContent =
                    `Restaurado: ${etiqueta}. ` +
                    (r.capital_restaurado ? 'El capital también. ' : 'Ese respaldo no traía capital: se conservó el actual. ') +
                    `Para deshacer, restaura la copia «antes de restaurar» más reciente.`;
            }
        } catch (err) {
            avisos.mostrar(String(err), 'error');
            boton.disabled = false;
            boton.textContent = textoOriginal;
        }
    }

    async handleAgregarCategoria(e: EventoDeFormulario): Promise<void> {
        e.preventDefault();
        const { api, avisos, enrutador, dom } = this.dep;
        const nom = dom.elemento<Campo>('cat_nom').value;
        try {
            await api.crearCategoria(nom);
            avisos.mostrar("Categoría agregada.");
            await enrutador.mostrar('ajustes');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    /**
     * Marca o desmarca una categoría como fija / recurrente. El cambio del casillero ya es la confirmación; si Rust lo
     * rechaza, el casillero vuelve a como estaba y se avisa.
     */
    async handleMarcarCategoriaFija(control: { checked: boolean }, id: number): Promise<void> {
        const { api, avisos } = this.dep;
        const fija = control.checked;
        try {
            const c = await api.marcarCategoriaFija(id, fija);
            avisos.mostrar(c.fija ? `«${c.nombre}» ahora es fija / recurrente.` : `«${c.nombre}» ahora es variable.`);
        } catch (err) {
            control.checked = !fija;
            avisos.mostrar(String(err), 'error');
        }
    }

    /**
     * Una categoría de la lista. Las que el sistema busca por nombre llevan una etiqueta «🔒 Sistema» **con su motivo en
     * texto** (no solo un color ni un icono) y no ofrecen renombrar ni fusionar; el botón de borrar se esconde, como
     * siempre, en «Otros» y «Suscripciones».
     */
    private filaDeCategoria(c: CategoriaDeLista): string {
        const sistema = c.motivo_sistema ?? null;
        const puedeBorrarse = c.nombre !== 'Otros' && c.nombre !== 'Suscripciones';
        const boton = 'padding:0.2rem 0.4rem; font-size:0.75rem;';
        return `
            <div data-categoria="${c.id}" style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.5rem 0.8rem; border-radius:var(--radius-sm);">
                <div style="display:flex; justify-content:space-between; align-items:center; gap:0.5rem;">
                    <strong>${escaparHtml(c.nombre)}</strong>
                    <span style="display:flex; gap:0.3rem; align-items:center; flex-shrink:0; flex-wrap:wrap; justify-content:flex-end;">
                        <label style="display:flex; align-items:center; gap:0.3rem; font-size:0.75rem;" title="${escaparHtml(AYUDA_DE_FIJAS)}">
                            <input type="checkbox" class="cat-fija" ${manejador('change', 'handleMarcarCategoriaFija', [c.id], 'elemento')}${c.fija ? ' checked' : ''} aria-label="Fija / recurrente: ${escaparHtml(c.nombre)}">
                            <span aria-hidden="true">↻</span> Fija / recurrente
                        </label>
                        ${sistema === null ? `
                            <button type="button" ${accion('handleRenombrarCategoria', c.id, c.nombre)} class="btn btn-secondary" style="${boton}" title="Cambiar el nombre">✏️ Renombrar</button>
                            <button type="button" ${accion('handleFusionarCategoria', c.id)} class="btn btn-secondary" style="${boton}" title="Mover sus gastos a otra categoría y eliminarla">🔀 Fusionar…</button>
                        ` : `
                            <span class="etiqueta-sistema" style="font-size:0.7rem; color:var(--text-muted); font-style:italic;">🔒 Sistema</span>
                        `}
                        ${puedeBorrarse ? `<button type="button" ${accion('handleEliminarCategoria', c.id)} class="btn btn-danger" style="${boton}" title="Eliminar">🗑️</button>` : ''}
                    </span>
                </div>
                ${sistema === null ? '' : `<div class="motivo-sistema" style="font-size:0.7rem; color:var(--text-muted); margin-top:0.25rem;">El sistema la usa para ${escaparHtml(sistema)}. Por eso no se renombra ni se fusiona en otra.</div>`}
            </div>`;
    }

    async handleRenombrarCategoria(id: number, nombreActual: string): Promise<void> {
        const { api, avisos, enrutador, dialogos } = this.dep;
        const respuesta = await dialogos.preguntar(`Nuevo nombre para «${nombreActual}» (sus gastos no cambian):`, nombreActual);
        if (respuesta === null) return;
        try {
            const c = await api.renombrarCategoria(id, respuesta);
            const antes = esDeInversionYObligaciones(nombreActual);
            const despues = esDeInversionYObligaciones(c.nombre);
            avisos.mostrar(
                antes === despues
                    ? 'Categoría renombrada.'
                    : `Categoría renombrada. Atención: en el Resumen, «${c.nombre}» ahora ${despues ? 'se excluye' : 'se cuenta'} ` +
                      'cuando «Incluir inversión y obligaciones» está apagado.',
            );
            await enrutador.mostrar('ajustes');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    /** Abre el diálogo de fusión: elegir el destino, ver la vista previa y confirmar. */
    async handleFusionarCategoria(origenId: number): Promise<void> {
        const { api, avisos, modales } = this.dep;
        let categorias: CategoriaDeLista[];
        try {
            categorias = await api.obtenerCategorias();
        } catch (err) {
            avisos.mostrar(String(err), 'error');
            return;
        }
        const origen = categorias.find(c => c.id === origenId);
        const posibles = categorias.filter(c => c.id !== origenId);
        if (!origen || posibles.length === 0) {
            avisos.mostrar('No hay otra categoría a la que fusionarla.', 'error');
            return;
        }
        this.fusionVigente = null;
        modales.abrir(ID_MODAL_FUSION, `
            <div class="card" role="dialog" aria-modal="true" aria-labelledby="fusion_titulo" style="width: 460px; max-width: 92vw; background: var(--bg-surface-opaque); max-height:90vh; overflow-y:auto;">
                <h3 id="fusion_titulo" tabindex="-1" style="font-family: var(--font-heading); margin-bottom: 0.4rem;">🔀 Fusionar «${escaparHtml(origen.nombre)}» en otra categoría</h3>
                <p style="font-size:0.75rem; color:var(--text-secondary); margin-bottom:1rem;">
                    Todos sus gastos pasarán a la categoría que elijas y «${escaparHtml(origen.nombre)}» se eliminará.
                </p>
                <div class="form-group">
                    <label for="fusion_destino">Categoría de destino</label>
                    <select id="fusion_destino" class="form-control" ${manejador('change', 'cambiarDestinoFusion', [origen.id], 'valor')}>
                        <option value="">Elige la categoría de destino…</option>
                        ${posibles.map(c => `<option value="${c.id}">${escaparHtml(c.nombre)}${c.motivo_sistema ? ' (sistema)' : ''}</option>`).join('')}
                    </select>
                </div>
                <div id="fusion_vista_previa" role="status" aria-live="polite" style="font-size:0.8rem; margin: 0.8rem 0; min-height:3rem; color:var(--text-secondary);">
                    Elige un destino para ver qué se moverá.
                </div>
                <div style="display:flex; justify-content:flex-end; gap:0.5rem; margin-top:1rem;">
                    <button type="button" ${accion('quitar', ID_MODAL_FUSION)} class="btn btn-secondary">Cancelar</button>
                    <button type="button" id="fusion_confirmar" ${accion('handleConfirmarFusion', origen.id)} class="btn" disabled>Confirmar fusión</button>
                </div>
            </div>
        `);
    }

    /** Elegido (o quitado) el destino: pide la vista previa y habilita «Confirmar» solo si la fusión es válida. */
    async cambiarDestinoFusion(valor: string, origenId: number): Promise<void> {
        const { api, dom } = this.dep;
        const vista = dom.elemento('fusion_vista_previa');
        const confirmar = dom.elemento<HTMLButtonElement>('fusion_confirmar');
        this.fusionVigente = null;
        confirmar.disabled = true;
        if (valor === '') {
            vista.innerHTML = 'Elige un destino para ver qué se moverá.';
            return;
        }
        vista.textContent = 'Calculando…';
        try {
            const previa = await api.previsualizarFusionCategorias(origenId, Number(valor));
            // Si mientras tanto se eligió otro destino, esta respuesta ya no vale.
            if (dom.buscar<Campo>('fusion_destino')?.value !== valor) return;
            this.fusionVigente = previa;
            vista.innerHTML = this.vistaPreviaDeFusion(previa);
            confirmar.disabled = false;
        } catch (err) {
            if (dom.buscar<Campo>('fusion_destino')?.value !== valor) return;
            vista.innerHTML = `<p class="fusion-error" style="color:var(--color-danger);">⚠️ ${escaparHtml(String(err))}</p>`;
        }
    }

    /** El cuerpo de la vista previa: qué se mueve, de cuándo a cuándo y qué conviene saber antes de confirmar. */
    private vistaPreviaDeFusion(p: VistaPreviaDeFusion): string {
        const advertencias = [...p.advertencias];
        const deOrigen = esDeInversionYObligaciones(p.origen_nombre);
        const deDestino = esDeInversionYObligaciones(p.destino_nombre);
        if (deOrigen !== deDestino) {
            advertencias.push(
                `En el Resumen, el conmutador «Incluir inversión y obligaciones» ${deOrigen ? 'deja fuera' : 'cuenta'} «${p.origen_nombre}» ` +
                `y ${deDestino ? 'deja fuera' : 'cuenta'} «${p.destino_nombre}»: tras la fusión, sus gastos ` +
                `${deDestino ? 'se excluirán' : 'se contarán'} con ese conmutador.`,
            );
        }
        const gastos = p.gastos === 1 ? '1 gasto' : `${p.gastos} gastos`;
        const rango = p.desde !== null && p.hasta !== null
            ? (p.desde === p.hasta ? ` Fecha: ${p.desde}.` : ` Del ${p.desde} al ${p.hasta}.`)
            : '';
        return `
            <p class="fusion-resumen"><strong>${escaparHtml(gastos)}</strong> de «${escaparHtml(p.origen_nombre)}» pasarán a «${escaparHtml(p.destino_nombre)}».${escaparHtml(rango)}</p>
            ${advertencias.map(a => `<p class="fusion-advertencia" style="color:var(--color-warning, #b45309); margin-top:0.4rem;">⚠️ ${escaparHtml(a)}</p>`).join('')}
            <p style="margin-top:0.4rem;">«${escaparHtml(p.origen_nombre)}» se eliminará. Queda un caso en el registro de correcciones.</p>`;
    }

    async handleConfirmarFusion(origenId: number): Promise<void> {
        const { api, avisos, enrutador, dom } = this.dep;
        const previa = this.fusionVigente;
        const destino = Number(dom.buscar<Campo>('fusion_destino')?.value);
        if (!previa || previa.origen_id !== origenId || previa.destino_id !== destino) return;
        const confirmar = dom.elemento<HTMLButtonElement>('fusion_confirmar');
        confirmar.disabled = true;
        try {
            const movidos = await api.fusionarCategorias(origenId, destino);
            dom.buscar(ID_MODAL_FUSION)?.remove();
            this.fusionVigente = null;
            avisos.mostrar(`Fusión hecha: ${movidos === 1 ? '1 gasto pasó' : `${movidos} gastos pasaron`} a «${previa.destino_nombre}».`);
            await enrutador.mostrar('ajustes');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
            confirmar.disabled = false;
        }
    }

    async handleEliminarCategoria(id: number): Promise<void> {
        const { api, avisos, enrutador, dialogos } = this.dep;
        if (await dialogos.confirmar("¿Eliminar esta categoría?")) {
            try {
                await api.eliminarCategoria(id);
                avisos.mostrar("Categoría eliminada.");
                await enrutador.mostrar('ajustes');
            } catch (err) {
                avisos.mostrar(String(err), 'error');
            }
        }
    }
}

/**
 * Lo que `window.appUI` expone de esta vista: un puente que delega en la
 * instancia. Los atributos `data-accion*` no cambian.
 */
export function puenteAjustes(vista: VistaAjustes): {
    handleAgregarCategoria: (e: EventoDeFormulario) => Promise<void>;
    handleEliminarCategoria: (id: number) => Promise<void>;
    handleMarcarCategoriaFija: (control: { checked: boolean }, id: number) => Promise<void>;
    handleRenombrarCategoria: (id: number, nombreActual: string) => Promise<void>;
    handleFusionarCategoria: (origenId: number) => Promise<void>;
    cambiarDestinoFusion: (valor: string, origenId: number) => Promise<void>;
    handleConfirmarFusion: (origenId: number) => Promise<void>;
    handleAgregarCliente: (e: EventoDeFormulario) => Promise<void>;
    handleEliminarCliente: (id: number) => Promise<void>;
    handleAgregarCuenta: (e: EventoDeFormulario) => Promise<void>;
    abrirEdicionCuenta: (cuenta: CuentaAhorro) => Promise<void>;
    handleArchivarCuenta: (id: number) => Promise<void>;
    handleReactivarCuenta: (id: number) => Promise<void>;
    handleAgregarTarjeta: (e: EventoDeFormulario) => Promise<void>;
    handleCrearRespaldo: (boton: BotonDeAccion) => Promise<void>;
    handleRestaurarRespaldo: (boton: BotonDeAccion) => Promise<void>;
    alternarCasosDeCorreccion: () => Promise<void>;
    handleEliminarGastoCorr: (id: number) => Promise<void>;
    handleEliminarIngresoInformalCorr: (id: number) => Promise<void>;
    handleEliminarIngresoCorr: (id: number) => Promise<void>;
    handleEliminarTransaccionCuentaCorr: (id: number) => Promise<void>;
} {
    return {
        handleAgregarCategoria: e => vista.handleAgregarCategoria(e),
        handleEliminarCategoria: id => vista.handleEliminarCategoria(id),
        handleMarcarCategoriaFija: (control, id) => vista.handleMarcarCategoriaFija(control, id),
        handleRenombrarCategoria: (id, nombreActual) => vista.handleRenombrarCategoria(id, nombreActual),
        handleFusionarCategoria: origenId => vista.handleFusionarCategoria(origenId),
        cambiarDestinoFusion: (valor, origenId) => vista.cambiarDestinoFusion(valor, origenId),
        handleConfirmarFusion: origenId => vista.handleConfirmarFusion(origenId),
        handleAgregarCliente: e => vista.handleAgregarCliente(e),
        handleEliminarCliente: id => vista.handleEliminarCliente(id),
        handleAgregarCuenta: e => vista.handleAgregarCuenta(e),
        abrirEdicionCuenta: cuenta => vista.abrirEdicionCuenta(cuenta),
        handleArchivarCuenta: id => vista.handleArchivarCuenta(id),
        handleReactivarCuenta: id => vista.handleReactivarCuenta(id),
        handleAgregarTarjeta: e => vista.handleAgregarTarjeta(e),
        handleCrearRespaldo: boton => vista.handleCrearRespaldo(boton),
        handleRestaurarRespaldo: boton => vista.handleRestaurarRespaldo(boton),
        alternarCasosDeCorreccion: () => vista.alternarCasosDeCorreccion(),
        handleEliminarGastoCorr: id => vista.handleEliminarGastoCorr(id),
        handleEliminarIngresoInformalCorr: id => vista.handleEliminarIngresoInformalCorr(id),
        handleEliminarIngresoCorr: id => vista.handleEliminarIngresoCorr(id),
        handleEliminarTransaccionCuentaCorr: id => vista.handleEliminarTransaccionCuentaCorr(id),
    };
}
