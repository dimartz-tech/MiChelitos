// Pruebas de la vista «Tarjetas de Crédito» en Node, sin navegador.
//
// Es la que más dinero mueve: abonos (con la tasa de cambio al pagar en dólares
// desde una cuenta en pesos), avances de efectivo (el cargo lo **simula el
// núcleo** antes de confirmar), bonificaciones, límites y los dos historiales
// desde los que se deshace un movimiento con motivo. Con la vista recibiendo sus
// dependencias se le dan dobles de todo, un reloj fijo y diálogos que responden
// **tarde**, como el WebView real. Datos inventados.
//
// Se importa el JavaScript compilado de la vista (`npm test` compila antes).

import test from 'node:test';
import assert from 'node:assert/strict';
import { llamadasDelManejador } from '../ayudas/manejadores_html.js';
import { VistaTarjetas, MANEJADORES_TARJETAS, puenteTarjetas } from '../../../src/js/vistas/tarjetas.js';

const tarjeta = (extra = {}) => ({
    id: 7, entidad: 'Banco Beta', nombre_tarjeta: 'Oro', fecha_corte: 15, fecha_limite_pago: 5,
    limite_pesos: 500, limite_dolares: 100, limite_ajustado_pesos: null, limite_ajustado_dolares: null,
    limite_sobregiro_pesos: 50, limite_sobregiro_dolares: 10, limite_efectivo_pesos: 400, limite_efectivo_dolares: 80,
    balance_pesos: 200, balance_dolares: 0, balance_corte_pesos: 150, balance_corte_dolares: 0,
    disponible_pesos: 300, disponible_dolares: 100, politica_liquidacion: 'origen',
    alerta_corte: false, alerta_pago: false, dias_corte_msg: 'Corte en 5 días', dias_pago_msg: 'Pago en 25 días', ...extra,
});
const CUENTAS = [
    { id: 1, nombre: 'Cuenta Pesos', divisa: 'DOP', balance_actual: 900 },
    { id: 2, nombre: 'Cuenta Dólares', divisa: 'USD', balance_actual: 40 },
];
const BONIFICACION = (extra = {}) => ({ id: 11, fecha: '10/03/2027', tarjeta_id: 7, entidad: 'Banco Beta', nombre_tarjeta: 'Oro', monto: 25, divisa: 'DOP', concepto: 'Cashback', ...extra });

const el = (spec = {}) => ({ value: '', hidden: false, disabled: false, readOnly: false, required: false, placeholder: '', textContent: '', innerHTML: '', ...spec, remove() { this.quitado = true; } });
/** Una cuenta del selector del avance: opciones con su divisa, y la elegida. */
const selectorCuentas = (valor, opciones) => ({
    ...el({ value: valor }), options: opciones.map(o => ({ value: o.value, dataset: { divisa: o.divisa }, hidden: false, disabled: false, text: o.text ?? '' })),
    get selectedOptions() { return this.options.filter(o => o.value === this.value); },
});
/** Responde después, como el WebView real. */
const tarde = valor => new Promise(resolver => setTimeout(() => resolver(valor), 2));

function montar({ tarjetas = [tarjeta()], prestamos = [], bonificaciones = [], campos = {}, falla = {}, confirmar = true, motivo = 'Registrado por error en la tarjeta equivocada', respuesta = '58.5', abonos = [], avances = [], cuentas = CUENTAS } = {}) {
    const llamadas = [], avisos = [], rutas = [], confirmaciones = [], preguntas = [], motivos = [], modales = [];
    const reg = (nombre, valor) => async (...a) => { llamadas.push([nombre, ...a]); if (falla[nombre]) throw new Error(falla[nombre]); return valor; };
    const api = {
        obtenerTarjetas: async () => tarjetas, obtenerPrestamos: async () => prestamos, obtenerBonificaciones: async () => bonificaciones,
        obtenerCuentas: async () => { if (falla.obtenerCuentas) throw new Error(falla.obtenerCuentas); return cuentas; },
        crearBonificacion: reg('crearBonificacion'), eliminarBonificacion: reg('eliminarBonificacion'),
        obtenerAbonosTarjeta: async id => { llamadas.push(['obtenerAbonosTarjeta', id]); if (falla.obtenerAbonosTarjeta) throw new Error(falla.obtenerAbonosTarjeta); return abonos; },
        revertirAbonoTarjeta: reg('revertirAbonoTarjeta', 'Abono revertido (prueba).'), registrarPagoTarjeta: reg('registrarPagoTarjeta'),
        simularAvanceEfectivo: async (...a) => { llamadas.push(['simularAvanceEfectivo', ...a]); if (falla.simularAvanceEfectivo) throw new Error(falla.simularAvanceEfectivo); return { monto: 90, cargo: 5, a_la_tarjeta: 95, a_la_cuenta: 90 }; },
        registrarAvanceEfectivo: reg('registrarAvanceEfectivo', 'Avance registrado (prueba).'),
        obtenerAvancesTarjeta: async id => { llamadas.push(['obtenerAvancesTarjeta', id]); return avances; },
        revertirAvanceEfectivo: reg('revertirAvanceEfectivo', 'Avance revertido (prueba).'), actualizarLimitesTarjeta: reg('actualizarLimitesTarjeta'),
    };
    const pantalla = { contenido: { innerHTML: '' } };
    const registro = { ...campos };
    const vista = new VistaTarjetas({
        api,
        avisos: { mostrar: (mensaje, tipo) => avisos.push({ mensaje, tipo }) },
        formato: { importe: v => `#${v}#` },
        enrutador: { mostrar: async ruta => { rutas.push(ruta); } },
        pantalla,
        dom: {
            elemento: id => { if (!(id in registro)) throw new Error(`No existe el elemento #${id} en la página.`); return registro[id]; },
            buscar: id => registro[id] ?? null,
        },
        dialogos: { confirmar: m => { confirmaciones.push(m); return tarde(confirmar); }, preguntar: (m, d) => { preguntas.push({ m, d }); return tarde(respuesta); } },
        motivo: { pedir: (titulo, texto) => { motivos.push({ titulo, texto }); return tarde(motivo); } },
        modales: { abrir: (id, html) => { modales.push({ id, html }); registro[id] = el({ innerHTML: html }); } },
        ahora: () => new Date(2027, 2, 10),
    });
    return { vista, llamadas, avisos, rutas, confirmaciones, preguntas, motivos, modales, pantalla, registro, evento: { preventDefault() { this.evitado = (this.evitado || 0) + 1; } } };
}

