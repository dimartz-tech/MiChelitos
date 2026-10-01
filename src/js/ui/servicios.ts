// Servicios compartidos que reciben las vistas por inyección (diseño «B» de
// `division_de_ui_limpia.md`).
//
// Una vista no importa globales ni toca `window`: declara en su constructor lo
// que necesita, con estas interfaces. En la aplicación se le pasan los reales;
// en una prueba de Node, dobles. Este archivo tiene los **tipos** y las
// implementaciones reales de cada servicio (la clase `AppUI`, de la que salieron,
// se retiró en la 1.65.0).
//
// Es un módulo ES (`composicion.ts` lo importa); las vistas lo importan con
// `import type`, que no deja rastro en el JavaScript compilado.

import { escaparHtml } from '../nucleo/html.js';

/**
 * Tipo de aviso. Se distingue por el icono (✅ el éxito, ⚠️ el resto) y por la clase
 * `toast-<tipo>`. `'info'` lo usa Tarjetas para explicar por qué un abono no
 * propone importe (saldo a favor o saldo cero): no es un error ni un éxito.
 */
export type TipoAviso = 'success' | 'error' | 'info';

/** Avisos emergentes (antes `appUI.showToast`). */
export interface Avisos {
    mostrar(mensaje: string, tipo?: TipoAviso): void;
}

/** Formato de importes (antes `appUI.formatMoney`). */
export interface Formato {
    importe(valor: number | string): string;
}

/** Cómo una vista pide dibujar una pestaña (antes `appUI.render(ruta)`). */
export interface Enrutador {
    mostrar(ruta: string): Promise<void>;
}

/** El contenedor donde la vista dibuja (antes `appUI.contentContainer`). */
export interface Pantalla {
    readonly contenido: { innerHTML: string };
}

/** Acceso al DOM por id (antes las globales `elemento()` y `buscar()`). */
export interface Dom {
    elemento<T extends HTMLElement = HTMLElement>(id: string): T;
    buscar<T extends HTMLElement = HTMLElement>(id: string): T | null;
}

/** La fecha de hoy, inyectada para que una prueba pueda fijarla. */
export type Reloj = () => Date;

/**
 * Los diálogos de confirmación y de entrada de texto (antes `confirm()` y
 * `prompt()` sueltos: hay 25 usos). **Asíncronos**: en el WebView de Tauri 1.x
 * `confirm()` devuelve una promesa —siempre «verdadera» en un `if`, así que
 * Cancelar no cancelaba— y `prompt()` devuelve `null` al instante sin mostrar
 * nada, de modo que toda corrección que pedía un motivo se abandonaba en
 * silencio. Quien llama los espera con `await`.
 */
export interface Dialogos {
    confirmar(mensaje: string): Promise<boolean>;
    preguntar(mensaje: string, porDefecto?: string): Promise<string | null>;
}

/**
 * La petición del motivo de una corrección que mueve dinero (antes
 * `appUI.pedirMotivoDeCorreccion`). Devuelve `null` si el titular se echa atrás.
 */
export interface Motivo {
    pedir(queOcurre: string, consecuencia: string): Promise<string | null>;
}

/**
 * Ventanas modales (antes nueve bloques idénticos en `ui.ts`: crear la capa
 * `modal-overlay`, ponerle su identificador y su contenido, y añadirla a la
 * página). El identificador sirve luego para cerrarla con `dom.elemento(id)`.
 */
export interface Modales {
    abrir(id: string, html: string): void;
}

/**
 * Un menú flotante de acciones junto a un botón (antes `abrirMenuPasivo` y
 * `cerrarMenuPasivo`, con el `AbortController` suelto en la clase). Solo hay
 * uno abierto a la vez; se cierra con el siguiente clic o con Escape. `ancla` es
 * el rectángulo del botón; `alElegir` recibe el `data-accion` del renglón pulsado,
 * ya con el menú cerrado.
 */
