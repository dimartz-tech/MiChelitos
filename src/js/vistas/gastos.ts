// Pestaña «Gastos» (diseño «B»: clase con dependencias inyectadas).
//
// Registro de consumos, la vista con más reglas de dinero de la interfaz: filtra
// por mes, calcula la previa de una conversión de divisa, decide qué tarjeta
// recomendar y liquida un consumo pendiente. Estrena **estado propio**: el mes
// elegido (`selectedGastosMonth`) deja de ser un campo de `AppUI` y pasa a ser
// privado de la vista. No usa `this` de ningún objeto mezclado, ni `AppAPI`,
// `elemento()`, `document` ni `Date` globales: todo entra por el constructor
// (`pruebas/js/vistas/gastos.test.js`). Los cuerpos son los de `ui.ts`.

import type { ApiDe, Avisos, Dom, Enrutador, Formato, Modales, Pantalla, Reloj, Vista } from '../ui/servicios';

import { argumentoJs, escaparHtml } from '../nucleo/html.js';

type ApiGastos = ApiDe<
    | 'obtenerGastos'
    | 'obtenerCategorias'
    | 'obtenerCuentas'
    | 'obtenerTarjetas'
    | 'crearGasto'
    | 'liquidarConsumoPendiente'
>;

export interface DependenciasGastos {
    api: ApiGastos;
    avisos: Avisos;
    formato: Formato;
    enrutador: Enrutador;
    pantalla: Pantalla;
    dom: Dom;
    modales: Modales;
    ahora: Reloj;
}

/** Los manejadores en línea (`onsubmit`/`onchange`/`oninput`/`onclick` → `appUI.…`) que la plantilla escribe. */
export const MANEJADORES_GASTOS = [
    'handleAgregarGasto', 'actualizarConversionGasto', 'toggleMetodoPago',
    'handleSelectGastosMonth', 'abrirLiquidacionConsumo', 'previsualizarTasa', 'handleLiquidacionSubmit',
] as const;
export type ManejadorGastos = (typeof MANEJADORES_GASTOS)[number];

/** Un formulario envía un evento con `preventDefault`; solo eso se usa. */
interface EventoDeFormulario {
    preventDefault(): void;
}

export class VistaGastos implements Vista {
    /** Mes elegido (`mm/aaaa`); `null` hasta que se elige uno (entonces manda el mes actual). */
    private selectedGastosMonth: string | null = null;

    constructor(private readonly dep: DependenciasGastos) {}

