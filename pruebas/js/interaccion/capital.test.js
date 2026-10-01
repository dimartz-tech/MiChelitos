// Pestaña Capital: certificados, bolsa y propiedades. Estos manejadores no
// llaman a un comando por operación: leen el capital entero, lo modifican y lo
// guardan con `guardarCapital`, así que lo que se comprueba es el objeto
// guardado. Datos inventados.

import test from 'node:test';
import assert from 'node:assert/strict';
import { cargarInterfaz, crearEvento } from '../ayudas/cargar_interfaz.js';
import {
    llamoUnaVez, noLlamoA, noLlamoANada, avisoExito, redibujo, noRedibujo, rechazoSeMuestra,
} from '../ayudas/afirmar.js';

/** Capital de partida; cada lectura devuelve una copia, como haría Rust. */
const capitalInicial = () => ({
    propiedades: {
        inmobiliario: [{ id: 'p1', subtipo: 'Casa', nombre: 'Propiedad uno', valor_estimado: '111.11' },
                       { id: 'p2', subtipo: 'Solar', nombre: 'Propiedad dos', valor_estimado: '222.22' }],
        vehiculos: [], maquinaria: [],
    },
    certificados: [{ banco: 'Banco A', monto: '1357.90', tasa: 5, vencimiento: '01/01/2027', tipo_pago: 'mensual' },
                   { banco: 'Banco B', monto: '2468.10', tasa: 6, vencimiento: '01/02/2027', tipo_pago: 'vencimiento' }],
    bolsa: [{ emisor: 'Emisor A', monto: '300.00', tasa: 7, vencimiento: '01/03/2027', tipo_pago: 'mensual' },
            { emisor: 'Emisor B', monto: '400.00', tasa: 8, vencimiento: '01/04/2027', tipo_pago: 'mensual' }],
});
const api = (extra = {}) => ({ obtenerCapital: () => capitalInicial(), ...extra });
const guardado = ui => ui.llamadasA('guardarCapital')[0].args[0];

// --- certificados ---

const cert = { cer_ban: 'Banco C', cer_mon: ' 3333.33 ', cer_tas: '9.5', cer_ven: '01/05/2027', cer_pag: 'mensual' };

test('agregar certificado: se añade al final del capital; el monto se guarda como TEXTO y la tasa como número', async () => {
    const ui = cargarInterfaz({ campos: cert, api: api() });
    await ui.appUI.handleAgregarCertificado(crearEvento());
    llamoUnaVez(ui, 'guardarCapital', [{
        ...capitalInicial(),
        certificados: [...capitalInicial().certificados,
            { banco: 'Banco C', monto: '3333.33', tasa: 9.5, vencimiento: '01/05/2027', tipo_pago: 'mensual' }],
    }]);
    avisoExito(ui, /Certificado guardado/);
    redibujo(ui, 'capital');
});

test('agregar certificado sin lista previa en el capital: la crea', async () => {
    const ui = cargarInterfaz({ campos: cert, api: { obtenerCapital: () => ({}) } });
    await ui.appUI.handleAgregarCertificado(crearEvento());
    assert.equal(guardado(ui).certificados.length, 1);
});

test('agregar certificado: si no se puede leer el capital no se guarda nada; si guardar falla, error', async () => {
    let ui = cargarInterfaz({ campos: cert });
    ui.rechazar('obtenerCapital', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleAgregarCertificado(crearEvento()));
    noLlamoA(ui, 'guardarCapital');

    ui = cargarInterfaz({ campos: cert, api: api() });
    ui.rechazar('guardarCapital', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleAgregarCertificado(crearEvento()));
});

test('retirar certificado: quita exactamente el de ese índice', async () => {
    const ui = cargarInterfaz({ confirm: true, api: api() });
    await ui.appUI.handleEliminarCertificado(0);
    assert.deepEqual(guardado(ui).certificados.map(c => c.banco), ['Banco B']);
    assert.deepEqual(guardado(ui).bolsa, capitalInicial().bolsa, 'lo demás queda igual');
    avisoExito(ui, /Certificado retirado/);
    redibujo(ui, 'capital');
});

test('retirar certificado: sin confirmación no se guarda nada', async () => {
    const ui = cargarInterfaz({ confirm: false, api: api() });
    await ui.appUI.handleEliminarCertificado(0);
    noLlamoANada(ui);
    noRedibujo(ui);
});

// --- bolsa ---

const bolsa = { bol_emi: 'Emisor C', bol_mon: ' 555.55 ', bol_tas: '4.25', bol_ven: '01/06/2027', bol_pag: 'vencimiento' };

