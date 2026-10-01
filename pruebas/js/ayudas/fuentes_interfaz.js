// Dónde está el código de la interfaz.
//
// Hasta la división de `ui.ts` por pestañas (`division_de_ui_limpia.md`) la
// interfaz era un solo archivo y varias pruebas de contrato lo leían por su
// nombre. Con las vistas repartidas en `vistas/`, leer solo `ui.ts` daría por
// buena una interfaz a la que le faltan pestañas. Este es el único sitio que
// sabe qué archivos la componen: las pruebas piden «la interfaz» y no un
// archivo.

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

export const RAIZ = join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..');
const JS = join(RAIZ, 'src', 'js');

function recorrer(dir) {
    return readdirSync(dir).flatMap(nombre => {
        const ruta = join(dir, nombre);
        return statSync(ruta).isDirectory() ? recorrer(ruta) : [ruta];
    });
}

/**
 * Fuentes TypeScript que **dibujan o atienden la interfaz**: todo `src/js` salvo
 * la API (`api.ts`), el núcleo puro (`nucleo/`) y las declaraciones de tipos.
 */
export function fuentesDeLaInterfaz() {
    return recorrer(JS)
        .filter(f => f.endsWith('.ts') && !f.endsWith('.d.ts'))
        .filter(f => !f.endsWith('api.ts') && !f.includes(join('src', 'js', 'nucleo') + '/'))
        .sort();
}

const unir = archivos => archivos.map(f => readFileSync(f, 'utf8')).join('\n');

/** El TypeScript de toda la interfaz, unido. */
export const leerInterfazTs = () => unir(fuentesDeLaInterfaz());

/** El JavaScript **compilado** de toda la interfaz, unido (`npm test` compila antes). */
export const leerInterfazJs = () => unir(fuentesDeLaInterfaz().map(f => f.replace(/\.ts$/, '.js')));

export const leerHtml = () => readFileSync(join(RAIZ, 'src', 'index.html'), 'utf8');
