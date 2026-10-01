// Sustituye al puente de Tauri en una página de prueba: responde a cada comando
// con lo que haya en `/volcado.json` (ver README.md). No contiene datos.
//
// Se inserta ANTES de `js/api.js` en el `index.html` de la copia que se compara.
window.__errores = [];
window.__desconocidos = new Set();
window.confirm = () => true;
window.prompt = () => null;
window.addEventListener('error', e => window.__errores.push(String(e.message)));
const __volcado = fetch('/volcado.json').then(r => r.json());
window.__TAURI__ = {
    invoke: async (cmd, args) => {
        const d = await __volcado;
        const id = args && (args.tarjetaId ?? args.id);
        if (cmd === 'obtener_abonos_tarjeta') return (d.por_tarjeta[id] || {}).abonos || [];
        if (cmd === 'obtener_avances_tarjeta') return (d.por_tarjeta[id] || {}).avances || [];
        if (cmd === 'obtener_movimientos_prestamo') return d.por_prestamo[id] || [];
        if (cmd === 'procesar_suscripciones') return [];
        if (cmd in d) return d[cmd];
        window.__desconocidos.add(cmd);
        return cmd.startsWith('obtener') || cmd.startsWith('listar') ? [] : null;
    },
};
