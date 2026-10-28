// Pestaña «Resumen» (diseño «B»: clase con dependencias inyectadas).
//
// Solo lectura: consultas a la API y una pantalla. Lo único interactivo es el bloque
// «Gasto por categoría» (mes, promedio y conmutador; ver `resumen_gastos.ts`). No
// usa `this` de ningún objeto mezclado ni `AppAPI` global: todo entra por el
// constructor, así que se puede probar en Node con dobles
// (`pruebas/js/vistas/resumen.test.js`). El cuerpo es el de `ui.ts`; solo cambian
// los accesos a lo compartido (ver `docs/historico/division_de_ui_limpia.md`).

import type { Categoria, MesDisponible } from '../tipos-ipc';
import type { ApiDe, Dom, Formato, Pantalla, Referencias, Reloj, Vista } from '../ui/servicios';

import { escaparHtml } from '../nucleo/html.js';
import { consultarPatrimonio, htmlCifrasDePatrimonio } from './patrimonio_cifras.js';
import {
    ID_ANTERIOR, ID_BLOQUE_GASTO, ID_CONMUTADOR, ID_MES, ID_PROMEDIO, ID_SIGUIENTE, PROMEDIO_POR_DEFECTO, PROMEDIOS_OFRECIDOS,
    type EstadoDeGasto, htmlDeGastoPorCategoria, idsDeInversionYObligaciones, mesDeValor, mesesDelSelector,
} from './resumen_gastos.js';

type ApiResumen = ApiDe<
    | 'obtenerCapital'
    | 'obtenerIngresos'
    | 'obtenerIngresosInformales'
    | 'obtenerGastos'
    | 'obtenerPatrimonio'
    | 'obtenerPrestamos'
    | 'obtenerSuscripciones'
    | 'obtenerTarjetas'
    | 'obtenerCategorias'
    | 'obtenerResumenGastos'
>;

export interface DependenciasResumen {
    api: ApiResumen;
    formato: Formato;
    pantalla: Pantalla;
    dom: Dom;
    ahora: Reloj;
    referencias: Referencias;
}

/** Lo que el titular tiene elegido en el bloque «Gasto por categoría» y lo último que se supo de él. */
interface EstadoDelBloque {
    anio: number;
    mes: number;
    nPromedio: number;
    incluirTodo: boolean;
    meses: MesDisponible[];
    estado: EstadoDeGasto;
    /** Contador de peticiones: una respuesta con otro número llegó tarde y se descarta. */
    secuencia: number;
    /** El catálogo de categorías (se pide una vez, solo si se apaga «Incluir inversión y obligaciones»). */
    categorias: Categoria[] | null;
    /** Los nombres que dejó fuera la última petición con el conmutador apagado. */
    excluidas: string[];
    /** El control que se tocó, para devolverle el foco al repintar. */
    enfoque: string | null;
}

export class VistaResumen implements Vista {
    private gasto: EstadoDelBloque = {
        anio: 0, mes: 0, nPromedio: PROMEDIO_POR_DEFECTO, incluirTodo: true, meses: [],
        estado: { tipo: 'cargando' }, secuencia: 0, categorias: null, excluidas: [], enfoque: null,
    };

    constructor(private readonly dep: DependenciasResumen) {}

