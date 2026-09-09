// --- MICHELITOS TAURI - RENDERIZADO DINÁMICO DE INTERFAZ (HTML DE ESCRITORIO) ---

class AppUI {
    constructor() {
        this.contentContainer = document.getElementById('app-content');
        this.notifContainer = document.getElementById('notification-container');
    }

    // --- TOASTS / NOTIFICACIONES ---
    showToast(message, type = 'success') {
        const toast = document.createElement('div');
        toast.className = `toast toast-${type}`;
        toast.innerHTML = `
            <span style="font-weight: bold;">${type === 'success' ? '✅' : '⚠️'}</span>
            <span>${message}</span>
        `;
        this.notifContainer.appendChild(toast);
        
        setTimeout(() => toast.classList.add('show'), 100);
        
        setTimeout(() => {
            toast.classList.remove('show');
            setTimeout(() => toast.remove(), 400);
        }, 4000);
    }

    // --- ENRUTADOR DE VISTAS ---
    async render(route) {
        this.contentContainer.innerHTML = '<p style="color: var(--text-muted); text-align:center; padding: 2rem;">Cargando módulo nativo...</p>';
        
        try {
            switch (route) {
                case 'dashboard':
                    await this.renderDashboard();
                    break;
                case 'ingresos':
                    await this.renderIngresos();
                    break;
                case 'gastos':
                    await this.renderGastos();
                    break;
                case 'tarjetas':
                    await this.renderTarjetas();
                    break;
                case 'cuentas':
                    await this.renderCuentas();
                    break;
                case 'efectivo':
                    await this.renderEfectivo();
                    break;
                case 'suscripciones':
                    await this.renderSuscripciones();
                    break;
                case 'capital':
                    await this.renderCapital();
                    break;
                case 'prestamos':
                    await this.renderPrestamos();
                    break;
                case 'resumen':
                    await this.renderResumen();
                    break;
                case 'ajustes':
                    await this.renderAjustes();
                    break;
                default:
                    await this.renderDashboard();
            }
        } catch (err) {
            this.contentContainer.innerHTML = `
                <div class="card" style="border-left: 4px solid var(--color-danger);">
                    <h3 style="color: var(--color-danger); margin-bottom: 0.5rem;">Error al renderizar el módulo</h3>
                    <p style="font-size: 0.9rem;">${err.toString()}</p>
                </div>
            `;
        }
    }