    // --- RENDER: GASTOS ---
    async render(): Promise<void> {
        const { api, formato, pantalla, ahora } = this.dep;
        const gastos = await api.obtenerGastos();
        const categorias = await api.obtenerCategorias();
        const tarjetas = await api.obtenerTarjetas();
        const cuentas = await api.obtenerCuentas();

        const hoy = ahora();
        const hoyStr = hoy.getDate().toString().padStart(2, '0') + '/' + (hoy.getMonth() + 1).toString().padStart(2, '0') + '/' + hoy.getFullYear();
        
        // Extraer todos los meses/años disponibles con transacciones
        const mesesDisponibles = [...new Set(gastos.map(g => {
            const parts = g.fecha.split('/');
            if (parts.length === 3) {
                return `${parts[1]}/${parts[2]}`; // Formato: "mm/yyyy"
            }
            return null;
        }).filter((m): m is string => Boolean(m)))].sort((a, b) => {
            const [mA, yA] = a.split('/').map(Number);
            const [mB, yB] = b.split('/').map(Number);
            return yB !== yA ? yB - yA : mB - mA;
        });

        const mesActualStr = ((hoy.getMonth() + 1).toString().padStart(2, '0')) + '/' + hoy.getFullYear();
        if (!mesesDisponibles.includes(mesActualStr)) {
            mesesDisponibles.unshift(mesActualStr);
        }

        const mSelected = this.selectedGastosMonth || mesActualStr;
        const gastosMes = gastos.filter(g => g.fecha.endsWith('/' + mSelected));

        // Separar cálculos por divisa para evitar mezclar DOP y USD
        const gastosDop = gastosMes.filter(g => (g.divisa || 'DOP') === 'DOP');
        const totalNetoDop = gastosDop.reduce((sum, g) => sum + g.monto, 0);
        const totalComisionesDop = gastosDop.reduce((sum, g) => sum + g.costo_adicional, 0);

        const gastosUsd = gastosMes.filter(g => (g.divisa || 'DOP') === 'USD');
        const totalNetoUsd = gastosUsd.reduce((sum, g) => sum + g.monto, 0);
        const totalComisionesUsd = gastosUsd.reduce((sum, g) => sum + g.costo_adicional, 0);

        // Agrupar gastos del mes seleccionado por categoría y divisa
        const categoryTotals: Record<string, Record<string, number>> = {};
        gastosMes.forEach(g => {
            const cat = g.categoria_nombre || 'Sin Categoría';
            const divisa = g.divisa || 'DOP';
            if (!categoryTotals[cat]) {
                categoryTotals[cat] = {};
            }
            if (!categoryTotals[cat][divisa]) {
                categoryTotals[cat][divisa] = 0;
            }
            categoryTotals[cat][divisa] += g.monto + g.costo_adicional;
        });

        pantalla.contenido.innerHTML = `
            <div class="section-title">
                <h1>Gastos y Egresos</h1>
                <span class="subtitle">Control presupuestario y comisiones financieras</span>
            </div>

            <div class="responsive-split-grid">
                <div class="card" style="height: fit-content;">
                    <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 1rem;">💸 Registrar Gasto</h3>
                    <form id="form-add-gasto" onsubmit="appUI.handleAgregarGasto(event)">
                        <div class="form-row">
                            <div class="form-group">
                                <label for="gas_fec">Fecha *</label>
                                <input type="text" id="gas_fec" class="form-control" value="${escaparHtml(hoyStr)}" placeholder="dd/mm/aaaa" required>
                            </div>
                            <div class="form-group">
                                <label for="gas_div">Divisa</label>
                                <select id="gas_div" class="form-control" onchange="appUI.actualizarConversionGasto()">
                                    <option value="DOP" selected>DOP</option>
                                    <option value="USD">USD</option>
                                </select>
                            </div>
                        </div>
                        <div class="form-row">
                            <div class="form-group">
                                <label for="gas_mon">Monto *</label>
                                    <input type="number" id="gas_mon" step="0.01" class="form-control" placeholder="0.00" oninput="appUI.actualizarConversionGasto()" required>
                            </div>
                            <div class="form-group">
                                <label for="gas_cat">Categoría *</label>
                                <select id="gas_cat" class="form-control" required>
                                    <option value="" disabled selected>Seleccione...</option>
                                    ${categorias.map(c => `<option value="${c.id}">${escaparHtml(c.nombre)}</option>`).join('')}
                                </select>
                            </div>
                        </div>
                        <div class="form-group">
                            <label for="gas_met">Método de Pago *</label>
                            <select id="gas_met" class="form-control" onchange="appUI.toggleMetodoPago(this.value); appUI.actualizarConversionGasto();" required>
                                <option value="efectivo" selected>Efectivo</option>
                                <option value="tarjeta">Tarjeta de Crédito</option>
                                <option value="transferencia">Transferencia Bancaria</option>
                            </select>
                        </div>

                        <!-- Selector tarjeta de crédito -->
                        <div id="gas_tarjeta_container" class="form-group" style="display:none;">
                            <label for="gas_tar">Tarjeta de Crédito *</label>
                            <select id="gas_tar" class="form-control">
                                <option value="" disabled selected>Seleccione tarjeta...</option>
                                ${tarjetas.map(t => `<option value="${t.id}">${escaparHtml(t.entidad)} - ${escaparHtml(t.nombre_tarjeta)}</option>`).join('')}
                            </select>
                        </div>

                        <!-- Selector cuenta de ahorro -->
                        <div id="gas_cuenta_container" class="form-group" style="display:none;">
                            <label for="gas_cue">Cuenta de Ahorro *</label>
                            <select id="gas_cue" class="form-control" onchange="appUI.actualizarConversionGasto()">
                                <option value="" disabled selected>Seleccione cuenta...</option>
                                ${cuentas.map(c => `<option value="${c.id}" data-divisa="${escaparHtml(c.divisa)}">${escaparHtml(c.nombre)} (${escaparHtml(c.divisa)}) - Bal: ${escaparHtml(c.divisa)} ${formato.importe(c.balance_actual)}</option>`).join('')}
                            </select>
                        </div>

                        <!-- Conversión: solo si el gasto y la cuenta van en divisas distintas -->
                        <div id="gas_conversion_container" class="form-group" style="display:none; background:rgba(255, 193, 7, 0.05); padding:0.6rem; border-radius: var(--radius-sm); border:1px solid rgba(255, 193, 7, 0.2);">
                            <label for="gas_tasa" style="font-size:0.8rem;">Tasa de cambio *</label>
                            <input type="number" id="gas_tasa" step="0.0001" class="form-control" placeholder="0.0000" oninput="appUI.actualizarConversionGasto()">
                            <div id="gas_conversion_previa" style="font-size:0.72rem; color:var(--text-secondary); margin-top:0.4rem;"></div>
                        </div>

                        <!-- Recordatorio de LBTR -->
                        <div id="gas_lbtr_container" class="form-group" style="display:none; background:rgba(0, 242, 254, 0.04); padding: 0.6rem; border-radius: var(--radius-sm); border:1px solid rgba(0, 242, 254, 0.15);">
                            <label class="form-checkbox" style="font-size:0.8rem;">
                                <input type="checkbox" id="gas_lbtr"> ¿Transferencia LBTR? (+100.00 DOP)
                            </label>
                        </div>

                        <div class="form-group">
                            <label for="gas_des">Descripción *</label>
                            <input type="text" id="gas_des" class="form-control" placeholder="Combustible, compra insumos..." required>
                        </div>

                        <button type="submit" class="btn" style="width:100%;">🚀 Registrar Gasto</button>
                    </form>
                </div>

                <div style="display:flex; flex-direction:column; gap:1.5rem;">
                    <!-- Resumen del mes -->
                    <div class="card">
                        <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom: 1rem;">
                            <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin:0;">📊 Resumen de ${this.formatMonthYearStr(mSelected)}</h3>
                            <select onchange="appUI.handleSelectGastosMonth(this.value)" class="form-control" style="width:auto; padding:0.3rem 0.6rem; font-size:0.8rem; margin:0;">
                                ${mesesDisponibles.map(m => `
                                    <option value="${escaparHtml(m)}" ${m === mSelected ? 'selected' : ''}>${this.formatMonthYearStr(m)}</option>
                                `).join('')}
                            </select>
                        </div>
                        
                        <div style="display:flex; flex-direction:column; gap:1.2rem;">
                            <!-- Resumen DOP -->
                            ${totalNetoDop > 0 || totalComisionesDop > 0 ? `
                                <div>
                                    <div style="font-size:0.75rem; font-weight:bold; color:var(--text-secondary); margin-bottom:0.4rem; text-align:left;">💵 Resumen en Pesos (DOP)</div>
                                    <div style="display:grid; grid-template-columns: repeat(3, 1fr); gap:1rem; text-align:center;">
                                        <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.6rem; border-radius:var(--radius-sm);">
                                            <span style="font-size:0.7rem; color:var(--text-secondary);">Monto Neto</span>
                                            <div class="amount" style="font-size:1.1rem; margin-top:0.2rem;">DOP ${formato.importe(totalNetoDop)}</div>
                                        </div>
                                        <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.6rem; border-radius:var(--radius-sm);">
                                            <span style="font-size:0.7rem; color:var(--text-secondary);">Comisiones</span>
                                            <div class="amount expense" style="font-size:1.1rem; margin-top:0.2rem;">DOP ${formato.importe(totalComisionesDop)}</div>
                                        </div>
                                        <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.6rem; border-radius:var(--radius-sm);">
                                            <span style="font-size:0.7rem; color:var(--text-secondary);">Total Debitado</span>
                                            <div class="amount" style="font-size:1.1rem; margin-top:0.2rem;">DOP ${formato.importe(totalNetoDop + totalComisionesDop)}</div>
                                        </div>
                                    </div>
                                </div>
                            ` : ''}

                            <!-- Resumen USD -->
                            ${totalNetoUsd > 0 || totalComisionesUsd > 0 ? `
                                <div>
                                    <div style="font-size:0.75rem; font-weight:bold; color:var(--text-secondary); margin-bottom:0.4rem; text-align:left;">🇺🇸 Resumen en Dólares (USD)</div>
                                    <div style="display:grid; grid-template-columns: repeat(3, 1fr); gap:1rem; text-align:center;">
                                        <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.6rem; border-radius:var(--radius-sm);">
                                            <span style="font-size:0.7rem; color:var(--text-secondary);">Monto Neto</span>
                                            <div class="amount" style="font-size:1.1rem; margin-top:0.2rem; color:#60a5fa;">USD ${formato.importe(totalNetoUsd)}</div>
                                        </div>
                                        <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.6rem; border-radius:var(--radius-sm);">
                                            <span style="font-size:0.7rem; color:var(--text-secondary);">Comisiones</span>
                                            <div class="amount expense" style="font-size:1.1rem; margin-top:0.2rem;">USD ${formato.importe(totalComisionesUsd)}</div>
                                        </div>
                                        <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.6rem; border-radius:var(--radius-sm);">
                                            <span style="font-size:0.7rem; color:var(--text-secondary);">Total Debitado</span>
                                            <div class="amount" style="font-size:1.1rem; margin-top:0.2rem; color:#60a5fa; font-weight:bold;">USD ${formato.importe(totalNetoUsd + totalComisionesUsd)}</div>
                                        </div>
                                    </div>
                                </div>
                            ` : ''}

                            ${!(totalNetoDop > 0 || totalComisionesDop > 0 || totalNetoUsd > 0 || totalComisionesUsd > 0) ? `
                                <p style="color:var(--text-muted); text-align:center; padding:1rem; font-size:0.8rem; margin:0;">No hay gastos registrados en este mes.</p>
                            ` : ''}
                        </div>

                        <!-- Distribución por Categorías -->
                        <div style="margin-top: 1.5rem; border-top: 1px solid var(--border-color); padding-top: 1.2rem;">
                            <h4 style="font-family: var(--font-heading); font-size: 1rem; margin-bottom: 0.8rem; color: var(--text-secondary);">Gastos por Categoría</h4>
                            ${Object.keys(categoryTotals).length > 0 ? `
                                <div style="display:flex; flex-direction:column; gap:1rem;">
                                    ${Object.entries(categoryTotals).map(([cat, values]) => {
                                        return Object.entries(values).map(([divisa, total]) => {
                                            const totalCurrency = gastosMes
                                                .filter(g => (g.divisa || 'DOP') === divisa)
                                                .reduce((sum, g) => sum + g.monto + g.costo_adicional, 0);
                                            const pct = totalCurrency > 0 ? (total / totalCurrency) * 100 : 0;
                                            
                                            let barColor = 'var(--accent-primary)';
                                            if (cat.toLowerCase().includes('impuesto')) barColor = 'var(--color-warning)';
                                            else if (cat.toLowerCase().includes('comida') || cat.toLowerCase().includes('restaurante')) barColor = 'var(--color-success)';
                                            else if (cat.toLowerCase().includes('personal')) barColor = 'var(--color-info)';
                                            else if (cat.toLowerCase().includes('bancar') || cat.toLowerCase().includes('comisi') || cat.toLowerCase().includes('tarjeta')) barColor = 'var(--color-danger)';
                                            
                                            return `
                                                <div>
                                                    <div style="display:flex; justify-content:space-between; font-size:0.8rem; margin-bottom:0.25rem;">
                                                        <span style="font-weight:600;">${escaparHtml(cat)} <span style="font-size:0.7rem; color:var(--text-muted);">(${escaparHtml(divisa)})</span></span>
                                                        <span style="font-family:var(--font-heading); font-weight:700;">
                                                            ${escaparHtml(divisa)} ${formato.importe(total)} 
                                                            <span style="font-size:0.7rem; color:var(--text-secondary); font-weight:normal; margin-left:0.25rem;">(${pct.toFixed(1)}%)</span>
                                                        </span>
                                                    </div>
                                                    <div style="background:rgba(255,255,255,0.03); height:6px; border-radius:3px; overflow:hidden; border:1px solid var(--border-color);">
                                                        <div style="background:${escaparHtml(barColor)}; width:${pct}%; height:100%; border-radius:3px;"></div>
                                                    </div>
                                                </div>
                                            `;
                                        }).join('');
                                    }).join('')}
                                </div>
                            ` : `
                                <p style="color:var(--text-muted); text-align:center; padding:1rem; font-size:0.8rem; margin:0;">No hay gastos registrados en este mes.</p>
                            `}
                        </div>
                    </div>

                    <!-- Historial -->
                    <div class="card">
                        <h3 style="font-family: var(--font-heading); font-size: 1.1rem; margin-bottom: 1rem;">📋 Historial de Egresos</h3>
                        ${gastos.length > 0 ? `
                            <div class="table-responsive">
                                <table class="table-modern">
                                    <thead>
                                        <tr>
                                            <th>Descripción</th>
                                            <th>Categoría</th>
                                            <th>Fecha</th>
                                            <th>Método</th>
                                            <th>Neto</th>
                                            <th>Costo Adicional</th>
                                            <th>Total</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        ${gastos.map(g => {
                                            const pendiente = g.estado_conversion === 'pendiente';
                                            const liquidado = g.estado_conversion === 'liquidado';
                                            // Un consumo liquidado se muestra por lo que realmente
                                            // costó en pesos; uno pendiente, en su divisa, porque
                                            // el importe en pesos aún no existe.
                                            const divisaFinal = liquidado ? 'DOP' : g.divisa;
                                            const montoFinal = liquidado ? g.monto_liquidado : g.monto;
                                            return `
                                            <tr>
                                                <td>
                                                    <strong>${escaparHtml(g.descripcion)}</strong>
                                                    ${pendiente ? `<button onclick="appUI.abrirLiquidacionConsumo(${g.id}, ${g.monto}, ${argumentoJs(g.divisa)}, ${argumentoJs(g.descripcion)})" class="btn" style="margin-left:0.4rem; padding:0.1rem 0.4rem; font-size:0.65rem; background:rgba(255,193,7,0.15); border:1px solid rgba(255,193,7,0.4); color:#ffc107;" title="El emisor aún no ha fijado el importe en pesos">⏳ Liquidar</button>` : ''}
                                                    ${liquidado ? `<span style="margin-left:0.4rem; font-size:0.65rem; color:var(--text-muted);" title="Tasa aplicada por el emisor">@ ${Number(g.tasa_conversion).toFixed(4)}</span>` : ''}
                                                </td>
                                                <td>${escaparHtml(g.categoria_nombre)}</td>
                                                <td>${escaparHtml(g.fecha)}</td>
                                                <td style="text-transform:capitalize;">${escaparHtml(g.metodo_pago)}</td>
                                                <td class="amount">${escaparHtml(g.divisa)} ${formato.importe(g.monto)}</td>
                                                <td class="amount expense">${escaparHtml(divisaFinal)} ${formato.importe(g.costo_adicional)}</td>
                                                <td class="amount" style="font-weight:bold;">${escaparHtml(divisaFinal)} ${formato.importe((montoFinal ?? 0) + g.costo_adicional)}</td>
                                            </tr>
                                        `;}).join('')}
                                    </tbody>
                                </table>
                            </div>
                        ` : `
                            <p style="color:var(--text-muted); text-align:center; padding:1rem;">No hay gastos registrados.</p>
                        `}
                    </div>
                </div>
            </div>
        `;
    }

