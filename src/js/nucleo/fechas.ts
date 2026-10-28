// Escritura de fechas con separador automático (dd/mm/aaaa).
//
// Función pura: recibe lo que hay en el campo **después** de que el navegador aplicó la edición
// y dónde quedó el cursor, y devuelve el texto ya formateado con el cursor donde debe estar. No
// toca el DOM (eso es `ui/mascaras.ts`) ni valida la fecha: «31/02/2026» o «28/10/26» pasan tal
// cual y los rechaza el backend con su mensaje. Tampoco expande el año de dos dígitos.

/** Lo que devuelve la máscara: el texto del campo y la posición del cursor dentro de él. */
export interface FechaEscrita {
    texto: string;
    cursor: number;
}

const SEPARADORES = new Set(['/', '-', '.']);
const esDigito = (c: string): boolean => c >= '0' && c <= '9';

/**
 * Da forma de dd/mm/aaaa a lo escrito.
 *
 * * Dígitos: la `/` se pone sola tras el día y tras el mes (de forma perezosa: solo cuando llega
 *   el dígito siguiente, o al final del texto si se está tecleando hacia delante).
 * * Separadores: `/`, `-` y `.` valen como `/`, pero solo si cierran el día o el mes; en cualquier
 *   otro sitio (inicio, tras otra barra, dentro del año) se descartan, así «28/» + «/» no da «28//».
 *   Cualquier otro carácter (letras, espacios) se descarta. El año admite 4 dígitos como máximo
 *   (10 caracteres en total); lo que sobra se descarta.
 * * Borrado (`borrando`): no se añade la barra final automática, porque devolvería la barra que
 *   el usuario acaba de borrar y el Retroceso se quedaría atascado; las barras que sí quedan en
 *   el texto se respetan, y se borran con el siguiente Retroceso.
 * * Edición en medio (cursor antes del final y sin borrar): los separadores que ya hubiera se
 *   ignoran y los dígitos se reparten de nuevo, como en cualquier máscara de campo.
 * * Cursor: se queda tras el mismo carácter que tenía a su izquierda; al teclear hacia delante
 *   salta además las barras automáticas que le siguen, para que el siguiente dígito caiga detrás.
 */
export function enmascararFecha(texto: string, cursor: number, borrando = false): FechaEscrita {
    const c = Math.max(0, Math.min(Number.isFinite(cursor) ? Math.trunc(cursor) : texto.length, texto.length));
    const respetaSeparadores = borrando || c >= texto.length;
    let salida = '';
    let tramo = 0;      // 0 día, 1 mes, 2 año
    let enTramo = 0;    // dígitos ya puestos en el tramo
    const finales: number[] = [];   // largo de la salida tras procesar cada unidad de código de la entrada

    for (const ch of texto) {
        if (esDigito(ch)) {
            if (tramo < 2 && enTramo === 2) { salida += '/'; tramo++; enTramo = 0; }
            if (!(tramo === 2 && enTramo === 4)) { salida += ch; enTramo++; }
        } else if (respetaSeparadores && SEPARADORES.has(ch) && tramo < 2 && enTramo > 0) {
            salida += '/'; tramo++; enTramo = 0;
        }
        for (let i = 0; i < ch.length; i++) finales.push(salida.length);
    }
    if (!borrando && tramo < 2 && enTramo === 2) salida += '/';

    let nuevo = c >= texto.length ? salida.length : c === 0 ? 0 : finales[c - 1]!;
    if (!borrando) while (salida[nuevo] === '/') nuevo++;
    return { texto: salida, cursor: nuevo };
}
