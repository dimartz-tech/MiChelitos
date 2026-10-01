// Pruebas de la vista «Capital» en Node, sin navegador.
//
// Es la primera vista que **guarda**: cada alta o baja lee el documento de
// capital entero, lo modifica y se lo devuelve a Rust. Aquí se le dan una API
// que lo anota, diálogos y reloj falsos, y se comprueba exactamente **qué
// documento se guarda**: sobre todo que los importes salgan como TEXTO tal cual
// se escribieron (Rust decide el céntimo con los dígitos) y que la vista no
// sume ni redondee nada. Los datos son inventados.
//
// Se importa el JavaScript compilado de la vista (`npm test` compila antes).

import test from 'node:test';
import assert from 'node:assert/strict';
import { VistaCapital, MANEJADORES_CAPITAL, puenteCapital } from '../../../src/js/vistas/capital.js';

const campo = value => ({ value });
const documento = () => ({
    certificados: [
        { banco: 'Banco Alfa', monto: 1000, tasa: 5, vencimiento: '01/01/2030', tipo_pago: 'A cuenta', alerta_vencimiento: true, alerta_msg: 'vence pronto' },
        { banco: 'Banco Beta', monto: 2000, tasa: 6, vencimiento: '01/01/2031', tipo_pago: 'Reinversión compuesta' },
    ],
    bolsa: [{ emisor: 'Emisor Gamma', monto: 500, tasa: 7, vencimiento: '01/01/2032', tipo_pago: 'A cuenta' }],
    propiedades: {
        inmobiliario: [{ id: 'a1', subtipo: 'Apto', nombre: 'Apto Delta', valor_estimado: 8000 }],
        vehiculos: [{ id: 'v1', subtipo: 'SUV', nombre: 'SUV Épsilon', valor_estimado: 2000 }],
        maquinaria: [],
    },
});

function montar({ capital = documento(), campos = {}, confirma = true, fallaAlGuardar = null, falla = null } = {}) {
    const guardados = [];
    const avisos = [];
    const rutas = [];
    const preguntas = [];
    const api = {
        obtenerCapital: async () => {
            if (falla) throw new Error(falla);
            return structuredClone(capital);   // como Tauri: cada lectura es una copia nueva
        },
        guardarCapital: async doc => {
            if (fallaAlGuardar) throw new Error(fallaAlGuardar);
            guardados.push(structuredClone(doc));
        },
    };
    const pantalla = { contenido: { innerHTML: '' } };
    const vista = new VistaCapital({
        api,
        avisos: { mostrar: (mensaje, tipo) => avisos.push({ mensaje, tipo }) },
        formato: { importe: v => `#${v}#` },
        enrutador: { mostrar: async ruta => { rutas.push(ruta); } },
        pantalla,
        dom: {
            elemento: id => {
                if (!(id in campos)) throw new Error(`No existe el elemento #${id} en la página.`);
                return campos[id];
            },
            buscar: id => campos[id] ?? null,
        },
        dialogos: { confirmar: m => { preguntas.push(m); return confirma; }, preguntar: () => null },
        ahora: () => new Date(1800000000000),
    });
    let evitado = 0;
    const evento = { preventDefault: () => { evitado++; } };
    return { vista, guardados, avisos, rutas, preguntas, pantalla, evento, evitado: () => evitado };
}

const CAMPOS_CERTIFICADO = { cer_ban: campo('Banco Nuevo'), cer_mon: campo(' 12345.675 '), cer_tas: campo('7.25'), cer_ven: campo('15/06/2030'), cer_pag: campo('A cuenta') };
const CAMPOS_BOLSA = { bol_emi: campo('Emisor Nuevo'), bol_mon: campo('999.99'), bol_tas: campo('4'), bol_ven: campo('20/12/2029'), bol_pag: campo('A cuenta') };
const CAMPOS_PROPIEDAD = { pro_tip: campo('vehiculos'), pro_sub: campo('Moto'), pro_nom: campo('Moto Zeta'), pro_val: campo(' 300.50 ') };

