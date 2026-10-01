// Pruebas del propio andamiaje: si el helper dejara de avisar de lo que lee un
// manejador sin declarar, todas las demás pruebas perderían su sentido.

import test from 'node:test';
import assert from 'node:assert/strict';
import { cargarInterfaz, crearEvento, AUSENTE } from '../ayudas/cargar_interfaz.js';

test('un id que el manejador lee y la prueba no declaró FALLA nombrando el id, aunque el manejador capture la excepción', async () => {
    const ui = cargarInterfaz({ campos: { cat_nom: 'X' } });
    await assert.rejects(
        () => ui.appUI.handleAgregarCliente(crearEvento()),   // lee cli_aj_nom, no declarado
        /no declaró el elemento #cli_aj_nom/,
    );
});

test('un id declarado AUSENTE se comporta como un elemento que no existe en la página', async () => {
    const ui = cargarInterfaz({ campos: { respaldo_resultado: AUSENTE } });
    assert.equal(ui.elemento('respaldo_resultado'), null);
});

test('un diálogo que el manejador invoca sin que la prueba lo configure también falla con mensaje claro', async () => {
    const ui = cargarInterfaz({});
    await assert.rejects(() => ui.appUI.handleEliminarCategoria(1), /no configuró confirm\(\)/);
});

test('el doble de AppAPI solo ofrece los métodos reales de api.ts (un nombre mal escrito falla como en la aplicación)', () => {
    const ui = cargarInterfaz({});
    assert.equal(typeof ui.api.crearGasto, 'function');
    assert.equal(ui.api.crearGastos, undefined);
    assert.ok(Object.keys(ui.api).length > 50);
});

test('cada prueba parte de una interfaz limpia: sin llamadas, avisos ni redibujos previos', () => {
    const ui = cargarInterfaz({});
    assert.deepEqual([ui.llamadas, ui.avisos, ui.renders], [[], [], []]);
});

test('el aviso se registra con su tipo y además se dibuja con el código real de los avisos', () => {
    const ui = cargarInterfaz({});
    ui.servicios.avisos.mostrar('Hola', 'error');
    assert.deepEqual(ui.avisos, [{ tipo: 'error', mensaje: 'Hola' }]);
    const contenedor = ui.elemento('notification-container');
    assert.equal(contenedor.hijos.length, 1);
    assert.equal(contenedor.hijos[0].className, 'toast toast-error');
});