export interface MenuFlotante {
    abrir(opciones: { id: string; html: string; ancla: Pick<DOMRect, 'right' | 'top' | 'bottom'>; alElegir: (accion: string) => void }): void;
    cerrar(): void;
}

/**
 * Valores de referencia que varias vistas comparten (antes la constante global
 * `TASA_USD_A_DOP` de `ui.ts`). Inyectados: una prueba puede darles otro valor
 * y comprobar que la vista no lo tiene escrito dentro.
 */
export interface Referencias {
    /** Tasa de presentación para expresar en pesos un importe en dólares. No toca ningún saldo. */
    readonly tasaUsdADop: number;
}

/** Lo que cualquier vista puede pedir. Cada vista toma el subconjunto que usa. */
export interface ServiciosComunes {
    avisos: Avisos;
    formato: Formato;
    enrutador: Enrutador;
    pantalla: Pantalla;
    dom: Dom;
    ahora: Reloj;
    dialogos: Dialogos;
    motivo: Motivo;
    modales: Modales;
    menus: MenuFlotante;
    referencias: Referencias;
}

/** El acceso a la API, recortado a los comandos que usa una vista. */
export type ApiDe<K extends keyof typeof AppAPI> = Pick<typeof AppAPI, K>;

/** Forma de una vista: dibuja su pestaña. Los manejadores son propios de cada una. */
export interface Vista {
    render(): Promise<void>;
}

/** Escapa el texto de un mensaje y deja en negrita lo marcado con `**…**`. */
function mensajeAHtml(mensaje: string): string {
    return escaparHtml(mensaje).replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
}

/**
 * Los diálogos reales: una ventana dentro de la página. No dependen de lo que
 * el WebView haga con `confirm()`/`prompt()` (ver `Dialogos`), y admiten texto,
 * que el diálogo nativo de Tauri no ofrece. Escape cancela; Enter acepta; en una
 * confirmación el foco empieza en **Cancelar**, porque casi todas son borrados.
 */
export function crearDialogosDePagina(): Dialogos {
    function abrir<T>(
        mensaje: string,
        { conTexto, porDefecto, alResolver }: { conTexto: boolean; porDefecto?: string; alResolver: (aceptado: boolean, texto: string) => T },
    ): Promise<T> {
        return new Promise<T>(resolver => {
            const capa = document.createElement('div');
            capa.className = 'modal-overlay';
            capa.style.zIndex = '2000';
            capa.setAttribute('role', 'dialog');
            capa.setAttribute('aria-modal', 'true');
            capa.innerHTML = `
                <div class="card" style="width: 440px; max-width: 92vw; background: var(--bg-surface-opaque);">
                    <p data-dialogo="mensaje" style="white-space: pre-line; font-size: 0.9rem; line-height: 1.5; margin-bottom: 1rem;">${mensajeAHtml(mensaje)}</p>
                    ${conTexto ? '<input type="text" data-dialogo="texto" class="form-control" style="margin-bottom: 1rem;">' : ''}
                    <div style="display:flex; justify-content:flex-end; gap:0.5rem;">
                        <button type="button" data-dialogo="cancelar" class="btn btn-secondary">Cancelar</button>
                        <button type="button" data-dialogo="aceptar" class="btn">Aceptar</button>
                    </div>
                </div>
            `;
            const campo = capa.querySelector<HTMLInputElement>('[data-dialogo="texto"]');
            const aceptar = capa.querySelector<HTMLElement>('[data-dialogo="aceptar"]');
            const cancelar = capa.querySelector<HTMLElement>('[data-dialogo="cancelar"]');
            if (campo) campo.value = porDefecto ?? '';

            const cerrar = (aceptado: boolean): void => {
                const texto = campo?.value ?? '';
                capa.remove();
                resolver(alResolver(aceptado, texto));
            };
            if (aceptar) aceptar.onclick = () => cerrar(true);
            if (cancelar) cancelar.onclick = () => cerrar(false);
            capa.onkeydown = (e: KeyboardEvent) => {
                if (e.key === 'Escape') { e.preventDefault(); cerrar(false); }
                else if (e.key === 'Enter' && campo && e.target === campo) { e.preventDefault(); cerrar(true); }
            };

            document.body.appendChild(capa);
            if (campo) { campo.focus(); campo.select(); } else cancelar?.focus();
        });
    }

    return {
        confirmar: mensaje => abrir(mensaje, { conTexto: false, alResolver: aceptado => aceptado }),
        preguntar: (mensaje, porDefecto) =>
            abrir(mensaje, { conTexto: true, porDefecto, alResolver: (aceptado, texto) => (aceptado ? texto : null) }),
    };
}

