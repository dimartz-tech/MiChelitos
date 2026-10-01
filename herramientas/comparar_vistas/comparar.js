// Se ejecuta en la consola de una página servida desde la MISMA raíz que
// `/anterior/index.html`, `/nueva/index.html` y `/volcado.json`.
//
// Pinta cada pestaña con los dos frontends y compara el texto. **Solo devuelve
// un resumen**: las líneas distintas salen con los dígitos enmascarados y
// recortadas, para poder compartir el resultado sin exponer importes.
(async () => {
    const cargar = src => new Promise(resolve => {
        const f = document.createElement('iframe');
        f.style.cssText = 'width:1200px;height:800px';
        f.onload = () => setTimeout(() => resolve(f), 2500);
        f.src = src;
        document.body.appendChild(f);
    });
    const anterior = await cargar('/anterior/index.html');
    const nueva = await cargar('/nueva/index.html');
    const pestanas = ['dashboard', 'ingresos', 'gastos', 'tarjetas', 'cuentas', 'efectivo',
        'suscripciones', 'capital', 'prestamos', 'resumen', 'ajustes'];
    const texto = async (f, t) => {
        f.contentWindow.navigate(t);
        await new Promise(r => setTimeout(r, 900));
        return f.contentWindow.document.getElementById('app-content').innerText;
    };
    const enmascarar = s => s.replace(/\d/g, '#').slice(0, 110);
    const resumen = { pestanas: {}, textoDefectuoso: {}, errores: {}, comandosDesconocidos: {} };
    for (const t of pestanas) {
        const a = await texto(anterior, t);
        const b = await texto(nueva, t);
        const la = a.split('\n'), lb = b.split('\n');
        const soloA = la.filter(l => !lb.includes(l)), soloB = lb.filter(l => !la.includes(l));
        resumen.pestanas[t] = a === b
            ? `idéntica (${b.length} caracteres)`
            : `difiere: ${soloA.length} líneas solo en la anterior, ${soloB.length} solo en la nueva` +
              ` | nueva: ${JSON.stringify(soloB.slice(0, 3).map(enmascarar))}` +
              ` | anterior: ${JSON.stringify(soloA.slice(0, 3).map(enmascarar))}`;
        const malos = (b.match(/undefined|NaN|\[object/g) || []).length;
        if (malos) resumen.textoDefectuoso[t] = malos;
    }
    resumen.errores = { anterior: anterior.contentWindow.__errores.length, nueva: nueva.contentWindow.__errores.length };
    resumen.comandosDesconocidos = {
        anterior: [...anterior.contentWindow.__desconocidos],
        nueva: [...nueva.contentWindow.__desconocidos],
    };
    return JSON.stringify(resumen, null, 1);
})();
