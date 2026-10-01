// Pruebas de la vista «Ingresos» en Node, sin navegador.
//
// Facturas formales e ingresos informales: alta, cobro y corrección. Corregir
// una factura ya cobrada mueve un saldo real, así que la vista pide un motivo
// o una confirmación **antes** de hablar con Rust. Con la vista recibiendo sus
// dependencias se le dan dobles de todo y un reloj fijo. Datos inventados.
//
// Se importa el JavaScript compilado de la vista (`npm test` compila antes).

import test from 'node:test';
import assert from 'node:assert/strict';
import { llamadasDelManejador } from '../ayudas/manejadores_html.js';
import { VistaIngresos, MANEJADORES_INGRESOS, puenteIngresos } from '../../../src/js/vistas/ingresos.js';

const factura = (extra = {}) => ({
    id: 4, numero_factura: 'FAC-0007', cliente_id: 1, cliente_nombre: 'Cliente Uno', cliente_rnc: '101000001',
    fecha_emision: '10/03/2027', monto_total: 1000, porcentaje_retencion: 10, monto_retenido: 100,
    estatus: 'emitida', monto_recibido: null, institucion_deposito: null, ...extra,
});
const informal = (extra = {}) => ({
    id: 8, descripcion: 'Trabajo suelto', fecha: '12/03/2027', monto: 300, estatus: 'pendiente',
    institucion_deposito: null, fecha_pago: null, ...extra,
});
const CLIENTES = [{ id: 1, nombre: 'Cliente Uno', rnc: '101000001' }, { id: 2, nombre: 'Cliente Dos', rnc: '101000002' }];
const CUENTAS = [{ id: 1, nombre: 'Cuenta Pesos', divisa: 'DOP', balance_actual: 5000 }];

/** Un elemento mínimo del DOM con lo que la vista lee y escribe. */
const el = (spec = {}) => ({ value: '', checked: false, hidden: false, innerHTML: '', style: {}, dataset: {}, ...spec, remove() { this.quitado = true; } });

function montar({ ingresos = [factura()], informales = [informal()], campos = {}, falla = null, confirmar = true, motivo = 'Cobro mal registrado' } = {}) {
    const llamadas = [], avisos = [], rutas = [], modales = [], confirmaciones = [], motivos = [];
    const comprobar = () => { if (falla) throw new Error(falla); };
    const api = {
        obtenerIngresos: async () => ingresos,
        obtenerIngresosInformales: async () => informales,
        obtenerClientes: async () => CLIENTES,
        obtenerCuentas: async () => CUENTAS,
        crearIngreso: async datos => { llamadas.push(['crearIngreso', datos]); comprobar(); },
        crearIngresoInformal: async (...a) => { llamadas.push(['crearIngresoInformal', ...a]); comprobar(); },
        actualizarIngreso: async (...a) => { llamadas.push(['actualizarIngreso', ...a]); comprobar(); return 'Factura corregida (prueba).'; },
        marcarIngresoPagado: async (...a) => { llamadas.push(['marcarIngresoPagado', ...a]); comprobar(); },
        marcarInformalPagado: async (...a) => { llamadas.push(['marcarInformalPagado', ...a]); comprobar(); },
    };
    const pantalla = { contenido: { innerHTML: '' } };
    const registro = { ...campos };
    const vista = new VistaIngresos({
        api,
        avisos: { mostrar: (mensaje, tipo) => avisos.push({ mensaje, tipo }) },
        formato: { importe: v => `#${v}#` },
        enrutador: { mostrar: async ruta => { rutas.push(ruta); } },
        pantalla,
        dom: {
            elemento: id => { if (!(id in registro)) throw new Error(`No existe el elemento #${id} en la página.`); return registro[id]; },
            buscar: id => registro[id] ?? null,
        },
        dialogos: { confirmar: m => { confirmaciones.push(m); return confirmar; }, preguntar: () => null },
        motivo: { pedir: (titulo, texto) => { motivos.push({ titulo, texto }); return motivo; } },
        // Una ventana abierta existe luego en el DOM con su contenido, como en la página.
        modales: { abrir: (id, html) => { modales.push({ id, html }); registro[id] = el({ innerHTML: html }); } },
        ahora: () => new Date(2027, 2, 5),
    });
    return { vista, llamadas, avisos, rutas, modales, confirmaciones, motivos, pantalla, registro, evento: { preventDefault() { this.evitado = (this.evitado || 0) + 1; } } };
}

