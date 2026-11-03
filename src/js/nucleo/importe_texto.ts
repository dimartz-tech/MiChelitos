// Importes que llegan de Rust como TEXTO exacto (`-1234.56`): formato y cuentas sin pasar por `Number`.
//
// Lo que Rust calculó se muestra tal cual; nada aquí redondea de nuevo ni acumula ruido de coma flotante.

/**
 * Un importe en texto exacto (`-1234.56`) con separador de miles. Trabaja sobre las cifras, sin pasar por `Number`:
 * la vista muestra lo que Rust calculó, no una cifra redondeada de nuevo. Lo que no es un importe vuelve igual.
 */
export function importeDeTexto(texto: string): string {
    const m = /^(-?)(\d+)(\.\d+)?$/.exec(texto);
    if (!m) return texto;
    return `${m[1]}${m[2].replace(/\B(?=(\d{3})+(?!\d))/g, ',')}${m[3] ?? ''}`;
}

/** Un decimal en texto (`12.34`, `-0.5`) como entero escalado a `decimales` cifras; `null` si no es un decimal. */
function escalado(texto: string, decimales: number): bigint | null {
    const m = /^(-?)(\d+)(?:\.(\d+))?$/.exec(texto);
    if (!m) return null;
    const frac = (m[3] ?? '').padEnd(decimales, '0').slice(0, decimales);
    const valor = BigInt(m[2] + frac);
    return m[1] === '-' ? -valor : valor;
}

/**
 * `monto × tasa` redondeado al centavo (mitad hacia arriba, en valor absoluto), en texto con dos decimales.
 * Es una estimación para rotular («≈ DOP y a tasa T»): con BigInt, sin `Number`. `null` si alguno no es un decimal.
 */
export function porTasaEnTexto(monto: string, tasa: string): string | null {
    const cs = escalado(monto, 2);
    const t = escalado(tasa, 6);
    if (cs === null || t === null) return null;
    const producto = cs * t; // centavos × 10^6
    const neg = producto < 0n;
    const abs = neg ? -producto : producto;
    const centavos = (abs + 500_000n) / 1_000_000n;
    const texto = centavos.toString().padStart(3, '0');
    return `${neg && centavos !== 0n ? '-' : ''}${texto.slice(0, -2)}.${texto.slice(-2)}`;
}
