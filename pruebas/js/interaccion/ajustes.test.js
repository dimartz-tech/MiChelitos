// Pestaña Ajustes: respaldos (crear y restaurar), categorías y clientes.
// (Las correcciones con motivo de Ajustes están con su pestaña de origen:
// gastos, ingresos y cuentas.) Datos inventados.

import test from 'node:test';
import assert from 'node:assert/strict';
import { cargarInterfaz, crearEvento, crearElemento, AUSENTE } from '../ayudas/cargar_interfaz.js';
import {
    llamoUnaVez, noLlamoANada, noLlamoA, avisoExito, redibujo, noRedibujo, rechazoSeMuestra,
} from '../ayudas/afirmar.js';

// --- crear respaldo ---

test('crear respaldo: deshabilita el botón mientras corre, muestra la ruta y lo deja como estaba', async () => {
    let boton;
    const ui = cargarInterfaz({
        campos: { respaldo_resultado: {} },
        api: {
            crearRespaldo: () => {
                assert.equal(boton.disabled, true, 'el botón debe estar deshabilitado durante el respaldo');
                assert.equal(boton.textContent, 'Respaldando…');
                return 'respaldos/michelitos_prueba.db';
            },
        },
    });
    boton = crearElemento('button', { textContent: 'Crear respaldo' });
    await ui.appUI.handleCrearRespaldo(boton);
    llamoUnaVez(ui, 'crearRespaldo', []);
    avisoExito(ui, /Respaldo creado y verificado/);
    assert.equal(ui.elemento('respaldo_resultado').textContent, 'respaldos/michelitos_prueba.db');
    assert.equal(boton.disabled, false);
    assert.equal(boton.textContent, 'Crear respaldo');
});

test('crear respaldo: si falla muestra el error, borra la ruta anterior y rehabilita el botón', async () => {
    const ui = cargarInterfaz({ campos: { respaldo_resultado: { textContent: 'ruta vieja' } } });
    ui.rechazar('crearRespaldo', 'Error simulado de Rust');
    const boton = crearElemento('button', { textContent: 'Crear respaldo' });
    await rechazoSeMuestra(ui, () => ui.appUI.handleCrearRespaldo(boton));
    assert.equal(ui.elemento('respaldo_resultado').textContent, '');
    assert.equal(boton.disabled, false);
    assert.equal(boton.textContent, 'Crear respaldo');
});

test('crear respaldo: funciona aunque la pantalla no tenga el recuadro de resultado', async () => {
    const ui = cargarInterfaz({ campos: { respaldo_resultado: AUSENTE }, api: { crearRespaldo: 'ruta' } });
    await ui.appUI.handleCrearRespaldo(crearElemento('button', { textContent: 'Crear respaldo' }));
    avisoExito(ui, /Respaldo creado/);
});

// --- restaurar respaldo ---

const selector = (valor = 'michelitos_2026-03-01T10-00-00_manual.db') => ({
    respaldo_elegido: {
        options: [
            { value: '', text: 'Elige…' },
            { value: valor, text: '01/03/2026 10:00:00 · manual' },
        ],
        selectedIndex: valor ? 1 : 0,
    },
    restauracion_resultado: {},
});

test('restaurar respaldo: confirma nombrando el respaldo, envía su nombre y muestra el resultado', async () => {
    const ui = cargarInterfaz({
        campos: selector(), confirm: true, api: { restaurarRespaldo: { capital_restaurado: true } },
    });
    const boton = crearElemento('button', { textContent: 'Restaurar' });
    await ui.appUI.handleRestaurarRespaldo(boton);
    llamoUnaVez(ui, 'restaurarRespaldo', ['michelitos_2026-03-01T10-00-00_manual.db']);
    assert.match(ui.confirmaciones[0].texto, /01\/03\/2026 10:00:00 · manual/);
    avisoExito(ui, /Respaldo restaurado/);
    redibujo(ui, 'ajustes');
    assert.match(ui.elemento('restauracion_resultado').textContent, /El capital también/);
});

test('restaurar respaldo sin capital: el resultado lo dice', async () => {
    const ui = cargarInterfaz({
        campos: selector(), confirm: true, api: { restaurarRespaldo: { capital_restaurado: false } },
    });
    await ui.appUI.handleRestaurarRespaldo(crearElemento('button'));
    assert.match(ui.elemento('restauracion_resultado').textContent, /no traía capital/);
});

