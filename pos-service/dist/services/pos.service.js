"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.PosService = void 0;
const database_1 = __importDefault(require("../config/database"));
const AppError_1 = require("../utils/AppError");
const audit_service_1 = require("./audit.service");
const inventory_service_1 = require("./inventory.service");
const axios_1 = __importDefault(require("axios"));
class PosService {
    static async getTurnoActivo(usuarioId) {
        const turno = await database_1.default.turno.findFirst({
            where: { usuarioId, estado: 'ABIERTO' }
        });
        return turno;
    }
    static async abrirTurno(req, montoApertura) {
        const usuarioId = req.user.sub;
        const turnoExistente = await this.getTurnoActivo(usuarioId);
        if (turnoExistente)
            throw new AppError_1.AppError('Ya existe un turno abierto para este usuario', 400);
        const turno = await database_1.default.turno.create({
            data: {
                usuarioId,
                montoApertura,
                estado: 'ABIERTO'
            }
        });
        await (0, audit_service_1.auditAction)(req, 'APERTURA_TURNO');
        return turno;
    }
    static async crearPedido(req, data) {
        const usuarioId = req.user.sub;
        const turno = await this.getTurnoActivo(usuarioId);
        if (!turno)
            throw new AppError_1.AppError('No hay un turno abierto', 400);
        // Validar stock de todos los items
        for (const item of data.items) {
            const ok = await (0, inventory_service_1.validateStock)(item.productoId, item.cantidad);
            if (!ok)
                throw new AppError_1.AppError(`Stock insuficiente para ${item.nombre}`, 400);
        }
        // Calcular montos (Float según Sprint 3)
        let subtotal = 0;
        data.items.forEach((item) => {
            item.subtotal = item.cantidad * item.precioUnitario;
            subtotal += item.subtotal;
        });
        const iva = parseFloat((subtotal * this.IVA_RATE).toFixed(2));
        const total = parseFloat((subtotal + iva).toFixed(2));
        const pedido = await database_1.default.pedido.create({
            data: {
                turnoId: turno.id,
                cajero_id: usuarioId,
                cliente_nombre: data.cliente_nombre,
                cliente_cedula: data.cliente_cedula,
                subtotal,
                iva,
                total,
                estado: 'EN_EDICION',
                items: {
                    create: data.items.map((item) => ({
                        productoId: item.productoId,
                        nombre: item.nombre,
                        cantidad: item.cantidad,
                        precioUnitario: item.precioUnitario,
                        subtotal: item.subtotal
                    }))
                }
            },
            include: { items: true }
        });
        // Reservar stock
        for (const item of data.items) {
            await (0, inventory_service_1.updateStock)(item.productoId, item.cantidad, 'RESERVAR');
        }
        await (0, audit_service_1.auditAction)(req, 'CREAR_PEDIDO');
        return pedido;
    }
    static async actualizarPedido(req, pedidoId, data) {
        const pedido = await database_1.default.pedido.findUnique({
            where: { id: pedidoId },
            include: { items: true }
        });
        if (!pedido)
            throw new AppError_1.AppError('Pedido no encontrado', 404);
        if (pedido.estado !== 'EN_EDICION')
            throw new AppError_1.AppError('Solo se pueden editar pedidos en edición', 400);
        const updateData = {
            cliente_nombre: data.cliente_nombre,
            cliente_cedula: data.cliente_cedula
        };
        const updatedPedido = await database_1.default.pedido.update({
            where: { id: pedidoId },
            data: updateData,
            include: { items: true }
        });
        await (0, audit_service_1.auditAction)(req, 'ACTUALIZAR_PEDIDO');
        return updatedPedido;
    }
    static async anularPedido(req, pedidoId) {
        const pedido = await database_1.default.pedido.findUnique({
            where: { id: pedidoId },
            include: { items: true }
        });
        if (!pedido)
            throw new AppError_1.AppError('Pedido no encontrado', 404);
        if (pedido.estado === 'PAGADO')
            throw new AppError_1.AppError('No se puede anular un pedido pagado', 400);
        const updatedPedido = await database_1.default.pedido.update({
            where: { id: pedidoId },
            data: { estado: 'ANULADO' }
        });
        // Liberar stock
        for (const item of pedido.items) {
            await (0, inventory_service_1.updateStock)(item.productoId, item.cantidad, 'LIBERAR');
        }
        await (0, audit_service_1.auditAction)(req, 'ANULAR_PEDIDO');
        return updatedPedido;
    }
    static async pagarPedido(req, pedidoId, data) {
        const pedido = await database_1.default.pedido.findUnique({
            where: { id: pedidoId },
            include: { items: true }
        });
        if (!pedido)
            throw new AppError_1.AppError('Pedido no encontrado', 404);
        if (pedido.estado === 'PAGADO' || pedido.estado === 'ANULADO') {
            throw new AppError_1.AppError('Pedido ya finalizado o anulado', 400);
        }
        if (data.montoRecibido < pedido.total) {
            throw new AppError_1.AppError('Monto insuficiente', 400);
        }
        const updatedPedido = await database_1.default.pedido.update({
            where: { id: pedidoId },
            data: {
                estado: 'PAGADO',
                metodoPago: data.metodoPago,
                fecha_pago: new Date()
            }
        });
        // Descontar stock definitivamente
        for (const item of pedido.items) {
            await (0, inventory_service_1.updateStock)(item.productoId, item.cantidad, 'DESCONTAR');
        }
        await (0, audit_service_1.auditAction)(req, 'PAGAR_PEDIDO');
        return {
            pedido: updatedPedido,
            vuelto: parseFloat((data.montoRecibido - pedido.total).toFixed(2))
        };
    }
    static async cerrarCaja(req, montoCierreFisico, sessionToken) {
        const usuarioId = req.user.sub;
        const turno = await this.getTurnoActivo(usuarioId);
        if (!turno)
            throw new AppError_1.AppError('No hay turno abierto para cerrar', 400);
        // Calcular ventas en EFECTIVO (Sprint 3)
        const pedidosEfectivo = await database_1.default.pedido.findMany({
            where: {
                turnoId: turno.id,
                estado: 'PAGADO',
                metodoPago: 'EFECTIVO'
            }
        });
        const ventas_efectivo = pedidosEfectivo.reduce((acc, p) => acc + p.total, 0);
        // Calcular diferencia: monto_fisico - (monto_inicial + ventas_efectivo)
        const diferencia = montoCierreFisico - (turno.montoApertura + ventas_efectivo);
        // EstadoTurno: Si diferencia != 0 -> DESCUADRADO, sino CERRADO
        const estadoFinal = diferencia !== 0 ? 'DESCUADRADO' : 'CERRADO';
        // Para el registro, también calculamos el cierre teórico total del sistema
        const todosLosPedidos = await database_1.default.pedido.findMany({
            where: { turnoId: turno.id, estado: 'PAGADO' }
        });
        const montoVentasTotal = todosLosPedidos.reduce((acc, p) => acc + p.total, 0);
        const montoCierreSistema = turno.montoApertura + montoVentasTotal;
        const updatedTurno = await database_1.default.turno.update({
            where: { id: turno.id },
            data: {
                estado: estadoFinal,
                fechaCierre: new Date(),
                montoCierreFisico,
                montoCierreSistema,
                diferencia
            }
        });
        // Invalidación de sesión: Destruir session_token local y notificar a auth-service
        await database_1.default.activeSession.deleteMany({
            where: { usuarioId }
        });
        try {
            await axios_1.default.post(`${process.env.AUTH_SERVICE_URL}/internal/invalidate-sessions`, { usuario_id: usuarioId }, { headers: { 'X-Internal-Api-Key': process.env.INTERNAL_API_KEY } });
        }
        catch (error) {
            console.error('Error invalidando sesiones en auth-service:', error);
        }
        await (0, audit_service_1.auditAction)(req, 'CIERRE_CAJA');
        return updatedTurno;
    }
}
exports.PosService = PosService;
PosService.IVA_RATE = parseFloat(process.env.IVA_RATE || '0.19');
//# sourceMappingURL=pos.service.js.map