    // --- RENDER: RESUMEN ---
    async render(): Promise<void> {
        const { api, formato, pantalla, ahora, referencias } = this.dep;
        const capital = await api.obtenerCapital();
        const ingresos = await api.obtenerIngresos();
        const informales = await api.obtenerIngresosInformales();
        const gastos = await api.obtenerGastos();
        const tarjetas = await api.obtenerTarjetas();
        const suscripciones = await api.obtenerSuscripciones();
        const prestamos = await api.obtenerPrestamos();
        // Las dos cifras del patrimonio las compone el núcleo; la cifra de arriba (la de siempre) se conserva tal cual.
        const patrimonio = await consultarPatrimonio(api);

        const hoy = ahora();
        const mesAnioActual = "/" + (hoy.getMonth() + 1).toString().padStart(2, '0') + '/' + hoy.getFullYear();

        // 1. Activos
        // Los totales los suma el núcleo, en centavos enteros: una suma de
        // decimales aquí arrastraría ruido de coma flotante, y sería una
        // regla de dinero sin pruebas.
        const totales = capital?.totales || {};
        const totalCertificados = totales.certificados ?? 0;
        const totalBolsa = totales.bolsa ?? 0;
        const totalInmueble = totales.inmobiliario ?? 0;
        const totalVehiculo = totales.vehiculos ?? 0;
        const totalMaquinaria = totales.maquinaria ?? 0;
        
        const totalActivos = totales.patrimonio ?? 0;

        // 2. Pasivos
        const totalTarjetas = tarjetas.reduce((sum, t) => sum + t.balance_pesos + (t.balance_dolares * referencias.tasaUsdADop), 0);
        
        // El pasivo es el saldo que se lleva, no una fracción del monto
        // original. La fracción suponía amortización lineal —falsa en todo
        // préstamo real— y en una línea revolvente ni se aplicaba: sin cuotas
        // contadas, el pasivo se quedaba en el monto desembolsado para siempre.
        const totalPrestamos = prestamos.reduce((sum, p) => sum + p.saldo_actual, 0);

        const totalPasivos = totalTarjetas + totalPrestamos;
        const patrimonioNeto = totalActivos - totalPasivos;
        const ratio = totalActivos > 0 ? ((totalPasivos / totalActivos) * 100.0) : 0.0;

        // 3. Flujos Fijos
        const cuotaPrestamos = prestamos.reduce((sum, p) => sum + (p.tipo_prestamo === 'flexible' || (p.cuotas_pendientes ?? 0) > 0 ? p.monto_cuota : 0.0), 0);
        // Las suscripciones se suman **por divisa y nunca entre divisas**: un dólar
        // no es un peso, y convertirlo con una tasa fija daría una cifra que no
        // corresponde con lo que se cobra de verdad. Los préstamos no tienen divisa
        // (son en pesos). El total de la cabecera es el de pesos; las demás divisas
        // se muestran aparte, cada una con la suya.
        // Subdivisión por frecuencia, y por divisa sin mezclarlas: lo mensual se suma tal cual; lo anual se
        // guarda como total del año y su equivalente al mes (total / 12) sale de él, no de cada suscripción.
        interface Subdivision { mensual: number; anual: number }
        const subdivisiones = new Map<string, Subdivision>();
        for (const s of suscripciones) {
            const divisa = s.divisa || 'DOP';
            const sub = subdivisiones.get(divisa) ?? { mensual: 0, anual: 0 };
            if (s.frecuencia === 'mensual') sub.mensual += s.monto; else sub.anual += s.monto;
            subdivisiones.set(divisa, sub);
        }
        const alMes = (sub: Subdivision | undefined): number => (sub ? sub.mensual + sub.anual / 12.0 : 0);
        const subDop = subdivisiones.get('DOP') ?? { mensual: 0, anual: 0 };
        const cuotaSuscripciones = alMes(subDop);
        const suscripcionesOtrasDivisas = [...subdivisiones.entries()]
            .filter(([d]) => d !== 'DOP')
            .sort(([x], [y]) => x.localeCompare(y))
            .map(([d, sub]): [string, number] => [d, alMes(sub)]);
        const filaSuscripcion = (rotulo: string, divisa: string, importe: number, nota = ''): string => `
                        <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); padding:0.5rem; border-radius:4px;">
                            <span>${escaparHtml(rotulo)}${nota ? ` <small style="color:var(--text-muted);">${escaparHtml(nota)}</small>` : ''}</span>
                            <strong>${escaparHtml(divisa)} ${formato.importe(importe)}</strong>
                        </div>`;
        const filasSuscripciones = (divisa: string, sub: Subdivision, siempre: boolean): string => {
            const en = divisa === 'DOP' ? '' : ` en ${divisa}`;
            return filaSuscripcion(`Suscripciones Mensuales${en}`, divisa, sub.mensual)
                + (siempre || sub.anual > 0
                    ? filaSuscripcion(`Suscripciones Anuales${en} (al mes)`, divisa, sub.anual / 12.0, `total del año: ${divisa} ${formato.importe(sub.anual)}`)
                    : '');
        };
        const cargaFija = cuotaPrestamos + cuotaSuscripciones;

        // Cobros de los próximos días (la regla de la ventana es del núcleo: `avisa`). Se avisa aquí, no solo en
        // Suscripciones, para dar tiempo a cambiar la tarjeta que cobra antes de que caiga el cargo.
        const cobrosProximos = suscripciones.filter(s => s.avisa);
        const avisoCobrosProximos = cobrosProximos.length === 0 ? '' : `
            <div class="card" style="border-left:3px solid var(--warning, #e0a020); margin-bottom:1rem;">
                <strong>🔔 Cobros próximos</strong>
                <ul style="margin:0.5rem 0 0 1rem; font-size:0.85rem;">
                    ${cobrosProximos.map(s => `<li><strong>${escaparHtml(s.plataforma)}</strong> — ${escaparHtml(s.divisa)} ${formato.importe(s.monto)} el ${escaparHtml(s.fecha_proximo_cobro)}, con ${escaparHtml(s.entidad)} (${escaparHtml(s.nombre_tarjeta)})</li>`).join('')}
                </ul>
                <p style="margin:0.5rem 0 0; font-size:0.75rem; color:var(--text-muted);">Para cobrarlos con otra tarjeta, edita la suscripción en la pestaña Suscripciones antes de la fecha.</p>
            </div>`;

        // 4. Balance del Mes (basado en monto cobrado/recibido)
        const ingresosMesFormalesCobrados = ingresos.filter(i => i.fecha_emision.endsWith(mesAnioActual)).reduce((sum, i) => sum + (i.monto_recibido || 0.0), 0);
        const ingresosMesInformalesCobrados = informales.filter(inf => inf.fecha.endsWith(mesAnioActual)).reduce((sum, inf) => sum + (inf.monto_recibido || 0.0), 0);
        const ingresosCobradosTotales = ingresosMesFormalesCobrados + ingresosMesInformalesCobrados;

        const ingresosFacturadosFormales = ingresos.filter(i => i.fecha_emision.endsWith(mesAnioActual)).reduce((sum, i) => sum + i.monto_total, 0);
        const ingresosFacturadosInformales = informales.filter(inf => inf.fecha.endsWith(mesAnioActual)).reduce((sum, inf) => sum + inf.monto, 0);
        const ingresosFacturadosTotales = ingresosFacturadosFormales + ingresosFacturadosInformales;

        const gastosTotales = gastos.filter(g => g.fecha.endsWith(mesAnioActual)).reduce((sum, g) => sum + g.monto + g.costo_adicional, 0);
        const balanceNeto = ingresosCobradosTotales - gastosTotales;

        // El bloque «Gasto por categoría» arranca en el mes en curso; lo demás que se eligió se conserva.
        this.gasto.anio = hoy.getFullYear();
        this.gasto.mes = hoy.getMonth() + 1;
        this.gasto.estado = { tipo: 'cargando' };
        this.gasto.excluidas = [];
        this.gasto.enfoque = null;
        const peticion = ++this.gasto.secuencia;

        pantalla.contenido.innerHTML = `
            <div class="section-title">
                <h1>Resumen Ejecutivo</h1>
                <span class="subtitle">Cálculos de patrimonio, deuda y flujos recurrentes</span>
            </div>

            ${avisoCobrosProximos}

            <!-- Balance Ejecutivo -->
            <div class="card" style="border-left: 6px solid var(--accent-primary);">
                <div style="display:flex; justify-content:space-between; align-items:center; flex-wrap:wrap; gap:1.5rem;">
                    <div>
                        <span style="font-size:0.85rem; color:var(--text-secondary); text-transform:uppercase; letter-spacing:0.05em;">Patrimonio Neto</span>
                        <h2 class="amount" style="font-size:2.2rem; margin-top:0.2rem; background: linear-gradient(135deg, var(--accent-primary), var(--accent-primary-hover)); -webkit-background-clip: text; -webkit-text-fill-color: transparent;">
                            DOP ${formato.importe(patrimonioNeto)}
                        </h2>
                    </div>
                    
                    <div style="background:rgba(255,255,255,0.02); border:1px solid var(--border-color); padding:0.8rem 1.2rem; border-radius:var(--radius-md); text-align:center; min-width:160px;">
                        <span style="font-size:0.75rem; color:var(--text-secondary); display:block; margin-bottom:0.25rem;">Carga Endeudamiento</span>
                        <strong style="font-size:1.5rem; font-family:var(--font-heading);">${ratio.toFixed(1)}%</strong>
                        <span class="badge ${ratio < 30 ? 'pagada' : ratio <= 50 ? 'emitida' : 'danger'}" style="display:block; margin-top:0.4rem; font-size:0.65rem;">
                            ${ratio < 30 ? 'Saludable' : ratio <= 50 ? 'Moderado' : 'Alto Riesgo'}
                        </span>
                    </div>
                </div>
            </div>

            ${htmlCifrasDePatrimonio(patrimonio, formato)}

            <div class="grid-2">
                <!-- Activos -->
                <div class="card">
                    <h3 style="font-family:var(--font-heading); font-size:1.15rem; color:var(--color-success); border-bottom:1px solid var(--border-color); padding-bottom:0.5rem; display:flex; justify-content:space-between; margin-bottom:1rem;">
                        <span>🏢 Activos (Patrimonio)</span>
                        <span>DOP ${formato.importe(totalActivos)}</span>
                    </h3>
                    <div style="display:flex; flex-direction:column; gap:0.6rem; font-size:0.85rem;">
                        <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); padding:0.5rem; border-radius:4px;">
                            <span>Certificados Financieros</span>
                            <strong>DOP ${formato.importe(totalCertificados)}</strong>
                        </div>
                        <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); padding:0.5rem; border-radius:4px;">
                            <span>Bolsa de Valores</span>
                            <strong>DOP ${formato.importe(totalBolsa)}</strong>
                        </div>
                        <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); padding:0.5rem; border-radius:4px;">
                            <span>Bienes y Propiedades</span>
                            <strong>DOP ${formato.importe(totalInmueble + totalVehiculo + totalMaquinaria)}</strong>
                        </div>
                    </div>
                </div>

                <!-- Pasivos -->
                <div class="card">
                    <h3 style="font-family:var(--font-heading); font-size:1.15rem; color:var(--color-danger); border-bottom:1px solid var(--border-color); padding-bottom:0.5rem; display:flex; justify-content:space-between; margin-bottom:1rem;">
                        <span>🏷️ Pasivos (Deudas)</span>
                        <span>DOP ${formato.importe(totalPasivos)}</span>
                    </h3>
                    <div style="display:flex; flex-direction:column; gap:0.6rem; font-size:0.85rem;">
                        <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); padding:0.5rem; border-radius:4px;">
                            <span>Balances Tarjetas de Crédito</span>
                            <strong>DOP ${formato.importe(totalTarjetas)}</strong>
                        </div>
                        <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); padding:0.5rem; border-radius:4px;">
                            <span>Préstamos Vigentes</span>
                            <strong>DOP ${formato.importe(totalPrestamos)}</strong>
                        </div>
                    </div>
                </div>
            </div>

            <div class="grid-2">
                <!-- Compromisos Fijos -->
                <div class="card">
                    <h3 style="font-family:var(--font-heading); font-size:1.15rem; border-bottom:1px solid var(--border-color); padding-bottom:0.5rem; display:flex; justify-content:space-between; margin-bottom:1rem;">
                        <span>🔄 Carga Fija Mensual</span>
                        <span class="amount expense">DOP ${formato.importe(cargaFija)}${suscripcionesOtrasDivisas.map(([d, m]) => ` + ${escaparHtml(d)} ${formato.importe(m)}`).join('')}</span>
                    </h3>
                    <div style="display:flex; flex-direction:column; gap:0.6rem; font-size:0.85rem;">
                        <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); padding:0.5rem; border-radius:4px;">
                            <span>Cuotas de Préstamos</span>
                            <strong>DOP ${formato.importe(cuotaPrestamos)}</strong>
                        </div>
                        ${filasSuscripciones('DOP', subDop, true)}
                        ${[...subdivisiones.entries()].filter(([d]) => d !== 'DOP').sort(([x], [y]) => x.localeCompare(y)).map(([d, sub]) => filasSuscripciones(d, sub, false)).join('')}
                    </div>
                </div>

                <!-- Balance del Mes -->
                <div class="card">
                    <h3 style="font-family:var(--font-heading); font-size:1.15rem; border-bottom:1px solid var(--border-color); padding-bottom:0.5rem; display:flex; justify-content:space-between; margin-bottom:1rem;">
                        <span>📊 Balance Mensual (Cobrado)</span>
                        <span class="amount ${balanceNeto >= 0 ? 'income' : 'expense'}">DOP ${formato.importe(balanceNeto)}</span>
                    </h3>
                    <div style="display:flex; flex-direction:column; gap:0.6rem; font-size:0.85rem;">
                        <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); padding:0.5rem; border-radius:4px;">
                            <span>Ingresos Cobrados (Flujo)</span>
                            <strong>DOP ${formato.importe(ingresosCobradosTotales)}</strong>
                        </div>
                        <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); padding:0.5rem; border-radius:4px; opacity:0.8;">
                            <span>Ingresos Facturados (Impuestos)</span>
                            <span>DOP ${formato.importe(ingresosFacturadosTotales)}</span>
                        </div>
                        <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); padding:0.5rem; border-radius:4px;">
                            <span>Gastos y Cargos (Este Mes)</span>
                            <strong>DOP ${formato.importe(gastosTotales)}</strong>
                        </div>
                    </div>
                </div>
            </div>

            <!-- Gasto por categoría y mes (lo calcula Rust: obtener_resumen_gastos) -->
            <div class="card" id="${ID_BLOQUE_GASTO}">${this.htmlDelBloque()}</div>
        `;
        // Un fallo del bloque se queda en el bloque: el resto del Resumen ya está dibujado.
        await this.cargarGasto(peticion);
    }

