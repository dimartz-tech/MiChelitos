// Pestaña Préstamos: alta, edición de condiciones, saldo del estado de cuenta,
// pago de cuota y baja. Datos inventados.

import test from 'node:test';
import assert from 'node:assert/strict';
import { cargarInterfaz, crearEvento, AUSENTE } from '../ayudas/cargar_interfaz.js';
import {
    llamoUnaVez, noLlamoANada, avisoExito, unAvisoDeError, redibujo, noRedibujo, rechazoSeMuestra,
} from '../ayudas/afirmar.js';

// --- alta ---

const alta = (extra = {}) => ({
    pre_tip: 'personal', pre_ins: 'Institución de Prueba', pre_mon: '10001.11', pre_tas: '12.5',
    pre_tot: '24', pre_pen: '20', pre_cuo: '555.55', pre_dia: '15', pre_sal: '9000.99', pre_lim: '', ...extra,
});
const prestamo = {
    tipo_prestamo: 'personal', monto_prestamo: 10001.11, institucion_financiera: 'Institución de Prueba',
    tasa_actual: 12.5, cuotas_totales: 24, cuotas_pendientes: 20, monto_cuota: 555.55, dia_pago: 15,
    saldo_actual: 9000.99, limite_credito: null,
};

test('alta de préstamo: cada campo llega a su nombre; el límite solo se envía a las líneas flexibles', async () => {
    const ui = cargarInterfaz({ campos: alta({ pre_lim: '3000.30' }) });
    await ui.appUI.handleAgregarPrestamo(crearEvento());
    llamoUnaVez(ui, 'crearPrestamo', [prestamo]);   // tipo «personal» ignora el límite escrito
    avisoExito(ui, /Financiamiento registrado/);
    redibujo(ui, 'prestamos');
});

test('alta de línea flexible: el límite de crédito viaja como número', async () => {
    const ui = cargarInterfaz({ campos: alta({ pre_tip: 'flexible', pre_lim: '3000.30' }) });
    await ui.appUI.handleAgregarPrestamo(crearEvento());
    llamoUnaVez(ui, 'crearPrestamo', [{ ...prestamo, tipo_prestamo: 'flexible', limite_credito: 3000.3 }]);
});

test('alta de préstamo: cuotas y saldo en blanco (o campos ausentes) = null, no 0', async () => {
    const ui = cargarInterfaz({
        campos: alta({ pre_tot: '', pre_pen: AUSENTE, pre_sal: '', pre_lim: AUSENTE }),
    });
    await ui.appUI.handleAgregarPrestamo(crearEvento());
    llamoUnaVez(ui, 'crearPrestamo', [{ ...prestamo, cuotas_totales: null, cuotas_pendientes: null, saldo_actual: null }]);
});

test('alta de préstamo: si Rust rechaza se muestra el error', async () => {
    const ui = cargarInterfaz({ campos: alta() });
    ui.rechazar('crearPrestamo', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleAgregarPrestamo(crearEvento()));
});

// --- edición de condiciones ---

const edicion = (extra = {}) => ({
    edp_lim_6: '', edp_tar_6: '', edp_tas_6: '11.75', edp_cuo_6: '444.44', edp_dia_6: '10',
    'modal-pre-6': {}, ...extra,
});

test('editar condiciones: (id, tasa, cuota, día) por su nombre; sin límite ni tarjeta = null; cierra el modal', async () => {
    const ui = cargarInterfaz({ campos: edicion() });
    await ui.appUI.handleEdicionPrestamo(crearEvento(), 6);
    llamoUnaVez(ui, 'actualizarPrestamo', [{
        id: 6, tasa_actual: 11.75, monto_cuota: 444.44, dia_pago: 10, limite_credito: null, tarjeta_id: null,
    }]);
    avisoExito(ui, /Condiciones actualizadas/);
    assert.deepEqual(ui.eliminados, ['modal-pre-6']);
    redibujo(ui, 'prestamos');
});

