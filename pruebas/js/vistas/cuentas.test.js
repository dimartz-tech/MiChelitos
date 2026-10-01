// Pruebas de la vista «Cuentas de Ahorro» en Node, sin navegador.
//
// La vista recibe sus dependencias, así que aquí se le pasan dobles (API que
// anota llamadas, avisos, DOM mínimo con `value`/`textContent`, reloj fijo) y se
// comprueba qué dibuja y qué hace al pulsar. Los datos son inventados.
//
// Se importa el JavaScript compilado de la vista (`npm test` compila antes).

import test from 'node:test';
import assert from 'node:assert/strict';
import { VistaCuentas, MANEJADORES_CUENTAS, puenteCuentas } from '../../../src/js/vistas/cuentas.js';

const CUENTAS = [
    { id: 1, nombre: 'Cuenta Alfa', divisa: 'DOP', balance_actual: 1000, entidad: null, comision_pago_impuestos: null },
    { id: 2, nombre: 'Cuenta Beta', divisa: 'USD', balance_actual: 50, entidad: null, comision_pago_impuestos: null },
];
const TRANSACCION = {
    id: 9, fecha: '05/03/2027', descripcion: 'Cambio de prueba', cuenta_origen_nombre: 'Cuenta Alfa',
    cuenta_destino_nombre: 'Cuenta Beta', monto_origen: 600, monto_destino: 10, cargo: 1.5, tasa_cambio: 60,
};

/** Un elemento mínimo del DOM: lo que la vista lee y escribe. */
function elementoFalso(spec = {}) {
    const el = { value: '', textContent: '', innerHTML: '', style: {}, ...spec };
    if (spec.divisa !== undefined) el.selectedOptions = [{ dataset: { divisa: spec.divisa } }];
    return el;
}

/** Monta la vista con dobles. `campos` son los elementos del DOM por id. */
function montar({ campos = {}, cuentas = CUENTAS, transacciones = [], fallaApi = null } = {}) {
    const llamadas = [];
    const avisos = [];
    const rutas = [];
    const api = {
        obtenerCuentas: async () => cuentas,
        obtenerTransaccionesCuentas: async () => transacciones,
        transferirEntreCuentas: async (...a) => {
            llamadas.push(['transferirEntreCuentas', ...a]);
            if (fallaApi) throw new Error(fallaApi);
        },
    };
    const pantalla = { contenido: { innerHTML: '' } };
    const dom = {
        elemento: id => {
            if (!(id in campos)) throw new Error(`No existe el elemento #${id} en la página.`);
            return campos[id];
        },
        buscar: id => campos[id] ?? null,
    };
    const vista = new VistaCuentas({
        api,
        avisos: { mostrar: (mensaje, tipo) => avisos.push({ mensaje, tipo }) },
        formato: { importe: v => `#${v}#` },
        enrutador: { mostrar: async ruta => { rutas.push(ruta); } },
        pantalla,
        dom,
        ahora: () => new Date(2027, 2, 5),
    });
    let evitado = 0;
    const evento = { preventDefault: () => { evitado++; } };
    return { vista, llamadas, avisos, rutas, pantalla, evento, evitado: () => evitado };
}

const CAMPOS_TRANSFERENCIA = {
    tra_fec: elementoFalso({ value: '05/03/2027' }), tra_ori: elementoFalso({ value: '2' }),
    tra_des: elementoFalso({ value: '1' }), tra_mon_ori: elementoFalso({ value: '10.5' }),
    tra_mon_des: elementoFalso({ value: '630' }), tra_car: elementoFalso({ value: '1.5' }),
    tra_des_txt: elementoFalso({ value: 'Cambio de prueba' }),
};

