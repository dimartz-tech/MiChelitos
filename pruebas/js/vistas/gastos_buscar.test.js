// Pestaña Gastos: autocompletado de la descripción, «repetir último» y búsqueda/filtros del historial.
// Datos inventados (el repositorio es público).

import test from 'node:test';
import assert from 'node:assert/strict';
import { accionesDe } from '../ayudas/manejadores_html.js';
import {
    VistaGastos, MANEJADORES_GASTOS, puenteGastos,
    normalizarBusqueda, sugerenciasDeDescripcion, ultimoGastoConDescripcion, planDeRepeticion,
    filtrarGastosListados, totalesEnCentavos, SIN_FILTROS,
} from '../../../src/js/vistas/gastos.js';
import { cargarInterfaz } from '../ayudas/cargar_interfaz.js';

let seq = 0;
const gasto = (extra = {}) => ({
    id: ++seq, fecha: '10/03/2027', monto: 100, divisa: 'DOP', descripcion: 'Gasto de prueba', categoria_id: 1, categoria_nombre: 'Comida',
    metodo_pago: 'efectivo', costo_adicional: 0, tarjeta_id: null, cuenta_ahorro_id: null,
    estado_conversion: null, monto_liquidado: null, tasa_conversion: null, ...extra,
});
const CATEGORIAS = [{ id: 1, nombre: 'Comida' }, { id: 2, nombre: 'Transporte' }, { id: 3, nombre: 'Café y Té' }];
const TARJETAS = [{ id: 7, entidad: 'Banco Alfa', nombre_tarjeta: 'Oro' }];
const CUENTAS = [{ id: 4, nombre: 'Cuenta Pesos', divisa: 'DOP', balance_actual: 1000 }];

const el = (spec = {}) => ({ value: '', checked: false, textContent: '', innerHTML: '', style: {}, ...spec });

function montar({ gastos, tarjetas = TARJETAS, cuentas = CUENTAS, campos = {} } = {}) {
    const llamadas = [];
    const pantalla = { contenido: { innerHTML: '' } };
    const api = {
        obtenerGastos: async () => gastos,
        obtenerCategorias: async () => CATEGORIAS,
        obtenerCuentas: async () => cuentas,
        obtenerTarjetas: async () => tarjetas,
        crearGasto: async d => { llamadas.push(d); },
        liquidarConsumoPendiente: async () => 1,
    };
    const todos = {
        gas_des: el(), gas_repetir: el(), gas_cat: el(), gas_div: el({ value: 'DOP' }), gas_met: el({ value: 'efectivo' }),
        gas_tar: el(), gas_cue: el(), gas_mon: el(), gas_f_txt: el(), gas_f_cat: el(), gas_f_met: el(), gas_f_per: el({ value: 'todos' }),
        gas_historial_lista: el(), gas_tarjeta_container: el(), gas_cuenta_container: el(), gas_lbtr_container: el(),
        gas_conversion_container: el(), gas_conversion_previa: el(), ...campos,
    };
    const vista = new VistaGastos({
        api, avisos: { mostrar() {} }, formato: { importe: v => `#${v}#` },
        enrutador: { mostrar: async () => {} }, pantalla,
        dom: { elemento: id => todos[id], buscar: id => todos[id] ?? null },
        modales: { abrir() {} }, ahora: () => new Date(2027, 2, 5),
    });
    return { vista, campos: todos, pantalla, llamadas, lista: () => todos.gas_historial_lista.innerHTML };
}

const escribir = (t, id, valor) => { t.campos[id].value = valor; };

// --- normalización y sugerencias ---

test('normalizarBusqueda: sin mayúsculas, tildes ni espacios sobrantes', () => {
    assert.equal(normalizarBusqueda('  CAFÉ   Ñandú  '), 'cafe nandu');
    assert.equal(normalizarBusqueda(null), '');
});

