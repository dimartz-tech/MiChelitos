// Pestaña Evolución: cambiar de serie y de mes, tomar la foto ahora (con y sin tasa), el error del
// comando, el mes sellado y el aviso antes de reemplazar la foto del mes. Datos inventados.

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
const FEBRERO = () => new Date(2027, 1, 14);
const MARZO = () => new Date(2027, 2, 14);

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
    const ui = cargarInterfaz({ campos: { evo_tasa: '' }, ahora: MARZO, api: { obtenerFotosPatrimonio: FOTOS, tomarFotoPatrimonio: foto(2027, 3, '2000.03', '900.09') } });
    await ui.appUI.handleTomarFotoEvolucion();
    llamoUnaVez(ui, 'tomarFotoPatrimonio', ['manual', undefined]);
    avisoExito(ui, /mar 2027.*2,000\.03/);
    redibujo(ui, 'evolucion');
});

test('tomar foto con tasa: acepta la coma decimal y manda el texto con punto', async () => {
    const ui = cargarInterfaz({ campos: { evo_tasa: ' 58,75 ' }, ahora: MARZO, api: { obtenerFotosPatrimonio: FOTOS, tomarFotoPatrimonio: foto(2027, 3, '2000.03', '900.09') } });
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
    const ui = cargarInterfaz({ campos: { evo_tasa: '' }, ahora: MARZO, api: { obtenerFotosPatrimonio: FOTOS } });
    ui.rechazar('tomarFotoPatrimonio', 'Error simulado de Rust');
    await ui.appUI.handleTomarFotoEvolucion();
    unAvisoDeError(ui, /No se pudo tomar la foto: Error simulado de Rust/);
    noRedibujo(ui);
});

test('mes sellado: el comando lo rechaza y la interfaz lo explica', async () => {
    // El mes en curso ya tiene foto y está sellada: no se pregunta (no hay nada que confirmar), decide Rust.
    const ui = cargarInterfaz({ campos: { evo_tasa: '' }, ahora: FEBRERO, api: { obtenerFotosPatrimonio: [FOTOS[0], foto(2027, 2, '1250.77', '640.31', { sellada: true })] } });
    ui.rechazar('tomarFotoPatrimonio', 'Una foto sellada del patrimonio no se modifica.');
    await ui.appUI.handleTomarFotoEvolucion();
    assert.equal(ui.confirmaciones.length, 0);
    unAvisoDeError(ui, /ya está sellado.*no se puede cambiar/);
    noRedibujo(ui);
});

// --- aviso antes de reemplazar la foto del mes ---

test('el mes en curso ya tiene foto: se avisa con su fecha, origen y patrimonio, y al aceptar se reemplaza', async () => {
    const ui = cargarInterfaz({ campos: { evo_tasa: '' }, ahora: FEBRERO, confirm: true, api: { obtenerFotosPatrimonio: FOTOS, tomarFotoPatrimonio: foto(2027, 2, '1300.19', '650.07', { origen: 'manual' }) } });
    await ui.appUI.handleTomarFotoEvolucion();
    assert.equal(ui.confirmaciones.length, 1);
    assert.match(ui.confirmaciones[0].texto, /Ya hay una foto de feb 2027: del 10\/02\/2027 \(Automática\), con patrimonio 1,250\.77 DOP/);
    assert.match(ui.confirmaciones[0].texto, /la reemplaza.*podrás restituir la anterior.*solo la última reemplazada.*¿Reemplazarla\?/);
    llamoUnaVez(ui, 'tomarFotoPatrimonio', ['manual', undefined]);
    avisoExito(ui, /feb 2027.*1,300\.19/);
    redibujo(ui, 'evolucion');
});

test('el mes en curso ya tiene foto y se cancela: no se toma, se dice cuál se conserva y no se redibuja', async () => {
    const ui = cargarInterfaz({ campos: { evo_tasa: '58,75' }, ahora: FEBRERO, confirm: false, api: { obtenerFotosPatrimonio: FOTOS } });
    await ui.appUI.handleTomarFotoEvolucion();
    assert.equal(ui.confirmaciones.length, 1);
    noLlamoA(ui, 'tomarFotoPatrimonio');
    avisoExito(ui, /No se tomó la foto: se conserva la del 10\/02\/2027/);
    noRedibujo(ui);
});