test('editar condiciones: límite y tarjeta vinculada viajan como número; límite «0» es 0, no null', async () => {
    let ui = cargarInterfaz({ campos: edicion({ edp_lim_6: '2500.25', edp_tar_6: '3' }) });
    await ui.appUI.handleEdicionPrestamo(crearEvento(), 6);
    let args = ui.llamadasA('actualizarPrestamo')[0].args[0];
    assert.equal(args.limite_credito, 2500.25);
    assert.equal(args.tarjeta_id, 3);

    ui = cargarInterfaz({ campos: edicion({ edp_lim_6: '0' }) });
    await ui.appUI.handleEdicionPrestamo(crearEvento(), 6);
    assert.equal(ui.llamadasA('actualizarPrestamo')[0].args[0].limite_credito, 0);
});

test('editar condiciones: sin campo de límite (préstamo no flexible) se envía null', async () => {
    const ui = cargarInterfaz({ campos: edicion({ edp_lim_6: AUSENTE }) });
    await ui.appUI.handleEdicionPrestamo(crearEvento(), 6);
    assert.equal(ui.llamadasA('actualizarPrestamo')[0].args[0].limite_credito, null);
});

test('editar condiciones: si Rust rechaza se muestra el error y el modal sigue abierto', async () => {
    const ui = cargarInterfaz({ campos: edicion() });
    ui.rechazar('actualizarPrestamo', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleEdicionPrestamo(crearEvento(), 6));
    assert.deepEqual(ui.eliminados, []);
});

// --- declarar el saldo del estado de cuenta ---

test('declarar saldo: el importe viaja como TEXTO recortado, con el id; el prompt ofrece el saldo estimado', async () => {
    const ui = cargarInterfaz({ prompt: ' 8123.457 ' });
    await ui.appUI.handleDeclararSaldo(6, 9000.99);
    llamoUnaVez(ui, 'declararSaldoPrestamo', [6, '8123.457']);
    assert.equal(ui.preguntas[0].resto[0], '9000.99', 'valor propuesto');
    avisoExito(ui, /Saldo conciliado/);
    redibujo(ui, 'prestamos');
});

test('declarar saldo: un saldo negativo es válido (a favor)', async () => {
    const ui = cargarInterfaz({ prompt: '-12.50' });
    await ui.appUI.handleDeclararSaldo(6, 9000.99);
    llamoUnaVez(ui, 'declararSaldoPrestamo', [6, '-12.50']);
});

test('declarar saldo: cancelar no hace nada; texto inválido no se envía', async () => {
    let ui = cargarInterfaz({ prompt: null });
    await ui.appUI.handleDeclararSaldo(6, 9000.99);
    noLlamoANada(ui);
    assert.deepEqual(ui.avisos, []);

    for (const texto of ['', 'abc', '12.3.4', '1e5']) {
        ui = cargarInterfaz({ prompt: texto });
        await ui.appUI.handleDeclararSaldo(6, 9000.99);
        noLlamoANada(ui);
        unAvisoDeError(ui, /importe válido/);
    }
});

test('declarar saldo: si Rust rechaza se muestra el error', async () => {
    const ui = cargarInterfaz({ prompt: '100.00' });
    ui.rechazar('declararSaldoPrestamo', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleDeclararSaldo(6, 9000.99));
});

// --- pagar cuota / eliminar ---

test('pagar cuota: envía el id (sin confirmación previa) y redibuja', async () => {
    const ui = cargarInterfaz();
    await ui.appUI.handlePagarCuota(6);
    llamoUnaVez(ui, 'pagarCuotaPrestamo', [6]);
    avisoExito(ui, /Abono de cuota registrado/);
    redibujo(ui, 'prestamos');
});

test('pagar cuota: si Rust rechaza se muestra el error', async () => {
    const ui = cargarInterfaz();
    ui.rechazar('pagarCuotaPrestamo', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handlePagarCuota(6));
});

test('eliminar préstamo: con confirmación envía el id; sin ella no hace nada', async () => {
    let ui = cargarInterfaz({ confirm: true });
    await ui.appUI.handleEliminarPrestamo(6);
    llamoUnaVez(ui, 'eliminarPrestamo', [6]);
    avisoExito(ui, /Registro eliminado/);
    redibujo(ui, 'prestamos');

    ui = cargarInterfaz({ confirm: false });
    await ui.appUI.handleEliminarPrestamo(6);
    noLlamoANada(ui);
    noRedibujo(ui);
});

test('eliminar préstamo: si Rust rechaza se muestra el error', async () => {
    const ui = cargarInterfaz({ confirm: true });
    ui.rechazar('eliminarPrestamo', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleEliminarPrestamo(6));
});
