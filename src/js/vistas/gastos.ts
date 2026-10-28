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

import { accion, alCambiar, alEnviar, alEscribir, escaparHtml, manejador } from '../nucleo/html.js';

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

/** Las acciones delegadas (`data-accion*` → `appUI.…`) que la plantilla escribe. */
export const MANEJADORES_GASTOS = [
    'handleAgregarGasto', 'actualizarConversionGasto', 'toggleMetodoPago', 'cambiarMetodoDePago',
    'handleSelectGastosMonth', 'abrirLiquidacionConsumo', 'previsualizarTasa', 'handleLiquidacionSubmit',
    'ofrecerUltimoGasto', 'aplicarUltimoGasto', 'usarUltimoMonto', 'filtrarGastos', 'limpiarFiltrosGastos',
] as const;
export type ManejadorGastos = (typeof MANEJADORES_GASTOS)[number];


// --- Ayudas puras (sin DOM): búsqueda, sugerencias y «repetir último» ---------------------

type GastoListado = Awaited<ReturnType<ApiGastos['obtenerGastos']>>[number];
type CategoriaListada = Awaited<ReturnType<ApiGastos['obtenerCategorias']>>[number];
type TarjetaListada = Awaited<ReturnType<ApiGastos['obtenerTarjetas']>>[number];
type CuentaListada = Awaited<ReturnType<ApiGastos['obtenerCuentas']>>[number];

/** Minúsculas, sin tildes ni diacríticos y con los espacios colapsados: para comparar texto del titular. */
export function normalizarBusqueda(texto: unknown): string {
    return String(texto ?? '')
        .normalize('NFD')
        .replace(/\p{M}/gu, '')
        .toLowerCase()
        .replace(/\s+/g, ' ')
        .trim();
}

/** Clave de una descripción para agrupar repeticiones: sin distinguir mayúsculas, tildes ni espacios sobrantes. */
function claveDeDescripcion(texto: string): string {
    return normalizarBusqueda(texto);
}

const ETIQUETAS_DE_METODO: Readonly<Record<string, string>> = { efectivo: 'Efectivo', tarjeta: 'Tarjeta', transferencia: 'Transferencia' };

/** Etiqueta legible de un método de pago (el valor interno no cambia); un valor desconocido se muestra tal cual. */
export function etiquetaDeMetodo(metodo: string): string {
    return ETIQUETAS_DE_METODO[metodo] ?? metodo;
}

/** `dd/mm/aaaa` → `aaaammdd` (comparable como texto); lo que no encaja queda al fondo. */
function ordenDeFecha(fecha: string): string {
    const p = fecha.split('/');
    return p.length === 3 ? `${p[2].padStart(4, '0')}${p[1].padStart(2, '0')}${p[0].padStart(2, '0')}` : '00000000';
}

/** ¿`a` es más reciente que `b`? (fecha y, a igual fecha, el id mayor). */
function esMasReciente(a: GastoListado, b: GastoListado): boolean {
    const fa = ordenDeFecha(a.fecha), fb = ordenDeFecha(b.fecha);
    return fa !== fb ? fa > fb : a.id > b.id;
}

/** Cuántas descripciones distintas se ofrecen como máximo. */
const MAX_SUGERENCIAS = 100;

/**
 * Las descripciones ya usadas, sin repetir (sin distinguir mayúsculas ni tildes), de la más frecuente a la menos;
 * a igual frecuencia, la usada más recientemente primero. Se muestra la grafía del uso más reciente.
 */
export function sugerenciasDeDescripcion(gastos: readonly GastoListado[]): string[] {
    const grupos = new Map<string, { texto: string; veces: number; ultimo: GastoListado }>();
    for (const g of gastos) {
        const clave = claveDeDescripcion(g.descripcion);
        if (!clave) continue;
        const grupo = grupos.get(clave);
        if (!grupo) { grupos.set(clave, { texto: g.descripcion.trim(), veces: 1, ultimo: g }); continue; }
        grupo.veces++;
        if (esMasReciente(g, grupo.ultimo)) { grupo.ultimo = g; grupo.texto = g.descripcion.trim(); }
    }
    return [...grupos.values()]
        .sort((a, b) => b.veces - a.veces || (esMasReciente(a.ultimo, b.ultimo) ? -1 : 1))
        .slice(0, MAX_SUGERENCIAS)
        .map(g => g.texto);
}

