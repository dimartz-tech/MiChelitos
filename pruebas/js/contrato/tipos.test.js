// Los tipos del contrato con Rust (`src/js/tipos-ipc.d.ts`) se generan de
// `main.rs`; esta prueba impide que se queden atrás. Si falla: ejecuta
// `npm run tipos:generar` y revisa el cambio.
//
// Sin esto, `npm run tipos` comprobaría el código contra tipos **viejos** y
// daría la razón a un error.

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { leerInterfazTs } from '../ayudas/fuentes_interfaz.js';
import { analizar, generar, tipoJs } from '../../../herramientas/generar_tipos_ipc.mjs';

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..');
const RUST = readFileSync(join(RAIZ, 'src-tauri', 'src', 'main.rs'), 'utf8');
const TIPOS = readFileSync(join(RAIZ, 'src', 'js', 'tipos-ipc.d.ts'), 'utf8');
const API = readFileSync(join(RAIZ, 'src', 'js', 'api.ts'), 'utf8');

test('tipos-ipc.d.ts está al día con main.rs', () => {
    assert.equal(TIPOS, generar(RUST), 'desactualizado: npm run tipos:generar');
});

test('todos los comandos y las estructuras de Rust están en los tipos', () => {
    const { respuestas, entradas, comandos } = analizar(RUST);
    assert.ok(comandos.length > 50, `se esperaban decenas de comandos, hay ${comandos.length}`);
    assert.ok(respuestas.length >= 16, `se esperaban las 16 estructuras de respuesta, hay ${respuestas.length}`);
    assert.ok(entradas.length >= 4, `se esperaban las estructuras de entrada, hay ${entradas.length}`);
    for (const c of comandos) assert.ok(TIPOS.includes(`    ${c.nombre}: { args:`), `falta el comando ${c.nombre}`);
});

test('api.ts tipa invoke con el mapa de comandos generado desde Rust', () => {
    assert.ok(API.includes("Comandos[K]['ret']"), 'invoke ya no está tipado con Comandos');
    assert.ok(API.includes("import('./tipos-ipc').Comandos"), 'api.ts ya no importa los tipos generados');
});

test('el traductor de tipos rechaza un tipo de Rust desconocido en vez de inventarlo', () => {
    assert.throws(() => tipoJs('HashMap<String, i64>'), /sin traducción/);
    assert.equal(tipoJs('Option<Vec<String>>'), 'string[] | null');
    assert.equal(tipoJs('Option<ipc::ImporteDecimal>'), 'string | null');
});

test('la interfaz no lee campos de suscripción que Rust ya no envía', () => {
    // `fecha_renovacion` fue el puntero de cobro antes de la Fase 5; hoy Rust
    // envía `fecha_proximo_cobro`. El aviso «Cobro próximo» siguió leyendo el
    // viejo y mostraba «undefined» sin que nada lo señalara.
    const UI = leerInterfazTs();
    const enviado = /export type Suscripcion = \{ ([^}]*) \}/.exec(TIPOS)?.[1] ?? '';
    assert.ok(enviado.includes('fecha_proximo_cobro'), 'Rust ya no envía fecha_proximo_cobro');
    assert.ok(!enviado.includes('fecha_renovacion'));
    assert.ok(!UI.includes('.fecha_renovacion'), 'la interfaz lee fecha_renovacion, que Rust no envía');
});
