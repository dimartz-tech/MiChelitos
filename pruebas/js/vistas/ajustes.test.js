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
import { llamadasDelManejador, nombresDelPuente } from '../ayudas/manejadores_html.js';

const MOTIVO_OTROS = 'asentar las comisiones de los abonos a tarjeta y el cargo de los avances de efectivo';
const CATEGORIAS = [
    { id: 1, nombre: 'Otros', fija: false, motivo_sistema: MOTIVO_OTROS },
    { id: 2, nombre: 'Suscripciones', fija: true, motivo_sistema: 'asentar los cargos recurrentes automáticos de las suscripciones' },
    { id: 3, nombre: 'Comida', fija: false, motivo_sistema: null },
    { id: 4, nombre: 'Renta', fija: false, motivo_sistema: null },
    { id: 5, nombre: 'Impuestos', fija: false, motivo_sistema: 'reconocer los pagos de impuestos' },
];
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
    cuentasArchivadas: [{ id: 9, nombre: 'Cuenta <Vieja>', divisa: 'DOP', balance_actual: 0, entidad: 'Banco "Sin"', comision_pago_impuestos: null }],
    respaldos: ['michelitos_2026-09-29T19-03-25_antes-de-instalar.db', 'manual<viejo>.db'],
});

/** Respuesta que llega después, como en el WebView real. */
const tarde = valor => new Promise(resolver => setTimeout(() => resolver(valor), 2));

const previaDe = (origenId, destinoId, extra = {}) => {
    const nombre = id => CATEGORIAS.find(c => c.id === id)?.nombre ?? `Categoría ${id}`;
    return { origen_id: origenId, origen_nombre: nombre(origenId), destino_id: destinoId, destino_nombre: nombre(destinoId), gastos: 3, desde: '21/07/2026', hasta: '03/09/2026', advertencias: [], origen_fija: false, destino_fija: false, ...extra };
};

function montar({ datos = DATOS(), campos = {}, falla = {}, confirmar = true, respuestas = [], motivo = 'Registrado por error en la cuenta equivocada', correcciones = [], previa = previaDe, movidos = 3 } = {}) {
    const llamadas = [], avisos = [], rutas = [], confirmaciones = [], preguntas = [], motivos = [], modales = [];
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
        renombrarCategoria: async (id, nombre) => {
            llamadas.push(['renombrarCategoria', id, nombre]);
            if (falla.renombrarCategoria) throw new Error(falla.renombrarCategoria);
            return { id, nombre: nombre.trim(), motivo_sistema: null };
        },
        previsualizarFusionCategorias: async (o, d) => {
            llamadas.push(['previsualizarFusionCategorias', o, d]);
            if (falla.previsualizarFusionCategorias) throw new Error(falla.previsualizarFusionCategorias);
            return typeof previa === 'function' ? previa(o, d) : previa;
        },
        fusionarCategorias: async (o, d) => {
            llamadas.push(['fusionarCategorias', o, d]);
            if (falla.fusionarCategorias) throw new Error(falla.fusionarCategorias);
            return movidos;
        },
        marcarCategoriaFija: async (id, fija) => {
            llamadas.push(['marcarCategoriaFija', id, fija]);
            if (falla.marcarCategoriaFija) throw new Error(falla.marcarCategoriaFija);
            return { id, nombre: datos.categorias.find(c => c.id === id)?.nombre ?? '?', fija };
        },
        crearCliente: registrar('crearCliente'), eliminarCliente: registrar('eliminarCliente'),
        crearCuenta: registrar('crearCuenta'), actualizarCuenta: registrar('actualizarCuenta'), archivarCuenta: registrar('archivarCuenta'), reactivarCuenta: registrar('reactivarCuenta'),
        obtenerCuentasArchivadas: async () => datos.cuentasArchivadas ?? [],
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
        modales: { abrir: (id, html) => { modales.push({ id, html }); } },
    });
    return { vista, llamadas, avisos, rutas, confirmaciones, preguntas, motivos, modales, pantalla, registro, evento: { preventDefault() { this.evitado = (this.evitado || 0) + 1; } } };
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