test('dibuja el título, las cuentas con el formato inyectado y la fecha de hoy del reloj inyectado', async () => {
    const t = montar({ transacciones: [TRANSACCION] });
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.match(html, /Cuentas de Ahorro/);
    assert.match(html, /Cuenta Alfa/);
    assert.match(html, /Cuenta Beta/);
    assert.match(html, /DOP #1000#/, 'el importe pasa por el servicio de formato');
    assert.match(html, /value="05\/03\/2027"/, 'la fecha por defecto sale del reloj inyectado');
    assert.match(html, /Cambio de prueba/, 'el historial lista la transferencia');
});

test('sin transferencias dibuja el mensaje vacío', async () => {
    const t = montar({ transacciones: [] });
    await t.vista.render();
    assert.match(t.pantalla.contenido.innerHTML, /No hay transferencias entre cuentas registradas/);
});

test('transferir: envía fecha, ids y los tres importes como números, avisa y vuelve a dibujar', async () => {
    const t = montar({ campos: CAMPOS_TRANSFERENCIA });
    await t.vista.handleTransferirCuentas(t.evento);
    assert.equal(t.evitado(), 1, 'debe cancelar el envío nativo del formulario');
    assert.deepEqual(t.llamadas, [['transferirEntreCuentas', '05/03/2027', 2, 1, 10.5, 630, 1.5, 'Cambio de prueba']]);
    assert.deepEqual(t.avisos, [{ mensaje: 'Transacción ejecutada con éxito.', tipo: undefined }]);
    assert.deepEqual(t.rutas, ['cuentas']);
});

test('transferir: si la API falla avisa del error y no vuelve a dibujar', async () => {
    const t = montar({ campos: CAMPOS_TRANSFERENCIA, fallaApi: 'saldo insuficiente' });
    await t.vista.handleTransferirCuentas(t.evento);
    assert.deepEqual(t.avisos, [{ mensaje: 'Error: saldo insuficiente', tipo: 'error' }]);
    assert.deepEqual(t.rutas, []);
});

test('rotular: si las divisas difieren pone «en USD/DOP» y muestra el aviso de cruce', () => {
    const campos = {
        tra_ori: elementoFalso({ divisa: 'USD' }), tra_des: elementoFalso({ divisa: 'DOP' }),
        tra_div_ori: elementoFalso(), tra_div_des: elementoFalso(), tra_aviso_divisas: elementoFalso(),
    };
    const t = montar({ campos });
    t.vista.rotularDivisasTransferencia();
    assert.equal(campos.tra_div_ori.textContent, 'en USD');
    assert.equal(campos.tra_div_des.textContent, 'en DOP');
    assert.equal(campos.tra_aviso_divisas.style.display, 'block');
    assert.match(campos.tra_aviso_divisas.innerHTML, /cruza divisas/);
    assert.match(campos.tra_aviso_divisas.innerHTML, /USD/);
});

test('rotular: con la misma divisa en los dos lados oculta el aviso', () => {
    const campos = {
        tra_ori: elementoFalso({ divisa: 'DOP' }), tra_des: elementoFalso({ divisa: 'DOP' }),
        tra_div_ori: elementoFalso(), tra_div_des: elementoFalso(),
        tra_aviso_divisas: elementoFalso({ style: { display: 'block' }, innerHTML: 'viejo' }),
    };
    const t = montar({ campos });
    t.vista.rotularDivisasTransferencia();
    assert.equal(campos.tra_aviso_divisas.style.display, 'none');
    assert.equal(campos.tra_aviso_divisas.innerHTML, '');
});

test('rotular: sin cuenta elegida los rótulos quedan vacíos y no hay aviso', () => {
    const campos = {
        tra_ori: elementoFalso(), tra_des: elementoFalso(),
        tra_div_ori: elementoFalso({ textContent: 'viejo' }), tra_div_des: elementoFalso({ textContent: 'viejo' }),
        tra_aviso_divisas: elementoFalso(),
    };
    const t = montar({ campos });
    t.vista.rotularDivisasTransferencia();
    assert.equal(campos.tra_div_ori.textContent, '');
    assert.equal(campos.tra_div_des.textContent, '');
    assert.equal(campos.tra_aviso_divisas.style.display, 'none');
});

test('rotular: si la pantalla no tiene el recuadro de aviso no falla', () => {
    const campos = { tra_ori: elementoFalso({ divisa: 'USD' }), tra_des: elementoFalso({ divisa: 'DOP' }) };
    const t = montar({ campos });
    assert.doesNotThrow(() => t.vista.rotularDivisasTransferencia());
});

test('el puente expone todos los manejadores y delega en la vista', async () => {
    const t = montar({ campos: CAMPOS_TRANSFERENCIA });
    const puente = puenteCuentas(t.vista);
    assert.deepEqual(Object.keys(puente).sort(), [...MANEJADORES_CUENTAS].sort());
    await puente.handleTransferirCuentas(t.evento);
    assert.equal(t.llamadas.length, 1);
});

test('todo appUI.x( que escribe la plantilla está en el puente', async () => {
    const t = montar({ transacciones: [TRANSACCION] });
    await t.vista.render();
    const llamados = new Set([...t.pantalla.contenido.innerHTML.matchAll(/appUI\.(\w+)\(/g)].map(m => m[1]));
    assert.ok(llamados.size >= 2);
    for (const nombre of llamados) assert.ok(MANEJADORES_CUENTAS.includes(nombre), `falta ${nombre} en el puente`);
});
