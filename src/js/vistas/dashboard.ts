// Pestaña «Dashboard» (diseño «B»: clase con dependencias inyectadas).
//
// Solo lectura: seis consultas a la API y una pantalla; sus únicos enlaces
// (`navigate('prestamos')`, `navigate('tarjetas')`) son globales de `app.ts`, no
// manejadores de `appUI`. No usa `this` de ningún objeto mezclado ni `AppAPI`
// global: todo entra por el constructor, así que se puede probar en Node con
// dobles (`pruebas/js/vistas/dashboard.test.js`). El cuerpo es el de `ui.ts`;
// solo cambian los accesos a lo compartido (ver `division_de_ui_limpia.md`).

import type { ApiDe, Formato, Pantalla, Reloj, Vista } from '../ui/servicios';

import { escaparHtml } from '../nucleo/html.js';

type ApiDashboard = ApiDe<
    | 'obtenerCapital'
    | 'obtenerGastos'
    | 'obtenerIngresos'
    | 'obtenerIngresosInformales'
    | 'obtenerPrestamos'
    | 'obtenerTarjetas'
>;

/**
 * Lo único que esta vista lee de los certificados y de la bolsa del capital.
 * Rust devuelve el capital como JSON libre (`any`); el cálculo de la alerta lo
 * hace el núcleo y llega ya en el propio elemento.
 */
interface ElementoConAlerta {
    alerta_vencimiento?: boolean;
    alerta_msg?: string;
    banco?: string;
    emisor?: string;
}

export interface DependenciasDashboard {
    api: ApiDashboard;
    formato: Formato;
    pantalla: Pantalla;
    ahora: Reloj;
}

export class VistaDashboard implements Vista {
    constructor(private readonly dep: DependenciasDashboard) {}