test('restaurar respaldo: sin respaldo elegido, no se pregunta ni se envía nada', async () => {
    const ui = cargarInterfaz({ campos: selector(''), confirm: true });
    await ui.appUI.handleRestaurarRespaldo(crearElemento('button'));
    noLlamoANada(ui);
    assert.deepEqual(ui.confirmaciones, []);
});

test('restaurar respaldo: si no se confirma no se envía nada y el botón no se toca', async () => {
    const ui = cargarInterfaz({ campos: selector(), confirm: false });
    const boton = crearElemento('button', { textContent: 'Restaurar' });
    await ui.appUI.handleRestaurarRespaldo(boton);
    noLlamoANada(ui);
    assert.equal(boton.disabled, false);
    assert.equal(boton.textContent, 'Restaurar');
    noRedibujo(ui);
});

test('restaurar respaldo: si Rust rechaza se muestra el error y el botón se rehabilita', async () => {
    const ui = cargarInterfaz({ campos: selector(), confirm: true });
    ui.rechazar('restaurarRespaldo', 'Error simulado de Rust');
    const boton = crearElemento('button', { textContent: 'Restaurar' });
    await rechazoSeMuestra(ui, () => ui.appUI.handleRestaurarRespaldo(boton));
    assert.equal(boton.disabled, false);
    assert.equal(boton.textContent, 'Restaurar');
});

// --- categorías ---

test('alta de categoría: envía el nombre y redibuja Ajustes', async () => {
    const ui = cargarInterfaz({ campos: { cat_nom: 'Categoría de Prueba' } });
    await ui.appUI.handleAgregarCategoria(crearEvento());
    llamoUnaVez(ui, 'crearCategoria', ['Categoría de Prueba']);
    avisoExito(ui, /Categoría agregada/);
    redibujo(ui, 'ajustes');
});

test('alta de categoría: si Rust rechaza (p. ej. duplicada) se muestra el error', async () => {
    const ui = cargarInterfaz({ campos: { cat_nom: 'Categoría de Prueba' } });
    ui.rechazar('crearCategoria', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleAgregarCategoria(crearEvento()));
});

test('baja de categoría: con confirmación envía el id; sin ella no hace nada; si Rust rechaza, error', async () => {
    let ui = cargarInterfaz({ confirm: true });
    await ui.appUI.handleEliminarCategoria(14);
    llamoUnaVez(ui, 'eliminarCategoria', [14]);
    avisoExito(ui, /Categoría eliminada/);
    redibujo(ui, 'ajustes');

    ui = cargarInterfaz({ confirm: false });
    await ui.appUI.handleEliminarCategoria(14);
    noLlamoANada(ui);

    ui = cargarInterfaz({ confirm: true });
    ui.rechazar('eliminarCategoria', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleEliminarCategoria(14));
});

test('marcar una categoría como fija / recurrente: envía (id, marca) y avisa; si Rust rechaza, deshace el casillero y avisa', async () => {
    let ui = cargarInterfaz({ api: { marcarCategoriaFija: { id: 14, nombre: 'Seguros', fija: true } } });
    await ui.appUI.handleMarcarCategoriaFija({ checked: true }, 14);
    llamoUnaVez(ui, 'marcarCategoriaFija', [14, true]);
    avisoExito(ui, /«Seguros» ahora es fija \/ recurrente/);
    noRedibujo(ui);

    ui = cargarInterfaz({});
    ui.rechazar('marcarCategoriaFija', 'Error simulado de Rust');
    const control = { checked: true };
    await rechazoSeMuestra(ui, () => ui.appUI.handleMarcarCategoriaFija(control, 14));
    assert.equal(control.checked, false);
});

test('renombrar categoría: pide el nombre, envía (id, nombre) y redibuja; cancelar no envía; si Rust rechaza, error', async () => {
    let ui = cargarInterfaz({ prompt: 'Vivienda', api: { renombrarCategoria: { id: 14, nombre: 'Vivienda', motivo_sistema: null } } });
    await ui.appUI.handleRenombrarCategoria(14, 'Renta');
    llamoUnaVez(ui, 'renombrarCategoria', [14, 'Vivienda']);
    avisoExito(ui, /Categoría renombrada/);
    redibujo(ui, 'ajustes');

    ui = cargarInterfaz({ prompt: null });
    await ui.appUI.handleRenombrarCategoria(14, 'Renta');
    noLlamoANada(ui);

    ui = cargarInterfaz({ prompt: 'Vivienda' });
    ui.rechazar('renombrarCategoria', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleRenombrarCategoria(14, 'Renta'));
});

