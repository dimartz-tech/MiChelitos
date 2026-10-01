// Pestaña Suscripciones: alta, edición, baja y los tres atajos de períodos
// pendientes / fecha de próximo cobro. Datos inventados.

import test from 'node:test';
import assert from 'node:assert/strict';
import { cargarInterfaz, crearEvento } from '../ayudas/cargar_interfaz.js';
import {
    llamoUnaVez, noLlamoANada, avisoExito, unAvisoDeError, redibujo, noRedibujo, rechazoSeMuestra,
} from '../ayudas/afirmar.js';

const MOTIVO = 'El proveedor no cobró ese período según el estado';

// --- alta ---

const alta = {
    sus_pla: 'Servicio de Prueba', sus_mon: ' 9.99 ', sus_div: 'USD', sus_dia: '17', sus_fre: 'anual',
    sus_tar: '3', sus_ren: '2026-04-17',
};

test('alta de suscripción: el monto viaja como TEXTO recortado y la fecha ISO se convierte a dd/mm/aaaa', async () => {
    const ui = cargarInterfaz({ campos: alta });
    await ui.appUI.handleAgregarSuscripcion(crearEvento());
    // (plataforma, monto, tarjeta, frecuencia, día, divisa, próximoCobro)
    llamoUnaVez(ui, 'crearSuscripcion', ['Servicio de Prueba', '9.99', 3, 'anual', 17, 'USD', '17/04/2026']);
    avisoExito(ui, /Suscripción recurrente guardada/);
    redibujo(ui, 'suscripciones');
});

test('alta de suscripción sin fecha de próximo cobro (o con formato inválido): no se envía', async () => {
    for (const ren of ['', '17/04/2026', 'abc']) {
        const ui = cargarInterfaz({ campos: { ...alta, sus_ren: ren } });
        await ui.appUI.handleAgregarSuscripcion(crearEvento());
        noLlamoANada(ui);
        unAvisoDeError(ui, /fecha del próximo cobro/);
    }
});

test('alta de suscripción: si Rust rechaza se muestra el error', async () => {
    const ui = cargarInterfaz({ campos: alta });
    ui.rechazar('crearSuscripcion', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleAgregarSuscripcion(crearEvento()));
});

// --- edición ---

const edicion = (extra = {}) => ({
    es_pla_8: ' Servicio Editado ', es_mon_8: ' 19.99 ', es_div_8: 'DOP', es_dia_8: '5', es_fre_8: 'mensual',
    es_tar_8: '4', es_ren_8: '2026-05-05', 'modal-edit-sus-8': {}, ...extra,
});

test('editar suscripción: (id, nombre, monto TEXTO, tarjeta, frecuencia, día, divisa, fecha dd/mm/aaaa); cierra el modal', async () => {
    const ui = cargarInterfaz({ campos: edicion() });
    await ui.appUI.handleEdicionSuscripcionSubmit(crearEvento(), 8);
    llamoUnaVez(ui, 'actualizarSuscripcion', [8, 'Servicio Editado', '19.99', 4, 'mensual', 5, 'DOP', '05/05/2026']);
    avisoExito(ui, /Suscripción actualizada/);
    assert.deepEqual(ui.eliminados, ['modal-edit-sus-8']);
    redibujo(ui, 'suscripciones');
});

test('editar suscripción: nombre vacío, monto no positivo o día fuera de 1-31 no se envían', async () => {
    const casos = [
        [{ es_pla_8: '   ' }, /nombre del servicio/],
        [{ es_mon_8: '0' }, /monto debe ser mayor/],
        [{ es_mon_8: '' }, /monto debe ser mayor/],
        [{ es_mon_8: '-1' }, /monto debe ser mayor/],
        [{ es_dia_8: '0' }, /entre 1 y 31/],
        [{ es_dia_8: '32' }, /entre 1 y 31/],
    ];
    for (const [cambio, aviso] of casos) {
        const ui = cargarInterfaz({ campos: edicion(cambio) });
        await ui.appUI.handleEdicionSuscripcionSubmit(crearEvento(), 8);
        noLlamoANada(ui);
        unAvisoDeError(ui, aviso);
        assert.deepEqual(ui.eliminados, []);
    }
});

test('editar suscripción sin fecha de próximo cobro: no se envía y el modal sigue abierto', async () => {
    const ui = cargarInterfaz({ campos: edicion({ es_ren_8: '' }) });
    await ui.appUI.handleEdicionSuscripcionSubmit(crearEvento(), 8);
    noLlamoANada(ui);
    unAvisoDeError(ui, /fecha del próximo cobro/);
    assert.deepEqual(ui.eliminados, []);
});

