// «Gasto por categoría» (Resumen): las funciones puras de formato, flechas, etiquetas y la tabla que dibujan.
// Datos inventados, con céntimos poco redondos.

import test from 'node:test';
import assert from 'node:assert/strict';
import { importeDeTexto, porTasaEnTexto } from '../../../src/js/nucleo/importe_texto.js';
import { importeDeTexto as desdeTarjetas } from '../../../src/js/vistas/tarjetas.js';
import {
    etiquetaDeMes, filasOrdenadas, htmlDeGastoPorCategoria, idsDeInversionYObligaciones, mesDeValor, mesesDelSelector,
    notaDeDolares, porcentajeDeMilesimas, valorDeMes, variacionDePb,
    alturasDeBarras, descripcionDeTendencia, etiquetaDeDireccion, htmlDeTendencia,
    AYUDA_DE_FIJAS, filasFiltradas, filtroDeValor, htmlDeFijoYVariable, textoDeSegmento,
} from '../../../src/js/vistas/resumen_gastos.js';
import { eleccion, fila, medidas, resumen, segmento, tendencia } from '../ayudas/resumen_gastos_datos.js';

const dibujar = (r = resumen(), e = eleccion()) => htmlDeGastoPorCategoria({ tipo: 'ok', resumen: r }, e);

test('importeDeTexto: separador de miles sobre el texto, sin Number, y la tarjeta reexporta el mismo ayudante', () => {
    assert.equal(importeDeTexto('1234567.89'), '1,234,567.89');
    assert.equal(importeDeTexto('-886.75'), '-886.75');
    assert.equal(importeDeTexto('900719925474099.99'), '900,719,925,474,099.99');
    assert.equal(importeDeTexto('no es importe'), 'no es importe');
    assert.equal(desdeTarjetas, importeDeTexto, 'una sola implementación');
});

test('porTasaEnTexto: producto exacto al centavo con BigInt', () => {
    assert.equal(porTasaEnTexto('12.34', '60.00'), '740.40');
    assert.equal(porTasaEnTexto('0.01', '59.50'), '0.60'); // 0.595 sube
    assert.equal(porTasaEnTexto('-1.01', '3'), '-3.03');
    assert.equal(porTasaEnTexto('0.00', '60'), '0.00');
    assert.equal(porTasaEnTexto('abc', '60'), null);
});

test('variacionDePb: flecha Y texto; null es «sin referencia», no cero', () => {
    assert.deepEqual(variacionDePb(2345), { sentido: 'sube', flecha: '▲', texto: 'sube 23.45 %' });
    assert.deepEqual(variacionDePb(-1250), { sentido: 'baja', flecha: '▼', texto: 'baja 12.50 %' });
    assert.deepEqual(variacionDePb(5), { sentido: 'sube', flecha: '▲', texto: 'sube 0.05 %' });
    assert.deepEqual(variacionDePb(0), { sentido: 'igual', flecha: '=', texto: 'igual' });
    assert.deepEqual(variacionDePb(null), { sentido: 'sin-referencia', flecha: '', texto: '—' });
});

test('porcentajeDeMilesimas y etiquetas de mes', () => {
    assert.equal(porcentajeDeMilesimas(333), '33.3 %');
    assert.equal(porcentajeDeMilesimas(1000), '100.0 %');
    assert.equal(porcentajeDeMilesimas(9), '0.9 %');
    assert.equal(etiquetaDeMes(2027, 3), 'marzo 2027');
    assert.equal(valorDeMes({ anio: 2027, mes: 3 }), '2027-03');
    assert.deepEqual(mesDeValor('2027-03'), { anio: 2027, mes: 3 });
    assert.equal(mesDeValor('2027-13'), null);
    assert.equal(mesDeValor('hostil'), null);
});

test('mesesDelSelector: los disponibles más el elegido, de más antiguo a más reciente y sin repetir', () => {
    const ms = mesesDelSelector([{ anio: 2027, mes: 2 }, { anio: 2026, mes: 11 }], { anio: 2027, mes: 3 });
    assert.deepEqual(ms.map(valorDeMes), ['2026-11', '2027-02', '2027-03']);
    assert.deepEqual(mesesDelSelector([{ anio: 2027, mes: 2 }], { anio: 2027, mes: 2 }).map(valorDeMes), ['2027-02']);
});