    // --- RENDER: DASHBOARD ---
    async render(): Promise<void> {
        const { api, formato, pantalla, ahora } = this.dep;
        const capital = await api.obtenerCapital();
        const gastos = await api.obtenerGastos();
        const ingresos = await api.obtenerIngresos();
        const informales = await api.obtenerIngresosInformales();
        const tarjetas = await api.obtenerTarjetas();
        const prestamos = await api.obtenerPrestamos();

        const hoy = ahora();
        const mesAnioActual = "/" + (hoy.getMonth() + 1).toString().padStart(2, '0') + '/' + hoy.getFullYear();

        // 1. Patrimonio total
        // Los totales los suma el núcleo, en centavos enteros: una suma de
        // decimales aquí arrastraría ruido de coma flotante, y sería una
        // regla de dinero sin pruebas.
        const totales = capital?.totales || {};
        const totalCertificados = totales.certificados ?? 0;
        const totalBolsa = totales.bolsa ?? 0;
        const totalInmueble = totales.inmobiliario ?? 0;
        const totalVehiculo = totales.vehiculos ?? 0;
        const totalMaquinaria = totales.maquinaria ?? 0;
        const patrimonioTotal = totales.patrimonio ?? 0;

        // 2. Gastos del mes
        const gastosMes = gastos.filter(g => g.fecha.endsWith(mesAnioActual));
        const totalGastado = gastosMes.reduce((sum, g) => sum + g.monto + g.costo_adicional, 0);

        // 3. Ingresos del mes
        const ingresosMes = ingresos.filter(i => i.fecha_emision.endsWith(mesAnioActual));
        const totalIngresosFormal = ingresosMes.reduce((sum, i) => sum + i.monto_total, 0);
        const totalRecibidoFormal = ingresosMes.reduce((sum, i) => sum + (i.monto_recibido || 0.0), 0);
        const totalRetenidoFormal = ingresosMes.reduce((sum, i) => sum + i.monto_retenido, 0);

        const informalesMes = informales.filter(inf => inf.fecha.endsWith(mesAnioActual));
        const totalInformalFacturado = informalesMes.reduce((sum, inf) => sum + inf.monto, 0);
        const totalInformalRecibido = informalesMes.reduce((sum, inf) => sum + (inf.monto_recibido || 0.0), 0);

        const totalIngresosMes = totalIngresosFormal + totalInformalFacturado;
        const totalRecibidoMes = totalRecibidoFormal + totalInformalRecibido;

        // 4. Alertas pendientes
        const alertas: { tipo: string; mensaje: string; nivel: string }[] = [];
        tarjetas.forEach(t => {
            if (t.alerta_corte) alertas.push({ tipo: 'Tarjeta (Corte)', mensaje: `${t.entidad} ${t.nombre_tarjeta}: ${t.dias_corte_msg}`, nivel: 'warning' });
            if (t.alerta_pago) alertas.push({ tipo: 'Tarjeta (Pago)', mensaje: `${t.entidad} ${t.nombre_tarjeta}: ${t.dias_pago_msg}`, nivel: 'danger' });
        });
        (capital.certificados || [] as ElementoConAlerta[]).forEach((c: ElementoConAlerta) => {
            if (c.alerta_vencimiento) alertas.push({ tipo: 'Certificado', mensaje: `Banco: ${c.banco} - ${c.alerta_msg}`, nivel: 'info' });
        });
        (capital.bolsa || [] as ElementoConAlerta[]).forEach((b: ElementoConAlerta) => {
            if (b.alerta_vencimiento) alertas.push({ tipo: 'Bolsa', mensaje: `Emisor: ${b.emisor} - ${b.alerta_msg}`, nivel: 'info' });
        });
        prestamos.forEach(p => {
            if (p.alerta_pago) alertas.push({ tipo: 'Deuda / Préstamo', mensaje: `${p.institucion_financiera} (${p.tipo_prestamo}): ${p.dias_pago_msg}`, nivel: 'danger' });
        });

        let htmlAlertas = '';
        if (alertas.length > 0) {
            htmlAlertas = `
                <div class="alerts-section">
                    <h3 style="font-size: 1rem; font-family: var(--font-heading); margin-bottom: 0.6rem; display: flex; align-items: center; gap: 0.4rem;">
                        ⚠️ Recordatorios del Sistema <span class="badge danger" style="padding: 1px 6px;">${alertas.length}</span>
                    </h3>
                    <div style="display: grid; grid-template-columns: repeat(2, 1fr); gap: 0.5rem;">
                        ${alertas.map(a => `
                            <div class="alert-banner ${escaparHtml(a.nivel)}">
                                <span>${a.nivel === 'danger' ? '🚨' : a.nivel === 'warning' ? '⚠️' : 'ℹ️'}</span>
                                <div><strong>[${escaparHtml(a.tipo)}]</strong> ${escaparHtml(a.mensaje)}</div>
                            </div>
                        `).join('')}
                    </div>
                </div>
            `;
        }

        pantalla.contenido.innerHTML = `
            <div class="section-title">
                <h1>Dashboard General</h1>
                <span class="subtitle">Análisis resumido en tiempo real</span>
            </div>

            ${htmlAlertas}

            <div class="kpi-row">
                <div class="kpi-box" style="border-left: 4px solid var(--accent-primary);">
                    <div>
                        <div style="font-size: 0.8rem; color: var(--text-secondary); text-transform: uppercase; letter-spacing: 0.05em;">Patrimonio Total</div>
                        <div class="amount" style="font-size: 1.6rem; margin-top: 0.2rem;">DOP ${formato.importe(patrimonioTotal)}</div>
                    </div>
                    <span style="font-size: 2.2rem;">🏢</span>
                </div>
                
                <div class="kpi-box" style="border-left: 4px solid var(--color-success);">
                    <div>
                        <div style="font-size: 0.8rem; color: var(--text-secondary); text-transform: uppercase; letter-spacing: 0.05em;">Ingresos del Mes (Cobrado)</div>
                        <div class="amount income" style="font-size: 1.6rem; margin-top: 0.2rem;">DOP ${formato.importe(totalRecibidoMes)}</div>
                        <div style="font-size: 0.7rem; color: var(--text-muted); margin-top: 0.3rem;">
                            Facturado: DOP ${formato.importe(totalIngresosMes)} | Retenido: DOP ${formato.importe(totalRetenidoFormal)}
                        </div>
                    </div>
                    <span style="font-size: 2.2rem;">📈</span>
                </div>

                <div class="kpi-box" style="border-left: 4px solid var(--color-danger);">
                    <div>
                        <div style="font-size: 0.8rem; color: var(--text-secondary); text-transform: uppercase; letter-spacing: 0.05em;">Gastos del Mes</div>
                        <div class="amount expense" style="font-size: 1.6rem; margin-top: 0.2rem;">DOP ${formato.importe(totalGastado)}</div>
                        <div style="font-size: 0.7rem; color: var(--text-muted); margin-top: 0.3rem;">
                            * Incluye comisiones de transferencias
                        </div>
                    </div>
                    <span style="font-size: 2.2rem;">📉</span>
                </div>
            </div>

            <div class="grid-2">
                <div class="card">
                    <h3 style="font-family: var(--font-heading); font-size: 1.1rem; margin-bottom: 1rem; display:flex; justify-content:space-between;">
                        Tarjetas de Crédito Destacadas
                        <button onclick="navigate('tarjetas')" class="btn btn-secondary" style="padding: 0.25rem 0.6rem; font-size: 0.75rem;">Detalles</button>
                    </h3>
                    ${tarjetas.length > 0 ? `
                        <div style="display:flex; flex-direction:column; gap:0.6rem;">
                            ${tarjetas.slice(0, 3).map(t => `
                                <div style="background: rgba(255,255,255,0.01); border: 1px solid var(--border-color); padding: 0.75rem; border-radius: var(--radius-sm); display:flex; justify-content:space-between; font-size:0.85rem; align-items: center;">
                                    <div><strong>${escaparHtml(t.entidad)}</strong> - ${escaparHtml(t.nombre_tarjeta)}</div>
                                    <div style="display:flex; flex-direction:column; align-items:flex-end;">
                                        <span style="font-weight:bold; color:var(--accent-primary);">DOP ${formato.importe(t.balance_pesos)}</span>
                                        <span style="font-size:0.75rem; color:#10b981; font-weight:bold;">USD ${formato.importe(t.balance_dolares)}</span>
                                    </div>
                                </div>
                            `).join('')}
                        </div>
                    ` : `
                        <p style="color: var(--text-muted); font-size:0.85rem;">No hay tarjetas registradas.</p>
                    `}
                </div>

                <div class="card">
                    <h3 style="font-family: var(--font-heading); font-size: 1.1rem; margin-bottom: 1rem; display:flex; justify-content:space-between;">
                        Deudas por Vencer
                        <button onclick="navigate('prestamos')" class="btn btn-secondary" style="padding: 0.25rem 0.6rem; font-size: 0.75rem;">Deudas</button>
                    </h3>
                    ${prestamos.length > 0 ? `
                        <div style="display:flex; flex-direction:column; gap:0.6rem;">
                            ${prestamos.slice(0, 3).map(p => `
                                <div style="background: rgba(255,255,255,0.01); border: 1px solid var(--border-color); padding: 0.75rem; border-radius: var(--radius-sm); display:flex; justify-content:space-between; font-size:0.85rem;">
                                    <div><strong style="text-transform:capitalize;">${escaparHtml(p.tipo_prestamo)}</strong> (${escaparHtml(p.institucion_financiera)})</div>
                                    <span class="amount expense">DOP ${formato.importe(p.monto_cuota)} / mes</span>
                                </div>
                            `).join('')}
                        </div>
                    ` : `
                        <p style="color: var(--text-muted); font-size:0.85rem;">No hay préstamos registrados.</p>
                    `}
                </div>
            </div>
        `;
    }
}

/** Esta vista no tiene manejadores en línea: no cuelga nada de `appUI`. */
export function puenteDashboard(_vista: VistaDashboard): Record<string, never> {
    return {};
}
