// Borra el JavaScript generado de `src/js/` antes de compilar.
//
// Desde la migración a TypeScript, **todo `.js` de `src/js/` es generado** por
// `tsc`. Si se borra o renombra un `.ts`, su `.js` quedaría huérfano y se
// empaquetaría en la aplicación; limpiar antes de compilar lo impide.
// Sin dependencias.

import { readdirSync, rmSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), '..', 'src', 'js');

function recorrer(dir) {
    for (const nombre of readdirSync(dir)) {
        const ruta = join(dir, nombre);
        if (statSync(ruta).isDirectory()) recorrer(ruta);
        else if (nombre.endsWith('.js')) rmSync(ruta);
    }
}

recorrer(RAIZ);
