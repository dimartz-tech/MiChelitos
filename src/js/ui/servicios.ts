// Servicios compartidos que reciben las vistas por inyección (diseño «B» de
// `division_de_ui_limpia.md`).
//
// Una vista no importa globales ni toca `window`: declara en su constructor lo
// que necesita, con estas interfaces. En la aplicación se le pasan los reales;
// en una prueba de Node, dobles. Este archivo tiene los **tipos** y el cableado
// con la clase vieja `AppUI` mientras dure la migración.
//
// Es un módulo ES (`composicion.ts` lo importa); las vistas lo importan con
// `import type`, que no deja rastro en el JavaScript compilado.

/** Tipo de aviso: lo único que hoy distingue un toast. */
export type TipoAviso = 'success' | 'error';

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
 * Los diálogos nativos del navegador (antes `confirm()` y `prompt()` sueltos:
 * hay 25 usos). Inyectados para poder probar un borrado sin navegador.
 */
export interface Dialogos {
    confirmar(mensaje: string): boolean;
    preguntar(mensaje: string, porDefecto?: string): string | null;
}

/**
 * La petición del motivo de una corrección que mueve dinero (antes
 * `appUI.pedirMotivoDeCorreccion`). Devuelve `null` si el titular se echa atrás.
 */
export interface Motivo {
    pedir(queOcurre: string, consecuencia: string): string | null;
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

/**
 * Lo que `AppUI` (la clase vieja) ofrece mientras queden vistas sin extraer.
 * Es una interfaz estructural, no el tipo `AppUI`: así `servicios.ts` no
 * depende de `ui.ts`.
 */
export interface AppUIAntigua {
    contentContainer: HTMLElement;
    readonly tasaUsdADop: number;
    showToast(mensaje: string, tipo?: string): void;
    formatMoney(valor: number | string): string;
    render(ruta: string): Promise<void>;
    pedirMotivoDeCorreccion(queOcurre: string, consecuencia: string): string | null;
    registrarVista(ruta: string, vista: Vista, puente: object): void;
}

/** Los diálogos reales del navegador. */
export const dialogosDelNavegador: Dialogos = {
    confirmar: mensaje => window.confirm(mensaje),
    preguntar: (mensaje, porDefecto) => window.prompt(mensaje, porDefecto),
};

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

/** Conecta los servicios con la clase vieja. Desaparece con la última vista. */
export function serviciosDesdeAppUI(
    app: AppUIAntigua,
    dom: Dom,
    ahora: Reloj = () => new Date(),
    dialogos: Dialogos = dialogosDelNavegador,
    modales: Modales = modalesDelNavegador,
    menus: MenuFlotante = crearMenuFlotanteDelNavegador(),
): ServiciosComunes {
    return {
        avisos: { mostrar: (mensaje, tipo) => app.showToast(mensaje, tipo) },
        formato: { importe: valor => app.formatMoney(valor) },
        enrutador: { mostrar: ruta => app.render(ruta) },
        pantalla: { get contenido() { return app.contentContainer; } },
        dom,
        ahora,
        dialogos,
        motivo: { pedir: (queOcurre, consecuencia) => app.pedirMotivoDeCorreccion(queOcurre, consecuencia) },
        modales,
        menus,
        referencias: { tasaUsdADop: app.tasaUsdADop },
    };
}
