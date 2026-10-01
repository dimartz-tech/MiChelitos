// Raíz de composición: el único sitio que conoce las piezas reales.
//
// Crea los servicios, construye cada vista con sus dependencias y la registra
// en `appUI` (la clase vieja, mientras queden vistas sin extraer). Se carga
// como módulo ES (`<script type="module">`): se ejecuta después de leer el
// documento y antes de `DOMContentLoaded`, que es cuando `app.ts` dibuja la
// primera pestaña, así que toda vista está registrada antes de poder pulsarse.
//
// Cada extracción de `ui.ts` añade su vista en `vistas/registro.ts`. Ver
// `division_de_ui_limpia.md`.

import { serviciosDesdeAppUI, type AppUIAntigua, type Dom } from './ui/servicios.js';
import { registrarVistas } from './vistas/registro.js';

// Globales de los scripts clásicos (`ui.ts`, `ui/dom.ts`): se declaran aquí,
// dentro de un módulo, para no chocar con sus declaraciones reales.
declare const appUI: AppUIAntigua;
declare function elemento<T extends HTMLElement = HTMLElement>(id: string): T;
declare function buscar<T extends HTMLElement = HTMLElement>(id: string): T | null;

const dom: Dom = {
    elemento: <T extends HTMLElement = HTMLElement>(id: string) => elemento<T>(id),
    buscar: <T extends HTMLElement = HTMLElement>(id: string) => buscar<T>(id),
};

const servicios = serviciosDesdeAppUI(appUI, dom);

// Qué vistas hay y cómo se construyen vive en `vistas/registro.ts`.
registrarVistas(appUI, servicios, AppAPI);
