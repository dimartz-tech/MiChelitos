// Pruebas de la vista «Financiamientos y Deudas» en Node, sin navegador.
//
// Es la pestaña que **calcula en la vista**: el pasivo total (financiamientos
// más tarjetas, con los dólares expresados en pesos), la carga mensual, el cupo
// disponible, el día que vence antes y la composición por tipo. Con la vista
// recibiendo sus dependencias se le dan dobles de todo, un reloj y una tasa
// fijos. Datos inventados, con cifras que se comprueban a mano.
//
// Se importa el JavaScript compilado de la vista (`npm test` compila antes).

import test from 'node:test';
import assert from 'node:assert/strict';
import { llamadasDelManejador, nombresDelPuente } from '../ayudas/manejadores_html.js';
import { VistaPrestamos, MANEJADORES_PRESTAMOS, puentePrestamos, categoriaDePrestamo } from '../../../src/js/vistas/prestamos.js';

const prestamo = (extra = {}) => ({
    id: 1, tipo_prestamo: 'consumo', monto_prestamo: 2000, institucion_financiera: 'Banco Alfa', tasa_actual: 12,
    cuotas_totales: 24, cuotas_pendientes: 5, monto_cuota: 100, dia_pago: 10, saldo_actual: 1000, limite_credito: null,
    tarjeta_id: null, tarjeta_nombre: null, dia_corte: null, disponible: null, es_revolvente: false, alerta_pago: false, dias_pago_msg: '',
    ...extra,
});
const tarjeta = (extra = {}) => ({
    id: 7, entidad: 'Banco Beta', nombre_tarjeta: 'Oro', fecha_corte: 1, fecha_limite_pago: 8,
    balance_pesos: 200, balance_dolares: 10, disponible_pesos: 800, disponible_dolares: 90, ...extra,
});
// Hoy es el día 5 y el dólar vale 60 en estas pruebas.
const DATOS = () => ({
    prestamos: [
        prestamo({ id: 1 }),                                                                                  // consumo: 1000, cuota 100, día 10
        prestamo({ id: 2, tipo_prestamo: 'hipotecario', saldo_actual: 5000, monto_cuota: 300, cuotas_pendientes: 100, dia_pago: 3 }),
        prestamo({ id: 3, tipo_prestamo: 'flexible', es_revolvente: true, saldo_actual: 400, monto_cuota: 40, cuotas_totales: null,
            cuotas_pendientes: null, limite_credito: 1000, disponible: 600, dia_pago: 20, tarjeta_id: 7 }),    // cuelga de la tarjeta 7
        prestamo({ id: 4, tipo_prestamo: 'vehiculo', saldo_actual: 50, monto_cuota: 999, cuotas_pendientes: 0, dia_pago: 6 }), // ya sin cuotas
    ],
    tarjetas: [tarjeta()],
});

/** Un elemento mínimo del DOM con lo que la vista lee y escribe. */
const el = (spec = {}) => ({ value: '', checked: false, disabled: false, required: false, innerHTML: '', textContent: '', style: {}, ...spec, remove() { this.quitado = true; } });
const CAMPOS_TOGGLE = () => ({ pre_campos_cuotas: el(), pre_tot: el(), pre_pen: el(), pre_grupo_limite: el(), pre_lim: el() });

const CUENTAS = [
    { id: 5, nombre: 'Ahorro <Alfa>', divisa: 'DOP', balance_actual: 900, entidad: 'Banco Alfa', comision_pago_impuestos: null },
    { id: 6, nombre: 'Cuenta Dólar', divisa: 'USD', balance_actual: 30, entidad: null, comision_pago_impuestos: null },
    { id: 8, nombre: 'Nómina', divisa: 'DOP', balance_actual: 50, entidad: null, comision_pago_impuestos: null },
];
const CATEGORIAS = [
    { id: 1, nombre: 'Otros', fija: false, motivo_sistema: 'x' },
    { id: 9, nombre: 'Préstamo', fija: true, motivo_sistema: null },
];
const PREVIA = { interes: '10.37', capital: '89.63', interes_estimado: true, saldo_previo: '987.41', saldo_resultante: '897.78', cuotas_pendientes_despues: 4 };
const movimiento = (extra = {}) => ({ id: 30, fecha: '15/02/2027', tipo: 'cuota', monto: 100, interes: 10, capital: 90, saldo_resultante: 910, gasto_id: 77, ...extra });

