// Núcleo puro de dinero: sin DOM, sin Tauri, sin estado global.
// Es el espejo en el frontend de src-tauri/src/dominio/dinero.rs.

// @ts-check

/** @typedef {'DOP' | 'USD'} Divisa */
/** @typedef {{ monto: number | string, divisa: string }} Importe */

/** @type {readonly Divisa[]} */
export const DIVISAS = Object.freeze(/** @type {Divisa[]} */ (['DOP', 'USD']));

const LOCALE = 'es-DO';

/** @type {Map<string, Intl.NumberFormat>} */
const formateadores = new Map();

export class ErrorDivisa extends Error {
    /** @param {string} mensaje */
    constructor(mensaje) {
        super(mensaje);
        this.name = 'ErrorDivisa';
    }
}

/**
 * @param {unknown} codigo
 * @returns {Divisa}
 */
export function normalizarDivisa(codigo) {
    const c = String(codigo ?? '').trim().toUpperCase();
    if (!DIVISAS.includes(/** @type {Divisa} */ (c))) {
        throw new ErrorDivisa(`Divisa no reconocida: '${codigo}'. Las divisas admitidas son DOP y USD.`);
    }
    return /** @type {Divisa} */ (c);
}

/** @param {unknown} codigo */
export function esDivisaValida(codigo) {
    try {
        normalizarDivisa(codigo);
        return true;
    } catch {
        return false;
    }
}

/**
 * @param {number | string} monto
 * @param {unknown} divisa
 * @returns {string}
 */
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
    return /** @type {Intl.NumberFormat} */ (formateadores.get(d)).format(n);
}

/**
 * @param {Importe} a
 * @param {Importe} b
 * @returns {{ monto: number, divisa: Divisa }}
 */
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
/**
 * @template {{ divisa: string }} M
 * @param {M[] | null | undefined} movimientos
 * @param {(m: M) => number | string} [obtenerMonto]
 * @returns {Record<Divisa, number>}
 */
export function totalizarPorDivisa(
    movimientos,
    obtenerMonto = (m) => /** @type {number | string} */ (/** @type {any} */ (m).monto),
) {
    const totales = /** @type {Record<Divisa, number>} */ ({});
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

/**
 * @param {Record<Divisa, number>} totales
 * @returns {Divisa[]}
 */
export function divisasPresentes(totales) {
    return DIVISAS.filter((d) => totales[d] !== 0);
}
