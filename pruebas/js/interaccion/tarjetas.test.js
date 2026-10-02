// Pestaña Tarjetas: alta, límites, abonos (con conversión), avances de efectivo
// y bonificaciones. Cada campo de los formularios lleva un valor DISTINTO para
// que un par de argumentos intercambiados no pase inadvertido. Datos inventados.

import test from 'node:test';
import assert from 'node:assert/strict';
import { cargarInterfaz, crearEvento, AUSENTE } from '../ayudas/cargar_interfaz.js';
import {
    llamoUnaVez, noLlamoANada, noLlamoA, avisoExito, unAvisoDeError, redibujo, noRedibujo, rechazoSeMuestra,
} from '../ayudas/afirmar.js';

const MOTIVO = 'Se registró contra la tarjeta equivocada por error';

// --- alta de tarjeta ---

const alta = {
    tar_ent: 'Banco de Prueba', tar_nom: 'Tarjeta Uno',
    tar_lim_dop: '1001.01', tar_sob_dop: '2002.02', tar_bal_dop: '3003.03', tar_cor_dop: '4004.04',
    tar_lim_usd: '5005.05', tar_sob_usd: '6006.06', tar_bal_usd: '7007.07', tar_cor_usd: '8008.08',
    tar_cor: '15', tar_pag: '5',
};

test('alta de tarjeta: cada importe llega en su posición (el orden de AppAPI.crearTarjeta no es el del formulario)', async () => {
    const ui = cargarInterfaz({ campos: alta });
    await ui.appUI.handleAgregarTarjeta(crearEvento());
    // (entidad, nombre, límDOP, límUSD, sobDOP, sobUSD, balDOP, balUSD, corteDOP, corteUSD, díaCorte, díaPago)
    llamoUnaVez(ui, 'crearTarjeta', [
        'Banco de Prueba', 'Tarjeta Uno',
        '1001.01', '5005.05', '2002.02', '6006.06', '3003.03', '7007.07', '4004.04', '8008.08', 15, 5,
    ]);
    avisoExito(ui, /Tarjeta registrada/);
    redibujo(ui, 'tarjetas');
});

test('alta de tarjeta: si Rust rechaza se muestra el error', async () => {
    const ui = cargarInterfaz({ campos: alta });
    ui.rechazar('crearTarjeta', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleAgregarTarjeta(crearEvento()));
});

// --- límites de una tarjeta ---

const limites = {
    edit_lim_dop_3: '1000.10', edit_sob_dop_3: '200.20', edit_cor_dop_3: '300.30',
    edit_lim_usd_3: '4000.40', edit_sob_usd_3: '500.50', edit_cor_usd_3: '600.60',
    edit_aju_dop_3: '', edit_aju_usd_3: '', edit_pol_3: 'origen', 'modal-edit-tar-3': {},
};

test('editar límites: orden de argumentos, ajustes vacíos = null, cierra el modal', async () => {
    const ui = cargarInterfaz({ campos: limites });
    await ui.appUI.handleEdicionLimitesTarjetaSubmit(crearEvento(), 3);
    // (id, límDOP, límUSD, sobDOP, sobUSD, corteDOP, corteUSD, ajusteDOP, ajusteUSD, política)
    llamoUnaVez(ui, 'actualizarLimitesTarjeta', [3, '1000.10', '4000.40', '200.20', '500.50', '300.30', '600.60', null, null, 'origen']);
    assert.deepEqual(ui.eliminados, ['modal-edit-tar-3']);
    redibujo(ui, 'tarjetas');
});

test('editar límites: un ajuste «0» es un tope deliberado (0), no «sin ajuste» (null)', async () => {
    const ui = cargarInterfaz({ campos: { ...limites, edit_aju_dop_3: '0', edit_aju_usd_3: '250.25' } });
    await ui.appUI.handleEdicionLimitesTarjetaSubmit(crearEvento(), 3);
    const args = ui.llamadasA('actualizarLimitesTarjeta')[0].args;
    assert.equal(args[7], '0');
    assert.equal(args[8], '250.25');
});

