// Pestaña Gastos: alta de gasto, liquidación de un consumo pendiente en divisa,
// borrado con motivo (Ajustes) y selector de mes. Datos inventados.

import test from 'node:test';
import assert from 'node:assert/strict';
import { cargarInterfaz, crearEvento, AUSENTE } from '../ayudas/cargar_interfaz.js';
import {
    llamoUnaVez, noLlamoANada, avisoExito, unAvisoDeError, redibujo, noRedibujo, rechazoSeMuestra,
} from '../ayudas/afirmar.js';

const base = {
    gas_fec: '15/03/2026', gas_mon: '123.45', gas_div: 'DOP', gas_des: 'Compra de prueba',
    gas_cat: '3', gas_met: 'efectivo', gas_lbtr: false, gas_tar: AUSENTE, gas_cue: AUSENTE,
};
const gastoEsperado = {
    fecha: '15/03/2026', monto: '123.45', divisa: 'DOP', descripcion: 'Compra de prueba', categoria_id: 3,
    metodo_pago: 'efectivo', es_lbtr: false, tarjeta_id: null, cuenta_ahorro_id: null, tasa_cambio: null,
};
const cuentaDop = { value: '9', text: 'Cuenta A', dataset: { divisa: 'DOP' } };

test('gasto en efectivo: sin tarjeta, sin cuenta, sin tasa; el importe viaja como texto', async () => {
    const ui = cargarInterfaz({ campos: base });
    await ui.appUI.handleAgregarGasto(crearEvento());
    llamoUnaVez(ui, 'crearGasto', [gastoEsperado]);
    avisoExito(ui, /Gasto registrado/);
    redibujo(ui, 'gastos');
});

test('el manejador cancela el envío nativo del formulario (preventDefault)', async () => {
    const ui = cargarInterfaz({ campos: base });
    const e = crearEvento();
    await ui.appUI.handleAgregarGasto(e);
    assert.equal(e.prevenido, true);
});

test('gasto con tarjeta: envía el id de la tarjeta y ninguna cuenta', async () => {
    const ui = cargarInterfaz({ campos: { ...base, gas_met: 'tarjeta', gas_tar: '4' } });
    await ui.appUI.handleAgregarGasto(crearEvento());
    llamoUnaVez(ui, 'crearGasto', [{ ...gastoEsperado, metodo_pago: 'tarjeta', tarjeta_id: 4 }]);
});

test('gasto por transferencia en la misma divisa: envía la cuenta y no la tasa', async () => {
    const ui = cargarInterfaz({
        campos: { ...base, gas_met: 'transferencia', gas_lbtr: true, gas_cue: { options: [cuentaDop] } },
    });
    await ui.appUI.handleAgregarGasto(crearEvento());
    llamoUnaVez(ui, 'crearGasto', [{
        ...gastoEsperado, metodo_pago: 'transferencia', es_lbtr: true, cuenta_ahorro_id: 9,
    }]);
});

test('gasto en USD desde cuenta en DOP: viaja la tasa tecleada', async () => {
    const ui = cargarInterfaz({
        campos: {
            ...base, gas_div: 'USD', gas_met: 'transferencia', gas_tasa: '58.5',
            gas_cue: { options: [cuentaDop] },
        },
    });
    await ui.appUI.handleAgregarGasto(crearEvento());
    llamoUnaVez(ui, 'crearGasto', [{
        ...gastoEsperado, divisa: 'USD', metodo_pago: 'transferencia', cuenta_ahorro_id: 9, tasa_cambio: 58.5,
    }]);
});

test('divisas cruzadas sin tasa: no se envía nada y se pide la tasa', async () => {
    const ui = cargarInterfaz({
        campos: {
            ...base, gas_div: 'USD', gas_met: 'transferencia', gas_tasa: '',
            gas_cue: { options: [cuentaDop] },
        },
    });
    await ui.appUI.handleAgregarGasto(crearEvento());
    noLlamoANada(ui);
    unAvisoDeError(ui, /indica la tasa de cambio/);
    noRedibujo(ui);
});

test('divisas cruzadas con tasa cero: tampoco se envía', async () => {
    const ui = cargarInterfaz({
        campos: {
            ...base, gas_div: 'USD', gas_met: 'transferencia', gas_tasa: '0',
            gas_cue: { options: [cuentaDop] },
        },
    });
    await ui.appUI.handleAgregarGasto(crearEvento());
    noLlamoANada(ui);
    unAvisoDeError(ui, /tasa/);
});