    private formatMonthYearStr(myStr: string | null | undefined): string {
        if (!myStr) return "";
        const [m, y] = myStr.split('/');
        const months = [
            "Enero", "Febrero", "Marzo", "Abril", "Mayo", "Junio",
            "Julio", "Agosto", "Septiembre", "Octubre", "Noviembre", "Diciembre"
        ];
        return `${months[parseInt(m) - 1]} ${y}`;
    }

    async handleSelectGastosMonth(val: string): Promise<void> {
        this.selectedGastosMonth = val;
        await this.render();
    }

    toggleMetodoPago(val: string): void {
        const { dom } = this.dep;
        const tarjeta = dom.buscar('gas_tarjeta_container');
        const cuenta = dom.buscar('gas_cuenta_container');
        const lbtr = dom.buscar('gas_lbtr_container');
        
        if (tarjeta) tarjeta.style.display = val === 'tarjeta' ? 'block' : 'none';
        if (cuenta) cuenta.style.display = val === 'transferencia' ? 'block' : 'none';
        if (lbtr) lbtr.style.display = val === 'transferencia' ? 'block' : 'none';
    }

    // --- MANEJADORES DE ENTRADAS ---
    /**
     * Muestra el campo de tasa solo cuando el gasto y la cuenta de débito van
     * en divisas distintas, y adelanta lo que saldrá de la cuenta.
     *
     * Mostrar el importe antes de confirmar evita el descuadre que obliga
     * después a revertir el gasto.
     */
    actualizarConversionGasto(): void {
        const { formato, dom } = this.dep;
        const contenedor = dom.buscar('gas_conversion_container');
        if (!contenedor) return;

        const metodo = dom.buscar<Campo>('gas_met')?.value;
        const selCuenta = dom.buscar<HTMLSelectElement>('gas_cue');
        const opcion = selCuenta?.selectedOptions?.[0];
        const divisaCuenta = opcion?.dataset?.divisa;
        const divisaGasto = dom.buscar<Campo>('gas_div')?.value;

        const cruzaDivisas = metodo === 'transferencia' && divisaCuenta && divisaGasto && divisaCuenta !== divisaGasto;
        contenedor.style.display = cruzaDivisas ? 'block' : 'none';

        const previa = dom.buscar('gas_conversion_previa');
        if (!cruzaDivisas) { if (previa) previa.textContent = ''; return; }

        const monto = Number(dom.buscar<Campo>('gas_mon')?.value) || 0;
        const tasa = Number(dom.buscar<Campo>('gas_tasa')?.value) || 0;
        if (!(monto > 0) || !(tasa > 0)) {
            previa!.textContent = `Indica la tasa para saber cuánto saldrá en ${divisaCuenta}.`;
            return;
        }

        // Misma regla que el dominio: convertir primero, retener después.
        const convertido = divisaGasto === 'USD' ? monto * tasa : monto / tasa;
        const convertidoRedondeado = Math.round(convertido * 100) / 100;
        const retencion = Math.round(convertidoRedondeado * 0.002 * 100) / 100;
        const lbtr = dom.buscar<HTMLInputElement>('gas_lbtr')?.checked ? 100 : 0;
        const total = convertidoRedondeado + retencion + lbtr;

        previa!.innerHTML = `Saldrán <strong>${escaparHtml(divisaCuenta)} ${formato.importe(total)}</strong> `
            + `— ${formato.importe(convertidoRedondeado)} convertidos`
            + (retencion ? ` + ${formato.importe(retencion)} de retención` : '')
            + (lbtr ? ` + ${formato.importe(lbtr)} de LBTR` : '');
    }