test('editar límites: política elegida viaja; sin selector o vacía vale «origen»', async () => {
    let ui = cargarInterfaz({ campos: { ...limites, edit_pol_3: 'destino' } });
    await ui.appUI.handleEdicionLimitesTarjetaSubmit(crearEvento(), 3);
    assert.equal(ui.llamadasA('actualizarLimitesTarjeta')[0].args[9], 'destino');

    ui = cargarInterfaz({ campos: { ...limites, edit_pol_3: AUSENTE } });
    await ui.appUI.handleEdicionLimitesTarjetaSubmit(crearEvento(), 3);
    assert.equal(ui.llamadasA('actualizarLimitesTarjeta')[0].args[9], 'origen');
});

test('editar límites: un ajuste mayor que el límite aprobado no se envía (DOP o USD)', async () => {
    for (const ajuste of [{ edit_aju_dop_3: '1000.11' }, { edit_aju_usd_3: '4000.41' }]) {
        const ui = cargarInterfaz({ campos: { ...limites, ...ajuste } });
        await ui.appUI.handleEdicionLimitesTarjetaSubmit(crearEvento(), 3);
        noLlamoANada(ui);
        unAvisoDeError(ui, /no puede superar al aprobado/);
        assert.deepEqual(ui.eliminados, []);
    }
});

test('editar límites: si Rust rechaza se muestra el error y el modal sigue abierto', async () => {
    const ui = cargarInterfaz({ campos: limites });
    ui.rechazar('actualizarLimitesTarjeta', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleEdicionLimitesTarjetaSubmit(crearEvento(), 3));
    assert.deepEqual(ui.eliminados, []);
});

// --- abono a una tarjeta ---

const abono = (extra = {}) => ({
    pag_fecha_3: '20/03/2026', pag_div_3: 'DOP', pag_monto_3: '321.45', pag_cuenta_3: '', pag_tasa_3: '', ...extra,
});

test('abono sin cuenta: (id, fecha, monto, divisa, null, 0) y sin consultar cuentas', async () => {
    const ui = cargarInterfaz({ campos: abono() });
    await ui.appUI.handleAbonoTarjeta(crearEvento(), 3);
    llamoUnaVez(ui, 'registrarPagoTarjeta', [3, '20/03/2026', '321.45', 'DOP', null, 0]);
    noLlamoA(ui, 'obtenerCuentas');
    avisoExito(ui, /Abono a tarjeta guardado/);
    redibujo(ui, 'tarjetas');
});

test('abono desde cuenta en la misma divisa: viaja el id de la cuenta elegida (número)', async () => {
    const ui = cargarInterfaz({
        campos: abono({ pag_cuenta_3: '8' }),
        api: { obtenerCuentas: [{ id: 7, divisa: 'DOP', nombre: 'Otra' }, { id: 8, divisa: 'DOP', nombre: 'Cuenta A' }] },
    });
    await ui.appUI.handleAbonoTarjeta(crearEvento(), 3);
    llamoUnaVez(ui, 'registrarPagoTarjeta', [3, '20/03/2026', '321.45', 'DOP', 8, 0]);
    assert.deepEqual(ui.preguntas, []);
});

test('abono USD desde cuenta en DOP con tasa tecleada: no se pregunta nada', async () => {
    const ui = cargarInterfaz({
        campos: abono({ pag_div_3: 'USD', pag_cuenta_3: '8', pag_tasa_3: '59.5' }),
        api: { obtenerCuentas: [{ id: 8, divisa: 'DOP', nombre: 'Cuenta A' }] },
    });
    await ui.appUI.handleAbonoTarjeta(crearEvento(), 3);
    llamoUnaVez(ui, 'registrarPagoTarjeta', [3, '20/03/2026', '321.45', 'USD', 8, 59.5]);
    assert.deepEqual(ui.preguntas, []);
});

test('abono USD desde cuenta en DOP sin tasa: la pide con prompt y la usa', async () => {
    const ui = cargarInterfaz({
        campos: abono({ pag_div_3: 'USD', pag_cuenta_3: '8' }),
        api: { obtenerCuentas: [{ id: 8, divisa: 'DOP', nombre: 'Cuenta A' }] },
        prompt: '58.25',
    });
    await ui.appUI.handleAbonoTarjeta(crearEvento(), 3);
    assert.equal(ui.preguntas.length, 1);
    llamoUnaVez(ui, 'registrarPagoTarjeta', [3, '20/03/2026', '321.45', 'USD', 8, 58.25]);
});

