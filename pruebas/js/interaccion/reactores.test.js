// Métodos que reaccionan a `onchange`/`oninput` (no son `handle*`) y que
// preparan lo que luego se envía: el monto prefijado de un abono, la vista
// previa de una conversión, el cobro parcial. Datos inventados.

import test from 'node:test';
import assert from 'node:assert/strict';
import { cargarInterfaz } from '../ayudas/cargar_interfaz.js';
import { noLlamoANada } from '../ayudas/afirmar.js';

// --- aplicarTipoAbono: el monto que se prefija es el que se enviará ---

const abono = (tipo, divisa, monto = '') => ({
    pag_tipo_3: tipo, pag_div_3: divisa, pag_monto_3: { value: monto, readOnly: false },
});

test('abono «al corte» en DOP: prefija el saldo al corte en pesos con dos decimales y bloquea el campo', () => {
    const ui = cargarInterfaz({ campos: abono('corte', 'DOP') });
    ui.appUI.aplicarTipoAbono(3, 1234.5, 99.1, 5000.25, 77.7);
    assert.equal(ui.elemento('pag_monto_3').value, '1234.50');
    assert.equal(ui.elemento('pag_monto_3').readOnly, true);
    assert.deepEqual(ui.avisos, []);
});

test('abono «al corte» en USD usa los saldos en dólares; el tipo «actual» usa el balance', () => {
    let ui = cargarInterfaz({ campos: abono('corte', 'USD') });
    ui.appUI.aplicarTipoAbono(3, 1234.5, 99.1, 5000.25, 77.7);
    assert.equal(ui.elemento('pag_monto_3').value, '99.10');

    ui = cargarInterfaz({ campos: abono('actual', 'DOP') });
    ui.appUI.aplicarTipoAbono(3, 1234.5, 99.1, 5000.25, 77.7);
    assert.equal(ui.elemento('pag_monto_3').value, '5000.25');

    ui = cargarInterfaz({ campos: abono('actual', 'USD') });
    ui.appUI.aplicarTipoAbono(3, 1234.5, 99.1, 5000.25, 77.7);
    assert.equal(ui.elemento('pag_monto_3').value, '77.70');
});

test('abono «personalizado»: libera el campo y no toca lo que el titular escribió', () => {
    const ui = cargarInterfaz({ campos: { ...abono('personalizado', 'DOP', '10.00'), pag_monto_3: { value: '10.00', readOnly: true } } });
    ui.appUI.aplicarTipoAbono(3, 1234.5, 99.1, 5000.25, 77.7);
    assert.equal(ui.elemento('pag_monto_3').value, '10.00');
    assert.equal(ui.elemento('pag_monto_3').readOnly, false);
});

test('abono con saldo a favor: prefija 0.00, bloquea el campo y lo explica', () => {
    const ui = cargarInterfaz({ campos: abono('actual', 'DOP') });
    ui.appUI.aplicarTipoAbono(3, 0, 0, -250.5, 0);
    assert.equal(ui.elemento('pag_monto_3').value, '0.00');
    assert.equal(ui.elemento('pag_monto_3').readOnly, true);
    assert.equal(ui.avisos.length, 1);
    assert.equal(ui.avisos[0].tipo, 'info');
    assert.match(ui.avisos[0].mensaje, /a favor/);
});

test('abono con saldo cero: prefija 0.00 y avisa de que no hay saldo', () => {
    const ui = cargarInterfaz({ campos: abono('corte', 'DOP') });
    ui.appUI.aplicarTipoAbono(3, 0, 0, 10, 10);
    assert.equal(ui.elemento('pag_monto_3').value, '0.00');
    assert.match(ui.avisos[0].mensaje, /No hay saldo al corte en DOP/);
});

// --- vistas previas ---

test('liquidación: la tasa que se previsualiza es importe en pesos ÷ monto original, con 4 decimales', () => {
    const ui = cargarInterfaz({ campos: { liq_monto_7: '585', liq_tasa_7: {} } });
    ui.appUI.previsualizarTasa(7, 10);
    assert.match(ui.elemento('liq_tasa_7').innerHTML, /58\.5000/);
});

test('liquidación: con importe o monto original no positivos la vista previa se vacía', () => {
    const ui = cargarInterfaz({ campos: { liq_monto_7: '0', liq_tasa_7: { innerHTML: 'vieja', textContent: 'vieja' } } });
    ui.appUI.previsualizarTasa(7, 10);
    assert.equal(ui.elemento('liq_tasa_7').textContent, '');
    noLlamoANada(ui);
});

const gasto = (extra = {}) => ({
    gas_conversion_container: {}, gas_conversion_previa: {}, gas_met: 'transferencia', gas_div: 'USD',
    gas_mon: '100', gas_tasa: '58.5', gas_lbtr: false,
    gas_cue: { options: [{ value: '9', text: 'Cuenta A', dataset: { divisa: 'DOP' } }] }, ...extra,
});

test('gasto en otra divisa: la vista previa suma conversión + retención del 0,2 % (+ LBTR si se marca)', () => {
    let ui = cargarInterfaz({ campos: gasto() });
    ui.appUI.actualizarConversionGasto();
    assert.equal(ui.elemento('gas_conversion_container').style.display, 'block');
    assert.match(ui.elemento('gas_conversion_previa').innerHTML, /DOP 5,861\.70/);

    ui = cargarInterfaz({ campos: gasto({ gas_lbtr: true }) });
    ui.appUI.actualizarConversionGasto();
    assert.match(ui.elemento('gas_conversion_previa').innerHTML, /DOP 5,961\.70/);
});

test('gasto sin cruce de divisas o sin tasa: la previa se oculta o pide la tasa', () => {
    let ui = cargarInterfaz({ campos: gasto({ gas_div: 'DOP' }) });
    ui.appUI.actualizarConversionGasto();
    assert.equal(ui.elemento('gas_conversion_container').style.display, 'none');

    ui = cargarInterfaz({ campos: gasto({ gas_tasa: '' }) });
    ui.appUI.actualizarConversionGasto();
    assert.match(ui.elemento('gas_conversion_previa').textContent, /Indica la tasa/);
});

// --- cobro parcial ---

test('cobro parcial: el importe solo se muestra mientras la casilla está marcada', () => {
    const ui = cargarInterfaz({ campos: { edit_parcial_caja_5: { hidden: true }, edit_parcial_chk_5: { checked: true } } });
    ui.appUI.alternarCobroParcial(5);
    assert.equal(ui.elemento('edit_parcial_caja_5').hidden, false);
    ui.elemento('edit_parcial_chk_5').checked = false;
    ui.appUI.alternarCobroParcial(5);
    assert.equal(ui.elemento('edit_parcial_caja_5').hidden, true);
});