test('sugerencias: únicas sin distinguir mayúsculas, por frecuencia y, a igualdad, por recencia; con la grafía más reciente', () => {
    const g = [
        gasto({ descripcion: 'almuerzo', fecha: '01/03/2027' }),
        gasto({ descripcion: 'Almuerzo', fecha: '05/03/2027' }),
        gasto({ descripcion: 'ALMUERZO ', fecha: '02/03/2027' }),
        gasto({ descripcion: 'Taxi', fecha: '01/02/2027' }),
        gasto({ descripcion: 'Libro', fecha: '03/03/2027' }),
        gasto({ descripcion: 'Taxi', fecha: '28/01/2027' }),
        gasto({ descripcion: 'Pan', fecha: '04/03/2027' }),
        gasto({ descripcion: '   ' }),
    ];
    // Almuerzo ×3, Taxi ×2; con 1 uso: Pan (04/03) antes que Libro (03/03).
    assert.deepEqual(sugerenciasDeDescripcion(g), ['Almuerzo', 'Taxi', 'Pan', 'Libro']);
});

test('sugerencias: a igual frecuencia y fecha manda el id mayor; las fechas se comparan por año, no como texto', () => {
    const g = [gasto({ descripcion: 'Uno', fecha: '31/12/2026' }), gasto({ descripcion: 'Dos', fecha: '01/01/2027' })];
    assert.deepEqual(sugerenciasDeDescripcion(g), ['Dos', 'Uno']);
});

test('el formulario dibuja un datalist con las sugerencias y las escapa', async () => {
    const hostil = `x"><img src=x onerror=alert(1)> '`;
    const t = montar({ gastos: [gasto({ descripcion: hostil }), gasto({ descripcion: 'Pan' })] });
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.match(html, /<input type="text" id="gas_des"[^>]*list="gas_des_sugerencias"/);
    assert.match(html, /<datalist id="gas_des_sugerencias">/);
    assert.match(html, /<option value="Pan"><\/option>/);
    assert.doesNotMatch(html, /<img/);
    assert.match(html, /<option value="x&quot;&gt;&lt;img src=x onerror=alert\(1\)&gt; &#39;"><\/option>/);
});

// --- repetir último ---

test('el último gasto con la descripción es el de fecha mayor, sin distinguir mayúsculas', () => {
    const a = gasto({ descripcion: 'Taxi', fecha: '01/03/2027', categoria_id: 1 });
    const b = gasto({ descripcion: 'taxi', fecha: '09/03/2027', categoria_id: 2 });
    const c = gasto({ descripcion: 'Taxi', fecha: '15/02/2027', categoria_id: 3 });
    assert.equal(ultimoGastoConDescripcion([a, b, c], ' TAXI '), b);
    assert.equal(ultimoGastoConDescripcion([a, b, c], 'Tax'), null);
    assert.equal(ultimoGastoConDescripcion([a, b, c], ''), null);
});

test('plan de repetición: efectivo, tarjeta y cuenta activas se rellenan; lo inactivo o inexistente se omite y se dice', () => {
    const cat = { categorias: CATEGORIAS, tarjetas: TARJETAS, cuentas: CUENTAS };
    const ef = planDeRepeticion(gasto({ categoria_id: 2, divisa: 'USD' }), cat);
    assert.deepEqual([ef.categoriaId, ef.metodo, ef.divisa, ef.omitidos], [2, 'efectivo', 'USD', []]);

    const tar = planDeRepeticion(gasto({ metodo_pago: 'tarjeta', tarjeta_id: 7 }), cat);
    assert.deepEqual([tar.metodo, tar.tarjetaId], ['tarjeta', 7]);
    const tarInactiva = planDeRepeticion(gasto({ metodo_pago: 'tarjeta', tarjeta_id: 99 }), cat);
    assert.equal(tarInactiva.metodo, null);
    assert.equal(tarInactiva.tarjetaId, null);
    assert.match(tarInactiva.omitidos.join(), /tarjeta.*ya no está activa/);

    const cue = planDeRepeticion(gasto({ metodo_pago: 'transferencia', cuenta_ahorro_id: 4 }), cat);
    assert.deepEqual([cue.metodo, cue.cuentaId], ['transferencia', 4]);
    const cueArchivada = planDeRepeticion(gasto({ metodo_pago: 'transferencia', cuenta_ahorro_id: 55 }), cat);
    assert.equal(cueArchivada.metodo, null);
    assert.match(cueArchivada.omitidos.join(), /cuenta.*ya no está activa/);

    const sinCat = planDeRepeticion(gasto({ categoria_id: 42 }), cat);
    assert.equal(sinCat.categoriaId, null);
    assert.match(sinCat.omitidos.join(), /categoría/);

    const raro = planDeRepeticion(gasto({ metodo_pago: 'cheque' }), cat);
    assert.equal(raro.metodo, null);
    assert.equal(raro.omitidos.length, 1);
});

