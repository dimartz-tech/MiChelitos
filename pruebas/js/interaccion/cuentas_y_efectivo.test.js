// Pestañas Cuentas y Efectivo, y el alta/edición/baja de cuentas de Ajustes.
// Datos inventados.

import test from 'node:test';
import assert from 'node:assert/strict';
import { cargarInterfaz, crearEvento } from '../ayudas/cargar_interfaz.js';
import {
    llamoUnaVez, noLlamoANada, noLlamoA, avisoExito, unAvisoDeError, redibujo, noRedibujo, rechazoSeMuestra,
} from '../ayudas/afirmar.js';

const MOTIVO = 'El traspaso se hizo entre las cuentas equivocadas';

// --- alta de cuenta (Ajustes) ---

const cuenta = { cue_aj_nom: 'Cuenta de Prueba', cue_aj_div: 'USD', cue_aj_bal: '1500.75', cue_aj_ent: 'Banco de Prueba', cue_aj_com: '' };

test('alta de cuenta: (nombre, divisa, balance como texto, entidad, comisión); comisión en blanco = null, no 0', async () => {
    const ui = cargarInterfaz({ campos: cuenta });
    await ui.appUI.handleAgregarCuenta(crearEvento());
    llamoUnaVez(ui, 'crearCuenta', ['Cuenta de Prueba', 'USD', '1500.75', 'Banco de Prueba', null]);
    avisoExito(ui, /Cuenta de ahorro registrada/);
    redibujo(ui, 'ajustes');
});

test('alta de cuenta: la comisión viaja como TEXTO tal cual se escribió, y «0» es una tarifa gratuita, no null', async () => {
    let ui = cargarInterfaz({ campos: { ...cuenta, cue_aj_com: '12.34' } });
    await ui.appUI.handleAgregarCuenta(crearEvento());
    assert.equal(ui.llamadasA('crearCuenta')[0].args[4], '12.34', 'la comisión viaja como TEXTO, no como número');

    ui = cargarInterfaz({ campos: { ...cuenta, cue_aj_com: '0' } });
    await ui.appUI.handleAgregarCuenta(crearEvento());
    assert.equal(ui.llamadasA('crearCuenta')[0].args[4], '0', 'un cero escrito es un cero, no «sin comisión»');

    ui = cargarInterfaz({ campos: { ...cuenta, cue_aj_com: '  7.5  ' } });
    await ui.appUI.handleAgregarCuenta(crearEvento());
    assert.equal(ui.llamadasA('crearCuenta')[0].args[4], '7.5', 'sin los espacios, pero sin convertir');
});

test('alta de cuenta: si Rust rechaza se muestra el error', async () => {
    const ui = cargarInterfaz({ campos: cuenta });
    ui.rechazar('crearCuenta', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleAgregarCuenta(crearEvento()));
});

// --- edición de cuenta (tres prompt) ---

const existente = { id: 4, nombre: 'Vieja', entidad: 'Entidad Vieja', comision_pago_impuestos: 5 };

test('editar cuenta: nombre, entidad y comisión por prompt; la comisión viaja como TEXTO', async () => {
    const ui = cargarInterfaz({ prompt: ['Cuenta Nueva', 'Entidad Nueva', ' 12.505 '] });
    await ui.appUI.abrirEdicionCuenta(existente);
    llamoUnaVez(ui, 'actualizarCuenta', [4, 'Cuenta Nueva', 'Entidad Nueva', '12.505']);
    avisoExito(ui, /Cuenta actualizada/);
    redibujo(ui, 'ajustes');
});

test('editar cuenta: comisión vacía = null («no declarada»)', async () => {
    const ui = cargarInterfaz({ prompt: ['Cuenta Nueva', 'Entidad Nueva', '   '] });
    await ui.appUI.abrirEdicionCuenta(existente);
    llamoUnaVez(ui, 'actualizarCuenta', [4, 'Cuenta Nueva', 'Entidad Nueva', null]);
});

