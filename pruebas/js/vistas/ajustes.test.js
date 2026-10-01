// Pruebas de la vista «Ajustes» en Node, sin navegador.
//
// Catálogos, respaldos y el panel de correcciones, que borra movimientos y por
// eso pregunta dos veces (confirmación y motivo). Con la vista recibiendo sus
// dependencias se le dan dobles de todo; los diálogos responden **tarde**, como
// el WebView real, para fijar que cada acción espera de verdad la respuesta.
// Datos inventados.
//
// Se importa el JavaScript compilado de la vista (`npm test` compila antes).

import test from 'node:test';
import assert from 'node:assert/strict';
import { VistaAjustes, MANEJADORES_AJUSTES, puenteAjustes } from '../../../src/js/vistas/ajustes.js';
import { llamadasDelManejador } from '../ayudas/manejadores_html.js';

const CATEGORIAS = [{ id: 1, nombre: 'Otros' }, { id: 2, nombre: 'Suscripciones' }, { id: 3, nombre: 'Comida' }];
const CLIENTES = [{ id: 4, nombre: 'Cliente Uno', rnc: '101000001' }];
const CUENTAS = [
    { id: 5, nombre: 'Cuenta Alfa', divisa: 'DOP', balance_actual: 750, entidad: 'Banco Alfa', comision_pago_impuestos: 12.5 },
    { id: 6, nombre: "Cuenta O'Brien", divisa: 'USD', balance_actual: 30, entidad: null, comision_pago_impuestos: null },
];
const gasto = id => ({ id, fecha: '10/03/2027', descripcion: `Gasto ${id}`, monto: 100, costo_adicional: 5, divisa: 'DOP', categoria_nombre: 'Comida', metodo_pago: 'efectivo' });
const informal = id => ({ id, fecha: '11/03/2027', descripcion: `Informal ${id}`, monto: 80, estatus: 'pagado', institucion_deposito: 'Banco Alfa' });
const ingreso = id => ({ id, fecha_emision: '12/03/2027', numero_factura: `FAC-${id}`, cliente_nombre: 'Cliente Uno', monto_total: 300, estatus: 'emitida' });
const transaccion = id => ({ id, fecha: '13/03/2027', cuenta_origen_nombre: 'Cuenta Alfa', cuenta_destino_nombre: 'Cuenta Beta', monto_origen: 40, cargo: 2, descripcion: null });

const el = (spec = {}) => ({ value: '', hidden: false, disabled: false, textContent: '', innerHTML: '', ...spec });

const DATOS = () => ({
    categorias: CATEGORIAS, clientes: CLIENTES, cuentas: CUENTAS, gastos: [gasto(1), gasto(2)], informales: [informal(1)],
    ingresos: [ingreso(1)], transacciones: [transaccion(1)],
    respaldos: ['michelitos_2026-09-29T19-03-25_antes-de-instalar.db', 'manual<viejo>.db'],
});

/** Respuesta que llega después, como en el WebView real. */
const tarde = valor => new Promise(resolver => setTimeout(() => resolver(valor), 2));

