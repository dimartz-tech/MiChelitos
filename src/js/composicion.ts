// Raíz de composición: el único sitio que conoce las piezas reales.
//
// Crea los servicios, construye cada vista con sus dependencias y la registra
// en `appUI` (la clase vieja, mientras queden vistas sin extraer). Se carga
// como módulo ES (`<script type="module">`): se ejecuta después de leer el
// documento y antes de `DOMContentLoaded`, que es cuando `app.ts` dibuja la
// primera pestaña, así que toda vista está registrada antes de poder pulsarse.
//
// Cada extracción de `ui.ts` añade aquí su vista (una línea de construcción y
// otra de registro). Ver `division_de_ui_limpia.md`.

import { serviciosDesdeAppUI, type AppUIAntigua, type Dom } from './ui/servicios.js';

// Globales de los scripts clásicos (`ui.ts`, `ui/dom.ts`): se declaran aquí,
// dentro de un módulo, para no chocar con sus declaraciones reales.
declare const appUI: AppUIAntigua;
declare function elemento<T extends HTMLElement = HTMLElement>(id: string): T;
declare function buscar<T extends HTMLElement = HTMLElement>(id: string): T | null;

const dom: Dom = {
    elemento: <T extends HTMLElement = HTMLElement>(id: string) => elemento<T>(id),
    buscar: <T extends HTMLElement = HTMLElement>(id: string) => buscar<T>(id),
};

// Todavía no hay ninguna vista extraída (la primera, `efectivo`, es el PR 1).
// Los servicios se construyen igualmente: así este módulo ejercita ya su carga
// en la aplicación empaquetada y el cableado con la clase vieja.
export const servicios = serviciosDesdeAppUI(appUI, dom);
