// --- MICHELITOS TAURI - RENDERIZADO DINÁMICO DE INTERFAZ (HTML DE ESCRITORIO) ---

// Tasa de referencia para expresar en pesos un pasivo en dólares. Es una
// aproximación de presentación: no toca ningún saldo almacenado, solo permite
// sumar dos divisas en un total. Vive aquí porque la usan el resumen general y
// el de pasivos, y dos copias de una tasa se desincronizan.
const TASA_USD_A_DOP = 60.0;

class AppUI {
    contentContainer: HTMLElement;
    notifContainer: HTMLElement;
    /** Cancela los oyentes del menú de pasivos abierto, si hay uno. */
    _menuPasivoAbort: AbortController | null = null;

    /**
     * La tasa de referencia, ofrecida a las vistas extraídas como servicio
     * (`Referencias`). La constante sigue aquí mientras la lean métodos que aún
     * no se han extraído; dos copias de una tasa se desincronizan.
     */
    readonly tasaUsdADop = TASA_USD_A_DOP;

    /**
     * Diálogos de confirmación y de texto (asíncronos: en el WebView de Tauri 1.x
     * `confirm()` devuelve una promesa y `prompt()` devuelve `null`). Los asigna
     * `serviciosDesdeAppUI` antes de que se dibuje nada.
     */
    dialogos!: import('./ui/servicios').Dialogos;

    /** Vistas extraídas a `src/js/vistas/`, por ruta (las registra `composicion.ts`). */
    vistas = new Map<string, { render(): Promise<void> }>();

    constructor() {
        this.contentContainer = elemento('app-content');
        this.notifContainer = elemento('notification-container');
    }

