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
// * `argumentoJs(valor)`: como **argumento de un manejador** (`onclick="appUI.f(${argumentoJs(x)})"`).
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
 * Un valor como argumento literal de un manejador en línea, para un atributo entre comillas
 * dobles: `onclick="appUI.abrir(${argumentoJs(cuenta)})"`. Admite cadenas, números, nulos y
 * objetos (se pasan como literal de objeto; un manejador que espera JSON en texto recibe
 * `argumentoJs(JSON.stringify(obj))`, es decir, una cadena).
 */
export function argumentoJs(valor: unknown): string {
    return escaparHtml(JSON.stringify(valor) ?? 'null');
}
