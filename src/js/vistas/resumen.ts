// Pestaña «Resumen» (diseño «B»: clase con dependencias inyectadas).
//
// Solo lectura: siete consultas a la API y una pantalla; no tiene manejadores. No
// usa `this` de ningún objeto mezclado ni `AppAPI` global: todo entra por el
// constructor, así que se puede probar en Node con dobles
// (`pruebas/js/vistas/resumen.test.js`). El cuerpo es el de `ui.ts`; solo cambian
// los accesos a lo compartido (ver `division_de_ui_limpia.md`).

import type { ApiDe, Formato, Pantalla, Referencias, Reloj, Vista } from '../ui/servicios';

import { escaparHtml } from '../nucleo/html.js';

type ApiResumen = ApiDe<
    | 'obtenerCapital'
    | 'obtenerIngresos'
    | 'obtenerIngresosInformales'
    | 'obtenerGastos'
    | 'obtenerPrestamos'
    | 'obtenerSuscripciones'
    | 'obtenerTarjetas'
>;

export interface DependenciasResumen {
    api: ApiResumen;
    formato: Formato;
    pantalla: Pantalla;
    ahora: Reloj;
    referencias: Referencias;
}

export class VistaResumen implements Vista {
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
        const suscripcionesPorDivisa = new Map<string, number>();
        for (const s of suscripciones) {
            const divisa = s.divisa || 'DOP';
            const alMes = s.frecuencia === 'mensual' ? s.monto : (s.monto / 12.0);
            suscripcionesPorDivisa.set(divisa, (suscripcionesPorDivisa.get(divisa) ?? 0) + alMes);
        }
        const cuotaSuscripciones = suscripcionesPorDivisa.get('DOP') ?? 0;
        const suscripcionesOtrasDivisas = [...suscripcionesPorDivisa.entries()].filter(([d]) => d !== 'DOP').sort(([x], [y]) => x.localeCompare(y));
        const cargaFija = cuotaPrestamos + cuotaSuscripciones;

        // 4. Balance del Mes (basado en monto cobrado/recibido)
        const ingresosMesFormalesCobrados = ingresos.filter(i => i.fecha_emision.endsWith(mesAnioActual)).reduce((sum, i) => sum + (i.monto_recibido || 0.0), 0);
        const ingresosMesInformalesCobrados = informales.filter(inf => inf.fecha.endsWith(mesAnioActual)).reduce((sum, inf) => sum + (inf.monto_recibido || 0.0), 0);
        const ingresosCobradosTotales = ingresosMesFormalesCobrados + ingresosMesInformalesCobrados;

        const ingresosFacturadosFormales = ingresos.filter(i => i.fecha_emision.endsWith(mesAnioActual)).reduce((sum, i) => sum + i.monto_total, 0);
        const ingresosFacturadosInformales = informales.filter(inf => inf.fecha.endsWith(mesAnioActual)).reduce((sum, inf) => sum + inf.monto, 0);
        const ingresosFacturadosTotales = ingresosFacturadosFormales + ingresosFacturadosInformales;

        const gastosTotales = gastos.filter(g => g.fecha.endsWith(mesAnioActual)).reduce((sum, g) => sum + g.monto + g.costo_adicional, 0);
        const balanceNeto = ingresosCobradosTotales - gastosTotales;

        pantalla.contenido.innerHTML = `
            <div class="section-title">
                <h1>Resumen Ejecutivo</h1>
                <span class="subtitle">Cálculos de patrimonio, deuda y flujos recurrentes</span>
            </div>

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
                        <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); padding:0.5rem; border-radius:4px;">
                            <span>Suscripciones Recurrentes</span>
                            <strong>DOP ${formato.importe(cuotaSuscripciones)}</strong>
                        </div>
                        ${suscripcionesOtrasDivisas.map(([d, m]) => `
                        <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); padding:0.5rem; border-radius:4px;">
                            <span>Suscripciones en ${escaparHtml(d)}</span>
                            <strong>${escaparHtml(d)} ${formato.importe(m)}</strong>
                        </div>`).join('')}
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
        `;
    }
}

/** Esta vista no tiene manejadores en línea: no cuelga nada de `appUI`. */
export function puenteResumen(_vista: VistaResumen): Record<string, never> {
    return {};
}