/** Las ventanas modales reales: una capa que se añade al final de `<body>`. */
export const modalesDelNavegador: Modales = {
    abrir(id, html) {
        const overlay = document.createElement('div');
        overlay.className = 'modal-overlay';
        overlay.id = id;
        overlay.innerHTML = html;
        document.body.appendChild(overlay);
    },
};

/**
 * El menú flotante real. Conserva aquí el `AbortController` que retira de una
 * vez los dos listeners de `document`: con `removeEventListener` por separado es
 * fácil olvidar uno y dejarlos acumulándose cada vez que el menú se abre y se cierra.
 */
export function crearMenuFlotanteDelNavegador(): MenuFlotante {
    let abierto: HTMLElement | null = null;
    let control: AbortController | null = null;

    const cerrar = (): void => {
        abierto?.remove();
        abierto = null;
        control?.abort();
        control = null;
    };

    return {
        cerrar,
        abrir({ id, html, ancla, alElegir }) {
            cerrar();
            const menu = document.createElement('div');
            menu.id = id;
            menu.style.cssText = `position:fixed; z-index:1000; background:var(--bg-surface-opaque);
            border:1px solid rgba(255,255,255,0.1); border-radius:var(--radius-sm); padding:0.35rem;
            width:190px; box-shadow:var(--shadow-md); font-size:0.8rem;`;
            menu.innerHTML = html;

            menu.querySelectorAll<HTMLElement>('[data-accion]').forEach(el => {
                el.onmouseenter = () => { el.style.background = 'rgba(255,255,255,0.06)'; };
                el.onmouseleave = () => { el.style.background = 'none'; };
                el.onclick = () => {
                    const accion = el.dataset.accion ?? '';
                    cerrar();
                    alElegir(accion);
                };
            });

            document.body.appendChild(menu);
            abierto = menu;

            // Se coloca después de insertarlo: sin medirlo no se sabe si cabe.
            const alto = menu.offsetHeight;
            menu.style.left = `${Math.max(8, ancla.right - menu.offsetWidth)}px`;
            menu.style.top = `${ancla.bottom + alto > window.innerHeight ? Math.max(8, ancla.top - alto - 4) : ancla.bottom + 4}px`;

            const propio = new AbortController();
            control = propio;

            // Diferido un tick: el clic que abre el menú todavía está burbujeando
            // hacia `document`, y sin esto lo cerraría al instante.
            setTimeout(() => {
                if (propio.signal.aborted) return;
                document.addEventListener('click', () => cerrar(), { signal: propio.signal });
                document.addEventListener('keydown', e => {
                    if (e.key === 'Escape') cerrar();
                }, { signal: propio.signal });
            }, 0);
        },
    };
}

/**
 * Tasa de referencia para expresar en pesos un importe en dólares (antes la
 * constante de `ui.ts`). Es una aproximación de presentación: no toca ningún saldo
 * almacenado, solo permite sumar dos divisas en un total. Vive en un solo sitio
 * porque dos copias de una tasa se desincronizan.
 */
export const TASA_USD_A_DOP = 60.0;

export const referenciasDelNavegador: Referencias = { tasaUsdADop: TASA_USD_A_DOP };

/** Los importes se muestran como el resto de la aplicación: `1,234.50`. */
export const formatoDelNavegador: Formato = {
    importe: valor => parseFloat(String(valor)).toLocaleString('es-DO', { minimumFractionDigits: 2, maximumFractionDigits: 2 }),
};