function montar({ datos = DATOS(), campos = {}, falla = null, confirmar = true, respuesta = '1234.50', tasa = 60, motivo = 'Pago registrado dos veces', previa = async () => PREVIA, movimientos = [movimiento(), movimiento({ id: 29, tipo: 'declaracion', gasto_id: null })] } = {}) {
    const llamadas = [], avisos = [], rutas = [], modales = [], confirmaciones = [], preguntas = [], menus = [], motivos = [];
    const comprobar = () => { if (falla) throw new Error(falla); };
    const api = {
        obtenerPrestamos: async () => datos.prestamos,
        obtenerTarjetas: async () => datos.tarjetas,
        crearPrestamo: async d => { llamadas.push(['crearPrestamo', d]); comprobar(); },
        actualizarPrestamo: async d => { llamadas.push(['actualizarPrestamo', d]); comprobar(); },
        declararSaldoPrestamo: async (...a) => { llamadas.push(['declararSaldoPrestamo', ...a]); comprobar(); },
        eliminarPrestamo: async id => { llamadas.push(['eliminarPrestamo', id]); comprobar(); },
        obtenerCuentas: async () => CUENTAS,
        obtenerCategorias: async () => CATEGORIAS,
        obtenerMovimientosPrestamo: async id => { llamadas.push(['obtenerMovimientosPrestamo', id]); return movimientos; },
        previsualizarPagoPrestamo: async (...a) => { llamadas.push(['previsualizarPagoPrestamo', ...a]); return previa(...a); },
        registrarPagoPrestamo: async d => { llamadas.push(['registrarPagoPrestamo', d]); comprobar(); return PREVIA; },
        revertirPagoPrestamo: async (...a) => { llamadas.push(['revertirPagoPrestamo', ...a]); comprobar(); return 'COR-2027-0004'; },
    };
    const pantalla = { contenido: { innerHTML: '' } };
    const registro = { ...CAMPOS_TOGGLE(), ...campos };
    const vista = new VistaPrestamos({
        api,
        avisos: { mostrar: (mensaje, tipo) => avisos.push({ mensaje, tipo }) },
        formato: { importe: v => `#${v}#` },
        enrutador: { mostrar: async ruta => { rutas.push(ruta); } },
        pantalla,
        dom: {
            elemento: id => { if (!(id in registro)) throw new Error(`No existe el elemento #${id} en la página.`); return registro[id]; },
            buscar: id => registro[id] ?? null,
        },
        dialogos: { confirmar: m => { confirmaciones.push(m); return confirmar; }, preguntar: (m, d) => { preguntas.push({ m, d }); return respuesta; } },
        modales: { abrir: (id, html) => { modales.push({ id, html }); registro[id] = el({ innerHTML: html }); } },
        menus: { abrir: o => { menus.push({ ...o, abierto: true }); }, cerrar: () => { menus.forEach(m => { m.abierto = false; }); } },
        referencias: { tasaUsdADop: tasa },
        ahora: () => new Date(2027, 2, 5),
        motivo: { pedir: async (que, consecuencia) => { motivos.push({ que, consecuencia }); return motivo; } },
    });
    return { vista, llamadas, avisos, rutas, modales, confirmaciones, preguntas, menus, motivos, pantalla, registro, evento: { preventDefault() { this.evitado = (this.evitado || 0) + 1; } } };
}

// --- Las cifras que la vista calcula ---

test('el resumen suma financiamientos y tarjetas, con los dólares a la tasa inyectada', () => {
    const t = montar();
    const r = t.vista.resumirPasivos(DATOS().prestamos, DATOS().tarjetas);
    assert.equal(r.total, 7250);             // 1000 + 5000 + 400 + 50 + (200 + 10 × 60)
    assert.equal(r.cargaMensual, 440);       // 100 + 300 + 40: la de cuotas = 0 no cuenta
    assert.equal(r.cupoDisponible, 6800);    // 600 + (800 + 90 × 60)
    const otra = montar({ tasa: 50 }).vista.resumirPasivos(DATOS().prestamos, DATOS().tarjetas);
    assert.equal(otra.total, 7150);          // la tasa no está escrita en la vista
});

test('la composición va por naturaleza del pasivo, de mayor a menor, y suma el 100 %', () => {
    const { composicion } = montar().vista.resumirPasivos(DATOS().prestamos, DATOS().tarjetas);
    assert.deepEqual(composicion.map(c => c.etiqueta), ['Hipotecario', 'Consumo', 'Tarjetas', 'Líneas', 'Vehículo']);
    assert.deepEqual(composicion.map(c => c.monto), [5000, 1000, 800, 400, 50]);
    assert.ok(Math.abs(composicion.reduce((s, c) => s + c.porcentaje, 0) - 100) < 1e-9);
    assert.ok(Math.abs(composicion[0].porcentaje - (5000 / 7250) * 100) < 1e-9);
});

test('sin nada que deber, el resumen es cero y la cabecera no se dibuja', async () => {
    const t = montar({ datos: { prestamos: [], tarjetas: [] } });
    const r = t.vista.resumirPasivos([], []);
    assert.deepEqual([r.total, r.cargaMensual, r.cupoDisponible, r.proximoVencimiento, r.composicion], [0, 0, 0, null, []]);
    await t.vista.render();
    assert.doesNotMatch(t.pantalla.contenido.innerHTML, /Pasivo total/);
    assert.doesNotMatch(t.pantalla.contenido.innerHTML, /undefined|NaN/);
});

test('el próximo vencimiento cuenta distancia en días desde hoy (el del reloj), no el número del día', () => {
    const t = montar();
    // hoy 5: préstamo del 10 (5 días), del 3 (28), línea del 20 (15), tarjeta del 8 (3); el de cuotas = 0 no cuenta
    assert.deepEqual(t.vista.proximoVencimiento(DATOS().prestamos, DATOS().tarjetas), { dia: 8, enDias: 3 });
    // sin la tarjeta, gana el del 10
    assert.deepEqual(t.vista.proximoVencimiento(DATOS().prestamos, []), { dia: 10, enDias: 5 });
    // el día 3 ya pasó este mes: está a 28 días, no a -2
    assert.deepEqual(t.vista.proximoVencimiento([prestamo({ dia_pago: 3 })], []), { dia: 3, enDias: 28 });
    assert.equal(t.vista.proximoVencimiento([], []), null);
});