test('pago con tarjeta sin elegir tarjeta (hay tarjetas): no se envía y se dice cuál falta', async () => {
    const ui = cargarInterfaz({
        campos: {
            ...base, gas_met: 'tarjeta',
            gas_tar: { options: [{ value: '', text: 'Seleccione' }, { value: '4', text: 'Tarjeta A' }], selectedIndex: 0 },
        },
    });
    await ui.appUI.handleAgregarGasto(crearEvento());
    noLlamoANada(ui);
    unAvisoDeError(ui, /Indica con qué tarjeta/);
});

test('pago con tarjeta sin ninguna tarjeta registrada: el aviso lo explica distinto', async () => {
    const ui = cargarInterfaz({
        campos: { ...base, gas_met: 'tarjeta', gas_tar: { options: [{ value: '', text: 'Seleccione' }] } },
    });
    await ui.appUI.handleAgregarGasto(crearEvento());
    noLlamoANada(ui);
    unAvisoDeError(ui, /No hay tarjetas registradas/);
});

test('gasto: si Rust rechaza se muestra el error, no se redibuja y no se rompe', async () => {
    const ui = cargarInterfaz({ campos: base });
    ui.rechazar('crearGasto', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleAgregarGasto(crearEvento()));
});

// --- liquidación de un consumo pendiente ---

test('liquidar consumo: envía id y el importe en pesos, cierra el modal y avisa la tasa', async () => {
    const ui = cargarInterfaz({ campos: { liq_monto_7: '777.77', 'modal-liq-7': {} }, api: { liquidarConsumoPendiente: 58.5 } });
    await ui.appUI.handleLiquidacionSubmit(crearEvento(), 7, 12.34);
    llamoUnaVez(ui, 'liquidarConsumoPendiente', [7, '777.77']);
    avisoExito(ui, /tasa de 58\.5000/);
    assert.deepEqual(ui.eliminados, ['modal-liq-7']);
    redibujo(ui, 'gastos');
});

test('liquidar consumo con importe cero o vacío: no se envía', async () => {
    for (const valor of ['0', '', '-5']) {
        const ui = cargarInterfaz({ campos: { liq_monto_7: valor } });
        await ui.appUI.handleLiquidacionSubmit(crearEvento(), 7, 12.34);
        noLlamoANada(ui);
        unAvisoDeError(ui, /mayor que cero/);
    }
});

test('liquidar consumo: si Rust rechaza, el modal sigue abierto', async () => {
    const ui = cargarInterfaz({ campos: { liq_monto_7: '777.77', 'modal-liq-7': {} } });
    ui.rechazar('liquidarConsumoPendiente', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleLiquidacionSubmit(crearEvento(), 7, 12.34));
    assert.deepEqual(ui.eliminados, []);
});

// --- borrar un gasto (Ajustes → correcciones) ---

const MOTIVO = 'Se registró dos veces por error de captura';

test('borrar gasto: confirma, pide motivo y envía id + motivo', async () => {
    const ui = cargarInterfaz({ confirm: true, prompt: MOTIVO, api: { eliminarGasto: 'CC-0001' } });
    await ui.appUI.handleEliminarGastoCorr(21);
    llamoUnaVez(ui, 'eliminarGasto', [21, MOTIVO]);
    avisoExito(ui, /Caso CC-0001/);
    redibujo(ui, 'ajustes');
});

test('borrar gasto: si se rechaza la confirmación no se pregunta ni se envía nada', async () => {
    const ui = cargarInterfaz({ confirm: false });
    await ui.appUI.handleEliminarGastoCorr(21);
    noLlamoANada(ui);
    assert.deepEqual(ui.preguntas, []);
    assert.deepEqual(ui.avisos, []);
});

test('borrar gasto: cancelar el motivo (prompt null) no envía nada', async () => {
    const ui = cargarInterfaz({ confirm: true, prompt: null });
    await ui.appUI.handleEliminarGastoCorr(21);
    noLlamoANada(ui);
    noRedibujo(ui);
});

test('borrar gasto: un motivo de menos de 15 caracteres se rechaza con aviso', async () => {
    const ui = cargarInterfaz({ confirm: true, prompt: 'error' });
    await ui.appUI.handleEliminarGastoCorr(21);
    noLlamoANada(ui);
    unAvisoDeError(ui, /demasiado corto/);
});