    // --- GASTO POR CATEGORÍA ---

    private htmlDelBloque(): string {
        const g = this.gasto;
        return htmlDeGastoPorCategoria(g.estado, {
            anio: g.anio, mes: g.mes, nPromedio: g.nPromedio, incluirTodo: g.incluirTodo, mesesDisponibles: g.meses, excluidas: g.excluidas,
        });
    }

    private pintarGasto(): void {
        const caja = this.dep.dom.buscar(ID_BLOQUE_GASTO);
        if (!caja) return; // la pestaña ya no está en pantalla
        caja.innerHTML = this.htmlDelBloque();
        // Repintar destruye el control que el titular tocó: el foco vuelve a él (teclado y lectores de pantalla).
        if (this.gasto.enfoque) this.dep.dom.buscar(this.gasto.enfoque)?.focus?.();
    }

    /** Pide el resumen del mes elegido. Solo la última petición pinta: una respuesta vieja (o su error) se descarta. */
    private async cargarGasto(mia: number): Promise<void> {
        const { api, referencias } = this.dep;
        const g = this.gasto;
        const vigente = (): boolean => g.secuencia === mia;
        try {
            let excluir: number[] = [];
            g.excluidas = [];
            if (!g.incluirTodo) {
                g.categorias ??= await api.obtenerCategorias();
                if (!vigente()) return;
                excluir = idsDeInversionYObligaciones(g.categorias);
                g.excluidas = g.categorias.filter(c => excluir.includes(c.id)).map(c => c.nombre);
            }
            const resumen = await api.obtenerResumenGastos(g.anio, g.mes, g.nPromedio, referencias.tasaUsdADop, excluir);
            if (!vigente()) return;
            g.meses = resumen.meses_disponibles;
            g.estado = { tipo: 'ok', resumen };
        } catch (err) {
            if (!vigente()) return;
            g.estado = { tipo: 'error', mensaje: String(err) };
        }
        this.pintarGasto();
    }

