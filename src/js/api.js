// --- MICHELITOS TAURI - PUENTE DE COMUNICACIÓN CON EL BACKEND DE RUST (IPC) ---

const { invoke } = window.__TAURI__ ? window.__TAURI__.tauri : { invoke: async () => [] };

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

    async crearTarjeta(entidad, nombre, limite, balance, corte, pago) {
        return await invoke('crear_tarjeta', {
            entidad,
            nombre,
            limite: Number(limite),
            balance: Number(balance),
            corte: Number(corte),
            pago: Number(pago)
        });
    },

    async actualizarLimiteTarjeta(id, limite) {
        return await invoke('actualizar_limite_tarjeta', {
            id: Number(id),
            limite: Number(limite)
        });
    },

    async registrarPagoTarjeta(id, fecha, monto) {
        return await invoke('registrar_pago_tarjeta', {
            id: Number(id),
            fecha,
            monto: Number(monto)
        });
    },

    // --- SUSCRIPCIONES ---
    async obtenerSuscripciones() {
        return await invoke('obtener_suscripciones');
    },

    async crearSuscripcion(plataforma, monto, tarjetaId, frecuencia) {
        return await invoke('crear_suscripcion', {
            plataforma,
            monto: Number(monto),
            tarjetaId: Number(tarjetaId),
            frecuencia
        });
    },

    async eliminarSuscripcion(id) {
        return await invoke('eliminar_suscripcion', { id: Number(id) });
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
    }
};