    async handleAgregarGasto(e: EventoDeFormulario): Promise<void> {
        e.preventDefault();
        const { api, avisos, enrutador, dom } = this.dep;
        const fec = dom.elemento<Campo>('gas_fec').value;
        // Texto, tal cual se escribió: el céntimo lo deciden los dígitos (convención de 1.21.0).
        const mon = dom.elemento<Campo>('gas_mon').value.trim();
        const div = dom.elemento<Campo>('gas_div').value;
        const des = dom.elemento<Campo>('gas_des').value;
        const cat = Number(dom.elemento<Campo>('gas_cat').value);
        const met = dom.elemento<Campo>('gas_met').value;
        const lbtr = dom.buscar('gas_lbtr') ? dom.elemento<HTMLInputElement>('gas_lbtr').checked : false;
        const tar = dom.buscar('gas_tar') ? Number(dom.elemento<Campo>('gas_tar').value) : null;
        const cue = dom.buscar('gas_cue') && met === 'transferencia' ? Number(dom.elemento<Campo>('gas_cue').value) : null;

        // La tasa solo viaja si el gasto y la cuenta van en divisas distintas.
        // El backend la exige en ese caso y la ignora en el resto.
        const opcionCuenta = dom.buscar<HTMLSelectElement>('gas_cue')?.selectedOptions?.[0];
        const divisaCuenta = opcionCuenta?.dataset?.divisa;
        const cruzaDivisas = met === 'transferencia' && divisaCuenta && divisaCuenta !== div;
        const tasa = cruzaDivisas ? Number(dom.buscar<Campo>('gas_tasa')?.value) || 0 : 0;

        if (cruzaDivisas && !(tasa > 0)) {
            avisos.mostrar(`El gasto va en ${div} y la cuenta en ${divisaCuenta}: indica la tasa de cambio.`, 'error');
            return;
        }

        // Un consumo con tarjeta tiene que decir con cuál. El backend ya lo
        // rechaza, pero conviene decirlo aquí con el nombre del campo: si no
        // hay ninguna tarjeta registrada el selector ni siquiera existe, y el
        // mensaje de error genérico no daría ninguna pista.
        if (met === 'tarjeta' && !((tar ?? 0) > 0)) {
            // El selector se dibuja aunque no haya ninguna tarjeta, así que lo
            // que distingue los dos casos es si ofrece alguna opción real.
            const selectorTarjeta = dom.buscar<HTMLSelectElement>('gas_tar');
            const hayTarjetas = [...(selectorTarjeta?.options ?? [])].some(o => Number(o.value) > 0);
            avisos.mostrar(
                hayTarjetas
                    ? 'Indica con qué tarjeta se pagó el gasto.'
                    : 'No hay tarjetas registradas: registra una antes de pagar con tarjeta.',
                'error'
            );
            return;
        }

        try {
            await api.crearGasto({
                fecha: fec,
                monto: mon,
                divisa: div,
                descripcion: des,
                categoria_id: cat,
                metodo_pago: met,
                es_lbtr: lbtr,
                tarjeta_id: met === 'tarjeta' ? tar : null,
                cuenta_ahorro_id: met === 'transferencia' ? cue : null,
                tasa_cambio: cruzaDivisas ? tasa : null
            });
            avisos.mostrar("Gasto registrado con éxito.");
            await enrutador.mostrar('gastos');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }

    /**
     * Liquida un consumo pendiente. Pide el IMPORTE en pesos, no la tasa: el
     * emisor comunica cuánto cargó, nunca a qué tasa lo hizo.
     */
    abrirLiquidacionConsumo(id: number, montoOrigen: number, divisaOrigen: string, descripcion: string): void {
        const { formato, modales } = this.dep;
        modales.abrir(`modal-liq-${id}`, `
            <div class="card" style="width: 420px; background: var(--bg-surface-opaque);">
                <h3 style="font-family: var(--font-heading); margin-bottom:0.4rem;">⏳ Liquidar consumo</h3>
                <p style="font-size:0.75rem; color:var(--text-secondary); margin-bottom:1rem;">
                    ${escaparHtml(descripcion)} — <strong>${escaparHtml(divisaOrigen)} ${formato.importe(montoOrigen)}</strong><br>
                    Indica el importe en pesos que aparece en tu estado de cuenta. La tasa se deduce sola.
                </p>
                <form onsubmit="appUI.handleLiquidacionSubmit(event, ${id}, ${montoOrigen})">
                    <div class="form-group">
                        <label>Importe cargado en pesos (DOP)</label>
                        <input type="number" step="0.01" id="liq_monto_${id}" class="form-control" placeholder="0.00"
                               oninput="appUI.previsualizarTasa(${id}, ${montoOrigen})" required autofocus>
                        <div id="liq_tasa_${id}" style="font-size:0.72rem; color:var(--text-secondary); margin-top:0.4rem;"></div>
                    </div>
                    <div style="display:flex; justify-content:flex-end; gap:0.5rem; margin-top:1.2rem;">
                        <button type="button" onclick="elemento('modal-liq-${id}').remove()" class="btn btn-secondary">Cancelar</button>
                        <button type="submit" class="btn">Liquidar</button>
                    </div>
                </form>
            </div>
        `);
    }

    previsualizarTasa(id: number, montoOrigen: number): void {
        const { dom } = this.dep;
        const destino = Number(dom.elemento<Campo>(`liq_monto_${id}`).value);
        const salida = dom.elemento(`liq_tasa_${id}`);
        if (!(destino > 0) || !(montoOrigen > 0)) { salida.textContent = ''; return; }
        salida.innerHTML = `Tasa aplicada por el emisor: <strong>${(destino / montoOrigen).toFixed(4)}</strong>`;
    }

    async handleLiquidacionSubmit(e: EventoDeFormulario, id: number, montoOrigen: number): Promise<void> {
        e.preventDefault();
        const { api, avisos, enrutador, dom } = this.dep;
        // Texto, tal cual se escribió (convención de 1.21.0); el número solo sirve para comparar.
        const monto = dom.elemento<Campo>(`liq_monto_${id}`).value.trim();
        if (!(Number(monto) > 0)) { avisos.mostrar("El importe en pesos debe ser mayor que cero.", "error"); return; }

        try {
            const tasa = await api.liquidarConsumoPendiente(id, monto);
            avisos.mostrar(`Consumo liquidado a una tasa de ${Number(tasa).toFixed(4)}.`);
            dom.buscar(`modal-liq-${id}`)?.remove();
            await enrutador.mostrar('gastos');
        } catch (err) {
            avisos.mostrar(String(err), 'error');
        }
    }
}

/**
 * Lo que `window.appUI` expone de esta vista: un puente que delega en la
 * instancia. Los atributos en línea no cambian.
 */
export function puenteGastos(vista: VistaGastos): {
    handleAgregarGasto: (e: EventoDeFormulario) => Promise<void>;
    actualizarConversionGasto: () => void;
    toggleMetodoPago: (val: string) => void;
    handleSelectGastosMonth: (val: string) => Promise<void>;
    abrirLiquidacionConsumo: (id: number, montoOrigen: number, divisaOrigen: string, descripcion: string) => void;
    previsualizarTasa: (id: number, montoOrigen: number) => void;
    handleLiquidacionSubmit: (e: EventoDeFormulario, id: number, montoOrigen: number) => Promise<void>;
} {
    return {
        handleAgregarGasto: e => vista.handleAgregarGasto(e),
        actualizarConversionGasto: () => vista.actualizarConversionGasto(),
        toggleMetodoPago: val => vista.toggleMetodoPago(val),
        handleSelectGastosMonth: val => vista.handleSelectGastosMonth(val),
        abrirLiquidacionConsumo: (id, montoOrigen, divisaOrigen, descripcion) => vista.abrirLiquidacionConsumo(id, montoOrigen, divisaOrigen, descripcion),
        previsualizarTasa: (id, montoOrigen) => vista.previsualizarTasa(id, montoOrigen),
        handleLiquidacionSubmit: (e, id, montoOrigen) => vista.handleLiquidacionSubmit(e, id, montoOrigen),
    };
}
