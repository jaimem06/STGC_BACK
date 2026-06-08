import prisma from '../config/database';
import { AppError } from '../utils/AppError';
import { auditAction } from './audit.service';
import { updateStock, validateStock } from './inventory.service';
import axios from 'axios';
import { Request } from 'express';

export class PosService {
  private static IVA_RATE = parseFloat(process.env.IVA_RATE || '0.19');

  static async getTurnoActivo(usuarioId: string) {
    const turno = await prisma.turno.findFirst({
      where: { usuarioId, estado: 'ABIERTO' }
    });
    return turno;
  }

  static async abrirTurno(req: Request, montoApertura: number) {
    const usuarioId = req.user!.sub;
    const turnoExistente = await this.getTurnoActivo(usuarioId);
    if (turnoExistente) throw new AppError('Ya existe un turno abierto para este usuario', 400);

    const turno = await prisma.turno.create({
      data: {
        usuarioId,
        montoApertura,
        estado: 'ABIERTO'
      }
    });

    await auditAction(req, 'APERTURA_TURNO');
    return turno;
  }

  static async crearPedido(req: Request, data: any) {
    const usuarioId = req.user!.sub;
    const turno = await this.getTurnoActivo(usuarioId);
    if (!turno) throw new AppError('No hay un turno abierto', 400);

    // Validar stock de todos los items
    for (const item of data.items) {
      const ok = await validateStock(item.productoId, item.cantidad);
      if (!ok) throw new AppError(`Stock insuficiente para ${item.nombre}`, 400);
    }

    // Calcular montos (Float según Sprint 3)
    let subtotal = 0;
    data.items.forEach((item: any) => {
      item.subtotal = item.cantidad * item.precioUnitario;
      subtotal += item.subtotal;
    });

    const iva = parseFloat((subtotal * this.IVA_RATE).toFixed(2));
    const total = parseFloat((subtotal + iva).toFixed(2));

    const pedido = await prisma.pedido.create({
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
          create: data.items.map((item: any) => ({
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
      await updateStock(item.productoId, item.cantidad, 'RESERVAR');
    }

    await auditAction(req, 'CREAR_PEDIDO');
    return pedido;
  }

  static async actualizarPedido(req: Request, pedidoId: string, data: any) {
    const pedido = await prisma.pedido.findUnique({
      where: { id: pedidoId },
      include: { items: true }
    });

    if (!pedido) throw new AppError('Pedido no encontrado', 404);
    if (pedido.estado !== 'EN_EDICION') throw new AppError('Solo se pueden editar pedidos en edición', 400);

    const updateData: any = { 
      cliente_nombre: data.cliente_nombre,
      cliente_cedula: data.cliente_cedula
    };
    
    const updatedPedido = await prisma.pedido.update({
      where: { id: pedidoId },
      data: updateData,
      include: { items: true }
    });

    await auditAction(req, 'ACTUALIZAR_PEDIDO');
    return updatedPedido;
  }

  static async anularPedido(req: Request, pedidoId: string) {
    const pedido = await prisma.pedido.findUnique({
      where: { id: pedidoId },
      include: { items: true }
    });

    if (!pedido) throw new AppError('Pedido no encontrado', 404);
    if (pedido.estado === 'PAGADO') throw new AppError('No se puede anular un pedido pagado', 400);

    const updatedPedido = await prisma.pedido.update({
      where: { id: pedidoId },
      data: { estado: 'ANULADO' }
    });

    // Liberar stock
    for (const item of pedido.items) {
      await updateStock(item.productoId, item.cantidad, 'LIBERAR');
    }

    await auditAction(req, 'ANULAR_PEDIDO');
    return updatedPedido;
  }

  static async pagarPedido(req: Request, pedidoId: string, data: any) {
    const pedido = await prisma.pedido.findUnique({
      where: { id: pedidoId },
      include: { items: true }
    });

    if (!pedido) throw new AppError('Pedido no encontrado', 404);
    if (pedido.estado === 'PAGADO' || pedido.estado === 'ANULADO') {
      throw new AppError('Pedido ya finalizado o anulado', 400);
    }

    if (data.montoRecibido < pedido.total) {
      throw new AppError('Monto insuficiente', 400);
    }

    const updatedPedido = await prisma.pedido.update({
      where: { id: pedidoId },
      data: {
        estado: 'PAGADO',
        metodoPago: data.metodoPago,
        fecha_pago: new Date()
      }
    });

    // Descontar stock definitivamente
    for (const item of pedido.items) {
      await updateStock(item.productoId, item.cantidad, 'DESCONTAR');
    }

    await auditAction(req, 'PAGAR_PEDIDO');
    return {
      pedido: updatedPedido,
      vuelto: parseFloat((data.montoRecibido - pedido.total).toFixed(2))
    };
  }

  static async cerrarCaja(req: Request, montoCierreFisico: number, sessionToken: string) {
    const usuarioId = req.user!.sub;
    const turno = await this.getTurnoActivo(usuarioId);
    if (!turno) throw new AppError('No hay turno abierto para cerrar', 400);

    // Calcular ventas en EFECTIVO (Sprint 3)
    const pedidosEfectivo = await prisma.pedido.findMany({
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
    const todosLosPedidos = await prisma.pedido.findMany({
      where: { turnoId: turno.id, estado: 'PAGADO' }
    });
    const montoVentasTotal = todosLosPedidos.reduce((acc, p) => acc + p.total, 0);
    const montoCierreSistema = turno.montoApertura + montoVentasTotal;

    const updatedTurno = await prisma.turno.update({
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
    await prisma.activeSession.deleteMany({
      where: { usuarioId }
    });

    try {
      await axios.post(
        `${process.env.AUTH_SERVICE_URL}/internal/invalidate-sessions`,
        { usuario_id: usuarioId },
        { headers: { 'X-Internal-Api-Key': process.env.INTERNAL_API_KEY } }
      );
    } catch (error) {
      console.error('Error invalidando sesiones en auth-service:', error);
    }

    await auditAction(req, 'CIERRE_CAJA');

    return updatedTurno;
  }
}
