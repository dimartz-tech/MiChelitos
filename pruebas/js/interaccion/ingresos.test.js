// Pestaña Ingresos: factura, ingreso informal, edición de una factura (también
// cobrada), cobros y borrados con motivo. Datos inventados.

import test from 'node:test';
import assert from 'node:assert/strict';
import { cargarInterfaz, crearEvento, AUSENTE } from '../ayudas/cargar_interfaz.js';
import {
    llamoUnaVez, noLlamoANada, avisoExito, unAvisoDeError, redibujo, noRedibujo, rechazoSeMuestra,
} from '../ayudas/afirmar.js';

const MOTIVO = 'La factura se emitió con el cliente equivocado';

// --- alta de factura ---

const factura = {
    num_fac: 'F-0001', fec_em: '10/02/2026', cli_nom: 'Cliente de Prueba', cli_rnc: '000000000',
    mon_tot: '1234.56', ret_por: '5',
};

test('alta de factura: el total viaja como texto, el porcentaje como número, los textos tal cual', async () => {
    const ui = cargarInterfaz({ campos: factura });
    await ui.appUI.handleAgregarIngreso(crearEvento());
    llamoUnaVez(ui, 'crearIngreso', [{
        numero_factura: 'F-0001', rnc_cliente: '000000000', nombre_cliente: 'Cliente de Prueba',
        fecha_emision: '10/02/2026', monto_total: '1234.56', porcentaje_retencion: 5,
    }]);
    avisoExito(ui, /Factura registrada/);
    redibujo(ui, 'ingresos');
});

test('alta de factura: si Rust rechaza se muestra el error y no se redibuja', async () => {
    const ui = cargarInterfaz({ campos: factura });
    ui.rechazar('crearIngreso', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleAgregarIngreso(crearEvento()));
});

test('elegir un cliente guardado rellena nombre y RNC de la factura', () => {
    const ui = cargarInterfaz({
        campos: {
            cli_select: {
                options: [
                    { value: '', text: 'Nuevo' },
                    { value: '3', text: 'Cliente A', attrs: { 'data-nombre': 'Cliente A', 'data-rnc': '111111111' } },
                ],
                selectedIndex: 1,
            },
            cli_nom: '', cli_rnc: '',
        },
    });
    ui.appUI.handleSelectCliente('3');
    assert.equal(ui.elemento('cli_nom').value, 'Cliente A');
    assert.equal(ui.elemento('cli_rnc').value, '111111111');
});

test('elegir «nuevo cliente» deja nombre y RNC vacíos', () => {
    const ui = cargarInterfaz({
        campos: {
            cli_select: { options: [{ value: '', text: 'Nuevo' }], selectedIndex: 0 },
            cli_nom: 'Resto', cli_rnc: 'Resto',
        },
    });
    ui.appUI.handleSelectCliente('');
    assert.equal(ui.elemento('cli_nom').value, '');
    assert.equal(ui.elemento('cli_rnc').value, '');
});

// --- ingreso informal ---

const informal = { fecha_inf: '11/02/2026', monto_inf: '777.77', desc_inf: 'Trabajo de prueba' };

test('ingreso informal: se envía (fecha, descripción, monto) en ese orden', async () => {
    const ui = cargarInterfaz({ campos: informal });
    await ui.appUI.handleAgregarIngresoInformal(crearEvento());
    llamoUnaVez(ui, 'crearIngresoInformal', ['11/02/2026', 'Trabajo de prueba', '777.77']);
    avisoExito(ui, /Ingreso informal guardado/);
    redibujo(ui, 'ingresos');
});

test('ingreso informal: si Rust rechaza se muestra el error', async () => {
    const ui = cargarInterfaz({ campos: informal });
    ui.rechazar('crearIngresoInformal', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleAgregarIngresoInformal(crearEvento()));
});

// --- edición de una factura ---

function edicion({ cobrada = false, recibido = '0', mon = '1200.40', ret = '10', parcial, parcialMarcado } = {}) {
    const campos = {
        edit_num_fac_5: 'F-0002', edit_fec_em_5: '12/02/2026', edit_cli_select_5: '3',
        edit_mon_tot_5: mon, edit_ret_por_5: ret,
        'modal-edit-for-5': { dataset: { cobrada: cobrada ? 'si' : 'no', recibido } },
        edit_parcial_chk_5: cobrada ? { checked: !!parcialMarcado } : AUSENTE,
    };
    if (parcial !== undefined) campos.edit_parcial_mon_5 = parcial;
    return campos;
}

