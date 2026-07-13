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
        const totalCertificados = (capital.certificados || []).reduce((sum, c) => sum + Number(c.monto || 0), 0);
        const totalBolsa = (capital.bolsa || []).reduce((sum, b) => sum + Number(b.monto || 0), 0);
        const totalInmueble = (capital.propiedades.inmobiliario || []).reduce((sum, p) => sum + Number(p.valor_estimado || 0), 0);
        const totalVehiculo = (capital.propiedades.vehiculos || []).reduce((sum, p) => sum + Number(p.valor_estimado || 0), 0);
        const totalMaquinaria = (capital.propiedades.maquinaria || []).reduce((sum, p) => sum + Number(p.valor_estimado || 0), 0);
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
                        <div style="font-size: 0.8rem; color: var(--text-secondary); text-transform: uppercase; letter-spacing: 0.05em;">Ingresos del Mes</div>
                        <div class="amount income" style="font-size: 1.6rem; margin-top: 0.2rem;">DOP ${this.formatMoney(totalIngresosMes)}</div>
                        <div style="font-size: 0.7rem; color: var(--text-muted); margin-top: 0.3rem;">
                            Cobrado: DOP ${this.formatMoney(totalRecibidoMes)} | Retenido: DOP ${this.formatMoney(totalRetenidoFormal)}
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
                                <div style="background: rgba(255,255,255,0.01); border: 1px solid var(--border-color); padding: 0.75rem; border-radius: var(--radius-sm); display:flex; justify-content:space-between; font-size:0.85rem;">
                                    <div><strong>${t.entidad}</strong> - ${t.nombre_tarjeta}</div>
                                    <span class="amount expense">DOP ${this.formatMoney(t.balance_actual)}</span>
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

    // --- RENDER: INGRESOS ---
    async renderIngresos() {
        const ingresos = await AppAPI.obtenerIngresos();
        const informales = await AppAPI.obtenerIngresosInformales();

        this.contentContainer.innerHTML = `
            <div class="section-title">
                <h1>Ingresos</h1>
                <span class="subtitle">Facturas formales y registro informal de flujos</span>
            </div>

            <div class="grid-3" style="grid-template-columns: 1fr 2fr; gap: 1.5rem;">
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
                                <input type="text" id="num_fac" class="form-control" placeholder="FAC-0001" required>
                            </div>
                            <div class="form-group">
                                <label for="fec_em">Fecha de Emisión *</label>
                                <input type="text" id="fec_em" class="form-control" placeholder="dd/mm/aaaa" required>
                            </div>
                            <div class="form-group">
                                <label for="cli_nom">Nombre Cliente *</label>
                                <input type="text" id="cli_nom" class="form-control" placeholder="Empresa Dominicana S.A." required>
                            </div>
                            <div class="form-group">
                                <label for="cli_rnc">RNC Cliente *</label>
                                <input type="text" id="cli_rnc" class="form-control" placeholder="131-XXXXXX" required>
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
                                <input type="text" id="fecha_inf" class="form-control" placeholder="dd/mm/aaaa" required>
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
                                        ${ingresos.map(i => `
                                            <tr>
                                                <td><strong>${i.numero_factura}</strong></td>
                                                <td>${i.cliente_nombre}<br><span style="font-size:0.75rem; color:var(--text-muted);">RNC: ${i.cliente_rnc}</span></td>
                                                <td>${i.fecha_emision}</td>
                                                <td class="amount">DOP ${this.formatMoney(i.monto_total)}</td>
                                                <td class="amount expense">DOP ${this.formatMoney(i.monto_retenido)}</td>
                                                <td><span class="badge ${i.estatus}">${i.estatus}</span></td>
                                                <td>
                                                    ${i.estatus === 'emitida' ? `
                                                        <button onclick="appUI.abrirCobroFormal(${i.id}, ${i.monto_total - i.monto_retenido})" class="btn" style="padding: 0.3rem 0.6rem; font-size:0.75rem;">💵 Cobrar</button>
                                                    ` : `
                                                        <span style="font-size:0.75rem; color:var(--text-muted); font-style:italic;">Dep: ${i.institucion_deposito}</span>
                                                    `}
                                                </td>
                                            </tr>
                                        `).join('')}
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

        const hoy = new Date();
        const mesAnioActual = "/" + (hoy.getMonth() + 1).toString().padStart(2, '0') + '/' + hoy.getFullYear();
        const gastosMes = gastos.filter(g => g.fecha.endsWith(mesAnioActual));
        const totalNeto = gastosMes.reduce((sum, g) => sum + g.monto, 0);
        const totalComisiones = gastosMes.reduce((sum, g) => sum + g.costo_adicional, 0);

        this.contentContainer.innerHTML = `
            <div class="section-title">
                <h1>Gastos y Egresos</h1>
                <span class="subtitle">Control presupuestario y comisiones financieras</span>
            </div>

            <div class="grid-3" style="grid-template-columns: 1fr 2fr; gap: 1.5rem;">
                <div class="card" style="height: fit-content;">
                    <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 1rem;">💸 Registrar Gasto</h3>
                    <form id="form-add-gasto" onsubmit="appUI.handleAgregarGasto(event)">
                        <div class="form-row">
                            <div class="form-group">
                                <label for="gas_fec">Fecha *</label>
                                <input type="text" id="gas_fec" class="form-control" placeholder="dd/mm/aaaa" required>
                            </div>
                            <div class="form-group">
                                <label for="gas_div">Divisa</label>
                                <select id="gas_div" class="form-control">
                                    <option value="DOP" selected>DOP</option>
                                    <option value="USD">USD</option>
                                </select>
                            </div>
                        </div>
                        <div class="form-row">
                            <div class="form-group">
                                <label for="gas_mon">Monto *</label>
                                    <input type="number" id="gas_mon" step="0.01" class="form-control" placeholder="0.00" required>
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
                            <select id="gas_met" class="form-control" onchange="appUI.toggleMetodoPago(this.value)" required>
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
                        <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 0.8rem;">📊 Resumen Mensual</h3>
                        <div style="display:grid; grid-template-columns: repeat(3, 1fr); gap:1rem; text-align:center;">
                            <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.8rem; border-radius:var(--radius-sm);">
                                <span style="font-size:0.75rem; color:var(--text-secondary);">Monto Neto</span>
                                <div class="amount" style="font-size:1.2rem; margin-top:0.2rem;">DOP ${this.formatMoney(totalNeto)}</div>
                            </div>
                            <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.8rem; border-radius:var(--radius-sm);">
                                <span style="font-size:0.75rem; color:var(--text-secondary);">Comisiones</span>
                                <div class="amount expense" style="font-size:1.2rem; margin-top:0.2rem;">DOP ${this.formatMoney(totalComisiones)}</div>
                            </div>
                            <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.8rem; border-radius:var(--radius-sm);">
                                <span style="font-size:0.75rem; color:var(--text-secondary);">Total Debitado</span>
                                <div class="amount" style="font-size:1.2rem; margin-top:0.2rem;">DOP ${this.formatMoney(totalNeto + totalComisiones)}</div>
                            </div>
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
                                        ${gastos.map(g => `
                                            <tr>
                                                <td><strong>${g.descripcion}</strong></td>
                                                <td>${g.categoria_nombre}</td>
                                                <td>${g.fecha}</td>
                                                <td style="text-transform:capitalize;">${g.metodo_pago}</td>
                                                <td class="amount">${g.divisa} ${this.formatMoney(g.monto)}</td>
                                                <td class="amount expense">${g.divisa} ${this.formatMoney(g.costo_adicional)}</td>
                                                <td class="amount" style="font-weight:bold;">${g.divisa} ${this.formatMoney(g.monto + g.costo_adicional)}</td>
                                            </tr>
                                        `).join('')}
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
        const lbtr = document.getElementById('gas_lbtr_container');
        
        if (tarjeta) tarjeta.style.display = val === 'tarjeta' ? 'block' : 'none';
        if (lbtr) lbtr.style.display = val === 'transferencia' ? 'block' : 'none';
    }

    // --- RENDER: TARJETAS ---
    async renderTarjetas() {
        const tarjetas = await AppAPI.obtenerTarjetas();

        this.contentContainer.innerHTML = `
            <div class="section-title">
                <h1>Tarjetas de Crédito</h1>
                <span class="subtitle">Monitoreo de consumos, abonos y alertas de corte</span>
            </div>

            <div class="grid-3" style="grid-template-columns: 1fr 2fr; gap:1.5rem;">
                <!-- Formulario -->
                <div class="card" style="height:fit-content;">
                    <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 1rem;">💳 Nueva Tarjeta</h3>
                    <form id="form-add-tarjeta" onsubmit="appUI.handleAgregarTarjeta(event)">
                        <div class="form-group">
                            <label for="tar_ent">Banco Emisor *</label>
                            <input type="text" id="tar_ent" class="form-control" placeholder="Banco Popular..." required>
                        </div>
                        <div class="form-group">
                            <label for="tar_nom">Nombre Tarjeta *</label>
                            <input type="text" id="tar_nom" class="form-control" placeholder="Visa Infinite..." required>
                        </div>
                        <div class="form-row">
                            <div class="form-group">
                                <label for="tar_lim">Límite (DOP) *</label>
                                <input type="number" id="tar_lim" step="0.01" class="form-control" placeholder="0.00" required>
                            </div>
                            <div class="form-group">
                                <label for="tar_bal">Balance Actual *</label>
                                <input type="number" id="tar_bal" step="0.01" class="form-control" placeholder="0.00" required>
                            </div>
                        </div>
                        <div class="form-row">
                            <div class="form-group">
                                <label for="tar_cor">Día Corte (1-31) *</label>
                                <input type="number" id="tar_cor" min="1" max="31" class="form-control" placeholder="15" required>
                            </div>
                            <div class="form-group">
                                <label for="tar_pag">Día Vence (1-31) *</label>
                                <input type="number" id="tar_pag" min="1" max="31" class="form-control" placeholder="5" required>
                            </div>
                        </div>
                        <button type="submit" class="btn" style="width:100%; margin-top:0.5rem;">🚀 Registrar Tarjeta</button>
                    </form>
                </div>

                <!-- Tarjetas -->
                <div style="display:flex; flex-direction:column; gap:1.5rem;">
                    ${tarjetas.length > 0 ? `
                        <div class="grid-2">
                            ${tarjetas.map(t => {
                                const porcentaje = Math.min(((t.balance_actual / t.limite) * 100), 100);
                                return `
                                    <div class="card" style="display:flex; flex-direction:column; gap:0.8rem;">
                                        <div style="display:flex; justify-content:space-between; border-bottom:1px solid var(--border-color); padding-bottom:0.5rem;">
                                            <div>
                                                <strong>${t.entidad}</strong>
                                                <div style="font-size:0.75rem; color:var(--text-muted);">${t.nombre_tarjeta}</div>
                                            </div>
                                            <span class="amount expense" style="font-size:1.25rem;">DOP ${this.formatMoney(t.balance_actual)}</span>
                                        </div>

                                        <!-- Barra de porcentaje -->
                                        <div>
                                            <div style="display:flex; justify-content:space-between; font-size:0.75rem; color:var(--text-secondary); margin-bottom:0.25rem;">
                                                <span>Uso: ${porcentaje.toFixed(1)}%</span>
                                                <span>Límite: DOP ${this.formatMoney(t.limite)}</span>
                                            </div>
                                            <div style="width:100%; height:6px; background:rgba(255,255,255,0.05); border-radius:3px; overflow:hidden;">
                                                <div style="width: ${porcentaje}%; height:100%; background: ${porcentaje > 85 ? 'var(--color-danger)' : 'var(--accent-primary)'};"></div>
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
                                        <form onsubmit="appUI.handleAbonoTarjeta(event, ${t.id})" style="display:flex; gap:0.4rem; border-top:1px dashed var(--border-color); padding-top:0.8rem; margin-top:0.3rem; align-items:flex-end;">
                                            <div style="flex:1;">
                                                <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Fecha</label>
                                                <input type="text" placeholder="dd/mm/aaaa" class="form-control" style="padding:0.4rem; font-size:0.75rem;" required>
                                            </div>
                                            <div style="flex:1;">
                                                <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Monto DOP</label>
                                                <input type="number" step="0.01" placeholder="0.00" class="form-control" style="padding:0.4rem; font-size:0.75rem;" required>
                                            </div>
                                            <button type="submit" class="btn" style="padding:0.4rem 0.8rem; font-size:0.75rem; height:fit-content;">Abonar</button>
                                        </form>
                                    </div>
                                `;
                            }).join('')}
                        </div>
                    ` : `
                        <p style="color:var(--text-muted); text-align:center; padding:2rem;">No hay tarjetas registradas.</p>
                    `}
                </div>
            </div>
        `;
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

            <div class="grid-3" style="grid-template-columns: 1fr 2fr; gap:1.5rem;">
                <div class="card" style="height:fit-content;">
                    <h3 style="font-family: var(--font-heading); font-size:1.15rem; margin-bottom: 1rem;">🔄 Nueva Suscripción</h3>
                    ${tarjetas.length > 0 ? `
                        <form id="form-add-suscripcion" onsubmit="appUI.handleAgregarSuscripcion(event)">
                            <div class="form-group">
                                <label for="sus_pla">Servicio / Plataforma *</label>
                                <input type="text" id="sus_pla" class="form-control" placeholder="Netflix, Spotify, AWS..." required>
                            </div>
                            <div class="form-row">
                                <div class="form-group">
                                    <label for="sus_mon">Monto DOP *</label>
                                    <input type="number" id="sus_mon" step="0.01" class="form-control" placeholder="0.00" required>
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
                                            <td>${s.entidad} (${s.nombre_tarjeta})</td>
                                            <td class="amount expense">DOP ${this.formatMoney(s.monto)}</td>
                                            <td>
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

    // --- RENDER: CAPITAL ---
    async renderCapital() {
        const capital = await AppAPI.obtenerCapital();
        const inmobiliario = capital.propiedades.inmobiliario || [];
        const vehiculos = capital.propiedades.vehiculos || [];
        const maquinaria = capital.propiedades.maquinaria || [];

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
                        <button type="submit" class="btn" style="width:100%; padding:0.5rem;">➕ Agregar Certificado</button>
                    </form>

                    <div style="display:flex; flex-direction:column; gap:0.5rem; max-height:220px; overflow-y:auto;">
                        ${(capital.certificados || []).map((c, i) => `
                            <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.6rem; border-radius:4px; font-size:0.75rem;">
                                <div style="display:flex; justify-content:space-between; font-weight:bold;">
                                    <span>${c.banco}</span>
                                    <span>DOP ${this.formatMoney(c.monto)}</span>
                                </div>
                                <div style="display:flex; justify-content:space-between; color:var(--text-secondary); margin-top:0.2rem;">
                                    <span>Tasa: ${c.tasa}% | Vence: ${c.vencimiento}</span>
                                    <button onclick="appUI.handleEliminarCertificado(${i})" style="background:none; border:none; color:var(--color-danger); cursor:pointer;">Eliminar</button>
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
                        <button type="submit" class="btn" style="width:100%; padding:0.5rem;">➕ Agregar Inversión</button>
                    </form>

                    <div style="display:flex; flex-direction:column; gap:0.5rem; max-height:220px; overflow-y:auto;">
                        ${(capital.bolsa || []).map((b, i) => `
                            <div style="background:rgba(255,255,255,0.01); border:1px solid var(--border-color); padding:0.6rem; border-radius:4px; font-size:0.75rem;">
                                <div style="display:flex; justify-content:space-between; font-weight:bold;">
                                    <span>${b.emisor}</span>
                                    <span>DOP ${this.formatMoney(b.monto)}</span>
                                </div>
                                <div style="display:flex; justify-content:space-between; color:var(--text-secondary); margin-top:0.2rem;">
                                    <span>Tasa: ${b.tasa}% | Vence: ${b.vencimiento}</span>
                                    <button onclick="appUI.handleEliminarBolsa(${i})" style="background:none; border:none; color:var(--color-danger); cursor:pointer;">Eliminar</button>
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

            <div class="grid-3" style="grid-template-columns: 1fr 2fr; gap:1.5rem;">
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
                                                ${p.tipo_prestamo === 'flexible' && p.cuotas_totales === null ? `
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
        const totalCertificados = (capital.certificados || []).reduce((sum, c) => sum + Number(c.monto || 0), 0);
        const totalBolsa = (capital.bolsa || []).reduce((sum, b) => sum + Number(b.monto || 0), 0);
        const totalInmueble = (capital.propiedades.inmobiliario || []).reduce((sum, p) => sum + Number(p.valor_estimado || 0), 0);
        const totalVehiculo = (capital.propiedades.vehiculos || []).reduce((sum, p) => sum + Number(p.valor_estimado || 0), 0);
        const totalMaquinaria = (capital.propiedades.maquinaria || []).reduce((sum, p) => sum + Number(p.valor_estimado || 0), 0);
        
        const totalActivos = totalCertificados + totalBolsa + totalInmueble + totalVehiculo + totalMaquinaria;

        // 2. Pasivos
        const totalTarjetas = tarjetas.reduce((sum, t) => sum + t.balance_actual, 0);
        
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

        // 4. Balance del Mes
        const ingresosMes = ingresos.filter(i => i.fecha_emision.endsWith(mesAnioActual)).reduce((sum, i) => sum + i.monto_total, 0);
        const informalesMes = informales.filter(inf => inf.fecha.endsWith(mesAnioActual)).reduce((sum, inf) => sum + inf.monto, 0);
        const ingresosTotales = ingresosMes + informalesMes;

        const gastosTotales = gastos.filter(g => g.fecha.endsWith(mesAnioActual)).reduce((sum, g) => sum + g.monto + g.costo_adicional, 0);
        const balanceNeto = ingresosTotales - gastosTotales;

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
                        <span>📊 Balance Mensual</span>
                        <span class="amount ${balanceNeto >= 0 ? 'income' : 'expense'}">DOP ${this.formatMoney(balanceNeto)}</span>
                    </h3>
                    <div style="display:flex; flex-direction:column; gap:0.6rem; font-size:0.85rem;">
                        <div style="display:flex; justify-content:space-between; background:rgba(255,255,255,0.01); padding:0.5rem; border-radius:4px;">
                            <span>Ingresos Totales (Este Mes)</span>
                            <strong>DOP ${this.formatMoney(ingresosTotales)}</strong>
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

        this.contentContainer.innerHTML = `
            <div class="section-title">
                <h1>Ajustes</h1>
                <span class="subtitle">Configuraciones de catálogos y entorno nativo</span>
            </div>

            <div class="grid-3" style="grid-template-columns: 1fr 2fr; gap:1.5rem;">
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
                                ${c.nombre !== 'Otros' ? `
                                    <button onclick="appUI.handleEliminarCategoria(${c.id})" class="btn btn-danger" style="padding:0.2rem 0.4rem; font-size:0.75rem;">🗑️</button>
                                ` : `
                                    <span style="font-size:0.7rem; color:var(--text-muted); font-style:italic;">Sistema</span>
                                `}
                            </div>
                        `).join('')}
                    </div>
                </div>

                <div class="card" style="height:fit-content;">
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
            </div>
        `;
    }

    // --- MANEJADORES DE ENTRADAS ---
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

        try {
            await AppAPI.crearGasto({
                fecha: fec,
                monto: mon,
                divisa: div,
                descripcion: des,
                categoria_id: cat,
                metodo_pago: met,
                es_lbtr: lbtr,
                tarjeta_id: met === 'tarjeta' ? tar : null
            });
            this.showToast("Gasto registrado con éxito.");
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
        const lim = Number(document.getElementById('tar_lim').value);
        const bal = Number(document.getElementById('tar_bal').value);
        const cor = Number(document.getElementById('tar_cor').value);
        const pag = Number(document.getElementById('tar_pag').value);

        try {
            await AppAPI.crearTarjeta(ent, nom, lim, bal, cor, pag);
            this.showToast("Tarjeta registrada.");
            await this.render('tarjetas');
        } catch (err) {
            this.showToast(err.toString(), 'error');
        }
    }

    async handleAbonoTarjeta(e, id) {
        e.preventDefault();
        const inputs = e.target.querySelectorAll('input');
        const fec = inputs[0].value;
        const mon = Number(inputs[1].value);

        try {
            await AppAPI.registrarPagoTarjeta(id, fec, mon);
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
        const fre = document.getElementById('sus_fre').value;
        const tar = Number(document.getElementById('sus_tar').value);

        try {
            await AppAPI.crearSuscripcion(pla, mon, tar, fre);
            this.showToast("Suscripción recurrente guardada.");
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

    async handleAgregarCertificado(e) {
        e.preventDefault();
        const ban = document.getElementById('cer_ban').value;
        const mon = Number(document.getElementById('cer_mon').value);
        const tas = Number(document.getElementById('cer_tas').value);
        const ven = document.getElementById('cer_ven').value;

        try {
            const capital = await AppAPI.obtenerCapital();
            if (!capital.certificados) capital.certificados = [];
            capital.certificados.push({ banco: ban, monto: mon, tasa: tas, vencimiento: ven });
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

        try {
            const capital = await AppAPI.obtenerCapital();
            if (!capital.bolsa) capital.bolsa = [];
            capital.bolsa.push({ emisor: emi, monto: mon, tasa: tas, vencimiento: ven });
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

    // --- COBROS DE INGRESOS (MODALES) ---
    abrirCobroFormal(id, sugerido) {
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
                        <label>Banco de Depósito *</label>
                        <input type="text" id="cob_ban_${id}" class="form-control" placeholder="Banco Popular..." required>
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

    abrirCobroInformal(id, sugerido) {
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
                        <label>Banco de Depósito *</label>
                        <input type="text" id="cob_ban_inf_${id}" class="form-control" placeholder="Banco..." required>
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

    // --- FORMATEADOR ---
    formatMoney(val) {
        return parseFloat(val).toLocaleString('es-DO', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
    }
}

const appUI = new AppUI();