/** Con menos caracteres que esto, un prefijo no se toma como pista (evita ofertas por una sola letra). */
const MIN_PREFIJO = 2;

/**
 * El gasto más reciente cuya descripción coincide con lo escrito, sin distinguir mayúsculas ni tildes.
 * 1) Una descripción idéntica gana siempre (aunque otras empiecen igual).
 * 2) Si no, un prefijo (de al menos {@link MIN_PREFIJO} caracteres) vale solo si todas las coincidencias son UNA
 *    misma descripción; si hay varias distintas es ambiguo y no se ofrece nada (adivinar rellenaría datos de otro gasto).
 */
export function ultimoGastoConDescripcion(gastos: readonly GastoListado[], descripcion: string): GastoListado | null {
    const clave = claveDeDescripcion(descripcion);
    if (!clave) return null;
    const porClave = new Map<string, GastoListado>();
    for (const g of gastos) {
        const k = claveDeDescripcion(g.descripcion);
        if (!k) continue;
        const previo = porClave.get(k);
        if (!previo || esMasReciente(g, previo)) porClave.set(k, g);
    }
    const exacto = porClave.get(clave);
    if (exacto) return exacto;
    if (clave.length < MIN_PREFIJO) return null;
    const candidatos = [...porClave.entries()].filter(([k]) => k.startsWith(clave));
    return candidatos.length === 1 ? candidatos[0][1] : null;
}

const METODOS_DEL_FORMULARIO = ['efectivo', 'tarjeta', 'transferencia'] as const;

/** Qué se rellenaría con los datos de un gasto anterior, y qué se omite porque ya no existe o no está activo. */
export interface PlanDeRepeticion {
    categoriaId: number | null;
    divisa: 'DOP' | 'USD' | null;
    metodo: (typeof METODOS_DEL_FORMULARIO)[number] | null;
    tarjetaId: number | null;
    cuentaId: number | null;
    /** Etiqueta legible de lo que se rellenaría (ya sin escapar). */
    resumen: { categoria: string | null; metodo: string | null; medio: string | null };
    /** Lo que no se puede rellenar, en palabras. */
    omitidos: string[];
}

/**
 * Decide qué campos del formulario se pueden rellenar con `g`. Solo se ofrece lo que sigue existiendo
 * entre los catálogos **activos** (`obtenerCategorias/Tarjetas/Cuentas` ya excluyen tarjetas desactivadas y
 * cuentas archivadas). El monto no entra aquí: se pide aparte.
 */
export function planDeRepeticion(
    g: GastoListado,
    catalogos: { categorias: readonly CategoriaListada[]; tarjetas: readonly TarjetaListada[]; cuentas: readonly CuentaListada[] },
): PlanDeRepeticion {
    const omitidos: string[] = [];
    const categoria = catalogos.categorias.find(c => c.id === g.categoria_id) ?? null;
    if (!categoria) omitidos.push('la categoría ya no existe');

    const divisa = g.divisa === 'USD' ? 'USD' : g.divisa === 'DOP' ? 'DOP' : null;

    let metodo: PlanDeRepeticion['metodo'] = null;
    let tarjetaId: number | null = null;
    let cuentaId: number | null = null;
    let medio: string | null = null;
    const m = (METODOS_DEL_FORMULARIO as readonly string[]).includes(g.metodo_pago) ? g.metodo_pago as PlanDeRepeticion['metodo'] : null;
    if (m === 'efectivo') {
        metodo = 'efectivo';
    } else if (m === 'tarjeta') {
        const t = g.tarjeta_id == null ? null : catalogos.tarjetas.find(x => x.id === g.tarjeta_id) ?? null;
        if (t) { metodo = 'tarjeta'; tarjetaId = t.id; medio = `${t.entidad} - ${t.nombre_tarjeta}`; }
        else omitidos.push('la tarjeta de ese gasto ya no está activa');
    } else if (m === 'transferencia') {
        const c = g.cuenta_ahorro_id == null ? null : catalogos.cuentas.find(x => x.id === g.cuenta_ahorro_id) ?? null;
        if (c) { metodo = 'transferencia'; cuentaId = c.id; medio = c.nombre; }
        else omitidos.push('la cuenta de ese gasto ya no está activa');
    } else {
        omitidos.push('el método de pago no se puede repetir');
    }

    return {
        categoriaId: categoria ? categoria.id : null,
        divisa, metodo, tarjetaId, cuentaId,
        resumen: { categoria: categoria ? categoria.nombre : null, metodo, medio },
        omitidos,
    };
}