test('escribir una descripción nueva no ofrece nada; una existente ofrece pero NO rellena ningún campo', async () => {
    const t = montar({ gastos: [gasto({ descripcion: 'Taxi', categoria_id: 2, metodo_pago: 'tarjeta', tarjeta_id: 7, monto: 250.5 })] });
    await t.vista.render();
    escribir(t, 'gas_des', 'Otra cosa');
    t.vista.ofrecerUltimoGasto();
    assert.equal(t.campos.gas_repetir.style.display, 'none');

    escribir(t, 'gas_des', 'taxi');
    t.vista.ofrecerUltimoGasto();
    assert.equal(t.campos.gas_repetir.style.display, 'block');
    assert.match(t.campos.gas_repetir.innerHTML, /Transporte/);
    assert.match(t.campos.gas_repetir.innerHTML, /Banco Alfa - Oro/);
    // Nada tocado hasta que el titular pulse el botón.
    assert.equal(t.campos.gas_cat.value, '');
    assert.equal(t.campos.gas_met.value, 'efectivo');
    assert.equal(t.campos.gas_mon.value, '');
    assert.deepEqual(t.llamadas, []);
    // Los botones son acciones delegadas, no atributos en línea.
    assert.deepEqual(accionesDe(t.campos.gas_repetir.innerHTML).map(a => a.nombre), ['aplicarUltimoGasto', 'usarUltimoMonto']);
    assert.doesNotMatch(t.campos.gas_repetir.innerHTML, /\son\w+=/);

    escribir(t, 'gas_des', '');
    t.vista.ofrecerUltimoGasto();
    assert.equal(t.campos.gas_repetir.style.display, 'none');
});

test('«Rellenar» pone categoría, método con su tarjeta y divisa, pero nunca el monto, y no guarda nada', async () => {
    const t = montar({ gastos: [gasto({ descripcion: 'Taxi', categoria_id: 2, metodo_pago: 'tarjeta', tarjeta_id: 7, divisa: 'USD', monto: 33 })] });
    await t.vista.render();
    escribir(t, 'gas_des', 'Taxi');
    t.vista.ofrecerUltimoGasto();
    t.vista.aplicarUltimoGasto();
    assert.deepEqual(
        [t.campos.gas_cat.value, t.campos.gas_met.value, t.campos.gas_tar.value, t.campos.gas_div.value, t.campos.gas_mon.value],
        ['2', 'tarjeta', '7', 'USD', ''],
    );
    assert.equal(t.campos.gas_tarjeta_container.style.display, 'block', 'se muestran los campos del método');
    assert.equal(t.campos.gas_des.value, 'Taxi');
    assert.deepEqual(t.llamadas, []);
});