function montar({ datos = DATOS(), campos = {}, falla = {}, confirmar = true, respuestas = [], motivo = 'Registrado por error en la cuenta equivocada', correcciones = [] } = {}) {
    const llamadas = [], avisos = [], rutas = [], confirmaciones = [], preguntas = [], motivos = [];
    const registrar = (nombre, valor) => async (...a) => {
        llamadas.push([nombre, ...a]);
        if (falla[nombre]) throw new Error(falla[nombre]);
        return valor;
    };
    const api = {
        obtenerCategorias: async () => datos.categorias,
        obtenerClientes: async () => datos.clientes,
        obtenerCuentas: async () => datos.cuentas,
        obtenerGastos: async () => datos.gastos,
        obtenerIngresosInformales: async () => datos.informales,
        obtenerTransaccionesCuentas: async () => datos.transacciones,
        obtenerIngresos: async () => datos.ingresos,
        listarRespaldos: async () => { if (falla.listarRespaldos) throw new Error('sin carpeta'); return datos.respaldos; },
        obtenerCorrecciones: async () => { if (falla.obtenerCorrecciones) throw new Error(falla.obtenerCorrecciones); return correcciones; },
        crearCategoria: registrar('crearCategoria'), eliminarCategoria: registrar('eliminarCategoria'),
        crearCliente: registrar('crearCliente'), eliminarCliente: registrar('eliminarCliente'),
        crearCuenta: registrar('crearCuenta'), actualizarCuenta: registrar('actualizarCuenta'), eliminarCuenta: registrar('eliminarCuenta'),
        crearTarjeta: registrar('crearTarjeta'),
        eliminarGasto: registrar('eliminarGasto', 'C-0001'), eliminarIngresoInformal: registrar('eliminarIngresoInformal'),
        eliminarIngreso: registrar('eliminarIngreso'), eliminarTransaccionCuenta: registrar('eliminarTransaccionCuenta'),
        crearRespaldo: registrar('crearRespaldo', 'respaldos/michelitos_prueba.db'),
        restaurarRespaldo: registrar('restaurarRespaldo', { capital_restaurado: true }),
    };
    const pantalla = { contenido: { innerHTML: '' } };
    const registro = { ...campos };
    const cola = [...respuestas];
    const vista = new VistaAjustes({
        api,
        avisos: { mostrar: (mensaje, tipo) => avisos.push({ mensaje, tipo }) },
        formato: { importe: v => `#${v}#` },
        enrutador: { mostrar: async ruta => { rutas.push(ruta); } },
        pantalla,
        dom: {
            elemento: id => { if (!(id in registro)) throw new Error(`No existe el elemento #${id} en la página.`); return registro[id]; },
            buscar: id => registro[id] ?? null,
        },
        dialogos: { confirmar: m => { confirmaciones.push(m); return tarde(confirmar); }, preguntar: (m, d) => { preguntas.push({ m, d }); return tarde(cola.shift() ?? null); } },
        motivo: { pedir: (titulo, texto) => { motivos.push({ titulo, texto }); return tarde(motivo); } },
    });
    return { vista, llamadas, avisos, rutas, confirmaciones, preguntas, motivos, pantalla, registro, evento: { preventDefault() { this.evitado = (this.evitado || 0) + 1; } } };
}

// --- Lo que se dibuja ---

test('dibuja los catálogos, los respaldos y el panel de correcciones', async () => {
    const t = montar();
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.match(html, /<h1>Ajustes<\/h1>/);
    assert.match(html, /Cliente Uno/);
    assert.match(html, /Cuenta Alfa/);
    assert.match(html, /Banco Alfa/);
    assert.match(html, /id="casos-correccion" hidden/);
    assert.doesNotMatch(html, /undefined|NaN/);
});

test('las categorías del sistema no se pueden borrar; las demás sí', async () => {
    const t = montar();
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.equal([...html.matchAll(/appUI\.handleEliminarCategoria\((\d+)\)/g)].map(m => m[1]).join(), '3');
    assert.equal([...html.matchAll(/>Sistema</g)].length, 2);
});

test('el respaldo se describe legible, y un nombre con HTML se escapa', async () => {
    const t = montar();
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.match(html, /29\/09\/2026 19:03:25 · antes de instalar/);
    assert.match(html, /value="manual&lt;viejo&gt;\.db"/);
    assert.doesNotMatch(html, /<viejo>/);
});

test('sin respaldos lo dice; si no se pueden leer, el resto de Ajustes sigue funcionando', async () => {
    const vacio = montar({ datos: { ...DATOS(), respaldos: [] } });
    await vacio.vista.render();
    assert.match(vacio.pantalla.contenido.innerHTML, /Todavía no hay respaldos/);
    assert.doesNotMatch(vacio.pantalla.contenido.innerHTML, /respaldo_elegido/);
    const roto = montar({ falla: { listarRespaldos: true } });
    await roto.vista.render();
    assert.match(roto.pantalla.contenido.innerHTML, /Todavía no hay respaldos/);
    assert.match(roto.pantalla.contenido.innerHTML, /Cuenta Alfa/);
});

