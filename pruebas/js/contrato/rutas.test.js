// Cada pestaña del menú tiene que dibujarse desde **un solo sitio**: o una vista
// extraída y registrada (`vistas/registro.ts`), o un `case` que quede en
// `ui.ts`.
//
// Existe por un riesgo propio de la división: el `switch` de `render()` termina
// en `default: renderDashboard()`, así que una vista que se extrae y **deja de
// registrarse** no da error, **pinta el Dashboard en su lugar**. Las pruebas de
// la vista no lo ven (ejercen la clase, no el registro) y una vista sin
// manejadores —como el Resumen— tampoco la delatan las de interacción.

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { RAIZ, leerHtml } from '../ayudas/fuentes_interfaz.js';
import { cargarInterfaz } from '../ayudas/cargar_interfaz.js';

const rutasDelMenu = () => [...leerHtml().matchAll(/navigate\('(\w+)'\)/g)].map(m => m[1]);
const rutasDelSwitch = () => {
    const ui = readFileSync(join(RAIZ, 'src', 'js', 'ui.ts'), 'utf8');
    const desdeRender = ui.slice(ui.indexOf('async render(route'));
    const cuerpo = desdeRender.slice(0, desdeRender.indexOf('default:'));
    return [...cuerpo.matchAll(/case '(\w+)':/g)].map(m => m[1]);
};
const rutasRegistradas = () => [...cargarInterfaz().appUI.vistas.keys()];

test('el menú tiene las once pestañas', () => {
    assert.equal(new Set(rutasDelMenu()).size, 11);
});

test('cada pestaña del menú se dibuja desde un solo sitio: una vista registrada o un case de ui.ts', () => {
    const registradas = new Set(rutasRegistradas());
    const enSwitch = new Set(rutasDelSwitch());
    const problemas = [];
    for (const ruta of new Set(rutasDelMenu())) {
        const sitios = Number(registradas.has(ruta)) + Number(enSwitch.has(ruta));
        // `dashboard` además puede ser el `default`: no cuenta como segundo sitio.
        if (sitios === 0) problemas.push(`${ruta}: no la dibuja nadie (caería en el Dashboard)`);
        if (sitios === 2) problemas.push(`${ruta}: está registrada y además tiene un case en ui.ts (sobra uno)`);
    }
    assert.deepEqual(problemas, []);
});

test('toda vista registrada corresponde a una pestaña del menú', () => {
    const menu = new Set(rutasDelMenu());
    const sobrantes = rutasRegistradas().filter(r => !menu.has(r));
    assert.deepEqual(sobrantes, [], 'vistas registradas con una ruta que el menú no ofrece');
});
