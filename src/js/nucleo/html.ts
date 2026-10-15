// Escape de HTML para las plantillas de las vistas: sin DOM ni Tauri, para poder probarlo
// en Node.
//
// Las vistas construyen su HTML con plantillas y lo asignan a `innerHTML`. Todo lo que
// interpolan que **no es suyo** —nombres, descripciones, conceptos, notas, motivos, mensajes
// de Rust— llega ahí como texto, y si lleva `<` o `"` se interpreta como marcado o rompe un
// atributo: un `<img src=x onerror=…>` en un nombre de cuenta ejecutaría código en el WebView.
// Hay dos formas de meter texto en una plantilla, según dónde caiga:
//
// * `escaparHtml(texto)`: en el texto de un elemento o en el valor de un atributo
//   (`<td>${escaparHtml(c.nombre)}</td>`, `value="${escaparHtml(c.nombre)}"`).
// * `argumentoJs(valor)`: como **argumento de una acción delegada** (lo usa `manejador(...)`; `data-args="${argumentoJs([x])}"`).
//   Genera un literal de JavaScript (una cadena, un número o un objeto) ya escapado para el
//   atributo. Sin él, un `'` en el texto cerraba la cadena del manejador y lo que siguiera
//   se ejecutaba como código; el escape a mano (`.replace(/'/g, "&#39;")`) no lo evitaba,
//   porque el navegador deshace el escape del atributo antes de ejecutar el manejador.
//
// Lo que **no** necesita escape: los números, los literales y lo que ya devolvió otro de estos
// dos ayudantes. `pruebas/js/contrato/escape_html.test.js` comprueba que ninguna interpolación
// de texto libre llega sin pasar por uno de ellos.

export function escaparHtml(texto: unknown): string {
    return String(texto)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;');
}

/**
 * Un valor como literal JSON, para un atributo entre comillas
 * dobles: `data-args="${argumentoJs([cuenta])}"`. Admite cadenas, números, nulos y
 * objetos (se pasan como literal de objeto; un manejador que espera JSON en texto recibe
 * `argumentoJs(JSON.stringify(obj))`, es decir, una cadena).
 */
export function argumentoJs(valor: unknown): string {
    return escaparHtml(JSON.stringify(valor) ?? 'null');
}

// --- Manejadores delegados -------------------------------------------------------------
//
// La interfaz ya no escribe `onclick="appUI.f(...)"`: un atributo en línea es código dentro
// del marcado, y obliga a que la política de contenido (CSP) admita `'unsafe-inline'` en
// `script-src-attr`. En su lugar, el elemento declara **datos**:
//
//     <button data-accion="handleEliminar" data-args="[7]">             (clic)
//     <form data-accion-enviar="handleAgregar">                         (envío)
//     <select data-accion-cambiar="rotular" data-pasa="valor">          (cambio)
//     <input data-accion-escribir="previsualizar" data-args="[3,50]">   (entrada)
//
// y un único oyente (`ui/delegacion.ts`) lee esos atributos y llama a la acción del puente.
// Los argumentos viajan como JSON en `data-args` (un arreglo) y se escapan con `escaparHtml`:
// ningún texto libre puede salirse del atributo ni ejecutarse; solo llega como dato.
// `data-pasa` dice qué recibe la acción **antes** de los argumentos: `evento`, `elemento`
// (el que tiene el atributo, el antiguo `this`) o `valor` (el antiguo `this.value`).

export type EventoDelegado = 'click' | 'submit' | 'change' | 'input';
export type PasoDelegado = 'evento' | 'elemento' | 'valor';

/** El atributo que lleva el nombre de la acción, por evento. */
export const ATRIBUTO_DE_ACCION: Readonly<Record<EventoDelegado, string>> = {
    click: 'data-accion',
    submit: 'data-accion-enviar',
    change: 'data-accion-cambiar',
    input: 'data-accion-escribir',
};

/**
 * Los atributos `data-*` que hacen que un elemento llame a `nombre(...args)` al ocurrir
 * `evento`. Se interpola dentro de la etiqueta: `<button ${accion('f', id)}>`.
 */
export function manejador(evento: EventoDelegado, nombre: string, args: readonly unknown[] = [], pasa?: PasoDelegado): string {
    const partes = [`${ATRIBUTO_DE_ACCION[evento]}="${escaparHtml(nombre)}"`];
    if (args.length > 0) partes.push(`data-args="${argumentoJs(args)}"`);
    if (pasa) partes.push(`data-pasa="${pasa}"`);
    return partes.join(' ');
}

/** Clic: `<button ${accion('handleEliminar', c.id)}>`. */
export const accion = (nombre: string, ...args: unknown[]): string => manejador('click', nombre, args);
/** Envío de un formulario: la acción recibe el evento primero, y el oyente ya llamó a `preventDefault`. */
export const alEnviar = (nombre: string, ...args: unknown[]): string => manejador('submit', nombre, args);
/** Cambio de un campo (la acción solo recibe los argumentos; use `manejador` para recibir el valor). */
export const alCambiar = (nombre: string, ...args: unknown[]): string => manejador('change', nombre, args);
/** Entrada de texto en un campo. */
export const alEscribir = (nombre: string, ...args: unknown[]): string => manejador('input', nombre, args);
