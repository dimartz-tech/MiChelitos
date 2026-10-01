// Los importes viajan a Rust **como texto, tal cual se escribieron** (convención de 1.21.0).
//
// `Number('1.005')` ya no vale lo que el titular tecleó (1.00499…) y el núcleo decidiría el
// céntimo sobre ese número; con el texto lo decide sobre sus dígitos (`1.005` sube a 1.01). Cada
// comando que aún recibe un `f64` se migra en su propio PR (`ImporteDecimal` en Rust, texto en el
// envoltorio y en la vista): ver `politica_redondeo.md`, «Migración de los importes a texto».
//
// Dos pruebas llevan la cuenta:
//
//  * Los envoltorios **ya migrados** mandan los dígitos intactos (se ejecutan con un `invoke`
//    falso y se mira lo que llega).
//  * Los que **siguen** convirtiendo un importe con `Number(...)` están listados abajo. La lista
//    solo puede **encogerse**: un envoltorio nuevo que convirtiera un importe con `Number` sin
//    figurar aquí rompe la prueba, y uno migrado que siga figurando también.

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { RAIZ } from '../ayudas/fuentes_interfaz.js';

const JS = join(RAIZ, 'src', 'js');

// --- los ya migrados -----------------------------------------------------------------------

/** Evalúa `api.js` con un `invoke` que anota lo que recibe, como lo cargaría el navegador. */
function cargarApi() {
    const recibidos = [];
    const window = { __TAURI__: { invoke: async (comando, argumentos) => { recibidos.push({ comando, argumentos }); return null; } } };
    const AppAPI = new Function('window', `${readFileSync(join(JS, 'api.js'), 'utf8')}\nreturn AppAPI;`)(window);
    return { AppAPI, recibidos };
}

// Un importe que **no sobrevive a ninguna conversión numérica**: `Number` y `parseFloat` lo dejarían en
// `75.25`. Si llega idéntico, nadie lo tocó. (`1.005` solo delataría un `Number`, no un `parseFloat`.)
const TRES_DECIMALES = '0075.250';

/** Cada fila: el método, cómo llamarlo con `1.005` en el importe, y los campos que deben llegar tal cual. */
const MIGRADOS = [
    ['crearIngresoInformal', A => A.crearIngresoInformal('06/03/2027', 'Clase', TRES_DECIMALES), ['monto']],
    ['crearCobroEfectivoInformal', A => A.crearCobroEfectivoInformal('06/03/2027', 'Cobro', TRES_DECIMALES, 'USD'), ['monto']],
    ['crearSuscripcion', A => A.crearSuscripcion('Plataforma', TRES_DECIMALES, 7, 'mensual', 15, 'USD', null), ['monto']],
    ['actualizarSuscripcion', A => A.actualizarSuscripcion(1, 'Plataforma', TRES_DECIMALES, 7, 'mensual', 15, 'USD', null), ['monto']],
    ['declararSaldoPrestamo', A => A.declararSaldoPrestamo(1, TRES_DECIMALES), ['saldo']],
    ['crearCuenta (comisión)', A => A.crearCuenta('Cuenta', 'DOP', 10, null, TRES_DECIMALES), ['comisionPagoImpuestos']],
    ['actualizarCuenta (comisión)', A => A.actualizarCuenta(1, 'Cuenta', null, TRES_DECIMALES), ['comisionPagoImpuestos']],
    ['actualizarIngreso (cobro parcial)', A => A.actualizarIngreso(1, 'F-1', 2, '01/01/2027', 100, 10, TRES_DECIMALES, null), ['cobroParcial']],
    ['simularAvanceEfectivo', A => A.simularAvanceEfectivo(TRES_DECIMALES, 'DOP', 'fijo', null, TRES_DECIMALES), ['monto', 'cargoFijo']],
    ['registrarAvanceEfectivo', A => A.registrarAvanceEfectivo(1, 2, '01/01/2027', TRES_DECIMALES, 'DOP', 'fijo', null, TRES_DECIMALES, null), ['monto', 'cargoFijo']],
];

