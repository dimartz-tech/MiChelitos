// Pruebas de la vista «Gastos» en Node, sin navegador.
//
// Es la vista con más reglas de dinero de la interfaz: filtra por el mes, suma
// por divisa sin mezclarlas, calcula la previa de una conversión (convertir
// primero, retener después) y arma el gasto que se envía a Rust. Con la vista
// recibiendo sus dependencias se le dan dobles de todo y un reloj fijo.
// Datos inventados.
//
// Se importa el JavaScript compilado de la vista (`npm test` compila antes).

import test from 'node:test';
import assert from 'node:assert/strict';
import { llamadasDelManejador } from '../ayudas/manejadores_html.js';
import { VistaGastos, MANEJADORES_GASTOS, puenteGastos } from '../../../src/js/vistas/gastos.js';

const gasto = (extra = {}) => ({
    id: 1, fecha: '10/03/2027', monto: 100, divisa: 'DOP', descripcion: 'Gasto de prueba', categoria_id: 1, categoria_nombre: 'Comida',
    metodo_pago: 'efectivo', costo_adicional: 5, tarjeta_id: null, cuenta_ahorro_id: null,
    estado_conversion: null, monto_liquidado: null, tasa_conversion: null, ...extra,
});
const TARJETAS = [{ id: 7, entidad: 'Banco Alfa', nombre_tarjeta: 'Oro', fecha_corte: 10, balance_pesos: 0, balance_dolares: 0 }];
const CUENTAS = [
    { id: 1, nombre: 'Cuenta Pesos', divisa: 'DOP', balance_actual: 1000 },
    { id: 2, nombre: 'Cuenta Dólares', divisa: 'USD', balance_actual: 50 },
];
const CATEGORIAS = [{ id: 1, nombre: 'Comida' }, { id: 2, nombre: 'Transporte' }];

/** Un elemento mínimo del DOM con lo que la vista lee y escribe. */
const el = (spec = {}) => ({ value: '', checked: false, textContent: '', innerHTML: '', style: {}, ...spec, remove() { this.quitado = true; } });
const select = (value, divisa) => ({ ...el({ value }), selectedOptions: [{ dataset: { divisa } }], options: [{ value: '' }, { value }] });

function montar({ gastos = [gasto()], campos = {}, tarjetas = TARJETAS, cuentas = CUENTAS, falla = null } = {}) {
    const llamadas = [], avisos = [], rutas = [], modales = [];
    const api = {
        obtenerGastos: async () => gastos,
        obtenerCategorias: async () => CATEGORIAS,
        obtenerCuentas: async () => cuentas,
        obtenerTarjetas: async () => tarjetas,
        crearGasto: async datos => { llamadas.push(['crearGasto', datos]); if (falla) throw new Error(falla); },
        liquidarConsumoPendiente: async (...a) => { llamadas.push(['liquidarConsumoPendiente', ...a]); if (falla) throw new Error(falla); return 59.5; },
    };
    const pantalla = { contenido: { innerHTML: '' } };
    const vista = new VistaGastos({
        api,
        avisos: { mostrar: (mensaje, tipo) => avisos.push({ mensaje, tipo }) },
        formato: { importe: v => `#${v}#` },
        enrutador: { mostrar: async ruta => { rutas.push(ruta); } },
        pantalla,
        dom: {
            elemento: id => { if (!(id in campos)) throw new Error(`No existe el elemento #${id} en la página.`); return campos[id]; },
            buscar: id => campos[id] ?? null,
        },
        modales: { abrir: (id, html) => modales.push({ id, html }) },
        ahora: () => new Date(2027, 2, 5),
    });
    return { vista, llamadas, avisos, rutas, modales, pantalla, evento: { preventDefault() { this.evitado = (this.evitado || 0) + 1; } } };
}

const CAMPOS_EFECTIVO = () => ({
    gas_fec: el({ value: '05/03/2027' }), gas_mon: el({ value: ' 0075.250 ' }), gas_div: el({ value: 'DOP' }), gas_des: el({ value: 'Compra de prueba' }),
    gas_cat: el({ value: '2' }), gas_met: el({ value: 'efectivo' }),
});