/** Los criterios del historial. `categoria` es el id como texto (`''` = todas); `periodo` `'mes'` limita al mes del resumen. */
export interface FiltrosDeGastos {
    texto: string;
    categoria: string;
    metodo: string;
    periodo: 'todos' | 'mes';
}

export const SIN_FILTROS: Readonly<FiltrosDeGastos> = Object.freeze({ texto: '', categoria: '', metodo: '', periodo: 'todos' });

/** Los gastos que cumplen **todos** los criterios (texto en descripción o categoría, categoría, método y periodo). */
export function filtrarGastosListados(gastos: readonly GastoListado[], f: FiltrosDeGastos, mes: string): GastoListado[] {
    const texto = normalizarBusqueda(f.texto);
    return gastos.filter(g => {
        if (f.periodo === 'mes' && !g.fecha.endsWith('/' + mes)) return false;
        if (f.categoria !== '' && String(g.categoria_id) !== f.categoria) return false;
        if (f.metodo !== '' && g.metodo_pago !== f.metodo) return false;
        if (texto && !normalizarBusqueda(g.descripcion).includes(texto) && !normalizarBusqueda(g.categoria_nombre).includes(texto)) return false;
        return true;
    });
}

/** Lo que costó cada gasto (columna «Total» del historial), sumado en centavos enteros y por divisa; sin aritmética `f64` sobre importes. */
export function totalesEnCentavos(gastos: readonly GastoListado[]): Record<string, number> {
    const centavos = (v: number | null | undefined): number => Math.round((v ?? 0) * 100);
    const totales: Record<string, number> = {};
    for (const g of gastos) {
        const liquidado = g.estado_conversion === 'liquidado';
        const divisa = liquidado ? 'DOP' : g.divisa;
        const monto = liquidado ? g.monto_liquidado : g.monto;
        totales[divisa] = (totales[divisa] ?? 0) + centavos(monto) + centavos(g.costo_adicional);
    }
    return totales;
}

/** Un formulario envía un evento con `preventDefault`; solo eso se usa. */
interface EventoDeFormulario {
    preventDefault(): void;
}

export class VistaGastos implements Vista {
    /** Mes elegido (`mm/aaaa`); `null` hasta que se elige uno (entonces manda el mes actual). */
    private selectedGastosMonth: string | null = null;

    /** Criterios del historial; sobreviven a redibujar la pestaña (p. ej. al cambiar de mes). */
    private filtros: FiltrosDeGastos = { ...SIN_FILTROS };
    /** Lo último que `render` leyó, para filtrar y sugerir sin volver a pedirlo a Rust en cada tecla. */
    private gastos: GastoListado[] = [];
    private categorias: CategoriaListada[] = [];
    private tarjetas: TarjetaListada[] = [];
    private cuentas: CuentaListada[] = [];
    private mesVigente = '';
    /** El gasto anterior que se está ofreciendo repetir (o `null`), con lo que se rellenaría. */
    private ofrecido: { gasto: GastoListado; plan: PlanDeRepeticion } | null = null;

    constructor(private readonly dep: DependenciasGastos) {}

