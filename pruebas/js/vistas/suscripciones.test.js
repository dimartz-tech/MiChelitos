// Pruebas de la vista «Suscripciones» en Node, sin navegador.
//
// Es la primera vista que abre una ventana modal, pide un motivo y usa los tres
// diálogos del navegador. Con la vista recibiendo sus dependencias se le dan
// dobles de todo y se comprueba qué dibuja y qué envía a Rust. Datos inventados.
//
// Se importa el JavaScript compilado de la vista (`npm test` compila antes).

import test from 'node:test';
import assert from 'node:assert/strict';
import { VistaSuscripciones, MANEJADORES_SUSCRIPCIONES, puenteSuscripciones } from '../../../src/js/vistas/suscripciones.js';

const TARJETAS = [{ id: 7, entidad: 'Banco Alfa', nombre_tarjeta: 'Oro' }];
const sus = (extra = {}) => ({
    id: 1, plataforma: 'Plataforma Uno', monto: 10, tarjeta_id: 7, frecuencia: 'mensual', dia_facturacion: 15,
    fecha_ultimo_pago: '15/02/2027', divisa: 'USD', entidad: 'Banco Alfa', nombre_tarjeta: 'Oro',
    fecha_proximo_cobro: '15/03/2027', pendientes: [], avisa: false, impedimento: null, ...extra,
});
const campo = value => ({ value, remove() { this.quitado = true; } });

function montar({ suscripciones = [sus()], tarjetas = TARJETAS, campos = {}, confirma = true, pregunta = '01/04/2027', motivo = 'Porque no se cobró', falla = null } = {}) {
    const llamadas = [], avisos = [], rutas = [], dialogos = [], motivos = [], modales = [];
    const registra = nombre => async (...a) => { llamadas.push([nombre, ...a]); if (falla) throw new Error(falla); return `respuesta de ${nombre}`; };
    const api = {
        obtenerSuscripciones: async () => suscripciones,
        obtenerTarjetas: async () => tarjetas,
        crearSuscripcion: registra('crearSuscripcion'),
        actualizarSuscripcion: registra('actualizarSuscripcion'),
        eliminarSuscripcion: registra('eliminarSuscripcion'),
        corregirProximoCobro: registra('corregirProximoCobro'),
        asentarPeriodoPendiente: registra('asentarPeriodoPendiente'),
        descartarPeriodoPendiente: registra('descartarPeriodoPendiente'),
    };
    const pantalla = { contenido: { innerHTML: '' } };
    const vista = new VistaSuscripciones({
        api,
        avisos: { mostrar: (mensaje, tipo) => avisos.push({ mensaje, tipo }) },
        formato: { importe: v => `#${v}#` },
        enrutador: { mostrar: async ruta => { rutas.push(ruta); } },
        pantalla,
        dom: {
            elemento: id => { if (!(id in campos)) throw new Error(`No existe el elemento #${id} en la página.`); return campos[id]; },
            buscar: id => campos[id] ?? null,
        },
        dialogos: { confirmar: m => { dialogos.push(m); return confirma; }, preguntar: (m, d) => { dialogos.push(m); return pregunta; } },
        motivo: { pedir: (a, b) => { motivos.push([a, b]); return motivo; } },
        modales: { abrir: (id, html) => modales.push({ id, html }) },
    });
    return { vista, llamadas, avisos, rutas, dialogos, motivos, modales, pantalla, evento: { preventDefault() { this.evitado = (this.evitado || 0) + 1; } } };
}

const CAMPOS_ALTA = () => ({ sus_pla: campo('Servicio Nuevo'), sus_mon: campo(' 13.99 '), sus_div: campo('USD'), sus_dia: campo('29'), sus_fre: campo('anual'), sus_tar: campo('7'), sus_ren: campo('2027-04-29') });
const CAMPOS_EDICION = (extra = {}) => ({ es_pla_1: campo(' Plataforma Editada '), es_mon_1: campo('20.5'), es_div_1: campo('DOP'), es_dia_1: campo('10'), es_fre_1: campo('mensual'), es_tar_1: campo('7'), es_ren_1: campo('2027-05-10'), 'modal-edit-sus-1': campo(''), ...extra });

