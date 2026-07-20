import prisma from '../config/database';
import { AppError } from '../utils/AppError';
import { auditAction } from './audit.service';
import { descontarStockVenta, validateStock } from './inventory.service';
import { guardarClienteParaReutilizar } from './clientes.service';
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

  // NUEVO: OBTENER PEDIDOS CON FILTRO DE ESTADO (HU005)
  static async getPedidos(req: Request, estados?: string) {
    const usuarioId = req.user!.sub;
    const turno = await this.getTurnoActivo(usuarioId);
    if (!turno) throw new AppError('No hay un turno abierto', 400);

    // Se busca en TODOS los turnos de este cajero, no solo el actualmente
    // abierto: un pedido EN_EDICION que quedó sin cobrar al cerrar caja no
    // debe volverse invisible/huérfano en la siguiente apertura de turno.
    const whereClause: any = { turno: { usuarioId } };
    if (estados) {
      whereClause.estado = { in: estados.split(',').map(e => e.trim()) };
    }

    return await prisma.pedido.findMany({
      where: whereClause,
      include: { items: true, pagos: true },
      orderBy: { fechaCreacion: 'desc' }
    });
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
        cliente_apellido: data.cliente_apellido,
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
      include: { items: true, pagos: true }
    });

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
      cliente_apellido: data.cliente_apellido !== undefined ? data.cliente_apellido : pedido.cliente_apellido,
      cliente_cedula: data.cliente_cedula !== undefined ? data.cliente_cedula : pedido.cliente_cedula
    };
    
    const token = req.headers.authorization;

    if (data.items && data.items.length > 0) {
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
    }

    const updatedPedido = await prisma.pedido.update({
      where: { id: pedidoId },
      data: updateData,
      include: { items: true, pagos: true }
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

    await auditAction(req, 'ANULAR_PEDIDO');
    return updatedPedido;
  }

  // NUEVO: PROCESAR ARREGLO DE PAGOS MÚLTIPLES (HU009)
  static async pagarPedido(req: Request, pedidoId: string, data: any) {
    const pedido = await prisma.pedido.findUnique({
      where: { id: pedidoId },
      include: { items: true }
    });

    if (!pedido) throw new AppError('Pedido no encontrado', 404);
    if (pedido.estado === 'PAGADO' || pedido.estado === 'ANULADO') {
      throw new AppError('Pedido ya finalizado o anulado', 400);
    }
    
    // HU009 - CA3: Validar que la suma coincida con el total (tolerancia mínima
    // para absorber el ruido de coma flotante; no permite descuadres reales).
    const sumaPagos = parseFloat(data.pagos.reduce((acc: number, p: any) => acc + p.monto, 0).toFixed(2));
    const totalPedido = parseFloat(Number(pedido.total).toFixed(2));

    if (Math.abs(sumaPagos - totalPedido) > 0.001) {
      throw new AppError(`La suma de los pagos (${sumaPagos}) no coincide con el total del pedido (${totalPedido})`, 400);
    }

    const updatedPedido = await prisma.pedido.update({
      where: { id: pedidoId },
      data: {
        estado: 'PAGADO',
        fecha_pago: new Date(),
        pagos: {
          create: data.pagos.map((p: any) => ({
            metodoPago: p.metodoPago,
            monto: p.monto,
            referencia_pago: p.referencia_pago
          }))
        }
      },
      include: { items: true }
    });

    const token = req.headers.authorization;
    const advertencias = await descontarStockVenta(
      pedido.items.map(i => ({ productoId: i.productoId, nombre: i.nombre, cantidad: i.cantidad })),
      pedidoId,
      token
    );

    // Best-effort: guarda el cliente para que el cajero pueda reutilizarlo en
    // la próxima venta sin volver a digitar sus datos (no aplica a consumidor final).
    await guardarClienteParaReutilizar(
      updatedPedido.cliente_nombre,
      updatedPedido.cliente_apellido,
      updatedPedido.cliente_cedula
    );

    await auditAction(req, 'PAGAR_PEDIDO');

    // Como la suma debe ser exacta, el vuelto siempre será 0 (según reglas actuales)
    return {
      pedido: updatedPedido,
      vuelto: 0,
      advertencias
    };
  }

  // NUEVO: GENERAR RESUMEN Y DESGLOSE AL CERRAR CAJA (HU010)
  static async cerrarCaja(req: Request, montoCierreFisico: number, _sessionToken: string) {
    const usuarioId = req.user!.sub;
    const turno = await this.getTurnoActivo(usuarioId);
    if (!turno) throw new AppError('No hay turno abierto para cerrar', 400);

    const pedidosPagados = await prisma.pedido.findMany({
      where: { turnoId: turno.id, estado: 'PAGADO' },
    });

    let montoVentasTotal = 0;

    pedidosPagados.forEach(p => {
      montoVentasTotal += p.total;
    });

    const montoCierreSistema = turno.montoApertura + montoVentasTotal;
    // Redondeamos a centavos para una comparación estable (evita descuadres por floats).
    const diferencia = Math.round((montoCierreFisico - montoCierreSistema) * 100) / 100;
    // Estados de cierre válidos del enum EstadoTurno (schema.prisma). El cast
    // evita fallos de compilación si el cliente Prisma generado está desincronizado
    // con el schema; el valor sí es válido para la columna en la base de datos.
    const estadoFinal: 'CERRADO_CONCILIADO' | 'CERRADO_CON_DESCUADRE' =
      diferencia === 0 ? 'CERRADO_CONCILIADO' : 'CERRADO_CON_DESCUADRE';

    const updatedTurno = await prisma.turno.update({
      where: { id: turno.id },
      data: {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        estado: estadoFinal as any,
        fechaCierre: new Date(),
        montoCierreFisico,
        montoCierreSistema,
        diferencia
      }
    });

    await auditAction(req, 'CIERRE_CAJA');

    // Estructura exacta que pide el frontend / HU010
    return {
      turno: updatedTurno,
      resumen: {
        totalTransacciones: pedidosPagados.length
      }
    };
  }
}