test('el mes en curso no tiene foto: no se pregunta aunque haya fotos de otros meses o del mismo mes de otro año', async () => {
    const ui = cargarInterfaz({ campos: { evo_tasa: '' }, ahora: MARZO, api: { obtenerFotosPatrimonio: [...FOTOS, foto(2026, 3, '900.41', '300.13')], tomarFotoPatrimonio: foto(2027, 3, '2000.03', '900.09') } });
    await ui.appUI.handleTomarFotoEvolucion();
    assert.equal(ui.confirmaciones.length, 0);
    llamoUnaVez(ui, 'tomarFotoPatrimonio', ['manual', undefined]);
});

test('si no se puede comprobar si el mes ya tiene foto, no se toma ninguna', async () => {
    const ui = cargarInterfaz({ campos: { evo_tasa: '' }, ahora: FEBRERO });
    ui.rechazar('obtenerFotosPatrimonio', 'Error simulado de lectura');
    await ui.appUI.handleTomarFotoEvolucion();
    noLlamoA(ui, 'tomarFotoPatrimonio');
    unAvisoDeError(ui, /No se tomó la foto: no se pudo comprobar.*Error simulado de lectura/);
    noRedibujo(ui);
});

test('una tasa mal escrita se rechaza antes de preguntar nada', async () => {
    const ui = cargarInterfaz({ campos: { evo_tasa: 'abc' }, ahora: FEBRERO, api: { obtenerFotosPatrimonio: FOTOS } });
    await ui.appUI.handleTomarFotoEvolucion();
    assert.equal(ui.confirmaciones.length, 0);
    noLlamoA(ui, 'obtenerFotosPatrimonio');
    noLlamoA(ui, 'tomarFotoPatrimonio');
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

const RECOMENDACIONES = {
    aviso: 'Reglas descriptivas, no asesoría financiera: muestran qué dicen tus propios datos.',
    anio_gastos: 2027, mes_gastos: 2, anio_presupuestos: 2027, mes_presupuestos: 3,
    recomendaciones: [{
        id: 'deuda_y_ahorro', regla: 'deuda_y_ahorro', severidad: 'info', titulo: 'Hay deuda en tarjetas y a la vez saldo en cuentas de ahorro',
        explicacion: 'No se estima el costo.', regla_texto: 'Deuda desde DOP 100.00.',
        cifras: [{ etiqueta: 'Deuda de tarjetas en pesos', tipo: 'importe', valor: '3210.45' }], destino: 'tarjetas',
    }],
    omitidas: 0, maximo_por_severidad: 5, reglas_evaluadas: 2, sin_datos: [], usd_excluido: [],
};

test('la pestaña con el enrutador real: pide las recomendaciones a la API real y las pinta con su enlace', async () => {
    const ui = cargarInterfaz({ renderReal: true, api: { obtenerFotosPatrimonio: FOTOS, obtenerRecomendaciones: RECOMENDACIONES } });
    await ui.enrutador.mostrar('evolucion');
    llamoUnaVez(ui, 'obtenerRecomendaciones', []);
    assert.match(html(ui), /Cómo aumentar tu patrimonio/);
    assert.match(html(ui), /DOP 3,210\.45/);
    assert.match(html(ui), /data-accion="navegar" data-args="\[&quot;tarjetas&quot;\]"/);
});

test('la pestaña con el enrutador real: si las recomendaciones fallan, el historial se ve igual', async () => {
    const ui = cargarInterfaz({ renderReal: true, api: { obtenerFotosPatrimonio: FOTOS } });
    ui.rechazar('obtenerRecomendaciones', 'base ocupada');
    await ui.enrutador.mostrar('evolucion');
    assert.match(html(ui), /id="evo-tabla"/);
    assert.match(html(ui), /No se pudieron calcular las recomendaciones: base ocupada/);
});

// --- deshacer el último reemplazo de la foto del mes ---

const PREVIA = { anio: 2027, mes: 2, fecha: '03/02/2027', origen: 'real', patrimonio: '1199.83' };

test('la pestaña ofrece restituir la foto anterior solo en un mes sin sellar que tenga previa', async () => {
    const ui = cargarInterfaz({ renderReal: true, api: { obtenerFotosPatrimonio: FOTOS, obtenerFotosPreviasPatrimonio: [PREVIA] } });
    await ui.enrutador.mostrar('evolucion');
    assert.match(html(ui), /Deshacer el último reemplazo/);
    assert.match(html(ui), /La foto de <strong>feb 2027<\/strong> reemplazó a la del 03\/02\/2027 \(Automática\), con patrimonio 1,199\.83 DOP/);
    assert.match(html(ui), /data-accion="handleDeshacerFotoEvolucion" data-args="\[2027,2\]"/);
    assert.doesNotMatch(html(ui), /onclick=/);
});

test('sin previas, con la previa de un mes sellado o si la lectura falla, no se ofrece deshacer y la pestaña se ve igual', async () => {
    const sellada = [FOTOS[0], { ...FOTOS[1], sellada: true }];
    for (const api of [
        { obtenerFotosPatrimonio: FOTOS, obtenerFotosPreviasPatrimonio: [] },
        { obtenerFotosPatrimonio: sellada, obtenerFotosPreviasPatrimonio: [PREVIA] },
        { obtenerFotosPatrimonio: FOTOS, obtenerFotosPreviasPatrimonio: [{ ...PREVIA, mes: 7 }] },
        { obtenerFotosPatrimonio: FOTOS, obtenerFotosPreviasPatrimonio: () => { throw new Error('Error simulado de lectura'); } },
    ]) {
        const ui = cargarInterfaz({ renderReal: true, api });
        await ui.enrutador.mostrar('evolucion');
        assert.doesNotMatch(html(ui), /Deshacer el último reemplazo|handleDeshacerFotoEvolucion/);
        assert.match(html(ui), /Tomar foto ahora/);
    }
});

test('el origen y la fecha de la previa se escapan', async () => {
    const ui = cargarInterfaz({ renderReal: true, api: { obtenerFotosPatrimonio: FOTOS, obtenerFotosPreviasPatrimonio: [{ ...PREVIA, origen: '<b>x</b>', fecha: '<i>y</i>' }] } });
    await ui.enrutador.mostrar('evolucion');
    assert.doesNotMatch(html(ui), /<b>x<\/b>|<i>y<\/i>/);
    assert.match(html(ui), /&lt;b&gt;x&lt;\/b&gt;/);
});

test('restituir: pide confirmar, envía año y mes, avisa con la foto que volvió y redibuja', async () => {
    const ui = cargarInterfaz({ confirm: true, api: { deshacerFotoPatrimonio: foto(2027, 2, '1199.83', '640.31', { fecha: '03/02/2027' }) } });
    await ui.appUI.handleDeshacerFotoEvolucion(2027, 2);
    assert.equal(ui.confirmaciones.length, 1);
    assert.match(ui.confirmaciones[0].texto, /¿Restituir la foto anterior de feb 2027\?.*La foto actual de ese mes se pierde/);
    llamoUnaVez(ui, 'deshacerFotoPatrimonio', [2027, 2]);
    avisoExito(ui, /Foto de feb 2027 restituida: la del 03\/02\/2027, con patrimonio 1,199\.83 DOP/);
    redibujo(ui, 'evolucion');
});

test('restituir cancelado: no se llama a Rust, no hay aviso y no se redibuja', async () => {
    const ui = cargarInterfaz({ confirm: false });
    await ui.appUI.handleDeshacerFotoEvolucion(2027, 2);
    noLlamoA(ui, 'deshacerFotoPatrimonio');
    assert.equal(ui.avisos.length, 0);
    noRedibujo(ui);
});

test('restituir rechazado por Rust: se muestra el motivo y no se redibuja', async () => {
    const ui = cargarInterfaz({ confirm: true });
    ui.rechazar('deshacerFotoPatrimonio', 'No hay una foto anterior que restituir para 02/2027.');
    await ui.appUI.handleDeshacerFotoEvolucion(2027, 2);
    unAvisoDeError(ui, /No se pudo restituir la foto anterior: No hay una foto anterior que restituir para 02\/2027\./);
    noRedibujo(ui);
});