test('«Rellenar» con una tarjeta desactivada o una cuenta archivada deja el método como estaba', async () => {
    const t = montar({ gastos: [gasto({ descripcion: 'Taxi', categoria_id: 2, metodo_pago: 'tarjeta', tarjeta_id: 99 })] });
    await t.vista.render();
    escribir(t, 'gas_des', 'Taxi');
    t.vista.ofrecerUltimoGasto();
    assert.match(t.campos.gas_repetir.innerHTML, /ya no está activa/);
    t.vista.aplicarUltimoGasto();
    assert.equal(t.campos.gas_cat.value, '2');
    assert.equal(t.campos.gas_met.value, 'efectivo');
    assert.equal(t.campos.gas_tar.value, '');

    const u = montar({ gastos: [gasto({ descripcion: 'Pago', metodo_pago: 'transferencia', cuenta_ahorro_id: 55 })], cuentas: [] });
    await u.vista.render();
    escribir(u, 'gas_des', 'Pago');
    u.vista.ofrecerUltimoGasto();
    u.vista.aplicarUltimoGasto();
    assert.equal(u.campos.gas_met.value, 'efectivo');
    assert.equal(u.campos.gas_cue.value, '');
});

test('«Usar el último monto» solo pone el monto cuando se pulsa', async () => {
    const t = montar({ gastos: [gasto({ descripcion: 'Taxi', monto: 250.5 })] });
    await t.vista.render();
    escribir(t, 'gas_des', 'Taxi');
    t.vista.ofrecerUltimoGasto();
    assert.match(t.campos.gas_repetir.innerHTML, /DOP #250.5#/);
    assert.equal(t.campos.gas_mon.value, '');
    t.vista.usarUltimoMonto();
    assert.equal(t.campos.gas_mon.value, '250.5');
});

test('sin oferta vigente, los botones no hacen nada', async () => {
    const t = montar({ gastos: [gasto({ descripcion: 'Taxi' })] });
    await t.vista.render();
    t.vista.aplicarUltimoGasto();
    t.vista.usarUltimoMonto();
    assert.equal(t.campos.gas_cat.value, '');
    assert.equal(t.campos.gas_mon.value, '');
});

test('la oferta escapa descripción, categoría y tarjeta del titular', async () => {
    const hostil = `<img src=x onerror=alert(1)>`;
    const t = montar({
        gastos: [gasto({ descripcion: hostil, metodo_pago: 'tarjeta', tarjeta_id: 7, categoria_id: 1 })],
        tarjetas: [{ id: 7, entidad: hostil, nombre_tarjeta: '"x"' }],
    });
    await t.vista.render();
    escribir(t, 'gas_des', hostil);
    t.vista.ofrecerUltimoGasto();
    assert.doesNotMatch(t.campos.gas_repetir.innerHTML, /<img/);
    assert.match(t.campos.gas_repetir.innerHTML, /&lt;img src=x onerror=alert\(1\)&gt;/);
});

// --- búsqueda y filtros ---

const HISTORIAL = () => [
    gasto({ descripcion: 'Café con leche', categoria_id: 3, categoria_nombre: 'Café y Té', fecha: '02/03/2027', monto: 0.1 }),
    gasto({ descripcion: 'Taxi al centro', categoria_id: 2, categoria_nombre: 'Transporte', fecha: '03/03/2027', monto: 0.2 }),
    gasto({ descripcion: 'Taxi de regreso', categoria_id: 2, categoria_nombre: 'Transporte', fecha: '20/02/2027', metodo_pago: 'tarjeta', tarjeta_id: 7, monto: 5 }),
    gasto({ descripcion: 'Pan', categoria_id: 1, categoria_nombre: 'Comida', fecha: '04/03/2027', monto: 1 }),
];

test('el texto busca en descripción y categoría, sin distinguir mayúsculas ni tildes', () => {
    const h = HISTORIAL();
    const r = (texto) => filtrarGastosListados(h, { ...SIN_FILTROS, texto }, '03/2027').map(g => g.descripcion);
    assert.deepEqual(r('CAFE'), ['Café con leche']);
    assert.deepEqual(r('Y TÉ'),['Café con leche']);
    assert.deepEqual(r('transporte'), ['Taxi al centro', 'Taxi de regreso']);
    assert.deepEqual(r('taxi centro'), [], 'es una frase contigua, no palabras sueltas');
    assert.deepEqual(r('   '), h.map(g => g.descripcion));
});

test('texto, categoría, método y periodo se combinan (todos deben cumplirse)', () => {
    const h = HISTORIAL();
    const r = (f) => filtrarGastosListados(h, { ...SIN_FILTROS, ...f }, '03/2027').map(g => g.descripcion);
    assert.deepEqual(r({ categoria: '2' }), ['Taxi al centro', 'Taxi de regreso']);
    assert.deepEqual(r({ categoria: '2', metodo: 'tarjeta' }), ['Taxi de regreso']);
    assert.deepEqual(r({ categoria: '2', periodo: 'mes' }), ['Taxi al centro']);
    assert.deepEqual(r({ texto: 'taxi', categoria: '2', metodo: 'efectivo', periodo: 'mes' }), ['Taxi al centro']);
    assert.deepEqual(r({ texto: 'pan', categoria: '2' }), []);
    assert.deepEqual(r({ periodo: 'mes' }).length, 3);
});

test('totalesEnCentavos suma por divisa sin deriva de f64 y respeta liquidados', () => {
    const g = [
        gasto({ monto: 0.1, costo_adicional: 0 }), gasto({ monto: 0.2, costo_adicional: 0 }), gasto({ monto: 0.7, costo_adicional: 0.1 }),
        gasto({ divisa: 'USD', monto: 10, costo_adicional: 0.05 }),
        gasto({ divisa: 'USD', monto: 10, costo_adicional: 0, estado_conversion: 'liquidado', monto_liquidado: 600.25 }),
    ];
    // 0.1 + 0.2 + 0.7 + 0.1 = 1.1 exacto (110 centavos); liquidado cuenta en DOP.
    assert.deepEqual(totalesEnCentavos(g), { DOP: 110 + 60025, USD: 1005 });
});

test('la lista muestra «N de M gastos», el total mostrado y se redibuja sin tocar el formulario', async () => {
    const t = montar({ gastos: HISTORIAL() });
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.match(html, /<strong>4<\/strong> de 4 gastos/);
    assert.match(html, /id="gas_f_txt"[^>]*data-accion-escribir="filtrarGastos"/);
    assert.match(html, /id="gas_f_cat"[^>]*data-accion-cambiar="filtrarGastos"/);
    assert.match(html, /data-accion="limpiarFiltrosGastos"/);
    assert.doesNotMatch(html, /\son(click|change|input|submit)=/);

    escribir(t, 'gas_f_txt', 'TAXI');
    t.vista.filtrarGastos();
    assert.match(t.lista(), /<strong>2<\/strong> de 4 gastos/);
    assert.match(t.lista(), /Total mostrado: DOP #5.2#/);
    assert.match(t.lista(), /no cambia con estos filtros/);
    assert.doesNotMatch(t.lista(), /Café con leche/);
    // La pestaña entera no se redibujó (el formulario conserva lo escrito).
    assert.equal(t.pantalla.contenido.innerHTML, html);
});

test('con periodo «solo el mes» el total M es el del mes; sin coincidencias se dice', async () => {
    const t = montar({ gastos: HISTORIAL() });
    await t.vista.render();
    escribir(t, 'gas_f_per', 'mes');
    t.vista.filtrarGastos();
    assert.match(t.lista(), /<strong>3<\/strong> de 3 gastos/);
    escribir(t, 'gas_f_txt', 'zzz');
    t.vista.filtrarGastos();
    assert.match(t.lista(), /<strong>0<\/strong> de 3 gastos/);
    assert.match(t.lista(), /Ningún gasto coincide/);
});

test('«Limpiar» restablece los controles y la lista', async () => {
    const t = montar({ gastos: HISTORIAL() });
    await t.vista.render();
    Object.assign(t.campos.gas_f_txt, { value: 'pan' });
    t.campos.gas_f_cat.value = '1'; t.campos.gas_f_met.value = 'tarjeta'; t.campos.gas_f_per.value = 'mes';
    t.vista.filtrarGastos();
    t.vista.limpiarFiltrosGastos();
    assert.deepEqual([t.campos.gas_f_txt.value, t.campos.gas_f_cat.value, t.campos.gas_f_met.value, t.campos.gas_f_per.value], ['', '', '', 'todos']);
    assert.match(t.lista(), /<strong>4<\/strong> de 4 gastos/);
    assert.doesNotMatch(t.lista(), /no cambia con estos filtros/);
});

test('los filtros sobreviven a cambiar de mes y el resumen del mes no depende de ellos', async () => {
    const t = montar({ gastos: HISTORIAL() });
    await t.vista.render();
    escribir(t, 'gas_f_txt', 'taxi');
    t.vista.filtrarGastos();
    await t.vista.handleSelectGastosMonth('02/2027');
    const html = t.pantalla.contenido.innerHTML;
    assert.match(html, /id="gas_f_txt"[^>]*value="taxi"/);
    assert.match(html, /<strong>2<\/strong> de 4 gastos/);
    assert.match(html, /Resumen de Febrero 2027/);
    assert.match(html, /DOP #5#/, 'el resumen de febrero sigue siendo el del mes, no el de la búsqueda');
});

test('el texto del titular en el historial y en los filtros se escapa', async () => {
    const hostil = `<img src=x onerror=alert(1)>"'`;
    const t = montar({ gastos: [gasto({ descripcion: hostil, categoria_nombre: hostil, metodo_pago: hostil })] });
    await t.vista.render();
    t.campos.gas_f_txt.value = hostil;
    t.vista.filtrarGastos();
    assert.doesNotMatch(t.pantalla.contenido.innerHTML + t.lista(), /<img/);
    assert.match(t.lista(), /<strong>1<\/strong> de 1 gastos/);
    // y el texto buscado vuelve a su caja escapado
    assert.match(t.pantalla.contenido.innerHTML, /value="&lt;img src=x onerror=alert\(1\)&gt;&quot;&#39;"/);
});

test('sin gastos no hay controles de búsqueda', async () => {
    const t = montar({ gastos: [] });
    await t.vista.render();
    assert.doesNotMatch(t.pantalla.contenido.innerHTML, /gas_f_txt/);
    assert.match(t.pantalla.contenido.innerHTML, /No hay gastos registrados/);
});

// --- puente real ---

test('el puente expone las acciones nuevas y la interfaz real las enlaza', () => {
    for (const n of ['ofrecerUltimoGasto', 'aplicarUltimoGasto', 'usarUltimoMonto', 'filtrarGastos', 'limpiarFiltrosGastos']) {
        assert.ok(MANEJADORES_GASTOS.includes(n), n);
    }
    const ui = cargarInterfaz({});
    for (const n of MANEJADORES_GASTOS) assert.equal(typeof ui.appUI[n], 'function', n);
    const t = montar({ gastos: [] });
    assert.deepEqual(Object.keys(puenteGastos(t.vista)).sort(), [...MANEJADORES_GASTOS].sort());
});

test('el HTML de la pestaña solo usa acciones del puente', async () => {
    const t = montar({ gastos: HISTORIAL() });
    await t.vista.render();
    const nombres = new Set(accionesDe(t.pantalla.contenido.innerHTML + t.lista()).map(a => a.nombre));
    for (const n of ['ofrecerUltimoGasto', 'filtrarGastos', 'limpiarFiltrosGastos']) assert.ok(nombres.has(n), n);
    for (const n of nombres) assert.ok(MANEJADORES_GASTOS.includes(n), n);
});
