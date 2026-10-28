// Pestaña Evolución: cambiar de serie y de mes, tomar la foto ahora (con y sin tasa), el error del
// comando y el mes sellado. Datos inventados.

import test from 'node:test';
import assert from 'node:assert/strict';
import { cargarInterfaz } from '../ayudas/cargar_interfaz.js';
import { llamoUnaVez, noLlamoA, avisoExito, unAvisoDeError, redibujo, noRedibujo } from '../ayudas/afirmar.js';

const foto = (anio, mes, patrimonio, liquido, extra = {}) => ({
    id: anio * 100 + mes, anio, mes, fecha: `10/${String(mes).padStart(2, '0')}/${anio}`, origen: 'real', sellada: false,
    tasa_usd_dop: '60.00', activos: patrimonio, pasivos: '0.00', patrimonio, liquido, calidad: 'completa',
    partidas: [{ rubro: 'efectivo', referencia: `Caja ${mes}`, divisa: 'DOP', importe: liquido, importe_dop: liquido }], ...extra,
});

const FOTOS = [foto(2027, 1, '1000.01', '500.05'), foto(2027, 2, '1250.77', '640.31')];
const html = ui => ui.elemento('app-content').innerHTML;

test('cambiar la serie vuelve a pedir las fotos y dibuja el líquido', async () => {
    const ui = cargarInterfaz({ api: { obtenerFotosPatrimonio: FOTOS } });
    await ui.appUI.handleCambiarSerieEvolucion('liquido');
    assert.equal(ui.llamadasA('obtenerFotosPatrimonio').length, 1);
    assert.match(html(ui), /Patrimonio líquido de 500\.05 en ene 2027 a 640\.31 en feb 2027/);
    await ui.appUI.handleCambiarSerieEvolucion('total');
    assert.match(html(ui), /Patrimonio total de 1,000\.01 en ene 2027 a 1,250\.77 en feb 2027/);
    noLlamoA(ui, 'tomarFotoPatrimonio');
});

test('cambiar el mes muestra las partidas de esa foto', async () => {
    const ui = cargarInterfaz({ api: { obtenerFotosPatrimonio: FOTOS } });
    await ui.appUI.handleCambiarMesEvolucion('2027-01');
    assert.match(html(ui), /Caja 1/);
    assert.doesNotMatch(html(ui), /Caja 2/);
    await ui.appUI.handleCambiarMesEvolucion('2027-02');
    assert.match(html(ui), /Caja 2/);
});

test('tomar foto sin tasa: origen manual, sin tasa, aviso de éxito y se redibuja la pestaña', async () => {
    const ui = cargarInterfaz({ campos: { evo_tasa: '' }, api: { tomarFotoPatrimonio: foto(2027, 3, '2000.03', '900.09') } });
    await ui.appUI.handleTomarFotoEvolucion();
    llamoUnaVez(ui, 'tomarFotoPatrimonio', ['manual', undefined]);
    avisoExito(ui, /mar 2027.*2,000\.03/);
    redibujo(ui, 'evolucion');
});

test('tomar foto con tasa: acepta la coma decimal y manda el texto con punto', async () => {
    const ui = cargarInterfaz({ campos: { evo_tasa: ' 58,75 ' }, api: { tomarFotoPatrimonio: foto(2027, 3, '2000.03', '900.09') } });
    await ui.appUI.handleTomarFotoEvolucion();
    llamoUnaVez(ui, 'tomarFotoPatrimonio', ['manual', '58.75']);
    avisoExito(ui);
});

test('tomar foto con una tasa que no es un número: no se envía y se explica', async () => {
    for (const mala of ['abc', '-5', '1.2.3', '60,00,1']) {
        const ui = cargarInterfaz({ campos: { evo_tasa: mala } });
        await ui.appUI.handleTomarFotoEvolucion();
        noLlamoA(ui, 'tomarFotoPatrimonio');
        unAvisoDeError(ui, /tasa/);
        noRedibujo(ui);
    }
});

test('si el comando falla se muestra el error y no se redibuja', async () => {
    const ui = cargarInterfaz({ campos: { evo_tasa: '' } });
    ui.rechazar('tomarFotoPatrimonio', 'Error simulado de Rust');
    await ui.appUI.handleTomarFotoEvolucion();
    unAvisoDeError(ui, /No se pudo tomar la foto: Error simulado de Rust/);
    noRedibujo(ui);
});

test('mes sellado: el comando lo rechaza y la interfaz lo explica', async () => {
    const ui = cargarInterfaz({ campos: { evo_tasa: '' } });
    ui.rechazar('tomarFotoPatrimonio', 'Una foto sellada del patrimonio no se modifica.');
    await ui.appUI.handleTomarFotoEvolucion();
    unAvisoDeError(ui, /ya está sellado.*no se puede cambiar/);
    noRedibujo(ui);
});

test('la pestaña con el enrutador real: error del comando de lectura pintado en pantalla', async () => {
    const ui = cargarInterfaz({ renderReal: true });
    ui.rechazar('obtenerFotosPatrimonio', 'base ilegible');
    await ui.enrutador.mostrar('evolucion');
    assert.match(html(ui), /No se pudo leer el historial del patrimonio/);
    assert.match(html(ui), /base ilegible/);
});

test('la pestaña con el enrutador real: sin fotos', async () => {
    const ui = cargarInterfaz({ renderReal: true, api: { obtenerFotosPatrimonio: [] } });
    await ui.enrutador.mostrar('evolucion');
    assert.match(html(ui), /Aún no hay fotos/);
});
