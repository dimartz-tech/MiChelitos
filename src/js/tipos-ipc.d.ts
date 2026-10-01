// GENERADO por herramientas/generar_tipos_ipc.mjs desde src-tauri/src/main.rs. No editar a mano:
// se regenera con `npm run tipos:generar` y una prueba exige que esté al día.
// Solo contiene tipos; no hay código que se ejecute.

// --- Lo que Rust devuelve ---
export type Cliente = { id: number; rnc: string; nombre: string };
export type Ingreso = { id: number; numero_factura: string; cliente_id: number; cliente_nombre: string; cliente_rnc: string; fecha_emision: string; estatus: string; monto_total: number; porcentaje_retencion: number; monto_retenido: number; institucion_deposito: string | null; fecha_pago: string | null; monto_recibido: number | null };
export type IngresoInformal = { id: number; fecha: string; descripcion: string; monto: number; estatus: string; institucion_deposito: string | null; fecha_pago: string | null; monto_recibido: number | null };
export type Gasto = { id: number; fecha: string; monto: number; divisa: string; descripcion: string; categoria_id: number; categoria_nombre: string; metodo_pago: string; costo_adicional: number; tarjeta_id: number | null; cuenta_ahorro_id: number | null; estado_conversion: string | null; monto_liquidado: number | null; tasa_conversion: number | null };
export type BonificacionDto = { id: number; fecha: string; tarjeta_id: number; entidad: string; nombre_tarjeta: string; monto: number; divisa: string; concepto: string; gasto_id: number | null };
export type Categoria = { id: number; nombre: string };
export type Tarjeta = { id: number; entidad: string; nombre_tarjeta: string; limite_pesos: number; limite_dolares: number; limite_ajustado_pesos: number | null; limite_ajustado_dolares: number | null; limite_sobregiro_pesos: number; limite_sobregiro_dolares: number; balance_pesos: number; balance_dolares: number; balance_corte_pesos: number; balance_corte_dolares: number; fecha_corte: number; fecha_limite_pago: number; politica_liquidacion: string; limite_efectivo_pesos: number; limite_efectivo_dolares: number; disponible_pesos: number; disponible_dolares: number; alerta_corte: boolean; alerta_pago: boolean; dias_corte_msg: string; dias_pago_msg: string };
export type Suscripcion = { id: number; plataforma: string; monto: number; tarjeta_id: number; frecuencia: string; dia_facturacion: number; fecha_ultimo_pago: string | null; divisa: string; entidad: string; nombre_tarjeta: string; fecha_proximo_cobro: string | null; pendientes: string[]; avisa: boolean; impedimento: string | null };
export type CuentaAhorro = { id: number; nombre: string; divisa: string; balance_actual: number; entidad: string | null; comision_pago_impuestos: number | null };
export type TransaccionCuenta = { id: number; fecha: string; cuenta_origen_id: number; cuenta_origen_nombre: string; cuenta_destino_id: number; cuenta_destino_nombre: string; monto_origen: number; monto_destino: number; tasa_cambio: number; cargo: number; descripcion: string | null };
export type Prestamo = { id: number; tipo_prestamo: string; monto_prestamo: number; institucion_financiera: string; tasa_actual: number; cuotas_totales: number | null; cuotas_pendientes: number | null; monto_cuota: number; dia_pago: number; saldo_actual: number; limite_credito: number | null; tarjeta_id: number | null; tarjeta_nombre: string | null; dia_corte: number | null; disponible: number | null; es_revolvente: boolean; alerta_pago: boolean; dias_pago_msg: string };
export type AbonoTarjeta = { id: number; fecha_pago: string; monto_pagado: number; divisa: string; cuenta_ahorro_id: number | null; cuenta_nombre: string | null; tasa_cambio: number | null };
export type SimulacionAvance = { monto: number; cargo: number; a_la_tarjeta: number; a_la_cuenta: number };
export type AvanceEfectivo = { id: number; fecha: string; monto: number; divisa: string; tipo_cargo: string; tasa: number | null; cargo: number; cuenta_ahorro_id: number; cuenta_nombre: string; nota: string | null };
export type MovimientoPrestamo = { id: number; fecha: string; tipo: string; monto: number; interes: number; capital: number; saldo_resultante: number };
export type CasoCorreccion = { numero_caso: string; fecha: string; tipo: string; referencia_id: number; descripcion: string; importe: number | null; divisa: string | null; motivo: string };