test('nombres con tilde y con su preposición; un código desconocido no rompe', () => {
    const { vista } = montar();
    assert.equal(vista.nombreTipo('vehiculo'), 'Vehículo');
    assert.equal(vista.nombreTipo('flexible'), 'Línea de crédito');
    assert.equal(vista.nombreTipo('otro'), 'Otro');
    assert.equal(vista.nombreFinanciamiento('hipotecario'), 'Préstamo hipotecario');
    assert.equal(vista.nombreFinanciamiento('vehiculo'), 'Préstamo de vehículo');
    assert.equal(vista.nombreFinanciamiento('raro'), 'Préstamo (raro)');
});

test('las facilidades de una tarjeta se agrupan dentro de ella; el resto, como independientes', () => {
    const g = montar().vista.agruparPasivosPorAcreedor(DATOS().prestamos, DATOS().tarjetas);
    assert.equal(g.tarjetas.length, 1);
    assert.deepEqual(g.tarjetas[0].facilidades.map(f => f.id), [3]);
    assert.deepEqual(g.independientes.map(p => p.id), [1, 2, 4]);
    assert.deepEqual(montar().vista.agruparPasivosPorAcreedor(DATOS().prestamos, [tarjeta({ id: 99 })]).tarjetas, []);
});

// --- Lo que se dibuja ---

