// --- MICHELITOS TAURI - PUENTE DE COMUNICACIÓN CON EL BACKEND DE RUST (IPC) ---

const invoke = (window.__TAURI__ && window.__TAURI__.invoke) || 
               (window.__TAURI__ && window.__TAURI__.tauri && window.__TAURI__.tauri.invoke) || 
               (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) ||
               (async (cmd) => {
                   if (cmd === 'obtener_capital') {
                       return {
                           propiedades: { inmobiliario: [], vehiculos: [], maquinaria: [] },
                           certificados: [],
                           bolsa: []
                       };
                   }
                   return [];
               });

const AppAPI = {
    // --- CATEGORÍAS ---
    async obtenerCategorias() {
        return await invoke('obtener_categorias');
    },

    async crearCategoria(nombre) {
        return await invoke('crear_categoria', { nombre });
    },

    async eliminarCategoria(id) {
        return await invoke('eliminar_categoria', { id: Number(id) });
    },

    // --- CLIENTES ---
    async obtenerClientes() {
        return await invoke('obtener_clientes');
    },

    async crearCliente(rnc, nombre) {
        return await invoke('crear_cliente', { rnc, nombre });
    },

    async eliminarCliente(id) {
        return await invoke('eliminar_cliente', { id: Number(id) });
    },

    // --- CUENTAS DE AHORRO ---
    async obtenerCuentas() {
        return await invoke('obtener_cuentas');
    },

    async crearCuenta(nombre, divisa, balance) {
        return await invoke('crear_cuenta', { nombre, divisa, balance: Number(balance) });
    },

    async eliminarCuenta(id) {
        return await invoke('eliminar_cuenta', { id: Number(id) });
    },

    async transferirEntreCuentas(fecha, origenId, destinoId, montoOrigen, montoDestino, cargo, descripcion) {
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

    async crearGasto(gastoData) {
        return await invoke('crear_gasto', { input: gastoData });
    },

    // --- INGRESOS FORMALES ---
    async obtenerIngresos() {
        return await invoke('obtener_ingresos');
    },

    async crearIngreso(ingresoData) {
        return await invoke('crear_ingreso', { input: ingresoData });
    },

    async actualizarIngreso(id, numeroFactura, clienteId, fechaEmision, montoTotal, porcentajeRetencion) {
        return await invoke('actualizar_ingreso', {
            id: Number(id),
            numeroFactura,
            clienteId: Number(clienteId),
            fechaEmision,
            montoTotal: Number(montoTotal),
            porcentajeRetencion: Number(porcentajeRetencion)
        });
    },

    async marcarIngresoPagado(id, institucion, fecha, montoRecibido) {
        return await invoke('marcar_ingreso_pagado', {
            id: Number(id),
            institucion,
            fecha,
            montoRecibido: Number(montoRecibido)
        });
    },

    // --- INGRESOS INFORMALES ---
    async obtenerIngresosInformales() {
        return await invoke('obtener_ingresos_informales');
    },

    async crearIngresoInformal(fecha, descripcion, monto) {
        return await invoke('crear_ingreso_informal', {
            fecha,
            descripcion,
            monto: Number(monto)
        });
    },

    async marcarInformalPagado(id, institucion, fecha, montoRecibido) {
        return await invoke('marcar_informal_pagado', {
            id: Number(id),
            institucion,
            fecha,
            montoRecibido: Number(montoRecibido)
        });
    },

    // --- TARJETAS ---
    async obtenerTarjetas() {
        return await invoke('obtener_tarjetas');
    },

    async crearTarjeta(entidad, nombre, limitePesos, limiteDolares, sobregiroPesos, sobregiroDolares, balancePesos, balanceDolares, balanceCortePesos, balanceCorteDolares, corte, pago) {
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

    async actualizarLimitesTarjeta(id, limitePesos, limiteDolares, sobregiroPesos, sobregiroDolares, balanceCortePesos, balanceCorteDolares) {
        return await invoke('actualizar_limites_tarjeta', {
            id: Number(id),
            limitePesos: Number(limitePesos),
            limiteDolares: Number(limiteDolares),
            sobregiroPesos: Number(sobregiroPesos),
            sobregiroDolares: Number(sobregiroDolares),
            balanceCortePesos: Number(balanceCortePesos),
            balanceCorteDolares: Number(balanceCorteDolares)
        });
    },

    async registrarPagoTarjeta(id, fec, mon, div, cuentaAhorroId = null, tasaCambio = 0) {
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

    async crearSuscripcion(plataforma, monto, tarjetaId, frecuencia, diaFacturacion, divisa) {
        return await invoke('crear_suscripcion', {
            plataforma,
            monto: Number(monto),
            tarjetaId: Number(tarjetaId),
            frecuencia,
            diaFacturacion: Number(diaFacturacion),
            divisa
        });
    },

    async eliminarSuscripcion(id) {
        return await invoke('eliminar_suscripcion', { id: Number(id) });
    },

    async procesarSuscripciones() {
        return await invoke('procesar_suscripciones');
    },

    // --- CAPITAL (NoSQL) ---
    async obtenerCapital() {
        return await invoke('obtener_capital');
    },

    async guardarCapital(capitalData) {
        return await invoke('guardar_capital', { data: capitalData });
    },

    // --- FINANCIAMIENTOS / DEUDAS ---
    async obtenerPrestamos() {
        return await invoke('obtener_prestamos');
    },

    async crearPrestamo(prestamoData) {
        return await invoke('crear_prestamo', { input: prestamoData });
    },

    async pagarCuotaPrestamo(id) {
        return await invoke('pagar_cuota_prestamo', { id: Number(id) });
    },

    async eliminarPrestamo(id) {
        return await invoke('eliminar_prestamo', { id: Number(id) });
    },

    // --- EFECTIVO ---
    async crearCobroEfectivoInformal(fecha, descripcion, monto, divisa) {
        return await invoke('crear_cobro_efectivo_informal', {
            fecha,
            descripcion,
            monto: Number(monto),
            divisa
        });
    },

    // --- CORRECCIONES ---
    async eliminarGasto(id) {
        return await invoke('eliminar_gasto', { id: Number(id) });
    },

    async eliminarTransaccionCuenta(id) {
        return await invoke('eliminar_transaccion_cuenta', { id: Number(id) });
    },

    async eliminarIngresoInformal(id) {
        return await invoke('eliminar_ingreso_informal', { id: Number(id) });
    },

    async eliminarIngreso(id) {
        return await invoke('eliminar_ingreso', { id: Number(id) });
    }
};
