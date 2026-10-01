// Cada pestaña del menú tiene que dibujarse desde una **vista registrada**
// (`vistas/registro.ts`), y toda vista registrada tiene que corresponder a una pestaña
// del menú.
//
// Existe por un riesgo propio del enrutador: una ruta que nadie reconoce **cae en el
// Dashboard**, así que una vista que **deja de registrarse** no da error: pinta el
// Dashboard en su lugar. Las pruebas de la vista no lo ven (ejercen la clase, no el
// registro) y una vista sin manejadores —como el Resumen— tampoco la delatan las de
// interacción. Hasta la 1.65.0 algunas pestañas seguían en un `case` de `ui.ts`; ya no
// queda ninguno, y el registro es el único sitio.

import test from 'node:test';
import assert from 'node:assert/strict';
import { leerHtml } from '../ayudas/fuentes_interfaz.js';
import { cargarInterfaz } from '../ayudas/cargar_interfaz.js';

const rutasDelMenu = () => [...leerHtml().matchAll(/navigate\('(\w+)'\)/g)].map(m => m[1]);
const rutasRegistradas = () => [...cargarInterfaz().vistas.keys()];

test('el menú tiene las once pestañas', () => {
    assert.equal(new Set(rutasDelMenu()).size, 11);
});

test('cada pestaña del menú tiene su vista registrada (si no, caería en el Dashboard)', () => {
    const registradas = new Set(rutasRegistradas());
    const sinVista = [...new Set(rutasDelMenu())].filter(ruta => !registradas.has(ruta));
    assert.deepEqual(sinVista, [], 'pestañas que no dibuja nadie');
});

test('toda vista registrada corresponde a una pestaña del menú', () => {
    const menu = new Set(rutasDelMenu());
    const sobrantes = rutasRegistradas().filter(r => !menu.has(r));
    assert.deepEqual(sobrantes, [], 'vistas registradas con una ruta que el menú no ofrece');
});

test('cada vista registrada dibuja su propia pestaña con el enrutador real, no el Dashboard', async () => {
    // Un registro con las rutas cruzadas dibujaría otra pestaña sin dar error.
    const dibujos = new Map();
    for (const ruta of rutasRegistradas()) {
        const interfaz = cargarInterfaz({ renderReal: true });
        const vista = interfaz.vistas.get(ruta);
        vista.render = async () => { dibujos.set(ruta, ruta); };
        await interfaz.enrutador.mostrar(ruta);
    }
    assert.equal(dibujos.size, 11);
});

test('una ruta que nadie reconoce cae en el Dashboard, como siempre (con el enrutador real)', async () => {
    const interfaz = cargarInterfaz({
        renderReal: true,
        api: {
            obtenerCapital: { totales: {}, certificados: [], bolsa: [] },
            obtenerGastos: [], obtenerIngresos: [], obtenerIngresosInformales: [],
            obtenerTarjetas: [], obtenerPrestamos: [],
        },
    });
    await interfaz.enrutador.mostrar('una-ruta-que-no-existe');
    assert.match(interfaz.elemento('app-content').innerHTML, /Dashboard General/);
    assert.doesNotMatch(interfaz.elemento('app-content').innerHTML, /Error al renderizar/);
});