/** Lo que `crearAvisos` necesita del navegador, inyectable para probarlo con un DOM de juguete. */
export interface EntornoDeAvisos {
    /** El contenedor donde se apilan los avisos. */
    contenedor: { appendChild(hijo: HTMLElement): unknown };
    crear?: () => HTMLElement;
    programar?: (accion: () => void, ms: number) => unknown;
}

/**
 * Los avisos emergentes reales (antes `appUI.showToast`): un `div` con el icono y
 * el mensaje que aparece, se queda cuatro segundos y se retira.
 */
export function crearAvisos({ contenedor, crear = () => document.createElement('div'), programar = (accion, ms) => setTimeout(accion, ms) }: EntornoDeAvisos): Avisos {
    return {
        mostrar(mensaje, tipo = 'success') {
            const aviso = crear();
            aviso.className = `toast toast-${tipo}`;
            aviso.innerHTML = `
            <span style="font-weight: bold;">${tipo === 'success' ? '✅' : '⚠️'}</span>
            <span>${escaparHtml(mensaje)}</span>
        `;
            contenedor.appendChild(aviso);

            programar(() => aviso.classList.add('show'), 100);

            programar(() => {
                aviso.classList.remove('show');
                programar(() => aviso.remove(), 400);
            }, 4000);
        },
    };
}

/**
 * La petición del motivo de una corrección que mueve dinero (antes
 * `appUI.pedirMotivoDeCorreccion`).
 *
 * Es fricción deliberada, no un trámite: el punto no es facilitar la operación sino
 * **reducir cuántas veces hace falta**. Por eso el diálogo dice qué se pierde y exige
 * una frase, no una palabra.
 *
 * `consecuencia` la pone quien llama, porque borrar y corregir no hacen lo mismo: uno
 * destruye el movimiento y el otro mueve un saldo. Un texto único para ambos mentiría
 * en uno de los dos casos.
 *
 * Devuelve `null` si el titular se echa atrás, o si el motivo es demasiado corto.
 */
export function crearMotivo(dialogos: Dialogos, avisos: Avisos): Motivo {
    return {
        async pedir(queOcurre, consecuencia) {
            const motivo = await dialogos.preguntar(
                `${queOcurre}.\n\n${consecuencia}\n\n` +
                "Explica qué pasó, con una frase que siga teniendo sentido dentro de seis meses:",
            );
            if (motivo === null) return null;
            if (motivo.trim().length < 15) {
                avisos.mostrar("El motivo es demasiado corto: explica qué pasó, no solo que pasó.", "error");
                return null;
            }
            return motivo;
        },
    };
}

/**
 * El enrutador de pestañas (antes `appUI.render`): muestra «Cargando…», le pide a la
 * vista registrada que dibuje, y si algo falla pinta el error en lugar de dejar la
 * pantalla a medias. **Una ruta que nadie reconoce cae en el Dashboard**, como
 * siempre: por eso una vista que deja de registrarse no da error sino que pinta el
 * Dashboard (lo vigila `pruebas/js/contrato/rutas.test.js`).
 */
export function crearEnrutador(pantalla: Pantalla, vistas: ReadonlyMap<string, Vista>): Enrutador {
    return {
        async mostrar(ruta) {
            pantalla.contenido.innerHTML = '<p style="color: var(--text-muted); text-align:center; padding: 2rem;">Cargando módulo nativo...</p>';

            try {
                await (vistas.get(ruta) ?? vistas.get('dashboard'))?.render();
            } catch (err) {
                pantalla.contenido.innerHTML = `
                <div class="card" style="border-left: 4px solid var(--color-danger);">
                    <h3 style="color: var(--color-danger); margin-bottom: 0.5rem;">Error al renderizar el módulo</h3>
                    <p style="font-size: 0.9rem;">${escaparHtml(String(err))}</p>
                </div>
            `;
            }
        },
    };
}