    // --- RENDER: GASTOS ---
    async render(): Promise<void> {
        const { api, formato, pantalla, ahora } = this.dep;
        const gastos = await api.obtenerGastos();
        const categorias = await api.obtenerCategorias();
        const tarjetas = await api.obtenerTarjetas();
        const cuentas = await api.obtenerCuentas();
        this.gastos = gastos; this.categorias = categorias; this.tarjetas = tarjetas; this.cuentas = cuentas;
        this.ofrecido = null;
        const sugerencias = sugerenciasDeDescripcion(gastos);

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
        this.mesVigente = mSelected;
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
                    <form id="form-add-gasto" ${alEnviar('handleAgregarGasto')}>
                        <div class="form-row">
                            <div class="form-group">
                                <label for="gas_fec">Fecha *</label>
                                <input type="text" id="gas_fec" class="form-control" value="${escaparHtml(hoyStr)}" placeholder="dd/mm/aaaa" required>
                            </div>
                            <div class="form-group">
                                <label for="gas_div">Divisa</label>
                                <select id="gas_div" class="form-control" ${alCambiar('actualizarConversionGasto')}>
                                    <option value="DOP" selected>DOP</option>
                                    <option value="USD">USD</option>
                                </select>
                            </div>
                        </div>
                        <div class="form-row">
                            <div class="form-group">
                                <label for="gas_mon">Monto *</label>
                                    <input type="number" id="gas_mon" step="0.01" class="form-control" placeholder="0.00" ${alEscribir('actualizarConversionGasto')} required>
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
                            <select id="gas_met" class="form-control" ${manejador('change', 'cambiarMetodoDePago', [], 'valor')} required>
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
                            <select id="gas_cue" class="form-control" ${alCambiar('actualizarConversionGasto')}>
                                <option value="" disabled selected>Seleccione cuenta...</option>
                                ${cuentas.map(c => `<option value="${c.id}" data-divisa="${escaparHtml(c.divisa)}">${escaparHtml(c.nombre)} (${escaparHtml(c.divisa)}) - Bal: ${escaparHtml(c.divisa)} ${formato.importe(c.balance_actual)}</option>`).join('')}
                            </select>
                        </div>

                        <!-- Conversión: solo si el gasto y la cuenta van en divisas distintas -->
                        <div id="gas_conversion_container" class="form-group" style="display:none; background:rgba(255, 193, 7, 0.05); padding:0.6rem; border-radius: var(--radius-sm); border:1px solid rgba(255, 193, 7, 0.2);">
                            <label for="gas_tasa" style="font-size:0.8rem;">Tasa de cambio *</label>
                            <input type="number" id="gas_tasa" step="0.0001" class="form-control" placeholder="0.0000" ${alEscribir('actualizarConversionGasto')}>
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
                            <input type="text" id="gas_des" class="form-control" placeholder="Combustible, compra insumos..." list="gas_des_sugerencias" autocomplete="off" ${alEscribir('ofrecerUltimoGasto')} required>
                            <datalist id="gas_des_sugerencias">
                                ${sugerencias.map(d => `<option value="${escaparHtml(d)}"></option>`).join('')}
                            </datalist>
                        </div>

                        <!-- Oferta de repetir el último gasto con esa descripción: no rellena nada hasta que se pulsa -->
                        <div id="gas_repetir" role="status" style="display:none; background:rgba(0, 242, 254, 0.04); padding:0.6rem; border-radius: var(--radius-sm); border:1px solid rgba(0, 242, 254, 0.15); margin-bottom:1rem; font-size:0.78rem;"></div>

                        <button type="submit" class="btn" style="width:100%;">🚀 Registrar Gasto</button>
                    </form>
                </div>

                <div style="display:flex; flex-direction:column; gap:1.5rem;">
                    <!-- Resumen del mes -->
                    <div class="card">
                        <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom: 1rem;">
                            <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin:0;">📊 Resumen de ${this.formatMonthYearStr(mSelected)}</h3>
                            <select ${manejador('change', 'handleSelectGastosMonth', [], 'valor')} class="form-control" style="width:auto; padding:0.3rem 0.6rem; font-size:0.8rem; margin:0;">
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
                            <div style="display:flex; flex-wrap:wrap; gap:0.5rem; align-items:center; margin-bottom:0.8rem;">
                                <input type="search" id="gas_f_txt" class="form-control" style="flex:2 1 12rem; margin:0;" placeholder="Buscar en descripción o categoría..." aria-label="Buscar gastos por descripción o categoría" autocomplete="off" value="${escaparHtml(this.filtros.texto)}" ${alEscribir('filtrarGastos')}>
                                <select id="gas_f_cat" class="form-control" style="flex:1 1 9rem; margin:0;" aria-label="Filtrar por categoría" ${alCambiar('filtrarGastos')}>
                                    <option value="">Todas las categorías</option>
                                    ${categorias.map(c => `<option value="${c.id}" ${String(c.id) === this.filtros.categoria ? 'selected' : ''}>${escaparHtml(c.nombre)}</option>`).join('')}
                                </select>
                                <select id="gas_f_met" class="form-control" style="flex:1 1 9rem; margin:0;" aria-label="Filtrar por método de pago" ${alCambiar('filtrarGastos')}>
                                    <option value="">Todos los métodos</option>
                                    ${[...new Set(gastos.map(g => g.metodo_pago))].sort().map(m => `<option value="${escaparHtml(m)}" ${m === this.filtros.metodo ? 'selected' : ''}>${escaparHtml(etiquetaDeMetodo(m))}</option>`).join('')}
                                </select>
                                <select id="gas_f_per" class="form-control" style="flex:1 1 9rem; margin:0;" aria-label="Periodo del historial" ${alCambiar('filtrarGastos')}>
                                    <option value="todos" ${this.filtros.periodo === 'todos' ? 'selected' : ''}>Todos los meses</option>
                                    <option value="mes" ${this.filtros.periodo === 'mes' ? 'selected' : ''}>Solo ${this.formatMonthYearStr(mSelected)}</option>
                                </select>
                                <button type="button" class="btn btn-secondary" style="padding:0.3rem 0.8rem; font-size:0.8rem;" ${accion('limpiarFiltrosGastos')}>Limpiar</button>
                            </div>
                            <div id="gas_historial_lista" aria-live="polite">${this.htmlListaHistorial()}</div>
                        ` : `
                            <p style="color:var(--text-muted); text-align:center; padding:1rem;">No hay gastos registrados.</p>
                        `}
                    </div>
                </div>
            </div>
        `;
    }

