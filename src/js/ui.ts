// --- MICHELITOS TAURI - RENDERIZADO DINÁMICO DE INTERFAZ (HTML DE ESCRITORIO) ---

// Tasa de referencia para expresar en pesos un pasivo en dólares. Es una
// aproximación de presentación: no toca ningún saldo almacenado, solo permite
// sumar dos divisas en un total. Vive aquí porque la usan el resumen general y
// el de pasivos, y dos copias de una tasa se desincronizan.
const TASA_USD_A_DOP = 60.0;

class AppUI {
    contentContainer: HTMLElement;
    notifContainer: HTMLElement;

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


    // --- PASIVOS: agregados y agrupación ---

    // --- PASIVOS: plantillas ---

    // --- AVANCES DE EFECTIVO ---

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

    // --- FORMATEADOR ---
    formatMoney(val) {
        return parseFloat(val).toLocaleString('es-DO', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
    }

}

const appUI = new AppUI();
