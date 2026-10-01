// --- MICHELITOS TAURI - PUENTE DE COMUNICACIÓN CON EL BACKEND DE RUST (IPC) ---

// Los tipos de cada comando (argumentos y respuesta) salen de Rust: ver
// `tipos-ipc.js`, generado por `herramientas/generar_tipos_ipc.mjs`.
type Comandos = import('./tipos-ipc').Comandos;

const tauri = window.__TAURI__;

const invoke: <K extends keyof Comandos>(
    comando: K,
    argumentos?: Comandos[K]['args'],
) => Promise<Comandos[K]['ret']> = ((tauri && tauri.invoke) ||
               (tauri && tauri.tauri && tauri.tauri.invoke) ||
               (tauri && tauri.core && tauri.core.invoke) ||
               (async (cmd: string) => {
                   if (cmd === 'obtener_capital') {
                       return {
                           propiedades: { inmobiliario: [], vehiculos: [], maquinaria: [] },
                           certificados: [],
                           bolsa: []
                       };
                   }
                   return [];
               })) as any;

const AppAPI = {
    // --- CATEGORÍAS ---
    async obtenerCategorias() {
        return await invoke('obtener_categorias');
    },

    async crearCategoria(nombre: string) {
        return await invoke('crear_categoria', { nombre });
    },

    async eliminarCategoria(id: number | string) {
        return await invoke('eliminar_categoria', { id: Number(id) });
    },

    // --- CLIENTES ---
    async obtenerClientes() {
        return await invoke('obtener_clientes');
    },

    async crearCliente(rnc: string, nombre: string) {
        return await invoke('crear_cliente', { rnc, nombre });
    },

    async eliminarCliente(id: number | string) {
        return await invoke('eliminar_cliente', { id: Number(id) });
    },

    // --- CUENTAS DE AHORRO ---
    async crearRespaldo() {
        return await invoke('crear_respaldo');
    },

    async listarRespaldos() {
        return await invoke('listar_respaldos');
    },

    async restaurarRespaldo(nombre: string) {
        return await invoke('restaurar_respaldo', { nombre: String(nombre) });
    },

    async obtenerCuentas() {
        return await invoke('obtener_cuentas');
    },

    async obtenerAbonosTarjeta(tarjetaId: number | string) {
        return await invoke('obtener_abonos_tarjeta', { tarjetaId: Number(tarjetaId) });
    },

    async revertirAbonoTarjeta(id: number | string, motivo: string) {
        return await invoke('revertir_abono_tarjeta', { id: Number(id), motivo });
    },

    // --- AVANCES DE EFECTIVO ---
    //
    // El monto y el cargo fijo viajan como **texto**, tal como se escribieron:
    // el céntimo lo decide el núcleo con sus dígitos y no una conversión a
    // coma flotante. El porcentaje es una tasa, no un importe, y va como número.

    // Calcula el cargo sin guardar nada, para enseñarlo antes de confirmar. La
    // regla vive en el núcleo: la interfaz no la duplica.
    async simularAvanceEfectivo(monto: string, divisa: string, tipoCargo: string, porcentaje: number | null | undefined, cargoFijo: string | number | null | undefined) {
        return await invoke('simular_avance_efectivo', {
            monto: String(monto),
            divisa,
            tipoCargo,
            porcentaje: porcentaje ?? null,
            cargoFijo: cargoFijo ?? null
        });
    },

    async registrarAvanceEfectivo(tarjetaId: number | string, cuentaAhorroId: number | string, fecha: string, monto: string, divisa: string, tipoCargo: string, porcentaje: number | null | undefined, cargoFijo: string | number | null | undefined, nota: string | null | undefined) {
        return await invoke('registrar_avance_efectivo', {
            tarjetaId: Number(tarjetaId),
            cuentaAhorroId: Number(cuentaAhorroId),
            fecha,
            monto: String(monto),
            divisa,
            tipoCargo,
            porcentaje: porcentaje ?? null,
            cargoFijo: cargoFijo ?? null,
            nota: nota || null
        });
    },

    async obtenerAvancesTarjeta(tarjetaId: number | string) {
        return await invoke('obtener_avances_tarjeta', { tarjetaId: Number(tarjetaId) });
    },

    async revertirAvanceEfectivo(id: number | string, motivo: string) {
        return await invoke('revertir_avance_efectivo', { id: Number(id), motivo });
    },

    async crearCuenta(nombre: string, divisa: string, balance: number | string, entidad: string | null | undefined, comisionPagoImpuestos: string | number | null | undefined) {
        return await invoke('crear_cuenta', {
            nombre,
            divisa,
            balance: Number(balance),
            entidad: entidad || null,
            // Texto, por el mismo motivo que el saldo del préstamo.
            comisionPagoImpuestos: comisionPagoImpuestos ?? null
        });
    },

    async actualizarCuenta(id: number | string, nombre: string, entidad: string | null | undefined, comisionPagoImpuestos: string | number | null | undefined) {
        return await invoke('actualizar_cuenta', {
            id: Number(id),
            nombre,
            entidad: entidad || null,
            comisionPagoImpuestos: comisionPagoImpuestos ?? null
        });
    },

    async eliminarCuenta(id: number | string) {
        return await invoke('eliminar_cuenta', { id: Number(id) });
    },

    async transferirEntreCuentas(fecha: string, origenId: number | string, destinoId: number | string, montoOrigen: number | string, montoDestino: number | string, cargo: number | string, descripcion: string) {
        return await invoke('transferir_entre_cuentas', {
            fecha,
            origenId: Number(origenId),
            destinoId: Number(destinoId),
            montoOrigen: Number(montoOrigen),
            montoDestino: Number(montoDestino),
            cargo: Number(cargo),
            descripcion
        });
    },

    async obtenerTransaccionesCuentas() {
        return await invoke('obtener_transacciones_cuentas');
    },

    // --- GASTOS ---
    async obtenerGastos() {
        return await invoke('obtener_gastos');
    },

    async crearGasto(gastoData: import('./tipos-ipc').GastoInput) {
        return await invoke('crear_gasto', { input: gastoData });
    },

    // --- INGRESOS FORMALES ---
    async obtenerIngresos() {
        return await invoke('obtener_ingresos');
    },

    async crearIngreso(ingresoData: import('./tipos-ipc').IngresoInput) {
        return await invoke('crear_ingreso', { input: ingresoData });
    },

    async actualizarIngreso(id: number | string, numeroFactura: string, clienteId: number | string, fechaEmision: string, montoTotal: number | string, porcentajeRetencion: number | string, cobroParcial: string | number | null | undefined, motivo: string | null | undefined) {
        return await invoke('actualizar_ingreso', {
            id: Number(id),
            numeroFactura,
            clienteId: Number(clienteId),
            fechaEmision,
            montoTotal: Number(montoTotal),
            porcentajeRetencion: Number(porcentajeRetencion),
            // Nulo es la regla: se da por cobrado el neto entero.
            cobroParcial: cobroParcial ?? null,
            // Solo hace falta cuando la corrección mueve un saldo.
            motivo: motivo ?? null
        });
    },

    async marcarIngresoPagado(id: number | string, cuentaAhorroId: number | string, fecha: string, montoRecibido: number | string) {
        return await invoke('marcar_ingreso_pagado', {
            id: Number(id),
            cuentaAhorroId: Number(cuentaAhorroId),
            fecha,
            montoRecibido: Number(montoRecibido)
        });
    },

    // --- INGRESOS INFORMALES ---
    async obtenerIngresosInformales() {
        return await invoke('obtener_ingresos_informales');
    },

    async crearIngresoInformal(fecha: string, descripcion: string, monto: number | string) {
        return await invoke('crear_ingreso_informal', {
            fecha,
            descripcion,
            monto: Number(monto)
        });
    },

    async marcarInformalPagado(id: number | string, cuentaAhorroId: number | string, fecha: string, montoRecibido: number | string) {
        return await invoke('marcar_informal_pagado', {
            id: Number(id),
            cuentaAhorroId: Number(cuentaAhorroId),
            fecha,
            montoRecibido: Number(montoRecibido)
        });
    },

    // --- TARJETAS ---
    async obtenerTarjetas() {
        return await invoke('obtener_tarjetas');
    },

    async crearTarjeta(entidad: string, nombre: string, limitePesos: number | string, limiteDolares: number | string, sobregiroPesos: number | string, sobregiroDolares: number | string, balancePesos: number | string, balanceDolares: number | string, balanceCortePesos: number | string, balanceCorteDolares: number | string, corte: number | string, pago: number | string) {
        return await invoke('crear_tarjeta', {
            entidad,
            nombre,
            limitePesos: Number(limitePesos),
            limiteDolares: Number(limiteDolares),
            sobregiroPesos: Number(sobregiroPesos),
            sobregiroDolares: Number(sobregiroDolares),
            balancePesos: Number(balancePesos),
            balanceDolares: Number(balanceDolares),
            balanceCortePesos: Number(balanceCortePesos),
            balanceCorteDolares: Number(balanceCorteDolares),
            corte: Number(corte),
            pago: Number(pago)
        });
    },

    // Los límites ajustados son opcionales: null significa "sin ajuste", y se
    // distingue de 0, que es un tope deliberado que congela la tarjeta.
    async actualizarLimitesTarjeta(id: number | string, limitePesos: number | string, limiteDolares: number | string, sobregiroPesos: number | string, sobregiroDolares: number | string, balanceCortePesos: number | string, balanceCorteDolares: number | string, ajustadoPesos: number | string | null = null, ajustadoDolares: number | string | null = null, politicaLiquidacion: string | null | undefined = 'origen') {
        return await invoke('actualizar_limites_tarjeta', {
            id: Number(id),
            limitePesos: Number(limitePesos),
            limiteDolares: Number(limiteDolares),
            sobregiroPesos: Number(sobregiroPesos),
            sobregiroDolares: Number(sobregiroDolares),
            balanceCortePesos: Number(balanceCortePesos),
            balanceCorteDolares: Number(balanceCorteDolares),
            limiteAjustadoPesos: ajustadoPesos === null || ajustadoPesos === '' ? null : Number(ajustadoPesos),
            limiteAjustadoDolares: ajustadoDolares === null || ajustadoDolares === '' ? null : Number(ajustadoDolares),
            politicaLiquidacion
        });
    },

    // --- BONIFICACIONES (cashback, promociones, recompensas) ---
    // Son créditos aparte que reducen la deuda de la tarjeta; nunca alteran
    // el consumo que las originó.
    async obtenerBonificaciones() {
        return await invoke('obtener_bonificaciones');
    },

    async crearBonificacion(fecha: string, tarjetaId: number | string, monto: number | string, divisa: string, concepto: string, gastoId: number | string | null = null) {
        return await invoke('crear_bonificacion', {
            fecha,
            tarjetaId: Number(tarjetaId),
            monto: Number(monto),
            divisa,
            concepto,
            gastoId: gastoId === null || gastoId === '' ? null : Number(gastoId)
        });
    },

    async eliminarBonificacion(id: number | string) {
        return await invoke('eliminar_bonificacion', { id: Number(id) });
    },

    async registrarPagoTarjeta(id: number | string, fec: string, mon: number, div: string, cuentaAhorroId: number | null = null, tasaCambio: number | string = 0) {
        return await invoke('registrar_pago_tarjeta', { 
            id: Number(id), 
            fecha: fec, 
            monto: mon, 
            divisa: div, 
            cuentaAhorroId: cuentaAhorroId ? Number(cuentaAhorroId) : null,
            tasaCambio: Number(tasaCambio || 0)
        });
    },

    // --- SUSCRIPCIONES ---
    async obtenerSuscripciones() {
        return await invoke('obtener_suscripciones');
    },

    // `fechaProximoCobro` va en dd/mm/aaaa y la usan las dos frecuencias:
    // desde que la fecha manda, una mensual la necesita igual que una anual.
    async crearSuscripcion(plataforma: string, monto: number | string, tarjetaId: number | string, frecuencia: string, diaFacturacion: number | string, divisa: string, fechaProximoCobro: string | null = null) {
        return await invoke('crear_suscripcion', {
            plataforma,
            // Texto, tal como se escribió: el céntimo lo deciden los dígitos.
            monto: String(monto),
            tarjetaId: Number(tarjetaId),
            frecuencia,
            diaFacturacion: Number(diaFacturacion),
            divisa,
            fechaProximoCobro
        });
    },

    // Edita una suscripción conservando fecha_ultimo_pago. Borrar y recrear
    // reiniciaría esa marca y provocaría un cobro duplicado en el mismo mes.
    async actualizarSuscripcion(id: number | string, plataforma: string, monto: number | string, tarjetaId: number | string, frecuencia: string, diaFacturacion: number | string, divisa: string, fechaProximoCobro: string | null = null) {
        return await invoke('actualizar_suscripcion', {
            id: Number(id),
            plataforma,
            // Texto, tal como se escribió: el céntimo lo deciden los dígitos.
            monto: String(monto),
            tarjetaId: Number(tarjetaId),
            frecuencia,
            diaFacturacion: Number(diaFacturacion),
            divisa,
            fechaProximoCobro
        });
    },

    // La salida cuando una suscripción se queda sin fecha y por tanto parada.
    async corregirProximoCobro(id: number | string, fecha: string) {
        return await invoke('corregir_proximo_cobro', { id: Number(id), fecha });
    },

    // Los períodos que vencieron sin que la aplicación estuviera abierta no
    // se cobran solos: se confirman uno a uno contra el estado de cuenta.
    async asentarPeriodoPendiente(id: number | string) {
        return await invoke('asentar_periodo_pendiente', { id: Number(id) });
    },

    async descartarPeriodoPendiente(id: number | string, motivo: string) {
        return await invoke('descartar_periodo_pendiente', { id: Number(id), motivo });
    },

    async eliminarSuscripcion(id: number | string) {
        return await invoke('eliminar_suscripcion', { id: Number(id) });
    },

    async procesarSuscripciones() {
        return await invoke('procesar_suscripciones');
    },

    // --- CAPITAL (NoSQL) ---
    async obtenerCapital() {
        return await invoke('obtener_capital');
    },

    // El documento de capital es JSON libre (propiedades, certificados, bolsa…):
    // Rust lo recibe como `Value`, así que aquí el `any` es explícito y deliberado.
    async guardarCapital(capitalData: any) {
        return await invoke('guardar_capital', { data: capitalData });
    },

    // --- FINANCIAMIENTOS / DEUDAS ---
    async obtenerPrestamos() {
        return await invoke('obtener_prestamos');
    },

    async crearPrestamo(prestamoData: import('./tipos-ipc').PrestamoInput) {
        return await invoke('crear_prestamo', { input: prestamoData });
    },

    async pagarCuotaPrestamo(id: number | string) {
        return await invoke('pagar_cuota_prestamo', { id: Number(id) });
    },

    async actualizarPrestamo(datos: import('./tipos-ipc').ActualizarPrestamoInput) {
        return await invoke('actualizar_prestamo', { input: datos });
    },

    async declararSaldoPrestamo(id: number | string, saldo: string) {
        // `saldo` viaja como **texto**: el céntimo lo decide el backend con la
        // regla del sistema, no `Number()` sobre binario.
        return await invoke('declarar_saldo_prestamo', { id: Number(id), saldo: String(saldo) });
    },

    async obtenerMovimientosPrestamo(id: number | string) {
        return await invoke('obtener_movimientos_prestamo', { id: Number(id) });
    },

    async eliminarPrestamo(id: number | string) {
        return await invoke('eliminar_prestamo', { id: Number(id) });
    },

    // --- EFECTIVO ---
    async crearCobroEfectivoInformal(fecha: string, descripcion: string, monto: number | string, divisa: string) {
        return await invoke('crear_cobro_efectivo_informal', {
            fecha,
            descripcion,
            monto: Number(monto),
            divisa
        });
    },

    // --- CORRECCIONES ---
    // El emisor comunica cuánto cargó en moneda local; la tasa la deduce el
    // backend y la devuelve para poder mostrarla.
    async liquidarConsumoPendiente(id: number | string, montoLiquidado: number | string) {
        return await invoke('liquidar_consumo_pendiente', {
            id: Number(id),
            montoLiquidado: Number(montoLiquidado)
        });
    },

    // --- CASOS DE CORRECCIÓN (AUDITORÍA) ---
    async obtenerCorrecciones() {
        return await invoke('obtener_correcciones');
    },

    async eliminarGasto(id: number | string, motivo: string) {
        return await invoke('eliminar_gasto', { id: Number(id), motivo });
    },

    async eliminarTransaccionCuenta(id: number | string, motivo: string) {
        return await invoke('eliminar_transaccion_cuenta', { id: Number(id), motivo });
    },

    async eliminarIngresoInformal(id: number | string, motivo: string) {
        return await invoke('eliminar_ingreso_informal', { id: Number(id), motivo });
    },

    async eliminarIngreso(id: number | string, motivo: string) {
        return await invoke('eliminar_ingreso', { id: Number(id), motivo });
    }
};