test('idsDeInversionYObligaciones: por nombre (sin tildes ni mayúsculas) con los ids del catálogo; lo que no existe no se inventa', () => {
    const cat = [
        { id: 41, nombre: 'Comida' }, { id: 7, nombre: 'Impuestos' }, { id: 12, nombre: 'Maquinaria/Equipos' },
        { id: 13, nombre: 'Préstamo' }, { id: 99, nombre: 'RENTA' }, { id: 5, nombre: 'Renta de oficina' },
    ];
    assert.deepEqual(idsDeInversionYObligaciones(cat).sort((a, b) => a - b), [7, 12, 13, 99]);
    assert.deepEqual(idsDeInversionYObligaciones([{ id: 1, nombre: 'Comida' }]), []);
    assert.deepEqual(idsDeInversionYObligaciones([{ id: 3, nombre: 'Alquiler' }]), [3]);
});

test('filasOrdenadas: «Cargos derivados» siempre al final', () => {
    const f = filasOrdenadas(resumen().filas);
    assert.deepEqual(f.map(x => x.categoria_nombre), ['Comida', 'Transporte', 'Cargos derivados']);
});

test('notaDeDolares: «incluye USD x ≈ DOP y a tasa T · estimado»', () => {
    assert.equal(notaDeDolares('12.34', '60.00'), 'incluye USD 12.34 ≈ DOP 740.40 a tasa 60.00 · estimado');
    assert.equal(notaDeDolares('1234.56', '60.00'), 'incluye USD 1,234.56 ≈ DOP 74,073.60 a tasa 60.00 · estimado');
});

test('la tabla es accesible: caption, th con scope y los importes con separador de miles', () => {
    const html = dibujar();
    assert.match(html, /<table[^>]*>\s*<caption[^>]*>Gasto por categoría de febrero 2027/);
    assert.match(html, /<th scope="col"[^>]*>Categoría<\/th>/);
    assert.match(html, /<th scope="row"[^>]*>Comida/);
    assert.match(html, /1,234\.57/);
    assert.match(html, /Total DOP 2,024\.68/);
    assert.doesNotMatch(html, /\son[a-z]+=/, 'sin manejadores en línea');
});