test('el panel de correcciones lista los más recientes: 15 gastos, 10 informales, 10 facturas y 15 transferencias', async () => {
    const muchos = n => Array.from({ length: n }, (_, i) => i + 1);
    const t = montar({ datos: { ...DATOS(), gastos: muchos(20).map(gasto), informales: muchos(15).map(informal), ingresos: muchos(15).map(ingreso), transacciones: muchos(20).map(transaccion) } });
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    const cuenta = nombre => [...html.matchAll(new RegExp(`appUI\\.${nombre}\\(`, 'g'))].length;
    assert.deepEqual([cuenta('handleEliminarGastoCorr'), cuenta('handleEliminarIngresoInformalCorr'), cuenta('handleEliminarIngresoCorr'), cuenta('handleEliminarTransaccionCuentaCorr')], [15, 10, 10, 15]);
});

test('sin movimientos, cada tabla de correcciones lo dice', async () => {
    const t = montar({ datos: { ...DATOS(), gastos: [], informales: [], ingresos: [], transacciones: [] } });
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    for (const texto of ['No hay gastos registrados', 'No hay ingresos informales registrados', 'No hay ingresos formales registrados', 'No hay transferencias de cuenta registradas']) assert.match(html, new RegExp(texto));
});

test('la cuenta lleva sus datos en el botón de editar: el manejador recibe el objeto exacto, aunque el nombre lleve comillas', async () => {
    const t = montar();
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    // Se ejecuta el atributo como lo haría el navegador (deshaciendo las entidades): la cuenta
    // llamada «Cuenta O'Brien» llega íntegra, sin cerrar ninguna cadena del manejador.
    assert.deepEqual(llamadasDelManejador(html, 'abrirEdicionCuenta'), [[CUENTAS[0]], [CUENTAS[1]]]);
    assert.match(html, /Tarifa fija por pagar impuestos/);
});

test('un nombre de cuenta con comillas, barras y marcado llega íntegro al manejador y no se interpreta', async () => {
    const hostil = { ...CUENTAS[0], nombre: `x'); alert(1); //"\\ <img src=x onerror="alert(2)"> &amp;` };
    const t = montar({ datos: { ...DATOS(), cuentas: [hostil] } });
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.deepEqual(llamadasDelManejador(html, 'abrirEdicionCuenta'), [[hostil]]);
    assert.doesNotMatch(html, /<img/);
    assert.match(html, /&lt;img src=x onerror=&quot;alert\(2\)&quot;&gt; &amp;amp;/);
});

// --- Altas ---

test('alta de categoría y de cliente: envían lo escrito y redibujan', async () => {
    const t = montar({ campos: { cat_nom: el({ value: 'Salud' }), cli_aj_nom: el({ value: 'Cliente Dos' }), cli_aj_rnc: el({ value: '101000002' }) } });
    await t.vista.handleAgregarCategoria(t.evento);
    await t.vista.handleAgregarCliente(t.evento);
    assert.equal(t.evento.evitado, 2);
    assert.deepEqual(t.llamadas, [['crearCategoria', 'Salud'], ['crearCliente', '101000002', 'Cliente Dos']]);   // el RNC va primero
    assert.deepEqual(t.avisos.map(a => a.mensaje), ['Categoría agregada.', 'Cliente registrado.']);
    assert.deepEqual(t.rutas, ['ajustes', 'ajustes']);
});

const CAMPOS_CUENTA = (extra = {}) => ({ cue_aj_nom: el({ value: 'Cuenta Nueva' }), cue_aj_div: el({ value: 'USD' }), cue_aj_bal: el({ value: '25.5' }), cue_aj_ent: el({ value: 'Banco Beta' }), cue_aj_com: el({ value: '' }), ...extra });

