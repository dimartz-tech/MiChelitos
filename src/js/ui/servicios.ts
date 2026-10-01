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

/** Conecta los servicios con la clase vieja. Desaparece con la última vista. */
export function serviciosDesdeAppUI(
    app: AppUIAntigua,
    dom: Dom,
    ahora: Reloj = () => new Date(),
    dialogos: Dialogos = dialogosDelNavegador,
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
    };
}