test('editar cuenta: cancelar cualquiera de los tres prompt no envía nada', async () => {
    for (const cola of [[null], ['A', null], ['A', 'B', null]]) {
        const ui = cargarInterfaz({ prompt: cola });
        await ui.appUI.abrirEdicionCuenta(existente);
        noLlamoANada(ui);
        assert.deepEqual(ui.avisos, []);
    }
});

test('editar cuenta: comisión con forma inválida o negativa no se envía', async () => {
    for (const [texto, aviso] of [['abc', /importe válido/], ['1.2.3', /importe válido/], ['-4', /no puede ser negativa/]]) {
        const ui = cargarInterfaz({ prompt: ['A', 'B', texto] });
        await ui.appUI.abrirEdicionCuenta(existente);
        noLlamoANada(ui);
        unAvisoDeError(ui, aviso);
    }
});

test('editar cuenta: si Rust rechaza se muestra el error', async () => {
    const ui = cargarInterfaz({ prompt: ['A', 'B', '1'] });
    ui.rechazar('actualizarCuenta', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.abrirEdicionCuenta(existente));
});

// --- baja de cuenta ---

test('baja de cuenta: con confirmación envía el id y redibuja Ajustes', async () => {
    const ui = cargarInterfaz({ confirm: true });
    await ui.appUI.handleEliminarCuenta(4);
    llamoUnaVez(ui, 'eliminarCuenta', [4]);
    avisoExito(ui, /Cuenta eliminada/);
    redibujo(ui, 'ajustes');
});

test('baja de cuenta: sin confirmación no se envía nada; si Rust rechaza se muestra el error', async () => {
    let ui = cargarInterfaz({ confirm: false });
    await ui.appUI.handleEliminarCuenta(4);
    noLlamoANada(ui);

    ui = cargarInterfaz({ confirm: true });
    ui.rechazar('eliminarCuenta', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleEliminarCuenta(4));
});

// --- transferencia entre cuentas ---

const transferencia = {
    tra_fec: '23/03/2026', tra_ori: '4', tra_des: '5', tra_mon_ori: '1000.10', tra_mon_des: '999.90',
    tra_car: '15.15', tra_des_txt: 'Traspaso de prueba',
};

test('transferir: origen, destino, importes (débito ≠ crédito) y cargo cada uno en su sitio', async () => {
    const ui = cargarInterfaz({ campos: transferencia });
    await ui.appUI.handleTransferirCuentas(crearEvento());
    // (fecha, origen, destino, montoOrigen, montoDestino, cargo, descripción)
    llamoUnaVez(ui, 'transferirEntreCuentas', ['23/03/2026', 4, 5, 1000.1, 999.9, 15.15, 'Traspaso de prueba']);
    avisoExito(ui, /Transacción ejecutada/);
    redibujo(ui, 'cuentas');
});

test('transferir: si Rust rechaza se muestra el error y no se redibuja', async () => {
    const ui = cargarInterfaz({ campos: transferencia });
    ui.rechazar('transferirEntreCuentas', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleTransferirCuentas(crearEvento()));
});

// --- borrar una transferencia (Ajustes → correcciones) ---

test('borrar transferencia: confirma, pide motivo y envía id + motivo', async () => {
    const ui = cargarInterfaz({ confirm: true, prompt: MOTIVO });
    await ui.appUI.handleEliminarTransaccionCuentaCorr(61);
    llamoUnaVez(ui, 'eliminarTransaccionCuenta', [61, MOTIVO]);
    avisoExito(ui, /Transferencia revertida/);
    redibujo(ui, 'ajustes');
});

test('borrar transferencia: sin confirmar, motivo cancelado o corto no envía nada; si Rust rechaza, error', async () => {
    for (const [confirm, prompt] of [[false, MOTIVO], [true, null], [true, 'corto']]) {
        const ui = cargarInterfaz({ confirm, prompt });
        await ui.appUI.handleEliminarTransaccionCuentaCorr(61);
        noLlamoANada(ui);
    }
    const ui = cargarInterfaz({ confirm: true, prompt: MOTIVO });
    ui.rechazar('eliminarTransaccionCuenta', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleEliminarTransaccionCuentaCorr(61));
});