test('consulta las cuatro fuentes y dibuja el título y el formulario', async () => {
    const t = montar();
    await t.vista.render();
    assert.match(t.pantalla.contenido.innerHTML, /Gastos y Egresos/);
    assert.match(t.pantalla.contenido.innerHTML, /id="form-add-gasto"/);
    assert.doesNotMatch(t.pantalla.contenido.innerHTML, /undefined|NaN/);
});

test('el mes por defecto es el del reloj inyectado, y aparece entre las opciones aunque no tenga gastos', async () => {
    const t = montar({ gastos: [gasto({ fecha: '10/02/2027' })] });
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.match(html, /Resumen de Marzo 2027/);
    assert.match(html, /<option value="03\/2027" selected>/);
    assert.match(html, /<option value="02\/2027" >/);
});

test('elegir un mes lo guarda y redibuja con ese mes seleccionado', async () => {
    const t = montar({ gastos: [gasto({ fecha: '10/02/2027' })] });
    await t.vista.handleSelectGastosMonth('02/2027');
    assert.match(t.pantalla.contenido.innerHTML, /Resumen de Febrero 2027/);
    assert.match(t.pantalla.contenido.innerHTML, /<option value="02\/2027" selected>/);
});

test('el mes elegido es estado de cada vista: una vista nueva empieza en el mes del reloj', async () => {
    const a = montar({ gastos: [gasto({ fecha: '10/02/2027' })] });
    await a.vista.handleSelectGastosMonth('02/2027');
    const b = montar({ gastos: [gasto({ fecha: '10/02/2027' })] });
    await b.vista.render();
    assert.match(b.pantalla.contenido.innerHTML, /Resumen de Marzo 2027/);
});

