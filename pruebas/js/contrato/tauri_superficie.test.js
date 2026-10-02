// Contrato de la superficie de Tauri (A-02 de la auditoría del 2026-10-02).
//
// La interfaz solo necesita `invoke` hacia los comandos de Rust, que Tauri 1 deja disponible siempre. Todo lo demás
// de la «allowlist» (sistema de archivos, shell, HTTP, diálogos, procesos…) es poder que la capa web no usa y que,
// ante una inyección en la interfaz, sería poder del atacante. Estas pruebas fijan que se mantiene cerrado y que
// la interfaz no empieza a depender de otra API sin que se decida.

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..');
const leer = ruta => readFileSync(join(RAIZ, ruta), 'utf8');

function fuentesTs(dir, acumulado = []) {
    for (const nombre of readdirSync(dir)) {
        const ruta = join(dir, nombre);
        if (statSync(ruta).isDirectory()) fuentesTs(ruta, acumulado);
        else if (nombre.endsWith('.ts') && !nombre.endsWith('.d.ts')) acumulado.push(ruta);
    }
    return acumulado;
}

test('la allowlist de Tauri está cerrada: ninguna API nativa queda expuesta a la interfaz', () => {
    const conf = JSON.parse(leer('src-tauri/tauri.conf.json'));
    assert.equal(conf.tauri.allowlist.all, false);
    const abiertas = Object.entries(conf.tauri.allowlist).filter(([k, v]) => k !== 'all' && JSON.stringify(v).includes('true'));
    assert.deepEqual(abiertas, [], 'una API de la allowlist está abierta: justifícala aquí y en la auditoría');
});

test('Cargo no activa `api-all` ni otras funciones de Tauri que la interfaz no usa', () => {
    const linea = leer('src-tauri/Cargo.toml').split('\n').find(l => /^tauri\s*=/.test(l));
    assert.ok(linea, 'no encuentro la dependencia tauri');
    assert.doesNotMatch(linea, /api-all/);
    assert.match(linea, /features\s*=\s*\[\s*\]/, 'las funciones de Tauri activadas deben coincidir con la allowlist (vacía)');
});

test('la interfaz solo usa `invoke` del puente de Tauri, nada de fs, shell, dialog, http, window o event', () => {
    const usos = [];
    for (const f of fuentesTs(join(RAIZ, 'src', 'js'))) {
        const texto = readFileSync(f, 'utf8');
        for (const m of texto.matchAll(/__TAURI__\s*\.\s*(\w+)/g)) usos.push(`${f.slice(RAIZ.length + 1)}: __TAURI__.${m[1]}`);
        for (const m of texto.matchAll(/\b(?:tauri|__TAURI__)\s*\.\s*(fs|shell|dialog|http|path|process|window|event|clipboard|notification|globalShortcut|os|updater)\b/g)) {
            usos.push(`${f.slice(RAIZ.length + 1)}: ${m[0]}`);
        }
        assert.doesNotMatch(texto, /@tauri-apps\/api/, `${f}: importa la API de Tauri`);
    }
    const inesperados = usos.filter(u => !/__TAURI__\.(invoke|tauri|core)$/.test(u) && !/__TAURI__$/.test(u));
    assert.deepEqual(inesperados, [], 'la interfaz usa una API nativa que la allowlist cerrada no permite');
});