test('dibuja certificados, bolsa y bienes por grupo, con los importes por el formato inyectado', async () => {
    const t = montar();
    await t.vista.render();
    const html = t.pantalla.contenido.innerHTML;
    assert.match(html, /Capital y Activos/);
    for (const texto of ['Banco Alfa', 'Banco Beta', 'Emisor Gamma', 'Apto Delta', 'SUV Épsilon']) assert.match(html, new RegExp(texto));
    assert.match(html, /DOP #1000#/);
    assert.match(html, /DOP #8000#/);
    assert.match(html, /vence pronto/, 'el aviso de vencimiento que calculó Rust');
    assert.doesNotMatch(html, /undefined|NaN/);
});

test('no suma nada: ni los totales del capital aparecen en esta pantalla', async () => {
    const t = montar({ capital: { ...documento(), totales: { patrimonio: 424242 } } });
    await t.vista.render();
    assert.doesNotMatch(t.pantalla.contenido.innerHTML, /424242/, 'los totales los muestran el Dashboard y el Resumen');
    assert.doesNotMatch(t.pantalla.contenido.innerHTML, /DOP #3000#|DOP #13500#/, 'ni una suma hecha en la vista');
});

test('un capital vacío o sin secciones no rompe la pantalla', async () => {
    const t = montar({ capital: {} });
    await t.vista.render();
    assert.match(t.pantalla.contenido.innerHTML, /Capital y Activos/);
});

test('alta de certificado: el monto viaja como TEXTO recortado, la tasa como número, y se guarda el documento entero', async () => {
    const t = montar({ campos: CAMPOS_CERTIFICADO });
    await t.vista.handleAgregarCertificado(t.evento);
    assert.equal(t.evitado(), 1, 'cancela el envío nativo del formulario');
    assert.equal(t.guardados.length, 1);
    const doc = t.guardados[0];
    assert.equal(doc.certificados.length, 3, 'los dos que había más el nuevo');
    assert.deepEqual(doc.certificados[2], { banco: 'Banco Nuevo', monto: '12345.675', tasa: 7.25, vencimiento: '15/06/2030', tipo_pago: 'A cuenta' });
    assert.equal(typeof doc.certificados[2].monto, 'string', 'sin convertir: los dígitos decide el céntimo Rust');
    assert.deepEqual(doc.bolsa, documento().bolsa, 'el resto del documento viaja intacto');
    assert.deepEqual(doc.propiedades, documento().propiedades);
    assert.deepEqual(t.avisos, [{ mensaje: 'Certificado guardado.', tipo: undefined }]);
    assert.deepEqual(t.rutas, ['capital']);
});

test('alta de certificado en un capital sin certificados: crea la lista', async () => {
    const t = montar({ capital: {}, campos: CAMPOS_CERTIFICADO });
    await t.vista.handleAgregarCertificado(t.evento);
    assert.equal(t.guardados[0].certificados.length, 1);
});

test('alta de certificado: si Rust rechaza el importe, avisa con su mensaje y no redibuja', async () => {
    const t = montar({ campos: CAMPOS_CERTIFICADO, fallaAlGuardar: 'Certificado 3 (Banco Nuevo): el monto no puede ser negativo' });
    await t.vista.handleAgregarCertificado(t.evento);
    assert.deepEqual(t.avisos, [{ mensaje: 'Error: Certificado 3 (Banco Nuevo): el monto no puede ser negativo', tipo: 'error' }]);
    assert.deepEqual(t.rutas, []);
});

test('baja de certificado: pide confirmación, quita el de ese índice y guarda; sin confirmar no toca nada', async () => {
    const si = montar();
    await si.vista.handleEliminarCertificado(0);
    assert.equal(si.preguntas.length, 1);
    assert.deepEqual(si.guardados[0].certificados.map(c => c.banco), ['Banco Beta']);
    assert.deepEqual(si.avisos, [{ mensaje: 'Certificado retirado.', tipo: undefined }]);
    assert.deepEqual(si.rutas, ['capital']);

    const no = montar({ confirma: false });
    await no.vista.handleEliminarCertificado(0);
    assert.deepEqual(no.guardados, []);
    assert.deepEqual(no.avisos, []);
    assert.deepEqual(no.rutas, []);
});

test('alta de bolsa: mismo trato — monto como TEXTO, tasa como número, emisor en su campo', async () => {
    const t = montar({ campos: CAMPOS_BOLSA });
    await t.vista.handleAgregarBolsa(t.evento);
    assert.deepEqual(t.guardados[0].bolsa[1], { emisor: 'Emisor Nuevo', monto: '999.99', tasa: 4, vencimiento: '20/12/2029', tipo_pago: 'A cuenta' });
    assert.deepEqual(t.avisos, [{ mensaje: 'Inversión de bolsa guardada.', tipo: undefined }]);
    assert.deepEqual(t.rutas, ['capital']);
});

test('baja de bolsa: confirmar quita la inversión; sin confirmar no guarda', async () => {
    const si = montar();
    await si.vista.handleEliminarBolsa(0);
    assert.deepEqual(si.guardados[0].bolsa, []);
    assert.deepEqual(si.avisos, [{ mensaje: 'Inversión liquidada.', tipo: undefined }]);
    const no = montar({ confirma: false });
    await no.vista.handleEliminarBolsa(0);
    assert.deepEqual(no.guardados, []);
});

test('alta de bien: el valor viaja como TEXTO y el identificador sale del reloj inyectado', async () => {
    const t = montar({ campos: CAMPOS_PROPIEDAD });
    await t.vista.handleAgregarPropiedad(t.evento);
    const nuevo = t.guardados[0].propiedades.vehiculos[1];
    assert.deepEqual(nuevo, { id: '1800000000000', subtipo: 'Moto', nombre: 'Moto Zeta', valor_estimado: '300.50' });
    assert.equal(typeof nuevo.valor_estimado, 'string');
    assert.deepEqual(t.avisos, [{ mensaje: 'Propiedad registrada.', tipo: undefined }]);
    assert.deepEqual(t.rutas, ['capital']);
});

test('alta de bien en un capital sin propiedades: crea las tres secciones y el grupo elegido', async () => {
    const t = montar({ capital: {}, campos: { ...CAMPOS_PROPIEDAD, pro_tip: campo('maquinaria') } });
    await t.vista.handleAgregarPropiedad(t.evento);
    const p = t.guardados[0].propiedades;
    assert.deepEqual(Object.keys(p).sort(), ['inmobiliario', 'maquinaria', 'vehiculos']);
    assert.equal(p.maquinaria.length, 1);
});

test('baja de bien: se quita por identificador, no por posición; un grupo inexistente no guarda ni redibuja', async () => {
    const t = montar();
    await t.vista.handleEliminarPropiedad('inmobiliario', 'a1');
    assert.deepEqual(t.guardados[0].propiedades.inmobiliario, []);
    assert.deepEqual(t.guardados[0].propiedades.vehiculos.map(p => p.id), ['v1'], 'los demás grupos intactos');
    assert.deepEqual(t.avisos, [{ mensaje: 'Bien eliminado.', tipo: undefined }]);

    const sin = montar({ capital: {} });
    await sin.vista.handleEliminarPropiedad('inmobiliario', 'a1');
    assert.deepEqual(sin.guardados, []);
    assert.deepEqual(sin.rutas, []);

    const no = montar({ confirma: false });
    await no.vista.handleEliminarPropiedad('inmobiliario', 'a1');
    assert.deepEqual(no.guardados, []);
});

test('el puente expone todos los manejadores y delega en la vista', async () => {
    const t = montar({ campos: CAMPOS_BOLSA });
    const puente = puenteCapital(t.vista);
    assert.deepEqual(Object.keys(puente).sort(), [...MANEJADORES_CAPITAL].sort());
    await puente.handleAgregarBolsa(t.evento);
    assert.equal(t.guardados.length, 1);
});

test('todo appUI.x( que escribe la plantilla está en el puente', async () => {
    const t = montar();
    await t.vista.render();
    const llamados = new Set([...t.pantalla.contenido.innerHTML.matchAll(/appUI\.(\w+)\(/g)].map(m => m[1]));
    assert.equal(llamados.size, 6);
    for (const nombre of llamados) assert.ok(MANEJADORES_CAPITAL.includes(nombre), `falta ${nombre} en el puente`);
});

// --- las bajas no pierden el error ---

const BAJAS = [
    ['certificado', t => t.vista.handleEliminarCertificado(0)],
    ['inversión de bolsa', t => t.vista.handleEliminarBolsa(0)],
    ['bien', t => t.vista.handleEliminarPropiedad('inmobiliario', 'a1')],
];

test('baja: si falla al guardar, se muestra el error, no se propaga, no se anuncia éxito y no se redibuja', async () => {
    for (const [que, dar] of BAJAS) {
        const t = montar({ fallaAlGuardar: 'Rust no pudo guardar' });
        await assert.doesNotReject(() => dar(t), que);
        assert.deepEqual(t.avisos, [{ mensaje: 'Error: Rust no pudo guardar', tipo: 'error' }], que);
        assert.deepEqual([t.guardados, t.rutas], [[], []], que);
    }
});

test('baja: si falla al leer el capital, se muestra el error y no se guarda nada', async () => {
    for (const [que, dar] of BAJAS) {
        const t = montar({ falla: 'Rust no pudo leer' });
        await assert.doesNotReject(() => dar(t), que);
        assert.deepEqual(t.avisos, [{ mensaje: 'Error: Rust no pudo leer', tipo: 'error' }], que);
        assert.deepEqual([t.guardados, t.rutas], [[], []], que);
    }
});

test('baja: sin confirmar no se lee ni se guarda nada, aunque Rust fuera a fallar', async () => {
    for (const [que, dar] of BAJAS) {
        const t = montar({ confirma: false, falla: 'no debería llamarse' });
        await dar(t);
        assert.deepEqual([t.avisos, t.guardados, t.rutas], [[], [], []], que);
    }
});