test('abono con conversión: cancelar el prompt de la tasa cancela el abono', async () => {
    const ui = cargarInterfaz({
        campos: abono({ pag_div_3: 'USD', pag_cuenta_3: '8' }),
        api: { obtenerCuentas: [{ id: 8, divisa: 'DOP', nombre: 'Cuenta A' }] },
        prompt: null,
    });
    await ui.appUI.handleAbonoTarjeta(crearEvento(), 3);
    noLlamoA(ui, 'registrarPagoTarjeta');
    assert.deepEqual(ui.avisos, [{ tipo: 'info', mensaje: 'Operación cancelada.' }]);
    noRedibujo(ui);
});

test('abono con conversión: una tasa inválida en el prompt (texto, 0, negativa) no se envía', async () => {
    for (const tasa of ['abc', '0', '-3']) {
        const ui = cargarInterfaz({
            campos: abono({ pag_div_3: 'USD', pag_cuenta_3: '8' }),
            api: { obtenerCuentas: [{ id: 8, divisa: 'DOP', nombre: 'Cuenta A' }] },
            prompt: tasa,
        });
        await ui.appUI.handleAbonoTarjeta(crearEvento(), 3);
        noLlamoA(ui, 'registrarPagoTarjeta');
        unAvisoDeError(ui, /Tasa de cambio inválida/);
    }
});

test('abono con monto cero, negativo o vacío: no se envía ni se consulta nada', async () => {
    for (const monto of ['0', '-1', '']) {
        const ui = cargarInterfaz({ campos: abono({ pag_monto_3: monto }) });
        await ui.appUI.handleAbonoTarjeta(crearEvento(), 3);
        noLlamoANada(ui);
        unAvisoDeError(ui, /mayor que cero/);
    }
});

test('abono: si no se pueden consultar las cuentas, no se envía el pago', async () => {
    const ui = cargarInterfaz({ campos: abono({ pag_cuenta_3: '8' }) });
    ui.rechazar('obtenerCuentas', 'Error simulado de Rust');
    await ui.appUI.handleAbonoTarjeta(crearEvento(), 3);
    noLlamoA(ui, 'registrarPagoTarjeta');
    unAvisoDeError(ui, /Error al validar cuenta: Error simulado de Rust/);
});

test('abono: si Rust rechaza el pago se muestra el error', async () => {
    const ui = cargarInterfaz({ campos: abono() });
    ui.rechazar('registrarPagoTarjeta', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleAbonoTarjeta(crearEvento(), 3));
});

// --- revertir un abono / un avance ---

test('revertir abono: confirma, pide motivo, envía (abono, motivo), redibuja y reabre los abonos de ESA tarjeta', async () => {
    // El desplegable de ESA tarjeta se reabre y consulta SUS abonos (el estado de la vista
    // ya no se espía desde fuera: se observa lo que hace).
    const ui = cargarInterfaz({
        confirm: true, prompt: MOTIVO,
        campos: { abonos_3: { hidden: true, innerHTML: '' } },
        api: { revertirAbonoTarjeta: 'Abono revertido (prueba).', obtenerAbonosTarjeta: [] },
    });
    await ui.appUI.handleRevertirAbono(41, 3);
    llamoUnaVez(ui, 'revertirAbonoTarjeta', [41, MOTIVO]);
    avisoExito(ui, /Abono revertido/);
    redibujo(ui, 'tarjetas');
    assert.deepEqual(ui.llamadasA('obtenerAbonosTarjeta').map(l => l.args), [[3]]);
    assert.equal(ui.elemento('abonos_3').hidden, false);
});

test('revertir abono: sin confirmar, motivo cancelado o motivo corto no envía nada', async () => {
    for (const [confirm, prompt] of [[false, MOTIVO], [true, null], [true, 'corto']]) {
        const ui = cargarInterfaz({ confirm, prompt });
        await ui.appUI.handleRevertirAbono(41, 3);
        noLlamoANada(ui);
        noRedibujo(ui);
    }
});