const CAMPOS_FORMAL = () => ({
    num_fac: el({ value: 'FAC-0008' }), fec_em: el({ value: '05/03/2027' }), cli_nom: el({ value: 'Cliente Uno' }),
    cli_rnc: el({ value: '101000001' }), mon_tot: el({ value: '1000.50' }), ret_por: el({ value: '10' }),
});

test('dibuja facturas e informales, y propone la factura siguiente a la última', async () => {
    const t = montar();
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.match(html, /<h1>Ingresos<\/h1>/);
    assert.match(html, /FAC-0007/);
    assert.match(html, /Trabajo suelto/);
    assert.match(html, /value="FAC-0008"/);
    assert.doesNotMatch(html, /undefined|NaN/);
});

test('sin facturas propone FAC-0001; un número sin dígitos finales recibe «-1»', async () => {
    const t = montar({ ingresos: [], informales: [] });
    await t.vista.render();
    assert.match(t.pantalla.contenido.innerHTML, /value="FAC-0001"/);
    const u = montar({ ingresos: [factura({ numero_factura: 'ESPECIAL' })] });
    await u.vista.render();
    assert.match(u.pantalla.contenido.innerHTML, /value="ESPECIAL-1"/);
});

test('la fecha de hoy sale del reloj inyectado, no del sistema', async () => {
    const t = montar();
    await t.vista.render();
    assert.match(t.pantalla.contenido.innerHTML, /05\/03\/2027/);
});