test('editar factura no cobrada: se envía sin parcial ni motivo, y se cierra el modal', async () => {
    const ui = cargarInterfaz({ campos: edicion(), api: { actualizarIngreso: 'Resumen de prueba' } });
    await ui.appUI.handleEdicionFormalSubmit(crearEvento(), 5);
    llamoUnaVez(ui, 'actualizarIngreso', [5, 'F-0002', 3, '12/02/2026', '1200.40', 10, null, null]);
    avisoExito(ui, /Resumen de prueba/);
    assert.deepEqual(ui.eliminados, ['modal-edit-for-5']);
    redibujo(ui, 'ingresos');
    assert.deepEqual(ui.preguntas, [], 'no se pide motivo si no hay nada cobrado');
});

test('editar factura: si la API no devuelve resumen, el aviso es el genérico', async () => {
    const ui = cargarInterfaz({ campos: edicion() });
    await ui.appUI.handleEdicionFormalSubmit(crearEvento(), 5);
    avisoExito(ui, /Factura corregida/);
});

test('editar factura cobrada cuyo neto cambia: pide motivo y lo envía', async () => {
    // neto = 1200,40 − 10 % = 1080,36; antes se recibieron 800,80 → hay ajuste.
    const ui = cargarInterfaz({ campos: edicion({ cobrada: true, recibido: '800.80' }), prompt: MOTIVO });
    await ui.appUI.handleEdicionFormalSubmit(crearEvento(), 5);
    llamoUnaVez(ui, 'actualizarIngreso', [5, 'F-0002', 3, '12/02/2026', '1200.40', 10, null, MOTIVO]);
    assert.equal(ui.preguntas.length, 1);
});

test('editar factura cobrada: cancelar el motivo no envía nada ni cierra el modal', async () => {
    const ui = cargarInterfaz({ campos: edicion({ cobrada: true, recibido: '800.80' }), prompt: null });
    await ui.appUI.handleEdicionFormalSubmit(crearEvento(), 5);
    noLlamoANada(ui);
    assert.deepEqual(ui.eliminados, []);
    noRedibujo(ui);
});

test('editar factura cobrada: un motivo corto se rechaza', async () => {
    const ui = cargarInterfaz({ campos: edicion({ cobrada: true, recibido: '800.80' }), prompt: 'corto' });
    await ui.appUI.handleEdicionFormalSubmit(crearEvento(), 5);
    noLlamoANada(ui);
    unAvisoDeError(ui, /demasiado corto/);
});

test('editar factura cobrada sin cambio de saldo: pide confirmación, no motivo', async () => {
    const ui = cargarInterfaz({ campos: edicion({ cobrada: true, recibido: '1080.36' }), confirm: true });
    await ui.appUI.handleEdicionFormalSubmit(crearEvento(), 5);
    llamoUnaVez(ui, 'actualizarIngreso', [5, 'F-0002', 3, '12/02/2026', '1200.40', 10, null, null]);
    assert.equal(ui.confirmaciones.length, 1);
    assert.deepEqual(ui.preguntas, []);
});

test('editar factura cobrada sin cambio de saldo: rechazar la confirmación no envía nada', async () => {
    const ui = cargarInterfaz({ campos: edicion({ cobrada: true, recibido: '1080.36' }), confirm: false });
    await ui.appUI.handleEdicionFormalSubmit(crearEvento(), 5);
    noLlamoANada(ui);
});

test('cobro parcial: el importe viaja como TEXTO tal como se escribió', async () => {
    const ui = cargarInterfaz({
        campos: edicion({ cobrada: true, recibido: '800.80', parcial: '450.50', parcialMarcado: true }),
        prompt: MOTIVO,
    });
    await ui.appUI.handleEdicionFormalSubmit(crearEvento(), 5);
    llamoUnaVez(ui, 'actualizarIngreso', [5, 'F-0002', 3, '12/02/2026', '1200.40', 10, '450.50', MOTIVO]);
});

test('cobro parcial marcado y vacío o con forma inválida: no se envía', async () => {
    for (const parcial of ['', '  ', 'abc', '-3', '1.2.3']) {
        const ui = cargarInterfaz({
            campos: edicion({ cobrada: true, recibido: '800.80', parcial, parcialMarcado: true }),
        });
        await ui.appUI.handleEdicionFormalSubmit(crearEvento(), 5);
        noLlamoANada(ui);
        unAvisoDeError(ui, /cuánto se cobró/);
    }
});