test('revertir abono: si Rust rechaza se muestra el error', async () => {
    const ui = cargarInterfaz({ confirm: true, prompt: MOTIVO });
    ui.rechazar('revertirAbonoTarjeta', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleRevertirAbono(41, 3));
});

test('revertir avance: confirma, pide motivo, envía (avance, motivo) y reabre los avances de ESA tarjeta', async () => {
    const ui = cargarInterfaz({
        confirm: true, prompt: MOTIVO,
        campos: { avances_3: { hidden: true, innerHTML: '' } },
        api: { revertirAvanceEfectivo: 'Avance revertido (prueba).', obtenerAvancesTarjeta: [] },
    });
    await ui.appUI.handleRevertirAvance(52, 3);
    llamoUnaVez(ui, 'revertirAvanceEfectivo', [52, MOTIVO]);
    avisoExito(ui, /Avance revertido/);
    redibujo(ui, 'tarjetas');
    assert.deepEqual(ui.llamadasA('obtenerAvancesTarjeta').map(l => l.args), [[3]]);
    assert.equal(ui.elemento('avances_3').hidden, false);
});

test('revertir avance: sin confirmar, motivo cancelado o corto no envía nada; si Rust rechaza, error', async () => {
    for (const [confirm, prompt] of [[false, MOTIVO], [true, null], [true, 'corto']]) {
        const ui = cargarInterfaz({ confirm, prompt });
        await ui.appUI.handleRevertirAvance(52, 3);
        noLlamoANada(ui);
    }
    const ui = cargarInterfaz({ confirm: true, prompt: MOTIVO });
    ui.rechazar('revertirAvanceEfectivo', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleRevertirAvance(52, 3));
});

// --- avance de efectivo ---

const avance = (extra = {}) => ({
    avc_fecha_3: '21/03/2026', avc_div_3: 'DOP', avc_monto_3: '500.50',
    avc_cuenta_3: { options: [{ value: '', text: 'Elige…' }, { value: '2', text: 'Cuenta A - 0000' }], selectedIndex: 1 },
    avc_nota_3: 'Nota de prueba', avc_tipo_3: 'porcentaje', avc_valor_3: '2.5', ...extra,
});
const simulacion = { monto: 500.5, cargo: 12.51, a_la_tarjeta: 513.01 };

test('avance con cargo porcentual: simula y registra; el monto viaja como TEXTO y el porcentaje como número', async () => {
    const ui = cargarInterfaz({
        campos: avance(), confirm: true,
        api: { simularAvanceEfectivo: simulacion, registrarAvanceEfectivo: 'Avance registrado (prueba).' },
    });
    await ui.appUI.handleAvanceEfectivo(crearEvento(), 3);
    llamoUnaVez(ui, 'simularAvanceEfectivo', ['500.50', 'DOP', 'porcentaje', 2.5, null]);
    llamoUnaVez(ui, 'registrarAvanceEfectivo', [3, '2', '21/03/2026', '500.50', 'DOP', 'porcentaje', 2.5, null, 'Nota de prueba']);
    assert.match(ui.confirmaciones[0].texto, /Cuenta A/);
    avisoExito(ui, /Avance registrado/);
    redibujo(ui, 'tarjetas');
});

test('avance con cargo fijo: el cargo viaja como TEXTO y sin porcentaje', async () => {
    const ui = cargarInterfaz({
        campos: avance({ avc_tipo_3: 'fijo', avc_valor_3: ' 25.00 ' }), confirm: true,
        api: { simularAvanceEfectivo: simulacion, registrarAvanceEfectivo: 'ok' },
    });
    await ui.appUI.handleAvanceEfectivo(crearEvento(), 3);
    llamoUnaVez(ui, 'registrarAvanceEfectivo', [3, '2', '21/03/2026', '500.50', 'DOP', 'fijo', null, '25.00', 'Nota de prueba']);
});

