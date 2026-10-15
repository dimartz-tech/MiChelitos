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
import { analizar, generar, leerFuentesRust, tipoJs } from '../../../herramientas/generar_tipos_ipc.mjs';

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..');
const RUST = leerFuentesRust();
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

// --- El generador con Rust repartido en módulos: datos inventados -----------

import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';

const MINIMO = (comandos, registro) => `
${comandos}
fn main() { tauri::Builder::default().invoke_handler(tauri::generate_handler![${registro}]); }
`;

test('acepta comandos pub, pub(crate) y async, y estructuras pub(crate) con tipos de otro módulo', () => {
    const rust = MINIMO(`
#[derive(Serialize)]
pub(crate) struct Resumen {
    pub total: i64,
    pub detalle: comandos::Linea,
}
#[derive(Serialize)]
pub struct Linea {
    texto: String,
}
#[tauri::command]
pub fn uno(id: i64) -> Result<Resumen, String> { todo!() }
#[tauri::command]
pub(crate) async fn dos(nombre_largo: Option<String>) -> Result<(), String> { todo!() }
`, 'uno, comandos::dos');
    const { comandos, respuestas } = analizar(rust);
    assert.deepEqual(comandos.map(c => c.nombre), ['uno', 'dos']);
    assert.deepEqual(comandos[1].argumentos, [{ clave: 'nombreLargo', opcional: true, tipo: 'string | null' }]);
    assert.deepEqual(respuestas[0], ['Resumen', [['total', 'number'], ['detalle', 'Linea']]]);
});

test('se niega si un #[tauri::command] tiene una forma que el generador no entiende', () => {
    const rust = MINIMO(`
#[tauri::command]
pub fn bien() -> Result<i64, String> { todo!() }
#[tauri::command]
fn sin_retorno() { }
`, 'bien, sin_retorno');
    assert.throws(() => analizar(rust), /2 #\[tauri::command\] y el generador solo entiende 1/);
});

test('se niega si un comando no está registrado, o si se registra uno que no existe', () => {
    const comando = `
#[tauri::command]
fn a() -> Result<i64, String> { todo!() }
#[tauri::command]
fn b() -> Result<i64, String> { todo!() }
`;
    assert.throws(() => analizar(MINIMO(comando, 'a')), /Sin registrar: \[b\]/);
    assert.throws(() => analizar(MINIMO(comando, 'a, b, fantasma')), /Registrados sin comando: \[fantasma\]/);
    assert.throws(() => analizar('#[tauri::command]\nfn a() -> Result<i64, String> { todo!() }'), /generate_handler/);
});

test('un #[tauri::command] mencionado solo en un comentario no cuenta', () => {
    const rust = MINIMO(`
// Cada comando lleva #[tauri::command] y se registra aparte.
#[tauri::command]
fn a() -> Result<i64, String> { todo!() }
`, 'a');
    assert.equal(analizar(rust).comandos.length, 1);
});

test('lee main.rs y, si existe, todos los módulos de comandos/ en orden', () => {
    const dir = mkdtempSync(join(tmpdir(), 'tipos-'));
    try {
        writeFileSync(join(dir, 'main.rs'), '// principal\n');
        assert.equal(leerFuentesRust(dir), '// principal\n', 'sin carpeta comandos/ solo main.rs');
        mkdirSync(join(dir, 'comandos'));
        writeFileSync(join(dir, 'comandos', 'b.rs'), '// b\n');
        writeFileSync(join(dir, 'comandos', 'a.rs'), '// a\n');
        writeFileSync(join(dir, 'comandos', 'notas.txt'), 'no cuenta');
        assert.equal(leerFuentesRust(dir), '// principal\n\n// a\n\n// b\n');
    } finally {
        rmSync(dir, { recursive: true, force: true });
    }
});
