// Raíz de composición: el único sitio que conoce las piezas reales.
//
// Compone la interfaz con el DOM y la API reales (`ui/componer.ts`, que es lo mismo
// que ejecutan las pruebas con dobles), cuelga el puente de manejadores de
// `window.appUI` —lo que llaman los manejadores en línea del HTML— y arranca la
// aplicación. Se carga como módulo ES (`<script type="module">`): se ejecuta después
// de leer el documento y antes de `DOMContentLoaded`, que es cuando `app.ts` dibuja la
// primera pestaña, así que toda vista está registrada antes de poder pulsarse.
//
// Qué vistas hay y cómo se construyen vive en `vistas/registro.ts`.

import { iniciarAplicacion } from './app.js';
import { componerInterfaz } from './ui/componer.js';
import type { Dom } from './ui/servicios.js';

// Globales de los scripts clásicos (`ui/dom.ts`): se declaran aquí, dentro de un
// módulo, para no chocar con sus declaraciones reales.
declare function elemento<T extends HTMLElement = HTMLElement>(id: string): T;
declare function buscar<T extends HTMLElement = HTMLElement>(id: string): T | null;

const dom: Dom = {
    elemento: <T extends HTMLElement = HTMLElement>(id: string) => elemento<T>(id),
    buscar: <T extends HTMLElement = HTMLElement>(id: string) => buscar<T>(id),
};

const { appUI, servicios, enrutador } = componerInterfaz(
    { dom, contenido: elemento('app-content'), avisos: { contenedor: elemento('notification-container') } },
    AppAPI,
);

// El HTML llama a `appUI.<método>(…)` desde sus atributos `onclick`/`onsubmit`/`onchange`.
(window as unknown as { appUI: typeof appUI }).appUI = appUI;

iniciarAplicacion({ enrutador, avisos: servicios.avisos, dom, api: AppAPI });
