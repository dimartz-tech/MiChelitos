// Pruebas de la vista «Caja y Efectivo» en Node, sin navegador.
//
// Es lo que el diseño «B» de `division_de_ui_limpia.md` permite y el «A» no:
// la vista recibe sus dependencias, así que aquí se le pasan dobles (una API
// falsa que anota las llamadas, un aviso falso, un DOM mínimo de campos con
// `value`) y se comprueba **qué hace al pulsar**, que es justo lo que la
// comparación de vistas con datos reales no cubre. Los datos son inventados.
//
// Se importa el JavaScript compilado de la vista (`npm test` compila antes).

import test from 'node:test';
import assert from 'node:assert/strict';
import { VistaEfectivo, MANEJADORES_EFECTIVO, puenteEfectivo } from '../../../src/js/vistas/efectivo.js';

const CUENTAS = [
    { id: 1, nombre: 'Cuenta Alfa', divisa: 'DOP', balance_actual: 1000, entidad: null, comision_pago_impuestos: null },
    { id: 2, nombre: 'Cuenta Beta', divisa: 'USD', balance_actual: 50, entidad: null, comision_pago_impuestos: null },
    { id: 10, nombre: 'Efectivo DOP', divisa: 'DOP', balance_actual: 200, entidad: null, comision_pago_impuestos: null },
    { id: 11, nombre: 'Efectivo USD', divisa: 'USD', balance_actual: 5, entidad: null, comision_pago_impuestos: null },
];

/** Monta la vista con dobles. `campos` son los valores que «escribió» el titular. */
function montar({ campos = {}, cuentas = CUENTAS, fallaApi = null } = {}) {
    const llamadas = [];
    const avisos = [];
    const rutas = [];
    const api = {
        obtenerCuentas: async () => cuentas,
        crearCobroEfectivoInformal: async (...a) => {
            llamadas.push(['crearCobroEfectivoInformal', ...a]);
            if (fallaApi) throw new Error(fallaApi);
        },
        transferirEntreCuentas: async (...a) => {
            llamadas.push(['transferirEntreCuentas', ...a]);
            if (fallaApi) throw new Error(fallaApi);
        },
    };
    const pantalla = { contenido: { innerHTML: '' } };
    const vista = new VistaEfectivo({
        api,
        avisos: { mostrar: (mensaje, tipo) => avisos.push({ mensaje, tipo }) },
        formato: { importe: v => `#${v}#` },
        enrutador: { mostrar: async ruta => { rutas.push(ruta); } },
        pantalla,
        dom: {
            elemento: id => {
                if (!(id in campos)) throw new Error(`No existe el elemento #${id} en la página.`);
                return { value: campos[id] };
            },
        },
        ahora: () => new Date(2027, 2, 5),
    });
    let evitado = 0;
    const evento = { preventDefault: () => { evitado++; } };
    return { vista, llamadas, avisos, rutas, pantalla, evento, evitado: () => evitado };
}

test('entrada informal: envía los campos a la API, avisa y vuelve a dibujar la pestaña', async () => {
    const t = montar({ campos: { efe_inf_fec: '05/03/2027', efe_inf_mon: '250.5', efe_inf_div: 'USD', efe_inf_des: 'Cobro de prueba' } });
    await t.vista.handleAgregarEfectivoInformal(t.evento);
    assert.equal(t.evitado(), 1, 'debe cancelar el envío nativo del formulario');
    assert.deepEqual(t.llamadas, [['crearCobroEfectivoInformal', '05/03/2027', 'Cobro de prueba', '250.5', 'USD']]);
    assert.deepEqual(t.avisos, [{ mensaje: 'Entrada en efectivo registrada correctamente.', tipo: undefined }]);
    assert.deepEqual(t.rutas, ['efectivo']);
});

test('entrada informal: el importe viaja como TEXTO, con los espacios recortados y los dígitos intactos', async () => {
    for (const [escrito, esperado] of [['  250.5 ', '250.5'], ['1.005', '1.005'], ['0075.250', '0075.250'], ['', '']]) {
        const t = montar({ campos: { efe_inf_fec: '05/03/2027', efe_inf_mon: escrito, efe_inf_div: 'DOP', efe_inf_des: 'x' } });
        await t.vista.handleAgregarEfectivoInformal(t.evento);
        assert.strictEqual(t.llamadas[0][3], esperado, JSON.stringify(escrito));
        assert.equal(typeof t.llamadas[0][3], 'string');
    }
});

test('entrada informal: si la API falla, avisa del error y no vuelve a dibujar', async () => {
    const t = montar({ fallaApi: 'sin conexión', campos: { efe_inf_fec: '05/03/2027', efe_inf_mon: '10', efe_inf_div: 'DOP', efe_inf_des: 'x' } });
    await t.vista.handleAgregarEfectivoInformal(t.evento);
    assert.deepEqual(t.avisos, [{ mensaje: 'Error: sin conexión', tipo: 'error' }]);
    assert.deepEqual(t.rutas, []);
});