test('variación: sube/baja/igual con flecha y texto; sin referencia muestra «—»', () => {
    const html = dibujar(resumen({
        filas: [
            fila(1, 'Sube', { variacion_anterior_pb: 1050, variacion_promedio_pb: null }),
            fila(2, 'Baja', { variacion_anterior_pb: -300, variacion_promedio_pb: 0 }),
        ],
    }));
    assert.match(html, /▲<\/span> sube 10\.50 %/);
    assert.match(html, /▼<\/span> baja 3\.00 %/);
    assert.match(html, /rg-igual[^>]*>[^<]*<span aria-hidden="true">=<\/span> igual/);
    assert.match(html, /rg-sin-referencia[^>]*title="Sin referencia[^>]*>—<\/span>/);
});

test('mes parcial: etiqueta «parcial, hasta el día N» y la comparación contra el mismo tramo', () => {
    const html = dibujar(resumen({
        parcial: true, hasta_dia: 9,
        filas: [fila(1, 'Comida', { anterior_mismo_dia: '400.40', variacion_mismo_dia_pb: 2500 })],
    }));
    assert.match(html, /<span class="badge[^>]*>parcial, hasta el día 9<\/span>/, 'la etiqueta junto al total');
    assert.match(html, /<caption[^>]*>[^<]*parcial, hasta el día 9/, 'y en el título de la tabla');
    assert.match(html, /al día 9 del mes anterior: DOP 400\.40[^]*sube 25\.00 %/);
    assert.doesNotMatch(dibujar(), /parcial/);
});

test('la parte en dólares va aparte, en la fila y en el total, con la tasa y la marca de estimado', () => {
    const html = dibujar(resumen({
        filas: [fila(1, 'Suscripciones', { usd_aparte: '12.34' })],
        totales: medidas({ importe: '2024.68', usd_aparte: '12.34' }),
    }));
    assert.match(html, /USD 12\.34<\/small>/);
    assert.match(html, /incluye USD 12\.34 ≈ DOP 740\.40 a tasa 60\.00 · estimado/);
    assert.doesNotMatch(dibujar(), /estimado/, 'sin dólares no hay nota');
});

test('«Cargos derivados» lleva su nota y las barras son imágenes con texto alternativo', () => {
    const html = dibujar();
    assert.match(html, /Cargos derivados<small[^>]*>Comisiones de abonos y cargos de avances/);
    assert.match(html, /role="img" aria-label="Comida: 33\.3 % del total"/);
    assert.match(html, /width:33\.3%/);
});

test('estados: cargando, mes sin gastos y error, con los controles siempre presentes', () => {
    const cargando = htmlDeGastoPorCategoria({ tipo: 'cargando' }, eleccion());
    assert.match(cargando, /role="status"[^>]*>Cargando el gasto de febrero 2027/);
    const vacio = dibujar(resumen({ filas: [] }));
    assert.match(vacio, /Sin gastos en febrero 2027/);
    assert.doesNotMatch(vacio, /<table/);
    const error = htmlDeGastoPorCategoria({ tipo: 'error', mensaje: 'falló <b>Rust</b>' }, eleccion());
    assert.match(error, /role="alert"/);
    assert.match(error, /falló &lt;b&gt;Rust&lt;\/b&gt;/);
    for (const h of [cargando, vacio, error]) assert.match(h, /id="rg-mes"/);
});

test('controles: meses disponibles (más el elegido), anterior/siguiente deshabilitados en los extremos, N y conmutador', () => {
    const html = htmlDeGastoPorCategoria({ tipo: 'cargando' }, eleccion({ anio: 2027, mes: 3 }));
    assert.match(html, /<option value="2027-03" selected>marzo 2027/);
    assert.match(html, /<option value="2027-01">enero 2027/);
    assert.match(html, /id="rg-mes-siguiente"[^>]*disabled/);
    assert.doesNotMatch(html, /id="rg-mes-anterior"[^>]*disabled/);
    assert.match(html, /<option value="3" selected>3 meses/);
    assert.match(html, /<option value="2">2 meses/);
    assert.match(html, /<option value="6">6 meses/);
    assert.match(html, /id="rg-incluir-todo"[^>]*checked/);
    assert.match(html, /Incluir inversión y obligaciones/);
    assert.doesNotMatch(htmlDeGastoPorCategoria({ tipo: 'cargando' }, eleccion({ incluirTodo: false })), /id="rg-incluir-todo"[^>]*checked/);
});

test('con el conmutador apagado se nombran las categorías que quedaron fuera', () => {
    const html = dibujar(resumen(), eleccion({ incluirTodo: false, excluidas: ['Impuestos', 'Renta'] }));
    assert.match(html, /Fuera de la tabla: Impuestos, Renta/);
});

test('un nombre de categoría hostil se escapa en la celda, en el aria-label y no abre etiquetas', () => {
    const hostil = `<img src=x onerror=alert(1)> "'&`;
    const html = dibujar(resumen({ filas: [fila(1, hostil)] }));
    assert.doesNotMatch(html, /<img/);
    assert.match(html, /&lt;img src=x onerror=alert\(1\)&gt; &quot;&#39;&amp;/);
    assert.doesNotMatch(html, /aria-label="[^"]*"[^>]*onerror/);
});

// --- Tendencia de 6 meses ---

const pts = importes => importes.map(importe => ({ importe }));

test('alturasDeBarras: escala al máximo de la fila; cero mide 0; con gasto, al menos el piso; ausente es null', () => {
    assert.deepEqual(alturasDeBarras(pts(['100.00', '50.00', '0.00', null, '0.01', '100.00'])), [100, 50, 0, null, 6, 100]);
    assert.deepEqual(alturasDeBarras(pts(['333.33', '1000.01'])), [33, 100], 'división entera sobre centavos');
    assert.deepEqual(alturasDeBarras(pts(['0.00', '0.00', null])), [0, 0, null], 'sin máximo positivo no hay barras');
    assert.deepEqual(alturasDeBarras(pts(['-5.00', '10.00'])), [0, 100], 'un negativo no dibuja barra');
    assert.deepEqual(alturasDeBarras(pts(['900719925474099.99', '450359962737049.99'])), [100, 49], 'enteros grandes sin Number (división entera)');
    assert.deepEqual(alturasDeBarras(pts(['basura', '10.00'])), [0, 100], 'un texto ilegible no rompe');
    assert.deepEqual(alturasDeBarras([]), []);
});

test('etiquetaDeDireccion: flecha Y texto; lo desconocido es «datos insuficientes»', () => {
    assert.deepEqual(etiquetaDeDireccion('sube'), { direccion: 'sube', flecha: '▲', texto: 'sube' });
    assert.deepEqual(etiquetaDeDireccion('baja'), { direccion: 'baja', flecha: '▼', texto: 'baja' });
    assert.deepEqual(etiquetaDeDireccion('estable'), { direccion: 'estable', flecha: '=', texto: 'estable' });
    assert.deepEqual(etiquetaDeDireccion('datos_insuficientes'), { direccion: 'datos_insuficientes', flecha: '', texto: 'datos insuficientes' });
    assert.equal(etiquetaDeDireccion('<b>x</b>').texto, 'datos insuficientes');
});

test('«estable» dice por qué: poco cambio (menos de DOP 500) o dentro del umbral; el motivo no afecta a otras direcciones', () => {
    assert.equal(etiquetaDeDireccion('estable', 'poco_cambio').texto, 'estable (cambio menor de DOP 500)');
    assert.equal(etiquetaDeDireccion('estable', 'dentro_del_umbral').texto, 'estable');
    assert.equal(etiquetaDeDireccion('estable', null).texto, 'estable');
    assert.equal(etiquetaDeDireccion('estable', '<b>x</b>').texto, 'estable', 'un motivo desconocido no se vuelca');
    assert.equal(etiquetaDeDireccion('sube', 'poco_cambio').texto, 'sube');
    assert.equal(etiquetaDeDireccion('datos_insuficientes', 'poco_cambio').texto, 'datos insuficientes');
    const t = tendencia(['20.00', '20.00', '20.00', '30.00', '30.00', '30.00'], 'estable', false, 'poco_cambio');
    assert.match(descripcionDeTendencia('Café', t), /^Café: estable \(cambio menor de DOP 500\); últimos meses: /);
    const html = htmlDeTendencia('Café', t);
    assert.match(html, /aria-label="Café: estable \(cambio menor de DOP 500\);/);
    assert.match(html, /rg-estable[^>]*><span aria-hidden="true">=<\/span> estable \(cambio menor de DOP 500\)/);
    assert.doesNotMatch(htmlDeTendencia('Café', tendencia(undefined, 'estable', false, 'dentro_del_umbral')), /cambio menor/);
});

test('descripcionDeTendencia: nombre, dirección y cada mes con su importe en texto; ausente y parcial dicen lo suyo', () => {
    const t = tendencia([null, '1234.57', '0.00', '80.01'], 'sube', true);
    const d = descripcionDeTendencia('Supermercado', t);
    assert.match(d, /^Supermercado: sube; últimos meses: /);
    assert.match(d, /noviembre 2026 sin datos/);
    assert.match(d, /diciembre 2026 DOP 1,234\.57/);
    assert.match(d, /enero 2027 DOP 0\.00/);
    assert.match(d, /febrero 2027 \(parcial, no cuenta en la dirección\) DOP 80\.01/);
});

test('htmlDeTendencia: una barra por mes con su altura, el parcial rayado, el ausente como hueco, texto y aria-label', () => {
    const html = htmlDeTendencia('Comida', tendencia([null, '100.00', '50.00', '0.00', '25.00', '75.00'], 'baja', true));
    assert.equal((html.match(/class="rg-t-barra[ "]/g) ?? []).length, 5, 'cinco barras (la ausente no dibuja)');
    assert.equal((html.match(/rg-t-hueco/g) ?? []).length, 1);
    assert.equal((html.match(/rg-t-parcial/g) ?? []).length, 1);
    assert.match(html, /height:100%/);
    assert.match(html, /height:50%/);
    assert.match(html, /height:0%/);
    assert.match(html, /repeating-linear-gradient/, 'el parcial lleva trama, no color distinto');
    assert.match(html, /role="img" aria-label="Comida: baja; últimos meses: /);
    assert.match(html, /▼<\/span> baja/);
    assert.match(html, /var\(--color-success/);
    assert.doesNotMatch(html, /\bon[a-z]+=/, 'sin manejadores en línea');
});

test('la dirección «sube» usa el color de alerta y «datos insuficientes» no lleva flecha', () => {
    assert.match(htmlDeTendencia('X', tendencia(undefined, 'sube')), /var\(--color-danger/);
    const sin = htmlDeTendencia('X', tendencia(['1.00', '2.00'], 'datos_insuficientes'));
    assert.match(sin, /datos insuficientes/);
    assert.doesNotMatch(sin, /▲|▼/);
});

test('la tabla trae la columna «Tendencia 6m», una celda por fila y la del total; desplazable y con ancho mínimo', () => {
    const html = dibujar();
    assert.match(html, /<th scope="col" title="[^"]*10 %[^"]*DOP 500[^"]*">Tendencia 6m /);
    assert.match(html, /al menos DOP 500 al mes\)/, 'el pie de la tabla resume la regla');
    assert.equal((html.match(/class="rg-tendencia"/g) ?? []).length, 4, 'tres filas y el total');
    assert.match(html, /aria-label="Total: estable; últimos meses: /);
    assert.match(html, /aria-label="Cargos derivados: sube; /);
    assert.match(html, /class="rg-desplazable" role="region" tabindex="0"/);
    assert.match(html, /min-width:46rem/);
});

test('un nombre hostil no sale sin escapar del aria-label ni del título de la tendencia', () => {
    const hostil = `"><script>alert(1)</script> '&`;
    const html = dibujar(resumen({ filas: [fila(1, hostil)] }));
    assert.doesNotMatch(html, /<script/);
    assert.match(html, /aria-label="&lt;|aria-label="&quot;&gt;&lt;script&gt;/);
    const suelto = htmlDeTendencia(hostil, tendencia());
    assert.doesNotMatch(suelto, /<script/);
    assert.doesNotMatch(suelto, /aria-label="[^"]*"[^>]*<script/);
});

test('los estados cargando y error no dibujan tendencia', () => {
    for (const estado of [{ tipo: 'cargando' }, { tipo: 'error', mensaje: 'x' }]) {
        assert.doesNotMatch(htmlDeGastoPorCategoria(estado, eleccion()), /rg-tendencia/);
    }
    const vacio = dibujar(resumen({ filas: [] }));
    assert.doesNotMatch(vacio, /rg-tendencia/);
});

// --- Fijo/recurrente y variable ---

const FIJAS = () => resumen({
    filas: [
        fila(1, 'Comida', { importe: '1500.10', porcentaje_milesimas: 600 }),
        fila(3, 'Seguros', { importe: '800.07', porcentaje_milesimas: 320 }, false, tendencia(), true),
        fila(-1, 'Cargos derivados', { importe: '50.03', porcentaje_milesimas: 20 }, true),
        fila(4, 'Suscripciones', { importe: '150.00', porcentaje_milesimas: 60 }, false, tendencia(), true),
    ],
    totales: medidas({ importe: '2500.20', porcentaje_milesimas: 1000 }),
    totales_fijo: segmento({ importe: '950.07', porcentaje_milesimas: 380, anterior: '900.01', variacion_anterior_pb: 556 }),
    totales_variable: segmento({ importe: '1550.13', porcentaje_milesimas: 620, anterior: '1700.02', variacion_anterior_pb: -882 }),
});

test('la categoría fija lleva la marca «fija / recurrente» con símbolo y texto; las variables y los derivados no', () => {
    const html = dibujar(FIJAS());
    const filas = html.split('<tr').filter(t => t.includes('<th scope="row"'));
    const conMarca = filas.filter(t => t.includes('rg-marca-fija')).map(t => /<th scope="row"[^>]*>([^<]+)/.exec(t)[1].trim());
    assert.deepEqual(conMarca, ['Seguros', 'Suscripciones']);
    assert.match(html, /<span aria-hidden="true">↻<\/span> fija \/ recurrente/);
    assert.ok(html.includes(`title="${AYUDA_DE_FIJAS}"`), 'la ayuda dice que el monto puede cambiar');
    assert.match(AYUDA_DE_FIJAS, /aunque el monto cambie un poco/);
});

test('la línea de resumen: Fijo/recurrente y Variable con importe, porcentaje y variación en flecha y texto', () => {
    const html = dibujar(FIJAS());
    assert.match(html, /Fijo\/recurrente: DOP 950\.07 \(38\.0 %\)/);
    assert.match(html, /Variable: DOP 1,550\.13 \(62\.0 %\)/);
    assert.match(html, /▲<\/span> sube 5\.56 %/);
    assert.match(html, /▼<\/span> baja 8\.82 %/);
    assert.match(html, /frente al mes anterior \(DOP 900\.01\)/);
    assert.equal(textoDeSegmento('Variable', FIJAS().totales_variable), 'Variable: DOP 1,550.13 (62.0 %)');
});

test('sin mes anterior la variación dice «—» y sin ninguna fija todo el total es variable', () => {
    const r = resumen();
    const html = htmlDeFijoYVariable(r);
    assert.match(html, /Fijo\/recurrente: DOP 0\.00 \(0\.0 %\)[\s\S]*—/);
    assert.match(html, /Variable: DOP 2,024\.68 \(100\.0 %\)/);
    assert.doesNotMatch(dibujar(r), /rg-marca-fija/);
});

test('el filtro solo filtra filas: los totales, la línea de resumen y el pie no cambian, y lo avisa', () => {
    const todas = dibujar(FIJAS(), eleccion({ filtro: 'todas' }));
    const soloFijas = dibujar(FIJAS(), eleccion({ filtro: 'fijas' }));
    const soloVariables = dibujar(FIJAS(), eleccion({ filtro: 'variables' }));
    const nombres = h => [...h.matchAll(/<th scope="row" style="text-align:left;">([^<]+)/g)].map(m => m[1].trim());
    assert.deepEqual(nombres(todas), ['Comida', 'Seguros', 'Suscripciones', 'Cargos derivados', 'Total del mes']);
    assert.deepEqual(nombres(soloFijas), ['Seguros', 'Suscripciones', 'Total del mes (todas las categorías)']);
    assert.deepEqual(nombres(soloVariables), ['Comida', 'Cargos derivados', 'Total del mes (todas las categorías)']);
    for (const h of [todas, soloFijas, soloVariables]) {
        assert.match(h, /Total DOP 2,500\.20/);
        assert.match(h, /<td style="text-align:right;"><strong>2,500\.20<\/strong><\/td>/);
        assert.match(h, /Fijo\/recurrente: DOP 950\.07/);
        assert.match(h, /Variable: DOP 1,550\.13/);
    }
    assert.doesNotMatch(todas, /rg-aviso-filtro/);
    assert.match(soloFijas, /rg-aviso-filtro[^>]*>Filtro activo: la tabla muestra solo las categorías fijas o recurrentes\. Es solo un filtro de filas: el total y la línea de arriba son los del mes completo/);
    assert.match(soloVariables, /solo las categorías variables/);
});

test('un filtro sin filas lo dice en la tabla, sin romperla', () => {
    const sinFijas = dibujar(resumen(), eleccion({ filtro: 'fijas' }));
    assert.match(sinFijas, /Ninguna categoría fija o recurrente tuvo gasto en febrero 2027/);
    assert.match(sinFijas, /<table/);
});

test('el selector del filtro: tres opciones, la elegida marcada, presente también cargando; filasFiltradas y filtroDeValor', () => {
    const html = htmlDeGastoPorCategoria({ tipo: 'cargando' }, eleccion({ filtro: 'fijas' }));
    assert.match(html, /id="rg-filtro"/);
    assert.match(html, /<option value="todas">Todas<\/option>/);
    assert.match(html, /<option value="fijas" selected>Solo fijas o recurrentes<\/option>/);
    assert.match(html, /<option value="variables">Solo variables<\/option>/);
    assert.match(html, /resumenGastoFiltro/);
    const filas = FIJAS().filas;
    assert.deepEqual(filasFiltradas(filas, 'fijas').map(f => f.categoria_id), [3, 4]);
    assert.deepEqual(filasFiltradas(filas, 'variables').map(f => f.categoria_id), [1, -1]);
    assert.equal(filasFiltradas(filas, 'todas').length, 4);
    assert.deepEqual(['fijas', 'variables', 'todas', 'otra cosa', ''].map(filtroDeValor), ['fijas', 'variables', 'todas', 'todas', 'todas']);
});

test('la marca, la línea de resumen y el aviso escapan un nombre hostil y no llevan manejadores en línea', () => {
    const hostil = `<img src=x onerror=alert(1)> "'&`;
    const html = dibujar(resumen({ filas: [fila(1, hostil, {}, false, tendencia(), true)] }), eleccion({ filtro: 'fijas' }));
    assert.doesNotMatch(html, /<img/);
    assert.match(html, /rg-marca-fija/);
    assert.doesNotMatch(html.replace(/"[^"]*"/g, '""'), /<[a-z][^>]*\son\w+=/i, 'ninguna etiqueta lleva atributos on*= (los valores entrecomillados no cuentan)');
});