test('solo la factura emitida ofrece «cobrar»; la cobrada muestra su depósito', async () => {
    const t = montar({
        ingresos: [factura(), factura({ id: 5, numero_factura: 'FAC-0006', estatus: 'pagada', institucion_deposito: 'Banco Alfa' })],
        informales: [informal(), informal({ id: 9, estatus: 'pagado', institucion_deposito: 'Banco Beta', fecha_pago: '13/03/2027' })],
    });
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.equal([...html.matchAll(/appUI\.abrirCobroFormal\(/g)].length, 1);
    assert.match(html, /abrirCobroFormal\(4, 900\)/);
    assert.match(html, /Dep: Banco Alfa/);
    assert.equal([...html.matchAll(/appUI\.abrirCobroInformal\(/g)].length, 1);
    assert.match(html, /En Banco Beta el 13\/03\/2027/);
});

test('alta de factura: importes y porcentaje como número, textos tal cual, y redibuja', async () => {
    const t = montar({ campos: CAMPOS_FORMAL() });
    await t.vista.handleAgregarIngreso(t.evento);
    assert.equal(t.evento.evitado, 1);
    assert.deepEqual(t.llamadas, [['crearIngreso', {
        numero_factura: 'FAC-0008', rnc_cliente: '101000001', nombre_cliente: 'Cliente Uno',
        fecha_emision: '05/03/2027', monto_total: 1000.5, porcentaje_retencion: 10,
    }]]);
    assert.deepEqual(t.avisos, [{ mensaje: 'Factura registrada exitosamente.', tipo: undefined }]);
    assert.deepEqual(t.rutas, ['ingresos']);
});

test('alta de factura: si Rust rechaza se avisa el error y no se redibuja', async () => {
    const t = montar({ campos: CAMPOS_FORMAL(), falla: 'Rechazada por prueba' });
    await t.vista.handleAgregarIngreso(t.evento);
    assert.deepEqual(t.avisos.map(a => a.tipo), ['error']);
    assert.match(t.avisos[0].mensaje, /Rechazada por prueba/);
    assert.deepEqual(t.rutas, []);
});

test('ingreso informal: (fecha, descripción, monto) en ese orden', async () => {
    const t = montar({ campos: { fecha_inf: el({ value: '06/03/2027' }), monto_inf: el({ value: '75.25' }), desc_inf: el({ value: 'Clase suelta' }) } });
    await t.vista.handleAgregarIngresoInformal(t.evento);
    assert.deepEqual(t.llamadas, [['crearIngresoInformal', '06/03/2027', 'Clase suelta', '75.25']]);
    assert.deepEqual(t.rutas, ['ingresos']);
});

test('ingreso informal: el importe viaja como TEXTO, con los espacios recortados y los dígitos intactos', async () => {
    for (const [escrito, esperado] of [[' 75.25 ', '75.25'], ['1.005', '1.005'], ['0075.250', '0075.250'], ['', '']]) {
        const t = montar({ campos: { fecha_inf: el({ value: '06/03/2027' }), monto_inf: el({ value: escrito }), desc_inf: el({ value: 'Clase' }) } });
        await t.vista.handleAgregarIngresoInformal(t.evento);
        assert.strictEqual(t.llamadas[0][3], esperado, JSON.stringify(escrito));
        assert.equal(typeof t.llamadas[0][3], 'string');
    }
});

test('elegir un cliente rellena nombre y RNC; «nuevo cliente» los deja vacíos', () => {
    const opcion = { getAttribute: n => ({ 'data-nombre': 'Cliente Dos', 'data-rnc': '101000002' })[n] };
    const campos = { cli_select: { options: [{ getAttribute: () => null }, opcion], selectedIndex: 1 }, cli_nom: el({ value: 'viejo' }), cli_rnc: el({ value: 'viejo' }) };
    const t = montar({ campos });
    t.vista.handleSelectCliente('2');
    assert.equal(campos.cli_nom.value, 'Cliente Dos');
    assert.equal(campos.cli_rnc.value, '101000002');
    campos.cli_select.selectedIndex = 0;
    t.vista.handleSelectCliente('');
    assert.equal(campos.cli_nom.value, '');
    assert.equal(campos.cli_rnc.value, '');
});

test('cobro de factura: abre su ventana con el neto sugerido y la cuenta elegible', async () => {
    const t = montar();
    await t.vista.abrirCobroFormal(4, 900);
    assert.equal(t.modales.length, 1);
    assert.equal(t.modales[0].id, 'modal-cobro-for-4');
    assert.match(t.modales[0].html, /DOP #900#/);
    assert.match(t.modales[0].html, /id="cob_ban_4"/);
    assert.match(t.modales[0].html, /Cuenta Pesos/);
});

test('cobro de factura: envía id, cuenta tal cual, fecha y monto como texto; cierra la ventana', async () => {
    const t = montar({ campos: { cob_ban_4: el({ value: '1' }), cob_fec_4: el({ value: '07/03/2027' }), cob_mon_4: el({ value: '900.00' }), 'modal-cobro-for-4': el() } });
    await t.vista.handleCobroFormalSubmit(t.evento, 4);
    assert.deepEqual(t.llamadas, [['marcarIngresoPagado', 4, '1', '07/03/2027', '900.00']]);
    assert.equal(t.registro['modal-cobro-for-4'].quitado, true);
    assert.deepEqual(t.rutas, ['ingresos']);
});

test('los cobros (de factura y de informal): el importe viaja como TEXTO, recortado y con los dígitos intactos', async () => {
    for (const [escrito, esperado] of [[' 900.5 ', '900.5'], ['8500.005', '8500.005'], ['0900.500', '0900.500']]) {
        const f = montar({ campos: { cob_ban_4: el({ value: '1' }), cob_fec_4: el({ value: '07/03/2027' }), cob_mon_4: el({ value: escrito }), 'modal-cobro-for-4': el() } });
        await f.vista.handleCobroFormalSubmit(f.evento, 4);
        assert.strictEqual(f.llamadas[0][4], esperado, `factura ${JSON.stringify(escrito)}`);
        const i = montar({ campos: { cob_ban_inf_8: el({ value: '1' }), cob_fec_inf_8: el({ value: '14/03/2027' }), cob_mon_inf_8: el({ value: escrito }), 'modal-cobro-inf-8': el() } });
        await i.vista.handleCobroInformalSubmit(i.evento, 8);
        assert.strictEqual(i.llamadas[0][4], esperado, `informal ${JSON.stringify(escrito)}`);
        assert.equal(typeof i.llamadas[0][4], 'string');
    }
});

test('cobro de factura: si Rust rechaza, la ventana sigue abierta', async () => {
    const t = montar({ campos: { cob_ban_4: el({ value: '1' }), cob_fec_4: el(), cob_mon_4: el({ value: '1' }), 'modal-cobro-for-4': el() }, falla: 'No' });
    await t.vista.handleCobroFormalSubmit(t.evento, 4);
    assert.equal(t.registro['modal-cobro-for-4'].quitado, undefined);
    assert.equal(t.avisos[0].tipo, 'error');
    assert.deepEqual(t.rutas, []);
});

test('cobro informal: su ventana, el envío y el cierre', async () => {
    const t = montar();
    await t.vista.abrirCobroInformal(8, 300);
    assert.equal(t.modales[0].id, 'modal-cobro-inf-8');
    assert.match(t.modales[0].html, /id="cob_ban_inf_8"/);
    t.registro.cob_ban_inf_8 = el({ value: '1' });
    t.registro.cob_fec_inf_8 = el({ value: '14/03/2027' });
    t.registro.cob_mon_inf_8 = el({ value: '300' });
    await t.vista.handleCobroInformalSubmit(t.evento, 8);
    assert.deepEqual(t.llamadas, [['marcarInformalPagado', 8, '1', '14/03/2027', '300']]);
    assert.equal(t.registro['modal-cobro-inf-8'].quitado, true);
});

test('editar factura: la ventana lleva el estado (cobrada y recibido) para el envío', async () => {
    const t = montar();
    const cobrada = factura({ estatus: 'pagada', monto_recibido: 850, institucion_deposito: 'Banco Alfa' });
    t.vista.abrirEdicionFormal(JSON.stringify(cobrada));
    await new Promise(r => setImmediate(r));
    const m = t.registro['modal-edit-for-4'];
    assert.equal(m.dataset.cobrada, 'si');
    assert.equal(m.dataset.recibido, '850');
    assert.match(m.innerHTML, /Banco Alfa/);
    assert.match(m.innerHTML, /id="edit_parcial_chk_4"/);
    const u = montar();
    u.vista.abrirEdicionFormal(JSON.stringify(factura()));
    await new Promise(r => setImmediate(r));
    assert.equal(u.registro['modal-edit-for-4'].dataset.cobrada, 'no');
    assert.doesNotMatch(u.registro['modal-edit-for-4'].innerHTML, /edit_parcial_chk_4/);
});

const CAMPOS_EDICION = (extra = {}, ventana = {}) => ({
    edit_num_fac_4: el({ value: 'FAC-0007' }), edit_fec_em_4: el({ value: '10/03/2027' }), edit_cli_select_4: el({ value: '2' }),
    edit_mon_tot_4: el({ value: '1000' }), edit_ret_por_4: el({ value: '10' }),
    'modal-edit-for-4': el({ dataset: { cobrada: 'no', recibido: '0', ...ventana } }), ...extra,
});

test('editar factura sin cobrar: se envía sin parcial ni motivo y se cierra', async () => {
    const t = montar({ campos: CAMPOS_EDICION() });
    await t.vista.handleEdicionFormalSubmit(t.evento, 4);
    assert.deepEqual(t.llamadas, [['actualizarIngreso', 4, 'FAC-0007', 2, '10/03/2027', '1000', 10, null, null]]);
    assert.equal(t.motivos.length + t.confirmaciones.length, 0);
    assert.deepEqual(t.avisos, [{ mensaje: 'Factura corregida (prueba).', tipo: undefined }]);
    assert.equal(t.registro['modal-edit-for-4'].quitado, true);
    assert.deepEqual(t.rutas, ['ingresos']);
});

test('editar factura cobrada cuyo neto cambia: pide motivo con el ajuste y lo envía', async () => {
    const t = montar({ campos: CAMPOS_EDICION({}, { cobrada: 'si', recibido: '800' }), motivo: 'Neto corregido' });
    await t.vista.handleEdicionFormalSubmit(t.evento, 4);
    assert.equal(t.motivos.length, 1);
    assert.match(t.motivos[0].texto, /DOP #100#/);
    assert.deepEqual(t.llamadas, [['actualizarIngreso', 4, 'FAC-0007', 2, '10/03/2027', '1000', 10, null, 'Neto corregido']]);
});

test('editar factura cobrada: cancelar el motivo no envía nada ni cierra la ventana', async () => {
    const t = montar({ campos: CAMPOS_EDICION({}, { cobrada: 'si', recibido: '800' }), motivo: null });
    await t.vista.handleEdicionFormalSubmit(t.evento, 4);
    assert.deepEqual(t.llamadas, []);
    assert.equal(t.registro['modal-edit-for-4'].quitado, undefined);
});

test('editar factura cobrada sin cambio de saldo: confirma con las cifras y no pide motivo', async () => {
    const t = montar({ campos: CAMPOS_EDICION({}, { cobrada: 'si', recibido: '900' }) });
    await t.vista.handleEdicionFormalSubmit(t.evento, 4);
    assert.equal(t.motivos.length, 0);
    assert.equal(t.confirmaciones.length, 1);
    assert.match(t.confirmaciones[0], /DOP #900#/);
    assert.match(t.confirmaciones[0], /neto completo/);
    assert.equal(t.llamadas.length, 1);
    const u = montar({ campos: CAMPOS_EDICION({}, { cobrada: 'si', recibido: '900' }), confirmar: false });
    await u.vista.handleEdicionFormalSubmit(u.evento, 4);
    assert.deepEqual(u.llamadas, []);
});

test('cobro parcial: el importe viaja como TEXTO tal como se escribió', async () => {
    const campos = CAMPOS_EDICION({ edit_parcial_chk_4: el({ checked: true }), edit_parcial_mon_4: el({ value: ' 700.10 ' }) }, { cobrada: 'si', recibido: '900' });
    const t = montar({ campos });
    await t.vista.handleEdicionFormalSubmit(t.evento, 4);
    assert.equal(t.llamadas[0][7], '700.10');
    assert.equal(typeof t.llamadas[0][7], 'string');
});

test('cobro parcial vacío, de forma inválida o mayor que el neto: no se envía', async () => {
    for (const [valor, texto] of [['', /Indica cuánto/], ['abc', /Indica cuánto/], ['900.01', /no puede superar el neto \(#900#\)/]]) {
        const campos = CAMPOS_EDICION({ edit_parcial_chk_4: el({ checked: true }), edit_parcial_mon_4: el({ value: valor }) }, { cobrada: 'si', recibido: '900' });
        const t = montar({ campos });
        await t.vista.handleEdicionFormalSubmit(t.evento, 4);
        assert.deepEqual(t.llamadas, [], valor);
        assert.equal(t.avisos[0].tipo, 'error');
        assert.match(t.avisos[0].mensaje, texto);
    }
});

test('editar factura: si Rust rechaza, la ventana sigue abierta', async () => {
    const t = montar({ campos: CAMPOS_EDICION(), falla: 'No' });
    await t.vista.handleEdicionFormalSubmit(t.evento, 4);
    assert.equal(t.registro['modal-edit-for-4'].quitado, undefined);
    assert.equal(t.avisos[0].tipo, 'error');
});

test('el cobro parcial solo despliega su caja mientras la casilla está marcada', () => {
    const caja = el({ hidden: true });
    const marca = el({ checked: true });
    const t = montar({ campos: { edit_parcial_caja_4: caja, edit_parcial_chk_4: marca } });
    t.vista.alternarCobroParcial(4);
    assert.equal(caja.hidden, false);
    marca.checked = false;
    t.vista.alternarCobroParcial(4);
    assert.equal(caja.hidden, true);
});

test('el puente expone todos los manejadores y delega en la vista', async () => {
    const t = montar({ campos: CAMPOS_FORMAL() });
    const puente = puenteIngresos(t.vista);
    assert.deepEqual(Object.keys(puente).sort(), [...MANEJADORES_INGRESOS].sort());
    await puente.handleAgregarIngreso(t.evento);
    assert.equal(t.llamadas.length, 1);
});

test('todo appUI.x( que escribe la plantilla y sus ventanas está en el puente', async () => {
    const t = montar({ ingresos: [factura(), factura({ id: 5, estatus: 'pagada' })] });
    await t.vista.render();
    await t.vista.abrirCobroFormal(4, 900);
    await t.vista.abrirCobroInformal(8, 300);
    t.vista.abrirEdicionFormal(JSON.stringify(factura({ estatus: 'pagada' })));
    await new Promise(r => setImmediate(r));
    const html = t.pantalla.contenido.innerHTML + t.modales.map(m => m.html).join('') + t.registro['modal-edit-for-4'].innerHTML;
    const llamados = new Set([...html.matchAll(/appUI\.(\w+)\(/g)].map(m => m[1]));
    assert.ok(llamados.size >= 8, [...llamados].join());
    for (const nombre of llamados) assert.ok(MANEJADORES_INGRESOS.includes(nombre), `falta ${nombre} en el puente`);
});

// --- texto del titular en el HTML y en los manejadores ---

test('una factura con nombre de cliente hostil llega íntegra al editor (JSON en texto) y no se interpreta', async () => {
    const hostil = `x'); alert(1); //"\\ <img src=x onerror="alert(2)"> &amp;`;
    const f = factura({ id: 11, cliente_nombre: hostil, numero_factura: hostil, institucion_deposito: hostil });
    const t = montar({ ingresos: [f] });
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    const [[texto]] = llamadasDelManejador(html, 'abrirEdicionFormal');
    assert.deepEqual(JSON.parse(texto), f);
    assert.doesNotMatch(html, /<img/);
});

test('el informal y el número de factura siguiente escapan lo que llevan', async () => {
    const hostil = `x'); alert(1); //"\\ <img src=x onerror="alert(2)"> &amp;`;
    const t = montar({ ingresos: [factura({ numero_factura: `FAC-"><img src=x>7` })], informales: [informal({ descripcion: hostil, estatus: 'pagado', institucion_deposito: hostil })] });
    await t.vista.render();
    assert.doesNotMatch(t.pantalla.contenido.innerHTML, /<img/);
});