    // --- RENDER: DASHBOARD ---
    async renderDashboard() {
        const capital = await AppAPI.obtenerCapital();
        const gastos = await AppAPI.obtenerGastos();
        const ingresos = await AppAPI.obtenerIngresos();
        const informales = await AppAPI.obtenerIngresosInformales();
        const tarjetas = await AppAPI.obtenerTarjetas();
        const prestamos = await AppAPI.obtenerPrestamos();

        const hoy = new Date();
        const mesAnioActual = "/" + (hoy.getMonth() + 1).toString().padStart(2, '0') + '/' + hoy.getFullYear();

        // 1. Patrimonio total
        const totalCertificados = (capital?.certificados || []).reduce((sum, c) => sum + Number(c.monto || 0), 0);
        const totalBolsa = (capital?.bolsa || []).reduce((sum, b) => sum + Number(b.monto || 0), 0);
        const totalInmueble = (capital?.propiedades?.inmobiliario || []).reduce((sum, p) => sum + Number(p.valor_estimado || 0), 0);
        const totalVehiculo = (capital?.propiedades?.vehiculos || []).reduce((sum, p) => sum + Number(p.valor_estimado || 0), 0);
        const totalMaquinaria = (capital?.propiedades?.maquinaria || []).reduce((sum, p) => sum + Number(p.valor_estimado || 0), 0);
        const patrimonioTotal = totalCertificados + totalBolsa + totalInmueble + totalVehiculo + totalMaquinaria;

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
        const alertas = [];
        tarjetas.forEach(t => {
            if (t.alerta_corte) alertas.push({ tipo: 'Tarjeta (Corte)', mensaje: `${t.entidad} ${t.nombre_tarjeta}: ${t.dias_corte_msg}`, nivel: 'warning' });
            if (t.alerta_pago) alertas.push({ tipo: 'Tarjeta (Pago)', mensaje: `${t.entidad} ${t.nombre_tarjeta}: ${t.dias_pago_msg}`, nivel: 'danger' });
        });
        (capital.certificados || []).forEach(c => {
            if (c.alerta_vencimiento) alertas.push({ tipo: 'Certificado', mensaje: `Banco: ${c.banco} - ${c.alerta_msg}`, nivel: 'info' });
        });
        (capital.bolsa || []).forEach(b => {
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
                            <div class="alert-banner ${a.nivel}">
                                <span>${a.nivel === 'danger' ? '🚨' : a.nivel === 'warning' ? '⚠️' : 'ℹ️'}</span>
                                <div><strong>[${a.tipo}]</strong> ${a.mensaje}</div>
                            </div>
                        `).join('')}
                    </div>
                </div>
            `;
        }

        this.contentContainer.innerHTML = `
            <div class="section-title">
                <h1>Dashboard General</h1>
                <span class="subtitle">Análisis resumido en tiempo real</span>
            </div>

            ${htmlAlertas}

            <div class="kpi-row">
                <div class="kpi-box" style="border-left: 4px solid var(--accent-primary);">
                    <div>
                        <div style="font-size: 0.8rem; color: var(--text-secondary); text-transform: uppercase; letter-spacing: 0.05em;">Patrimonio Total</div>
                        <div class="amount" style="font-size: 1.6rem; margin-top: 0.2rem;">DOP ${this.formatMoney(patrimonioTotal)}</div>
                    </div>
                    <span style="font-size: 2.2rem;">🏢</span>
                </div>
                
                <div class="kpi-box" style="border-left: 4px solid var(--color-success);">
                    <div>
                        <div style="font-size: 0.8rem; color: var(--text-secondary); text-transform: uppercase; letter-spacing: 0.05em;">Ingresos del Mes (Cobrado)</div>
                        <div class="amount income" style="font-size: 1.6rem; margin-top: 0.2rem;">DOP ${this.formatMoney(totalRecibidoMes)}</div>
                        <div style="font-size: 0.7rem; color: var(--text-muted); margin-top: 0.3rem;">
                            Facturado: DOP ${this.formatMoney(totalIngresosMes)} | Retenido: DOP ${this.formatMoney(totalRetenidoFormal)}
                        </div>
                    </div>
                    <span style="font-size: 2.2rem;">📈</span>
                </div>

                <div class="kpi-box" style="border-left: 4px solid var(--color-danger);">
                    <div>
                        <div style="font-size: 0.8rem; color: var(--text-secondary); text-transform: uppercase; letter-spacing: 0.05em;">Gastos del Mes</div>
                        <div class="amount expense" style="font-size: 1.6rem; margin-top: 0.2rem;">DOP ${this.formatMoney(totalGastado)}</div>
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
                                    <div><strong>${t.entidad}</strong> - ${t.nombre_tarjeta}</div>
                                    <div style="display:flex; flex-direction:column; align-items:flex-end;">
                                        <span style="font-weight:bold; color:var(--accent-primary);">DOP ${this.formatMoney(t.balance_pesos)}</span>
                                        <span style="font-size:0.75rem; color:#10b981; font-weight:bold;">USD ${this.formatMoney(t.balance_dolares)}</span>
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
                                    <div><strong style="text-transform:capitalize;">${p.tipo_prestamo}</strong> (${p.institucion_financiera})</div>
                                    <span class="amount expense">DOP ${this.formatMoney(p.monto_cuota)} / mes</span>
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

    async renderIngresos() {
        const ingresos = await AppAPI.obtenerIngresos();
        const informales = await AppAPI.obtenerIngresosInformales();
        const clientes = await AppAPI.obtenerClientes();

        const hoy = new Date();
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

        this.contentContainer.innerHTML = `
            <div class="section-title">
                <h1>Ingresos</h1>
                <span class="subtitle">Facturas formales y registro informal de flujos</span>
            </div>

            <div class="responsive-split-grid">
                <!-- Formulario -->
                <div class="card" style="height: fit-content;">
                    <div style="display: flex; gap: 0.4rem; margin-bottom: 1.2rem; background: rgba(255,255,255,0.02); padding: 3px; border-radius: var(--radius-sm); border: 1px solid var(--border-color);">
                        <button onclick="document.getElementById('tauri-form-formal').style.display='block'; document.getElementById('tauri-form-informal').style.display='none'; this.className='btn'; document.getElementById('btn-tab-inf').className='btn btn-secondary';" id="btn-tab-for" class="btn" style="flex:1; padding: 0.4rem; font-size: 0.8rem;">📄 Formal</button>
                        <button onclick="document.getElementById('tauri-form-formal').style.display='none'; document.getElementById('tauri-form-informal').style.display='block'; this.className='btn'; document.getElementById('btn-tab-for').className='btn btn-secondary';" id="btn-tab-inf" class="btn btn-secondary" style="flex:1; padding: 0.4rem; font-size: 0.8rem; border:none;">💸 Informal</button>
                    </div>

                    <!-- FORMULARIO FORMAL -->
                    <div id="tauri-form-formal">
                        <form id="form-add-formal" onsubmit="appUI.handleAgregarIngreso(event)">
                            <div class="form-group">
                                <label for="num_fac">Número de Factura *</label>
                                <input type="text" id="num_fac" class="form-control" value="${nextInvoice}" required>
                            </div>
                            <div class="form-group">
                                <label for="fec_em">Fecha de Emisión *</label>
                                <input type="text" id="fec_em" class="form-control" value="${hoyStr}" placeholder="dd/mm/aaaa" required>
                            </div>
                            <div class="form-group">
                                <label for="cli_select">Cliente *</label>
                                <select id="cli_select" class="form-control" onchange="appUI.handleSelectCliente(this.value)" required>
                                    <option value="" disabled selected>Seleccione cliente...</option>
                                    ${clientes.map(c => `<option value="${c.id}" data-rnc="${c.rnc}" data-nombre="${c.nombre}">${c.nombre} (RNC: ${c.rnc})</option>`).join('')}
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
                                <input type="text" id="fecha_inf" class="form-control" value="${hoyStr}" placeholder="dd/mm/aaaa" required>
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
                                            const iEscaped = JSON.stringify(i).replace(/'/g, "&#39;").replace(/"/g, "&quot;");
                                            return `
                                                <tr>
                                                    <td><strong>${i.numero_factura}</strong></td>
                                                    <td>${i.cliente_nombre}<br><span style="font-size:0.75rem; color:var(--text-muted);">RNC: ${i.cliente_rnc}</span></td>
                                                    <td>${i.fecha_emision}</td>
                                                    <td class="amount">DOP ${this.formatMoney(i.monto_total)}</td>
                                                    <td class="amount expense">DOP ${this.formatMoney(i.monto_retenido)}</td>
                                                    <td><span class="badge ${i.estatus}">${i.estatus}</span></td>
                                                    <td>
                                                        <div style="display:flex; gap:0.3rem;">
                                                            ${i.estatus === 'emitida' ? `
                                                                <button onclick="appUI.abrirEdicionFormal('${iEscaped}')" class="btn btn-secondary" style="padding: 0.3rem 0.5rem; font-size:0.75rem; border:none; background:rgba(255,255,255,0.05);" title="Corregir factura">✏️</button>
                                                                <button onclick="appUI.abrirCobroFormal(${i.id}, ${i.monto_total - i.monto_retenido})" class="btn" style="padding: 0.3rem 0.5rem; font-size:0.75rem;">💵 Cobrar</button>
                                                            ` : `
                                                                <span style="font-size:0.75rem; color:var(--text-muted); font-style:italic;">Dep: ${i.institucion_deposito}</span>
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
                                                <td><strong>${inf.descripcion}</strong></td>
                                                <td>${inf.fecha}</td>
                                                <td class="amount income">DOP ${this.formatMoney(inf.monto)}</td>
                                                <td><span class="badge ${inf.estatus === 'pagado' ? 'pagada' : 'emitida'}">${inf.estatus}</span></td>
                                                <td>
                                                    ${inf.estatus === 'pagado' ? `
                                                        <span style="font-size:0.75rem; color:var(--text-muted);">En ${inf.institucion_deposito} el ${inf.fecha_pago}</span>
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


    // --- RENDER: GASTOS ---
    async renderGastos() {
        const gastos = await AppAPI.obtenerGastos();
        const categorias = await AppAPI.obtenerCategorias();
        const tarjetas = await AppAPI.obtenerTarjetas();
        const cuentas = await AppAPI.obtenerCuentas();

        const hoy = new Date();
        const hoyStr = hoy.getDate().toString().padStart(2, '0') + '/' + (hoy.getMonth() + 1).toString().padStart(2, '0') + '/' + hoy.getFullYear();
        
        // Extraer todos los meses/años disponibles con transacciones
        const mesesDisponibles = [...new Set(gastos.map(g => {
            const parts = g.fecha.split('/');
            if (parts.length === 3) {
                return `${parts[1]}/${parts[2]}`; // Formato: "mm/yyyy"
            }
            return null;
        }).filter(Boolean))].sort((a, b) => {
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
        const categoryTotals = {};
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

        this.contentContainer.innerHTML = `
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
                                <input type="text" id="gas_fec" class="form-control" value="${hoyStr}" placeholder="dd/mm/aaaa" required>
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
                                    ${categorias.map(c => `<option value="${c.id}">${c.nombre}</option>`).join('')}
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
                                ${tarjetas.map(t => `<option value="${t.id}">${t.entidad} - ${t.nombre_tarjeta}</option>`).join('')}
                            </select>
                        </div>

                        <!-- Selector cuenta de ahorro -->
                        <div id="gas_cuenta_container" class="form-group" style="display:none;">
                            <label for="gas_cue">Cuenta de Ahorro *</label>
                            <select id="gas_cue" class="form-control" onchange="appUI.actualizarConversionGasto()">
                                <option value="" disabled selected>Seleccione cuenta...</option>
                                ${cuentas.map(c => `<option value="${c.id}" data-divisa="${c.divisa}">${c.nombre} (${c.divisa}) - Bal: ${c.divisa} ${this.formatMoney(c.balance_actual)}</option>`).join('')}
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
                                    <option value="${m}" ${m === mSelected ? 'selected' : ''}>${this.formatMonthYearStr(m)}</option>
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
                                            <div class="amount" style="font-size:1.1rem; margin-top:0.2rem;">DOP ${this.formatMoney(totalNetoDop)}</div>
                                        </div>
                                        <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.6rem; border-radius:var(--radius-sm);">
                                            <span style="font-size:0.7rem; color:var(--text-secondary);">Comisiones</span>
                                            <div class="amount expense" style="font-size:1.1rem; margin-top:0.2rem;">DOP ${this.formatMoney(totalComisionesDop)}</div>
                                        </div>
                                        <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.6rem; border-radius:var(--radius-sm);">
                                            <span style="font-size:0.7rem; color:var(--text-secondary);">Total Debitado</span>
                                            <div class="amount" style="font-size:1.1rem; margin-top:0.2rem;">DOP ${this.formatMoney(totalNetoDop + totalComisionesDop)}</div>
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
                                            <div class="amount" style="font-size:1.1rem; margin-top:0.2rem; color:#60a5fa;">USD ${this.formatMoney(totalNetoUsd)}</div>
                                        </div>
                                        <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.6rem; border-radius:var(--radius-sm);">
                                            <span style="font-size:0.7rem; color:var(--text-secondary);">Comisiones</span>
                                            <div class="amount expense" style="font-size:1.1rem; margin-top:0.2rem;">USD ${this.formatMoney(totalComisionesUsd)}</div>
                                        </div>
                                        <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.6rem; border-radius:var(--radius-sm);">
                                            <span style="font-size:0.7rem; color:var(--text-secondary);">Total Debitado</span>
                                            <div class="amount" style="font-size:1.1rem; margin-top:0.2rem; color:#60a5fa; font-weight:bold;">USD ${this.formatMoney(totalNetoUsd + totalComisionesUsd)}</div>
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
                                                        <span style="font-weight:600;">${cat} <span style="font-size:0.7rem; color:var(--text-muted);">(${divisa})</span></span>
                                                        <span style="font-family:var(--font-heading); font-weight:700;">
                                                            ${divisa} ${this.formatMoney(total)} 
                                                            <span style="font-size:0.7rem; color:var(--text-secondary); font-weight:normal; margin-left:0.25rem;">(${pct.toFixed(1)}%)</span>
                                                        </span>
                                                    </div>
                                                    <div style="background:rgba(255,255,255,0.03); height:6px; border-radius:3px; overflow:hidden; border:1px solid var(--border-color);">
                                                        <div style="background:${barColor}; width:${pct}%; height:100%; border-radius:3px;"></div>
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
                                                    <strong>${g.descripcion}</strong>
                                                    ${pendiente ? `<button onclick="appUI.abrirLiquidacionConsumo(${g.id}, ${g.monto}, '${g.divisa}', '${String(g.descripcion).replace(/'/g, "&apos;")}')" class="btn" style="margin-left:0.4rem; padding:0.1rem 0.4rem; font-size:0.65rem; background:rgba(255,193,7,0.15); border:1px solid rgba(255,193,7,0.4); color:#ffc107;" title="El emisor aún no ha fijado el importe en pesos">⏳ Liquidar</button>` : ''}
                                                    ${liquidado ? `<span style="margin-left:0.4rem; font-size:0.65rem; color:var(--text-muted);" title="Tasa aplicada por el emisor">@ ${Number(g.tasa_conversion).toFixed(4)}</span>` : ''}
                                                </td>
                                                <td>${g.categoria_nombre}</td>
                                                <td>${g.fecha}</td>
                                                <td style="text-transform:capitalize;">${g.metodo_pago}</td>
                                                <td class="amount">${g.divisa} ${this.formatMoney(g.monto)}</td>
                                                <td class="amount expense">${divisaFinal} ${this.formatMoney(g.costo_adicional)}</td>
                                                <td class="amount" style="font-weight:bold;">${divisaFinal} ${this.formatMoney(montoFinal + g.costo_adicional)}</td>
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

    toggleMetodoPago(val) {
        const tarjeta = document.getElementById('gas_tarjeta_container');
        const cuenta = document.getElementById('gas_cuenta_container');
        const lbtr = document.getElementById('gas_lbtr_container');
        
        if (tarjeta) tarjeta.style.display = val === 'tarjeta' ? 'block' : 'none';
        if (cuenta) cuenta.style.display = val === 'transferencia' ? 'block' : 'none';
        if (lbtr) lbtr.style.display = val === 'transferencia' ? 'block' : 'none';
    }

    // --- RENDER: TARJETAS ---
    async renderTarjetas() {
        const tarjetas = await AppAPI.obtenerTarjetas();
        const cuentas = await AppAPI.obtenerCuentas();
        const bonificaciones = await AppAPI.obtenerBonificaciones();

        const hoy = new Date();
        const hoyStr = hoy.getDate().toString().padStart(2, '0') + '/' + (hoy.getMonth() + 1).toString().padStart(2, '0') + '/' + hoy.getFullYear();

        const diaActual = hoy.getDate();
        const mesActual = hoy.getMonth();
        const anioActual = hoy.getFullYear();

        let mejorTarjeta = null;
        let maxDiasRestantes = -1;

        tarjetas.forEach(t => {
            let diaCorte = t.fecha_corte;
            let fechaCorteEsteMes = new Date(anioActual, mesActual, diaCorte);
            let fechaProximoCorte;
            
            if (diaActual <= diaCorte) {
                fechaProximoCorte = fechaCorteEsteMes;
            } else {
                fechaProximoCorte = new Date(anioActual, mesActual + 1, diaCorte);
            }
            
            const diffMs = fechaProximoCorte - hoy;
            const diffDays = Math.ceil(diffMs / (1000 * 60 * 60 * 24));
            
            t.dias_para_corte = diffDays;
            
            if (diffDays > maxDiasRestantes) {
                maxDiasRestantes = diffDays;
                mejorTarjeta = t;
            }
        });

        this.contentContainer.innerHTML = `
            <div class="section-title" style="display:flex; justify-content:space-between; align-items:center; gap:1rem;">
                <div>
                    <h1>Tarjetas de Crédito</h1>
                    <span class="subtitle">Monitoreo de consumos, abonos y alertas de corte (Multidivisa)</span>
                </div>
                <button onclick="navigate('ajustes')" class="btn" style="padding: 0.5rem 1rem; font-size: 0.85rem;">➕ Registrar Tarjeta</button>
            </div>

            <div style="display:flex; flex-direction:column; gap:2rem; width:100%;">
                <!-- Tarjeta Recomendada Hoy -->
                ${mejorTarjeta ? `
                    <div class="card" style="border-left: 5px solid #10b981; background: rgba(16, 185, 129, 0.02); padding: 1rem 1.2rem; width:100%;">
                        <div style="display:flex; justify-content:space-between; align-items:center; gap: 1rem;">
                            <div>
                                <span style="font-size:0.75rem; text-transform:uppercase; color:#10b981; font-weight:bold; letter-spacing:0.05em; display:block; margin-bottom:0.15rem;">💡 Tarjeta Recomendada Hoy</span>
                                <strong style="font-size:1.05rem; font-family:var(--font-heading); display:block;">${mejorTarjeta.entidad} - ${mejorTarjeta.nombre_tarjeta}</strong>
                                <p style="font-size:0.75rem; color:var(--text-secondary); margin-top:0.2rem; line-height:1.3;">
                                    Próximo corte en <strong>${maxDiasRestantes} días</strong> (Día ${mejorTarjeta.fecha_corte}). Usarla hoy te otorga la mayor cantidad de días de financiamiento sin intereses.
                                </p>
                            </div>
                            <span style="font-size: 1.8rem; opacity: 0.85;">💡</span>
                        </div>
                    </div>
                ` : ''}

                <!-- Grilla de Tarjetas (Ancho Completo) -->
                ${tarjetas.length > 0 ? `
                    <div style="display:grid; grid-template-columns: repeat(auto-fill, minmax(340px, 1fr)); gap:1.5rem; width:100%;">
                        ${tarjetas.map(t => {
                            const limiteTotalDop = t.limite_efectivo_pesos + t.limite_sobregiro_pesos;
                            const disponibleDop = t.disponible_pesos;
                            const pctDop = limiteTotalDop > 0 ? Math.min((t.balance_pesos / limiteTotalDop) * 100, 100) : 0;

                            const limiteTotalUsd = t.limite_efectivo_dolares + t.limite_sobregiro_dolares;
                            const disponibleUsd = t.disponible_dolares;
                            const pctUsd = limiteTotalUsd > 0 ? Math.min((t.balance_dolares / limiteTotalUsd) * 100, 100) : 0;

                            const tarjetaAlDia = (t.balance_corte_pesos <= 0 && t.balance_corte_dolares <= 0);
                            const tEscaped = JSON.stringify(t).replace(/'/g, "&#39;").replace(/"/g, "&quot;");

                            return `
                                <div class="card" style="display:flex; flex-direction:column; gap:0.8rem;">
                                    <div style="display:flex; justify-content:space-between; border-bottom:1px solid var(--border-color); padding-bottom:0.5rem; align-items:center;">
                                        <div>
                                            <strong>${t.entidad}</strong>
                                            <div style="font-size:0.75rem; color:var(--text-muted);">${t.nombre_tarjeta}</div>
                                        </div>
                                        <div>
                                            ${tarjetaAlDia ? `
                                                <span class="badge pagada" style="font-size:0.7rem;">✔️ Tarjeta al día</span>
                                            ` : `
                                                <span class="badge" style="font-size:0.7rem; background:rgba(255, 69, 58, 0.15); color:#ff453a;">Corte: DOP ${this.formatMoney(t.balance_corte_pesos)} / USD ${this.formatMoney(t.balance_corte_dolares)}</span>
                                            `}
                                        </div>
                                    </div>

                                    <!-- Pesos Section -->
                                    <div>
                                        <div style="display:flex; justify-content:space-between; font-size:0.75rem; color:var(--text-secondary); margin-bottom:0.2rem;">
                                            <span>🇩🇴 DOP (Uso: ${pctDop.toFixed(1)}%)</span>
                                            <span>Uso: DOP ${this.formatMoney(t.balance_pesos)}</span>
                                        </div>
                                        <div style="width:100%; height:6px; background:rgba(255,255,255,0.05); border-radius:3px; overflow:hidden; margin-bottom:0.3rem;">
                                            <div style="width: ${pctDop}%; height:100%; background: ${pctDop > 85 ? '#ff453a' : 'var(--accent-primary)'};"></div>
                                        </div>
                                        <div style="display:flex; justify-content:space-between; font-size:0.75rem; color:var(--text-muted);">
                                            <span>Disp: DOP ${this.formatMoney(disponibleDop)}</span>
                                            <span title="${t.limite_ajustado_pesos != null ? `Aprobado por el banco: DOP ${this.formatMoney(t.limite_pesos)}` : ''}">${t.limite_ajustado_pesos != null ? '🔒 ' : ''}Lím: DOP ${this.formatMoney(limiteTotalDop)}</span>
                                        </div>
                                    </div>

                                    <!-- Dólares Section -->
                                    <div>
                                        <div style="display:flex; justify-content:space-between; font-size:0.75rem; color:var(--text-secondary); margin-bottom:0.2rem;">
                                            <span>🇺🇸 USD (Uso: ${pctUsd.toFixed(1)}%)</span>
                                            <span>Uso: USD ${this.formatMoney(t.balance_dolares)}</span>
                                        </div>
                                        <div style="width:100%; height:6px; background:rgba(255,255,255,0.05); border-radius:3px; overflow:hidden; margin-bottom:0.3rem;">
                                            <div style="width: ${pctUsd}%; height:100%; background: ${pctUsd > 85 ? '#ff453a' : '#10b981'};"></div>
                                        </div>
                                        <div style="display:flex; justify-content:space-between; font-size:0.75rem; color:var(--text-muted);">
                                            <span>Disp: USD ${this.formatMoney(disponibleUsd)}</span>
                                            <span title="${t.limite_ajustado_dolares != null ? `Aprobado por el banco: USD ${this.formatMoney(t.limite_dolares)}` : ''}">${t.limite_ajustado_dolares != null ? '🔒 ' : ''}Lím: USD ${this.formatMoney(limiteTotalUsd)}</span>
                                        </div>
                                    </div>

                                    <!-- Alertas -->
                                    <div style="display:flex; flex-direction:column; gap:0.3rem;">
                                        <div class="alert-banner ${t.alerta_corte ? 'warning' : 'info'}" style="padding:0.4rem 0.6rem; font-size:0.75rem;">
                                            <span>📅</span>
                                            <div>${t.dias_corte_msg}</div>
                                        </div>
                                        <div class="alert-banner ${t.alerta_pago ? 'danger' : 'info'}" style="padding:0.4rem 0.6rem; font-size:0.75rem;">
                                            <span>🚨</span>
                                            <div>${t.dias_pago_msg}</div>
                                        </div>
                                    </div>

                                    <!-- Abono rápido -->
                                    <form onsubmit="appUI.handleAbonoTarjeta(event, ${t.id})" style="display:flex; flex-direction:column; gap:0.5rem; border-top:1px dashed var(--border-color); padding-top:0.8rem; margin-top:0.3rem;">
                                        <div style="display:flex; gap:0.4rem; align-items:flex-end; width:100%;">
                                            <div style="flex:1.2;">
                                                <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Fecha</label>
                                                <input type="text" id="pag_fecha_${t.id}" value="${hoyStr}" placeholder="dd/mm/aaaa" class="form-control" style="padding:0.4rem; font-size:0.75rem;" required>
                                            </div>
                                            <div style="flex:1;">
                                                <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Divisa</label>
                                                <select id="pag_div_${t.id}" class="form-control" style="padding:0.4rem; font-size:0.75rem;" onchange="appUI.aplicarTipoAbono(${t.id}, ${t.balance_corte_pesos}, ${t.balance_corte_dolares}, ${t.balance_pesos}, ${t.balance_dolares})">
                                                    <option value="DOP">DOP</option>
                                                    <option value="USD">USD</option>
                                                </select>
                                            </div>
                                            <div style="flex:1.2;">
                                                <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Monto</label>
                                                <input type="number" step="0.01" id="pag_monto_${t.id}" placeholder="0.00" class="form-control" style="padding:0.4rem; font-size:0.75rem;" required>
                                            </div>
                                        </div>
                                        <div style="width:100%;">
                                            <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Tipo de abono</label>
                                            <select id="pag_tipo_${t.id}" class="form-control" style="padding:0.4rem; font-size:0.75rem; width:100%;" onchange="appUI.aplicarTipoAbono(${t.id}, ${t.balance_corte_pesos}, ${t.balance_corte_dolares}, ${t.balance_pesos}, ${t.balance_dolares})">
                                                <option value="personalizado">Personalizado</option>
                                                <option value="corte">Pago al corte</option>
                                                <option value="actual">Saldo del balance actual</option>
                                            </select>
                                        </div>
                                        <div style="display:flex; gap:0.4rem; align-items:flex-end; width:100%;">
                                            <div style="flex:2;">
                                                <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Cuenta Débito (Opcional)</label>
                                                <select id="pag_cuenta_${t.id}" class="form-control" style="padding:0.4rem; font-size:0.75rem; width:100%;">
                                                    <option value="">-- Ninguna (Efectivo/Otro) --</option>
                                                    ${cuentas.map(c => `<option value="${c.id}">${c.nombre} (${c.divisa}) - Bal: ${c.divisa} ${this.formatMoney(c.balance_actual)}</option>`).join('')}
                                                </select>
                                            </div>
                                            <div style="flex:1;">
                                                <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Tasa Cambio</label>
                                                <input type="number" step="0.01" id="pag_tasa_${t.id}" value="0.00" placeholder="0.00" class="form-control" style="padding:0.4rem; font-size:0.75rem;">
                                            </div>
                                            <button type="submit" class="btn" style="padding:0.4rem 0.6rem; font-size:0.75rem; height:fit-content; background: linear-gradient(135deg, #10b981, #059669); color:white; flex:1;">Abonar</button>
                                        </div>
                                    </form>

                                    <!-- Configurar límites -->
                                    <button onclick="appUI.abrirEdicionLimitesTarjeta('${tEscaped}')" class="btn" style="padding:0.3rem; font-size:0.75rem; background:rgba(255,255,255,0.02); border:1px solid var(--border-color); color:var(--text-secondary); width:100%;">⚙️ Configurar Límites / Corte</button>
                                </div>
                            `;
                        }).join('')}
                    </div>
                ` : `
                    <p style="color:var(--text-muted); text-align:center; padding:2rem; background:var(--bg-surface); border:1px solid var(--border-color); border-radius:var(--radius-md);">No hay tarjetas registradas.</p>
                `}

                ${tarjetas.length > 0 ? `
                <div class="card" style="margin-top:1.2rem;">
                    <h3 style="font-family:var(--font-heading); margin-bottom:0.3rem;">🎁 Bonificaciones recibidas</h3>
                    <p style="font-size:0.75rem; color:var(--text-secondary); margin-bottom:1rem;">
                        Cashback, devoluciones promocionales y recompensas. Se registran como crédito aparte y reducen la deuda de la tarjeta, sin alterar el consumo que las generó.
                        Un mismo consumo puede recibir varias.
                    </p>

                    <form onsubmit="appUI.handleAgregarBonificacion(event)" style="display:flex; gap:0.5rem; flex-wrap:wrap; align-items:flex-end; margin-bottom:1rem;">
                        <div style="flex:1; min-width:110px;">
                            <label style="font-size:0.7rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Fecha</label>
                            <input type="text" id="bon_fecha" value="${hoyStr}" placeholder="dd/mm/aaaa" class="form-control" style="padding:0.4rem; font-size:0.78rem;" required>
                        </div>
                        <div style="flex:1.6; min-width:170px;">
                            <label style="font-size:0.7rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Tarjeta</label>
                            <select id="bon_tarjeta" class="form-control" style="padding:0.4rem; font-size:0.78rem;" required>
                                ${tarjetas.map(t => `<option value="${t.id}">${t.entidad} - ${t.nombre_tarjeta}</option>`).join('')}
                            </select>
                        </div>
                        <div style="flex:0.8; min-width:80px;">
                            <label style="font-size:0.7rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Divisa</label>
                            <select id="bon_divisa" class="form-control" style="padding:0.4rem; font-size:0.78rem;">
                                <option value="DOP">DOP</option><option value="USD">USD</option>
                            </select>
                        </div>
                        <div style="flex:1; min-width:100px;">
                            <label style="font-size:0.7rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Monto</label>
                            <input type="number" step="0.01" id="bon_monto" placeholder="0.00" class="form-control" style="padding:0.4rem; font-size:0.78rem;" required>
                        </div>
                        <div style="flex:2; min-width:190px;">
                            <label style="font-size:0.7rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Concepto</label>
                            <input type="text" id="bon_concepto" list="conceptos_bonificacion" placeholder="Cashback compra por internet..." class="form-control" style="padding:0.4rem; font-size:0.78rem;" required>
                            <datalist id="conceptos_bonificacion">
                                <option value="Cashback compra por internet"></option>
                                <option value="Cashback Personalizado"></option>
                                <option value="Cashback Promocional"></option>
                                <option value="Recompensas Qik Rebate"></option>
                                <option value="Devolución promocional"></option>
                            </datalist>
                        </div>
                        <button type="submit" class="btn" style="padding:0.45rem 0.9rem; font-size:0.78rem; background:linear-gradient(135deg, #10b981, #059669); color:white;">Registrar</button>
                    </form>

                    ${bonificaciones.length > 0 ? `
                        <div style="max-height:240px; overflow-y:auto;">
                        <table class="data-table" style="font-size:0.78rem;">
                            <thead><tr><th>Fecha</th><th>Tarjeta</th><th>Concepto</th><th style="text-align:right;">Monto</th><th></th></tr></thead>
                            <tbody>
                                ${bonificaciones.map(b => `
                                    <tr>
                                        <td>${b.fecha}</td>
                                        <td>${b.entidad} (${b.nombre_tarjeta})</td>
                                        <td>${b.concepto}</td>
                                        <td class="amount" style="text-align:right; color:var(--color-success);">+${b.divisa} ${this.formatMoney(b.monto)}</td>
                                        <td><button onclick="appUI.handleEliminarBonificacion(${b.id})" class="btn btn-danger" style="padding:0.2rem 0.4rem; font-size:0.7rem;">🗑️</button></td>
                                    </tr>
                                `).join('')}
                            </tbody>
                        </table>
                        </div>
                        <p style="font-size:0.75rem; color:var(--text-secondary); margin-top:0.6rem;">
                            Total acreditado: ${['DOP','USD'].map(d => {
                                const suma = bonificaciones.filter(b => b.divisa === d).reduce((s,b) => s + b.monto, 0);
                                return suma > 0 ? `<strong>${d} ${this.formatMoney(suma)}</strong>` : '';
                            }).filter(Boolean).join(' · ') || '—'}
                        </p>
                    ` : `
                        <p style="color:var(--text-muted); font-size:0.8rem; text-align:center; padding:0.8rem;">Aún no has registrado bonificaciones.</p>
                    `}
                </div>
                ` : ''}
            </div>
        `;
    }

    async handleAgregarBonificacion(e) {
        e.preventDefault();
        const fecha = document.getElementById('bon_fecha').value.trim();
        const tarjeta = Number(document.getElementById('bon_tarjeta').value);
        const divisa = document.getElementById('bon_divisa').value;
        const monto = Number(document.getElementById('bon_monto').value);
        const concepto = document.getElementById('bon_concepto').value.trim();

        if (!(monto > 0)) { this.showToast("El monto de la bonificación debe ser mayor que cero.", "error"); return; }
        if (!concepto) { this.showToast("Indica el concepto: distingue un cashback de una promoción o recompensa.", "error"); return; }

        try {
            await AppAPI.crearBonificacion(fecha, tarjeta, monto, divisa, concepto);
            this.showToast("Bonificación registrada. La deuda de la tarjeta se redujo.");
            await this.render('tarjetas');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    async handleEliminarBonificacion(id) {
        try {
            await AppAPI.eliminarBonificacion(id);
            this.showToast("Bonificación revertida. La deuda vuelve a su valor anterior.");
            await this.render('tarjetas');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    // --- RENDER: SUSCRIPCIONES ---
    async renderSuscripciones() {
        const suscripciones = await AppAPI.obtenerSuscripciones();
        const tarjetas = await AppAPI.obtenerTarjetas();

        this.contentContainer.innerHTML = `
            <div class="section-title">
                <h1>Suscripciones Recurrentes</h1>
                <span class="subtitle">Monitoreo de membresías mensuales y anuales debidamente cargadas</span>
            </div>

            <div class="responsive-split-grid">
                <div class="card" style="height:fit-content;">
                    <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 1rem;">🔄 Nueva Suscripción</h3>
                    ${tarjetas.length > 0 ? `
                        <form id="form-add-suscripcion" onsubmit="appUI.handleAgregarSuscripcion(event)">
                            <div class="form-group">
                                <label for="sus_pla">Servicio / Plataforma *</label>
                                <input type="text" id="sus_pla" class="form-control" placeholder="Netflix, Spotify, AWS..." required>
                            </div>
                            <div class="form-row">
                                <div class="form-group" style="flex:2;">
                                    <label for="sus_mon">Monto *</label>
                                    <input type="number" id="sus_mon" step="0.01" class="form-control" placeholder="0.00" required>
                                </div>
                                <div class="form-group" style="flex:1;">
                                    <label for="sus_div">Divisa</label>
                                    <select id="sus_div" class="form-control">
                                        <option value="DOP" selected>DOP</option>
                                        <option value="USD">USD</option>
                                    </select>
                                </div>
                            </div>
                            <div class="form-row">
                                <div class="form-group">
                                    <label for="sus_dia">Día Facturación (1-31) *</label>
                                    <input type="number" id="sus_dia" min="1" max="31" value="1" class="form-control" required>
                                </div>
                                <div class="form-group">
                                    <label for="sus_fre">Frecuencia</label>
                                    <select id="sus_fre" class="form-control">
                                        <option value="mensual" selected>Mensual</option>
                                        <option value="anual">Anual</option>
                                    </select>
                                </div>
                            </div>
                            <div class="form-group">
                                <label for="sus_tar">Tarjeta de Cargo *</label>
                                <select id="sus_tar" class="form-control" required>
                                    <option value="" disabled selected>Seleccione...</option>
                                    ${tarjetas.map(t => `<option value="${t.id}">${t.entidad} - ${t.nombre_tarjeta}</option>`).join('')}
                                </select>
                            </div>
                            <button type="submit" class="btn" style="width:100%; margin-top:0.5rem;">🚀 Registrar Cargo</button>
                        </form>
                    ` : `
                        <p style="color:var(--text-muted); text-align:center; padding:1rem 0;">
                            Registra primero una tarjeta de crédito para poder asociar suscripciones.
                        </p>
                    `}
                </div>

                <div class="card">
                    <h3 style="font-family: var(--font-heading); font-size: 1.1rem; margin-bottom: 1rem;">📋 Cargos Activos</h3>
                    ${suscripciones.length > 0 ? `
                        <div class="table-responsive">
                            <table class="table-modern">
                                <thead>
                                    <tr>
                                        <th>Servicio</th>
                                        <th>Frecuencia</th>
                                        <th>Día Pago</th>
                                        <th>Último Pago</th>
                                        <th>Tarjeta Cargo</th>
                                        <th>Monto</th>
                                        <th>Acciones</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    ${suscripciones.map(s => `
                                        <tr>
                                            <td><strong>${s.plataforma}</strong></td>
                                            <td style="text-transform:capitalize;">${s.frecuencia}</td>
                                            <td>Día ${s.dia_facturacion}</td>
                                            <td>${s.fecha_ultimo_pago || '<span style="font-style:italic;color:var(--text-muted);">Pendiente</span>'}</td>
                                            <td>${s.entidad} (${s.nombre_tarjeta})</td>
                                            <td class="amount expense">${s.divisa} ${this.formatMoney(s.monto)}</td>
                                            <td>
                                                <button onclick='appUI.abrirEdicionSuscripcion(${JSON.stringify(s).replace(/'/g, "&apos;")})' class="btn" style="padding: 0.3rem 0.5rem; font-size:0.8rem; background:rgba(255,255,255,0.05); border:1px solid var(--border-color);" title="Editar">✏️</button>
                                                <button onclick="appUI.handleEliminarSuscripcion(${s.id})" class="btn btn-danger" style="padding: 0.3rem 0.5rem; font-size:0.8rem;">🗑️</button>
                                            </td>
                                        </tr>
                                    `).join('')}
                                </tbody>
                            </table>
                        </div>
                    ` : `
                        <p style="color:var(--text-muted); text-align:center; padding:2rem;">No hay suscripciones registradas.</p>
                    `}
                </div>
            </div>
        `;
    }

    // --- RENDER: CUENTAS DE AHORRO ---
    async renderCuentas() {
        const cuentas = await AppAPI.obtenerCuentas();
        const transacciones = await AppAPI.obtenerTransaccionesCuentas();

        const hoy = new Date();
        const hoyStr = hoy.getDate().toString().padStart(2, '0') + '/' + (hoy.getMonth() + 1).toString().padStart(2, '0') + '/' + hoy.getFullYear();

        this.contentContainer.innerHTML = `
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
                                <strong style="font-size:1.1rem; color:var(--accent-primary);">DOP ${this.formatMoney(cuentas.filter(c => c.divisa === 'DOP').reduce((sum, c) => sum + c.balance_actual, 0))}</strong>
                            </div>
                            <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.6rem; border-radius:var(--radius-sm); display:flex; justify-content:space-between; align-items:center;">
                                <span style="font-size:0.8rem; color:var(--text-secondary);">Total USD</span>
                                <strong style="font-size:1.1rem; color: #10b981;">USD ${this.formatMoney(cuentas.filter(c => c.divisa === 'USD').reduce((sum, c) => sum + c.balance_actual, 0))}</strong>
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
                                <input type="text" id="tra_fec" class="form-control" value="${hoyStr}" required>
                            </div>
                            <div class="form-group">
                                <label for="tra_ori">Cuenta Origen *</label>
                                <select id="tra_ori" class="form-control" required>
                                    <option value="" disabled selected>Seleccione...</option>
                                    ${cuentas.map(c => `<option value="${c.id}">${c.nombre} (${c.divisa}) - Bal: ${c.divisa} ${this.formatMoney(c.balance_actual)}</option>`).join('')}
                                </select>
                            </div>
                            <div class="form-group">
                                <label for="tra_des">Cuenta Destino *</label>
                                <select id="tra_des" class="form-control" required>
                                    <option value="" disabled selected>Seleccione...</option>
                                    ${cuentas.map(c => `<option value="${c.id}">${c.nombre} (${c.divisa}) - Bal: ${c.divisa} ${this.formatMoney(c.balance_actual)}</option>`).join('')}
                                </select>
                            </div>
                            <div class="form-row">
                                <div class="form-group">
                                    <label for="tra_mon_ori">Monto Débito (Origen) *</label>
                                    <input type="number" id="tra_mon_ori" step="0.01" class="form-control" placeholder="0.00" required>
                                </div>
                                <div class="form-group">
                                    <label for="tra_mon_des">Monto Crédito (Destino) *</label>
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
                                    <div style="font-size:0.75rem; color:var(--text-muted); font-weight:bold; letter-spacing:1px; margin-bottom:0.3rem;">AHORRO - ${c.divisa}</div>
                                    <div style="font-family: var(--font-heading); font-size:1.1rem; font-weight:bold; margin-bottom:0.5rem; white-space:nowrap; overflow:hidden; text-overflow:ellipsis;" title="${c.nombre}">${c.nombre}</div>
                                    <div style="font-size:1.25rem; font-weight:bold; color: ${c.divisa === 'USD' ? '#10b981' : 'var(--accent-primary)'};">${c.divisa} ${this.formatMoney(c.balance_actual)}</div>
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
                                                <td>${t.fecha}</td>
                                                <td><strong>${t.descripcion || '-'}</strong></td>
                                                <td>${t.cuenta_origen_nombre}</td>
                                                <td class="amount expense">${t.monto_origen > 0 ? `- ${t.monto_origen.toLocaleString('es-DO', {minimumFractionDigits: 2, maximumFractionDigits: 2})}` : '-'}</td>
                                                <td>${t.cuenta_destino_nombre}</td>
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

    // --- RENDER: EFECTIVO ---
    async renderEfectivo() {
        const cuentas = await AppAPI.obtenerCuentas();
        const cuentasAhorro = cuentas.filter(c => c.nombre !== 'Efectivo DOP' && c.nombre !== 'Efectivo USD');
        const efectivoDop = cuentas.find(c => c.nombre === 'Efectivo DOP') || { balance_actual: 0 };
        const efectivoUsd = cuentas.find(c => c.nombre === 'Efectivo USD') || { balance_actual: 0 };

        const hoy = new Date();
        const hoyStr = hoy.getDate().toString().padStart(2, '0') + '/' + (hoy.getMonth() + 1).toString().padStart(2, '0') + '/' + hoy.getFullYear();

        this.contentContainer.innerHTML = `
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
                                <strong style="font-size:1.3rem; color:var(--accent-primary);">DOP ${this.formatMoney(efectivoDop.balance_actual)}</strong>
                            </div>
                            <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.8rem; border-radius:var(--radius-md); display:flex; justify-content:space-between; align-items:center;">
                                <span style="font-size:0.85rem; color:var(--text-secondary);">Efectivo USD</span>
                                <strong style="font-size:1.3rem; color: #10b981;">USD ${this.formatMoney(efectivoUsd.balance_actual)}</strong>
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
                                    ${cuentasAhorro.map(c => `<option value="${c.id}">${c.nombre} (${c.divisa}) - Bal: ${c.divisa} ${this.formatMoney(c.balance_actual)}</option>`).join('')}
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

    // --- RENDER: CAPITAL ---
    async renderCapital() {
        const capital = await AppAPI.obtenerCapital();
        const inmobiliario = capital?.propiedades?.inmobiliario || [];
        const vehiculos = capital?.propiedades?.vehiculos || [];
        const maquinaria = capital?.propiedades?.maquinaria || [];

        this.contentContainer.innerHTML = `
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
                        ${(capital.certificados || []).map((c, i) => `
                            <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.6rem; border-radius:4px; font-size:0.75rem;">
                                <div style="display:flex; justify-content:space-between; font-weight:bold;">
                                    <span>${c.banco}</span>
                                    <span>DOP ${this.formatMoney(c.monto)}</span>
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
                        ${(capital.bolsa || []).map((b, i) => `
                            <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.6rem; border-radius:4px; font-size:0.75rem;">
                                <div style="display:flex; justify-content:space-between; font-weight:bold;">
                                    <span>${b.emisor}</span>
                                    <span>DOP ${this.formatMoney(b.monto)}</span>
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
                            ${inmobiliario.map(p => `
                                <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.4rem; border-radius:4px; margin-bottom:0.2rem;">
                                    <span>${p.nombre}</span>
                                    <span>DOP ${this.formatMoney(p.valor_estimado)} <button onclick="appUI.handleEliminarPropiedad('inmobiliario', '${p.id}')" style="background:none; border:none; color:var(--color-danger); cursor:pointer; margin-left:0.4rem;">🗑️</button></span>
                                </div>
                            `).join('')}
                        ` : ''}

                        ${vehiculos.length > 0 ? `
                            <div style="font-weight:bold; color:var(--accent-primary); margin-top:0.4rem;">Vehículos:</div>
                            ${vehiculos.map(p => `
                                <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.4rem; border-radius:4px; margin-bottom:0.2rem;">
                                    <span>${p.nombre}</span>
                                    <span>DOP ${this.formatMoney(p.valor_estimado)} <button onclick="appUI.handleEliminarPropiedad('vehiculos', '${p.id}')" style="background:none; border:none; color:var(--color-danger); cursor:pointer; margin-left:0.4rem;">🗑️</button></span>
                                </div>
                            `).join('')}
                        ` : ''}

                        ${maquinaria.length > 0 ? `
                            <div style="font-weight:bold; color:var(--accent-primary); margin-top:0.4rem;">Equipos:</div>
                            ${maquinaria.map(p => `
                                <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.4rem; border-radius:4px; margin-bottom:0.2rem;">
                                    <span>${p.nombre}</span>
                                    <span>DOP ${this.formatMoney(p.valor_estimado)} <button onclick="appUI.handleEliminarPropiedad('maquinaria', '${p.id}')" style="background:none; border:none; color:var(--color-danger); cursor:pointer; margin-left:0.4rem;">🗑️</button></span>
                                </div>
                            `).join('')}
                        ` : ''}
                    </div>
                </div>
            </div>
        `;
    }

    // --- RENDER: PRESTAMOS ---
    async renderPrestamos() {
        const prestamos = await AppAPI.obtenerPrestamos();

        this.contentContainer.innerHTML = `
            <div class="section-title">
                <h1>Financiamientos y Deudas</h1>
                <span class="subtitle">Monitoreo de deudas, cuotas de préstamos y vencimientos mensuales</span>
            </div>

            <div class="responsive-split-grid">
                <!-- Formulario -->
                <div class="card" style="height:fit-content;">
                    <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 1rem;">📝 Registrar Financiamiento</h3>
                    <form id="form-add-prestamo" onsubmit="appUI.handleAgregarPrestamo(event)">
                        <div class="form-group">
                            <label for="pre_tip">Tipo de Préstamo *</label>
                            <select id="pre_tip" class="form-control" onchange="appUI.toggleCamposPrestamos(this.value)" required>
                                <option value="consumo" selected>Préstamo de Consumo</option>
                                <option value="hipotecario">Préstamo Hipotecario</option>
                                <option value="vehiculo">Préstamo de Vehículo</option>
                                <option value="flexible">Préstamo Flexible (Línea)</option>
                            </select>
                        </div>
                        <div class="form-group">
                            <label for="pre_ins">Institución Financiera *</label>
                            <input type="text" id="pre_ins" class="form-control" placeholder="Banco BHD, Popular..." required>
                        </div>
                        <div class="form-row">
                            <div class="form-group">
                                <label for="pre_mon">Monto Préstamo *</label>
                                <input type="number" step="0.01" id="pre_mon" class="form-control" placeholder="0.00" required>
                            </div>
                            <div class="form-group">
                                <label for="pre_tas">Tasa Actual (%) *</label>
                                <input type="number" step="0.01" id="pre_tas" class="form-control" placeholder="14.5" required>
                            </div>
                        </div>

                        <!-- Campos condicionales -->
                        <div id="pre_campos_cuotas" class="form-row">
                            <div class="form-group">
                                <label for="pre_tot">Cuotas Totales *</label>
                                <input type="number" id="pre_tot" class="form-control" placeholder="36" required>
                            </div>
                            <div class="form-group">
                                <label for="pre_pen">Cuotas Pendientes *</label>
                                <input type="number" id="pre_pen" class="form-control" placeholder="24" required>
                            </div>
                        </div>

                        <div class="form-row">
                            <div class="form-group">
                                <label for="pre_cuo">Monto Cuota *</label>
                                <input type="number" step="0.01" id="pre_cuo" class="form-control" placeholder="0.00" required>
                            </div>
                            <div class="form-group">
                                <label for="pre_dia">Día del mes Vence *</label>
                                <input type="number" min="1" max="31" id="pre_dia" class="form-control" placeholder="15" required>
                            </div>
                        </div>

                        <button type="submit" class="btn" style="width:100%; margin-top:0.5rem;">🚀 Registrar Deuda</button>
                    </form>
                </div>

                <!-- Lista de Deudas -->
                <div class="card">
                    <h3 style="font-family: var(--font-heading); font-size: 1.1rem; margin-bottom: 1rem;">📋 Listado de Deudas</h3>
                    ${prestamos.length > 0 ? `
                        <div class="table-responsive">
                            <table class="table-modern">
                                <thead>
                                    <tr>
                                        <th>Banco / Préstamo</th>
                                        <th>Monto Inicial</th>
                                        <th>Tasa</th>
                                        <th>Cuota</th>
                                        <th>Restantes</th>
                                        <th>Vence</th>
                                        <th>Acciones</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    ${prestamos.map(p => `
                                        <tr>
                                            <td>
                                                <strong style="text-transform:capitalize;">${p.tipo_prestamo}</strong><br>
                                                <span style="font-size:0.75rem; color:var(--text-secondary);">${p.institucion_financiera}</span>
                                            </td>
                                            <td class="amount">DOP ${this.formatMoney(p.monto_prestamo)}</td>
                                            <td>${p.tasa_actual}%</td>
                                            <td class="amount expense">DOP ${this.formatMoney(p.monto_cuota)}</td>
                                            <td>
                                                ${p.tipo_prestamo === 'flexible' ? `
                                                    <span style="color:var(--text-muted); font-style:italic;">Flexible</span>
                                                ` : `
                                                    <strong>${p.cuotas_pendientes} / ${p.cuotas_totales}</strong>
                                                `}
                                            </td>
                                            <td>
                                                Día ${p.dia_pago}
                                                ${p.alerta_pago ? `
                                                    <br><span class="badge danger" style="padding:1px 6px; font-size:0.65rem; text-transform:none; margin-top:0.2rem;">${p.dias_pago_msg}</span>
                                                ` : `
                                                    <br><span style="font-size:0.75rem; color:var(--text-muted);">${p.dias_pago_msg}</span>
                                                `}
                                            </td>
                                            <td>
                                                <div style="display:flex; gap:0.4rem; align-items:center;">
                                                    ${p.tipo_prestamo !== 'flexible' && p.cuotas_pendientes > 0 ? `
                                                        <button onclick="appUI.handlePagarCuota(${p.id})" class="btn" style="padding: 0.3rem 0.6rem; font-size:0.75rem; background: linear-gradient(135deg, var(--color-success), #059669); color: white;">Abonar</button>
                                                    ` : ''}
                                                    <button onclick="appUI.handleEliminarPrestamo(${p.id})" class="btn btn-danger" style="padding: 0.3rem 0.5rem; font-size:0.75rem;">🗑️</button>
                                                </div>
                                            </td>
                                        </tr>
                                    `).join('')}
                                </tbody>
                            </table>
                        </div>
                    ` : `
                        <p style="color:var(--text-muted); text-align:center; padding:2rem;">No hay deudas registradas.</p>
                    `}
                </div>
            </div>
        `;
    }

    toggleCamposPrestamos(val) {
        const container = document.getElementById('pre_campos_cuotas');
        const tot = document.getElementById('pre_tot');
        const pen = document.getElementById('pre_pen');

        if (val === 'flexible') {
            container.style.display = 'none';
            tot.required = false;
            pen.required = false;
            tot.value = '';
            pen.value = '';
        } else {
            container.style.display = 'grid';
            tot.required = true;
            pen.required = true;
        }
    }

    // --- RENDER: RESUMEN ---
    async renderResumen() {
        const capital = await AppAPI.obtenerCapital();
        const ingresos = await AppAPI.obtenerIngresos();
        const informales = await AppAPI.obtenerIngresosInformales();
        const gastos = await AppAPI.obtenerGastos();
        const tarjetas = await AppAPI.obtenerTarjetas();
        const suscripciones = await AppAPI.obtenerSuscripciones();
        const prestamos = await AppAPI.obtenerPrestamos();

        const hoy = new Date();
        const mesAnioActual = "/" + (hoy.getMonth() + 1).toString().padStart(2, '0') + '/' + hoy.getFullYear();

        // 1. Activos
        const totalCertificados = (capital?.certificados || []).reduce((sum, c) => sum + Number(c.monto || 0), 0);
        const totalBolsa = (capital?.bolsa || []).reduce((sum, b) => sum + Number(b.monto || 0), 0);
        const totalInmueble = (capital?.propiedades?.inmobiliario || []).reduce((sum, p) => sum + Number(p.valor_estimado || 0), 0);
        const totalVehiculo = (capital?.propiedades?.vehiculos || []).reduce((sum, p) => sum + Number(p.valor_estimado || 0), 0);
        const totalMaquinaria = (capital?.propiedades?.maquinaria || []).reduce((sum, p) => sum + Number(p.valor_estimado || 0), 0);
        
        const totalActivos = totalCertificados + totalBolsa + totalInmueble + totalVehiculo + totalMaquinaria;

        // 2. Pasivos
        const TASA_USD_A_DOP = 60.0;
        const totalTarjetas = tarjetas.reduce((sum, t) => sum + t.balance_pesos + (t.balance_dolares * TASA_USD_A_DOP), 0);
        
        let totalPrestamos = 0.0;
        prestamos.forEach(p => {
            if (p.tipo_prestamo === 'flexible') {
                totalPrestamos += p.monto_prestamo;
            } else {
                if (p.cuotas_totales > 0) {
                    totalPrestamos += (p.cuotas_pendientes / p.cuotas_totales) * p.monto_prestamo;
                }
            }
        });

        const totalPasivos = totalTarjetas + totalPrestamos;
        const patrimonioNeto = totalActivos - totalPasivos;
        const ratio = totalActivos > 0 ? ((totalPasivos / totalActivos) * 100.0) : 0.0;

        // 3. Flujos Fijos
        const cuotaPrestamos = prestamos.reduce((sum, p) => sum + (p.tipo_prestamo === 'flexible' || p.cuotas_pendientes > 0 ? p.monto_cuota : 0.0), 0);
        const cuotaSuscripciones = suscripciones.reduce((sum, s) => sum + (s.frecuencia === 'mensual' ? s.monto : (s.monto / 12.0)), 0);
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

        this.contentContainer.innerHTML = `
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
                            DOP ${this.formatMoney(patrimonioNeto)}
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
                        <span>DOP ${this.formatMoney(totalActivos)}</span>
                    </h3>
                    <div style="display:flex; flex-direction:column; gap:0.6rem; font-size:0.85rem;">
                        <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); padding:0.5rem; border-radius:4px;">
                            <span>Certificados Financieros</span>
                            <strong>DOP ${this.formatMoney(totalCertificados)}</strong>
                        </div>
                        <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); padding:0.5rem; border-radius:4px;">
                            <span>Bolsa de Valores</span>
                            <strong>DOP ${this.formatMoney(totalBolsa)}</strong>
                        </div>
                        <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); padding:0.5rem; border-radius:4px;">
                            <span>Bienes y Propiedades</span>
                            <strong>DOP ${this.formatMoney(totalInmueble + totalVehiculo + totalMaquinaria)}</strong>
                        </div>
                    </div>
                </div>

                <!-- Pasivos -->
                <div class="card">
                    <h3 style="font-family:var(--font-heading); font-size:1.15rem; color:var(--color-danger); border-bottom:1px solid var(--border-color); padding-bottom:0.5rem; display:flex; justify-content:space-between; margin-bottom:1rem;">
                        <span>🏷️ Pasivos (Deudas)</span>
                        <span>DOP ${this.formatMoney(totalPasivos)}</span>
                    </h3>
                    <div style="display:flex; flex-direction:column; gap:0.6rem; font-size:0.85rem;">
                        <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); padding:0.5rem; border-radius:4px;">
                            <span>Balances Tarjetas de Crédito</span>
                            <strong>DOP ${this.formatMoney(totalTarjetas)}</strong>
                        </div>
                        <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); padding:0.5rem; border-radius:4px;">
                            <span>Préstamos Vigentes</span>
                            <strong>DOP ${this.formatMoney(totalPrestamos)}</strong>
                        </div>
                    </div>
                </div>
            </div>

            <div class="grid-2">
                <!-- Compromisos Fijos -->
                <div class="card">
                    <h3 style="font-family:var(--font-heading); font-size:1.15rem; border-bottom:1px solid var(--border-color); padding-bottom:0.5rem; display:flex; justify-content:space-between; margin-bottom:1rem;">
                        <span>🔄 Carga Fija Mensual</span>
                        <span class="amount expense">DOP ${this.formatMoney(cargaFija)}</span>
                    </h3>
                    <div style="display:flex; flex-direction:column; gap:0.6rem; font-size:0.85rem;">
                        <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); padding:0.5rem; border-radius:4px;">
                            <span>Cuotas de Préstamos</span>
                            <strong>DOP ${this.formatMoney(cuotaPrestamos)}</strong>
                        </div>
                        <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); padding:0.5rem; border-radius:4px;">
                            <span>Suscripciones Recurrentes</span>
                            <strong>DOP ${this.formatMoney(cuotaSuscripciones)}</strong>
                        </div>
                    </div>
                </div>

                <!-- Balance del Mes -->
                <div class="card">
                    <h3 style="font-family:var(--font-heading); font-size:1.15rem; border-bottom:1px solid var(--border-color); padding-bottom:0.5rem; display:flex; justify-content:space-between; margin-bottom:1rem;">
                        <span>📊 Balance Mensual (Cobrado)</span>
                        <span class="amount ${balanceNeto >= 0 ? 'income' : 'expense'}">DOP ${this.formatMoney(balanceNeto)}</span>
                    </h3>
                    <div style="display:flex; flex-direction:column; gap:0.6rem; font-size:0.85rem;">
                        <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); padding:0.5rem; border-radius:4px;">
                            <span>Ingresos Cobrados (Flujo)</span>
                            <strong>DOP ${this.formatMoney(ingresosCobradosTotales)}</strong>
                        </div>
                        <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); padding:0.5rem; border-radius:4px; opacity:0.8;">
                            <span>Ingresos Facturados (Impuestos)</span>
                            <span>DOP ${this.formatMoney(ingresosFacturadosTotales)}</span>
                        </div>
                        <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); padding:0.5rem; border-radius:4px;">
                            <span>Gastos y Cargos (Este Mes)</span>
                            <strong>DOP ${this.formatMoney(gastosTotales)}</strong>
                        </div>
                    </div>
                </div>
            </div>
        `;
    }

    // --- RENDER: AJUSTES ---
    async renderAjustes() {
        const categorias = await AppAPI.obtenerCategorias();
        const clientes = await AppAPI.obtenerClientes();
        const cuentas = await AppAPI.obtenerCuentas();
        const gastos = await AppAPI.obtenerGastos();
        const informales = await AppAPI.obtenerIngresosInformales();
        const transacciones = await AppAPI.obtenerTransaccionesCuentas();
        const ingresos = await AppAPI.obtenerIngresos();

        this.contentContainer.innerHTML = `
            <div class="section-title">
                <h1>Ajustes</h1>
                <span class="subtitle">Configuraciones de catálogos y entorno nativo</span>
            </div>

            <div style="display:grid; grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); gap:1.5rem; margin-bottom: 2rem;">
                <!-- Categorías -->
                <div class="card" style="height:fit-content;">
                    <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 0.5rem;">🏷️ Categorías de Gastos</h3>
                    <p style="font-size:0.75rem; color:var(--text-secondary); margin-bottom:1rem;">Añade nuevas etiquetas o remueve las que no utilices.</p>
                    
                    <form id="form-add-categoria" onsubmit="appUI.handleAgregarCategoria(event)" style="display:flex; gap:0.4rem; margin-bottom: 1rem;">
                        <input type="text" id="cat_nom" class="form-control" placeholder="Nueva categoría..." style="padding:0.5rem 0.8rem;" required>
                        <button type="submit" class="btn" style="padding:0.5rem 0.8rem;">➕</button>
                    </form>

                    <div style="display:flex; flex-direction:column; gap:0.4rem; max-height:220px; overflow-y:auto; font-size:0.85rem;">
                        ${categorias.map(c => `
                            <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.5rem 0.8rem; border-radius:var(--radius-sm); display:flex; justify-content:space-between; align-items:center;">
                                <strong>${c.nombre}</strong>
                                ${c.nombre !== 'Otros' && c.nombre !== 'Suscripciones' ? `
                                    <button onclick="appUI.handleEliminarCategoria(${c.id})" class="btn btn-danger" style="padding:0.2rem 0.4rem; font-size:0.75rem;">🗑️</button>
                                ` : `
                                    <span style="font-size:0.7rem; color:var(--text-muted); font-style:italic;">Sistema</span>
                                `}
                            </div>
                        `).join('')}
                    </div>
                </div>

                <!-- Clientes -->
                <div class="card" style="height:fit-content;">
                    <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 0.5rem;">👥 Catálogo de Clientes</h3>
                    <p style="font-size:0.75rem; color:var(--text-secondary); margin-bottom:1rem;">Registra clientes para utilizarlos en facturación formal.</p>
                    
                    <form id="form-add-cliente" onsubmit="appUI.handleAgregarCliente(event)" style="display:flex; flex-direction:column; gap:0.4rem; margin-bottom: 1rem;">
                        <input type="text" id="cli_aj_nom" class="form-control" placeholder="Nombre (Ej. Empresa S.A.)..." style="padding:0.4rem 0.6rem;" required>
                        <input type="text" id="cli_aj_rnc" class="form-control" placeholder="RNC (Ej. 131XXXXXX)..." style="padding:0.4rem 0.6rem;" required>
                        <button type="submit" class="btn" style="padding:0.4rem; font-size:0.85rem;">🚀 Registrar Cliente</button>
                    </form>

                    <div style="display:flex; flex-direction:column; gap:0.4rem; max-height:220px; overflow-y:auto; font-size:0.85rem;">
                        ${clientes.length > 0 ? clientes.map(cl => `
                            <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.5rem 0.8rem; border-radius:var(--radius-sm); display:flex; justify-content:space-between; align-items:center;">
                                <div>
                                    <strong>${cl.nombre}</strong><br>
                                    <span style="font-size:0.7rem; color:var(--text-muted);">RNC: ${cl.rnc}</span>
                                </div>
                                <button onclick="appUI.handleEliminarCliente(${cl.id})" class="btn btn-danger" style="padding:0.2rem 0.4rem; font-size:0.75rem;">🗑️</button>
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
                    
                    <form id="form-add-cuenta" onsubmit="appUI.handleAgregarCuenta(event)" style="display:flex; flex-direction:column; gap:0.4rem; margin-bottom: 1rem;">
                        <input type="text" id="cue_aj_nom" class="form-control" placeholder="Nombre de la Cuenta (Ej. BHD Ahorro)..." required style="padding:0.4rem 0.6rem;">
                        <div style="display:flex; gap:0.4rem;">
                            <select id="cue_aj_div" class="form-control" style="flex:1; padding:0.4rem 0.6rem;">
                                <option value="DOP" selected>DOP</option>
                                <option value="USD">USD</option>
                            </select>
                            <input type="number" id="cue_aj_bal" class="form-control" placeholder="Balance Inicial..." step="0.01" value="0.00" required style="flex:1.5; padding:0.4rem 0.6rem;">
                        </div>
                        <button type="submit" class="btn" style="padding:0.4rem; font-size:0.85rem; background: linear-gradient(135deg, var(--accent-primary), #00cdac); color:white;">🚀 Registrar Cuenta</button>
                    </form>

                    <div style="display:flex; flex-direction:column; gap:0.4rem; max-height:220px; overflow-y:auto; font-size:0.85rem;">
                        ${cuentas.length > 0 ? cuentas.map(c => `
                            <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.5rem 0.8rem; border-radius:var(--radius-sm); display:flex; justify-content:space-between; align-items:center;">
                                <div>
                                    <strong>${c.nombre}</strong><br>
                                    <span style="font-size:0.75rem; color:var(--accent-primary); font-weight:bold;">${c.divisa} ${this.formatMoney(c.balance_actual)}</span>
                                </div>
                                <button onclick="appUI.handleEliminarCuenta(${c.id})" class="btn btn-danger" style="padding:0.2rem 0.4rem; font-size:0.75rem;">🗑️</button>
                            </div>
                        `).join('') : `
                            <p style="color:var(--text-muted); text-align:center; padding:1rem; font-size:0.75rem;">No hay cuentas registradas.</p>
                        `}
                    </div>
                </div>

                <!-- Tarjetas de Crédito -->
                <div class="card" style="height:fit-content;">
                    <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 0.5rem;">💳 Tarjetas de Crédito</h3>
                    <p style="font-size:0.75rem; color:var(--text-secondary); margin-bottom:1rem;">Registra tarjetas de crédito para consumos y abonos.</p>
                    
                    <form id="form-add-tarjeta" onsubmit="appUI.handleAgregarTarjeta(event)" style="display:flex; flex-direction:column; gap:0.4rem;">
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
                <p style="font-size:0.75rem; color:var(--text-secondary); margin-bottom:1.2rem;">
                    Revertir y eliminar transacciones mal registradas. El sistema ajustará automáticamente los saldos de cuentas, tarjetas y efectivo asociados.
                </p>
                
                <div style="display:flex; gap:0.5rem; margin-bottom: 1rem; border-bottom: 1px solid var(--border-color); padding-bottom: 0.5rem;">
                    <button type="button" onclick="document.getElementById('corr-gastos').style.display='block'; document.getElementById('corr-ingresos').style.display='none'; document.getElementById('corr-trans').style.display='none'; this.className='btn'; document.getElementById('btn-corr-ing').className='btn btn-secondary'; document.getElementById('btn-corr-tra').className='btn btn-secondary';" id="btn-corr-gas" class="btn" style="padding: 0.35rem 0.75rem; font-size:0.8rem;">💸 Gastos</button>
                    <button type="button" onclick="document.getElementById('corr-gastos').style.display='none'; document.getElementById('corr-ingresos').style.display='block'; document.getElementById('corr-trans').style.display='none'; this.className='btn'; document.getElementById('btn-corr-gas').className='btn btn-secondary'; document.getElementById('btn-corr-tra').className='btn btn-secondary';" id="btn-corr-ing" class="btn btn-secondary" style="padding: 0.35rem 0.75rem; font-size:0.8rem; border:none;">📈 Ingresos</button>
                    <button type="button" onclick="document.getElementById('corr-gastos').style.display='none'; document.getElementById('corr-ingresos').style.display='none'; document.getElementById('corr-trans').style.display='block'; this.className='btn'; document.getElementById('btn-corr-gas').className='btn btn-secondary'; document.getElementById('btn-corr-ing').className='btn btn-secondary';" id="btn-corr-tra" class="btn btn-secondary" style="padding: 0.35rem 0.75rem; font-size:0.8rem; border:none;">🔄 Transferencias</button>
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
                                            <td>${g.fecha}</td>
                                            <td><strong>${g.descripcion}</strong></td>
                                            <td>${g.categoria_nombre}</td>
                                            <td style="text-transform:capitalize;">${g.metodo_pago}</td>
                                            <td class="amount expense">${g.divisa} ${this.formatMoney(g.monto + g.costo_adicional)}</td>
                                            <td>
                                                <button type="button" onclick="appUI.handleEliminarGastoCorr(${g.id})" class="btn btn-danger" style="padding: 0.2rem 0.4rem; font-size: 0.7rem;">🗑️ Revertir</button>
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
                                            <td>${inf.fecha}</td>
                                            <td><strong>${inf.descripcion}</strong></td>
                                            <td class="amount income">DOP ${this.formatMoney(inf.monto)}</td>
                                            <td><span class="badge ${inf.estatus === 'pagado' ? 'pagada' : 'emitida'}">${inf.estatus}</span></td>
                                            <td>${inf.institucion_deposito || '-'}</td>
                                            <td>
                                                <button type="button" onclick="appUI.handleEliminarIngresoInformalCorr(${inf.id})" class="btn btn-danger" style="padding: 0.2rem 0.4rem; font-size: 0.7rem;">🗑️ Revertir</button>
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
                                            <td><strong>${i.numero_factura}</strong></td>
                                            <td>${i.cliente_nombre}</td>
                                            <td>${i.fecha_emision}</td>
                                            <td class="amount income">DOP ${this.formatMoney(i.monto_total)}</td>
                                            <td><span class="badge ${i.estatus === 'pagada' ? 'pagada' : 'emitida'}">${i.estatus}</span></td>
                                            <td>
                                                <button type="button" onclick="appUI.handleEliminarIngresoCorr(${i.id})" class="btn btn-danger" style="padding: 0.2rem 0.4rem; font-size: 0.7rem;">🗑️ Eliminar</button>
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
                                            <td>${t.fecha}</td>
                                            <td><strong>${t.descripcion || '-'}</strong></td>
                                            <td>${t.cuenta_origen_nombre}</td>
                                            <td class="amount expense">- ${this.formatMoney(t.monto_origen)}</td>
                                            <td>${t.cuenta_destino_nombre}</td>
                                            <td class="amount expense">${t.cargo > 0 ? this.formatMoney(t.cargo) : '-'}</td>
                                            <td>
                                                <button type="button" onclick="appUI.handleEliminarTransaccionCuentaCorr(${t.id})" class="btn btn-danger" style="padding: 0.2rem 0.4rem; font-size: 0.7rem;">🗑️ Revertir</button>
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

    // --- MANEJADORES DE ENTRADAS ---
    /**
     * Muestra el campo de tasa solo cuando el gasto y la cuenta de débito van
     * en divisas distintas, y adelanta lo que saldrá de la cuenta.
     *
     * Mostrar el importe antes de confirmar evita el descuadre que obliga
     * después a revertir el gasto.
     */
    actualizarConversionGasto() {
        const contenedor = document.getElementById('gas_conversion_container');
        if (!contenedor) return;

        const metodo = document.getElementById('gas_met')?.value;
        const selCuenta = document.getElementById('gas_cue');
        const opcion = selCuenta?.selectedOptions?.[0];
        const divisaCuenta = opcion?.dataset?.divisa;
        const divisaGasto = document.getElementById('gas_div')?.value;

        const cruzaDivisas = metodo === 'transferencia' && divisaCuenta && divisaGasto && divisaCuenta !== divisaGasto;
        contenedor.style.display = cruzaDivisas ? 'block' : 'none';

        const previa = document.getElementById('gas_conversion_previa');
        if (!cruzaDivisas) { if (previa) previa.textContent = ''; return; }

        const monto = Number(document.getElementById('gas_mon')?.value) || 0;
        const tasa = Number(document.getElementById('gas_tasa')?.value) || 0;
        if (!(monto > 0) || !(tasa > 0)) {
            previa.textContent = `Indica la tasa para saber cuánto saldrá en ${divisaCuenta}.`;
            return;
        }

        // Misma regla que el dominio: convertir primero, retener después.
        const convertido = divisaGasto === 'USD' ? monto * tasa : monto / tasa;
        const convertidoRedondeado = Math.round(convertido * 100) / 100;
        const retencion = Math.round(convertidoRedondeado * 0.002 * 100) / 100;
        const lbtr = document.getElementById('gas_lbtr')?.checked ? 100 : 0;
        const total = convertidoRedondeado + retencion + lbtr;

        previa.innerHTML = `Saldrán <strong>${divisaCuenta} ${this.formatMoney(total)}</strong> `
            + `— ${this.formatMoney(convertidoRedondeado)} convertidos`
            + (retencion ? ` + ${this.formatMoney(retencion)} de retención` : '')
            + (lbtr ? ` + ${this.formatMoney(lbtr)} de LBTR` : '');
    }

    async handleAgregarGasto(e) {
        e.preventDefault();
        const fec = document.getElementById('gas_fec').value;
        const mon = Number(document.getElementById('gas_mon').value);
        const div = document.getElementById('gas_div').value;
        const des = document.getElementById('gas_des').value;
        const cat = Number(document.getElementById('gas_cat').value);
        const met = document.getElementById('gas_met').value;
        const lbtr = document.getElementById('gas_lbtr') ? document.getElementById('gas_lbtr').checked : false;
        const tar = document.getElementById('gas_tar') ? Number(document.getElementById('gas_tar').value) : null;
        const cue = document.getElementById('gas_cue') && met === 'transferencia' ? Number(document.getElementById('gas_cue').value) : null;

        // La tasa solo viaja si el gasto y la cuenta van en divisas distintas.
        // El backend la exige en ese caso y la ignora en el resto.
        const opcionCuenta = document.getElementById('gas_cue')?.selectedOptions?.[0];
        const divisaCuenta = opcionCuenta?.dataset?.divisa;
        const cruzaDivisas = met === 'transferencia' && divisaCuenta && divisaCuenta !== div;
        const tasa = cruzaDivisas ? Number(document.getElementById('gas_tasa')?.value) || 0 : 0;

        if (cruzaDivisas && !(tasa > 0)) {
            this.showToast(`El gasto va en ${div} y la cuenta en ${divisaCuenta}: indica la tasa de cambio.`, 'error');
            return;
        }

        try {
            await AppAPI.crearGasto({
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
            this.showToast("Gasto registrado con éxito.");
            await this.render('gastos');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    /**
     * Liquida un consumo pendiente. Pide el IMPORTE en pesos, no la tasa: el
     * emisor comunica cuánto cargó, nunca a qué tasa lo hizo.
     */
    abrirLiquidacionConsumo(id, montoOrigen, divisaOrigen, descripcion) {
        const overlay = document.createElement('div');
        overlay.className = 'modal-overlay';
        overlay.id = `modal-liq-${id}`;
        overlay.innerHTML = `
            <div class="card" style="width: 420px; background: var(--bg-surface-opaque);">
                <h3 style="font-family: var(--font-heading); margin-bottom:0.4rem;">⏳ Liquidar consumo</h3>
                <p style="font-size:0.75rem; color:var(--text-secondary); margin-bottom:1rem;">
                    ${descripcion} — <strong>${divisaOrigen} ${this.formatMoney(montoOrigen)}</strong><br>
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
                        <button type="button" onclick="document.getElementById('modal-liq-${id}').remove()" class="btn btn-secondary">Cancelar</button>
                        <button type="submit" class="btn">Liquidar</button>
                    </div>
                </form>
            </div>
        `;
        document.body.appendChild(overlay);
    }

    previsualizarTasa(id, montoOrigen) {
        const destino = Number(document.getElementById(`liq_monto_${id}`).value);
        const salida = document.getElementById(`liq_tasa_${id}`);
        if (!(destino > 0) || !(montoOrigen > 0)) { salida.textContent = ''; return; }
        salida.innerHTML = `Tasa aplicada por el emisor: <strong>${(destino / montoOrigen).toFixed(4)}</strong>`;
    }

    async handleLiquidacionSubmit(e, id, montoOrigen) {
        e.preventDefault();
        const monto = Number(document.getElementById(`liq_monto_${id}`).value);
        if (!(monto > 0)) { this.showToast("El importe en pesos debe ser mayor que cero.", "error"); return; }

        try {
            const tasa = await AppAPI.liquidarConsumoPendiente(id, monto);
            this.showToast(`Consumo liquidado a una tasa de ${Number(tasa).toFixed(4)}.`);
            document.getElementById(`modal-liq-${id}`)?.remove();
            await this.render('gastos');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    async handleAgregarIngreso(e) {
        e.preventDefault();
        const fac = document.getElementById('num_fac').value;
        const fec = document.getElementById('fec_em').value;
        const cli = document.getElementById('cli_nom').value;
        const rnc = document.getElementById('cli_rnc').value;
        const mon = Number(document.getElementById('mon_tot').value);
        const ret = Number(document.getElementById('ret_por').value);

        try {
            await AppAPI.crearIngreso({
                numero_factura: fac,
                rnc_cliente: rnc,
                nombre_cliente: cli,
                fecha_emision: fec,
                monto_total: mon,
                porcentaje_retencion: ret
            });
            this.showToast("Factura registrada exitosamente.");
            await this.render('ingresos');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    async handleAgregarIngresoInformal(e) {
        e.preventDefault();
        const fec = document.getElementById('fecha_inf').value;
        const mon = Number(document.getElementById('monto_inf').value);
        const des = document.getElementById('desc_inf').value;

        try {
            await AppAPI.crearIngresoInformal(fec, des, mon);
            this.showToast("Ingreso informal guardado.");
            await this.render('ingresos');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    async handleAgregarTarjeta(e) {
        e.preventDefault();
        const ent = document.getElementById('tar_ent').value;
        const nom = document.getElementById('tar_nom').value;
        const limDop = Number(document.getElementById('tar_lim_dop').value);
        const sobDop = Number(document.getElementById('tar_sob_dop').value);
        const balDop = Number(document.getElementById('tar_bal_dop').value);
        const corDop = Number(document.getElementById('tar_cor_dop').value);
        const limUsd = Number(document.getElementById('tar_lim_usd').value);
        const sobUsd = Number(document.getElementById('tar_sob_usd').value);
        const balUsd = Number(document.getElementById('tar_bal_usd').value);
        const corUsd = Number(document.getElementById('tar_cor_usd').value);
        const cor = Number(document.getElementById('tar_cor').value);
        const pag = Number(document.getElementById('tar_pag').value);

        try {
            await AppAPI.crearTarjeta(ent, nom, limDop, limUsd, sobDop, sobUsd, balDop, balUsd, corDop, corUsd, cor, pag);
            this.showToast("Tarjeta registrada.");
            await this.render('tarjetas');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    /**
     * Rellena el monto del abono según el tipo elegido y la divisa activa.
     * El importe queda visible antes de confirmar, en lugar de resolverse de
     * forma opaca al enviar el formulario.
     */
    aplicarTipoAbono(id, cortePesos, corteDolares, balPesos, balDolares) {
        const tipo = document.getElementById(`pag_tipo_${id}`).value;
        const divisa = document.getElementById(`pag_div_${id}`).value;
        const campoMonto = document.getElementById(`pag_monto_${id}`);

        if (tipo === 'personalizado') {
            campoMonto.readOnly = false;
            return;
        }

        const esDolares = divisa === 'USD';
        const saldo = tipo === 'corte'
            ? (esDolares ? corteDolares : cortePesos)
            : (esDolares ? balDolares : balPesos);

        campoMonto.value = Number(saldo).toFixed(2);
        campoMonto.readOnly = true;

        if (Number(saldo) === 0) {
            this.showToast(`No hay saldo ${tipo === 'corte' ? 'al corte' : 'actual'} en ${divisa}.`, 'info');
        }
    }

    async handleAbonoTarjeta(e, id) {
        e.preventDefault();

        const fec = document.getElementById(`pag_fecha_${id}`).value;
        const div = document.getElementById(`pag_div_${id}`).value;
        const mon = Number(document.getElementById(`pag_monto_${id}`).value);
        const cueId = document.getElementById(`pag_cuenta_${id}`).value;

        if (!(mon > 0)) {
            this.showToast("El monto del abono debe ser mayor que cero.", "error");
            return;
        }

        let tasaCambio = Number(document.getElementById(`pag_tasa_${id}`).value) || 0;
        if (cueId) {
            try {
                const cuentas = await AppAPI.obtenerCuentas();
                const cuenta = cuentas.find(c => c.id === Number(cueId));
                if (cuenta && cuenta.divisa === "DOP" && div === "USD") {
                    if (tasaCambio <= 0) {
                        const promptVal = prompt(`Estás realizando un abono de USD ${mon} desde la cuenta en Pesos "${cuenta.nombre}".\nPor favor, ingresa la tasa de cambio (DOP por 1 USD):`, "60.0");
                        if (promptVal === null) {
                            this.showToast("Operación cancelada.", "info");
                            return;
                        }
                        tasaCambio = Number(promptVal);
                        if (isNaN(tasaCambio) || tasaCambio <= 0) {
                            this.showToast("Tasa de cambio inválida.", "error");
                            return;
                        }
                    }
                }
            } catch (err) {
                this.showToast("Error al validar cuenta: " + err.toString(), 'error');
                return;
            }
        }

        try {
            await AppAPI.registrarPagoTarjeta(id, fec, mon, div, cueId ? Number(cueId) : null, tasaCambio);
            this.showToast("Abono a tarjeta guardado.");
            await this.render('tarjetas');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    async handleAgregarSuscripcion(e) {
        e.preventDefault();
        const pla = document.getElementById('sus_pla').value;
        const mon = Number(document.getElementById('sus_mon').value);
        const div = document.getElementById('sus_div').value;
        const dia = Number(document.getElementById('sus_dia').value);
        const fre = document.getElementById('sus_fre').value;
        const tar = Number(document.getElementById('sus_tar').value);

        try {
            await AppAPI.crearSuscripcion(pla, mon, tar, fre, dia, div);
            this.showToast("Suscripción recurrente guardada.");
            await this.render('suscripciones');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    async abrirEdicionSuscripcion(s) {
        const tarjetas = await AppAPI.obtenerTarjetas();
        const overlay = document.createElement('div');
        overlay.className = 'modal-overlay';
        overlay.id = `modal-edit-sus-${s.id}`;
        overlay.innerHTML = `
            <div class="card" style="width: 420px; background: var(--bg-surface-opaque); max-height:90vh; overflow-y:auto;">
                <h3 style="font-family: var(--font-heading); margin-bottom: 0.4rem;">✏️ Editar Suscripción</h3>
                <p style="font-size:0.72rem; color:var(--text-secondary); margin-bottom:1rem;">
                    Se conserva el último pago registrado (${s.fecha_ultimo_pago || 'ninguno'}), de modo que editar no provoca un cobro repetido este período.
                </p>
                <form onsubmit="appUI.handleEdicionSuscripcionSubmit(event, ${s.id})">
                    <div class="form-group">
                        <label>Servicio / Plataforma</label>
                        <input type="text" id="es_pla_${s.id}" class="form-control" value="${s.plataforma}" required>
                    </div>
                    <div class="form-row">
                        <div class="form-group">
                            <label>Monto</label>
                            <input type="number" step="0.01" id="es_mon_${s.id}" class="form-control" value="${s.monto}" required>
                        </div>
                        <div class="form-group">
                            <label>Divisa</label>
                            <select id="es_div_${s.id}" class="form-control">
                                <option value="DOP" ${s.divisa === 'DOP' ? 'selected' : ''}>DOP</option>
                                <option value="USD" ${s.divisa === 'USD' ? 'selected' : ''}>USD</option>
                            </select>
                        </div>
                    </div>
                    <div class="form-row">
                        <div class="form-group">
                            <label>Día de facturación</label>
                            <input type="number" min="1" max="31" id="es_dia_${s.id}" class="form-control" value="${s.dia_facturacion}" required>
                        </div>
                        <div class="form-group">
                            <label>Frecuencia</label>
                            <select id="es_fre_${s.id}" class="form-control">
                                <option value="mensual" ${s.frecuencia === 'mensual' ? 'selected' : ''}>Mensual</option>
                                <option value="anual" ${s.frecuencia === 'anual' ? 'selected' : ''}>Anual</option>
                            </select>
                        </div>
                    </div>
                    <div class="form-group">
                        <label>Tarjeta de cargo</label>
                        <select id="es_tar_${s.id}" class="form-control" required>
                            ${tarjetas.map(t => `<option value="${t.id}" ${t.id === s.tarjeta_id ? 'selected' : ''}>${t.entidad} - ${t.nombre_tarjeta}</option>`).join('')}
                        </select>
                    </div>
                    <div style="display:flex; justify-content:flex-end; gap:0.5rem; margin-top:1.2rem;">
                        <button type="button" onclick="document.getElementById('modal-edit-sus-${s.id}').remove()" class="btn btn-secondary">Cancelar</button>
                        <button type="submit" class="btn">Guardar Cambios</button>
                    </div>
                </form>
            </div>
        `;
        document.body.appendChild(overlay);
    }

    async handleEdicionSuscripcionSubmit(e, id) {
        e.preventDefault();
        const pla = document.getElementById(`es_pla_${id}`).value.trim();
        const mon = Number(document.getElementById(`es_mon_${id}`).value);
        const div = document.getElementById(`es_div_${id}`).value;
        const dia = Number(document.getElementById(`es_dia_${id}`).value);
        const fre = document.getElementById(`es_fre_${id}`).value;
        const tar = Number(document.getElementById(`es_tar_${id}`).value);

        if (!pla) { this.showToast("El nombre del servicio no puede estar vacío.", "error"); return; }
        if (!(mon > 0)) { this.showToast("El monto debe ser mayor que cero.", "error"); return; }
        if (!(dia >= 1 && dia <= 31)) { this.showToast("El día de facturación debe estar entre 1 y 31.", "error"); return; }

        try {
            await AppAPI.actualizarSuscripcion(id, pla, mon, tar, fre, dia, div);
            this.showToast("Suscripción actualizada. Los cargos ya realizados no se alteran.");
            document.getElementById(`modal-edit-sus-${id}`).remove();
            await this.render('suscripciones');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    async handleEliminarSuscripcion(id) {
        if (confirm("¿Deseas dar de baja esta suscripción?")) {
            await AppAPI.eliminarSuscripcion(id);
            this.showToast("Suscripción eliminada.");
            await this.render('suscripciones');
        }
    }

    async handleAgregarCliente(e) {
        e.preventDefault();
        const nom = document.getElementById('cli_aj_nom').value;
        const rnc = document.getElementById('cli_aj_rnc').value;
        try {
            await AppAPI.crearCliente(rnc, nom);
            this.showToast("Cliente registrado.");
            await this.render('ajustes');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    async handleEliminarCliente(id) {
        if (confirm("¿Deseas eliminar este cliente?")) {
            try {
                await AppAPI.eliminarCliente(id);
                this.showToast("Cliente eliminado.");
                await this.render('ajustes');
            } catch (err) {
                this.showToast(err.toString(), 'error');
            }
        }
    }

    async handleAgregarCuenta(e) {
        e.preventDefault();
        const nom = document.getElementById('cue_aj_nom').value;
        const div = document.getElementById('cue_aj_div').value;
        const bal = Number(document.getElementById('cue_aj_bal').value);
        try {
            await AppAPI.crearCuenta(nom, div, bal);
            this.showToast("Cuenta de ahorro registrada.");
            await this.render('ajustes');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    async handleEliminarCuenta(id) {
        if (confirm("¿Deseas eliminar esta cuenta de ahorro?")) {
            try {
                await AppAPI.eliminarCuenta(id);
                this.showToast("Cuenta eliminada.");
                await this.render('ajustes');
            } catch (err) {
                this.showToast(err.toString(), 'error');
            }
        }
    }

    async handleTransferirCuentas(e) {
        e.preventDefault();
        const fec = document.getElementById('tra_fec').value;
        const ori = Number(document.getElementById('tra_ori').value);
        const des = Number(document.getElementById('tra_des').value);
        const monOri = Number(document.getElementById('tra_mon_ori').value);
        const monDes = Number(document.getElementById('tra_mon_des').value);
        const car = Number(document.getElementById('tra_car').value);
        const txt = document.getElementById('tra_des_txt').value;
        try {
            await AppAPI.transferirEntreCuentas(fec, ori, des, monOri, monDes, car, txt);
            this.showToast("Transacción ejecutada con éxito.");
            await this.render('cuentas');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    async handleAgregarEfectivoInformal(e) {
        e.preventDefault();
        const fec = document.getElementById('efe_inf_fec').value;
        const mon = Number(document.getElementById('efe_inf_mon').value);
        const div = document.getElementById('efe_inf_div').value;
        const des = document.getElementById('efe_inf_des').value;
        try {
            await AppAPI.crearCobroEfectivoInformal(fec, des, mon, div);
            this.showToast("Entrada en efectivo registrada correctamente.");
            await this.render('efectivo');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    async handleRetirarAEfectivo(e) {
        e.preventDefault();
        const fec = document.getElementById('efe_ret_fec').value;
        const oriId = Number(document.getElementById('efe_ret_ori').value);
        const mon = Number(document.getElementById('efe_ret_mon').value);
        const car = Number(document.getElementById('efe_ret_car').value);
        const desTxt = document.getElementById('efe_ret_des_txt').value;

        try {
            const cuentas = await AppAPI.obtenerCuentas();
            const ori = cuentas.find(c => c.id === oriId);
            if (!ori) throw new Error("Cuenta origen no encontrada");

            const cashName = ori.divisa === 'USD' ? 'Efectivo USD' : 'Efectivo DOP';
            const des = cuentas.find(c => c.nombre === cashName);
            if (!des) throw new Error(`Cuenta destino ${cashName} no encontrada`);

            await AppAPI.transferirEntreCuentas(fec, ori.id, des.id, mon, mon, car, desTxt);
            this.showToast("Retiro de efectivo ejecutado exitosamente.");
            await this.render('efectivo');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    async handleEliminarGastoCorr(id) {
        if (confirm("¿Estás seguro de que deseas revertir y eliminar este gasto? Los balances asociados serán restaurados.")) {
            try {
                await AppAPI.eliminarGasto(id);
                this.showToast("Gasto revertido y eliminado.");
                await this.render('ajustes');
            } catch (err) {
                this.showToast(err.toString(), 'error');
            }
        }
    }

    async handleEliminarIngresoInformalCorr(id) {
        if (confirm("¿Estás seguro de que deseas revertir y eliminar este ingreso informal? El balance asociado (si ya fue cobrado en efectivo) será descontado.")) {
            try {
                await AppAPI.eliminarIngresoInformal(id);
                this.showToast("Ingreso informal revertido y eliminado.");
                await this.render('ajustes');
            } catch (err) {
                this.showToast(err.toString(), 'error');
            }
        }
    }

    async handleEliminarIngresoCorr(id) {
        if (confirm("¿Estás seguro de que deseas revertir y eliminar esta factura/ingreso formal?")) {
            try {
                await AppAPI.eliminarIngreso(id);
                this.showToast("Ingreso formal eliminado.");
                await this.render('ajustes');
            } catch (err) {
                this.showToast(err.toString(), 'error');
            }
        }
    }

    async handleEliminarTransaccionCuentaCorr(id) {
        if (confirm("¿Estás seguro de que deseas revertir y eliminar esta transferencia? Los saldos de las cuentas origen y destino serán restaurados.")) {
            try {
                await AppAPI.eliminarTransaccionCuenta(id);
                this.showToast("Transferencia revertida y eliminada.");
                await this.render('ajustes');
            } catch (err) {
                this.showToast(err.toString(), 'error');
            }
        }
    }

    handleSelectCliente(val) {
        const select = document.getElementById('cli_select');
        const option = select.options[select.selectedIndex];
        if (option) {
            document.getElementById('cli_nom').value = option.getAttribute('data-nombre') || "";
            document.getElementById('cli_rnc').value = option.getAttribute('data-rnc') || "";
        }
    }

    async handleAgregarCertificado(e) {
        e.preventDefault();
        const ban = document.getElementById('cer_ban').value;
        const mon = Number(document.getElementById('cer_mon').value);
        const tas = Number(document.getElementById('cer_tas').value);
        const ven = document.getElementById('cer_ven').value;
        const pag = document.getElementById('cer_pag').value;

        try {
            const capital = await AppAPI.obtenerCapital();
            if (!capital.certificados) capital.certificados = [];
            capital.certificados.push({ banco: ban, monto: mon, tasa: tas, vencimiento: ven, tipo_pago: pag });
            await AppAPI.guardarCapital(capital);
            this.showToast("Certificado guardado.");
            await this.render('capital');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    async handleEliminarCertificado(idx) {
        if (confirm("¿Retirar este certificado financiero?")) {
            const capital = await AppAPI.obtenerCapital();
            capital.certificados.splice(idx, 1);
            await AppAPI.guardarCapital(capital);
            this.showToast("Certificado retirado.");
            await this.render('capital');
        }
    }

    async handleAgregarBolsa(e) {
        e.preventDefault();
        const emi = document.getElementById('bol_emi').value;
        const mon = Number(document.getElementById('bol_mon').value);
        const tas = Number(document.getElementById('bol_tas').value);
        const ven = document.getElementById('bol_ven').value;
        const pag = document.getElementById('bol_pag').value;

        try {
            const capital = await AppAPI.obtenerCapital();
            if (!capital.bolsa) capital.bolsa = [];
            capital.bolsa.push({ emisor: emi, monto: mon, tasa: tas, vencimiento: ven, tipo_pago: pag });
            await AppAPI.guardarCapital(capital);
            this.showToast("Inversión de bolsa guardada.");
            await this.render('capital');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    async handleEliminarBolsa(idx) {
        if (confirm("¿Liquidar esta inversión de bolsa?")) {
            const capital = await AppAPI.obtenerCapital();
            capital.bolsa.splice(idx, 1);
            await AppAPI.guardarCapital(capital);
            this.showToast("Inversión liquidada.");
            await this.render('capital');
        }
    }

    async handleAgregarPropiedad(e) {
        e.preventDefault();
        const tip = document.getElementById('pro_tip').value;
        const sub = document.getElementById('pro_sub').value;
        const nom = document.getElementById('pro_nom').value;
        const val = Number(document.getElementById('pro_val').value);

        try {
            const capital = await AppAPI.obtenerCapital();
            if (!capital.propiedades) capital.propiedades = { inmobiliario: [], vehiculos: [], maquinaria: [] };
            if (!capital.propiedades[tip]) capital.propiedades[tip] = [];
            
            capital.propiedades[tip].push({
                id: Date.now().toString(),
                subtipo: sub,
                nombre: nom,
                valor_estimado: val
            });
            await AppAPI.guardarCapital(capital);
            this.showToast("Propiedad registrada.");
            await this.render('capital');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    async handleEliminarPropiedad(tipo, id) {
        if (confirm("¿Eliminar este bien del capital?")) {
            const capital = await AppAPI.obtenerCapital();
            if (capital.propiedades && capital.propiedades[tipo]) {
                capital.propiedades[tipo] = capital.propiedades[tipo].filter(p => p.id !== id);
                await AppAPI.guardarCapital(capital);
                this.showToast("Bien eliminado.");
                await this.render('capital');
            }
        }
    }

    async handleAgregarPrestamo(e) {
        e.preventDefault();
        const tip = document.getElementById('pre_tip').value;
        const ins = document.getElementById('pre_ins').value;
        const mon = Number(document.getElementById('pre_mon').value);
        const tas = Number(document.getElementById('pre_tas').value);
        const tot = document.getElementById('pre_tot') && document.getElementById('pre_tot').value ? Number(document.getElementById('pre_tot').value) : null;
        const pen = document.getElementById('pre_pen') && document.getElementById('pre_pen').value ? Number(document.getElementById('pre_pen').value) : null;
        const cuo = Number(document.getElementById('pre_cuo').value);
        const dia = Number(document.getElementById('pre_dia').value);

        try {
            await AppAPI.crearPrestamo({
                tipo_prestamo: tip,
                monto_prestamo: mon,
                institucion_financiera: ins,
                tasa_actual: tas,
                cuotas_totales: tot,
                cuotas_pendientes: pen,
                monto_cuota: cuo,
                dia_pago: dia
            });
            this.showToast("Financiamiento registrado con éxito.");
            await this.render('prestamos');
        } catch (err) {
            // Captura y presentación de la excepción del backend de Rust
            this.showToast(err.toString(), 'error');
        }
    }

    async handlePagarCuota(id) {
        try {
            await AppAPI.pagarCuotaPrestamo(id);
            this.showToast("Abono de cuota registrado.");
            await this.render('prestamos');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    async handleEliminarPrestamo(id) {
        if (confirm("¿Deseas eliminar este registro de deuda?")) {
            try {
                await AppAPI.eliminarPrestamo(id);
                this.showToast("Registro eliminado.");
                await this.render('prestamos');
            } catch (err) {
                this.showToast(err.toString(), 'error');
            }
        }
    }

    async handleAgregarCategoria(e) {
        e.preventDefault();
        const nom = document.getElementById('cat_nom').value;
        try {
            await AppAPI.crearCategoria(nom);
            this.showToast("Categoría agregada.");
            await this.render('ajustes');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    async handleEliminarCategoria(id) {
        if (confirm("¿Eliminar esta categoría?")) {
            try {
                await AppAPI.eliminarCategoria(id);
                this.showToast("Categoría eliminada.");
                await this.render('ajustes');
            } catch (err) {
                this.showToast(err.toString(), 'error');
            }
        }
    }

    abrirEdicionFormal(iJsonStr) {
        const i = JSON.parse(iJsonStr);
        AppAPI.obtenerClientes().then(clientes => {
            const overlay = document.createElement('div');
            overlay.className = 'modal-overlay';
            overlay.id = `modal-edit-for-${i.id}`;
            overlay.innerHTML = `
                <div class="card" style="width: 450px; background: var(--bg-surface-opaque);">
                    <h3 style="font-family: var(--font-heading); margin-bottom: 0.6rem;">✏️ Corregir / Editar Factura</h3>
                    <form onsubmit="appUI.handleEdicionFormalSubmit(event, ${i.id})">
                        <div class="form-group">
                            <label>Número de Factura *</label>
                            <input type="text" id="edit_num_fac_${i.id}" class="form-control" value="${i.numero_factura}" required>
                        </div>
                        <div class="form-group">
                            <label>Fecha de Emisión *</label>
                            <input type="text" id="edit_fec_em_${i.id}" class="form-control" value="${i.fecha_emision}" required>
                        </div>
                        <div class="form-group">
                            <label>Cliente *</label>
                            <select id="edit_cli_select_${i.id}" class="form-control" required>
                                ${clientes.map(c => `<option value="${c.id}" ${c.id === i.cliente_id ? 'selected' : ''}>${c.nombre} (RNC: ${c.rnc})</option>`).join('')}
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
                            <button type="button" onclick="document.getElementById('modal-edit-for-${i.id}').remove()" class="btn btn-secondary">Cancelar</button>
                            <button type="submit" class="btn">Guardar Cambios</button>
                        </div>
                    </form>
                </div>
            `;
            document.body.appendChild(overlay);
        }).catch(err => this.showToast(err.toString(), 'error'));
    }

    async handleEdicionFormalSubmit(e, id) {
        e.preventDefault();
        const fac = document.getElementById(`edit_num_fac_${id}`).value;
        const fec = document.getElementById(`edit_fec_em_${id}`).value;
        const cliId = Number(document.getElementById(`edit_cli_select_${id}`).value);
        const mon = Number(document.getElementById(`edit_mon_tot_${id}`).value);
        const ret = Number(document.getElementById(`edit_ret_por_${id}`).value);

        try {
            await AppAPI.actualizarIngreso(id, fac, cliId, fec, mon, ret);
            this.showToast("Factura corregida exitosamente.");
            document.getElementById(`modal-edit-for-${id}`).remove();
            await this.render('ingresos');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    abrirEdicionLimitesTarjeta(tJsonStr) {
        const t = JSON.parse(tJsonStr);
        const overlay = document.createElement('div');
        overlay.className = 'modal-overlay';
        overlay.id = `modal-edit-tar-${t.id}`;
        overlay.innerHTML = `
            <div class="card" style="width: 450px; background: var(--bg-surface-opaque); max-height:90vh; overflow-y:auto;">
                <h3 style="font-family: var(--font-heading); margin-bottom: 0.6rem;">⚙️ Configurar Límites / Corte</h3>
                <p style="font-size:0.75rem; color:var(--text-secondary); margin-bottom:1rem;">${t.entidad} - ${t.nombre_tarjeta}</p>
                <form onsubmit="appUI.handleEdicionLimitesTarjetaSubmit(event, ${t.id})">
                    <h4 style="font-size:0.85rem; color:var(--accent-primary); margin-bottom:0.5rem; border-bottom:1px solid var(--border-color); padding-bottom:0.2rem;">Pesos (DOP)</h4>
                    <div class="form-row">
                        <div class="form-group">
                            <label>Límite DOP</label>
                            <input type="number" step="0.01" id="edit_lim_dop_${t.id}" class="form-control" value="${t.limite_pesos}">
                        </div>
                        <div class="form-group">
                            <label>Sobregiro DOP</label>
                            <input type="number" step="0.01" id="edit_sob_dop_${t.id}" class="form-control" value="${t.limite_sobregiro_pesos}">
                        </div>
                    </div>
                    <div class="form-group">
                        <label>Límite ajustado DOP <span style="font-weight:400; color:var(--text-muted);">— opcional</span></label>
                        <input type="number" step="0.01" id="edit_aju_dop_${t.id}" class="form-control" placeholder="Sin ajuste" value="${t.limite_ajustado_pesos ?? ''}">
                        <span style="font-size:0.68rem; color:var(--text-muted);">Tope que te impones por debajo del aprobado. Déjalo vacío si no lo usas.</span>
                    </div>
                    <div class="form-group">
                        <label>Balance Corte DOP</label>
                        <input type="number" step="0.01" id="edit_cor_dop_${t.id}" class="form-control" value="${t.balance_corte_pesos}">
                    </div>

                    <h4 style="font-size:0.85rem; color:#10b981; margin-top:1rem; margin-bottom:0.5rem; border-bottom:1px solid var(--border-color); padding-bottom:0.2rem;">Dólares (USD)</h4>
                    <div class="form-row">
                        <div class="form-group">
                            <label>Límite USD</label>
                            <input type="number" step="0.01" id="edit_lim_usd_${t.id}" class="form-control" value="${t.limite_dolares}">
                        </div>
                        <div class="form-group">
                            <label>Sobregiro USD</label>
                            <input type="number" step="0.01" id="edit_sob_usd_${t.id}" class="form-control" value="${t.limite_sobregiro_dolares}">
                        </div>
                    </div>
                    <div class="form-group">
                        <label>Límite ajustado USD <span style="font-weight:400; color:var(--text-muted);">— opcional</span></label>
                        <input type="number" step="0.01" id="edit_aju_usd_${t.id}" class="form-control" placeholder="Sin ajuste" value="${t.limite_ajustado_dolares ?? ''}">
                    </div>
                    <div class="form-group">
                        <label>Balance Corte USD</label>
                        <input type="number" step="0.01" id="edit_cor_usd_${t.id}" class="form-control" value="${t.balance_corte_dolares}">
                    </div>

                    <h4 style="font-size:0.85rem; color:var(--text-secondary); margin-top:1rem; margin-bottom:0.5rem; border-bottom:1px solid var(--border-color); padding-bottom:0.2rem;">Compras en divisa</h4>
                    <div class="form-group">
                        <label>¿Cómo liquida el emisor las compras en dólares?</label>
                        <select id="edit_pol_${t.id}" class="form-control">
                            <option value="origen" ${t.politica_liquidacion !== 'traduce' ? 'selected' : ''}>Se quedan en dólares</option>
                            <option value="traduce" ${t.politica_liquidacion === 'traduce' ? 'selected' : ''}>Las traduce a pesos días después</option>
                        </select>
                        <span style="font-size:0.68rem; color:var(--text-muted);">Si las traduce, el consumo queda pendiente hasta que indiques el importe en pesos que aparezca en tu estado.</span>
                    </div>

                    <div style="display:flex; justify-content:flex-end; gap:0.5rem; margin-top:1.2rem;">
                        <button type="button" onclick="document.getElementById('modal-edit-tar-${t.id}').remove()" class="btn btn-secondary">Cancelar</button>
                        <button type="submit" class="btn">Guardar Parámetros</button>
                    </div>
                </form>
            </div>
        `;
        document.body.appendChild(overlay);
    }

    async handleEdicionLimitesTarjetaSubmit(e, id) {
        e.preventDefault();
        const limDop = Number(document.getElementById(`edit_lim_dop_${id}`).value);
        const sobDop = Number(document.getElementById(`edit_sob_dop_${id}`).value);
        const corDop = Number(document.getElementById(`edit_cor_dop_${id}`).value);
        const limUsd = Number(document.getElementById(`edit_lim_usd_${id}`).value);
        const sobUsd = Number(document.getElementById(`edit_sob_usd_${id}`).value);
        const corUsd = Number(document.getElementById(`edit_cor_usd_${id}`).value);

        // Vacío es "sin ajuste"; cero es un tope deliberado. Se leen como texto
        // para no confundir ambos casos.
        const ajuDopTexto = document.getElementById(`edit_aju_dop_${id}`).value.trim();
        const ajuUsdTexto = document.getElementById(`edit_aju_usd_${id}`).value.trim();
        const ajuDop = ajuDopTexto === '' ? null : Number(ajuDopTexto);
        const ajuUsd = ajuUsdTexto === '' ? null : Number(ajuUsdTexto);

        if ((ajuDop !== null && ajuDop > limDop) || (ajuUsd !== null && ajuUsd > limUsd)) {
            this.showToast("El límite ajustado no puede superar al aprobado.", "error");
            return;
        }

        try {
            const politica = document.getElementById(`edit_pol_${id}`)?.value || 'origen';
            await AppAPI.actualizarLimitesTarjeta(id, limDop, limUsd, sobDop, sobUsd, corDop, corUsd, ajuDop, ajuUsd, politica);
            this.showToast("Parámetros actualizados correctamente.");
            document.getElementById(`modal-edit-tar-${id}`).remove();
            await this.render('tarjetas');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    // --- COBROS DE INGRESOS (MODALES) ---
    async abrirCobroFormal(id, sugerido) {
        try {
            const cuentas = await AppAPI.obtenerCuentas();
            const overlay = document.createElement('div');
            overlay.className = 'modal-overlay';
            overlay.id = `modal-cobro-for-${id}`;
            overlay.innerHTML = `
                <div class="card" style="width: 400px; background: var(--bg-surface-opaque);">
                    <h3 style="font-family: var(--font-heading); margin-bottom: 0.6rem;">💰 Registrar Cobro Factura</h3>
                    <p style="font-size: 0.8rem; color: var(--text-secondary); margin-bottom: 1.2rem;">
                        Monto neto sugerido a recibir: <strong>DOP ${this.formatMoney(sugerido)}</strong>
                    </p>
                    <form onsubmit="appUI.handleCobroFormalSubmit(event, ${id})">
                        <div class="form-group">
                            <label>Cuenta de Depósito *</label>
                            <select id="cob_ban_${id}" class="form-control" required>
                                <option value="" disabled selected>Seleccione cuenta...</option>
                                ${cuentas.map(c => `<option value="${c.nombre}">${c.nombre} (${c.divisa}) - Bal: ${c.divisa} ${this.formatMoney(c.balance_actual)}</option>`).join('')}
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
                            <button type="button" onclick="document.getElementById('modal-cobro-for-${id}').remove()" class="btn btn-secondary">Cancelar</button>
                            <button type="submit" class="btn">Cobrar</button>
                        </div>
                    </form>
                </div>
            `;
            document.body.appendChild(overlay);
        } catch (err) {
            this.showToast("Error al obtener cuentas: " + err.toString(), 'error');
        }
    }

    async handleCobroFormalSubmit(e, id) {
        e.preventDefault();
        const ban = document.getElementById(`cob_ban_${id}`).value;
        const fec = document.getElementById(`cob_fec_${id}`).value;
        const mon = Number(document.getElementById(`cob_mon_${id}`).value);

        try {
            await AppAPI.marcarIngresoPagado(id, ban, fec, mon);
            this.showToast("Factura marcada como pagada.");
            document.getElementById(`modal-cobro-for-${id}`).remove();
            await this.render('ingresos');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    async abrirCobroInformal(id, sugerido) {
        try {
            const cuentas = await AppAPI.obtenerCuentas();
            const overlay = document.createElement('div');
            overlay.className = 'modal-overlay';
            overlay.id = `modal-cobro-inf-${id}`;
            overlay.innerHTML = `
                <div class="card" style="width: 400px; background: var(--bg-surface-opaque);">
                    <h3 style="font-family: var(--font-heading); margin-bottom: 0.6rem; color: #10b981;">💰 Registrar Cobro Informal</h3>
                    <p style="font-size: 0.8rem; color: var(--text-secondary); margin-bottom: 1.2rem;">
                        Monto esperado: <strong>DOP ${this.formatMoney(sugerido)}</strong>
                    </p>
                    <form onsubmit="appUI.handleCobroInformalSubmit(event, ${id})">
                        <div class="form-group">
                            <label>Cuenta de Depósito *</label>
                            <select id="cob_ban_inf_${id}" class="form-control" required>
                                <option value="" disabled selected>Seleccione cuenta...</option>
                                ${cuentas.map(c => `<option value="${c.nombre}">${c.nombre} (${c.divisa}) - Bal: ${c.divisa} ${this.formatMoney(c.balance_actual)}</option>`).join('')}
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
                            <button type="button" onclick="document.getElementById('modal-cobro-inf-${id}').remove()" class="btn btn-secondary">Cancelar</button>
                            <button type="submit" class="btn" style="background: linear-gradient(135deg, #10b981, #059669); color:white;">Cobrar</button>
                        </div>
                    </form>
                </div>
            `;
            document.body.appendChild(overlay);
        } catch (err) {
            this.showToast("Error al obtener cuentas: " + err.toString(), 'error');
        }
    }

    async handleCobroInformalSubmit(e, id) {
        e.preventDefault();
        const ban = document.getElementById(`cob_ban_inf_${id}`).value;
        const fec = document.getElementById(`cob_fec_inf_${id}`).value;
        const mon = Number(document.getElementById(`cob_mon_inf_${id}`).value);

        try {
            await AppAPI.marcarInformalPagado(id, ban, fec, mon);
            this.showToast("Ingreso informal registrado como pagado.");
            document.getElementById(`modal-cobro-inf-${id}`).remove();
            await this.render('ingresos');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    formatMonthYearStr(myStr) {
        if (!myStr) return "";
        const [m, y] = myStr.split('/');
        const months = [
            "Enero", "Febrero", "Marzo", "Abril", "Mayo", "Junio",
            "Julio", "Agosto", "Septiembre", "Octubre", "Noviembre", "Diciembre"
        ];
        return `${months[parseInt(m) - 1]} ${y}`;
    }

    async handleSelectGastosMonth(val) {
        this.selectedGastosMonth = val;
        await this.renderGastos();
    }

    // --- FORMATEADOR ---
    formatMoney(val) {
        return parseFloat(val).toLocaleString('es-DO', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
    }
}

const appUI = new AppUI();