test('alta de cuenta: el balance como número, la comisión como texto y en blanco como «no declarada»', async () => {
    const t = montar({ campos: CAMPOS_CUENTA() });
    await t.vista.handleAgregarCuenta(t.evento);
    assert.deepEqual(t.llamadas, [['crearCuenta', 'Cuenta Nueva', 'USD', '25.5', 'Banco Beta', null]]);
    const u = montar({ campos: CAMPOS_CUENTA({ cue_aj_com: el({ value: ' 3.456 ' }) }) });
    await u.vista.handleAgregarCuenta(u.evento);
    assert.equal(u.llamadas[0][5], '3.456');          // tal cual, recortada: el céntimo lo decide el núcleo
    assert.equal(typeof u.llamadas[0][5], 'string');
});

test('alta de cuenta: el saldo inicial viaja como TEXTO, recortado y con los dígitos intactos', async () => {
    for (const [escrito, esperado] of [[' 25.5 ', '25.5'], ['100.005', '100.005'], ['0025.500', '0025.500'], ['0', '0']]) {
        const t = montar({ campos: CAMPOS_CUENTA({ cue_aj_bal: el({ value: escrito }) }) });
        await t.vista.handleAgregarCuenta(t.evento);
        assert.strictEqual(t.llamadas[0][3], esperado, JSON.stringify(escrito));
        assert.equal(typeof t.llamadas[0][3], 'string');
    }
});

test('alta de tarjeta: los doce valores, en el orden que espera la API, y redibuja Tarjetas', async () => {
    const campos = {
        tar_ent: el({ value: 'Banco Beta' }), tar_nom: el({ value: 'Oro' }),
        tar_lim_dop: el({ value: '1' }), tar_sob_dop: el({ value: '2' }), tar_bal_dop: el({ value: '3' }), tar_cor_dop: el({ value: '4' }),
        tar_lim_usd: el({ value: '5' }), tar_sob_usd: el({ value: '6' }), tar_bal_usd: el({ value: '7' }), tar_cor_usd: el({ value: '8' }),
        tar_cor: el({ value: '9' }), tar_pag: el({ value: '10' }),
    };
    const t = montar({ campos });
    await t.vista.handleAgregarTarjeta(t.evento);
    // (entidad, nombre, límite DOP, límite USD, sobregiro DOP, sobregiro USD, balance DOP, balance USD, corte DOP, corte USD, día de corte, día de pago)
    assert.deepEqual(t.llamadas, [['crearTarjeta', 'Banco Beta', 'Oro', 1, 5, 2, 6, 3, 7, 4, 8, 9, 10]]);
    assert.deepEqual(t.rutas, ['tarjetas']);
});

test('si Rust rechaza un alta se avisa el error y no se redibuja', async () => {
    const t = montar({ campos: CAMPOS_CUENTA(), falla: { crearCuenta: 'Rechazada por prueba' } });
    await t.vista.handleAgregarCuenta(t.evento);
    assert.equal(t.avisos[0].tipo, 'error');
    assert.match(t.avisos[0].mensaje, /Rechazada por prueba/);
    assert.deepEqual(t.rutas, []);
});

// --- Bajas con confirmación ---

test('las bajas de categoría, cliente y cuenta esperan la confirmación; Cancelar no borra', async () => {
    for (const [metodo, api, id] of [['handleEliminarCategoria', 'eliminarCategoria', 3], ['handleEliminarCliente', 'eliminarCliente', 4], ['handleEliminarCuenta', 'eliminarCuenta', 5]]) {
        const t = montar();
        const espera = t.vista[metodo](id);
        assert.deepEqual(t.llamadas, [], `${metodo}: no debe actuar antes de la respuesta`);
        await espera;
        assert.deepEqual(t.llamadas, [[api, id]]);
        assert.deepEqual(t.rutas, ['ajustes']);
        const no = montar({ confirmar: false });
        await no.vista[metodo](id);
        assert.deepEqual([no.llamadas, no.avisos, no.rutas], [[], [], []], metodo);
    }
});

