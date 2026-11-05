// Datos inventados del resumen de gastos por categoría (DTO `ResumenGastos`), con céntimos poco redondos.

export const medidas = (extra = {}) => ({
    importe: '1234.57', porcentaje_milesimas: 333, anterior: '1000.03', variacion_anterior_pb: 2345, anterior_mismo_dia: null,
    variacion_mismo_dia_pb: null, promedio: '1100.11', variacion_promedio_pb: -1250, usd_aparte: '0.00', n: 4, ...extra,
});

/** Una serie de 6 meses que termina en febrero de 2027; `importes` de más antiguo a más reciente (`null` = ausente). */
export const tendencia = (importes = ['100.01', '200.02', '150.03', '300.04', '250.05', '120.06'], direccion = 'sube', parcial = false, motivo_estable = null) => ({
    meses: importes.map((importe, i) => {
        const indice = 2027 * 12 + 1 - (importes.length - 1 - i);
        return { anio: Math.floor(indice / 12), mes: (indice % 12) + 1, importe, parcial: parcial && i === importes.length - 1 };
    }),
    direccion,
    motivo_estable,
});

export const fila = (id, nombre, m = {}, derivado = false, t = tendencia()) => ({ categoria_id: id, categoria_nombre: nombre, derivado, medidas: medidas(m), tendencia: t });

export const resumen = (extra = {}) => ({
    anio: 2027, mes: 2, parcial: false, hasta_dia: null, tasa_usd: '60.00', meses_promedio: 3, id_cargos_derivados: -1,
    filas: [fila(1, 'Comida'), fila(-1, 'Cargos derivados', { importe: '12.34', porcentaje_milesimas: 9 }, true), fila(2, 'Transporte', { importe: '777.77', porcentaje_milesimas: 667 })],
    totales: medidas({ importe: '2024.68', porcentaje_milesimas: 1000 }),
    tendencia_total: tendencia(['300.01', '400.02', '350.03', '600.04', '550.05', '420.06'], 'estable'),
    meses_disponibles: [{ anio: 2027, mes: 1 }, { anio: 2027, mes: 2 }],
    ...extra,
});

export const eleccion = (extra = {}) => ({
    anio: 2027, mes: 2, nPromedio: 3, incluirTodo: true, mesesDisponibles: [{ anio: 2027, mes: 1 }, { anio: 2027, mes: 2 }], excluidas: [], ...extra,
});