    /** El contador, los totales de lo mostrado y la tabla del historial, según los filtros vigentes. */
    private htmlListaHistorial(): string {
        const { formato } = this.dep;
        const alcance = this.filtros.periodo === 'mes' ? this.gastos.filter(g => g.fecha.endsWith('/' + this.mesVigente)) : this.gastos;
        const gastos = filtrarGastosListados(this.gastos, this.filtros, this.mesVigente);
        const totales = Object.entries(totalesEnCentavos(gastos));
        const hayFiltro = this.filtros.texto.trim() !== '' || this.filtros.categoria !== '' || this.filtros.metodo !== '' || this.filtros.periodo !== 'todos';
        const cabecera = `
            <div style="display:flex; flex-wrap:wrap; justify-content:space-between; gap:0.5rem; font-size:0.78rem; color:var(--text-secondary); margin-bottom:0.6rem;">
                <span id="gas_f_contador"><strong>${gastos.length}</strong> de ${alcance.length} gastos</span>
                <span>${totales.length > 0 ? 'Total mostrado: ' + totales.map(([d, c]) => `${escaparHtml(d)} ${formato.importe(c / 100)}`).join(' · ') : ''}</span>
            </div>
            ${hayFiltro ? '<div style="font-size:0.7rem; color:var(--text-muted); margin-bottom:0.6rem;">El resumen de arriba corresponde al mes elegido y no cambia con estos filtros.</div>' : ''}`;
        if (gastos.length === 0) {
            return cabecera + '<p style="color:var(--text-muted); text-align:center; padding:1rem; font-size:0.85rem;">Ningún gasto coincide con la búsqueda.</p>';
        }
        return cabecera + `
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
                                    ${pendiente ? `<button ${accion('abrirLiquidacionConsumo', g.id, g.monto, g.divisa, g.descripcion)} class="btn" style="margin-left:0.4rem; padding:0.1rem 0.4rem; font-size:0.65rem; background:rgba(255,193,7,0.15); border:1px solid rgba(255,193,7,0.4); color:#ffc107;" title="El emisor aún no ha fijado el importe en pesos">⏳ Liquidar</button>` : ''}
                                    ${liquidado ? `<span style="margin-left:0.4rem; font-size:0.65rem; color:var(--text-muted);" title="Tasa aplicada por el emisor">@ ${Number(g.tasa_conversion).toFixed(4)}</span>` : ''}
                                </td>
                                <td>${escaparHtml(g.categoria_nombre)}</td>
                                <td>${escaparHtml(g.fecha)}</td>
                                <td>${escaparHtml(etiquetaDeMetodo(g.metodo_pago))}</td>
                                <td class="amount">${escaparHtml(g.divisa)} ${formato.importe(g.monto)}</td>
                                <td class="amount expense">${escaparHtml(divisaFinal)} ${formato.importe(g.costo_adicional)}</td>
                                <td class="amount" style="font-weight:bold;">${escaparHtml(divisaFinal)} ${formato.importe((montoFinal ?? 0) + g.costo_adicional)}</td>
                            </tr>
                        `;}).join('')}
                    </tbody>
                </table>
            </div>`;
    }

    private redibujarHistorial(): void {
        const lista = this.dep.dom.buscar('gas_historial_lista');
        if (lista) lista.innerHTML = this.htmlListaHistorial();
    }

    /** Lee los cuatro controles de búsqueda y redibuja solo la lista (la caja de texto conserva el foco). */
    filtrarGastos(): void {
        const { dom } = this.dep;
        const periodo = dom.buscar<Campo>('gas_f_per')?.value;
        this.filtros = {
            texto: dom.buscar<Campo>('gas_f_txt')?.value ?? '',
            categoria: dom.buscar<Campo>('gas_f_cat')?.value ?? '',
            metodo: dom.buscar<Campo>('gas_f_met')?.value ?? '',
            periodo: periodo === 'mes' ? 'mes' : 'todos',
        };
        this.redibujarHistorial();
    }

    limpiarFiltrosGastos(): void {
        const { dom } = this.dep;
        this.filtros = { ...SIN_FILTROS };
        for (const [id, valor] of [['gas_f_txt', ''], ['gas_f_cat', ''], ['gas_f_met', ''], ['gas_f_per', 'todos']] as const) {
            const campo = dom.buscar<Campo>(id);
            if (campo) campo.value = valor;
        }
        this.redibujarHistorial();
    }

    // --- REPETIR ÚLTIMO ---
    /** Al escribir o elegir la descripción: si ya existe, ofrece (sin rellenar nada) repetir los datos del último gasto con ella. */
    ofrecerUltimoGasto(): void {
        const { dom, formato } = this.dep;
        const panel = dom.buscar('gas_repetir');
        if (!panel) return;
        const gasto = ultimoGastoConDescripcion(this.gastos, dom.buscar<Campo>('gas_des')?.value ?? '');
        if (!gasto) {
            this.ofrecido = null;
            panel.style.display = 'none';
            panel.innerHTML = '';
            return;
        }
        const plan = planDeRepeticion(gasto, { categorias: this.categorias, tarjetas: this.tarjetas, cuentas: this.cuentas });
        this.ofrecido = { gasto, plan };
        const partes = [plan.resumen.categoria, plan.resumen.metodo ? etiquetaDeMetodo(plan.resumen.metodo) : null, plan.resumen.medio, plan.divisa]
            .filter((p): p is string => Boolean(p)).map(p => `<strong>${escaparHtml(p)}</strong>`);
        panel.innerHTML = `
            <div style="margin-bottom:0.4rem;">Ya registraste «${escaparHtml(gasto.descripcion)}» (último: ${escaparHtml(gasto.fecha)})${partes.length ? ': ' + partes.join(' · ') : ''}.</div>
            ${plan.omitidos.length ? `<div style="color:var(--text-muted); margin-bottom:0.4rem;">No se rellenará: ${plan.omitidos.map(o => escaparHtml(o)).join('; ')}.</div>` : ''}
            <div style="display:flex; flex-wrap:wrap; gap:0.4rem;">
                <button type="button" class="btn btn-secondary" style="padding:0.2rem 0.6rem; font-size:0.72rem;" ${accion('aplicarUltimoGasto')}>Rellenar con esos datos</button>
                <button type="button" class="btn btn-secondary" style="padding:0.2rem 0.6rem; font-size:0.72rem;" ${accion('usarUltimoMonto')}>Usar el último monto (${escaparHtml(gasto.divisa)} ${formato.importe(gasto.monto)})</button>
            </div>`;
        panel.style.display = 'block';
    }

    /** Rellena categoría, método (con su tarjeta o cuenta) y divisa del gasto ofrecido. No toca el monto, la fecha ni la descripción, y no guarda nada. */
    aplicarUltimoGasto(): void {
        const { dom } = this.dep;
        if (!this.ofrecido) return;
        const { plan } = this.ofrecido;
        const poner = (id: string, valor: string): void => { const c = dom.buscar<Campo>(id); if (c) c.value = valor; };
        if (plan.categoriaId !== null) poner('gas_cat', String(plan.categoriaId));
        if (plan.divisa) poner('gas_div', plan.divisa);
        if (plan.metodo) {
            poner('gas_met', plan.metodo);
            if (plan.tarjetaId !== null) poner('gas_tar', String(plan.tarjetaId));
            if (plan.cuentaId !== null) poner('gas_cue', String(plan.cuentaId));
            this.cambiarMetodoDePago(plan.metodo);
        } else {
            this.actualizarConversionGasto();
        }
    }

    /** Pone en el monto el del gasto ofrecido (solo si el titular lo pide) y recalcula la previa de conversión. */
    usarUltimoMonto(): void {
        const { dom } = this.dep;
        if (!this.ofrecido) return;
        const campo = dom.buscar<Campo>('gas_mon');
        if (campo) campo.value = String(this.ofrecido.gasto.monto);
        this.actualizarConversionGasto();
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

    /** Al elegir el método de pago: muestra sus campos y recalcula la conversión (antes dos llamadas en un solo `onchange`). */
    cambiarMetodoDePago(val: string): void {
        this.toggleMetodoPago(val);
        this.actualizarConversionGasto();
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
                <form ${alEnviar('handleLiquidacionSubmit', id, montoOrigen)}>
                    <div class="form-group">
                        <label>Importe cargado en pesos (DOP)</label>
                        <input type="number" step="0.01" id="liq_monto_${id}" class="form-control" placeholder="0.00"
                               ${alEscribir('previsualizarTasa', id, montoOrigen)} required autofocus>
                        <div id="liq_tasa_${id}" style="font-size:0.72rem; color:var(--text-secondary); margin-top:0.4rem;"></div>
                    </div>
                    <div style="display:flex; justify-content:flex-end; gap:0.5rem; margin-top:1.2rem;">
                        <button type="button" ${accion('quitar', `modal-liq-${id}`)} class="btn btn-secondary">Cancelar</button>
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
 * instancia. Los atributos `data-accion*` no cambian.
 */
export function puenteGastos(vista: VistaGastos): {
    handleAgregarGasto: (e: EventoDeFormulario) => Promise<void>;
    actualizarConversionGasto: () => void;
    toggleMetodoPago: (val: string) => void;
    cambiarMetodoDePago: (val: string) => void;
    handleSelectGastosMonth: (val: string) => Promise<void>;
    abrirLiquidacionConsumo: (id: number, montoOrigen: number, divisaOrigen: string, descripcion: string) => void;
    previsualizarTasa: (id: number, montoOrigen: number) => void;
    handleLiquidacionSubmit: (e: EventoDeFormulario, id: number, montoOrigen: number) => Promise<void>;
    ofrecerUltimoGasto: () => void;
    aplicarUltimoGasto: () => void;
    usarUltimoMonto: () => void;
    filtrarGastos: () => void;
    limpiarFiltrosGastos: () => void;
} {
    return {
        handleAgregarGasto: e => vista.handleAgregarGasto(e),
        actualizarConversionGasto: () => vista.actualizarConversionGasto(),
        toggleMetodoPago: val => vista.toggleMetodoPago(val),
        cambiarMetodoDePago: val => vista.cambiarMetodoDePago(val),
        handleSelectGastosMonth: val => vista.handleSelectGastosMonth(val),
        abrirLiquidacionConsumo: (id, montoOrigen, divisaOrigen, descripcion) => vista.abrirLiquidacionConsumo(id, montoOrigen, divisaOrigen, descripcion),
        previsualizarTasa: (id, montoOrigen) => vista.previsualizarTasa(id, montoOrigen),
        handleLiquidacionSubmit: (e, id, montoOrigen) => vista.handleLiquidacionSubmit(e, id, montoOrigen),
        ofrecerUltimoGasto: () => vista.ofrecerUltimoGasto(),
        aplicarUltimoGasto: () => vista.aplicarUltimoGasto(),
        usarUltimoMonto: () => vista.usarUltimoMonto(),
        filtrarGastos: () => vista.filtrarGastos(),
        limpiarFiltrosGastos: () => vista.limpiarFiltrosGastos(),
    };
}