const CATEGORIAS_FUSION = [
    { id: 14, nombre: 'Renta', motivo_sistema: null },
    { id: 15, nombre: 'Alquiler', motivo_sistema: null },
    { id: 1, nombre: 'Otros', motivo_sistema: 'asentar las comisiones' },
];

test('fusionar categoría: abre la ventana; el destino pide la vista previa; confirmar fusiona y redibuja', async () => {
    const previa = { origen_id: 14, origen_nombre: 'Renta', destino_id: 15, destino_nombre: 'Alquiler', gastos: 3, desde: '21/07/2026', hasta: '03/09/2026', advertencias: [] };
    const ui = cargarInterfaz({
        campos: { fusion_destino: { value: '15' }, fusion_vista_previa: {}, fusion_confirmar: { disabled: true }, 'modal-fusion-categoria': {} },
        api: { obtenerCategorias: CATEGORIAS_FUSION, previsualizarFusionCategorias: previa, fusionarCategorias: 3 },
    });

    await ui.appUI.handleFusionarCategoria(14);
    assert.equal(ui.modalesAbiertos.length, 1);
    assert.equal(ui.modalesAbiertos[0].id, 'modal-fusion-categoria');

    await ui.appUI.cambiarDestinoFusion('15', 14);
    llamoUnaVez(ui, 'previsualizarFusionCategorias', [14, 15]);
    assert.match(ui.elemento('fusion_vista_previa').innerHTML, /3 gastos<\/strong> de «Renta» pasarán a «Alquiler»/);
    assert.equal(ui.elemento('fusion_confirmar').disabled, false);
    noLlamoA(ui, 'fusionarCategorias');

    await ui.appUI.handleConfirmarFusion(14);
    llamoUnaVez(ui, 'fusionarCategorias', [14, 15]);
    avisoExito(ui, /Fusión hecha: 3 gastos pasaron a «Alquiler»/);
    redibujo(ui, 'ajustes');
    assert.ok(ui.eliminados.length > 0, 'la ventana se cerró');
});

test('fusionar categoría: confirmar sin haber visto la vista previa no fusiona nada', async () => {
    const ui = cargarInterfaz({ campos: { fusion_destino: { value: '15' }, fusion_confirmar: { disabled: true } } });
    await ui.appUI.handleConfirmarFusion(14);
    noLlamoANada(ui);
});

// --- clientes ---

test('alta de cliente: AppAPI.crearCliente recibe (RNC, nombre), no (nombre, RNC)', async () => {
    const ui = cargarInterfaz({ campos: { cli_aj_nom: 'Cliente de Prueba', cli_aj_rnc: '000000001' } });
    await ui.appUI.handleAgregarCliente(crearEvento());
    llamoUnaVez(ui, 'crearCliente', ['000000001', 'Cliente de Prueba']);
    avisoExito(ui, /Cliente registrado/);
    redibujo(ui, 'ajustes');
});

test('alta de cliente: si Rust rechaza se muestra el error', async () => {
    const ui = cargarInterfaz({ campos: { cli_aj_nom: 'Cliente de Prueba', cli_aj_rnc: '000000001' } });
    ui.rechazar('crearCliente', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleAgregarCliente(crearEvento()));
});

test('baja de cliente: con confirmación envía el id; sin ella no hace nada; si Rust rechaza, error', async () => {
    let ui = cargarInterfaz({ confirm: true });
    await ui.appUI.handleEliminarCliente(15);
    llamoUnaVez(ui, 'eliminarCliente', [15]);
    avisoExito(ui, /Cliente eliminado/);
    redibujo(ui, 'ajustes');

    ui = cargarInterfaz({ confirm: false });
    await ui.appUI.handleEliminarCliente(15);
    noLlamoANada(ui);

    ui = cargarInterfaz({ confirm: true });
    ui.rechazar('eliminarCliente', 'Error simulado de Rust');
    await rechazoSeMuestra(ui, () => ui.appUI.handleEliminarCliente(15));
});
