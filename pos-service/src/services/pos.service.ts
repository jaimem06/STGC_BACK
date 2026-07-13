import prisma from '../config/database';
import { AppError } from '../utils/AppError';
import { auditAction } from './audit.service';
import { updateStock, validateStock } from './inventory.service';
import axios from 'axios';
import { Request } from 'express';

export class PosService {
  private static IVA_RATE = parseFloat(process.env.IVA_RATE || '0.15');

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

    const token = req.headers.authorization;

    for (const item of data.items) {
      const ok = await validateStock(item.productoId, item.cantidad, token);
      if (!ok) throw new AppError('Producto no disponible', 400); 
    }

    // Calcular montos
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
      await updateStock(item.productoId, item.cantidad, 'RESERVAR', token);
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
    
    if (pedido.estado !== 'EN_EDICION') {
      throw new AppError('El pedido no puede ser modificado porque ya se encuentra en estado Pagado/Cerrado/Facturado/Anulado', 400);
    }

    const updateData: any = { 
      cliente_nombre: data.cliente_nombre !== undefined ? data.cliente_nombre : pedido.cliente_nombre,
      cliente_cedula: data.cliente_cedula !== undefined ? data.cliente_cedula : pedido.cliente_cedula
    };
    
    const token = req.headers.authorization;

    if (data.items && data.items.length > 0) {
      for (const item of pedido.items) {
        await updateStock(item.productoId, item.cantidad, 'LIBERAR', token);
      }

      let subtotal = 0;
      for (const item of data.items) {
        const ok = await validateStock(item.productoId, item.cantidad, token);
        if (!ok) throw new AppError('Producto no disponible', 400);
        item.subtotal = item.cantidad * item.precioUnitario;
        subtotal += item.subtotal;
      }

      const iva = parseFloat((subtotal * this.IVA_RATE).toFixed(2));
      const total = parseFloat((subtotal + iva).toFixed(2));

      updateData.subtotal = subtotal;
      updateData.iva = iva;
      updateData.total = total;

      updateData.items = {
        deleteMany: {}, 
        create: data.items.map((item: any) => ({
          productoId: item.productoId,
          nombre: item.nombre,
          cantidad: item.cantidad,
          precioUnitario: item.precioUnitario,
          subtotal: item.subtotal
        }))
      };

      for (const item of data.items) {
        await updateStock(item.productoId, item.cantidad, 'RESERVAR', token);
      }
    }

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

    // Liberar stock reservado
    const token = req.headers.authorization;
    for (const item of pedido.items) {
      await updateStock(item.productoId, item.cantidad, 'LIBERAR', token);
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
        referencia_pago: data.referencia_pago, // CUMPLIMIENTO HU009 CA5: Guardar referencia
        fecha_pago: new Date()
      }
    });

    const token = req.headers.authorization;
    for (const item of pedido.items) {
      await updateStock(item.productoId, item.cantidad, 'DESCONTAR', token);
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

    const ventas_efectivo = pedidosEfectivo.reduce((acc: number, p: any) => acc + p.total, 0);
    
    // Calcular diferencia: monto_fisico - (monto_inicial + ventas_efectivo)
    const diferencia = montoCierreFisico - (turno.montoApertura + ventas_efectivo);
    
    // EstadoTurno: Si diferencia != 0 -> DESCUADRADO, sino CERRADO
    const estadoFinal = diferencia !== 0 ? 'DESCUADRADO' : 'CERRADO';

    // Para el registro, también calculamos el cierre teórico total del sistema
    const todosLosPedidos = await prisma.pedido.findMany({
      where: { turnoId: turno.id, estado: 'PAGADO' }
    });
    const montoVentasTotal = todosLosPedidos.reduce((acc: number, p: any) => acc + p.total, 0);
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

    // El cierre de caja no debe cerrar la sesión del usuario (logout) en el sistema.
    // Simplemente cierra el turno del POS.

    await auditAction(req, 'CIERRE_CAJA');

    return updatedTurno;
  }
}