test('«Otros» y «Suscripciones» no se pueden borrar; las demás sí (como siempre)', async () => {
    const t = montar();
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.equal(llamadasDelManejador(html, 'handleEliminarCategoria').map(a => a[0]).join(), '3,4,5');
});

test('las del sistema se marcan con icono y texto, explican por qué, y no ofrecen renombrar ni fusionar', async () => {
    const t = montar();
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.equal([...html.matchAll(/🔒 Sistema</g)].length, 3, 'Otros, Suscripciones e Impuestos');
    assert.match(html, new RegExp(`El sistema la usa para ${MOTIVO_OTROS}\\. Por eso no se renombra ni se fusiona en otra`));
    // Solo las corrientes (3 «Comida» y 4 «Renta») llevan los botones.
    assert.deepEqual(llamadasDelManejador(html, 'handleRenombrarCategoria'), [[3, 'Comida'], [4, 'Renta']]);
    assert.deepEqual(llamadasDelManejador(html, 'handleFusionarCategoria'), [[3], [4]]);
    assert.match(html, /✏️ Renombrar/);
    assert.match(html, /🔀 Fusionar…/);
});

test('un nombre de categoría hostil se escapa en la lista y llega íntegro a los manejadores', async () => {
    const hostil = `x'); alert(1); //"\\ <img src=x onerror="alert(2)"> &amp;`;
    const t = montar({ datos: { ...DATOS(), categorias: [...CATEGORIAS, { id: 9, nombre: hostil, motivo_sistema: null }] } });
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.deepEqual(llamadasDelManejador(html, 'handleRenombrarCategoria').at(-1), [9, hostil]);
    assert.doesNotMatch(html, /<img/);
    assert.match(html, /&lt;img src=x onerror=&quot;alert\(2\)&quot;&gt; &amp;amp;/);
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
    const cuenta = nombre => llamadasDelManejador(html, nombre).length;
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

test('alta de tarjeta: los importes viajan como texto recortado y uno en blanco se envía como «0»', async () => {
    const campos = {
        tar_ent: el({ value: 'B' }), tar_nom: el({ value: 'T' }),
        tar_lim_dop: el({ value: ' 1000.005 ' }), tar_sob_dop: el({ value: '' }), tar_bal_dop: el({ value: '0075.250' }), tar_cor_dop: el({ value: '4' }),
        tar_lim_usd: el({ value: '5' }), tar_sob_usd: el({ value: '6' }), tar_bal_usd: el({ value: '7' }), tar_cor_usd: el({ value: '8' }),
        tar_cor: el({ value: '9' }), tar_pag: el({ value: '10' }),
    };
    const t = montar({ campos });
    await t.vista.handleAgregarTarjeta(t.evento);
    assert.deepEqual(t.llamadas, [['crearTarjeta', 'B', 'T', '1000.005', '5', '0', '6', '0075.250', '7', '4', '8', 9, 10]]);
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
    assert.deepEqual(t.llamadas, [['crearTarjeta', 'Banco Beta', 'Oro', '1', '5', '2', '6', '3', '7', '4', '8', 9, 10]]);
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

test('las bajas de categoría y cliente y el archivo de una cuenta esperan la confirmación; Cancelar no cambia nada', async () => {
    for (const [metodo, api, id] of [['handleEliminarCategoria', 'eliminarCategoria', 3], ['handleEliminarCliente', 'eliminarCliente', 4], ['handleArchivarCuenta', 'archivarCuenta', 5]]) {
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
    const t = montar({ falla: { archivarCuenta: 'No' } });
    await t.vista.handleArchivarCuenta(5);
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

// --- Casos de corrección: detalle de un traspaso borrado ---

const DETALLE = (extra = {}) => JSON.stringify({
    version: 1, traspaso_id: 7, fecha: '07/10/2026', descripcion: 'Compra de dólares',
    origen: { cuenta_id: 5, cuenta_nombre: 'Ahorros DOP', importe: { monto: '5875.37', divisa: 'DOP' } },
    destino: { cuenta_id: 6, cuenta_nombre: 'Dólares', importe: { monto: '100.00', divisa: 'USD' } },
    tasa: '0.017021', cargo: { monto: '25.55', divisa: 'DOP' }, ...extra,
});
const CASO_TRASPASO = (extra = {}) => ({
    numero_caso: 'COR-2026-0003', fecha: '03/11/2026', tipo: 'traspaso', referencia_id: 7, descripcion: 'Compra de dólares',
    importe: 5875.37, divisa: 'DOP', motivo: 'Traspaso registrado en la cuenta equivocada', detalle: DETALLE(), ...extra,
});

test('el panel de casos muestra el detalle de un traspaso: fecha, las dos cuentas con su importe y divisa, tasa y cargo', async () => {
    const caja = el({ hidden: true });
    const t = montar({ campos: { 'casos-correccion': caja }, correcciones: [CASO_TRASPASO()] });
    await t.vista.alternarCasosDeCorreccion();
    const html = caja.innerHTML;
    assert.match(html, /07\/10\/2026: Ahorros DOP \(5875\.37 DOP\) → Dólares \(100\.00 USD\)/);
    assert.match(html, /tasa 0\.017021/);
    assert.match(html, /cargo 25\.55 DOP/);
    assert.match(html, /«Compra de dólares»/);
    assert.doesNotMatch(html, /Sin detalle/);
});

test('un traspaso sin tasa ni descripción omite esas partes', async () => {
    const caja = el({ hidden: true });
    const t = montar({ campos: { 'casos-correccion': caja }, correcciones: [CASO_TRASPASO({ detalle: DETALLE({ tasa: null, descripcion: null }) })] });
    await t.vista.alternarCasosDeCorreccion();
    assert.doesNotMatch(caja.innerHTML, /tasa|«/);
    assert.match(caja.innerHTML, /cargo 25\.55 DOP/);
});

test('un caso de traspaso anterior (detalle null) dice «Sin detalle (caso anterior)»; un JSON ilegible, «ilegible»', async () => {
    const caja = el({ hidden: true });
    await montar({ campos: { 'casos-correccion': caja }, correcciones: [CASO_TRASPASO({ detalle: null })] }).vista.alternarCasosDeCorreccion();
    assert.match(caja.innerHTML, /Sin detalle \(caso anterior\)/);
    const rota = el({ hidden: true });
    await montar({ campos: { 'casos-correccion': rota }, correcciones: [CASO_TRASPASO({ detalle: '{mal' })] }).vista.alternarCasosDeCorreccion();
    assert.match(rota.innerHTML, /Sin detalle \(ilegible\)/);
});

test('los casos que no son traspasos no muestran línea de detalle', async () => {
    const caja = el({ hidden: true });
    await montar({ campos: { 'casos-correccion': caja }, correcciones: [CASO_TRASPASO({ tipo: 'gasto', detalle: null })] }).vista.alternarCasosDeCorreccion();
    assert.doesNotMatch(caja.innerHTML, /caso-detalle|Sin detalle/);
});

test('el detalle escapa nombres de cuenta, descripción, divisa y tasa: nada se interpreta como marcado', async () => {
    const caja = el({ hidden: true });
    const hostil = DETALLE({
        descripcion: '<img src=x onerror="a">', tasa: '<i>1</i>',
        origen: { cuenta_id: 5, cuenta_nombre: `<b>O'"&`, importe: { monto: '<1>', divisa: '<D>' } },
    });
    await montar({ campos: { 'casos-correccion': caja }, correcciones: [CASO_TRASPASO({ detalle: hostil })] }).vista.alternarCasosDeCorreccion();
    const html = caja.innerHTML;
    assert.doesNotMatch(html, /<img|<b>|<i>|<1>|<D>/);
    assert.match(html, /&lt;b&gt;O&#39;&quot;&amp;/);
    assert.match(html, /«&lt;img src=x onerror=&quot;a&quot;&gt;»/);
    assert.doesNotMatch(html, /\bon\w+\s*=\s*"[^&]/, 'sin atributos on* en el detalle');
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
    await t.vista.handleFusionarCategoria(4);   // la ventana de fusión también escribe manejadores
    const llamados = nombresDelPuente(t.pantalla.contenido.innerHTML + t.modales[0].html);
    for (const nombre of llamados) assert.ok(MANEJADORES_AJUSTES.includes(nombre), `falta ${nombre} en el puente`);
    for (const nombre of MANEJADORES_AJUSTES) assert.ok(llamados.has(nombre), `${nombre} está en el puente pero la plantilla no lo usa`);
});

// --- Renombrar una categoría ---

test('renombrar: pregunta con el nombre actual, envía lo escrito, avisa y redibuja', async () => {
    const t = montar({ respuestas: ['  Alimentos  '] });
    await t.vista.handleRenombrarCategoria(3, 'Comida');
    assert.equal(t.preguntas.length, 1);
    assert.match(t.preguntas[0].m, /Nuevo nombre para «Comida»/);
    assert.equal(t.preguntas[0].d, 'Comida', 'el nombre actual es la respuesta por defecto');
    assert.deepEqual(t.llamadas, [['renombrarCategoria', 3, '  Alimentos  ']], 'el recorte lo hace Rust: la vista no toca el texto');
    assert.deepEqual(t.avisos, [{ mensaje: 'Categoría renombrada.', tipo: undefined }]);
    assert.deepEqual(t.rutas, ['ajustes']);
});

test('renombrar: cancelar el diálogo no envía nada', async () => {
    const t = montar({ respuestas: [null] });
    await t.vista.handleRenombrarCategoria(4, 'Renta');
    assert.deepEqual(t.llamadas, []);
    assert.deepEqual(t.avisos, []);
    assert.deepEqual(t.rutas, []);
});

test('renombrar: si Rust rechaza (duplicada, reservada, del sistema…) muestra el motivo y no redibuja', async () => {
    const t = montar({ respuestas: ['alimentacion'], falla: { renombrarCategoria: 'La categoría ya existe.' } });
    await t.vista.handleRenombrarCategoria(4, 'Renta');
    assert.equal(t.avisos.length, 1);
    assert.equal(t.avisos[0].tipo, 'error');
    assert.match(t.avisos[0].mensaje, /La categoría ya existe\./);
    assert.deepEqual(t.rutas, []);
});

test('renombrar: advierte si el cambio mueve la categoría dentro o fuera del conmutador «Incluir inversión y obligaciones»', async () => {
    const entra = montar({ respuestas: ['Prestamo'] });
    await entra.vista.handleRenombrarCategoria(3, 'Comida');
    assert.match(entra.avisos[0].mensaje, /Atención: en el Resumen, «Prestamo» ahora se excluye cuando «Incluir inversión y obligaciones» está apagado/);

    const sale = montar({ respuestas: ['Vivienda'] });
    await sale.vista.handleRenombrarCategoria(4, 'Renta');
    assert.match(sale.avisos[0].mensaje, /«Vivienda» ahora se cuenta/);

    const igual = montar({ respuestas: ['alquiler'] });
    await igual.vista.handleRenombrarCategoria(4, 'Renta');   // Renta y Alquiler están las dos en la regla: nada cambia
    assert.equal(igual.avisos[0].mensaje, 'Categoría renombrada.');
});

// --- Fusionar: la ventana ---

test('fusionar abre una ventana accesible con selector de destino, vista previa vacía y «Confirmar» deshabilitado', async () => {
    const t = montar();
    await t.vista.handleFusionarCategoria(4);
    assert.equal(t.modales.length, 1);
    const { id, html } = t.modales[0];
    assert.equal(id, 'modal-fusion-categoria');
    assert.match(html, /role="dialog" aria-modal="true" aria-labelledby="fusion_titulo"/);
    assert.match(html, /id="fusion_titulo" tabindex="-1"[^>]*>🔀 Fusionar «Renta» en otra categoría/);
    assert.match(html, /<label for="fusion_destino">/);
    assert.match(html, /id="fusion_vista_previa" role="status" aria-live="polite"/);
    assert.match(html, /id="fusion_confirmar"[^>]* disabled>Confirmar fusión/);
    const destinos = [...html.matchAll(/<option value="(\d*)">([^<]*)</g)].map(m => [m[1], m[2]]);
    assert.deepEqual(destinos, [['', 'Elige la categoría de destino…'], ['1', 'Otros (sistema)'], ['2', 'Suscripciones (sistema)'], ['3', 'Comida'], ['5', 'Impuestos (sistema)']], 'no se ofrece a sí misma; las del sistema sí, rotuladas');
    assert.deepEqual(llamadasDelManejador(html, 'cambiarDestinoFusion'), [['VALOR', 4]]);
    assert.deepEqual(llamadasDelManejador(html, 'handleConfirmarFusion'), [[4]]);
    assert.deepEqual(llamadasDelManejador(html, 'quitar'), [['modal-fusion-categoria']]);
});

test('fusionar: nombres hostiles de origen y destino se escapan en la ventana', async () => {
    const hostil = `<img src=x onerror="alert(1)"> '&`;
    const t = montar({ datos: { ...DATOS(), categorias: [...CATEGORIAS, { id: 9, nombre: hostil, motivo_sistema: null }] } });
    await t.vista.handleFusionarCategoria(9);
    assert.doesNotMatch(t.modales[0].html, /<img/);
    assert.match(t.modales[0].html, /&lt;img src=x onerror=&quot;alert\(1\)&quot;&gt; &#39;&amp;/);
    const u = montar({ datos: { ...DATOS(), categorias: [...CATEGORIAS, { id: 9, nombre: hostil, motivo_sistema: null }] } });
    await u.vista.handleFusionarCategoria(4);
    assert.doesNotMatch(u.modales[0].html, /<img/);
});

test('fusionar: sin otra categoría, o si no se puede leer el catálogo, avisa y no abre nada', async () => {
    const sola = montar({ datos: { ...DATOS(), categorias: [CATEGORIAS[2]] } });
    await sola.vista.handleFusionarCategoria(3);
    assert.equal(sola.modales.length, 0);
    assert.equal(sola.avisos[0].tipo, 'error');
    const rota = montar();
    rota.vista.dep.api.obtenerCategorias = async () => { throw new Error('sin base'); };
    await rota.vista.handleFusionarCategoria(3);
    assert.equal(rota.modales.length, 0);
    assert.match(rota.avisos[0].mensaje, /sin base/);
});

// --- Fusionar: la vista previa ---

const camposDeFusion = (valor = '3') => ({ fusion_destino: el({ value: valor }), fusion_vista_previa: el(), fusion_confirmar: el({ disabled: true }) });

test('vista previa: cuántos gastos, el rango de fechas y «Confirmar» habilitado', async () => {
    const t = montar({ campos: camposDeFusion('3') });
    await t.vista.cambiarDestinoFusion('3', 4);
    assert.deepEqual(t.llamadas, [['previsualizarFusionCategorias', 4, 3]]);
    const html = t.registro.fusion_vista_previa.innerHTML;
    assert.match(html, /<strong>3 gastos<\/strong> de «Renta» pasarán a «Comida»\. Del 21\/07\/2026 al 03\/09\/2026\./);
    assert.match(html, /«Renta» se eliminará\. Queda un caso en el registro de correcciones\./);
    assert.equal(t.registro.fusion_confirmar.disabled, false);
});

test('vista previa: un gasto, una sola fecha, y las advertencias de Rust', async () => {
    const t = montar({ campos: camposDeFusion('3'), previa: (o, d) => previaDe(o, d, { gastos: 1, desde: '05/09/2026', hasta: '05/09/2026', advertencias: ['Algo que conviene saber.'] }) });
    await t.vista.cambiarDestinoFusion('3', 4);
    const html = t.registro.fusion_vista_previa.innerHTML;
    assert.match(html, /<strong>1 gasto<\/strong>/);
    assert.match(html, /Fecha: 05\/09\/2026\./);
    assert.match(html, /⚠️ Algo que conviene saber\./);
});

test('vista previa: sin gastos no inventa un rango', async () => {
    const t = montar({ campos: camposDeFusion('3'), previa: (o, d) => previaDe(o, d, { gastos: 0, desde: null, hasta: null, advertencias: ['«Renta» no tiene gastos: solo se eliminará la categoría.'] }) });
    await t.vista.cambiarDestinoFusion('3', 4);
    const html = t.registro.fusion_vista_previa.innerHTML;
    assert.match(html, /<strong>0 gastos<\/strong>/);
    assert.doesNotMatch(html, /Del |Fecha:/);
    assert.match(html, /no tiene gastos/);
    assert.equal(t.registro.fusion_confirmar.disabled, false, 'fusionar una vacía es válido');
});

test('vista previa: advierte, con texto, si la fusión cambia cómo cuenta el conmutador «Incluir inversión y obligaciones»', async () => {
    // Renta (en la regla) → Comida (fuera de ella): sus gastos pasarán a contarse siempre.
    const t = montar({ campos: camposDeFusion('3') });
    await t.vista.cambiarDestinoFusion('3', 4);
    assert.match(t.registro.fusion_vista_previa.innerHTML, /⚠️ En el Resumen, el conmutador «Incluir inversión y obligaciones» deja fuera «Renta» y cuenta «Comida»: tras la fusión, sus gastos se contarán con ese conmutador\./);
    // Dentro de la regla las dos (Renta → Impuestos): sin advertencia.
    const u = montar({ campos: camposDeFusion('5') });
    await u.vista.cambiarDestinoFusion('5', 4);
    assert.doesNotMatch(u.registro.fusion_vista_previa.innerHTML, /conmutador/);
    // Comida (fuera) → Renta (dentro): se excluirán.
    const v = montar({ campos: camposDeFusion('4') });
    await v.vista.cambiarDestinoFusion('4', 3);
    assert.match(v.registro.fusion_vista_previa.innerHTML, /se excluirán con ese conmutador/);
});

test('vista previa: los nombres hostiles de la respuesta se escapan', async () => {
    const hostil = `<script>alert(1)</script> "'`;
    const t = montar({ campos: camposDeFusion('3'), previa: (o, d) => previaDe(o, d, { origen_nombre: hostil, destino_nombre: hostil, advertencias: [hostil], desde: hostil, hasta: hostil }) });
    await t.vista.cambiarDestinoFusion('3', 4);
    const html = t.registro.fusion_vista_previa.innerHTML;
    assert.doesNotMatch(html, /<script>/);
    assert.match(html, /&lt;script&gt;alert\(1\)&lt;\/script&gt; &quot;&#39;/);
});

test('vista previa: sin destino elegido, «Confirmar» se deshabilita y no se consulta nada', async () => {
    const t = montar({ campos: camposDeFusion('') });
    t.registro.fusion_confirmar.disabled = false;
    await t.vista.cambiarDestinoFusion('', 4);
    assert.deepEqual(t.llamadas, []);
    assert.equal(t.registro.fusion_confirmar.disabled, true);
    assert.match(t.registro.fusion_vista_previa.innerHTML, /Elige un destino/);
});

test('vista previa: si Rust la rechaza, se muestra el motivo y «Confirmar» sigue deshabilitado', async () => {
    const t = montar({ campos: camposDeFusion('3'), falla: { previsualizarFusionCategorias: 'No se puede fusionar en otra la categoría «Otros»' } });
    await t.vista.cambiarDestinoFusion('3', 1);
    assert.match(t.registro.fusion_vista_previa.innerHTML, /No se puede fusionar en otra la categoría «Otros»/);
    assert.equal(t.registro.fusion_confirmar.disabled, true);
});

test('vista previa: una respuesta tardía de un destino que ya no es el elegido se descarta', async () => {
    const t = montar({ campos: camposDeFusion('5') });   // el selector ya está en otro destino
    await t.vista.cambiarDestinoFusion('3', 4);
    assert.doesNotMatch(t.registro.fusion_vista_previa.innerHTML, /pasarán a/);
    assert.equal(t.registro.fusion_confirmar.disabled, true);
});

// --- Fusionar: confirmar ---

test('confirmar: fusiona con la vista previa vigente, cierra la ventana, avisa cuántos gastos y redibuja', async () => {
    const campos = { ...camposDeFusion('3'), 'modal-fusion-categoria': { remove() { this.quitado = true; } } };
    const t = montar({ campos, movidos: 3 });
    await t.vista.cambiarDestinoFusion('3', 4);
    await t.vista.handleConfirmarFusion(4);
    assert.deepEqual(t.llamadas.at(-1), ['fusionarCategorias', 4, 3]);
    assert.equal(t.registro['modal-fusion-categoria'].quitado, true);
    assert.equal(t.avisos.at(-1).mensaje, 'Fusión hecha: 3 gastos pasaron a «Comida».');
    assert.deepEqual(t.rutas, ['ajustes']);
});

test('confirmar: con un solo gasto el aviso va en singular', async () => {
    const campos = { ...camposDeFusion('3'), 'modal-fusion-categoria': { remove() {} } };
    const t = montar({ campos, movidos: 1 });
    await t.vista.cambiarDestinoFusion('3', 4);
    await t.vista.handleConfirmarFusion(4);
    assert.equal(t.avisos.at(-1).mensaje, 'Fusión hecha: 1 gasto pasó a «Comida».');
});

test('confirmar: sin vista previa vigente, o con otro destino en el selector, no hace nada', async () => {
    const sin = montar({ campos: camposDeFusion('3') });
    await sin.vista.handleConfirmarFusion(4);
    assert.deepEqual(sin.llamadas, []);

    const t = montar({ campos: camposDeFusion('3') });
    await t.vista.cambiarDestinoFusion('3', 4);
    t.registro.fusion_destino.value = '5';   // el titular cambió el selector y la vista previa aún no llegó
    await t.vista.handleConfirmarFusion(4);
    assert.equal(t.llamadas.filter(l => l[0] === 'fusionarCategorias').length, 0);
});

test('confirmar: si Rust rechaza, muestra el error, deja la ventana abierta y rehabilita «Confirmar»', async () => {
    const campos = { ...camposDeFusion('3'), 'modal-fusion-categoria': { remove() { this.quitado = true; } } };
    const t = montar({ campos, falla: { fusionarCategorias: 'fallo de prueba' } });
    await t.vista.cambiarDestinoFusion('3', 4);
    await t.vista.handleConfirmarFusion(4);
    assert.equal(t.avisos.at(-1).tipo, 'error');
    assert.match(t.avisos.at(-1).mensaje, /fallo de prueba/);
    assert.equal(t.registro['modal-fusion-categoria'].quitado, undefined);
    assert.equal(t.registro.fusion_confirmar.disabled, false);
    assert.deepEqual(t.rutas, []);
});

// --- Cuentas archivadas ---

test('las archivadas se listan aparte, sin balance, con su estado en texto, y su nombre hostil se escapa', async () => {
    const t = montar();
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.match(html, /Cuentas archivadas \(1\)/);
    assert.match(html, /Archivada ·/, 'el estado va en texto, no solo en el estilo');
    assert.match(html, /Cuenta &lt;Vieja&gt;/);
    assert.deepEqual(llamadasDelManejador(html, 'handleReactivarCuenta'), [[9]]);
    assert.doesNotMatch(html, /Cuenta <Vieja>/);
    const bloque = html.slice(html.indexOf('Cuentas archivadas'), html.indexOf('Tarjetas de Crédito'));
    assert.doesNotMatch(bloque, /#0#/, 'las archivadas no muestran balance');
});

test('sin cuentas archivadas no hay bloque de archivadas', async () => {
    const t = montar({ datos: { ...DATOS(), cuentasArchivadas: [] } });
    await t.vista.render();
    assert.doesNotMatch(t.pantalla.contenido.innerHTML, /Cuentas archivadas/);
});

// --- Categorías fijas / recurrentes ---

test('cada categoría trae su casillero «Fija / recurrente», con la marca actual, etiqueta accesible y ayuda', async () => {
    const t = montar();
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    const casilleros = [...html.matchAll(/<input type="checkbox" class="cat-fija"[^>]*>/g)].map(m => m[0]);
    assert.equal(casilleros.length, 5);
    assert.deepEqual(casilleros.map(c => /\schecked/.test(c)), [false, true, false, false, false], 'la marca viene del catálogo');
    assert.match(casilleros[1], /aria-label="Fija \/ recurrente: Suscripciones"/);
    const llamadas = llamadasDelManejador(html, 'handleMarcarCategoriaFija');
    assert.deepEqual(llamadas.map(a => a.slice(1)), [[1], [2], [3], [4], [5]]);
    assert.equal(llamadas[0][0], 'ELEMENTO');
    assert.match(html, /Marca las categorías cuyo gasto se repite cada período, aunque el monto cambie un poco/);
    assert.doesNotMatch(html, /\son(click|change)=/);
});

test('el nombre hostil de una categoría se escapa en la etiqueta del casillero', async () => {
    const datos = DATOS();
    datos.categorias = [{ id: 9, nombre: `<img src=x onerror=alert(1)> "'`, fija: false }];
    const t = montar({ datos });
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.doesNotMatch(html, /<img/);
    assert.match(html, /aria-label="Fija \/ recurrente: &lt;img/);
});

test('marcar envía (id, true), avisa y no redibuja; desmarcar envía false', async () => {
    const t = montar();
    const control = { checked: true };
    await t.vista.handleMarcarCategoriaFija(control, 3);
    assert.deepEqual(t.llamadas, [['marcarCategoriaFija', 3, true]]);
    assert.deepEqual(t.avisos.map(a => a.mensaje), ['«Comida» ahora es fija / recurrente.']);
    assert.equal(control.checked, true);
    assert.deepEqual(t.rutas, []);
    const otro = { checked: false };
    await t.vista.handleMarcarCategoriaFija(otro, 3);
    assert.deepEqual(t.llamadas.at(-1), ['marcarCategoriaFija', 3, false]);
    assert.match(t.avisos.at(-1).mensaje, /ahora es variable/);
});

test('si Rust rechaza el cambio el casillero vuelve a como estaba y se avisa del error', async () => {
    const t = montar({ falla: { marcarCategoriaFija: 'No se encontró la categoría con identificador 3.' } });
    const control = { checked: true };
    await t.vista.handleMarcarCategoriaFija(control, 3);
    assert.equal(control.checked, false, 'se deshace el cambio visible');
    assert.equal(t.avisos.at(-1).tipo, 'error');
    assert.match(t.avisos.at(-1).mensaje, /No se encontró la categoría/);
});

test('vista previa: la advertencia sobre la marca «fija / recurrente» que manda Rust se muestra con su icono y en texto', async () => {
    const aviso = '«Renta» es fija / recurrente y «Comida» es variable: tras la fusión, los gastos tomarán la marca de «Comida» (variable).';
    const t = montar({ campos: camposDeFusion('3'), previa: (o, d) => previaDe(o, d, { origen_fija: true, advertencias: [aviso] }) });
    await t.vista.cambiarDestinoFusion('3', 4);
    assert.match(t.registro.fusion_vista_previa.innerHTML, /⚠️ «Renta» es fija \/ recurrente y «Comida» es variable: tras la fusión, los gastos tomarán la marca de «Comida» \(variable\)\./);
});

test('la lista conserva el casillero «Fija / recurrente» de las categorías que se pueden renombrar y fusionar, y de las del sistema', async () => {
    const t = montar();
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.equal([...html.matchAll(/class="cat-fija"/g)].length, 5);
    assert.deepEqual(llamadasDelManejador(html, 'handleRenombrarCategoria').length, 2);
});
