// La premisa que sostiene el tramo 4 de la política de redondeo: un importe
// que llega a Rust como número **nunca trae fracción de céntimo**, porque el
// navegador no deja enviar el formulario con más de dos decimales.
//
// Con dos decimales el recorrido texto → número → centavos es exacto (barrido
// de 2·10⁸ importes sin una discrepancia); con tres, 6,6 de cada 100 importes
// terminados en 5 se deciden distinto. Ver `politica_redondeo.md`, tramo 4c.
//
// Se comprobó a mano en la aplicación empaquetada: escribir `1.005` en un campo
// de importe muestra el aviso del navegador y no guarda nada.
//
// Esta prueba impide que la premisa se rompa sin que nadie lo note: un campo
// de importe nuevo sin `step="0.01"`, un formulario con `novalidate`, o un
// envío de dinero que no pase por `onsubmit`. Lee `ui.js` como texto, sin
// dependencias, igual que la prueba del contrato con Rust.

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { leerInterfazJs } from '../ayudas/fuentes_interfaz.js';

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..');
const UI = leerInterfazJs();
const HTML = readFileSync(join(RAIZ, 'src', 'index.html'), 'utf8');

// Campos numéricos que **no son dinero** y por eso no llevan `step="0.01"`.
// Cada uno con su razón; un campo nuevo que no esté aquí debe llevar el step.
const NO_SON_DINERO = new Map([
    ['gas_tasa', 'tasa de cambio, cuatro decimales (step 0.0001)'],
    ['sus_dia', 'día de facturación'],
    ['pre_tot', 'número de cuotas'],
    ['pre_pen', 'cuotas pendientes'],
    ['pre_dia', 'día de pago'],
    ['tar_cor', 'día de corte'],
    ['tar_pag', 'día de pago'],
    ['edp_dia_*', 'día de pago (edición)'],
    ['es_dia_*', 'día de facturación (edición)'],
]);

function camposNumericos() {
    // Las etiquetas <input> de la interfaz se escriben en una sola línea; no
    // contienen `>` dentro de sus atributos.
    return [...UI.matchAll(/<input\b[^>]*\btype=["']number["'][^>]*>/g)].map(m => m[0]);
}

// El id de un campo; los generados con `${...}` se reducen a su parte fija
// seguida de `*`. La comparación es **exacta**: por prefijo, `tar_cor` (día de
// corte) dejaría pasar `tar_cor_dop` (importe).
const idDe = campo => {
    const id = /\bid=["']([^"']+)["']/.exec(campo)?.[1] ?? '';
    return id.includes('${') ? `${id.split('${')[0]}*` : id;
};
const declarado = id => NO_SON_DINERO.has(id);

test('todo campo numérico de la interfaz es de importe con step 0.01 o está declarado como no monetario', () => {
    const campos = camposNumericos();
    assert.ok(campos.length > 40, `se esperaban decenas de campos numéricos, hay ${campos.length}`);

    const sinStep = campos.filter(c => !/\bstep=["']0\.01["']/.test(c));
    const sinDeclarar = sinStep.filter(c => !declarado(idDe(c)));
    assert.deepEqual(
        sinDeclarar,
        [],
        'campos numéricos sin step="0.01" que no están en NO_SON_DINERO: si es un importe, añade el step; ' +
        'si no lo es, decláralo con su razón.',
    );
});

test('la lista de campos no monetarios no tiene entradas obsoletas', () => {
    const ids = camposNumericos().map(idDe);
    const obsoletas = [...NO_SON_DINERO.keys()].filter(p => !ids.includes(p));
    assert.deepEqual(obsoletas, [], 'declarados pero ya no existen en la interfaz');
});

test('los campos declarados como no monetarios no llevan step de céntimo por descuido', () => {
    // Un día de facturación con step 0.01 aceptaría 15.37: la lista dice «no
    // es dinero», y ahí el step correcto es el entero por defecto o el de la tasa.
    const mal = camposNumericos().filter(
        c => declarado(idDe(c)) && /\bstep=["']0\.01["']/.test(c),
    );
    assert.deepEqual(mal, []);
});

test('ningún formulario desactiva la validación del navegador', () => {
    for (const [nombre, texto] of [['ui.js', UI], ['index.html', HTML]]) {
        assert.ok(!/novalidate|formnovalidate|noValidate/i.test(texto), `${nombre} desactiva la validación`);
    }
});

// Los métodos de `api.js` que envían dinero a Rust. Cada uno debe llamarse
// desde un manejador que el formulario enlaza con `onsubmit`, es decir, **tras**
// la validación del navegador: un botón con `onclick` la saltaría.
// Las vistas extraídas de `ui.ts` reciben la API por inyección y la llaman como
// `api.metodo(...)`; la clase vieja, como `AppAPI.metodo(...)`.
const ENVIAN_DINERO = [
    'crearGasto', 'crearIngreso', 'actualizarIngreso', 'marcarIngresoPagado',
    'crearIngresoInformal', 'marcarInformalPagado', 'crearCobroEfectivoInformal',
    'crearTarjeta', 'actualizarLimitesTarjeta', 'registrarPagoTarjeta',
    'crearCuenta', 'transferirEntreCuentas', 'crearBonificacion',
    'liquidarConsumoPendiente', 'crearPrestamo', 'actualizarPrestamo',
];

test('todo envío de dinero sale de un manejador enlazado con onsubmit', () => {
    let llamadas = 0;
    const sinSubmit = [];
    for (const metodo of ENVIAN_DINERO) {
        const usos = [...UI.matchAll(new RegExp(`\\b(?:AppAPI|api)\\.${metodo}\\(`, 'g'))];
        assert.ok(usos.length > 0, `${metodo} ya no se usa: quítalo de ENVIAN_DINERO`);
        for (const uso of usos) {
            llamadas++;
            const antes = UI.slice(0, uso.index);
            const manejador = [...antes.matchAll(/\n\s+(?:async )?(\w+)\([^)]*\)\s*\{/g)].pop()?.[1];
            if (!new RegExp(`onsubmit=["']appUI\\.${manejador}\\(`).test(UI)) {
                sinSubmit.push(`${metodo} desde ${manejador}`);
            }
        }
    }
    assert.ok(llamadas >= ENVIAN_DINERO.length);
    assert.deepEqual(sinSubmit, [], 'envíos de dinero que no pasan por onsubmit');
});

test('ninguna llamada de dinero recibe un importe calculado en el navegador', () => {
    // El único importe derivado que viajaba, el cobro parcial de una factura,
    // ya se envía como texto. Un cálculo con `Math.round(... * 100) / 100`
    // dentro de una llamada a la API volvería a decidir el céntimo en el lugar
    // equivocado.
    const llamadas = ENVIAN_DINERO.flatMap(m =>
        [...UI.matchAll(new RegExp(`\\b(?:AppAPI|api)\\.${m}\\(([^;]*?)\\);`, 'gs'))].map(x => [m, x[1]]),
    );
    const calculadas = llamadas.filter(([, args]) => /Math\.(round|floor|ceil)|toFixed/.test(args));
    assert.deepEqual(calculadas, []);
});