    /** Cambia lo elegido y vuelve a pedir; muestra «cargando» mientras llega. */
    private async recargarGasto(control: string): Promise<void> {
        const g = this.gasto;
        g.estado = { tipo: 'cargando' };
        g.enfoque = control;
        const mia = ++g.secuencia;
        this.pintarGasto();
        await this.cargarGasto(mia);
    }

    /** El selector de mes: valor «aaaa-mm». */
    async elegirMesDeGasto(valor: string): Promise<void> {
        const m = mesDeValor(valor);
        if (!m) return;
        this.gasto.anio = m.anio;
        this.gasto.mes = m.mes;
        await this.recargarGasto(ID_MES);
    }

    /** Anterior / siguiente: se mueve entre los meses del selector (con datos, más el elegido). */
    async moverMesDeGasto(delta: number): Promise<void> {
        const g = this.gasto;
        const meses = mesesDelSelector(g.meses, { anio: g.anio, mes: g.mes });
        const i = meses.findIndex(m => m.anio === g.anio && m.mes === g.mes);
        const destino = meses[i + Math.sign(delta)];
        if (!destino) return;
        g.anio = destino.anio;
        g.mes = destino.mes;
        await this.recargarGasto(delta < 0 ? ID_ANTERIOR : ID_SIGUIENTE);
    }