test('avance exonerado: ni porcentaje ni cargo fijo', async () => {
    const ui = cargarInterfaz({
        campos: avance({ avc_tipo_3: 'exonerado', avc_valor_3: '' }), confirm: true,
        api: { simularAvanceEfectivo: simulacion, registrarAvanceEfectivo: 'ok' },
    });
    await ui.appUI.handleAvanceEfectivo(crearEvento(), 3);
    llamoUnaVez(ui, 'registrarAvanceEfectivo', [3, '2', '21/03/2026', '500.50', 'DOP', 'exonerado', null, null, 'Nota de prueba']);
});

test('avance sin cuenta elegida: no se simula ni se registra', async () => {
    const ui = cargarInterfaz({
        campos: avance({ avc_cuenta_3: { options: [{ value: '', text: 'Elige…' }], selectedIndex: 0 } }),
    });
    await ui.appUI.handleAvanceEfectivo(crearEvento(), 3);
    noLlamoANada(ui);
    unAvisoDeError(ui, /Elige la cuenta/);
});

test('avance: si el titular no confirma lo simulado, no se registra', async () => {
    const ui = cargarInterfaz({ campos: avance(), confirm: false, api: { simularAvanceEfectivo: simulacion } });
    await ui.appUI.handleAvanceEfectivo(crearEvento(), 3);
    noLlamoA(ui, 'registrarAvanceEfectivo');
    assert.deepEqual(ui.avisos, []);
    noRedibujo(ui);
});

test('avance: si la simulación falla no se pregunta ni se registra; si falla el registro se muestra el error', async () => {
    let ui = cargarInterfaz({ campos: avance() });
    ui.rechazar('simularAvanceEfectivo', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleAvanceEfectivo(crearEvento(), 3));
    noLlamoA(ui, 'registrarAvanceEfectivo');
    assert.deepEqual(ui.confirmaciones, []);

    ui = cargarInterfaz({ campos: avance(), confirm: true, api: { simularAvanceEfectivo: simulacion } });
    ui.rechazar('registrarAvanceEfectivo', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleAvanceEfectivo(crearEvento(), 3));
});

// --- bonificaciones ---

const bonificacion = { bon_fecha: ' 22/03/2026 ', bon_tarjeta: '3', bon_divisa: 'DOP', bon_monto: '66.60', bon_concepto: ' Cashback de prueba ' };

test('bonificación: (fecha, tarjeta, monto, divisa, concepto) con textos recortados', async () => {
    const ui = cargarInterfaz({ campos: bonificacion });
    await ui.appUI.handleAgregarBonificacion(crearEvento());
    llamoUnaVez(ui, 'crearBonificacion', ['22/03/2026', 3, '66.60', 'DOP', 'Cashback de prueba']);
    avisoExito(ui, /Bonificación registrada/);
    redibujo(ui, 'tarjetas');
});

test('bonificación con monto no positivo o vacío: no se envía', async () => {
    for (const monto of ['0', '-5', '']) {
        const ui = cargarInterfaz({ campos: { ...bonificacion, bon_monto: monto } });
        await ui.appUI.handleAgregarBonificacion(crearEvento());
        noLlamoANada(ui);
        unAvisoDeError(ui, /mayor que cero/);
    }
});

test('bonificación sin concepto (o solo espacios): no se envía', async () => {
    for (const concepto of ['', '   ']) {
        const ui = cargarInterfaz({ campos: { ...bonificacion, bon_concepto: concepto } });
        await ui.appUI.handleAgregarBonificacion(crearEvento());
        noLlamoANada(ui);
        unAvisoDeError(ui, /Indica el concepto/);
    }
});

test('bonificación: si Rust rechaza se muestra el error', async () => {
    const ui = cargarInterfaz({ campos: bonificacion });
    ui.rechazar('crearBonificacion', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleAgregarBonificacion(crearEvento()));
});

test('revertir bonificación: envía el id (sin confirmación previa) y redibuja', async () => {
    const ui = cargarInterfaz();
    await ui.appUI.handleEliminarBonificacion(9);
    llamoUnaVez(ui, 'eliminarBonificacion', [9]);
    avisoExito(ui, /Bonificación revertida/);
    redibujo(ui, 'tarjetas');
});

test('revertir bonificación: si Rust rechaza se muestra el error', async () => {
    const ui = cargarInterfaz();
    ui.rechazar('eliminarBonificacion', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleEliminarBonificacion(9));
});