test('editar suscripción: si Rust rechaza se muestra el error y el modal sigue abierto', async () => {
    const ui = cargarInterfaz({ campos: edicion() });
    ui.rechazar('actualizarSuscripcion', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleEdicionSuscripcionSubmit(crearEvento(), 8));
    assert.deepEqual(ui.eliminados, []);
});

// --- baja ---

test('baja de suscripción: con confirmación envía el id y redibuja', async () => {
    const ui = cargarInterfaz({ confirm: true });
    await ui.appUI.handleEliminarSuscripcion(8);
    llamoUnaVez(ui, 'eliminarSuscripcion', [8]);
    avisoExito(ui, /Suscripción eliminada/);
    redibujo(ui, 'suscripciones');
});

test('baja de suscripción: sin confirmación no se envía nada', async () => {
    const ui = cargarInterfaz({ confirm: false });
    await ui.appUI.handleEliminarSuscripcion(8);
    noLlamoANada(ui);
    noRedibujo(ui);
});

// Hasta la 1.66.0 este manejador, a diferencia de sus hermanos, no tenía `try/catch`: si
// Rust rechazaba, la promesa del `onclick` rechazaba sin que nadie la atrapase y el
// usuario no veía ningún aviso.
test('baja de suscripción: si Rust rechaza se muestra el error y no se propaga', async () => {
    const ui = cargarInterfaz({ confirm: true });
    ui.rechazar('eliminarSuscripcion', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleEliminarSuscripcion(8));
});

// --- próximo cobro y períodos pendientes ---

test('corregir próximo cobro: la fecha dd/mm/aaaa (recortada) viaja con el id', async () => {
    const ui = cargarInterfaz({ prompt: ' 01/06/2026 ' });
    await ui.appUI.handleCorregirProximoCobro(8);
    llamoUnaVez(ui, 'corregirProximoCobro', [8, '01/06/2026']);
    avisoExito(ui, /Fecha puesta/);
    redibujo(ui, 'suscripciones');
});

test('corregir próximo cobro: cancelar no hace nada; un formato inválido se avisa y no se envía', async () => {
    let ui = cargarInterfaz({ prompt: null });
    await ui.appUI.handleCorregirProximoCobro(8);
    noLlamoANada(ui);
    assert.deepEqual(ui.avisos, []);

    for (const texto of ['', '2026-06-01', '1/6/2026', 'pronto']) {
        ui = cargarInterfaz({ prompt: texto });
        await ui.appUI.handleCorregirProximoCobro(8);
        noLlamoANada(ui);
        unAvisoDeError(ui, /dd\/mm\/aaaa/);
    }
});

test('corregir próximo cobro: si Rust rechaza se muestra el error', async () => {
    const ui = cargarInterfaz({ prompt: '01/06/2026' });
    ui.rechazar('corregirProximoCobro', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleCorregirProximoCobro(8));
});

test('asentar período pendiente: confirma y muestra lo que devuelve Rust', async () => {
    const ui = cargarInterfaz({ confirm: true, api: { asentarPeriodoPendiente: 'Cargo asentado (prueba).' } });
    await ui.appUI.handleAsentarPendiente(8, '01/03/2026');
    llamoUnaVez(ui, 'asentarPeriodoPendiente', [8]);
    avisoExito(ui, /Cargo asentado/);
    redibujo(ui, 'suscripciones');
    assert.match(ui.confirmaciones[0].texto, /01\/03\/2026/);
});

test('asentar período pendiente: sin confirmar no se envía; si Rust rechaza se muestra el error', async () => {
    let ui = cargarInterfaz({ confirm: false });
    await ui.appUI.handleAsentarPendiente(8, '01/03/2026');
    noLlamoANada(ui);

    ui = cargarInterfaz({ confirm: true });
    ui.rechazar('asentarPeriodoPendiente', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleAsentarPendiente(8, '01/03/2026'));
});

test('descartar período pendiente: envía (id, motivo) y muestra lo que devuelve Rust', async () => {
    const ui = cargarInterfaz({ prompt: MOTIVO, api: { descartarPeriodoPendiente: 'Período descartado (prueba).' } });
    await ui.appUI.handleDescartarPendiente(8, '01/03/2026');
    llamoUnaVez(ui, 'descartarPeriodoPendiente', [8, MOTIVO]);
    avisoExito(ui, /Período descartado/);
    redibujo(ui, 'suscripciones');
});

test('descartar período pendiente: motivo cancelado o corto no envía; si Rust rechaza se muestra el error', async () => {
    for (const prompt of [null, 'corto']) {
        const ui = cargarInterfaz({ prompt });
        await ui.appUI.handleDescartarPendiente(8, '01/03/2026');
        noLlamoANada(ui);
    }
    const ui = cargarInterfaz({ prompt: MOTIVO });
    ui.rechazar('descartarPeriodoPendiente', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleDescartarPendiente(8, '01/03/2026'));
});