test('agregar inversión de bolsa: monto TEXTO, tasa número, añadida a «bolsa» (no a certificados)', async () => {
    const ui = cargarInterfaz({ campos: bolsa, api: api() });
    await ui.appUI.handleAgregarBolsa(crearEvento());
    llamoUnaVez(ui, 'guardarCapital', [{
        ...capitalInicial(),
        bolsa: [...capitalInicial().bolsa,
            { emisor: 'Emisor C', monto: '555.55', tasa: 4.25, vencimiento: '01/06/2027', tipo_pago: 'vencimiento' }],
    }]);
    avisoExito(ui, /Inversión de bolsa guardada/);
    redibujo(ui, 'capital');
});

test('agregar inversión de bolsa: si Rust rechaza se muestra el error', async () => {
    const ui = cargarInterfaz({ campos: bolsa, api: api() });
    ui.rechazar('guardarCapital', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleAgregarBolsa(crearEvento()));
});

test('liquidar inversión de bolsa: quita exactamente la de ese índice', async () => {
    const ui = cargarInterfaz({ confirm: true, api: api() });
    await ui.appUI.handleEliminarBolsa(1);
    assert.deepEqual(guardado(ui).bolsa.map(b => b.emisor), ['Emisor A']);
    assert.deepEqual(guardado(ui).certificados, capitalInicial().certificados);
    avisoExito(ui, /Inversión liquidada/);
    redibujo(ui, 'capital');
});

test('liquidar inversión de bolsa: sin confirmación no se guarda nada', async () => {
    const ui = cargarInterfaz({ confirm: false, api: api() });
    await ui.appUI.handleEliminarBolsa(1);
    noLlamoANada(ui);
});

// --- propiedades ---

const propiedad = { pro_tip: 'vehiculos', pro_sub: 'Automóvil', pro_nom: 'Vehículo de prueba', pro_val: ' 4444.44 ' };

test('agregar propiedad: va al grupo elegido, con el valor como TEXTO y un id de texto', async () => {
    const ui = cargarInterfaz({ campos: propiedad, api: api() });
    await ui.appUI.handleAgregarPropiedad(crearEvento());
    const g = guardado(ui);
    assert.equal(g.propiedades.vehiculos.length, 1);
    const { id, ...resto } = g.propiedades.vehiculos[0];
    assert.match(id, /^\d+$/, 'el id es una marca de tiempo en texto');
    assert.deepEqual(resto, { subtipo: 'Automóvil', nombre: 'Vehículo de prueba', valor_estimado: '4444.44' });
    assert.deepEqual(g.propiedades.inmobiliario, capitalInicial().propiedades.inmobiliario);
    avisoExito(ui, /Propiedad registrada/);
    redibujo(ui, 'capital');
});

test('agregar propiedad: si Rust rechaza se muestra el error', async () => {
    const ui = cargarInterfaz({ campos: propiedad, api: api() });
    ui.rechazar('guardarCapital', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleAgregarPropiedad(crearEvento()));
});

test('eliminar propiedad: quita solo la de ese id dentro de ese grupo', async () => {
    const ui = cargarInterfaz({ confirm: true, api: api() });
    await ui.appUI.handleEliminarPropiedad('inmobiliario', 'p2');
    assert.deepEqual(guardado(ui).propiedades.inmobiliario.map(p => p.id), ['p1']);
    avisoExito(ui, /Bien eliminado/);
    redibujo(ui, 'capital');
});

test('eliminar propiedad: sin confirmación no se guarda nada', async () => {
    const ui = cargarInterfaz({ confirm: false, api: api() });
    await ui.appUI.handleEliminarPropiedad('inmobiliario', 'p2');
    noLlamoANada(ui);
});

// Las tres bajas del capital (certificado, bolsa y bien) leen el documento entero, lo
// modifican y lo guardan. Hasta la 1.66.0 no tenían `try/catch`: si `obtenerCapital`
// o `guardarCapital` rechazaban, el error se perdía sin aviso. Ahora lo muestran y no
// anuncian éxito ni redibujan.
for (const [nombre, llamar] of [
    ['handleEliminarCertificado', ui => ui.appUI.handleEliminarCertificado(0)],
    ['handleEliminarBolsa', ui => ui.appUI.handleEliminarBolsa(0)],
    ['handleEliminarPropiedad', ui => ui.appUI.handleEliminarPropiedad('inmobiliario', 'p1')],
]) {
    for (const metodo of ['guardarCapital', 'obtenerCapital']) {
        test(`${nombre}: si Rust rechaza al ${metodo === 'guardarCapital' ? 'guardar' : 'leer'} se muestra el error y no se propaga`, async () => {
            const ui = cargarInterfaz({ confirm: true, api: api() });
            ui.rechazar(metodo, 'Error simulado de Rust');
            await rechazoSeMuestra(ui, () => llamar(ui));
        });
    }
}
