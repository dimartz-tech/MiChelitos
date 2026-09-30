// Acceso tipado al DOM, compartido por las vistas.
//
// `document.getElementById` devuelve `HTMLElement | null` y luego se le lee
// `.value`: TypeScript exige aceptar el `null` en cada uno de los 232 sitios
// de `ui.ts`. Estos dos ayudantes lo resuelven una vez y con el tipo correcto:
//
// * `elemento` —el camino normal— devuelve el elemento o **falla diciendo cuál
//   falta**. Antes fallaba con «Cannot read properties of null», sin decir qué
//   id; el fallo es el mismo, el mensaje ahora sirve.
// * `buscar` conserva el `null` para los sitios donde la ausencia es legítima
//   (un modal que puede no estar abierto).
//
// Se carga como script clásico (index.html), así que sus nombres son globales.

/** Un campo de formulario: todos tienen `.value`. */
type Campo = HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement;

function elemento<T extends HTMLElement = HTMLElement>(id: string): T {
    const encontrado = document.getElementById(id);
    if (encontrado === null) {
        throw new Error(`No existe el elemento #${id} en la página.`);
    }
    return encontrado as T;
}

function buscar<T extends HTMLElement = HTMLElement>(id: string): T | null {
    return document.getElementById(id) as T | null;
}