for (const [nombre, llamar, campos] of MIGRADOS) {
    test(`${nombre}: el importe llega a Rust como texto, con sus dígitos intactos (ceros incluidos)`, async () => {
        const { AppAPI, recibidos } = cargarApi();
        await llamar(AppAPI);
        assert.equal(recibidos.length, 1);
        for (const campo of campos) {
            assert.strictEqual(recibidos[0].argumentos[campo], TRES_DECIMALES, `${nombre}.${campo}`);
        }
    });
}

test('un importe de tres decimales llega intacto: es el que `Number` deja en 1.00499… y el núcleo decide con sus dígitos', async () => {
    const { AppAPI, recibidos } = cargarApi();
    await AppAPI.crearIngresoInformal('06/03/2027', 'Clase', '1.005');
    assert.strictEqual(recibidos[0].argumentos.monto, '1.005');
});

// --- los que faltan --------------------------------------------------------------------------

/**
 * Importes que el envoltorio todavía convierte con `Number(...)`, y los cinco comandos que reciben
 * un objeto de importes ya como números desde la vista (`crearGasto`, `crearIngreso`,
 * `crearPrestamo`, `actualizarPrestamo` y el monto de `registrarPagoTarjeta`). **Cada PR de
 * migración quita sus entradas de aquí.**
 */
const PENDIENTES = [
    'actualizarIngreso.montoTotal',
    'actualizarLimitesTarjeta.balanceCorteDolares', 'actualizarLimitesTarjeta.balanceCortePesos',
    'actualizarLimitesTarjeta.limiteDolares', 'actualizarLimitesTarjeta.limitePesos',
    'actualizarLimitesTarjeta.sobregiroDolares', 'actualizarLimitesTarjeta.sobregiroPesos',
    'crearBonificacion.monto', 'crearCuenta.balance',
    'crearTarjeta.balanceCorteDolares', 'crearTarjeta.balanceCortePesos',
    'crearTarjeta.balanceDolares', 'crearTarjeta.balancePesos',
    'crearTarjeta.limiteDolares', 'crearTarjeta.limitePesos',
    'crearTarjeta.sobregiroDolares', 'crearTarjeta.sobregiroPesos',
    'liquidarConsumoPendiente.montoLiquidado',
    'marcarInformalPagado.montoRecibido', 'marcarIngresoPagado.montoRecibido',
    'transferirEntreCuentas.cargo', 'transferirEntreCuentas.montoDestino', 'transferirEntreCuentas.montoOrigen',
];

const ES_IMPORTE = /monto|limite|sobregiro|balance|saldo|cuota|cargo|comision|valor|importe/i;

/** `metodo.campo` por cada `campo: Number(parametro)` de `api.ts` cuyo nombre es de un importe. */
function conversionesDeImportes() {
    const encontradas = [];
    let metodo = null;
    for (const linea of readFileSync(join(JS, 'api.ts'), 'utf8').split('\n')) {
        const m = /^    async (\w+)\(/.exec(linea);
        if (m) metodo = m[1];
        for (const c of linea.matchAll(/(\w+):\s*Number\((\w+)\)/g)) {
            if (ES_IMPORTE.test(c[1]) || ES_IMPORTE.test(c[2])) encontradas.push(`${metodo}.${c[1]}`);
        }
    }
    return encontradas.sort();
}

test('todo importe que un envoltorio convierte con Number(...) está en la lista de pendientes', () => {
    const nuevas = conversionesDeImportes().filter(x => !PENDIENTES.includes(x));
    assert.deepEqual(nuevas, [], 'un importe nuevo debe viajar como texto: `String(x)` en el envoltorio y `ImporteDecimal` en Rust');
});

test('la lista de pendientes no tiene entradas obsoletas: lo ya migrado se quita', () => {
    const actuales = conversionesDeImportes();
    const obsoletas = PENDIENTES.filter(x => !actuales.includes(x));
    assert.deepEqual(obsoletas, [], 'migrados pero aún listados: quítalos de PENDIENTES');
});