test('si Rust rechaza una baja se avisa el error', async () => {
    const t = montar({ falla: { eliminarCuenta: 'No' } });
    await t.vista.handleEliminarCuenta(5);
    assert.equal(t.avisos[0].tipo, 'error');
    assert.deepEqual(t.rutas, []);
});

// --- Editar una cuenta (tres preguntas) ---

test('editar cuenta: pregunta nombre, entidad y comisión con lo que hay, y envía la comisión como texto', async () => {
    const t = montar({ respuestas: ['Cuenta Renombrada', 'Banco Gamma', ' 7.25 '] });
    await t.vista.abrirEdicionCuenta(CUENTAS[0]);
    assert.deepEqual(t.preguntas.map(p => p.d), ['Cuenta Alfa', 'Banco Alfa', '12.5']);
    assert.match(t.preguntas[1].m, /«Cuenta Renombrada»/);
    assert.deepEqual(t.llamadas, [['actualizarCuenta', 5, 'Cuenta Renombrada', 'Banco Gamma', '7.25']]);
    assert.deepEqual(t.rutas, ['ajustes']);
});

test('editar cuenta: sin entidad ni comisión parte de cadenas vacías, y la comisión vacía es «no declarada»', async () => {
    const t = montar({ respuestas: ['Cuenta O Brien', '', '  '] });
    await t.vista.abrirEdicionCuenta(CUENTAS[1]);
    assert.deepEqual(t.preguntas.map(p => p.d), ["Cuenta O'Brien", '', '']);
    assert.deepEqual(t.llamadas, [['actualizarCuenta', 6, 'Cuenta O Brien', '', null]]);
});

test('editar cuenta: cancelar en cualquiera de las tres preguntas no envía nada', async () => {
    for (const respuestas of [[null], ['Nombre', null], ['Nombre', 'Entidad', null]]) {
        const t = montar({ respuestas });
        await t.vista.abrirEdicionCuenta(CUENTAS[0]);
        assert.deepEqual([t.llamadas, t.avisos], [[], []], JSON.stringify(respuestas));
    }
});

test('editar cuenta: una comisión que no es un importe o es negativa se rechaza con aviso', async () => {
    for (const [mala, texto] of [['abc', /importe válido/], ['1.2.3', /importe válido/], ['-5', /no puede ser negativa/]]) {
        const t = montar({ respuestas: ['N', 'E', mala] });
        await t.vista.abrirEdicionCuenta(CUENTAS[0]);
        assert.deepEqual(t.llamadas, [], mala);
        assert.equal(t.avisos[0].tipo, 'error', mala);
        assert.match(t.avisos[0].mensaje, texto, mala);
    }
});

// --- Casos de corrección (panel) ---