// --- Lo que se dibuja ---

test('recomienda la tarjeta con más días hasta su corte, contando desde el reloj inyectado', async () => {
    // hoy es el 10: el día 15 cae en 5 días; el día 5 ya pasó y cae el 5 del mes siguiente (26 días)
    const t = montar({ tarjetas: [tarjeta({ id: 7, fecha_corte: 15 }), tarjeta({ id: 8, nombre_tarjeta: 'Plata', fecha_corte: 5 })] });
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.match(html, /Próximo corte en <strong>26 días<\/strong> \(Día 5\)/);
    assert.doesNotMatch(html, /undefined|NaN/);
});

test('un corte que hoy mismo es el que más tarda gana: el día de hoy cuenta como corte de este mes', async () => {
    const t = montar({ tarjetas: [tarjeta({ fecha_corte: 10 }), tarjeta({ id: 8, fecha_corte: 12 })] });
    await t.vista.render();
    assert.match(t.pantalla.contenido.innerHTML, /Próximo corte en <strong>2 días<\/strong> \(Día 12\)/);
});

test('sin tarjetas no hay recomendación ni formulario de bonificaciones, y lo dice', async () => {
    const t = montar({ tarjetas: [] });
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.doesNotMatch(html, /Próximo corte en/);
    assert.doesNotMatch(html, /id="bon_/);
});

test('un saldo a favor se rotula «A favor», no como un uso negativo', () => {
    const { vista } = montar();
    assert.deepEqual(vista.etiquetaBalanceTarjeta(-100, 'DOP'), { texto: 'A favor: DOP #100#', color: '#10b981' });
    assert.deepEqual(vista.etiquetaBalanceTarjeta(40, 'USD'), { texto: 'Uso: USD #40#', color: 'inherit' });
});

test('el porcentaje de uso se acota entre 0 y 100, y sin límite es 0', () => {
    const { vista } = montar();
    assert.equal(vista.porcentajeUso(50, 200), 25);
    assert.equal(vista.porcentajeUso(-30, 200), 0);     // saldo a favor: una barra no puede tener anchura negativa
    assert.equal(vista.porcentajeUso(500, 200), 100);
    assert.equal(vista.porcentajeUso(50, 0), 0);
});

test('una facilidad que cuelga de la tarjeta se advierte en ella, con su cuota', async () => {
    const t = montar({ prestamos: [{ id: 3, tarjeta_id: 7, institucion_financiera: 'Banco Beta', monto_cuota: 35 }, { id: 4, tarjeta_id: null, institucion_financiera: 'Financiera Zeta', monto_cuota: 99 }] });
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.match(html, /Esta tarjeta cobra/);
    assert.match(html, /<strong>Banco Beta<\/strong> \(cuota DOP #35#\)/);
    assert.doesNotMatch(html, /Financiera Zeta/);
});

test('las bonificaciones se suman por divisa y sin ellas se dice «—»', async () => {
    const t = montar({ bonificaciones: [BONIFICACION(), BONIFICACION({ id: 12, monto: 5 }), BONIFICACION({ id: 13, monto: 7, divisa: 'USD' })] });
    await t.vista.render();
    assert.match(t.pantalla.contenido.innerHTML, /Total acreditado: <strong>DOP #30#<\/strong> · <strong>USD #7#<\/strong>/);
    assert.match(t.pantalla.contenido.innerHTML, /appUI\.handleEliminarBonificacion\(13\)/);
    const vacio = montar();
    await vacio.vista.render();
    assert.doesNotMatch(vacio.pantalla.contenido.innerHTML, /Total acreditado/);
});

// --- Proponer el importe de un abono ---

const CAMPOS_ABONO = (extra = {}) => ({ pag_tipo_7: el({ value: 'corte' }), pag_div_7: el({ value: 'DOP' }), pag_monto_7: el(), ...extra });

test('abono: propone el saldo al corte o el actual según el tipo y la divisa, de solo lectura', () => {
    const t = montar({ campos: CAMPOS_ABONO() });
    t.vista.aplicarTipoAbono(7, 150, 40, 200, 60);
    assert.deepEqual([t.registro.pag_monto_7.value, t.registro.pag_monto_7.readOnly], ['150.00', true]);
    t.registro.pag_tipo_7.value = 'actual';
    t.vista.aplicarTipoAbono(7, 150, 40, 200, 60);
    assert.equal(t.registro.pag_monto_7.value, '200.00');
    t.registro.pag_div_7.value = 'USD';
    t.vista.aplicarTipoAbono(7, 150, 40, 200, 60);
    assert.equal(t.registro.pag_monto_7.value, '60.00');
    t.registro.pag_tipo_7.value = 'corte';
    t.vista.aplicarTipoAbono(7, 150, 40, 200, 60);
    assert.equal(t.registro.pag_monto_7.value, '40.00');
    assert.deepEqual(t.avisos, []);
});

test('abono: «personalizado» deja escribir el importe y no avisa', () => {
    const t = montar({ campos: CAMPOS_ABONO({ pag_tipo_7: el({ value: 'personalizado' }), pag_monto_7: el({ readOnly: true, value: '12' }) }) });
    t.vista.aplicarTipoAbono(7, 150, 40, 200, 60);
    assert.equal(t.registro.pag_monto_7.readOnly, false);
    assert.equal(t.registro.pag_monto_7.value, '12');
    assert.deepEqual(t.avisos, []);
});

test('abono: con saldo a favor no propone un negativo: pone 0, bloquea el campo y explica por qué', () => {
    const t = montar({ campos: CAMPOS_ABONO({ pag_tipo_7: el({ value: 'actual' }) }) });
    t.vista.aplicarTipoAbono(7, 150, 40, -25, 0);
    assert.deepEqual([t.registro.pag_monto_7.value, t.registro.pag_monto_7.readOnly], ['0.00', true]);
    assert.deepEqual(t.avisos, [{ mensaje: 'La tarjeta tiene DOP #25# a favor. No hay saldo actual que abonar.', tipo: 'info' }]);
});

test('abono: con saldo cero propone 0.00 y lo avisa', () => {
    const t = montar({ campos: CAMPOS_ABONO() });
    t.vista.aplicarTipoAbono(7, 0, 0, 200, 60);
    assert.equal(t.registro.pag_monto_7.value, '0.00');
    assert.deepEqual(t.avisos, [{ mensaje: 'No hay saldo al corte en DOP.', tipo: 'info' }]);
});

// --- Registrar un abono ---

const CAMPOS_PAGO = (extra = {}) => ({
    pag_fecha_7: el({ value: '10/03/2027' }), pag_div_7: el({ value: 'DOP' }), pag_monto_7: el({ value: '120.5' }),
    pag_cuenta_7: el({ value: '' }), pag_tasa_7: el({ value: '' }), ...extra,
});

test('abono en efectivo: el monto como número, sin cuenta y sin tasa; redibuja Tarjetas', async () => {
    const t = montar({ campos: CAMPOS_PAGO() });
    await t.vista.handleAbonoTarjeta(t.evento, 7);
    assert.equal(t.evento.evitado, 1);
    assert.deepEqual(t.llamadas, [['registrarPagoTarjeta', 7, '10/03/2027', '120.5', 'DOP', null, 0]]);
    assert.deepEqual(t.avisos, [{ mensaje: 'Abono a tarjeta guardado.', tipo: undefined }]);
    assert.deepEqual(t.rutas, ['tarjetas']);
});

test('abono: el importe viaja como TEXTO, recortado y con los dígitos intactos (la tasa sigue siendo un número)', async () => {
    for (const [escrito, esperado] of [[' 120.5 ', '120.5'], ['1000.005', '1000.005'], ['0120.500', '0120.500']]) {
        const t = montar({ campos: CAMPOS_PAGO({ pag_monto_7: el({ value: escrito }) }) });
        await t.vista.handleAbonoTarjeta(t.evento, 7);
        assert.strictEqual(t.llamadas[0][3], esperado, JSON.stringify(escrito));
        assert.equal(typeof t.llamadas[0][3], 'string');
    }
});

test('abono: un importe que no es mayor que cero se rechaza antes de tocar la API', async () => {
    for (const monto of ['0', '-5', '', 'abc']) {
        const t = montar({ campos: CAMPOS_PAGO({ pag_monto_7: el({ value: monto }) }) });
        await t.vista.handleAbonoTarjeta(t.evento, 7);
        assert.deepEqual(t.llamadas, [], monto);
        assert.equal(t.avisos[0].tipo, 'error', monto);
    }
});

test('abono en dólares desde una cuenta en pesos pide la tasa, con 60.0 por defecto, y la envía', async () => {
    const t = montar({ campos: CAMPOS_PAGO({ pag_div_7: el({ value: 'USD' }), pag_cuenta_7: el({ value: '1' }) }), respuesta: '58.5' });
    const espera = t.vista.handleAbonoTarjeta(t.evento, 7);
    assert.deepEqual(t.llamadas, [], 'no envía antes de que el titular responda');
    await espera;
    assert.equal(t.preguntas[0].d, '60.0');
    assert.match(t.preguntas[0].m, /USD 120\.5 desde la cuenta en Pesos "Cuenta Pesos"/);
    assert.deepEqual(t.llamadas, [['registrarPagoTarjeta', 7, '10/03/2027', '120.5', 'USD', 1, 58.5]]);
});

test('abono con la tasa ya escrita, o entre cuentas de la misma divisa, no la pregunta', async () => {
    const conTasa = montar({ campos: CAMPOS_PAGO({ pag_div_7: el({ value: 'USD' }), pag_cuenta_7: el({ value: '1' }), pag_tasa_7: el({ value: '61.25' }) }) });
    await conTasa.vista.handleAbonoTarjeta(conTasa.evento, 7);
    assert.equal(conTasa.preguntas.length, 0);
    assert.equal(conTasa.llamadas[0][6], 61.25);
    const mismaDivisa = montar({ campos: CAMPOS_PAGO({ pag_div_7: el({ value: 'USD' }), pag_cuenta_7: el({ value: '2' }) }) });
    await mismaDivisa.vista.handleAbonoTarjeta(mismaDivisa.evento, 7);
    assert.equal(mismaDivisa.preguntas.length, 0);
    assert.deepEqual(mismaDivisa.llamadas[0].slice(4), ['USD', 2, 0]);
});

test('abono: si se cancela la tasa o no es válida, no se envía nada', async () => {
    for (const [respuesta, mensaje, tipo] of [[null, /Operación cancelada/, 'info'], ['abc', /Tasa de cambio inválida/, 'error'], ['0', /Tasa de cambio inválida/, 'error'], ['-3', /Tasa de cambio inválida/, 'error']]) {
        const t = montar({ campos: CAMPOS_PAGO({ pag_div_7: el({ value: 'USD' }), pag_cuenta_7: el({ value: '1' }) }), respuesta });
        await t.vista.handleAbonoTarjeta(t.evento, 7);
        assert.deepEqual(t.llamadas, [], String(respuesta));
        assert.match(t.avisos[0].mensaje, mensaje);
        assert.equal(t.avisos[0].tipo, tipo);
    }
});

test('abono: si falla la consulta de cuentas o el envío, se avisa el error', async () => {
    const a = montar({ campos: CAMPOS_PAGO({ pag_cuenta_7: el({ value: '1' }) }), falla: { obtenerCuentas: 'Sin acceso' } });
    await a.vista.handleAbonoTarjeta(a.evento, 7);
    assert.match(a.avisos[0].mensaje, /Error al validar cuenta: .*Sin acceso/);
    assert.deepEqual(a.llamadas, []);
    const b = montar({ campos: CAMPOS_PAGO(), falla: { registrarPagoTarjeta: 'Rechazado' } });
    await b.vista.handleAbonoTarjeta(b.evento, 7);
    assert.equal(b.avisos[0].tipo, 'error');
    assert.deepEqual(b.rutas, []);
});

// --- Avance de efectivo ---

const CAMPOS_AVANCE = (extra = {}) => ({
    avc_div_7: el({ value: 'DOP' }), avc_tipo_7: el({ value: 'porcentaje' }),
    avc_cuenta_7: selectorCuentas('1', [{ value: '', divisa: '' }, { value: '1', divisa: 'DOP', text: 'Cuenta Pesos - Bal: 900' }, { value: '2', divisa: 'USD', text: 'Cuenta Dólares - Bal: 40' }]),
    avc_valor_caja_7: el({ hidden: true }), avc_valor_et_7: el(), avc_valor_7: el({ value: '6.25' }),
    avc_fecha_7: el({ value: ' 10/03/2027 ' }), avc_monto_7: el({ value: ' 90 ' }), avc_nota_7: el({ value: ' Para la renta ' }), forma: null, ...extra,
});

test('el formulario del avance solo ofrece las cuentas de su divisa, y deselecciona una que ya no sirve', () => {
    const t = montar({ campos: CAMPOS_AVANCE() });
    t.registro.avc_div_7.value = 'USD';
    t.vista.aplicarTipoAvance(7);
    const [vacia, pesos, dolares] = t.registro.avc_cuenta_7.options;
    assert.deepEqual([vacia.hidden, pesos.hidden, pesos.disabled, dolares.hidden, dolares.disabled], [false, true, true, false, false]);
    assert.equal(t.registro.avc_cuenta_7.value, '', 'la cuenta en pesos elegida deja de valer');
});

test('el formulario del avance ajusta el valor al tipo de cargo: porcentaje, fijo o exonerado', () => {
    const t = montar({ campos: CAMPOS_AVANCE() });
    t.vista.aplicarTipoAvance(7);
    assert.deepEqual([t.registro.avc_valor_caja_7.hidden, t.registro.avc_valor_7.required, t.registro.avc_valor_et_7.textContent, t.registro.avc_valor_7.placeholder], [false, true, 'Porcentaje (%)', '6.25']);
    t.registro.avc_tipo_7.value = 'fijo';
    t.vista.aplicarTipoAvance(7);
    assert.deepEqual([t.registro.avc_valor_et_7.textContent, t.registro.avc_valor_7.placeholder], ['Cargo fijo (DOP)', '0.00']);
    t.registro.avc_tipo_7.value = 'exonerado';
    t.vista.aplicarTipoAvance(7);
    assert.deepEqual([t.registro.avc_valor_caja_7.hidden, t.registro.avc_valor_7.required, t.registro.avc_valor_7.value], [true, false, '']);
});

test('desplegar el formulario del avance lo ajusta; plegarlo no', () => {
    const forma = el({ hidden: true });
    const t = montar({ campos: { ...CAMPOS_AVANCE(), avance_7: forma } });
    t.vista.alternarAvance(7);
    assert.equal(forma.hidden, false);
    assert.equal(t.registro.avc_valor_et_7.textContent, 'Porcentaje (%)');
    t.registro.avc_valor_et_7.textContent = 'tocado';
    t.vista.alternarAvance(7);
    assert.equal(forma.hidden, true);
    assert.equal(t.registro.avc_valor_et_7.textContent, 'tocado');
    assert.doesNotThrow(() => montar().vista.alternarAvance(7));    // sin formulario en la página, no hace nada
});

test('registrar un avance: simula en el núcleo, confirma con SUS cifras y solo entonces registra', async () => {
    const t = montar({ campos: CAMPOS_AVANCE() });
    const espera = t.vista.handleAvanceEfectivo(t.evento, 7);
    await new Promise(r => setImmediate(r));
    assert.deepEqual(t.llamadas.map(l => l[0]), ['simularAvanceEfectivo'], 'primero simula, sin registrar');
    assert.deepEqual(t.llamadas[0].slice(1), ['90', 'DOP', 'porcentaje', 6.25, null]);
    await espera;
    assert.match(t.confirmaciones[0], /Monto: DOP #90# → Cuenta Pesos/);
    assert.match(t.confirmaciones[0], /Cargo \(6\.25%\): DOP #5#/);
    assert.match(t.confirmaciones[0], /La deuda de la tarjeta sube: DOP #95#/);
    assert.deepEqual(t.llamadas[1], ['registrarAvanceEfectivo', 7, '1', '10/03/2027', '90', 'DOP', 'porcentaje', 6.25, null, 'Para la renta']);
    assert.deepEqual(t.rutas, ['tarjetas']);
});

test('avance con cargo fijo: el monto y el cargo van como TEXTO; el porcentaje, que es una tasa, como número', async () => {
    const t = montar({ campos: CAMPOS_AVANCE({ avc_tipo_7: el({ value: 'fijo' }), avc_valor_7: el({ value: '3.456' }) }) });
    await t.vista.handleAvanceEfectivo(t.evento, 7);
    const registro = t.llamadas.find(l => l[0] === 'registrarAvanceEfectivo');
    assert.deepEqual([registro[4], registro[6], registro[7], registro[8]], ['90', 'fijo', null, '3.456']);
    assert.equal(typeof registro[8], 'string');
    assert.match(t.confirmaciones[0], /Cargo fijo: DOP #5#/);
});

test('avance exonerado: sin porcentaje ni cargo fijo, aunque el campo tenga algo', async () => {
    const t = montar({ campos: CAMPOS_AVANCE({ avc_tipo_7: el({ value: 'exonerado' }), avc_valor_7: el({ value: '9' }) }) });
    await t.vista.handleAvanceEfectivo(t.evento, 7);
    const registro = t.llamadas.find(l => l[0] === 'registrarAvanceEfectivo');
    assert.deepEqual([registro[6], registro[7], registro[8]], ['exonerado', null, null]);
    assert.match(t.confirmaciones[0], /Cargo \(exonerado\)/);
});

test('avance: sin cuenta, o sin confirmar, no se registra nada', async () => {
    const sinCuenta = montar({ campos: CAMPOS_AVANCE({ avc_cuenta_7: selectorCuentas('', [{ value: '', divisa: '' }]) }) });
    await sinCuenta.vista.handleAvanceEfectivo(sinCuenta.evento, 7);
    assert.deepEqual(sinCuenta.llamadas, []);
    assert.match(sinCuenta.avisos[0].mensaje, /Elige la cuenta/);
    const no = montar({ campos: CAMPOS_AVANCE(), confirmar: false });
    await no.vista.handleAvanceEfectivo(no.evento, 7);
    assert.deepEqual(no.llamadas.map(l => l[0]), ['simularAvanceEfectivo']);
    assert.deepEqual(no.rutas, []);
});

test('avance: si la simulación o el registro fallan, se avisa el error', async () => {
    const a = montar({ campos: CAMPOS_AVANCE(), falla: { simularAvanceEfectivo: 'Cuenta de otra divisa' } });
    await a.vista.handleAvanceEfectivo(a.evento, 7);
    assert.equal(a.avisos[0].tipo, 'error');
    assert.equal(a.confirmaciones.length, 0, 'sin simulación no hay nada que confirmar');
    const b = montar({ campos: CAMPOS_AVANCE(), falla: { registrarAvanceEfectivo: 'Rechazado' } });
    await b.vista.handleAvanceEfectivo(b.evento, 7);
    assert.equal(b.avisos[0].tipo, 'error');
    assert.deepEqual(b.rutas, []);
});

// --- Historiales: abonos y avances ---

test('el historial de abonos se carga al abrirlo, se oculta al repetir y trae el botón de deshacer de cada uno', async () => {
    const caja = el({ hidden: true });
    const t = montar({ campos: { abonos_7: caja }, abonos: [{ id: 41, fecha_pago: '10/03/2027', monto_pagado: 80, divisa: 'USD', cuenta_nombre: null, tasa_cambio: 58.5 }, { id: 42, fecha_pago: '11/03/2027', monto_pagado: 20, divisa: 'DOP', cuenta_nombre: 'Cuenta Pesos', tasa_cambio: 1 }] });
    const espera = t.vista.alternarAbonos(7);
    assert.equal(caja.hidden, false);
    assert.match(caja.innerHTML, /Cargando/);
    await espera;
    assert.deepEqual(t.llamadas, [['obtenerAbonosTarjeta', 7]]);
    assert.match(caja.innerHTML, /USD #80#/);
    assert.match(caja.innerHTML, /sin cuenta asociada · tasa 58\.5/);
    assert.doesNotMatch(caja.innerHTML, /DOP #20#[\s\S]*tasa 1</);       // una tasa de 1 no se muestra
    assert.match(caja.innerHTML, /appUI\.handleRevertirAbono\(41, 7\)/);
    await t.vista.alternarAbonos(7);
    assert.equal(caja.hidden, true);
    assert.equal(t.llamadas.length, 1, 'al ocultarlo no vuelve a consultar');
});

test('historial de abonos: sin abonos lo dice, un fallo se muestra en el panel, y sin panel no hace nada', async () => {
    const vacia = el({ hidden: true });
    await montar({ campos: { abonos_7: vacia } }).vista.alternarAbonos(7);
    assert.match(vacia.innerHTML, /Sin abonos registrados/);
    const rota = el({ hidden: true });
    await montar({ campos: { abonos_7: rota }, falla: { obtenerAbonosTarjeta: 'Sin acceso' } }).vista.alternarAbonos(7);
    assert.match(rota.innerHTML, /Sin acceso/);
    await assert.doesNotReject(() => montar().vista.alternarAbonos(7));
});

test('el historial de avances: cargo por tipo, nota y botón de deshacer; sin avances lo dice', async () => {
    const caja = el({ hidden: true });
    const t = montar({ campos: { avances_7: caja }, avances: [
        { id: 52, fecha: '10/03/2027', monto: 90, divisa: 'DOP', tipo_cargo: 'porcentaje', tasa: 6.25, cargo: 5, cuenta_nombre: 'Cuenta Pesos', nota: 'Para la renta' },
        { id: 53, fecha: '11/03/2027', monto: 10, divisa: 'DOP', tipo_cargo: 'exonerado', tasa: null, cargo: 0, cuenta_nombre: 'Cuenta Pesos', nota: null },
    ] });
    await t.vista.alternarAvances(7);
    assert.match(caja.innerHTML, /cargo 6\.25% = DOP #5#/);
    assert.match(caja.innerHTML, /Para la renta/);
    assert.match(caja.innerHTML, /exonerado/);
    assert.match(caja.innerHTML, /appUI\.handleRevertirAvance\(52, 7\)/);
    const vacio = el({ hidden: true });
    await montar({ campos: { avances_7: vacio } }).vista.alternarAvances(7);
    assert.match(vacio.innerHTML, /Sin avances registrados/);
});

const REVERSIONES = [
    ['handleRevertirAbono', 'revertirAbonoTarjeta', 'abonos_7', 'obtenerAbonosTarjeta', 41, /Vas a borrar este abono a tarjeta/, /repondrá la deuda/],
    ['handleRevertirAvance', 'revertirAvanceEfectivo', 'avances_7', 'obtenerAvancesTarjeta', 52, /Vas a borrar este avance de efectivo/, /Si ya gastaste ese dinero/],
];

test('deshacer un abono o un avance: confirma, pide el motivo, envía (id, motivo), redibuja y reabre el historial de ESA tarjeta', async () => {
    const MOTIVO = 'Registrado dos veces por error';
    for (const [metodo, api, caja, lectura, id, titulo, aviso] of REVERSIONES) {
        const t = montar({ campos: { [caja]: el({ hidden: true }) }, motivo: MOTIVO });
        const espera = t.vista[metodo](id, 7);
        assert.deepEqual(t.llamadas, [], `${metodo}: no debe actuar antes de las respuestas`);
        await espera;
        assert.match(t.confirmaciones[0], aviso, metodo);
        assert.match(t.motivos[0].titulo, titulo, metodo);
        assert.deepEqual(t.llamadas, [[api, id, MOTIVO], [lectura, 7]], metodo);
        assert.equal(t.registro[caja].hidden, false, metodo);
        assert.deepEqual(t.rutas, ['tarjetas'], metodo);
        assert.match(t.avisos[0].mensaje, /revertido \(prueba\)/, metodo);
    }
});

test('deshacer: sin confirmar o con el motivo cancelado no se borra nada; si Rust rechaza, se avisa', async () => {
    for (const [metodo, api, caja, , id] of REVERSIONES) {
        const sinConfirmar = montar({ confirmar: false });
        await sinConfirmar.vista[metodo](id, 7);
        assert.deepEqual([sinConfirmar.llamadas, sinConfirmar.motivos.length], [[], 0], metodo);
        const sinMotivo = montar({ motivo: null });
        await sinMotivo.vista[metodo](id, 7);
        assert.deepEqual([sinMotivo.llamadas, sinMotivo.rutas], [[], []], metodo);
        const rechazada = montar({ falla: { [api]: 'No' }, campos: { [caja]: el({ hidden: true }) } });
        await rechazada.vista[metodo](id, 7);
        assert.equal(rechazada.avisos.at(-1).tipo, 'error', metodo);
        assert.deepEqual(rechazada.rutas, [], metodo);
    }
});

// --- Bonificaciones ---

const CAMPOS_BON = (extra = {}) => ({ bon_fecha: el({ value: ' 10/03/2027 ' }), bon_tarjeta: el({ value: '7' }), bon_divisa: el({ value: 'USD' }), bon_monto: el({ value: '12.5' }), bon_concepto: el({ value: ' Cashback de marzo ' }), ...extra });

test('alta de bonificación: texto recortado, tarjeta y monto como número, y redibuja', async () => {
    const t = montar({ campos: CAMPOS_BON() });
    await t.vista.handleAgregarBonificacion(t.evento);
    assert.equal(t.evento.evitado, 1);
    assert.deepEqual(t.llamadas, [['crearBonificacion', '10/03/2027', 7, '12.5', 'USD', 'Cashback de marzo']]);
    assert.match(t.avisos[0].mensaje, /La deuda de la tarjeta se redujo/);
    assert.deepEqual(t.rutas, ['tarjetas']);
});

test('bonificación: el importe viaja como TEXTO, con los espacios recortados y los dígitos intactos', async () => {
    for (const [escrito, esperado] of [[' 12.5 ', '12.5'], ['1.005', '1.005'], ['0012.500', '0012.500']]) {
        const t = montar({ campos: CAMPOS_BON({ bon_monto: el({ value: escrito }) }) });
        await t.vista.handleAgregarBonificacion(t.evento);
        assert.strictEqual(t.llamadas[0][3], esperado, JSON.stringify(escrito));
        assert.equal(typeof t.llamadas[0][3], 'string');
    }
});

test('bonificación: sin monto mayor que cero o sin concepto no se envía', async () => {
    for (const [extra, mensaje] of [[{ bon_monto: el({ value: '0' }) }, /mayor que cero/], [{ bon_monto: el({ value: '' }) }, /mayor que cero/], [{ bon_concepto: el({ value: '   ' }) }, /Indica el concepto/]]) {
        const t = montar({ campos: CAMPOS_BON(extra) });
        await t.vista.handleAgregarBonificacion(t.evento);
        assert.deepEqual(t.llamadas, []);
        assert.equal(t.avisos[0].tipo, 'error');
        assert.match(t.avisos[0].mensaje, mensaje);
    }
});

test('revertir una bonificación envía el id y redibuja; si Rust rechaza, avisa', async () => {
    const t = montar();
    await t.vista.handleEliminarBonificacion(11);
    assert.deepEqual(t.llamadas, [['eliminarBonificacion', 11]]);
    assert.match(t.avisos[0].mensaje, /La deuda vuelve a su valor anterior/);
    assert.deepEqual(t.rutas, ['tarjetas']);
    const u = montar({ falla: { eliminarBonificacion: 'No' } });
    await u.vista.handleEliminarBonificacion(11);
    assert.equal(u.avisos[0].tipo, 'error');
    assert.deepEqual(u.rutas, []);
});

// --- Límites y política de liquidación ---

test('la ventana de límites lleva los valores de la tarjeta y su política elegida', () => {
    const t = montar();
    t.vista.abrirEdicionLimitesTarjeta(JSON.stringify(tarjeta({ politica_liquidacion: 'traduce', limite_ajustado_pesos: 450 })));
    assert.equal(t.modales[0].id, 'modal-edit-tar-7');
    assert.match(t.modales[0].html, /Banco Beta - Oro/);
    assert.match(t.modales[0].html, /id="edit_lim_dop_7"[^>]*value="500"/);
    assert.match(t.modales[0].html, /id="edit_aju_dop_7"[^>]*value="450"/);
    assert.match(t.modales[0].html, /value="traduce" selected/);
});

const CAMPOS_LIMITES = (extra = {}) => ({
    edit_lim_dop_7: el({ value: '500' }), edit_sob_dop_7: el({ value: '50' }), edit_cor_dop_7: el({ value: '150' }),
    edit_lim_usd_7: el({ value: '100' }), edit_sob_usd_7: el({ value: '10' }), edit_cor_usd_7: el({ value: '30' }),
    edit_aju_dop_7: el({ value: '' }), edit_aju_usd_7: el({ value: '' }), edit_pol_7: el({ value: 'traduce' }), 'modal-edit-tar-7': el(), ...extra,
});

test('guardar límites: texto, ajuste en blanco como «sin ajuste» y política elegida; cierra la ventana y redibuja', async () => {
    const t = montar({ campos: CAMPOS_LIMITES() });
    await t.vista.handleEdicionLimitesTarjetaSubmit(t.evento, 7);
    // (id, límite DOP, límite USD, sobregiro DOP, sobregiro USD, corte DOP, corte USD, ajustado DOP, ajustado USD, política)
    assert.deepEqual(t.llamadas, [['actualizarLimitesTarjeta', 7, '500', '100', '50', '10', '150', '30', null, null, 'traduce']]);
    assert.equal(t.registro['modal-edit-tar-7'].quitado, true);
    assert.deepEqual(t.rutas, ['tarjetas']);
});

test('un límite ajustado de cero es un tope deliberado, no «sin ajuste»; la política por defecto es «origen»', async () => {
    const t = montar({ campos: CAMPOS_LIMITES({ edit_aju_dop_7: el({ value: '0' }), edit_pol_7: undefined }) });
    delete t.registro.edit_pol_7;
    await t.vista.handleEdicionLimitesTarjetaSubmit(t.evento, 7);
    assert.deepEqual([t.llamadas[0][8], t.llamadas[0][9], t.llamadas[0][10]], ['0', null, 'origen']);
});

test('guardar límites: un límite en blanco se envía como «0» y un ajustado conserva sus dígitos escritos', async () => {
    const t = montar({ campos: CAMPOS_LIMITES({ edit_sob_dop_7: el({ value: '  ' }), edit_aju_dop_7: el({ value: '0075.250' }), edit_aju_usd_7: el({ value: ' 1.005 ' }) }) });
    await t.vista.handleEdicionLimitesTarjetaSubmit(t.evento, 7);
    assert.deepEqual(t.llamadas[0].slice(1, 10), [7, '500', '100', '0', '10', '150', '30', '0075.250', '1.005']);
});

test('un ajustado se compara como número con el aprobado (1000 supera a 999), no como texto', async () => {
    const t = montar({ campos: CAMPOS_LIMITES({ edit_lim_dop_7: el({ value: '999' }), edit_aju_dop_7: el({ value: '1000' }) }) });
    await t.vista.handleEdicionLimitesTarjetaSubmit(t.evento, 7);
    assert.deepEqual(t.llamadas, []);
});

test('un límite ajustado mayor que el aprobado se rechaza antes de tocar la API', async () => {
    for (const extra of [{ edit_aju_dop_7: el({ value: '501' }) }, { edit_aju_usd_7: el({ value: '101' }) }]) {
        const t = montar({ campos: CAMPOS_LIMITES(extra) });
        await t.vista.handleEdicionLimitesTarjetaSubmit(t.evento, 7);
        assert.deepEqual(t.llamadas, []);
        assert.match(t.avisos[0].mensaje, /no puede superar al aprobado/);
        assert.equal(t.registro['modal-edit-tar-7'].quitado, undefined);
    }
});

test('guardar límites: si Rust rechaza, la ventana sigue abierta', async () => {
    const t = montar({ campos: CAMPOS_LIMITES(), falla: { actualizarLimitesTarjeta: 'No' } });
    await t.vista.handleEdicionLimitesTarjetaSubmit(t.evento, 7);
    assert.equal(t.avisos[0].tipo, 'error');
    assert.equal(t.registro['modal-edit-tar-7'].quitado, undefined);
    assert.deepEqual(t.rutas, []);
});

// --- El puente ---

test('el puente expone todos los manejadores y delega en la vista', async () => {
    const t = montar({ campos: CAMPOS_PAGO() });
    const puente = puenteTarjetas(t.vista);
    assert.deepEqual(Object.keys(puente).sort(), [...MANEJADORES_TARJETAS].sort());
    await puente.handleAbonoTarjeta(t.evento, 7);
    assert.equal(t.llamadas.length, 1);
});

test('todo appUI.x( que escribe la plantilla y su ventana está en el puente, y todo el puente está en la plantilla', async () => {
    const t = montar({ tarjetas: [tarjeta()], bonificaciones: [BONIFICACION()], abonos: [{ id: 41, fecha_pago: '10/03/2027', monto_pagado: 8, divisa: 'DOP', cuenta_nombre: 'x', tasa_cambio: 1 }], avances: [{ id: 52, fecha: '10/03/2027', monto: 9, divisa: 'DOP', tipo_cargo: 'fijo', tasa: null, cargo: 1, cuenta_nombre: 'x', nota: null }], campos: { abonos_7: el({ hidden: true }), avances_7: el({ hidden: true }) } });
    await t.vista.render();
    await t.vista.alternarAbonos(7);
    await t.vista.alternarAvances(7);
    t.vista.abrirEdicionLimitesTarjeta(JSON.stringify(tarjeta()));
    const html = t.pantalla.contenido.innerHTML + t.registro.abonos_7.innerHTML + t.registro.avances_7.innerHTML + t.modales[0].html;
    const llamados = new Set([...html.matchAll(/appUI\.(\w+)\(/g)].map(m => m[1]));
    for (const nombre of llamados) assert.ok(MANEJADORES_TARJETAS.includes(nombre), `falta ${nombre} en el puente`);
    for (const nombre of MANEJADORES_TARJETAS) assert.ok(llamados.has(nombre), `${nombre} está en el puente pero ninguna plantilla lo usa`);
});

// --- texto del titular en el HTML y en los manejadores ---

test('una tarjeta con nombre hostil llega íntegra al editor de límites (JSON en texto) y no se interpreta', async () => {
    const hostil = `x'); alert(1); //"\\ <img src=x onerror="alert(2)"> &amp;`;
    const tj = tarjeta({ entidad: hostil, nombre_tarjeta: hostil });
    const t = montar({ tarjetas: [tj], bonificaciones: [BONIFICACION({ concepto: hostil, entidad: hostil, nombre_tarjeta: hostil })] });
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    const [[texto]] = llamadasDelManejador(html, 'abrirEdicionLimitesTarjeta');
    assert.equal(JSON.parse(texto).nombre_tarjeta, hostil);
    assert.doesNotMatch(html, /<img/);
});

test('los historiales y su ventana escapan los nombres y las notas del titular', async () => {
    const hostil = `x'); alert(1); //"\\ <img src=x onerror="alert(2)"> &amp;`;
    const t = montar({
        campos: { abonos_7: el({ hidden: true }), avances_7: el({ hidden: true }) },
        abonos: [{ id: 1, fecha_pago: '10/03/2027', monto_pagado: 8, divisa: 'DOP', cuenta_nombre: hostil, tasa_cambio: 1 }],
        avances: [{ id: 2, fecha: '10/03/2027', monto: 9, divisa: 'DOP', tipo_cargo: 'fijo', tasa: null, cargo: 1, cuenta_nombre: hostil, nota: hostil }],
    });
    await t.vista.alternarAbonos(7);
    await t.vista.alternarAvances(7);
    t.vista.abrirEdicionLimitesTarjeta(JSON.stringify(tarjeta({ entidad: hostil })));
    for (const html of [t.registro.abonos_7.innerHTML, t.registro.avances_7.innerHTML, t.modales[0].html]) assert.doesNotMatch(html, /<img/);
});

test('el error de un historial se escapa', async () => {
    const caja = el({ hidden: true });
    const t = montar({ campos: { abonos_7: caja }, falla: { obtenerAbonosTarjeta: 'No se pudo <img src=x onerror=alert(1)>' } });
    await t.vista.alternarAbonos(7);
    assert.doesNotMatch(caja.innerHTML, /<img/);
    assert.match(caja.innerHTML, /&lt;img src=x onerror=alert\(1\)&gt;/);
});