    /** «Promedio de N meses». */
    async elegirPromedioDeGasto(valor: string): Promise<void> {
        const n = Number(valor);
        if (!PROMEDIOS_OFRECIDOS.includes(n)) return;
        this.gasto.nPromedio = n;
        await this.recargarGasto(ID_PROMEDIO);
    }

    /** «Incluir inversión y obligaciones». */
    async conmutarInversionYObligaciones(control: { checked: boolean }): Promise<void> {
        this.gasto.incluirTodo = control.checked;
        await this.recargarGasto(ID_CONMUTADOR);
    }
}

/** Lo que `window.appUI` expone de esta vista: los controles del bloque «Gasto por categoría». */
export function puenteResumen(vista: VistaResumen): {
    resumenGastoMes: (valor: string) => Promise<void>;
    resumenGastoMover: (delta: number) => Promise<void>;
    resumenGastoPromedio: (valor: string) => Promise<void>;
    resumenGastoConmutar: (control: { checked: boolean }) => Promise<void>;
} {
    return {
        resumenGastoMes: valor => vista.elegirMesDeGasto(valor),
        resumenGastoMover: delta => vista.moverMesDeGasto(delta),
        resumenGastoPromedio: valor => vista.elegirPromedioDeGasto(valor),
        resumenGastoConmutar: control => vista.conmutarInversionYObligaciones(control),
    };
}
