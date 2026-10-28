// Resumen › «Gasto por categoría»: lo que ocurre al tocar el mes, el promedio y el conmutador, con la composición
// real de la interfaz y una API falsa. Datos inventados.

import test from 'node:test';
import assert from 'node:assert/strict';
import { cargarInterfaz } from '../ayudas/cargar_interfaz.js';
import { fila, resumen } from '../ayudas/resumen_gastos_datos.js';

const IDS = ['resumen-gasto-categoria', 'rg-mes', 'rg-mes-anterior', 'rg-mes-siguiente', 'rg-promedio', 'rg-incluir-todo'];
const CATEGORIAS = [{ id: 41, nombre: 'Comida' }, { id: 7, nombre: 'Impuestos' }, { id: 12, nombre: 'Maquinaria/Equipos' }, { id: 13, nombre: 'Prestamo' }, { id: 99, nombre: 'Renta' }];

/** Todo lo que `render()` consulta, con respuestas mínimas, más lo que la prueba pise. */
function interfaz(api = {}) {
    const campos = Object.fromEntries(IDS.map(id => [id, {}]));
    const ui = cargarInterfaz({
        campos,
        api: {
            obtenerCapital: { totales: {} }, obtenerIngresos: [], obtenerIngresosInformales: [], obtenerGastos: [],
            obtenerTarjetas: [], obtenerSuscripciones: [], obtenerPrestamos: [], obtenerPatrimonio: null,
            obtenerCategorias: CATEGORIAS, obtenerResumenGastos: resumen(),
            ...api,
        },
    });
    return { ui, vista: ui.vistas.get('resumen'), bloque: () => ui.elemento('resumen-gasto-categoria').innerHTML };
}

const hoy = ui => { const d = ui.servicios.ahora(); return { anio: d.getFullYear(), mes: d.getMonth() + 1 }; };
const llamadas = ui => ui.llamadasA('obtenerResumenGastos').map(l => l.args);

/** Una promesa que la prueba resuelve a mano (para ordenar las respuestas). */
function diferida() {
    let resolver, rechazar;
    const promesa = new Promise((res, rej) => { resolver = res; rechazar = rej; });
    return { promesa, resolver, rechazar };
}

test('al abrir pide el mes en curso, promedio de 3, todo incluido, con la tasa de referencia, y dibuja la tabla', async () => {
    const { ui, vista, bloque } = interfaz();
    await vista.render();
    const { anio, mes } = hoy(ui);
    assert.deepEqual(llamadas(ui), [[anio, mes, 3, ui.servicios.referencias.tasaUsdADop, []]]);
    assert.match(bloque(), /<table/);
    assert.match(bloque(), /Comida/);
    assert.deepEqual(ui.llamadasA('obtenerCategorias'), [], 'sin conmutador apagado no hace falta el catálogo');
});

test('cambiar el mes pide ese mes; «anterior» y «siguiente» se mueven por los meses disponibles', async () => {
    const { ui, vista } = interfaz({ obtenerResumenGastos: () => resumen({ meses_disponibles: [{ anio: 2026, mes: 5 }, { anio: 2026, mes: 6 }, { anio: 2026, mes: 7 }] }) });
    await vista.render();
    await ui.appUI.resumenGastoMes('2026-06');
    assert.deepEqual(llamadas(ui).at(-1).slice(0, 3), [2026, 6, 3]);
    await ui.appUI.resumenGastoMover(-1);
    assert.deepEqual(llamadas(ui).at(-1).slice(0, 3), [2026, 5, 3]);
    await ui.appUI.resumenGastoMover(-1); // ya es el más antiguo
    assert.equal(llamadas(ui).length, 3, 'en el extremo no hay otra petición');
    await ui.appUI.resumenGastoMover(1);
    assert.deepEqual(llamadas(ui).at(-1).slice(0, 3), [2026, 6, 3]);
    await ui.appUI.resumenGastoMes('hostil');
    assert.equal(llamadas(ui).length, 4, 'un valor que no es un mes se ignora');
});

test('el promedio ofrece 2, 3 y 6 y manda N; otro valor se ignora', async () => {
    const { ui, vista } = interfaz();
    await vista.render();
    await ui.appUI.resumenGastoPromedio('6');
    assert.equal(llamadas(ui).at(-1)[2], 6);
    await ui.appUI.resumenGastoPromedio('2');
    assert.equal(llamadas(ui).at(-1)[2], 2);
    await ui.appUI.resumenGastoPromedio('5');
    assert.equal(llamadas(ui).length, 3);
});

test('apagar «Incluir inversión y obligaciones» excluye por los ids del catálogo y lo dice; volver a encender no excluye nada', async () => {
    const { ui, vista, bloque } = interfaz();
    await vista.render();
    await ui.appUI.resumenGastoConmutar({ checked: false });
    assert.deepEqual(llamadas(ui).at(-1)[4].sort((a, b) => a - b), [7, 12, 13, 99]);
    assert.match(bloque(), /Fuera de la tabla: Impuestos, Maquinaria\/Equipos, Prestamo, Renta/);
    await ui.appUI.resumenGastoConmutar({ checked: false });
    assert.equal(ui.llamadasA('obtenerCategorias').length, 1, 'el catálogo se pide una sola vez');
    await ui.appUI.resumenGastoConmutar({ checked: true });
    assert.deepEqual(llamadas(ui).at(-1)[4], []);
    assert.doesNotMatch(bloque(), /Fuera de la tabla/);
});