test('dibuja la tabla con el formato inyectado, el día de facturación y la fecha del próximo cobro', async () => {
    const t = montar();
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.match(html, /Plataforma Uno/);
    assert.match(html, /USD #10#/);
    assert.match(html, /Día 15/);
    assert.match(html, /15\/03\/2027/);
    assert.doesNotMatch(html, /undefined|NaN/);
});

test('el aviso «Cobro próximo» muestra la fecha del próximo cobro (la 1.43.0 imprimía «undefined»)', async () => {
    const t = montar({ suscripciones: [sus({ avisa: true })] });
    await t.vista.render();
    assert.match(t.pantalla.contenido.innerHTML, /Cobro próximo[\s\S]*Plataforma Uno[\s\S]*USD #10# el 15\/03\/2027/);
    assert.doesNotMatch(t.pantalla.contenido.innerHTML, /el undefined/);
});

test('con períodos pendientes ofrece asentar o descartar el más antiguo y dice cuántos más hay', async () => {
    const t = montar({ suscripciones: [sus({ pendientes: ['15/01/2027', '15/02/2027'] })] });
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.match(html, /el más antiguo: <strong>15\/01\/2027<\/strong>/);
    assert.match(html, /\(y 1 más\)/);
    assert.match(html, /appUI\.handleAsentarPendiente\(1, '15\/01\/2027'\)/);
    assert.match(html, /appUI\.handleDescartarPendiente\(1, '15\/01\/2027'\)/);
});

test('una suscripción parada se anuncia con su impedimento y ofrece corregir la fecha', async () => {
    const t = montar({ suscripciones: [sus({ impedimento: 'No tiene fecha de próximo cobro', fecha_proximo_cobro: null })] });
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.match(html, /No se cobrarán/);
    assert.match(html, /No tiene fecha de próximo cobro/);
    assert.match(html, /Sin fecha: no se cobrará/);
    assert.match(html, /appUI\.handleCorregirProximoCobro\(1\)/);
});

test('sin suscripciones dibuja el mensaje vacío', async () => {
    const t = montar({ suscripciones: [] });
    await t.vista.render();
    assert.match(t.pantalla.contenido.innerHTML, /No hay suscripciones registradas/);
});

test('alta: el monto viaja como TEXTO recortado, día y tarjeta como números, y la fecha pasa de ISO a dd/mm/aaaa', async () => {
    const t = montar({ campos: CAMPOS_ALTA() });
    await t.vista.handleAgregarSuscripcion(t.evento);
    assert.equal(t.evento.evitado, 1);
    assert.deepEqual(t.llamadas, [['crearSuscripcion', 'Servicio Nuevo', '13.99', 7, 'anual', 29, 'USD', '29/04/2027']]);
    assert.equal(typeof t.llamadas[0][2], 'string', 'sin convertir: los dígitos decide Rust');
    assert.deepEqual(t.avisos, [{ mensaje: 'Suscripción recurrente guardada.', tipo: undefined }]);
    assert.deepEqual(t.rutas, ['suscripciones']);
});

test('alta sin fecha del próximo cobro: avisa y no envía nada', async () => {
    const t = montar({ campos: { ...CAMPOS_ALTA(), sus_ren: campo('') } });
    await t.vista.handleAgregarSuscripcion(t.evento);
    assert.deepEqual(t.llamadas, []);
    assert.deepEqual(t.avisos, [{ mensaje: 'Indica la fecha del próximo cobro.', tipo: 'error' }]);
    assert.deepEqual(t.rutas, []);
});

test('alta: si Rust rechaza, avisa del error y no redibuja', async () => {
    const t = montar({ campos: CAMPOS_ALTA(), falla: 'el importe no puede ser cero' });
    await t.vista.handleAgregarSuscripcion(t.evento);
    assert.deepEqual(t.avisos, [{ mensaje: 'Error: el importe no puede ser cero', tipo: 'error' }]);
    assert.deepEqual(t.rutas, []);
});

test('abrir la edición abre una ventana modal con su identificador, la tarjeta elegida y la fecha en ISO', async () => {
    const t = montar();
    await t.vista.abrirEdicionSuscripcion(sus());
    assert.equal(t.modales.length, 1);
    assert.equal(t.modales[0].id, 'modal-edit-sus-1');
    assert.match(t.modales[0].html, /Editar Suscripción/);
    assert.match(t.modales[0].html, /value="2027-03-15"/, 'la fecha guardada como dd/mm/aaaa vuelve a ISO para el campo de fecha');
    assert.match(t.modales[0].html, /<option value="7" selected>Banco Alfa - Oro/);
    assert.match(t.modales[0].html, /appUI\.handleEdicionSuscripcionSubmit\(event, 1\)/);
});

test('edición: envía todo con el monto como TEXTO, cierra la ventana y redibuja', async () => {
    const campos = CAMPOS_EDICION();
    const t = montar({ campos });
    await t.vista.handleEdicionSuscripcionSubmit(t.evento, 1);
    assert.deepEqual(t.llamadas, [['actualizarSuscripcion', 1, 'Plataforma Editada', '20.5', 7, 'mensual', 10, 'DOP', '10/05/2027']]);
    assert.equal(campos['modal-edit-sus-1'].quitado, true, 'la ventana se cierra');
    assert.deepEqual(t.avisos, [{ mensaje: 'Suscripción actualizada. Los cargos ya realizados no se alteran.', tipo: undefined }]);
    assert.deepEqual(t.rutas, ['suscripciones']);
});

test('edición: nombre vacío, monto no positivo o día fuera de 1-31 no se envían', async () => {
    for (const [cambio, mensaje] of [
        [{ es_pla_1: campo('   ') }, 'El nombre del servicio no puede estar vacío.'],
        [{ es_mon_1: campo('0') }, 'El monto debe ser mayor que cero.'],
        [{ es_dia_1: campo('32') }, 'El día de facturación debe estar entre 1 y 31.'],
    ]) {
        const t = montar({ campos: CAMPOS_EDICION(cambio) });
        await t.vista.handleEdicionSuscripcionSubmit(t.evento, 1);
        assert.deepEqual(t.llamadas, [], mensaje);
        assert.deepEqual(t.avisos, [{ mensaje, tipo: 'error' }]);
    }
});

test('edición sin fecha del próximo cobro: avisa y no envía', async () => {
    const t = montar({ campos: CAMPOS_EDICION({ es_ren_1: campo('') }) });
    await t.vista.handleEdicionSuscripcionSubmit(t.evento, 1);
    assert.deepEqual(t.llamadas, []);
    assert.deepEqual(t.avisos, [{ mensaje: 'Indica la fecha del próximo cobro.', tipo: 'error' }]);
});

test('baja: pide confirmación y envía el id; sin confirmar no hace nada', async () => {
    const si = montar();
    await si.vista.handleEliminarSuscripcion(4);
    assert.deepEqual(si.llamadas, [['eliminarSuscripcion', 4]]);
    assert.deepEqual(si.rutas, ['suscripciones']);
    const no = montar({ confirma: false });
    await no.vista.handleEliminarSuscripcion(4);
    assert.deepEqual(no.llamadas, []);
    assert.deepEqual(no.rutas, []);
});

test('corregir el próximo cobro: la fecha recortada viaja con el id; cancelar o un formato inválido no envían', async () => {
    const bien = montar({ pregunta: ' 01/04/2027 ' });
    await bien.vista.handleCorregirProximoCobro(2);
    assert.deepEqual(bien.llamadas, [['corregirProximoCobro', 2, '01/04/2027']]);
    assert.deepEqual(bien.avisos, [{ mensaje: 'Fecha puesta. La suscripción vuelve a su ciclo.', tipo: undefined }]);

    const cancela = montar({ pregunta: null });
    await cancela.vista.handleCorregirProximoCobro(2);
    assert.deepEqual(cancela.llamadas, []);
    assert.deepEqual(cancela.avisos, []);

    const mala = montar({ pregunta: '1 de abril' });
    await mala.vista.handleCorregirProximoCobro(2);
    assert.deepEqual(mala.llamadas, []);
    assert.deepEqual(mala.avisos, [{ mensaje: 'La fecha debe escribirse como dd/mm/aaaa.', tipo: 'error' }]);
});

test('asentar un período: confirma nombrando la fecha y muestra lo que devuelve Rust', async () => {
    const t = montar();
    await t.vista.handleAsentarPendiente(3, '15/01/2027');
    assert.match(t.dialogos[0], /Se asentará el cargo con fecha 15\/01\/2027/);
    assert.deepEqual(t.llamadas, [['asentarPeriodoPendiente', 3]]);
    assert.deepEqual(t.avisos, [{ mensaje: 'respuesta de asentarPeriodoPendiente', tipo: undefined }]);
    const no = montar({ confirma: false });
    await no.vista.handleAsentarPendiente(3, '15/01/2027');
    assert.deepEqual(no.llamadas, []);
});

test('descartar un período: pide el motivo con la fecha y lo envía junto al id; sin motivo no envía', async () => {
    const t = montar({ motivo: 'El estado de cuenta no lo muestra' });
    await t.vista.handleDescartarPendiente(3, '15/01/2027');
    assert.match(t.motivos[0][0], /Dar por no cobrado el período del 15\/01\/2027/);
    assert.deepEqual(t.llamadas, [['descartarPeriodoPendiente', 3, 'El estado de cuenta no lo muestra']]);
    const no = montar({ motivo: null });
    await no.vista.handleDescartarPendiente(3, '15/01/2027');
    assert.deepEqual(no.llamadas, []);
    assert.deepEqual(no.avisos, []);
});

test('el puente expone todos los manejadores y delega en la vista', async () => {
    const t = montar({ campos: CAMPOS_ALTA() });
    const puente = puenteSuscripciones(t.vista);
    assert.deepEqual(Object.keys(puente).sort(), [...MANEJADORES_SUSCRIPCIONES].sort());
    await puente.handleAgregarSuscripcion(t.evento);
    assert.equal(t.llamadas.length, 1);
});

test('todo appUI.x( que escribe la plantilla (y la ventana de edición) está en el puente', async () => {
    const t = montar({ suscripciones: [sus({ pendientes: ['15/01/2027'], impedimento: 'x' })] });
    await t.vista.render();
    await t.vista.abrirEdicionSuscripcion(sus());
    const html = t.pantalla.contenido.innerHTML + t.modales[0].html;
    const llamados = new Set([...html.matchAll(/appUI\.(\w+)\(/g)].map(m => m[1]));
    assert.ok(llamados.size >= 6);
    for (const nombre of llamados) assert.ok(MANEJADORES_SUSCRIPCIONES.includes(nombre), `falta ${nombre} en el puente`);
});