// --- Lo que Rust recibe en estructuras ---
export type GastoInput = { fecha: string; monto: string | number; divisa: string; descripcion: string; categoria_id: number; metodo_pago: string; es_lbtr: boolean; tarjeta_id: number | null; cuenta_ahorro_id: number | null; tasa_cambio: number | null };
export type IngresoInput = { numero_factura: string; rnc_cliente: string; nombre_cliente: string; fecha_emision: string; monto_total: number; porcentaje_retencion: number };
export type PrestamoInput = { tipo_prestamo: string; monto_prestamo: number; institucion_financiera: string; tasa_actual: number; cuotas_totales: number | null; cuotas_pendientes: number | null; monto_cuota: number; dia_pago: number; saldo_actual: number | null; limite_credito: number | null };
export type ActualizarPrestamoInput = { id: number; tasa_actual: number; monto_cuota: number; dia_pago: number; limite_credito: number | null; tarjeta_id: number | null };

// --- Cada comando: sus argumentos (claves en camelCase, como las envía Tauri) y su respuesta ---
export type Comandos = {
    obtener_categorias: { args: {  }; ret: Categoria[] };
    crear_categoria: { args: { nombre: string }; ret: Categoria };
    eliminar_categoria: { args: { id: number }; ret: null };
    obtener_gastos: { args: {  }; ret: Gasto[] };
    crear_gasto: { args: { input: GastoInput }; ret: number };
    obtener_ingresos: { args: {  }; ret: Ingreso[] };
    crear_ingreso: { args: { input: IngresoInput }; ret: number };
    marcar_ingreso_pagado: { args: { id: number; cuentaAhorroId: number; fecha: string; montoRecibido: string | number }; ret: null };
    obtener_ingresos_informales: { args: {  }; ret: IngresoInformal[] };
    crear_ingreso_informal: { args: { fecha: string; descripcion: string; monto: string | number }; ret: number };
    marcar_informal_pagado: { args: { id: number; cuentaAhorroId: number; fecha: string; montoRecibido: string | number }; ret: null };
    obtener_tarjetas: { args: {  }; ret: Tarjeta[] };
    crear_tarjeta: { args: { entidad: string; nombre: string; limitePesos: string | number; limiteDolares: string | number; sobregiroPesos: string | number; sobregiroDolares: string | number; balancePesos: string | number; balanceDolares: string | number; balanceCortePesos: string | number; balanceCorteDolares: string | number; corte: number; pago: number }; ret: number };
    actualizar_limites_tarjeta: { args: { id: number; limitePesos: string | number; limiteDolares: string | number; sobregiroPesos: string | number; sobregiroDolares: string | number; balanceCortePesos: string | number; balanceCorteDolares: string | number; limiteAjustadoPesos?: string | number | null; limiteAjustadoDolares?: string | number | null; politicaLiquidacion?: string | null }; ret: null };
    registrar_pago_tarjeta: { args: { id: number; fecha: string; monto: string | number; divisa: string; cuentaAhorroId?: number | null; tasaCambio: number }; ret: null };
    obtener_abonos_tarjeta: { args: { tarjetaId: number }; ret: AbonoTarjeta[] };
    revertir_abono_tarjeta: { args: { id: number; motivo: string }; ret: string };
    simular_avance_efectivo: { args: { monto: string | number; divisa: string; tipoCargo: string; porcentaje?: number | null; cargoFijo?: string | number | null }; ret: SimulacionAvance };
    registrar_avance_efectivo: { args: { tarjetaId: number; cuentaAhorroId: number; fecha: string; monto: string | number; divisa: string; tipoCargo: string; porcentaje?: number | null; cargoFijo?: string | number | null; nota?: string | null }; ret: string };
    obtener_avances_tarjeta: { args: { tarjetaId: number }; ret: AvanceEfectivo[] };
    revertir_avance_efectivo: { args: { id: number; motivo: string }; ret: string };
    obtener_suscripciones: { args: {  }; ret: Suscripcion[] };
    crear_suscripcion: { args: { plataforma: string; monto: string | number; tarjetaId: number; frecuencia: string; diaFacturacion: number; divisa: string; fechaProximoCobro?: string | null }; ret: number };
    actualizar_suscripcion: { args: { id: number; plataforma: string; monto: string | number; tarjetaId: number; frecuencia: string; diaFacturacion: number; divisa: string; fechaProximoCobro?: string | null }; ret: null };
    corregir_proximo_cobro: { args: { id: number; fecha: string }; ret: null };
    eliminar_suscripcion: { args: { id: number }; ret: null };
    procesar_suscripciones: { args: {  }; ret: string[] };
    asentar_periodo_pendiente: { args: { id: number }; ret: string };
    descartar_periodo_pendiente: { args: { id: number; motivo: string }; ret: string };
    obtener_capital: { args: {  }; ret: any };
    guardar_capital: { args: { data: any }; ret: null };
    obtener_prestamos: { args: {  }; ret: Prestamo[] };
    crear_prestamo: { args: { input: PrestamoInput }; ret: number };
    actualizar_prestamo: { args: { input: ActualizarPrestamoInput }; ret: null };
    pagar_cuota_prestamo: { args: { id: number; fecha?: string | null }; ret: null };
    declarar_saldo_prestamo: { args: { id: number; saldo: string | number; fecha?: string | null }; ret: null };
    obtener_movimientos_prestamo: { args: { id: number }; ret: MovimientoPrestamo[] };
    eliminar_prestamo: { args: { id: number }; ret: null };
    obtener_clientes: { args: {  }; ret: Cliente[] };
    crear_cliente: { args: { rnc: string; nombre: string }; ret: Cliente };
    eliminar_cliente: { args: { id: number }; ret: null };
    crear_respaldo: { args: {  }; ret: string };
    listar_respaldos: { args: {  }; ret: string[] };
    restaurar_respaldo: { args: { nombre: string }; ret: any };
    obtener_cuentas: { args: {  }; ret: CuentaAhorro[] };
    crear_cuenta: { args: { nombre: string; divisa: string; balance: string | number; entidad?: string | null; comisionPagoImpuestos?: string | number | null }; ret: number };
    actualizar_cuenta: { args: { id: number; nombre: string; entidad?: string | null; comisionPagoImpuestos?: string | number | null }; ret: null };
    eliminar_cuenta: { args: { id: number }; ret: null };
    transferir_entre_cuentas: { args: { fecha: string; origenId: number; destinoId: number; montoOrigen: string | number; montoDestino: string | number; cargo: string | number; descripcion: string }; ret: null };
    obtener_transacciones_cuentas: { args: {  }; ret: TransaccionCuenta[] };
    actualizar_ingreso: { args: { id: number; numeroFactura: string; clienteId: number; fechaEmision: string; montoTotal: string | number; porcentajeRetencion: number; cobroParcial?: string | number | null; motivo?: string | null }; ret: string };
    crear_cobro_efectivo_informal: { args: { fecha: string; descripcion: string; monto: string | number; divisa: string }; ret: number };
    obtener_bonificaciones: { args: {  }; ret: BonificacionDto[] };
    crear_bonificacion: { args: { fecha: string; tarjetaId: number; monto: string | number; divisa: string; concepto: string; gastoId?: number | null }; ret: number };
    eliminar_bonificacion: { args: { id: number }; ret: null };
    liquidar_consumo_pendiente: { args: { id: number; montoLiquidado: string | number }; ret: number };
    obtener_correcciones: { args: {  }; ret: CasoCorreccion[] };
    eliminar_gasto: { args: { id: number; motivo: string }; ret: string };
    eliminar_transaccion_cuenta: { args: { id: number; motivo: string }; ret: string };
    eliminar_ingreso_informal: { args: { id: number; motivo: string }; ret: string };
    eliminar_ingreso: { args: { id: number; motivo: string }; ret: string };
};
