// Núcleo puro de dinero: sin DOM, sin Tauri, sin estado global.
// Es el espejo en el frontend de src-tauri/src/dominio/dinero.rs.

export const DIVISAS = Object.freeze(['DOP', 'USD']);

const LOCALE = 'es-DO';

const formateadores = new Map();

export class ErrorDivisa extends Error {
    constructor(mensaje) {
        super(mensaje);
        this.name = 'ErrorDivisa';
    }
}

export function normalizarDivisa(codigo) {
    const c = String(codigo ?? '').trim().toUpperCase();
    if (!DIVISAS.includes(c)) {
        throw new ErrorDivisa(`Divisa no reconocida: '${codigo}'. Las divisas admitidas son DOP y USD.`);
    }
    return c;
}

export function esDivisaValida(codigo) {
    try {
        normalizarDivisa(codigo);
        return true;
    } catch {
        return false;
    }
}

export function formatear(monto, divisa) {
    const d = normalizarDivisa(divisa);
    const n = Number(monto);
    if (!Number.isFinite(n)) {
        throw new ErrorDivisa(`El monto '${monto}' no es un número válido.`);
    }
    if (!formateadores.has(d)) {
        formateadores.set(d, new Intl.NumberFormat(LOCALE, {
            style: 'currency',
            currency: d,
            minimumFractionDigits: 2,
            maximumFractionDigits: 2,
        }));
    }
    return formateadores.get(d).format(n);
}

export function sumar(a, b) {
    const da = normalizarDivisa(a.divisa);
    const db = normalizarDivisa(b.divisa);
    if (da !== db) {
        throw new ErrorDivisa(`No se pueden combinar montos en ${da} y ${db}: indique una tasa de cambio para convertirlos.`);
    }
    return { monto: Number(a.monto) + Number(b.monto), divisa: da };
}

/**
 * Agrupa importes por divisa en lugar de sumarlos en un único total.
 * Es la corrección estructural del descuadre que la versión 1.3.4 resolvió
 * a mano en la vista de Gastos: nunca devuelve una cifra multidivisa.
 */
export function totalizarPorDivisa(movimientos, obtenerMonto = (m) => m.monto) {
    const totales = {};
    for (const d of DIVISAS) totales[d] = 0;

    for (const m of movimientos ?? []) {
        const d = normalizarDivisa(m.divisa);
        const n = Number(obtenerMonto(m));
        if (!Number.isFinite(n)) {
            throw new ErrorDivisa(`El monto '${obtenerMonto(m)}' no es un número válido.`);
        }
        totales[d] += n;
    }
    return totales;
}

export function divisasPresentes(totales) {
    return DIVISAS.filter((d) => totales[d] !== 0);
}
