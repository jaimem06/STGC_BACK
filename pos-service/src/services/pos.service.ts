import prisma from '../config/database';
import { AppError } from '../utils/AppError';
import { auditAction } from './audit.service';
import { updateStock, validateStock } from './inventory.service';
import axios from 'axios';

export class PosService {
  private static IVA_RATE = parseFloat(process.env.IVA_RATE || '0.19');

  static async getTurnoActivo(usuarioId: string) {
    const turno = await prisma.turno.findFirst({
      where: { usuarioId, estado: 'ABIERTO' }
    });
    return turno;
  }

  static async abrirTurno(usuarioId: string, montoApertura: number) {
    const turnoExistente = await this.getTurnoActivo(usuarioId);
    if (turnoExistente) throw new AppError('Ya existe un turno abierto para este usuario', 400);

    const turno = await prisma.turno.create({
      data: {
        usuarioId,
        montoApertura,
        estado: 'ABIERTO'
      }
    });

    await auditAction(usuarioId, 'APERTURA_TURNO', { turnoId: turno.id });
    return turno;
  }

  static async crearPedido(usuarioId: string, data: any) {
    const turno = await this.getTurnoActivo(usuarioId);
    if (!turno) throw new AppError('No hay un turno abierto', 400);

    // Validar stock de todos los items
    for (const item of data.items) {
      const ok = await validateStock(item.productoId, item.cantidad);
      if (!ok) throw new AppError(`Stock insuficiente para ${item.nombre}`, 400);
    }

    // Calcular montos (evitando coma flotante)
    let subtotal = 0;
    data.items.forEach((item: any) => {
      item.subtotal = item.cantidad * item.precioUnitario;
      subtotal += item.subtotal;
    });

    const iva = Math.round(subtotal * this.IVA_RATE);
    const total = subtotal + iva;

    const pedido = await prisma.pedido.create({
      data: {
        turnoId: turno.id,
        cliente: data.cliente,
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

    await auditAction(usuarioId, 'CREAR_PEDIDO', { pedidoId: pedido.id });
    return pedido;
  }

  static async actualizarPedido(usuarioId: string, pedidoId: string, data: any) {
    const pedido = await prisma.pedido.findUnique({
      where: { id: pedidoId },
      include: { items: true }
    });

    if (!pedido) throw new AppError('Pedido no encontrado', 404);
    if (pedido.estado !== 'EN_EDICION') throw new AppError('Solo se pueden editar pedidos en edición', 400);

    // Lógica compleja de actualización de items y stock omitida por brevedad, 
    // pero básicamente es liberar anterior y reservar nuevo.
    
    // Simplificado: Actualizar cliente y recalcular si hay items nuevos
    const updateData: any = { cliente: data.cliente };
    
    const updatedPedido = await prisma.pedido.update({
      where: { id: pedidoId },
      data: updateData,
      include: { items: true }
    });

    await auditAction(usuarioId, 'ACTUALIZAR_PEDIDO', { pedidoId });
    return updatedPedido;
  }

  static async anularPedido(usuarioId: string, pedidoId: string) {
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

    await auditAction(usuarioId, 'ANULAR_PEDIDO', { pedidoId });
    return updatedPedido;
  }

  static async pagarPedido(usuarioId: string, pedidoId: string, data: any) {
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
        metodoPago: data.metodoPago
      }
    });

    // Descontar stock definitivamente
    for (const item of pedido.items) {
      await updateStock(item.productoId, item.cantidad, 'DESCONTAR');
    }

    await auditAction(usuarioId, 'PAGAR_PEDIDO', { pedidoId, metodoPago: data.metodoPago });
    return {
      pedido: updatedPedido,
      vuelto: data.montoRecibido - pedido.total
    };
  }

  static async cerrarCaja(usuarioId: string, montoCierreFisico: number, sessionToken: string) {
    const turno = await this.getTurnoActivo(usuarioId);
    if (!turno) throw new AppError('No hay turno abierto para cerrar', 400);

    // Calcular ventas del sistema
    const pedidos = await prisma.pedido.findMany({
      where: { turnoId: turno.id, estado: 'PAGADO' }
    });

    const montoVentas = pedidos.reduce((acc, p) => acc + p.total, 0);
    const montoCierreSistema = turno.montoApertura + montoVentas;

    const updatedTurno = await prisma.turno.update({
      where: { id: turno.id },
      data: {
        estado: 'CERRADO',
        fechaCierre: new Date(),
        montoCierreFisico,
        montoCierreSistema
      }
    });

    // REQUISITO CRÍTICO: Invalidar sesiones concurrentes
    // 1. Eliminar de nuestra DB local
    await prisma.activeSession.deleteMany({
      where: { usuarioId }
    });

    // 2. Notificar a auth-service para invalidación global
    try {
      await axios.post(
        `${process.env.AUTH_SERVICE_URL}/internal/invalidate-sessions`,
        { usuario_id: usuarioId },
        { headers: { 'X-Internal-Api-Key': process.env.INTERNAL_API_KEY } }
      );
    } catch (error) {
      console.error('Error invalidando sesiones en auth-service:', error);
    }

    await auditAction(usuarioId, 'CIERRE_CAJA', { 
      turnoId: turno.id, 
      diferencia: montoCierreFisico - montoCierreSistema 
    });

    return updatedTurno;
  }
}