    /**
     * Registra una vista extraída y cuelga sus manejadores de `appUI`, que es lo
     * que llaman los manejadores en línea de su HTML. Es el puente de la
     * migración: desaparece con esta clase.
     */
    registrarVista(ruta: string, vista: { render(): Promise<void> }, puente: object) {
        this.vistas.set(ruta, vista);
        Object.assign(this, puente);
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
            // Las vistas ya extraídas (src/js/vistas/) van primero; el `switch`
            // conserva las que siguen aquí.
            const registrada = this.vistas.get(route);
            if (registrada) {
                await registrada.render();
                return;
            }
            switch (route) {
                case 'tarjetas':
                    await this.renderTarjetas();
                    break;
                case 'ajustes':
                    await this.renderAjustes();
                    break;
                default:
                    // Una ruta que nadie reconoce cae en el Dashboard, como siempre.
                    await this.vistas.get('dashboard')?.render();
            }
        } catch (err) {
            this.contentContainer.innerHTML = `
                <div class="card" style="border-left: 4px solid var(--color-danger);">
                    <h3 style="color: var(--color-danger); margin-bottom: 0.5rem;">Error al renderizar el módulo</h3>
                    <p style="font-size: 0.9rem;">${String(err)}</p>
                </div>
            `;
        }
    }


    // --- RENDER: TARJETAS ---
    async renderTarjetas() {
        const tarjetas = await AppAPI.obtenerTarjetas();
        const cuentas = await AppAPI.obtenerCuentas();
        const bonificaciones = await AppAPI.obtenerBonificaciones();
        const prestamos = await AppAPI.obtenerPrestamos();

        const hoy = new Date();
        const hoyStr = hoy.getDate().toString().padStart(2, '0') + '/' + (hoy.getMonth() + 1).toString().padStart(2, '0') + '/' + hoy.getFullYear();

        const diaActual = hoy.getDate();
        const mesActual = hoy.getMonth();
        const anioActual = hoy.getFullYear();

        let mejorTarjeta = null as import('./tipos-ipc').Tarjeta | null;
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
            
            const diffMs = fechaProximoCorte.getTime() - hoy.getTime();
            const diffDays = Math.ceil(diffMs / (1000 * 60 * 60 * 24));
            
            (t as import('./tipos-ipc').Tarjeta & { dias_para_corte: number }).dias_para_corte = diffDays;
            
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
                            const pctDop = this.porcentajeUso(t.balance_pesos, limiteTotalDop);

                            const limiteTotalUsd = t.limite_efectivo_dolares + t.limite_sobregiro_dolares;
                            const disponibleUsd = t.disponible_dolares;
                            const pctUsd = this.porcentajeUso(t.balance_dolares, limiteTotalUsd);

                            const balDop = this.etiquetaBalanceTarjeta(t.balance_pesos, 'DOP');
                            const balUsd = this.etiquetaBalanceTarjeta(t.balance_dolares, 'USD');

                            const tarjetaAlDia = (t.balance_corte_pesos <= 0 && t.balance_corte_dolares <= 0);

                            // Una facilidad que cuelga de esta tarjeta se cobra dentro de su
                            // pago. Su saldo se contabiliza aparte en el patrimonio, así que
                            // si además entrara en el balance de la tarjeta se contaría dos
                            // veces. El aviso está aquí, junto al abono, porque es el momento
                            // en que se decide qué cifra registrar.
                            const facilidades = prestamos.filter(p => p.tarjeta_id === t.id);
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

                                    ${facilidades.length > 0 ? `
                                        <div style="background:rgba(255,193,7,0.08); border:1px solid rgba(255,193,7,0.3); border-radius:var(--radius-sm); padding:0.5rem 0.6rem; font-size:0.7rem; color:var(--text-secondary);">
                                            ⚠️ Esta tarjeta cobra
                                            ${facilidades.map(f => `<strong>${f.institucion_financiera}</strong> (cuota DOP ${this.formatMoney(f.monto_cuota)})`).join(', ')}.
                                            Al conciliar el balance, no incluyas la cuota si ya cuenta como saldo de la facilidad:
                                            se duplicaría en el patrimonio.
                                        </div>
                                    ` : ''}

                                    <!-- Pesos Section -->
                                    <div>
                                        <div style="display:flex; justify-content:space-between; font-size:0.75rem; color:var(--text-secondary); margin-bottom:0.2rem;">
                                            <span>🇩🇴 DOP (Uso: ${pctDop.toFixed(1)}%)</span>
                                            <span style="color:${balDop.color};">${balDop.texto}</span>
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
                                            <span style="color:${balUsd.color};">${balUsd.texto}</span>
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
                                                    ${cuentas.map(c => `<option value="${c.id}" data-divisa="${c.divisa}">${c.nombre} (${c.divisa}) - Bal: ${c.divisa} ${this.formatMoney(c.balance_actual)}</option>`).join('')}
                                                </select>
                                            </div>
                                            <div style="flex:1;">
                                                <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Tasa Cambio</label>
                                                <input type="number" step="0.01" id="pag_tasa_${t.id}" value="0.00" placeholder="0.00" class="form-control" style="padding:0.4rem; font-size:0.75rem;">
                                            </div>
                                            <button type="submit" class="btn" style="padding:0.4rem 0.6rem; font-size:0.75rem; height:fit-content; background: linear-gradient(135deg, #10b981, #059669); color:white; flex:1;">Abonar</button>
                                        </div>
                                    </form>

                                    <!-- Abonos registrados -->
                                    <button onclick="appUI.alternarAbonos(${t.id})" class="btn" style="padding:0.3rem; font-size:0.75rem; background:rgba(255,255,255,0.02); border:1px solid var(--border-color); color:var(--text-secondary); width:100%;">🧾 Abonos registrados</button>
                                    <div id="abonos_${t.id}" hidden style="font-size:0.75rem;"></div>

                                    <!-- Avance de efectivo -->
                                    <button onclick="appUI.alternarAvance(${t.id})" class="btn" style="padding:0.3rem; font-size:0.75rem; background:rgba(255,255,255,0.02); border:1px solid var(--border-color); color:var(--text-secondary); width:100%;">💵 Avance de efectivo</button>
                                    <form id="avance_${t.id}" hidden onsubmit="appUI.handleAvanceEfectivo(event, ${t.id})" style="font-size:0.75rem;">
                                        <div style="display:flex; flex-direction:column; gap:0.5rem;">
                                            <p style="font-size:0.7rem; color:var(--text-secondary); margin:0;">
                                                La tarjeta pone el dinero en una cuenta de ahorro. La deuda sube por el monto <strong>y</strong> su cargo; la cuenta recibe el monto sin el cargo.
                                            </p>
                                            <div style="display:flex; gap:0.4rem; align-items:flex-end;">
                                                <div style="flex:1.2;">
                                                    <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Fecha</label>
                                                    <input type="text" id="avc_fecha_${t.id}" value="${hoyStr}" placeholder="dd/mm/aaaa" class="form-control" style="padding:0.4rem; font-size:0.75rem;" required>
                                                </div>
                                                <div style="flex:1;">
                                                    <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Divisa</label>
                                                    <select id="avc_div_${t.id}" class="form-control" style="padding:0.4rem; font-size:0.75rem;" onchange="appUI.aplicarTipoAvance(${t.id})">
                                                        <option value="DOP">DOP</option>
                                                        <option value="USD">USD</option>
                                                    </select>
                                                </div>
                                                <div style="flex:1.2;">
                                                    <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Monto</label>
                                                    <input type="number" step="0.01" id="avc_monto_${t.id}" placeholder="0.00" class="form-control" style="padding:0.4rem; font-size:0.75rem;" required>
                                                </div>
                                            </div>
                                            <div>
                                                <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Cuenta que recibe *</label>
                                                <select id="avc_cuenta_${t.id}" class="form-control" style="padding:0.4rem; font-size:0.75rem; width:100%;" required>
                                                    <option value="">-- Selecciona --</option>
                                                    ${cuentas.map(c => `<option value="${c.id}" data-divisa="${c.divisa}">${c.nombre} (${c.divisa}) - Bal: ${c.divisa} ${this.formatMoney(c.balance_actual)}</option>`).join('')}
                                                </select>
                                            </div>
                                            <div style="display:flex; gap:0.4rem; align-items:flex-end;">
                                                <div style="flex:1.3;">
                                                    <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Cargo</label>
                                                    <select id="avc_tipo_${t.id}" class="form-control" style="padding:0.4rem; font-size:0.75rem; width:100%;" onchange="appUI.aplicarTipoAvance(${t.id})">
                                                        <option value="porcentaje">Porcentaje</option>
                                                        <option value="fijo">Monto fijo</option>
                                                        <option value="exonerado">Exonerado</option>
                                                    </select>
                                                </div>
                                                <div id="avc_valor_caja_${t.id}" style="flex:1;">
                                                    <label id="avc_valor_et_${t.id}" style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Porcentaje (%)</label>
                                                    <input type="number" step="0.01" id="avc_valor_${t.id}" placeholder="6.25" class="form-control" style="padding:0.4rem; font-size:0.75rem;">
                                                </div>
                                            </div>
                                            <div>
                                                <label style="font-size:0.65rem; color:var(--text-muted); display:block; margin-bottom:0.2rem;">Nota (opcional; por qué se exoneró, por ejemplo)</label>
                                                <input type="text" id="avc_nota_${t.id}" class="form-control" style="padding:0.4rem; font-size:0.75rem;">
                                            </div>
                                            <button type="submit" class="btn" style="padding:0.4rem 0.6rem; font-size:0.75rem; background: linear-gradient(135deg, #6366f1, #4f46e5); color:white;">Registrar avance</button>
                                        </div>
                                    </form>
                                    <button onclick="appUI.alternarAvances(${t.id})" class="btn" style="padding:0.3rem; font-size:0.75rem; background:rgba(255,255,255,0.02); border:1px solid var(--border-color); color:var(--text-secondary); width:100%;">🧾 Avances registrados</button>
                                    <div id="avances_${t.id}" hidden style="font-size:0.75rem;"></div>

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
        const fecha = elemento<Campo>('bon_fecha').value.trim();
        const tarjeta = Number(elemento<Campo>('bon_tarjeta').value);
        const divisa = elemento<Campo>('bon_divisa').value;
        const monto = Number(elemento<Campo>('bon_monto').value);
        const concepto = elemento<Campo>('bon_concepto').value.trim();

        if (!(monto > 0)) { this.showToast("El monto de la bonificación debe ser mayor que cero.", "error"); return; }
        if (!concepto) { this.showToast("Indica el concepto: distingue un cashback de una promoción o recompensa.", "error"); return; }

        try {
            await AppAPI.crearBonificacion(fecha, tarjeta, monto, divisa, concepto);
            this.showToast("Bonificación registrada. La deuda de la tarjeta se redujo.");
            await this.render('tarjetas');
        } catch (err) {
            this.showToast(String(err), 'error');
        }
    }

    async handleEliminarBonificacion(id) {
        try {
            await AppAPI.eliminarBonificacion(id);
            this.showToast("Bonificación revertida. La deuda vuelve a su valor anterior.");
            await this.render('tarjetas');
        } catch (err) {
            this.showToast(String(err), 'error');
        }
    }

    // --- PASIVOS: agregados y agrupación ---

    // --- PASIVOS: plantillas ---

    // --- RENDER: AJUSTES ---
    async renderAjustes() {
        const categorias = await AppAPI.obtenerCategorias();
        const clientes = await AppAPI.obtenerClientes();
        const cuentas = await AppAPI.obtenerCuentas();
        const gastos = await AppAPI.obtenerGastos();
        const informales = await AppAPI.obtenerIngresosInformales();
        const transacciones = await AppAPI.obtenerTransaccionesCuentas();
        const ingresos = await AppAPI.obtenerIngresos();
        // Sin respaldos legibles el resto de Ajustes debe seguir funcionando.
        const respaldos = await AppAPI.listarRespaldos().catch(() => []);

        this.contentContainer.innerHTML = `
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
                    <button onclick="appUI.handleCrearRespaldo(this)" class="btn" style="width:100%;">Respaldar ahora</button>
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
                           <button onclick="appUI.handleRestaurarRespaldo(this)" class="btn" style="width:100%;">Restaurar este respaldo</button>`}
                    <div id="restauracion_resultado" style="font-size:0.72rem; color:var(--text-muted); margin-top:0.75rem; word-break:break-all;"></div>
                </div>

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
                                    <strong>${c.nombre}</strong>
                                    ${c.entidad ? `<span style="font-size:0.7rem; color:var(--text-muted);"> · ${c.entidad}</span>` : ''}
                                    <br>
                                    <span style="font-size:0.75rem; color:var(--accent-primary); font-weight:bold;">${c.divisa} ${this.formatMoney(c.balance_actual)}</span>
                                    ${c.comision_pago_impuestos != null ? `<span style="font-size:0.7rem; color:var(--color-warning);" title="Tarifa fija por pagar impuestos desde esta cuenta"> · 🧾 ${this.formatMoney(c.comision_pago_impuestos)}</span>` : ''}
                                </div>
                                <div style="display:flex; gap:0.3rem;">
                                    <button onclick='appUI.abrirEdicionCuenta(${JSON.stringify(c).replace(/'/g, "&#39;")})' class="btn" style="padding:0.2rem 0.4rem; font-size:0.75rem;">✏️</button>
                                    <button onclick="appUI.handleEliminarCuenta(${c.id})" class="btn btn-danger" style="padding:0.2rem 0.4rem; font-size:0.75rem;">🗑️</button>
                                </div>
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
                    <button type="button" onclick="appUI.alternarCasosDeCorreccion()" class="btn btn-secondary" style="padding:0.25rem 0.6rem; font-size:0.75rem;">📋 Ver casos abiertos</button>
                </div>
                <div id="casos-correccion" hidden style="margin-bottom:1.2rem;"></div>
                
                <div style="display:flex; gap:0.5rem; margin-bottom: 1rem; border-bottom: 1px solid var(--border-color); padding-bottom: 0.5rem;">
                    <button type="button" onclick="elemento('corr-gastos').style.display='block'; elemento('corr-ingresos').style.display='none'; elemento('corr-trans').style.display='none'; this.className='btn'; elemento('btn-corr-ing').className='btn btn-secondary'; elemento('btn-corr-tra').className='btn btn-secondary';" id="btn-corr-gas" class="btn" style="padding: 0.35rem 0.75rem; font-size:0.8rem;">💸 Gastos</button>
                    <button type="button" onclick="elemento('corr-gastos').style.display='none'; elemento('corr-ingresos').style.display='block'; elemento('corr-trans').style.display='none'; this.className='btn'; elemento('btn-corr-gas').className='btn btn-secondary'; elemento('btn-corr-tra').className='btn btn-secondary';" id="btn-corr-ing" class="btn btn-secondary" style="padding: 0.35rem 0.75rem; font-size:0.8rem; border:none;">📈 Ingresos</button>
                    <button type="button" onclick="elemento('corr-gastos').style.display='none'; elemento('corr-ingresos').style.display='none'; elemento('corr-trans').style.display='block'; this.className='btn'; elemento('btn-corr-gas').className='btn btn-secondary'; elemento('btn-corr-ing').className='btn btn-secondary';" id="btn-corr-tra" class="btn btn-secondary" style="padding: 0.35rem 0.75rem; font-size:0.8rem; border:none;">🔄 Transferencias</button>
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

    async handleAgregarTarjeta(e) {
        e.preventDefault();
        const ent = elemento<Campo>('tar_ent').value;
        const nom = elemento<Campo>('tar_nom').value;
        const limDop = Number(elemento<Campo>('tar_lim_dop').value);
        const sobDop = Number(elemento<Campo>('tar_sob_dop').value);
        const balDop = Number(elemento<Campo>('tar_bal_dop').value);
        const corDop = Number(elemento<Campo>('tar_cor_dop').value);
        const limUsd = Number(elemento<Campo>('tar_lim_usd').value);
        const sobUsd = Number(elemento<Campo>('tar_sob_usd').value);
        const balUsd = Number(elemento<Campo>('tar_bal_usd').value);
        const corUsd = Number(elemento<Campo>('tar_cor_usd').value);
        const cor = Number(elemento<Campo>('tar_cor').value);
        const pag = Number(elemento<Campo>('tar_pag').value);

        try {
            await AppAPI.crearTarjeta(ent, nom, limDop, limUsd, sobDop, sobUsd, balDop, balUsd, corDop, corUsd, cor, pag);
            this.showToast("Tarjeta registrada.");
            await this.render('tarjetas');
        } catch (err) {
            this.showToast(String(err), 'error');
        }
    }

    /**
     * Rellena el monto del abono según el tipo elegido y la divisa activa.
     * El importe queda visible antes de confirmar, en lugar de resolverse de
     * forma opaca al enviar el formulario.
     */
    aplicarTipoAbono(id, cortePesos, corteDolares, balPesos, balDolares) {
        const tipo = elemento<Campo>(`pag_tipo_${id}`).value;
        const divisa = elemento<Campo>(`pag_div_${id}`).value;
        const campoMonto = elemento<HTMLInputElement>(`pag_monto_${id}`);

        if (tipo === 'personalizado') {
            campoMonto.readOnly = false;
            return;
        }

        const esDolares = divisa === 'USD';
        const saldo = tipo === 'corte'
            ? (esDolares ? corteDolares : cortePesos)
            : (esDolares ? balDolares : balPesos);

        const etiqueta = tipo === 'corte' ? 'al corte' : 'actual';

        // Con saldo a favor no hay nada que abonar: prefijar el negativo solo
        // conseguiría que el envío fallara con "el monto debe ser mayor que
        // cero", que no explica lo que de verdad ocurre.
        if (Number(saldo) < 0) {
            campoMonto.value = '0.00';
            campoMonto.readOnly = true;
            this.showToast(
                `La tarjeta tiene ${divisa} ${this.formatMoney(Math.abs(saldo))} a favor. No hay saldo ${etiqueta} que abonar.`,
                'info'
            );
            return;
        }

        campoMonto.value = Number(saldo).toFixed(2);
        campoMonto.readOnly = true;

        if (Number(saldo) === 0) {
            this.showToast(`No hay saldo ${etiqueta} en ${divisa}.`, 'info');
        }
    }

    /**
     * Despliega los abonos de una tarjeta para poder deshacer uno.
     *
     * Se cargan al abrir y no al pintar la vista: son un histórico que casi
     * nunca se mira, y traerlos para las siete tarjetas a la vez sería pagar
     * siempre por lo que se usa de vez en cuando.
     */
    async alternarAbonos(id) {
        const caja = buscar(`abonos_${id}`);
        if (!caja) return;

        if (!caja.hidden) {
            caja.hidden = true;
            return;
        }

        caja.hidden = false;
        caja.innerHTML = `<p style="color:var(--text-muted); padding:0.5rem;">Cargando…</p>`;

        try {
            const abonos = await AppAPI.obtenerAbonosTarjeta(id);
            if (abonos.length === 0) {
                caja.innerHTML = `<p style="color:var(--text-muted); padding:0.5rem;">Sin abonos registrados.</p>`;
                return;
            }

            caja.innerHTML = abonos.map(a => {
                // Un abono sin cuenta no movió ningún saldo de ahorro, y
                // deshacerlo tampoco lo hará. Decirlo evita que alguien
                // espere una devolución que no va a llegar.
                const origen = a.cuenta_nombre
                    ? a.cuenta_nombre
                    : 'sin cuenta asociada';
                const tasa = a.tasa_cambio && a.tasa_cambio !== 1
                    ? ` · tasa ${a.tasa_cambio}`
                    : '';
                return `
                    <div style="display:flex; justify-content:space-between; align-items:center; gap:0.5rem; padding:0.4rem 0.5rem; border-bottom:1px solid var(--border-color);">
                        <div>
                            <strong>${a.divisa} ${this.formatMoney(a.monto_pagado)}</strong>
                            <div style="font-size:0.7rem; color:var(--text-muted);">${a.fecha_pago} · ${origen}${tasa}</div>
                        </div>
                        <button onclick="appUI.handleRevertirAbono(${a.id}, ${id})" class="btn btn-danger" style="padding:0.2rem 0.45rem; font-size:0.7rem;" title="Deshacer este abono">↩︎</button>
                    </div>`;
            }).join('');
        } catch (err) {
            caja.innerHTML = `<p style="color:var(--color-danger); padding:0.5rem;">${String(err)}</p>`;
        }
    }

    /**
     * Deshace un abono, avisando de todo lo que va a mover.
     *
     * La confirmación enumera los tres efectos porque un abono no es una
     * fila: revertirlo repone deuda, devuelve dinero y borra la comisión.
     */
    async handleRevertirAbono(abonoId, tarjetaId) {
        const confirmado = await this.dialogos.confirmar(
            "¿Deshacer este abono?\n\n" +
            "Se repondrá la deuda de la tarjeta, volverá a la cuenta el importe con su comisión, " +
            "y se eliminará el gasto que la recogía.\n\n" +
            "El registro del abono desaparece."
        );
        if (!confirmado) return;

        try {
            const motivo = await this.pedirMotivoDeCorreccion(
                    `Vas a borrar este abono a tarjeta`,
                    'Esto **destruye el movimiento**: no queda un asiento que lo anule, solo el caso de auditoría que estás a punto de abrir.'
                );
            if (motivo === null) return;
            const resumen = await AppAPI.revertirAbonoTarjeta(abonoId, motivo);
            this.showToast(resumen);
            await this.render('tarjetas');
            // Se vuelve a abrir el desplegable para que se vea el resultado
            // en lugar de dejar al usuario frente a una tarjeta cerrada.
            await this.alternarAbonos(tarjetaId);
        } catch (err) {
            this.showToast(String(err), 'error');
        }
    }

    // --- AVANCES DE EFECTIVO ---

    /// Despliega el formulario de avance de una tarjeta.
    alternarAvance(id) {
        const forma = buscar(`avance_${id}`);
        if (!forma) return;
        forma.hidden = !forma.hidden;
        if (!forma.hidden) this.aplicarTipoAvance(id);
    }

    /// Ajusta el formulario a la divisa y al tipo de cargo elegidos.
    ///
    /// Solo se ofrecen las cuentas de la divisa del avance: el núcleo lo exige
    /// y es mejor no dejar elegir lo que se va a rechazar. Que la cuenta y el
    /// avance coincidan en divisa es una regla; aquí solo se evita el
    /// tropiezo, y el núcleo la sigue comprobando.
    aplicarTipoAvance(id) {
        const divisa = elemento<Campo>(`avc_div_${id}`).value;
        const tipo = elemento<Campo>(`avc_tipo_${id}`).value;
        const cuenta = elemento<HTMLSelectElement>(`avc_cuenta_${id}`);

        for (const opcion of cuenta.options) {
            if (!opcion.value) continue;
            const coincide = opcion.dataset.divisa === divisa;
            opcion.hidden = !coincide;
            opcion.disabled = !coincide;
        }
        if (cuenta.selectedOptions[0]?.disabled) cuenta.value = '';

        const caja = elemento(`avc_valor_caja_${id}`);
        const etiqueta = elemento(`avc_valor_et_${id}`);
        const valor = elemento<HTMLInputElement>(`avc_valor_${id}`);
        caja.hidden = tipo === 'exonerado';
        valor.required = tipo !== 'exonerado';
        if (tipo === 'porcentaje') {
            etiqueta.textContent = 'Porcentaje (%)';
            valor.placeholder = '6.25';
        } else if (tipo === 'fijo') {
            etiqueta.textContent = `Cargo fijo (${divisa})`;
            valor.placeholder = '0.00';
        } else {
            valor.value = '';
        }
    }

    /// Lo que el formulario dice del cargo, en la forma que el núcleo espera.
    ///
    /// El monto y el cargo fijo se mandan **como texto**, tal cual se
    /// escribieron; el porcentaje, que es una tasa y no un importe, como
    /// número.
    valoresDeAvance(id) {
        const tipo = elemento<Campo>(`avc_tipo_${id}`).value;
        const bruto = elemento<Campo>(`avc_valor_${id}`).value.trim();
        return {
            tipo,
            porcentaje: tipo === 'porcentaje' && bruto !== '' ? Number(bruto) : null,
            fijo: tipo === 'fijo' && bruto !== '' ? bruto : null,
        };
    }

    /// Registra un avance, enseñando antes lo que va a mover.
    ///
    /// El cargo se calcula en el núcleo —`simularAvanceEfectivo`—, no aquí:
    /// una regla en el HTML es una regla sin pruebas, y la cifra que se
    /// confirma tiene que ser exactamente la que se va a asentar.
    async handleAvanceEfectivo(e, id) {
        e.preventDefault();
        const fecha = elemento<Campo>(`avc_fecha_${id}`).value.trim();
        const divisa = elemento<Campo>(`avc_div_${id}`).value;
        const monto = elemento<Campo>(`avc_monto_${id}`).value.trim();
        const cuentaSel = elemento<HTMLSelectElement>(`avc_cuenta_${id}`);
        const cuentaId = cuentaSel.value;
        const nota = elemento<Campo>(`avc_nota_${id}`).value.trim();
        const { tipo, porcentaje, fijo } = this.valoresDeAvance(id);

        if (!cuentaId) {
            this.showToast("Elige la cuenta que recibe el avance.", "error");
            return;
        }

        try {
            const sim = await AppAPI.simularAvanceEfectivo(monto, divisa, tipo, porcentaje, fijo);
            const cargoTxt = tipo === 'porcentaje' ? `Cargo (${porcentaje}%)`
                : tipo === 'fijo' ? 'Cargo fijo'
                : 'Cargo (exonerado)';
            const ok = await this.dialogos.confirmar(
                `Avance de efectivo\n\n` +
                `Monto: ${divisa} ${this.formatMoney(sim.monto)} → ${cuentaSel.selectedOptions[0].text.split(' - ')[0]}\n` +
                `${cargoTxt}: ${divisa} ${this.formatMoney(sim.cargo)}\n` +
                `La deuda de la tarjeta sube: ${divisa} ${this.formatMoney(sim.a_la_tarjeta)}\n\n` +
                `¿Registrar?`
            );
            if (!ok) return;

            const resumen = await AppAPI.registrarAvanceEfectivo(
                id, cuentaId, fecha, monto, divisa, tipo, porcentaje, fijo, nota
            );
            this.showToast(resumen);
            await this.render('tarjetas');
        } catch (err) {
            this.showToast(String(err), 'error');
        }
    }

    /// Despliega los avances de una tarjeta para poder deshacer uno.
    ///
    /// Se cargan al abrir y no al pintar la vista, igual que los abonos: es un
    /// histórico que casi nunca se mira.
    async alternarAvances(id) {
        const caja = buscar(`avances_${id}`);
        if (!caja) return;
        if (!caja.hidden) {
            caja.hidden = true;
            return;
        }
        caja.hidden = false;
        caja.innerHTML = `<p style="color:var(--text-muted); padding:0.5rem;">Cargando…</p>`;

        try {
            const avances = await AppAPI.obtenerAvancesTarjeta(id);
            if (avances.length === 0) {
                caja.innerHTML = `<p style="color:var(--text-muted); padding:0.5rem;">Sin avances registrados.</p>`;
                return;
            }
            caja.innerHTML = avances.map(a => {
                const cargo = a.tipo_cargo === 'porcentaje' ? `cargo ${a.tasa}% = ${a.divisa} ${this.formatMoney(a.cargo)}`
                    : a.tipo_cargo === 'fijo' ? `cargo fijo ${a.divisa} ${this.formatMoney(a.cargo)}`
                    : 'exonerado';
                const nota = a.nota ? `<div style="font-size:0.7rem; color:var(--text-muted); font-style:italic;">${a.nota}</div>` : '';
                return `
                    <div style="display:flex; justify-content:space-between; align-items:center; gap:0.5rem; padding:0.4rem 0.5rem; border-bottom:1px solid var(--border-color);">
                        <div>
                            <strong>${a.divisa} ${this.formatMoney(a.monto)}</strong>
                            <div style="font-size:0.7rem; color:var(--text-muted);">${a.fecha} · ${a.cuenta_nombre} · ${cargo}</div>
                            ${nota}
                        </div>
                        <button onclick="appUI.handleRevertirAvance(${a.id}, ${id})" class="btn btn-danger" style="padding:0.2rem 0.45rem; font-size:0.7rem;" title="Deshacer este avance">↩︎</button>
                    </div>`;
            }).join('');
        } catch (err) {
            caja.innerHTML = `<p style="color:var(--color-danger); padding:0.5rem;">${String(err)}</p>`;
        }
    }

    /// Deshace un avance, avisando de todo lo que va a mover.
    async handleRevertirAvance(avanceId, tarjetaId) {
        const confirmado = await this.dialogos.confirmar(
            "¿Deshacer este avance de efectivo?\n\n" +
            "La deuda de la tarjeta bajará por el monto y su cargo, la cuenta devolverá el monto, " +
            "y se eliminará el gasto que recogía el cargo.\n\n" +
            "Si ya gastaste ese dinero, la cuenta quedará en negativo: es el estado verdadero, y no se recorta."
        );
        if (!confirmado) return;

        try {
            const motivo = await this.pedirMotivoDeCorreccion(
                `Vas a borrar este avance de efectivo`,
                'Esto **destruye el movimiento**: no queda un asiento que lo anule, solo el caso de auditoría que estás a punto de abrir.'
            );
            if (motivo === null) return;
            const resumen = await AppAPI.revertirAvanceEfectivo(avanceId, motivo);
            this.showToast(resumen);
            await this.render('tarjetas');
            await this.alternarAvances(tarjetaId);
        } catch (err) {
            this.showToast(String(err), 'error');
        }
    }

    async handleAbonoTarjeta(e, id) {
        e.preventDefault();

        const fec = elemento<Campo>(`pag_fecha_${id}`).value;
        const div = elemento<Campo>(`pag_div_${id}`).value;
        const mon = Number(elemento<Campo>(`pag_monto_${id}`).value);
        const cueId = elemento<Campo>(`pag_cuenta_${id}`).value;

        if (!(mon > 0)) {
            this.showToast("El monto del abono debe ser mayor que cero.", "error");
            return;
        }

        let tasaCambio = Number(elemento<Campo>(`pag_tasa_${id}`).value) || 0;
        if (cueId) {
            try {
                const cuentas = await AppAPI.obtenerCuentas();
                const cuenta = cuentas.find(c => c.id === Number(cueId));
                if (cuenta && cuenta.divisa === "DOP" && div === "USD") {
                    if (tasaCambio <= 0) {
                        const promptVal = await this.dialogos.preguntar(`Estás realizando un abono de USD ${mon} desde la cuenta en Pesos "${cuenta.nombre}".\nPor favor, ingresa la tasa de cambio (DOP por 1 USD):`, "60.0");
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
                this.showToast("Error al validar cuenta: " + String(err), 'error');
                return;
            }
        }

        try {
            await AppAPI.registrarPagoTarjeta(id, fec, mon, div, cueId ? Number(cueId) : null, tasaCambio);
            this.showToast("Abono a tarjeta guardado.");
            await this.render('tarjetas');
        } catch (err) {
            this.showToast(String(err), 'error');
        }
    }

    async handleAgregarCliente(e) {
        e.preventDefault();
        const nom = elemento<Campo>('cli_aj_nom').value;
        const rnc = elemento<Campo>('cli_aj_rnc').value;
        try {
            await AppAPI.crearCliente(rnc, nom);
            this.showToast("Cliente registrado.");
            await this.render('ajustes');
        } catch (err) {
            this.showToast(String(err), 'error');
        }
    }

    async handleEliminarCliente(id) {
        if (await this.dialogos.confirmar("¿Deseas eliminar este cliente?")) {
            try {
                await AppAPI.eliminarCliente(id);
                this.showToast("Cliente eliminado.");
                await this.render('ajustes');
            } catch (err) {
                this.showToast(String(err), 'error');
            }
        }
    }

    async handleAgregarCuenta(e) {
        e.preventDefault();
        const nom = elemento<Campo>('cue_aj_nom').value;
        const div = elemento<Campo>('cue_aj_div').value;
        const bal = Number(elemento<Campo>('cue_aj_bal').value);
        const ent = elemento<Campo>('cue_aj_ent').value;
        // Un campo en blanco es «no declarada», no cero: se envía nulo para
        // que la ausencia siga siendo distinguible de una tarifa gratuita.
        const comTexto = elemento<Campo>('cue_aj_com').value;
        // Como texto, tal cual se escribió: el céntimo lo deciden los dígitos (convención de 1.21.0).
        const com = comTexto.trim() === '' ? null : comTexto.trim();
        try {
            await AppAPI.crearCuenta(nom, div, bal, ent, com);
            this.showToast("Cuenta de ahorro registrada.");
            await this.render('ajustes');
        } catch (err) {
            this.showToast(String(err), 'error');
        }
    }

    /**
     * Corrige los datos que el titular declara sobre una cuenta.
     *
     * No ofrece el balance a propósito: moverlo sin dejar rastro sería la
     * única forma de que un saldo cambiara sin un asiento detrás.
     */
    async abrirEdicionCuenta(cuenta) {
        const nombre = await this.dialogos.preguntar(`Nombre de la cuenta:`, cuenta.nombre);
        if (nombre === null) return;

        const entidad = await this.dialogos.preguntar(
            `Entidad con la que se mantiene «${nombre.trim()}»:`,
            cuenta.entidad || ''
        );
        if (entidad === null) return;

        const comisionActual = cuenta.comision_pago_impuestos != null
            ? String(cuenta.comision_pago_impuestos)
            : '';
        const comision = await this.dialogos.preguntar(
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
            this.showToast("La comisión debe ser un importe válido.", "error");
            return;
        }
        if (valor !== null && valor.startsWith('-')) {
            this.showToast("La comisión no puede ser negativa.", "error");
            return;
        }

        try {
            await AppAPI.actualizarCuenta(cuenta.id, nombre, entidad, valor);
            this.showToast("Cuenta actualizada.");
            await this.render('ajustes');
        } catch (err) {
            this.showToast(String(err), 'error');
        }
    }

    async handleEliminarCuenta(id) {
        if (await this.dialogos.confirmar("¿Deseas eliminar esta cuenta de ahorro?")) {
            try {
                await AppAPI.eliminarCuenta(id);
                this.showToast("Cuenta eliminada.");
                await this.render('ajustes');
            } catch (err) {
                this.showToast(String(err), 'error');
            }
        }
    }

    /**
     * Despliega los casos de corrección abiertos.
     *
     * Existe para que el rastro se pueda leer: un registro de auditoría que
     * nadie puede consultar no audita nada. Se carga al abrir porque es un
     * histórico que se mira de vez en cuando.
     */
    async alternarCasosDeCorreccion() {
        const caja = buscar('casos-correccion');
        if (!caja) return;
        if (!caja.hidden) { caja.hidden = true; return; }

        caja.hidden = false;
        caja.innerHTML = `<p style="color:var(--text-muted); font-size:0.75rem; padding:0.5rem;">Cargando…</p>`;

        try {
            const casos = await AppAPI.obtenerCorrecciones();
            if (casos.length === 0) {
                caja.innerHTML = `<p style="color:var(--text-muted); font-size:0.75rem; padding:0.5rem;">Ningún caso abierto. Es la mejor cifra posible.</p>`;
                return;
            }
            caja.innerHTML = `
                <div style="max-height:220px; overflow-y:auto; border:1px solid var(--border-color); border-radius:var(--radius-sm);">
                    ${casos.map(c => `
                        <div style="padding:0.5rem 0.7rem; border-bottom:1px solid var(--border-color); font-size:0.75rem;">
                            <div style="display:flex; justify-content:space-between; gap:0.5rem;">
                                <strong style="color:var(--accent-primary);">${c.numero_caso}</strong>
                                <span style="color:var(--text-muted);">${c.fecha} · ${c.tipo}</span>
                            </div>
                            <div style="margin-top:0.2rem;">
                                ${c.descripcion}${c.importe != null ? ` — ${c.divisa || ''} ${this.formatMoney(c.importe)}` : ''}
                            </div>
                            <div style="margin-top:0.2rem; color:var(--text-secondary); font-style:italic;">${c.motivo}</div>
                        </div>
                    `).join('')}
                </div>`;
        } catch (err) {
            caja.innerHTML = `<p style="color:var(--color-danger); font-size:0.75rem; padding:0.5rem;">${String(err)}</p>`;
        }
    }

    /**
     * Pide el motivo de una corrección que mueve dinero.
     *
     * Es fricción deliberada, no un trámite: el punto no es facilitar la
     * operación sino **reducir cuántas veces hace falta**. Por eso el diálogo
     * dice qué se pierde y exige una frase, no una palabra.
     *
     * `consecuencia` la pone quien llama, porque borrar y corregir no hacen lo
     * mismo: uno destruye el movimiento y el otro mueve un saldo. Un texto
     * único para ambos mentiría en uno de los dos casos.
     *
     * Devuelve `null` si el titular se echa atrás.
     */
    async pedirMotivoDeCorreccion(queOcurre, consecuencia) {
        const motivo = await this.dialogos.preguntar(
            `${queOcurre}.\n\n${consecuencia}\n\n` +
            "Explica qué pasó, con una frase que siga teniendo sentido dentro de seis meses:"
        );
        if (motivo === null) return null;
        if (motivo.trim().length < 15) {
            this.showToast("El motivo es demasiado corto: explica qué pasó, no solo que pasó.", "error");
            return null;
        }
        return motivo;
    }

    async handleEliminarGastoCorr(id) {
        if (await this.dialogos.confirmar("¿Estás seguro de que deseas revertir y eliminar este gasto? Los balances asociados serán restaurados.")) {
            try {
                const motivo = await this.pedirMotivoDeCorreccion(
                    `Vas a borrar este gasto`,
                    'Esto **destruye el movimiento**: no queda un asiento que lo anule, solo el caso de auditoría que estás a punto de abrir.'
                );
                if (motivo === null) return;
                const caso = await AppAPI.eliminarGasto(id, motivo);
                this.showToast(`Gasto revertido y eliminado. Caso ${caso}.`);
                await this.render('ajustes');
            } catch (err) {
                this.showToast(String(err), 'error');
            }
        }
    }

    async handleEliminarIngresoInformalCorr(id) {
        if (await this.dialogos.confirmar("¿Estás seguro de que deseas revertir y eliminar este ingreso informal? El balance asociado (si ya fue cobrado en efectivo) será descontado.")) {
            try {
                const motivo = await this.pedirMotivoDeCorreccion(
                    `Vas a borrar este ingreso informal`,
                    'Esto **destruye el movimiento**: no queda un asiento que lo anule, solo el caso de auditoría que estás a punto de abrir.'
                );
                if (motivo === null) return;
                const caso = await AppAPI.eliminarIngresoInformal(id, motivo);
                this.showToast("Ingreso informal revertido y eliminado.");
                await this.render('ajustes');
            } catch (err) {
                this.showToast(String(err), 'error');
            }
        }
    }

    async handleEliminarIngresoCorr(id) {
        if (await this.dialogos.confirmar("¿Estás seguro de que deseas revertir y eliminar esta factura/ingreso formal?")) {
            try {
                const motivo = await this.pedirMotivoDeCorreccion(
                    `Vas a borrar esta factura`,
                    'Esto **destruye el movimiento**: no queda un asiento que lo anule, solo el caso de auditoría que estás a punto de abrir.'
                );
                if (motivo === null) return;
                const caso = await AppAPI.eliminarIngreso(id, motivo);
                this.showToast("Ingreso formal eliminado.");
                await this.render('ajustes');
            } catch (err) {
                this.showToast(String(err), 'error');
            }
        }
    }

    async handleEliminarTransaccionCuentaCorr(id) {
        if (await this.dialogos.confirmar("¿Estás seguro de que deseas revertir y eliminar esta transferencia? Los saldos de las cuentas origen y destino serán restaurados.")) {
            try {
                const motivo = await this.pedirMotivoDeCorreccion(
                    `Vas a borrar este traspaso entre cuentas`,
                    'Esto **destruye el movimiento**: no queda un asiento que lo anule, solo el caso de auditoría que estás a punto de abrir.'
                );
                if (motivo === null) return;
                const caso = await AppAPI.eliminarTransaccionCuenta(id, motivo);
                this.showToast("Transferencia revertida y eliminada.");
                await this.render('ajustes');
            } catch (err) {
                this.showToast(String(err), 'error');
            }
        }
    }

    /**
     * Toma un respaldo bajo demanda.
     *
     * Se deshabilita el botón mientras corre: un `VACUUM INTO` sobre una base
     * grande tarda, y sin esto se acumularían copias por impaciencia.
     */
    async handleCrearRespaldo(boton) {
        const salida = buscar('respaldo_resultado');
        boton.disabled = true;
        const textoOriginal = boton.textContent;
        boton.textContent = 'Respaldando…';
        try {
            const ruta = await AppAPI.crearRespaldo();
            this.showToast('Respaldo creado y verificado.');
            if (salida) salida.textContent = ruta;
        } catch (err) {
            this.showToast(String(err), 'error');
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
    async handleRestaurarRespaldo(boton) {
        const elegido = buscar<HTMLSelectElement>('respaldo_elegido');
        if (!elegido || !elegido.value) return;
        const etiqueta = elegido.options[elegido.selectedIndex].textContent;
        const nombre = elegido.value;

        if (!await this.dialogos.confirmar(
            `Se restaurará el respaldo:\n${etiqueta}\n\n` +
            `Todo lo registrado después de esa fecha se perderá de la vista (base y capital). ` +
            `Antes se guardará una copia del estado actual para poder deshacerlo.\n\n¿Restaurar?`
        )) return;

        boton.disabled = true;
        const textoOriginal = boton.textContent;
        boton.textContent = 'Restaurando…';
        try {
            const r = await AppAPI.restaurarRespaldo(nombre);
            this.showToast('Respaldo restaurado.');
            await this.render('ajustes');
            const salida = buscar('restauracion_resultado');
            if (salida) {
                salida.textContent =
                    `Restaurado: ${etiqueta}. ` +
                    (r.capital_restaurado ? 'El capital también. ' : 'Ese respaldo no traía capital: se conservó el actual. ') +
                    `Para deshacer, restaura la copia «antes de restaurar» más reciente.`;
            }
        } catch (err) {
            this.showToast(String(err), 'error');
            boton.disabled = false;
            boton.textContent = textoOriginal;
        }
    }

    async handleAgregarCategoria(e) {
        e.preventDefault();
        const nom = elemento<Campo>('cat_nom').value;
        try {
            await AppAPI.crearCategoria(nom);
            this.showToast("Categoría agregada.");
            await this.render('ajustes');
        } catch (err) {
            this.showToast(String(err), 'error');
        }
    }

    async handleEliminarCategoria(id) {
        if (await this.dialogos.confirmar("¿Eliminar esta categoría?")) {
            try {
                await AppAPI.eliminarCategoria(id);
                this.showToast("Categoría eliminada.");
                await this.render('ajustes');
            } catch (err) {
                this.showToast(String(err), 'error');
            }
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
                        <button type="button" onclick="elemento('modal-edit-tar-${t.id}').remove()" class="btn btn-secondary">Cancelar</button>
                        <button type="submit" class="btn">Guardar Parámetros</button>
                    </div>
                </form>
            </div>
        `;
        document.body.appendChild(overlay);
    }

    async handleEdicionLimitesTarjetaSubmit(e, id) {
        e.preventDefault();
        const limDop = Number(elemento<Campo>(`edit_lim_dop_${id}`).value);
        const sobDop = Number(elemento<Campo>(`edit_sob_dop_${id}`).value);
        const corDop = Number(elemento<Campo>(`edit_cor_dop_${id}`).value);
        const limUsd = Number(elemento<Campo>(`edit_lim_usd_${id}`).value);
        const sobUsd = Number(elemento<Campo>(`edit_sob_usd_${id}`).value);
        const corUsd = Number(elemento<Campo>(`edit_cor_usd_${id}`).value);

        // Vacío es "sin ajuste"; cero es un tope deliberado. Se leen como texto
        // para no confundir ambos casos.
        const ajuDopTexto = elemento<Campo>(`edit_aju_dop_${id}`).value.trim();
        const ajuUsdTexto = elemento<Campo>(`edit_aju_usd_${id}`).value.trim();
        const ajuDop = ajuDopTexto === '' ? null : Number(ajuDopTexto);
        const ajuUsd = ajuUsdTexto === '' ? null : Number(ajuUsdTexto);

        if ((ajuDop !== null && ajuDop > limDop) || (ajuUsd !== null && ajuUsd > limUsd)) {
            this.showToast("El límite ajustado no puede superar al aprobado.", "error");
            return;
        }

        try {
            const politica = buscar<Campo>(`edit_pol_${id}`)?.value || 'origen';
            await AppAPI.actualizarLimitesTarjeta(id, limDop, limUsd, sobDop, sobUsd, corDop, corUsd, ajuDop, ajuUsd, politica);
            this.showToast("Parámetros actualizados correctamente.");
            elemento(`modal-edit-tar-${id}`).remove();
            await this.render('tarjetas');
        } catch (err) {
            this.showToast(String(err), 'error');
        }
    }

    // --- FORMATEADOR ---
    formatMoney(val) {
        return parseFloat(val).toLocaleString('es-DO', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
    }

    /// Un balance de tarjeta negativo es saldo a favor del titular, no un uso
    /// negativo. Rotularlo "Uso: DOP -100.00" invita a leerlo como un error.
    etiquetaBalanceTarjeta(balance, divisa) {
        const esAFavor = Number(balance) < 0;
        return {
            texto: `${esAFavor ? 'A favor' : 'Uso'}: ${divisa} ${this.formatMoney(Math.abs(balance))}`,
            color: esAFavor ? '#10b981' : 'inherit',
        };
    }

    /// Porcentaje de uso acotado a [0, 100] para dibujar la barra. Un saldo a
    /// favor da negativo y una barra con anchura negativa no se renderiza.
    porcentajeUso(balance, limiteTotal) {
        if (!(limiteTotal > 0)) return 0;
        return Math.min(Math.max((balance / limiteTotal) * 100, 0), 100);
    }
}

const appUI = new AppUI();
