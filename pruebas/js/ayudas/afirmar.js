// Afirmaciones comunes de las pruebas de interacción: legibles y con mensajes
// que dicen qué se envió de verdad cuando algo no coincide.

import assert from 'node:assert/strict';

const describir = i => JSON.stringify(i.llamadas.map(l => ({ [l.metodo]: l.args })));

/** Se llamó exactamente una vez a `metodo`, y con estos argumentos (comparación estricta: `123.45` ≠ `'123.45'`). */
export function llamoUnaVez(interfaz, metodo, argsEsperados) {
    const hechas = interfaz.llamadasA(metodo);
    assert.equal(hechas.length, 1, `se esperaba 1 llamada a ${metodo}; llamadas: ${describir(interfaz)}`);
    assert.deepEqual(hechas[0].args, argsEsperados, `argumentos de ${metodo}`);
}

/** Nada se envió a Rust. */
export function noLlamoANada(interfaz) {
    assert.deepEqual(interfaz.llamadas, [], `no debía llamar a la API, pero llamó: ${describir(interfaz)}`);
}

/** No se llamó a `metodo` (puede haberse llamado a otros, p. ej. consultas previas). */
export function noLlamoA(interfaz, metodo) {
    assert.deepEqual(interfaz.llamadasA(metodo), [], `no debía llamar a ${metodo}`);
}

/** El último aviso es de éxito (y su texto casa con `patron`). */
export function avisoExito(interfaz, patron) {
    const ultimo = interfaz.avisos.at(-1);
    assert.ok(ultimo, 'no se mostró ningún aviso');
    assert.equal(ultimo.tipo, 'success', `se esperaba éxito, hubo ${JSON.stringify(interfaz.avisos)}`);
    if (patron) assert.match(ultimo.mensaje, patron);
}

/** Hay exactamente un aviso y es un error cuyo texto casa con `patron`. */
export function unAvisoDeError(interfaz, patron) {
    assert.equal(interfaz.avisos.length, 1, `se esperaba un solo aviso; hubo ${JSON.stringify(interfaz.avisos)}`);
    assert.equal(interfaz.avisos[0].tipo, 'error', `se esperaba error; hubo ${JSON.stringify(interfaz.avisos)}`);
    if (patron) assert.match(interfaz.avisos[0].mensaje, patron);
}

/** Se redibujó la pestaña `ruta` (una vez). */
export function redibujo(interfaz, ruta) {
    assert.deepEqual(interfaz.renders, [ruta], `renders: ${JSON.stringify(interfaz.renders)}`);
}

/** No se redibujó nada. */
export function noRedibujo(interfaz) {
    assert.deepEqual(interfaz.renders, [], 'no debía redibujar');
}

/**
 * Camino «la API rechaza»: se muestra el error (texto tal cual), la pantalla no
 * se redibuja ni se cierra nada y la promesa del manejador **no** rechaza.
 */
export async function rechazoSeMuestra(interfaz, ejecutar, mensaje = 'Error simulado de Rust') {
    await assert.doesNotReject(ejecutar(), 'el manejador no debe propagar el rechazo de la API');
    const errores = interfaz.avisosDeError();
    assert.equal(errores.length, 1, `se esperaba un aviso de error; avisos: ${JSON.stringify(interfaz.avisos)}`);
    assert.ok(errores[0].mensaje.includes(mensaje), `el aviso debe contener «${mensaje}»: ${errores[0].mensaje}`);
    assert.equal(interfaz.avisos.filter(a => a.tipo === 'success').length, 0, 'no debe anunciar éxito');
    noRedibujo(interfaz);
}