test('cobro parcial mayor que el neto: no se envía y el aviso dice el neto', async () => {
    const ui = cargarInterfaz({
        campos: edicion({ cobrada: true, recibido: '800.80', parcial: '1080.37', parcialMarcado: true }),
    });
    await ui.appUI.handleEdicionFormalSubmit(crearEvento(), 5);
    noLlamoANada(ui);
    unAvisoDeError(ui, /no puede superar el neto/);
});

test('editar factura: si Rust rechaza se muestra el error y el modal sigue abierto', async () => {
    const ui = cargarInterfaz({ campos: edicion() });
    ui.rechazar('actualizarIngreso', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleEdicionFormalSubmit(crearEvento(), 5));
    assert.deepEqual(ui.eliminados, []);
});

// --- cobros ---

test('cobro de factura: id de factura, cuenta (tal cual sale del selector), fecha y monto numérico', async () => {
    const ui = cargarInterfaz({
        campos: { cob_ban_5: '2', cob_fec_5: '13/02/2026', cob_mon_5: '888.88', 'modal-cobro-for-5': {} },
    });
    await ui.appUI.handleCobroFormalSubmit(crearEvento(), 5);
    llamoUnaVez(ui, 'marcarIngresoPagado', [5, '2', '13/02/2026', '888.88']);
    avisoExito(ui, /marcada como pagada/);
    assert.deepEqual(ui.eliminados, ['modal-cobro-for-5']);
    redibujo(ui, 'ingresos');
});

test('cobro de factura: si Rust rechaza se muestra el error y el modal sigue abierto', async () => {
    const ui = cargarInterfaz({
        campos: { cob_ban_5: '2', cob_fec_5: '13/02/2026', cob_mon_5: '888.88', 'modal-cobro-for-5': {} },
    });
    ui.rechazar('marcarIngresoPagado', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleCobroFormalSubmit(crearEvento(), 5));
    assert.deepEqual(ui.eliminados, []);
});

test('cobro informal: id, cuenta, fecha y monto numérico; cierra su modal', async () => {
    const ui = cargarInterfaz({
        campos: { cob_ban_inf_6: '2', cob_fec_inf_6: '14/02/2026', cob_mon_inf_6: '555.55', 'modal-cobro-inf-6': {} },
    });
    await ui.appUI.handleCobroInformalSubmit(crearEvento(), 6);
    llamoUnaVez(ui, 'marcarInformalPagado', [6, '2', '14/02/2026', '555.55']);
    avisoExito(ui, /registrado como pagado/);
    assert.deepEqual(ui.eliminados, ['modal-cobro-inf-6']);
    redibujo(ui, 'ingresos');
});

test('cobro informal: si Rust rechaza se muestra el error y el modal sigue abierto', async () => {
    const ui = cargarInterfaz({
        campos: { cob_ban_inf_6: '2', cob_fec_inf_6: '14/02/2026', cob_mon_inf_6: '555.55', 'modal-cobro-inf-6': {} },
    });
    ui.rechazar('marcarInformalPagado', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleCobroInformalSubmit(crearEvento(), 6));
    assert.deepEqual(ui.eliminados, []);
});

// --- borrados con motivo (Ajustes → correcciones) ---

// Se llama con `ui.appUI.<método>(` escrito en claro: `cobertura.test.js` lo busca así.
for (const [manejador, metodo, ruta, llamar] of [
    ['handleEliminarIngresoCorr', 'eliminarIngreso', 'ajustes', ui => ui.appUI.handleEliminarIngresoCorr(31)],
    ['handleEliminarIngresoInformalCorr', 'eliminarIngresoInformal', 'ajustes', ui => ui.appUI.handleEliminarIngresoInformalCorr(31)],
]) {
    test(`${manejador}: confirma, pide motivo y envía id + motivo`, async () => {
        const ui = cargarInterfaz({ confirm: true, prompt: MOTIVO });
        await llamar(ui);
        llamoUnaVez(ui, metodo, [31, MOTIVO]);
        avisoExito(ui);
        redibujo(ui, ruta);
    });

    test(`${manejador}: sin confirmar, con motivo cancelado o corto no se envía nada`, async () => {
        for (const [confirm, prompt] of [[false, MOTIVO], [true, null], [true, 'corto']]) {
            const ui = cargarInterfaz({ confirm, prompt });
            await llamar(ui);
            noLlamoANada(ui);
            noRedibujo(ui);
        }
    });

    test(`${manejador}: si Rust rechaza se muestra el error`, async () => {
        const ui = cargarInterfaz({ confirm: true, prompt: MOTIVO });
        ui.rechazar(metodo, 'Error simulado de Rust');
        await rechazoSeMuestra(ui, () => llamar(ui));
    });
}