test('dibuja la cabecera, el grupo de la tarjeta con su suma y los independientes', async () => {
    const t = montar();
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.match(html, /Financiamientos y Deudas/);
    assert.match(html, /Pasivo total[\s\S]*?DOP #7250#/);
    assert.match(html, /Carga mensual[\s\S]*?#440#/);
    assert.match(html, /Cupo disponible[\s\S]*?#6800#/);
    assert.match(html, /Próximo vence[\s\S]*?Día 8/);
    assert.match(html, /Suma de lo que cuelga aquí<\/div>\s*<div[^>]*>DOP #1200#/);   // 200 + 600 + 400
    assert.match(html, /Financiamientos independientes[\s\S]*?DOP #6050#/);            // 1000 + 5000 + 50
    assert.match(html, /Banco Beta · Corte día 1 · Pago día 8/);
    assert.doesNotMatch(html, /undefined|NaN/);
});

test('una línea muestra su cupo; un amortizable, sus cuotas pendientes', async () => {
    const t = montar();
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.match(html, /Disponible #600# <\/span>|Disponible #600#<\/span>/);
    assert.match(html, /de #1000#/);
    assert.match(html, />5\/24</);
});

test('una línea sin límite declarado lo dice en lugar de inventar uno', async () => {
    const t = montar({ datos: { prestamos: [prestamo({ es_revolvente: true, limite_credito: null, disponible: null, tipo_prestamo: 'flexible', cuotas_pendientes: null })], tarjetas: [] } });
    await t.vista.render();
    assert.match(t.pantalla.contenido.innerHTML, /Sin límite declarado/);
});

test('al dibujar, los campos condicionales se ajustan al tipo ya seleccionado', async () => {
    const campos = { pre_tip: el({ value: 'flexible' }) };
    const t = montar({ campos });
    await t.vista.render();
    assert.equal(t.registro.pre_campos_cuotas.style.display, 'none');
    assert.equal(t.registro.pre_grupo_limite.style.display, 'block');
});

test('el límite solo se ofrece en una línea: cambiar de tipo oculta, exige y vacía lo que corresponde', () => {
    const t = montar({ campos: { pre_tot: el({ value: '12', required: true }), pre_pen: el({ value: '3', required: true }), pre_lim: el({ value: '500' }) } });
    t.vista.toggleCamposPrestamos('flexible');
    assert.equal(t.registro.pre_campos_cuotas.style.display, 'none');
    assert.deepEqual([t.registro.pre_tot.required, t.registro.pre_pen.required, t.registro.pre_tot.value, t.registro.pre_pen.value], [false, false, '', '']);
    assert.equal(t.registro.pre_grupo_limite.style.display, 'block');
    assert.equal(t.registro.pre_lim.value, '500');   // al ofrecerlo no se borra
    t.vista.toggleCamposPrestamos('consumo');
    assert.equal(t.registro.pre_campos_cuotas.style.display, 'grid');
    assert.deepEqual([t.registro.pre_tot.required, t.registro.pre_pen.required], [true, true]);
    assert.equal(t.registro.pre_grupo_limite.style.display, 'none');
    assert.equal(t.registro.pre_lim.value, '');       // un amortizable no puede llevar límite
});

// --- Alta ---

const CAMPOS_ALTA = (extra = {}) => ({
    pre_tip: el({ value: 'consumo' }), pre_ins: el({ value: 'Banco Alfa' }), pre_mon: el({ value: '5000.5' }), pre_tas: el({ value: '12.5' }),
    pre_cuo: el({ value: '250' }), pre_dia: el({ value: '15' }), pre_sal: el({ value: '' }), ...extra,
});

test('alta de consumo: importes como texto, el saldo en blanco como «no declarado»', async () => {
    const t = montar({ campos: CAMPOS_ALTA({ pre_tot: el({ value: '24' }), pre_pen: el({ value: '20' }), pre_lim: el({ value: '999' }) }) });
    await t.vista.handleAgregarPrestamo(t.evento);
    assert.equal(t.evento.evitado, 1);
    assert.deepEqual(t.llamadas, [['crearPrestamo', {
        tipo_prestamo: 'consumo', monto_prestamo: '5000.5', institucion_financiera: 'Banco Alfa', tasa_actual: 12.5,
        cuotas_totales: 24, cuotas_pendientes: 20, monto_cuota: '250', dia_pago: 15, saldo_actual: null,
        limite_credito: null,   // un consumo no lleva límite aunque el campo tenga algo
    }]]);
    assert.deepEqual(t.avisos, [{ mensaje: 'Financiamiento registrado con éxito.', tipo: undefined }]);
    assert.deepEqual(t.rutas, ['prestamos']);
});

test('alta de una línea: el límite viaja, las cuotas no, y un saldo declarado cuenta', async () => {
    const t = montar({ campos: CAMPOS_ALTA({ pre_tip: el({ value: 'flexible' }), pre_tot: el({ value: '' }), pre_pen: el({ value: '' }), pre_lim: el({ value: '1500' }), pre_sal: el({ value: '300' }) }) });
    await t.vista.handleAgregarPrestamo(t.evento);
    const d = t.llamadas[0][1];
    assert.deepEqual([d.tipo_prestamo, d.cuotas_totales, d.cuotas_pendientes, d.limite_credito, d.saldo_actual], ['flexible', null, null, '1500', '300']);
});

test('los importes del alta y de la edición conservan los dígitos escritos (sin pasar por número)', async () => {
    let t = montar({ campos: CAMPOS_ALTA({ pre_tip: el({ value: 'flexible' }), pre_mon: el({ value: ' 0075.250 ' }), pre_cuo: el({ value: '0010.500' }), pre_tot: el({ value: '' }), pre_pen: el({ value: '' }), pre_sal: el({ value: '0020.100' }), pre_lim: el({ value: '0300.700' }) }) });
    await t.vista.handleAgregarPrestamo(t.evento);
    let d = t.llamadas[0][1];
    assert.deepEqual([d.monto_prestamo, d.monto_cuota, d.saldo_actual, d.limite_credito], ['0075.250', '0010.500', '0020.100', '0300.700']);

    t = montar({ campos: CAMPOS_EDICION({ edp_cuo_3: el({ value: '0010.500' }), edp_lim_3: el({ value: ' 0300.700 ' }) }) });
    await t.vista.handleEdicionPrestamo(t.evento, 3);
    d = t.llamadas[0][1];
    assert.deepEqual([d.monto_cuota, d.limite_credito], ['0010.500', '0300.700']);
});

test('un saldo declarado en cero cuenta como cero, no como «no declarado»', async () => {
    const t = montar({ campos: CAMPOS_ALTA({ pre_tot: el({ value: '12' }), pre_pen: el({ value: '12' }), pre_sal: el({ value: '0' }) }) });
    await t.vista.handleAgregarPrestamo(t.evento);
    assert.equal(t.llamadas[0][1].saldo_actual, '0');
});

test('alta: si Rust rechaza se avisa el error y no se redibuja', async () => {
    const t = montar({ campos: CAMPOS_ALTA({ pre_tot: el({ value: '12' }), pre_pen: el({ value: '12' }) }), falla: 'Rechazado por prueba' });
    await t.vista.handleAgregarPrestamo(t.evento);
    assert.equal(t.avisos[0].tipo, 'error');
    assert.match(t.avisos[0].mensaje, /Rechazado por prueba/);
    assert.deepEqual(t.rutas, []);
});

// --- Edición de condiciones ---

test('editar condiciones: la ventana pide el límite solo a una línea y lista las tarjetas con sus fechas', async () => {
    const t = montar();
    await t.vista.abrirEdicionPrestamo(DATOS().prestamos[2]);
    assert.equal(t.modales[0].id, 'modal-pre-3');
    assert.match(t.modales[0].html, /id="edp_lim_3"/);
    assert.match(t.modales[0].html, /data-pago="8" data-corte="1" selected/);
    const u = montar();
    await u.vista.abrirEdicionPrestamo(DATOS().prestamos[0]);
    assert.doesNotMatch(u.modales[0].html, /edp_lim_1/);
});

test('vincular a una tarjeta explica que cede las fechas y desactiva el día propio', () => {
    const campos = {
        edp_tar_3: el({ value: '7', selectedOptions: [{ dataset: { corte: '1', pago: '8' } }] }),
        edp_aviso_3: el(), edp_dia_3: el(),
    };
    const t = montar({ campos });
    t.vista.avisarFechasDerivadas(3);
    assert.match(campos.edp_aviso_3.innerHTML, /corte día 1/);
    assert.match(campos.edp_aviso_3.innerHTML, /pago día 8/);
    assert.equal(campos.edp_dia_3.disabled, true);
    campos.edp_tar_3.value = '';
    t.vista.avisarFechasDerivadas(3);
    assert.equal(campos.edp_aviso_3.textContent, '');
    assert.equal(campos.edp_dia_3.disabled, false);
});

const CAMPOS_EDICION = (extra = {}) => ({
    edp_tas_3: el({ value: '14.5' }), edp_cuo_3: el({ value: '55' }), edp_dia_3: el({ value: '20' }), edp_tar_3: el({ value: '7' }),
    edp_lim_3: el({ value: '2000' }), 'modal-pre-3': el(), ...extra,
});

test('editar condiciones: envía texto y números, cierra la ventana y redibuja', async () => {
    const t = montar({ campos: CAMPOS_EDICION() });
    await t.vista.handleEdicionPrestamo(t.evento, 3);
    assert.deepEqual(t.llamadas, [['actualizarPrestamo', { id: 3, tasa_actual: 14.5, monto_cuota: '55', dia_pago: 20, limite_credito: '2000', tarjeta_id: 7 }]]);
    assert.equal(t.registro['modal-pre-3'].quitado, true);
    assert.deepEqual(t.avisos, [{ mensaje: 'Condiciones actualizadas.', tipo: undefined }]);
    assert.deepEqual(t.rutas, ['prestamos']);
});

test('editar condiciones: límite en blanco y sin tarjeta son «no declarado» y «ninguna», no cero', async () => {
    const t = montar({ campos: CAMPOS_EDICION({ edp_lim_3: el({ value: '' }), edp_tar_3: el({ value: '' }) }) });
    await t.vista.handleEdicionPrestamo(t.evento, 3);
    const d = t.llamadas[0][1];
    assert.deepEqual([d.limite_credito, d.tarjeta_id], [null, null]);
    // un amortizable no tiene campo de límite: tampoco viaja
    const u = montar({ campos: (() => { const c = CAMPOS_EDICION(); delete c.edp_lim_3; return c; })() });
    await u.vista.handleEdicionPrestamo(u.evento, 3);
    assert.equal(u.llamadas[0][1].limite_credito, null);
});

test('editar condiciones: si Rust rechaza, la ventana sigue abierta', async () => {
    const t = montar({ campos: CAMPOS_EDICION(), falla: 'No' });
    await t.vista.handleEdicionPrestamo(t.evento, 3);
    assert.equal(t.registro['modal-pre-3'].quitado, undefined);
    assert.equal(t.avisos[0].tipo, 'error');
    assert.deepEqual(t.rutas, []);
});

// --- Conciliar, abonar y eliminar ---

test('conciliar: pregunta con la estimación y el saldo por defecto, y envía el texto tal cual', async () => {
    const t = montar({ respuesta: ' 1,234.50 ' });
    await t.vista.handleDeclararSaldo(1, 1275);
    assert.match(t.preguntas[0].m, /DOP #1275#/);
    assert.equal(t.preguntas[0].d, '1275.00');
    assert.deepEqual(t.llamadas, [['declararSaldoPrestamo', 1, '1,234.50']]);
    assert.equal(typeof t.llamadas[0][2], 'string');
    assert.deepEqual(t.rutas, ['prestamos']);
});

test('conciliar: cancelar no envía nada; una forma inválida se rechaza con aviso', async () => {
    const a = montar({ respuesta: null });
    await a.vista.handleDeclararSaldo(1, 1000);
    assert.deepEqual([a.llamadas, a.avisos], [[], []]);
    for (const mala of ['', 'abc', '12.3.4', '1e5']) {
        const t = montar({ respuesta: mala });
        await t.vista.handleDeclararSaldo(1, 1000);
        assert.deepEqual(t.llamadas, [], mala);
        assert.equal(t.avisos[0].tipo, 'error', mala);
    }
});

test('conciliar: si Rust rechaza se avisa el error y no se redibuja', async () => {
    const t = montar({ falla: 'No', respuesta: '10' });
    await t.vista.handleDeclararSaldo(1, 1000);
    assert.equal(t.avisos[0].tipo, 'error');
    assert.deepEqual(t.rutas, []);
});

test('eliminar: sin confirmar no envía nada; confirmado, envía el id y redibuja; si Rust rechaza, avisa', async () => {
    const no = montar({ confirmar: false });
    await no.vista.handleEliminarPrestamo(2);
    assert.deepEqual([no.llamadas, no.avisos, no.rutas], [[], [], []]);
    assert.equal(no.confirmaciones.length, 1);
    const t = montar();
    await t.vista.handleEliminarPrestamo(2);
    assert.deepEqual(t.llamadas, [['eliminarPrestamo', 2]]);
    assert.deepEqual(t.rutas, ['prestamos']);
    const u = montar({ falla: 'No' });
    await u.vista.handleEliminarPrestamo(2);
    assert.equal(u.avisos[0].tipo, 'error');
    assert.deepEqual(u.rutas, []);
});

// --- El menú de acciones de cada fila ---

const eventoDeMenu = () => ({
    parada: false,
    stopPropagation() { this.parada = true; },
    currentTarget: { getBoundingClientRect: () => ({ right: 300, top: 50, bottom: 80 }) },
});

test('abrir el menú: detiene el clic, lo ancla al botón y ofrece pagar y ver movimientos a todo financiamiento', () => {
    const t = montar();
    const ev = eventoDeMenu();
    t.vista.abrirMenuPasivo(ev, DATOS().prestamos[0]);
    assert.equal(ev.parada, true);
    assert.equal(t.menus.length, 1);
    assert.equal(t.menus[0].id, 'menu-pasivo');
    assert.deepEqual(t.menus[0].ancla, { right: 300, top: 50, bottom: 80 });
    // También sin cuotas pendientes: un abono a capital sigue siendo posible.
    t.vista.abrirMenuPasivo(eventoDeMenu(), DATOS().prestamos[3]);
    for (const a of ['pagar', 'movimientos', 'conciliar', 'editar', 'eliminar']) {
        for (const m of t.menus) assert.match(m.html, new RegExp(`data-opcion="${a}"`), a);
    }
    assert.doesNotMatch(t.menus[0].html, /data-opcion="abonar"/, 'el botón que no sacaba dinero de ninguna cuenta ya no está');
});

test('cada renglón del menú hace su acción sobre ese financiamiento', async () => {
    const t = montar({ respuesta: '700' });
    const p = DATOS().prestamos[0];
    const elegir = async accion => { t.vista.abrirMenuPasivo(eventoDeMenu(), p); t.menus.at(-1).alElegir(accion); await new Promise(r => setImmediate(r)); };
    await elegir('pagar');
    assert.equal(t.modales.at(-1).id, 'modal-pago-1');
    await elegir('movimientos');
    assert.equal(t.modales.at(-1).id, 'modal-mov-1');
    await elegir('conciliar');
    assert.deepEqual(t.llamadas.at(-1), ['declararSaldoPrestamo', 1, '700']);
    assert.match(t.preguntas.at(-1).m, /DOP #1000#/);
    await elegir('editar');
    assert.equal(t.modales.at(-1).id, 'modal-pre-1');
    await elegir('eliminar');
    assert.deepEqual(t.llamadas.at(-1), ['eliminarPrestamo', 1]);
    await elegir('algo desconocido');
    assert.equal(t.llamadas.filter(l => !l[0].startsWith('obtener')).length, 2);
});

// --- El puente ---

test('el puente expone todos los manejadores y delega en la vista', async () => {
    const t = montar({ campos: CAMPOS_ALTA({ pre_tot: el({ value: '12' }), pre_pen: el({ value: '12' }) }) });
    const puente = puentePrestamos(t.vista);
    assert.deepEqual(Object.keys(puente).sort(), [...MANEJADORES_PRESTAMOS].sort());
    await puente.handleAgregarPrestamo(t.evento);
    assert.equal(t.llamadas.length, 1);
});

test('todo appUI.x( que escribe la plantilla y su ventana está en el puente', async () => {
    const t = montar();
    await t.vista.render();
    await t.vista.abrirEdicionPrestamo(DATOS().prestamos[2]);
    const html = t.pantalla.contenido.innerHTML + t.modales[0].html;
    const llamados = nombresDelPuente(html);
    assert.ok(llamados.size >= 4, [...llamados].join());
    for (const nombre of llamados) assert.ok(MANEJADORES_PRESTAMOS.includes(nombre), `falta ${nombre} en el puente`);
});

// --- texto del titular en el HTML y en los manejadores ---

test('un préstamo con institución hostil llega íntegro al menú (objeto) y no se interpreta', async () => {
    const hostil = `x'); alert(1); //"\\ <img src=x onerror="alert(2)"> &amp;`;
    const p = prestamo({ id: 21, institucion_financiera: hostil });
    const t = montar({ datos: { prestamos: [p], tarjetas: [tarjeta({ entidad: hostil, nombre_tarjeta: hostil })] } });
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    const [[, recibido]] = llamadasDelManejador(html, 'abrirMenuPasivo');
    assert.deepEqual(recibido, p);
    assert.doesNotMatch(html, /<img/);
});

test('la ventana de edición de un préstamo escapa la institución y las tarjetas', async () => {
    const hostil = `x'); alert(1); //"\\ <img src=x onerror="alert(2)"> &amp;`;
    const t = montar({ datos: { prestamos: [], tarjetas: [tarjeta({ entidad: hostil, nombre_tarjeta: hostil })] } });
    await t.vista.abrirEdicionPrestamo(prestamo({ institucion_financiera: hostil }));
    assert.doesNotMatch(t.modales[0].html, /<img/);
});

// --- Pagar un préstamo desde una cuenta ---

/** Los campos del formulario de pago del préstamo 1, con lo que escribiría el titular. */
const camposDePago = (extra = {}) => ({
    pag_tip_1: el({ value: 'cuota' }),
    pag_mon_1: el({ value: '100.00' }),
    pag_int_1: el({ value: '' }),
    pag_grupo_int_1: el(),
    pag_fec_1: el({ value: ' 05/03/2027 ' }),
    pag_ori_1: el({ value: '5', selectedOptions: [{ textContent: ' Ahorro <Alfa> — Banco Alfa ' }] }),
    pag_cat_1: el({ value: '9' }),
    pag_prev_1: el(),
    ...extra,
});

test('la categoría propuesta es la que se llama «Préstamo(s)», sin distinguir tildes ni mayúsculas', () => {
    const c = (id, nombre) => ({ id, nombre, fija: false, motivo_sistema: null });
    assert.equal(categoriaDePrestamo([c(1, 'Otros'), c(2, ' PRESTAMOS ')])?.id, 2);
    assert.equal(categoriaDePrestamo([c(1, 'Préstamo')])?.id, 1);
    assert.equal(categoriaDePrestamo([c(1, 'Préstamo hipotecario'), c(2, 'Otros')]), null);
    assert.equal(categoriaDePrestamo([]), null);
});

test('abrir el pago: propone la cuota, hoy y la categoría Préstamo; ofrece solo cuentas en pesos y la caja', async () => {
    const t = montar({ campos: camposDePago() });
    await t.vista.abrirPagoPrestamo(DATOS().prestamos[0]);
    const { id, html } = t.modales[0];
    assert.equal(id, 'modal-pago-1');
    assert.match(html, /id="pag_mon_1"[^>]*value="100.00"/);
    assert.match(html, /value="05\/03\/2027"/);
    assert.match(html, /data-mascara="fecha"/);
    assert.match(html, /<option value="5">Ahorro &lt;Alfa&gt; — Banco Alfa<\/option>/);
    assert.match(html, /<option value="8">Nómina<\/option>/);
    assert.doesNotMatch(html, /Cuenta Dólar/);
    assert.match(html, /<option value="efectivo">/);
    assert.match(html, /<option value="9" selected>Préstamo<\/option>/);
    assert.match(html, /data-accion-enviar="handleRegistrarPagoPrestamo"/);
    // Al abrir ya calcula la vista previa con lo propuesto.
    assert.deepEqual(t.llamadas.at(-1), ['previsualizarPagoPrestamo', 1, 'cuota', '100.00', null]);
});

test('abrir el pago: si no se pueden leer las cuentas avisa y no abre nada', async () => {
    const t = montar();
    t.vista.dep.api.obtenerCuentas = async () => { throw new Error('sin base'); };
    await t.vista.abrirPagoPrestamo(DATOS().prestamos[0]);
    assert.deepEqual(t.modales, []);
    assert.equal(t.avisos[0].tipo, 'error');
});

test('vista previa: muestra interés, capital, saldo y cuotas con escape; un abono a capital oculta y no envía el interés', async () => {
    const t = montar({ campos: camposDePago({ pag_int_1: el({ value: '1,000.07' }), pag_mon_1: el({ value: '2,000.50' }) }) });
    await t.vista.cambiarPagoPrestamo(1);
    assert.deepEqual(t.llamadas.at(-1), ['previsualizarPagoPrestamo', 1, 'cuota', '2000.50', '1000.07']);
    const salida = t.registro.pag_prev_1.innerHTML;
    assert.match(salida, /Interés DOP #10.37# \(estimado\)/);
    assert.match(salida, /Capital DOP #89.63#/);
    assert.match(salida, /DOP #987.41# → <strong>DOP #897.78#<\/strong>/);
    assert.match(salida, /Quedarán 4 cuotas/);
    assert.equal(t.registro.pag_grupo_int_1.style.display, '');

    t.registro.pag_tip_1.value = 'abono_capital';
    await t.vista.cambiarPagoPrestamo(1);
    assert.equal(t.registro.pag_grupo_int_1.style.display, 'none');
    assert.deepEqual(t.llamadas.at(-1), ['previsualizarPagoPrestamo', 1, 'abono_capital', '2000.50', null]);
});

test('vista previa: sin cuotas que contar no dice cuántas quedan', async () => {
    const t = montar({ campos: camposDePago(), previa: async () => ({ ...PREVIA, interes_estimado: false, cuotas_pendientes_despues: null }) });
    await t.vista.cambiarPagoPrestamo(1);
    assert.doesNotMatch(t.registro.pag_prev_1.innerHTML, /Quedarán|estimado/);
});

test('vista previa: un importe ilegible o un error de Rust se explican en el recuadro, sin avisos', async () => {
    for (const [campos, patron, llama] of [
        [{ pag_mon_1: el({ value: 'abc' }) }, /Escribe el importe/, false],
        [{ pag_int_1: el({ value: '1.2.3' }) }, /interés debe ser un importe/, false],
    ]) {
        const t = montar({ campos: camposDePago(campos) });
        await t.vista.cambiarPagoPrestamo(1);
        assert.match(t.registro.pag_prev_1.textContent, patron);
        assert.equal(t.llamadas.some(l => l[0] === 'previsualizarPagoPrestamo'), llama);
        assert.deepEqual(t.avisos, []);
    }
    const t = montar({ campos: camposDePago(), previa: async () => { throw new Error('El préstamo ya no tiene cuotas pendientes'); } });
    await t.vista.cambiarPagoPrestamo(1);
    assert.match(t.registro.pag_prev_1.textContent, /ya no tiene cuotas/);
    assert.deepEqual(t.avisos, []);
});

test('vista previa: una respuesta que llega después de otra más reciente se descarta', async () => {
    let soltarPrimera;
    const respuestas = [new Promise(r => { soltarPrimera = () => r({ ...PREVIA, capital: '1.11' }); }), Promise.resolve({ ...PREVIA, capital: '2.22' })];
    const t = montar({ campos: camposDePago(), previa: () => respuestas.shift() });
    const primera = t.vista.cambiarPagoPrestamo(1);
    await t.vista.cambiarPagoPrestamo(1);
    soltarPrimera();
    await primera;
    assert.match(t.registro.pag_prev_1.innerHTML, /Capital DOP #2.22#/);
});

test('registrar el pago: confirma con importe, fecha y origen, y envía cada campo con su tipo', async () => {
    const t = montar({ campos: camposDePago({ pag_int_1: el({ value: ' 12.34 ' }) }) });
    t.registro['modal-pago-1'] = el();
    await t.vista.handleRegistrarPagoPrestamo(t.evento, 1);
    assert.equal(t.evento.evitado, 1);
    assert.match(t.confirmaciones[0], /la cuota de DOP #100# del 05\/03\/2027, pagada desde «Ahorro <Alfa> — Banco Alfa» \(más la retención/);
    assert.deepEqual(t.llamadas, [['registrarPagoPrestamo', {
        prestamo_id: 1, tipo: 'cuota', monto: '100.00', interes: '12.34', fecha: '05/03/2027', cuenta_id: 5, categoria_id: 9,
    }]]);
    assert.deepEqual(t.avisos, [{ mensaje: 'Pago registrado. Saldo del préstamo: DOP #897.78#.', tipo: undefined }]);
    assert.equal(t.registro['modal-pago-1'].quitado, true);
    assert.deepEqual(t.rutas, ['prestamos']);
});

test('registrar el pago: desde la caja va sin cuenta, y un abono a capital no lleva interés aunque se haya escrito', async () => {
    const t = montar({ campos: camposDePago({ pag_tip_1: el({ value: 'abono_capital' }), pag_int_1: el({ value: '5' }), pag_ori_1: el({ value: 'efectivo' }) }) });
    await t.vista.handleRegistrarPagoPrestamo(t.evento, 1);
    assert.match(t.confirmaciones[0], /el abono a capital de DOP #100# .* desde la caja de efectivo/);
    const enviado = t.llamadas[0][1];
    assert.deepEqual([enviado.tipo, enviado.interes, enviado.cuenta_id], ['abono_capital', null, null]);
});

test('registrar el pago: sin confirmar no envía nada; un importe ilegible ni pregunta', async () => {
    const no = montar({ campos: camposDePago(), confirmar: false });
    await no.vista.handleRegistrarPagoPrestamo(no.evento, 1);
    assert.deepEqual([no.llamadas, no.avisos, no.rutas], [[], [], []]);
    const malo = montar({ campos: camposDePago({ pag_mon_1: el({ value: '' }) }) });
    await malo.vista.handleRegistrarPagoPrestamo(malo.evento, 1);
    assert.deepEqual([malo.llamadas, malo.confirmaciones], [[], []]);
    assert.equal(malo.avisos[0].tipo, 'error');
});

test('registrar el pago: si Rust rechaza avisa, deja el formulario abierto y no redibuja', async () => {
    const t = montar({ campos: camposDePago(), falla: 'La cuenta está archivada' });
    t.registro['modal-pago-1'] = el();
    await t.vista.handleRegistrarPagoPrestamo(t.evento, 1);
    assert.deepEqual(t.avisos, [{ mensaje: 'Error: La cuenta está archivada', tipo: 'error' }]);
    assert.notEqual(t.registro['modal-pago-1'].quitado, true);
    assert.deepEqual(t.rutas, []);
});

test('movimientos: una fila por movimiento con su tipo; solo el último, y solo si salió de una cuenta, se puede revertir', async () => {
    const t = montar();
    await t.vista.abrirMovimientosPrestamo(prestamo({ institucion_financiera: 'Banco <Raro>' }));
    const { id, html } = t.modales[0];
    assert.equal(id, 'modal-mov-1');
    assert.match(html, /Banco &lt;Raro&gt;/);
    assert.match(html, /Cuota <span[^>]*>· cuenta<\/span>/);
    assert.match(html, /Conciliación/);
    assert.equal((html.match(/handleRevertirPagoPrestamo/g) ?? []).length, 1);
    assert.match(html, /data-accion="handleRevertirPagoPrestamo" data-args="\[30,1\]"/);

    const u = montar({ movimientos: [movimiento({ tipo: 'declaracion', gasto_id: null }), movimiento({ id: 20 })] });
    await u.vista.abrirMovimientosPrestamo(prestamo());
    assert.doesNotMatch(u.modales[0].html, /handleRevertirPagoPrestamo/, 'el último es una conciliación: nada que revertir');

    const v = montar({ movimientos: [] });
    await v.vista.abrirMovimientosPrestamo(prestamo());
    assert.match(v.modales[0].html, /Todavía no hay movimientos/);
});

test('revertir un pago: confirma, pide el motivo, envía y dice el caso; cierra el libro y redibuja', async () => {
    const t = montar();
    t.registro['modal-mov-1'] = el();
    await t.vista.handleRevertirPagoPrestamo(30, 1);
    assert.equal(t.confirmaciones.length, 1);
    assert.equal(t.motivos.length, 1);
    assert.deepEqual(t.llamadas, [['revertirPagoPrestamo', 30, 'Pago registrado dos veces']]);
    assert.deepEqual(t.avisos, [{ mensaje: 'Pago revertido. Caso COR-2027-0004.', tipo: undefined }]);
    assert.equal(t.registro['modal-mov-1'].quitado, true);
    assert.deepEqual(t.rutas, ['prestamos']);
});

test('revertir un pago: sin confirmar o sin motivo no envía nada; si Rust rechaza, avisa', async () => {
    const no = montar({ confirmar: false });
    await no.vista.handleRevertirPagoPrestamo(30, 1);
    assert.deepEqual([no.llamadas, no.motivos, no.avisos], [[], [], []]);
    const sinMotivo = montar({ motivo: null });
    await sinMotivo.vista.handleRevertirPagoPrestamo(30, 1);
    assert.deepEqual([sinMotivo.llamadas, sinMotivo.avisos, sinMotivo.rutas], [[], [], []]);
    const u = montar({ falla: 'Solo se puede revertir el último movimiento' });
    await u.vista.handleRevertirPagoPrestamo(30, 1);
    assert.equal(u.avisos[0].tipo, 'error');
    assert.deepEqual(u.rutas, []);
});
