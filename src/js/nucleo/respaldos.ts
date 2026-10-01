// Presentación de los respaldos: sin DOM ni Tauri, para poder probarla en Node.
// Es un módulo que importa la vista de Ajustes (`vistas/ajustes.ts`): ya no se
// carga como script clásico.
//
// El nombre de un respaldo es `michelitos_<AAAA-MM-DDTHH-MM-SS>_<motivo>.db`
// (ver `respaldo.rs`). Se muestra como «29/09/2026 19:03:25 · antes de instalar»,
// con la fecha como el resto de la aplicación. Los nombres que no siguen ese
// patrón —respaldos manuales antiguos— se muestran tal cual, sin la extensión.
export function describirRespaldo(nombre: unknown): string {
    const sinExtension = String(nombre).replace(/\.db$/, '');
    const m = /^michelitos_(\d{4})-(\d{2})-(\d{2})T(\d{2})-(\d{2})-(\d{2})_(.+)$/.exec(sinExtension);
    if (!m) return sinExtension;
    const [, a, mes, d, h, mi, s, motivo] = m;
    return `${d}/${mes}/${a} ${h}:${mi}:${s} · ${motivo.replace(/-/g, ' ')}`;
}

// `escaparHtml` vive en `nucleo/html.ts` (lo usan todas las vistas); se reexporta aquí
// porque la lista de respaldos fue su primer uso y sus pruebas lo importan de este módulo.
export { escaparHtml } from './html.js';
