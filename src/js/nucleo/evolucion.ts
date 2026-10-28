// Aritmética de la pestaña «Evolución»: pura, sin DOM ni Tauri, probada en Node.
//
// Los importes de las fotos del patrimonio llegan de Rust como **texto exacto** («1234.56»,
// «-80.10»). Aquí nunca se pasan por `Number`: se convierten a **centavos enteros** (`bigint`),
// con los que se calculan variaciones, porcentajes y la escala del eje; y se formatean sobre
// esos enteros. Solo al final, cuando un centavo ya es una **coordenada de pantalla**, se
// pasa a número (la posición de un píxel no es dinero).
//
// Redondeo: un importe con más de dos decimales (Rust no los envía, pero no se da por hecho) se
// redondea al centavo «mitad lejos de cero» mirando solo el tercer decimal; el porcentaje se
// redondea igual a centésimas de punto porcentual. Ver `politica_redondeo.md`.

/** Lo que esta pestaña lee de una foto del historial (el resto del DTO no hace falta). */
export interface FotoEvolucion {
    anio: number;
    mes: number;
    patrimonio: string;
    liquido: string;
    activos: string;
    pasivos: string;
    origen: string;
    calidad: string;
}

const IMPORTE = /^([+-])?(\d+)(?:\.(\d*))?$/;

/** «1234.56» → 123456n; «-0.5» → -50n. Lanza `Error` si el texto no es un importe decimal. */
export function enCentavos(texto: string): bigint {
    const m = IMPORTE.exec(texto.trim());
    if (!m) throw new Error(`Importe ilegible: «${texto}»`);
    const frac = m[3] ?? '';
    const base = BigInt(m[2]) * 100n + BigInt((frac + '00').slice(0, 2));
    const redondeo = frac.length > 2 && frac.charCodeAt(2) >= 53 ? 1n : 0n; // '5' = 53
    const abs = base + redondeo;
    return m[1] === '-' ? -abs : abs;
}

const conMiles = (entero: bigint): string => entero.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ',');

/** 123456n → «1,234.56»; -5n → «-0.05». `decimales: false` redondea a unidades («1,235»). */
export function formatearCentavos(c: bigint, decimales = true): string {
    const negativo = c < 0n;
    const abs = negativo ? -c : c;
    if (!decimales) {
        const unidades = (abs + 50n) / 100n;
        return (negativo && unidades !== 0n ? '-' : '') + conMiles(unidades);
    }
    const texto = `${conMiles(abs / 100n)}.${(abs % 100n).toString().padStart(2, '0')}`;
    return negativo ? `-${texto}` : texto;
}

/** Como `formatearCentavos` pero con signo explícito: «+1,234.56», «-0.05», «0.00». */
export function formatearConSigno(c: bigint): string {
    return c > 0n ? `+${formatearCentavos(c)}` : formatearCentavos(c);
}

/** Formatea un importe de texto; si no se puede leer, devuelve el texto tal cual (nunca lanza). */
export function formatearTexto(texto: string): string {
    try { return formatearCentavos(enCentavos(texto)); } catch { return texto; }
}

const MESES = ['ene', 'feb', 'mar', 'abr', 'may', 'jun', 'jul', 'ago', 'sep', 'oct', 'nov', 'dic'];

/** 2027, 3 → «mar 2027». */
export function nombreDeMes(anio: number, mes: number): string {
    return `${MESES[mes - 1] ?? String(mes)} ${anio}`;
}

/** Las fotos de la más antigua a la más reciente (por año y mes, no por el orden en que llegaron). */
export function ordenarFotos<T extends { anio: number; mes: number }>(fotos: readonly T[]): T[] {
    return [...fotos].sort((a, b) => a.anio - b.anio || a.mes - b.mes);
}

export type Direccion = 'sube' | 'baja' | 'igual';

export interface Variacion {
    /** Diferencia en centavos (actual - anterior). */
    delta: bigint;
    /** Diferencia en centésimas de punto porcentual (123n = 1.23 %), o `null` si la base es 0. */
    centesimas: bigint | null;
    direccion: Direccion;
}

/** Variación de `actual` respecto de `anterior` (centavos). El porcentaje es sobre el valor absoluto de la base. */
export function variacion(actual: bigint, anterior: bigint): Variacion {
    const delta = actual - anterior;
    const direccion: Direccion = delta > 0n ? 'sube' : delta < 0n ? 'baja' : 'igual';
    let centesimas: bigint | null = null;
    if (anterior !== 0n) {
        const base = anterior < 0n ? -anterior : anterior;
        const abs = delta < 0n ? -delta : delta;
        const c = (abs * 10000n * 2n + base) / (base * 2n); // mitad lejos de cero
        centesimas = delta < 0n ? -c : c;
    }
    return { delta, centesimas, direccion };
}

/** 123n → «+1.23 %»; -5n → «-0.05 %»; 0n → «0.00 %». */
export function formatearPorcentaje(centesimas: bigint): string {
    const negativo = centesimas < 0n;
    const abs = negativo ? -centesimas : centesimas;
    const texto = `${abs / 100n}.${(abs % 100n).toString().padStart(2, '0')} %`;
    return negativo ? `-${texto}` : centesimas > 0n ? `+${texto}` : texto;
}

/** La serie del patrimonio que se muestra: el total, o solo lo líquido. */
export type Serie = 'total' | 'liquido';

export const valorDeSerie = (f: Pick<FotoEvolucion, 'patrimonio' | 'liquido'>, serie: Serie): bigint =>
    enCentavos(serie === 'liquido' ? f.liquido : f.patrimonio);

export const etiquetaDeSerie = (serie: Serie): string => (serie === 'liquido' ? 'Patrimonio líquido' : 'Patrimonio total');

/** Una foto es estimada (no observada el día que se tomó) si se reconstruyó o su calidad lo dice. */
export const esEstimada = (f: Pick<FotoEvolucion, 'origen' | 'calidad'>): boolean =>
    f.origen === 'reconstruida' || f.calidad === 'estimada';