test('borrar gasto: si Rust rechaza se muestra el error', async () => {
    const ui = cargarInterfaz({ confirm: true, prompt: MOTIVO });
    ui.rechazar('eliminarGasto', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleEliminarGastoCorr(21));
});

// --- selector de mes ---

test('elegir mes en Gastos: guarda el mes y redibuja solo Gastos', async () => {
    // Desde la extracción de la vista, el mes elegido es estado **privado** de la vista
    // de Gastos: ya no se puede leer ni sustituir `appUI.renderGastos`. Lo observable
    // es que, al elegir un mes, la pantalla se dibuja con ese mes seleccionado.
    const gasto = (fecha, descripcion) => ({
        id: 1, fecha, monto: 100, divisa: 'DOP', descripcion, categoria_id: 1, categoria_nombre: 'Otros',
        metodo_pago: 'efectivo', costo_adicional: 0, tarjeta_id: null, cuenta_ahorro_id: null,
        estado_conversion: null, monto_liquidado: null, tasa_conversion: null,
    });
    const ui = cargarInterfaz({
        api: {
            obtenerGastos: [gasto('10/02/2026', 'Gasto de febrero'), gasto('10/03/2026', 'Gasto de marzo')],
            obtenerCategorias: [{ id: 1, nombre: 'Otros' }], obtenerCuentas: [], obtenerTarjetas: [],
        },
    });
    await ui.appUI.handleSelectGastosMonth('02/2026');
    const html = ui.elemento('app-content').innerHTML;
    assert.match(html, /<option value="02\/2026" selected>/, 'el mes elegido queda seleccionado');
    assert.match(html, /Resumen de Febrero 2026/);
    // Solo lecturas: dibujar la pantalla no escribe nada.
    assert.deepEqual([...ui.nombresLlamados].sort(), ['obtenerCategorias', 'obtenerCuentas', 'obtenerGastos', 'obtenerPresupuestosMes', 'obtenerTarjetas']);
});

// --- presupuestos mensuales por categoría ---

test('fijar presupuesto: envía la categoría y el monto como texto, avisa y redibuja', async () => {
    const ui = cargarInterfaz({ campos: { pre_cat: '3', pre_monto: ' 0450.07 ' }, api: { fijarPresupuesto: { categoria_nombre: 'Comida', monto: '450.07' } } });
    const e = crearEvento();
    await ui.appUI.handleFijarPresupuesto(e);
    assert.equal(e.prevenido, true);
    llamoUnaVez(ui, 'fijarPresupuesto', [3, '0450.07']);
    avisoExito(ui, /Comida.*450\.07/);
    redibujo(ui, 'gastos');
});

test('fijar presupuesto con monto vacío, cero o negativo: no se envía', async () => {
    for (const monto of ['', '0', '-5', 'abc']) {
        const ui = cargarInterfaz({ campos: { pre_cat: '3', pre_monto: monto } });
        await ui.appUI.handleFijarPresupuesto(crearEvento());
        noLlamoANada(ui);
        unAvisoDeError(ui, /mayor que cero/);
        noRedibujo(ui);
    }
});

test('fijar presupuesto sin categoría elegida: no se envía', async () => {
    const ui = cargarInterfaz({ campos: { pre_cat: '', pre_monto: '10' } });
    await ui.appUI.handleFijarPresupuesto(crearEvento());
    noLlamoANada(ui);
    unAvisoDeError(ui, /categoría/);
});

test('fijar presupuesto: si Rust rechaza, se muestra el error y no se redibuja', async () => {
    const ui = cargarInterfaz({ campos: { pre_cat: '3', pre_monto: '10' } });
    ui.rechazar('fijarPresupuesto', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleFijarPresupuesto(crearEvento()));
});

test('quitar presupuesto: envía el id, avisa y redibuja', async () => {
    const ui = cargarInterfaz({ api: { quitarPresupuesto: true } });
    await ui.appUI.handleQuitarPresupuesto(5);
    llamoUnaVez(ui, 'quitarPresupuesto', [5]);
    avisoExito(ui, /quitado/);
    redibujo(ui, 'gastos');
});

test('quitar presupuesto: si Rust rechaza, se muestra el error y no se redibuja', async () => {
    const ui = cargarInterfaz();
    ui.rechazar('quitarPresupuesto', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleQuitarPresupuesto(5));
});