test('el panel de casos: abre mostrando «Cargando» y luego la lista; al pulsar otra vez, se oculta', async () => {
    const caja = el({ hidden: true });
    const t = montar({ campos: { 'casos-correccion': caja }, correcciones: [{ numero_caso: 'C-0007', fecha: '14/03/2027', tipo: 'gasto', descripcion: 'Comida', importe: 105, divisa: 'DOP', motivo: 'Registrado por error' }] });
    const espera = t.vista.alternarCasosDeCorreccion();
    assert.equal(caja.hidden, false);
    assert.match(caja.innerHTML, /Cargando/);
    await espera;
    assert.match(caja.innerHTML, /C-0007/);
    assert.match(caja.innerHTML, /DOP #105#/);
    assert.match(caja.innerHTML, /Registrado por error/);
    await t.vista.alternarCasosDeCorreccion();
    assert.equal(caja.hidden, true);
});

test('el panel de casos: sin casos lo dice, y un fallo se muestra en el panel sin romper nada', async () => {
    const vacio = el({ hidden: true });
    await montar({ campos: { 'casos-correccion': vacio } }).vista.alternarCasosDeCorreccion();
    assert.match(vacio.innerHTML, /Ningún caso abierto/);
    const roto = el({ hidden: true });
    await montar({ campos: { 'casos-correccion': roto }, falla: { obtenerCorrecciones: 'Sin acceso' } }).vista.alternarCasosDeCorreccion();
    assert.match(roto.innerHTML, /Sin acceso/);
});

// El motivo (y la descripción) los escribe el titular y se guardan tal cual: si llegaran al
// HTML sin escapar, un `<` en un motivo se interpretaría como marcado, y un `<img onerror=…>`
// ejecutaría código en el WebView de la aplicación (que tiene acceso a todos los comandos).
const CASO_HOSTIL = (extra = {}) => ({
    numero_caso: 'C-<0007>', fecha: '14/03/<2027>', tipo: 'gasto<script>', descripcion: 'Cena <img src=x onerror="alert(1)"> y más',
    importe: 105, divisa: 'D<O>P', motivo: 'Lo cobraron "dos" veces & <b>nadie</b> avisó', ...extra,
});

test('el panel de casos escapa lo que escribe el titular: el marcado no se interpreta', async () => {
    const caja = el({ hidden: true });
    const t = montar({ campos: { 'casos-correccion': caja }, correcciones: [CASO_HOSTIL()] });
    await t.vista.alternarCasosDeCorreccion();
    const html = caja.innerHTML;
    assert.doesNotMatch(html, /<img|<script|<b>nadie|<0007>|<O>|<2027>/, 'ningún texto del titular se coló como marcado');
    assert.match(html, /Cena &lt;img src=x onerror=&quot;alert\(1\)&quot;&gt; y más/);
    assert.match(html, /Lo cobraron &quot;dos&quot; veces &amp; &lt;b&gt;nadie&lt;\/b&gt; avisó/);
    assert.match(html, /C-&lt;0007&gt;/);
    assert.match(html, /14\/03\/&lt;2027&gt; · gasto&lt;script&gt;/);
    assert.match(html, /D&lt;O&gt;P #105#/);
});

test('el panel de casos sigue mostrando igual el texto corriente: tildes, comas y ampersand se ven como se escribieron', async () => {
    const caja = el({ hidden: true });
    const t = montar({ campos: { 'casos-correccion': caja }, correcciones: [CASO_HOSTIL({ numero_caso: 'C-0008', fecha: '14/03/2027', tipo: 'ingreso', descripcion: 'Tom & Jerry, S.A. — año nuevo', divisa: 'USD', motivo: 'Corrección por el día 5: «error» de captura' })] });
    await t.vista.alternarCasosDeCorreccion();
    assert.match(caja.innerHTML, /Tom &amp; Jerry, S\.A\. — año nuevo — USD #105#/);
    assert.match(caja.innerHTML, /Corrección por el día 5: «error» de captura/);
});

test('el error del panel también se escapa: el mensaje de Rust puede traer texto del titular', async () => {
    const caja = el({ hidden: true });
    const t = montar({ campos: { 'casos-correccion': caja }, falla: { obtenerCorrecciones: 'No se encontró <b>Cliente "X"</b>' } });
    await t.vista.alternarCasosDeCorreccion();
    assert.doesNotMatch(caja.innerHTML, /<b>/);
    assert.match(caja.innerHTML, /No se encontró &lt;b&gt;Cliente &quot;X&quot;&lt;\/b&gt;/);
});

test('el panel de casos: si la página no tiene el panel, no hace nada', async () => {
    await assert.doesNotReject(() => montar().vista.alternarCasosDeCorreccion());
});

// --- Correcciones: confirmación y motivo ---

const CORRECCIONES = [
    ['handleEliminarGastoCorr', 'eliminarGasto', 7, /Vas a borrar este gasto/],
    ['handleEliminarIngresoInformalCorr', 'eliminarIngresoInformal', 8, /ingreso informal/],
    ['handleEliminarIngresoCorr', 'eliminarIngreso', 9, /esta factura/],
    ['handleEliminarTransaccionCuentaCorr', 'eliminarTransaccionCuenta', 10, /traspaso entre cuentas/],
];

test('cada corrección pide confirmación y luego el motivo, y solo entonces envía id y motivo', async () => {
    const MOTIVO = 'Registrado dos veces por error';
    for (const [metodo, api, id, titulo] of CORRECCIONES) {
        const t = montar({ motivo: MOTIVO });
        const espera = t.vista[metodo](id);
        assert.deepEqual(t.llamadas, [], `${metodo}: no debe actuar antes de las respuestas`);
        await espera;
        assert.deepEqual(t.llamadas, [[api, id, MOTIVO]], metodo);
        assert.match(t.motivos[0].titulo, titulo, metodo);
        assert.match(t.motivos[0].texto, /destruye el movimiento/, metodo);
        assert.deepEqual(t.rutas, ['ajustes'], metodo);
        assert.equal(t.avisos.at(-1).tipo, undefined, metodo);
    }
});

test('el aviso de borrar un gasto trae el número de caso', async () => {
    const t = montar();
    await t.vista.handleEliminarGastoCorr(7);
    assert.match(t.avisos[0].mensaje, /Caso C-0001/);
});

test('sin confirmar, o con el motivo cancelado, no se borra nada (y sin confirmar ni se pide el motivo)', async () => {
    for (const [metodo] of CORRECCIONES) {
        const sinConfirmar = montar({ confirmar: false });
        await sinConfirmar.vista[metodo](7);
        assert.deepEqual([sinConfirmar.llamadas, sinConfirmar.motivos.length, sinConfirmar.rutas], [[], 0, []], metodo);
        const sinMotivo = montar({ motivo: null });
        await sinMotivo.vista[metodo](7);
        assert.deepEqual([sinMotivo.llamadas, sinMotivo.rutas], [[], []], metodo);
    }
});

test('si Rust rechaza una corrección se avisa el error y no se redibuja', async () => {
    for (const [metodo, api, id] of CORRECCIONES) {
        const t = montar({ falla: { [api]: 'Rechazada' } });
        await t.vista[metodo](id);
        assert.equal(t.avisos.at(-1).tipo, 'error', metodo);
        assert.deepEqual(t.rutas, [], metodo);
    }
});

// --- Respaldos ---

test('respaldar: deshabilita el botón mientras corre, muestra la ruta y lo deja como estaba', async () => {
    const salida = el();
    const boton = { disabled: false, textContent: 'Respaldar ahora' };
    let durante;
    const t = montar({ campos: { respaldo_resultado: salida } });
    const original = t.vista.dep.api.crearRespaldo;
    t.vista.dep.api.crearRespaldo = async () => { durante = { ...boton }; return original(); };
    await t.vista.handleCrearRespaldo(boton);
    assert.deepEqual(durante, { disabled: true, textContent: 'Respaldando…' });
    assert.deepEqual(boton, { disabled: false, textContent: 'Respaldar ahora' });
    assert.equal(salida.textContent, 'respaldos/michelitos_prueba.db');
    assert.equal(t.avisos[0].mensaje, 'Respaldo creado y verificado.');
});

test('respaldar: si falla muestra el error, borra la ruta anterior y rehabilita el botón', async () => {
    const salida = el({ textContent: 'ruta vieja' });
    const boton = { disabled: false, textContent: 'Respaldar ahora' };
    const t = montar({ campos: { respaldo_resultado: salida }, falla: { crearRespaldo: 'Disco lleno' } });
    await t.vista.handleCrearRespaldo(boton);
    assert.equal(t.avisos[0].tipo, 'error');
    assert.equal(salida.textContent, '');
    assert.deepEqual(boton, { disabled: false, textContent: 'Respaldar ahora' });
});

const CAMPOS_RESTAURAR = () => ({
    respaldo_elegido: el({ value: 'michelitos_2026-09-29T19-03-25_antes-de-instalar.db', selectedIndex: 0, options: [{ textContent: '29/09/2026 19:03:25 · antes de instalar' }] }),
    restauracion_resultado: el(),
});

test('restaurar: confirma con la etiqueta del respaldo y la advertencia, y solo entonces restaura', async () => {
    const boton = { disabled: false, textContent: 'Restaurar este respaldo' };
    const t = montar({ campos: CAMPOS_RESTAURAR() });
    const espera = t.vista.handleRestaurarRespaldo(boton);
    assert.deepEqual(t.llamadas, []);
    await espera;
    assert.match(t.confirmaciones[0], /29\/09\/2026 19:03:25 · antes de instalar/);
    assert.match(t.confirmaciones[0], /Antes se guardará una copia/);
    assert.deepEqual(t.llamadas, [['restaurarRespaldo', 'michelitos_2026-09-29T19-03-25_antes-de-instalar.db']]);
    assert.match(t.registro.restauracion_resultado.textContent, /Restaurado: 29\/09\/2026.*El capital también/);
    assert.deepEqual(t.rutas, ['ajustes']);
});

test('restaurar: un respaldo sin capital lo dice; sin elegir, o sin confirmar, no hace nada', async () => {
    const t = montar({ campos: CAMPOS_RESTAURAR() });
    t.vista.dep.api.restaurarRespaldo = async () => ({ capital_restaurado: false });
    await t.vista.handleRestaurarRespaldo({ disabled: false, textContent: 'x' });
    assert.match(t.registro.restauracion_resultado.textContent, /no traía capital: se conservó el actual/);
    const sinElegir = montar({ campos: { respaldo_elegido: el({ value: '' }) } });
    await sinElegir.vista.handleRestaurarRespaldo({ disabled: false, textContent: 'x' });
    assert.deepEqual([sinElegir.llamadas, sinElegir.confirmaciones], [[], []]);
    const sinPagina = montar();
    await sinPagina.vista.handleRestaurarRespaldo({ disabled: false, textContent: 'x' });
    assert.deepEqual(sinPagina.llamadas, []);
    const no = montar({ campos: CAMPOS_RESTAURAR(), confirmar: false });
    const boton = { disabled: false, textContent: 'Restaurar' };
    await no.vista.handleRestaurarRespaldo(boton);
    assert.deepEqual([no.llamadas, boton.disabled], [[], false]);
});

test('restaurar: si falla muestra el error y deja el botón como estaba', async () => {
    const boton = { disabled: false, textContent: 'Restaurar este respaldo' };
    const t = montar({ campos: CAMPOS_RESTAURAR(), falla: { restaurarRespaldo: 'Respaldo dañado' } });
    await t.vista.handleRestaurarRespaldo(boton);
    assert.equal(t.avisos[0].tipo, 'error');
    assert.deepEqual(boton, { disabled: false, textContent: 'Restaurar este respaldo' });
    assert.deepEqual(t.rutas, []);
});

// --- El puente ---

test('el puente expone todos los manejadores y delega en la vista', async () => {
    const t = montar({ campos: { cat_nom: el({ value: 'Salud' }) } });
    const puente = puenteAjustes(t.vista);
    assert.deepEqual(Object.keys(puente).sort(), [...MANEJADORES_AJUSTES].sort());
    await puente.handleAgregarCategoria(t.evento);
    assert.deepEqual(t.llamadas, [['crearCategoria', 'Salud']]);
});

test('todo appUI.x( que escribe la plantilla está en el puente, y todo el puente está en la plantilla', async () => {
    const t = montar();
    await t.vista.render();
    const llamados = new Set([...t.pantalla.contenido.innerHTML.matchAll(/appUI\.(\w+)\(/g)].map(m => m[1]));
    for (const nombre of llamados) assert.ok(MANEJADORES_AJUSTES.includes(nombre), `falta ${nombre} en el puente`);
    for (const nombre of MANEJADORES_AJUSTES) assert.ok(llamados.has(nombre), `${nombre} está en el puente pero la plantilla no lo usa`);
});