test('mes parcial: se rotula «parcial, hasta el día N»', async () => {
    const { ui, vista, bloque } = interfaz({ obtenerResumenGastos: resumen({ parcial: true, hasta_dia: 11 }) });
    await vista.render();
    assert.match(bloque(), /parcial, hasta el día 11/);
    assert.ok(ui);
});

test('mes sin gastos: mensaje claro y sin tabla', async () => {
    const { vista, bloque } = interfaz({ obtenerResumenGastos: resumen({ filas: [] }) });
    await vista.render();
    assert.match(bloque(), /Sin gastos en/);
    assert.doesNotMatch(bloque(), /<table/);
});

test('si el comando falla el bloque muestra el error y el resto del Resumen se dibuja igual', async () => {
    const { ui, vista, bloque } = interfaz({ obtenerResumenGastos: () => Promise.reject('el comando se cayó <b>') });
    await assert.doesNotReject(vista.render());
    assert.match(bloque(), /role="alert"/);
    assert.match(bloque(), /el comando se cayó &lt;b&gt;/);
    assert.match(ui.elemento('app-content').innerHTML, /Resumen Ejecutivo/);
    // y se recupera al elegir otro mes
    ui.responder('obtenerResumenGastos', resumen());
    await ui.appUI.resumenGastoMes('2026-01');
    assert.match(bloque(), /<table/);
});

test('un fallo al leer el catálogo (conmutador apagado) también se queda en el bloque', async () => {
    const { ui, vista, bloque } = interfaz();
    await vista.render();
    ui.rechazar('obtenerCategorias', 'sin catálogo');
    await ui.appUI.resumenGastoConmutar({ checked: false });
    assert.match(bloque(), /sin catálogo/);
});

test('mientras llega la respuesta se ve «Cargando»', async () => {
    const d = diferida();
    const { ui, vista, bloque } = interfaz();
    await vista.render();
    ui.responder('obtenerResumenGastos', () => d.promesa);
    const espera = ui.appUI.resumenGastoPromedio('6');
    assert.match(bloque(), /Cargando el gasto/);
    d.resolver(resumen());
    await espera;
    assert.match(bloque(), /<table/);
});

test('una respuesta vieja nunca pisa a la más reciente (ni su error)', async () => {
    const lenta = diferida();
    const { ui, vista, bloque } = interfaz();
    await vista.render();
    ui.responder('obtenerResumenGastos', (anio, mes) => (mes === 3 ? lenta.promesa : Promise.resolve(resumen({ anio, mes, filas: [fila(5, 'Categoría del mes nuevo')] }))));
    const primera = ui.appUI.resumenGastoMes('2027-03');
    const segunda = ui.appUI.resumenGastoMes('2027-04');
    await segunda;
    assert.match(bloque(), /Categoría del mes nuevo/);
    lenta.resolver(resumen({ anio: 2027, mes: 3, filas: [fila(6, 'Categoría vieja')] }));
    await primera;
    assert.match(bloque(), /Categoría del mes nuevo/);
    assert.doesNotMatch(bloque(), /Categoría vieja/);

    // lo mismo con un error tardío
    const tardia = diferida();
    ui.responder('obtenerResumenGastos', (anio, mes) => (mes === 3 ? tardia.promesa : Promise.resolve(resumen({ anio, mes }))));
    const a = ui.appUI.resumenGastoMes('2027-03');
    const b = ui.appUI.resumenGastoMes('2027-04');
    await b;
    tardia.rechazar('error tardío');
    await a;
    assert.doesNotMatch(bloque(), /error tardío/);
    assert.match(bloque(), /<table/);
});

test('volver a abrir la pestaña descarta una respuesta que seguía pendiente', async () => {
    const lenta = diferida();
    const { ui, vista, bloque } = interfaz();
    ui.responder('obtenerResumenGastos', () => lenta.promesa);
    const primera = vista.render();
    await Promise.resolve();
    ui.responder('obtenerResumenGastos', resumen({ filas: [fila(5, 'De la segunda apertura')] }));
    await vista.render();
    lenta.resolver(resumen({ filas: [fila(6, 'De la primera apertura')] }));
    await primera;
    assert.match(bloque(), /De la segunda apertura/);
    assert.doesNotMatch(bloque(), /De la primera apertura/);
});

test('tras repintar, el foco vuelve al control que se tocó', async () => {
    const { ui, vista } = interfaz();
    await vista.render();
    await ui.appUI.resumenGastoPromedio('6');
    assert.equal(ui.elemento('rg-promedio').enfocado, true);
});