test('retiro: un cargo en blanco se envía como «0» (sin cargo), no como texto vacío', async () => {
    const t = montar({ campos: { efe_ret_fec: '05/03/2027', efe_ret_ori: '2', efe_ret_mon: '20', efe_ret_car: '', efe_ret_des_txt: 'Retiro de efectivo' } });
    await t.vista.handleRetirarAEfectivo(t.evento);
    assert.deepEqual(t.llamadas, [['transferirEntreCuentas', '05/03/2027', 2, 11, '20', '20', '0', 'Retiro de efectivo']]);
});

test('retiro: la cuenta en dólares va al efectivo en dólares, con monto y cargo como texto', async () => {
    const t = montar({ campos: { efe_ret_fec: '05/03/2027', efe_ret_ori: '2', efe_ret_mon: '20', efe_ret_car: '1.5', efe_ret_des_txt: 'Retiro de efectivo' } });
    await t.vista.handleRetirarAEfectivo(t.evento);
    assert.deepEqual(t.llamadas, [['transferirEntreCuentas', '05/03/2027', 2, 11, '20', '20', '1.5', 'Retiro de efectivo']]);
    assert.deepEqual(t.avisos, [{ mensaje: 'Retiro de efectivo ejecutado exitosamente.', tipo: undefined }]);
    assert.deepEqual(t.rutas, ['efectivo']);
});

test('retiro: la cuenta en pesos va al efectivo en pesos', async () => {
    const t = montar({ campos: { efe_ret_fec: '05/03/2027', efe_ret_ori: '1', efe_ret_mon: '100', efe_ret_car: '0', efe_ret_des_txt: 'Retiro' } });
    await t.vista.handleRetirarAEfectivo(t.evento);
    assert.equal(t.llamadas[0][3], 10, 'destino: Efectivo DOP');
});

test('retiro: una cuenta origen que no existe no llega a la API y se avisa', async () => {
    const t = montar({ campos: { efe_ret_fec: '05/03/2027', efe_ret_ori: '99', efe_ret_mon: '1', efe_ret_car: '0', efe_ret_des_txt: 'x' } });
    await t.vista.handleRetirarAEfectivo(t.evento);
    assert.deepEqual(t.llamadas, []);
    assert.deepEqual(t.avisos, [{ mensaje: 'Error: Cuenta origen no encontrada', tipo: 'error' }]);
    assert.deepEqual(t.rutas, []);
});

test('retiro: sin la cuenta de efectivo de destino se avisa diciendo cuál falta', async () => {
    const sinEfectivoUsd = CUENTAS.filter(c => c.nombre !== 'Efectivo USD');
    const t = montar({ cuentas: sinEfectivoUsd, campos: { efe_ret_fec: 'f', efe_ret_ori: '2', efe_ret_mon: '1', efe_ret_car: '0', efe_ret_des_txt: 'x' } });
    await t.vista.handleRetirarAEfectivo(t.evento);
    assert.deepEqual(t.llamadas, []);
    assert.match(t.avisos[0].mensaje, /Cuenta destino Efectivo USD no encontrada/);
});

test('dibujo: fecha de hoy inyectada, importes con el formato inyectado y sin las cuentas de efectivo en el origen', async () => {
    const t = montar();
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.match(html, /id="efe_inf_fec"[^>]*value="05\/03\/2027"/);
    assert.match(html, /DOP #200#/);
    assert.match(html, /USD #5#/);
    const origen = html.match(/<select id="efe_ret_ori"[\s\S]*?<\/select>/)[0];
    assert.match(origen, /Cuenta Alfa/);
    assert.match(origen, /Cuenta Beta/);
    assert.doesNotMatch(origen, /Efectivo/);
});

test('puente: todo appUI.x() que la plantilla escribe existe en lo que se expone, y delega en la vista', async () => {
    const t = montar({ campos: { efe_inf_fec: 'f', efe_inf_mon: '1', efe_inf_div: 'DOP', efe_inf_des: 'x' } });
    await t.vista.render();
    const usados = [...t.pantalla.contenido.innerHTML.matchAll(/\bappUI\.(\w+)\(/g)].map(m => m[1]);
    assert.ok(usados.length >= 2, 'la plantilla debe tener manejadores');
    const puente = puenteEfectivo(t.vista);
    assert.deepEqual([...MANEJADORES_EFECTIVO].sort(), Object.keys(puente).sort());
    for (const nombre of usados) assert.equal(typeof puente[nombre], 'function', `appUI.${nombre} quedaría colgado`);
    await puente.handleAgregarEfectivoInformal(t.evento);
    assert.equal(t.llamadas.length, 1, 'el puente debe llegar al manejador de la vista');
});
