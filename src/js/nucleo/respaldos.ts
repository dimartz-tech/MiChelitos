// Presentación de los respaldos: sin DOM ni Tauri, para poder probarla en Node.
// Se carga como script clásico (index.html), así que no exporta nada: la
// prueba evalúa el texto compilado.
//
// El nombre de un respaldo es `michelitos_<AAAA-MM-DDTHH-MM-SS>_<motivo>.db`
// (ver `respaldo.rs`). Se muestra como «29/09/2026 19:03:25 · antes de instalar»,
// con la fecha como el resto de la aplicación. Los nombres que no siguen ese
// patrón —respaldos manuales antiguos— se muestran tal cual, sin la extensión.
function describirRespaldo(nombre: unknown): string {
    const sinExtension = String(nombre).replace(/\.db$/, '');
    const m = /^michelitos_(\d{4})-(\d{2})-(\d{2})T(\d{2})-(\d{2})-(\d{2})_(.+)$/.exec(sinExtension);
    if (!m) return sinExtension;
    const [, a, mes, d, h, mi, s, motivo] = m;
    return `${d}/${mes}/${a} ${h}:${mi}:${s} · ${motivo.replace(/-/g, ' ')}`;
}

// Los nombres salen de una carpeta del usuario: se escapan antes de ir a un
// atributo o a un texto del HTML.
function escaparHtml(texto: unknown): string {
    return String(texto)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;');
}
