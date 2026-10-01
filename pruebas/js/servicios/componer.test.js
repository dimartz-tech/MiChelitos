// La composición (`componerInterfaz`): lo que arma la aplicación al arrancar y que
// antes hacía la clase `AppUI`. Lo que se comprueba aquí es lo que, si falla, falla
// **al pulsar** y no al cargar: una pestaña sin vista, un manejador pisado por otro o
// un servicio que no es el que la vista creía.

import test from 'node:test';
import assert from 'node:assert/strict';
import { cargarInterfaz } from '../ayudas/cargar_interfaz.js';
import { crearRegistroDeVistas } from '../../../src/js/ui/componer.js';
import { MANEJADORES_GASTOS } from '../../../src/js/vistas/gastos.js';
import { MANEJADORES_TARJETAS } from '../../../src/js/vistas/tarjetas.js';
import { MANEJADORES_AJUSTES } from '../../../src/js/vistas/ajustes.js';

test('compone las once vistas, cada una con su ruta', () => {
    const ui = cargarInterfaz();
    assert.deepEqual([...ui.vistas.keys()].sort(), ['ajustes', 'capital', 'cuentas', 'dashboard', 'efectivo', 'gastos', 'ingresos', 'prestamos', 'resumen', 'suscripciones', 'tarjetas']);
    for (const vista of ui.vistas.values()) assert.equal(typeof vista.render, 'function');
});

test('el puente es solo de funciones y contiene los manejadores de cada vista', () => {
    const { appUI } = cargarInterfaz();
    const claves = Object.keys(appUI);
    assert.ok(claves.length > 60, `se esperaban decenas de manejadores, hay ${claves.length}`);
    for (const nombre of claves) assert.equal(typeof appUI[nombre], 'function', nombre);
    for (const lista of [MANEJADORES_GASTOS, MANEJADORES_TARJETAS, MANEJADORES_AJUSTES]) {
        for (const nombre of lista) assert.ok(claves.includes(nombre), `falta ${nombre}`);
    }
});

test('el puente no tiene lo que fue de la clase: ni showToast, ni render, ni formatMoney', () => {
    const { appUI } = cargarInterfaz();
    for (const nombre of ['showToast', 'render', 'formatMoney', 'pedirMotivoDeCorreccion', 'registrarVista', 'vistas', 'dialogos']) {
        assert.equal(nombre in appUI, false, `${nombre} no debería colgar del puente`);
    }
});

test('el registro rechaza una ruta repetida y un manejador repetido, en lugar de pisarlos en silencio', () => {
    const vistas = new Map(), puente = {};
    const registro = crearRegistroDeVistas(vistas, puente);
    registro.registrar('a', { render: async () => {} }, { hacer: () => 1 });
    assert.throws(() => registro.registrar('a', { render: async () => {} }, { otro: () => 2 }), /ya tiene una vista registrada/);
    assert.throws(() => registro.registrar('b', { render: async () => {} }, { hacer: () => 3 }), /Manejadores repetidos en «b»: hacer/);
    assert.equal(puente.hacer(), 1, 'el manejador original no se pisó');
    assert.deepEqual([...vistas.keys()], ['a'], 'la vista rechazada no quedó registrada');
});

test('ningún manejador de ninguna vista pisa a otro (si no, la composición no arrancaría)', () => {
    assert.doesNotThrow(() => cargarInterfaz());
});

test('los servicios que ven las vistas son los de la composición, y el motivo usa los avisos y los diálogos de esa carga', async () => {
    const ui = cargarInterfaz({ prompt: 'corto' });
    const resultado = await ui.servicios.motivo.pedir('Vas a borrar', 'Esto destruye.');
    assert.equal(resultado, null);
    assert.deepEqual(ui.avisos.map(a => a.tipo), ['error'], 'el aviso de motivo corto pasó por los avisos de la carga');
    assert.match(ui.preguntas[0].texto, /Vas a borrar/, 'la pregunta pasó por los diálogos de la carga');
});

test('con el enrutador real las vistas se dibujan a través de él; con el sustituto solo se anota la ruta', async () => {
    const solo = cargarInterfaz();
    await solo.servicios.enrutador.mostrar('gastos');
    assert.deepEqual(solo.renders, ['gastos']);
    const real = cargarInterfaz({ renderReal: true });
    real.vistas.get('gastos').render = async () => { real.elemento('app-content').innerHTML = 'dibujada'; };
    await real.servicios.enrutador.mostrar('gastos');
    assert.deepEqual(real.renders, []);
    assert.equal(real.elemento('app-content').innerHTML, 'dibujada');
});