test('los totales por categoría no mezclan divisas y suman el cargo al monto', async () => {
    const t = montar({ gastos: [gasto({ monto: 100, costo_adicional: 5 }), gasto({ id: 2, monto: 20, costo_adicional: 0, divisa: 'USD' })] });
    await t.vista.render();
    assert.match(t.pantalla.contenido.innerHTML, /DOP #105#/);
    assert.match(t.pantalla.contenido.innerHTML, /USD #20#/);
});

test('método de pago: cada uno muestra solo sus campos', () => {
    const campos = { gas_tarjeta_container: el(), gas_cuenta_container: el(), gas_lbtr_container: el() };
    const t = montar({ campos });
    t.vista.toggleMetodoPago('tarjeta');
    assert.deepEqual([campos.gas_tarjeta_container.style.display, campos.gas_cuenta_container.style.display, campos.gas_lbtr_container.style.display], ['block', 'none', 'none']);
    t.vista.toggleMetodoPago('transferencia');
    assert.deepEqual([campos.gas_tarjeta_container.style.display, campos.gas_cuenta_container.style.display, campos.gas_lbtr_container.style.display], ['none', 'block', 'block']);
    t.vista.toggleMetodoPago('efectivo');
    assert.deepEqual([campos.gas_tarjeta_container.style.display, campos.gas_cuenta_container.style.display, campos.gas_lbtr_container.style.display], ['none', 'none', 'none']);
});

test('previa de la conversión: convierte primero, retiene después y suma el LBTR', () => {
    const campos = {
        gas_conversion_container: el(), gas_conversion_previa: el(), gas_met: el({ value: 'transferencia' }),
        gas_cue: select('2', 'DOP'), gas_div: el({ value: 'USD' }), gas_mon: el({ value: '100' }), gas_tasa: el({ value: '60' }), gas_lbtr: el({ checked: true }),
    };
    const t = montar({ campos });
    t.vista.actualizarConversionGasto();
    assert.equal(campos.gas_conversion_container.style.display, 'block');
    // 100 USD × 60 = 6 000; retención 0,2 % = 12; LBTR 100 → saldrán 6 112
    assert.match(campos.gas_conversion_previa.innerHTML, /DOP #6112#/);
    assert.match(campos.gas_conversion_previa.innerHTML, /#6000# convertidos/);
    assert.match(campos.gas_conversion_previa.innerHTML, /#12# de retención/);
    assert.match(campos.gas_conversion_previa.innerHTML, /#100# de LBTR/);
});

test('previa de la conversión: sin tasa pide la tasa; sin cruce de divisas se oculta', () => {
    const base = { gas_conversion_container: el(), gas_conversion_previa: el(), gas_met: el({ value: 'transferencia' }), gas_cue: select('2', 'DOP'), gas_div: el({ value: 'USD' }), gas_mon: el({ value: '100' }), gas_tasa: el({ value: '' }), gas_lbtr: el() };
    const sinTasa = montar({ campos: base });
    sinTasa.vista.actualizarConversionGasto();
    assert.match(base.gas_conversion_previa.textContent, /Indica la tasa para saber cuánto saldrá en DOP/);

    const mismas = { ...base, gas_div: el({ value: 'DOP' }), gas_conversion_container: el({ style: { display: 'block' } }) };
    const sinCruce = montar({ campos: mismas });
    sinCruce.vista.actualizarConversionGasto();
    assert.equal(mismas.gas_conversion_container.style.display, 'none');
    assert.equal(mismas.gas_conversion_previa.textContent, '');
});

test('alta en efectivo: envía el gasto completo, con la tarjeta, la cuenta y la tasa en null', async () => {
    const t = montar({ campos: CAMPOS_EFECTIVO() });
    await t.vista.handleAgregarGasto(t.evento);
    assert.equal(t.evento.evitado, 1);
    assert.deepEqual(t.llamadas, [['crearGasto', {
        fecha: '05/03/2027', monto: '0075.250', divisa: 'DOP', descripcion: 'Compra de prueba', categoria_id: 2, metodo_pago: 'efectivo',
        es_lbtr: false, tarjeta_id: null, cuenta_ahorro_id: null, tasa_cambio: null,
    }]]);
    assert.deepEqual(t.avisos, [{ mensaje: 'Gasto registrado con éxito.', tipo: undefined }]);
    assert.deepEqual(t.rutas, ['gastos']);
});

test('alta con tarjeta: envía la tarjeta elegida; sin elegir, avisa según haya o no tarjetas', async () => {
    const ok = montar({ campos: { ...CAMPOS_EFECTIVO(), gas_met: el({ value: 'tarjeta' }), gas_tar: el({ value: '7' }) } });
    await ok.vista.handleAgregarGasto(ok.evento);
    assert.equal(ok.llamadas[0][1].tarjeta_id, 7);
    assert.equal(ok.llamadas[0][1].cuenta_ahorro_id, null);

    const sinElegir = montar({ campos: { ...CAMPOS_EFECTIVO(), gas_met: el({ value: 'tarjeta' }), gas_tar: { ...el({ value: '' }), options: [{ value: '' }, { value: '7' }] } } });
    await sinElegir.vista.handleAgregarGasto(sinElegir.evento);
    assert.deepEqual(sinElegir.llamadas, []);
    assert.deepEqual(sinElegir.avisos, [{ mensaje: 'Indica con qué tarjeta se pagó el gasto.', tipo: 'error' }]);

    const sinTarjetas = montar({ campos: { ...CAMPOS_EFECTIVO(), gas_met: el({ value: 'tarjeta' }), gas_tar: { ...el({ value: '' }), options: [{ value: '' }] } } });
    await sinTarjetas.vista.handleAgregarGasto(sinTarjetas.evento);
    assert.deepEqual(sinTarjetas.avisos, [{ mensaje: 'No hay tarjetas registradas: registra una antes de pagar con tarjeta.', tipo: 'error' }]);
});

test('alta por transferencia: con divisas distintas exige la tasa y la envía; con la misma divisa no viaja tasa', async () => {
    const cruce = (tasa) => ({ ...CAMPOS_EFECTIVO(), gas_met: el({ value: 'transferencia' }), gas_div: el({ value: 'USD' }), gas_cue: select('1', 'DOP'), gas_tasa: el({ value: tasa }) });
    const sin = montar({ campos: cruce('') });
    await sin.vista.handleAgregarGasto(sin.evento);
    assert.deepEqual(sin.llamadas, []);
    assert.deepEqual(sin.avisos, [{ mensaje: 'El gasto va en USD y la cuenta en DOP: indica la tasa de cambio.', tipo: 'error' }]);

    const con = montar({ campos: cruce('58.5') });
    await con.vista.handleAgregarGasto(con.evento);
    assert.equal(con.llamadas[0][1].tasa_cambio, 58.5);
    assert.equal(con.llamadas[0][1].cuenta_ahorro_id, 1);
    assert.equal(con.llamadas[0][1].tarjeta_id, null);

    const misma = montar({ campos: { ...cruce('58.5'), gas_div: el({ value: 'DOP' }) } });
    await misma.vista.handleAgregarGasto(misma.evento);
    assert.equal(misma.llamadas[0][1].tasa_cambio, null);
});

test('los selectores ocultos no cuentan: en la pantalla real la tarjeta y la cuenta existen siempre, y solo viaja lo del método elegido', async () => {
    const todos = (met) => ({ ...CAMPOS_EFECTIVO(), gas_met: el({ value: met }), gas_tar: el({ value: '7' }), gas_cue: select('1', 'DOP') });
    const efectivo = montar({ campos: todos('efectivo') });
    await efectivo.vista.handleAgregarGasto(efectivo.evento);
    assert.equal(efectivo.llamadas[0][1].tarjeta_id, null, 'en efectivo no viaja la tarjeta aunque el selector tenga valor');
    assert.equal(efectivo.llamadas[0][1].cuenta_ahorro_id, null, 'ni la cuenta');

    const tarjeta = montar({ campos: todos('tarjeta') });
    await tarjeta.vista.handleAgregarGasto(tarjeta.evento);
    assert.equal(tarjeta.llamadas[0][1].tarjeta_id, 7);
    assert.equal(tarjeta.llamadas[0][1].cuenta_ahorro_id, null, 'con tarjeta no viaja la cuenta');

    const transferencia = montar({ campos: todos('transferencia') });
    await transferencia.vista.handleAgregarGasto(transferencia.evento);
    assert.equal(transferencia.llamadas[0][1].cuenta_ahorro_id, 1);
    assert.equal(transferencia.llamadas[0][1].tarjeta_id, null, 'con transferencia no viaja la tarjeta');
});

test('alta: si Rust rechaza avisa del error y no redibuja', async () => {
    const t = montar({ campos: CAMPOS_EFECTIVO(), falla: 'categoría inexistente' });
    await t.vista.handleAgregarGasto(t.evento);
    assert.deepEqual(t.avisos, [{ mensaje: 'Error: categoría inexistente', tipo: 'error' }]);
    assert.deepEqual(t.rutas, []);
});

test('liquidar un consumo: abre la ventana con su identificador y el monto formateado', () => {
    const t = montar();
    t.vista.abrirLiquidacionConsumo(9, 100, 'USD', 'Compra en dólares');
    assert.equal(t.modales.length, 1);
    assert.equal(t.modales[0].id, 'modal-liq-9');
    assert.match(t.modales[0].html, /Compra en dólares/);
    assert.match(t.modales[0].html, /USD #100#/);
    assert.match(t.modales[0].html, /appUI\.handleLiquidacionSubmit\(event, 9, 100\)/);
});

test('liquidar un consumo: el importe viaja como TEXTO, con los espacios recortados y los dígitos intactos', async () => {
    for (const [escrito, esperado] of [[' 6050.5 ', '6050.5'], ['6050.005', '6050.005'], ['0060.500', '0060.500']]) {
        const t = montar({ campos: { liq_monto_9: el({ value: escrito }), 'modal-liq-9': el() } });
        await t.vista.handleLiquidacionSubmit(t.evento, 9, 100);
        assert.strictEqual(t.llamadas[0][2], esperado, JSON.stringify(escrito));
        assert.equal(typeof t.llamadas[0][2], 'string');
    }
});

test('liquidar un consumo: envía el importe en pesos como texto, muestra la tasa, cierra la ventana y redibuja', async () => {
    const campos = { liq_monto_9: el({ value: '6000' }), 'modal-liq-9': el() };
    const t = montar({ campos });
    await t.vista.handleLiquidacionSubmit(t.evento, 9, 100);
    assert.deepEqual(t.llamadas, [['liquidarConsumoPendiente', 9, '6000']]);
    assert.equal(campos['modal-liq-9'].quitado, true);
    assert.match(t.avisos[0].mensaje, /tasa de 59\.5000/);
    assert.deepEqual(t.rutas, ['gastos']);
});

test('liquidar un consumo: un importe que no es positivo no se envía', async () => {
    const t = montar({ campos: { liq_monto_9: el({ value: '0' }) } });
    await t.vista.handleLiquidacionSubmit(t.evento, 9, 100);
    assert.deepEqual(t.llamadas, []);
    assert.equal(t.avisos[0].tipo, 'error');
});

test('previa de la tasa de liquidación: importe / monto de origen con cuatro decimales; vacía si no es positivo', () => {
    const campos = { liq_monto_9: el({ value: '6000' }), liq_tasa_9: el() };
    const t = montar({ campos });
    t.vista.previsualizarTasa(9, 100);
    assert.match(campos.liq_tasa_9.innerHTML, /60\.0000/);
    campos.liq_monto_9.value = '0';
    t.vista.previsualizarTasa(9, 100);
    assert.equal(campos.liq_tasa_9.textContent, '');
});

test('el puente expone todos los manejadores y delega en la vista', async () => {
    const t = montar({ campos: CAMPOS_EFECTIVO() });
    const puente = puenteGastos(t.vista);
    assert.deepEqual(Object.keys(puente).sort(), [...MANEJADORES_GASTOS].sort());
    await puente.handleAgregarGasto(t.evento);
    assert.equal(t.llamadas.length, 1);
});

test('todo appUI.x( que escribe la plantilla (y la ventana de liquidación) está en el puente', async () => {
    const t = montar();
    await t.vista.render();
    t.vista.abrirLiquidacionConsumo(9, 100, 'USD', 'x');
    const html = t.pantalla.contenido.innerHTML + t.modales[0].html;
    const llamados = new Set([...html.matchAll(/appUI\.(\w+)\(/g)].map(m => m[1]));
    assert.ok(llamados.size >= 6);
    for (const nombre of llamados) assert.ok(MANEJADORES_GASTOS.includes(nombre), `falta ${nombre} en el puente`);
});

// --- texto del titular en el HTML y en los manejadores ---

test('una descripción con comillas y marcado llega íntegra al botón de liquidar y no se interpreta', async () => {
    const hostil = `x'); alert(1); //"\\ <img src=x onerror="alert(2)"> &amp;`;
    const t = montar({ gastos: [gasto({ id: 9, estado_conversion: 'pendiente', divisa: 'USD', monto: 50, descripcion: hostil })] });
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.deepEqual(llamadasDelManejador(html, 'abrirLiquidacionConsumo'), [[9, 50, 'USD', hostil]]);
    assert.doesNotMatch(html, /<img/);
    assert.match(html, /&lt;img src=x onerror=&quot;alert\(2\)&quot; ?&gt;|&lt;img src=x onerror=&quot;alert\(2\)&quot;&gt;/);
});

test('la ventana de liquidación también escapa la descripción', async () => {
    const t = montar();
    t.vista.abrirLiquidacionConsumo(9, 50, 'USD', `x'); alert(1); //"\\ <img src=x onerror="alert(2)"> &amp;`);
    assert.doesNotMatch(t.modales[0].html, /<img/);
});