// --- Efectivo: entrada informal ---

const entrada = { efe_inf_fec: '24/03/2026', efe_inf_mon: '321.10', efe_inf_div: 'USD', efe_inf_des: 'Cobro de prueba' };

test('entrada de efectivo informal: (fecha, descripción, monto, divisa) en ese orden', async () => {
    const ui = cargarInterfaz({ campos: entrada });
    await ui.appUI.handleAgregarEfectivoInformal(crearEvento());
    llamoUnaVez(ui, 'crearCobroEfectivoInformal', ['24/03/2026', 'Cobro de prueba', '321.10', 'USD']);
    avisoExito(ui, /Entrada en efectivo registrada/);
    redibujo(ui, 'efectivo');
});

test('entrada de efectivo informal: si Rust rechaza se muestra el error', async () => {
    const ui = cargarInterfaz({ campos: entrada });
    ui.rechazar('crearCobroEfectivoInformal', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleAgregarEfectivoInformal(crearEvento()));
});

// --- Efectivo: retirar de una cuenta ---

const retiro = { efe_ret_fec: '25/03/2026', efe_ret_ori: '4', efe_ret_mon: '200.20', efe_ret_car: '3.30', efe_ret_des_txt: 'Retiro de prueba' };
const cuentas = [
    { id: 4, nombre: 'Cuenta DOP', divisa: 'DOP' },
    { id: 5, nombre: 'Cuenta USD', divisa: 'USD' },
    { id: 11, nombre: 'Efectivo DOP', divisa: 'DOP' },
    { id: 12, nombre: 'Efectivo USD', divisa: 'USD' },
];

test('retirar a efectivo desde una cuenta en DOP: destino «Efectivo DOP», mismo monto en ambos lados', async () => {
    const ui = cargarInterfaz({ campos: retiro, api: { obtenerCuentas: cuentas } });
    await ui.appUI.handleRetirarAEfectivo(crearEvento());
    llamoUnaVez(ui, 'transferirEntreCuentas', ['25/03/2026', 4, 11, 200.2, 200.2, 3.3, 'Retiro de prueba']);
    avisoExito(ui, /Retiro de efectivo ejecutado/);
    redibujo(ui, 'efectivo');
});

test('retirar a efectivo desde una cuenta en USD: destino «Efectivo USD»', async () => {
    const ui = cargarInterfaz({ campos: { ...retiro, efe_ret_ori: '5' }, api: { obtenerCuentas: cuentas } });
    await ui.appUI.handleRetirarAEfectivo(crearEvento());
    llamoUnaVez(ui, 'transferirEntreCuentas', ['25/03/2026', 5, 12, 200.2, 200.2, 3.3, 'Retiro de prueba']);
});

test('retirar a efectivo: cuenta origen inexistente o sin cuenta de efectivo destino no envía nada', async () => {
    let ui = cargarInterfaz({ campos: { ...retiro, efe_ret_ori: '99' }, api: { obtenerCuentas: cuentas } });
    await ui.appUI.handleRetirarAEfectivo(crearEvento());
    noLlamoA(ui, 'transferirEntreCuentas');
    unAvisoDeError(ui, /Cuenta origen no encontrada/);

    ui = cargarInterfaz({ campos: retiro, api: { obtenerCuentas: cuentas.filter(c => c.id !== 11) } });
    await ui.appUI.handleRetirarAEfectivo(crearEvento());
    noLlamoA(ui, 'transferirEntreCuentas');
    unAvisoDeError(ui, /Cuenta destino Efectivo DOP no encontrada/);
    noRedibujo(ui);
});

test('retirar a efectivo: si Rust rechaza la transferencia se muestra el error', async () => {
    const ui = cargarInterfaz({ campos: retiro, api: { obtenerCuentas: cuentas } });
    ui.rechazar('transferirEntreCuentas', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleRetirarAEfectivo(crearEvento()));
});
