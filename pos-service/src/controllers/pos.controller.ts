type Request = any;
type Response = any;
import { PosService } from '../services/pos.service';
import { getInventoryCafeteria } from '../services/inventory.service';
import { generateTicket80mm } from '../services/pdf.service';
import { 
  CreatePedidoSchema, 
  UpdatePedidoSchema, 
  PagoPedidoSchema, 
  CierreCajaSchema 
} from '../dtos/pos.dto';
import { AppError } from '../utils/AppError';
import { ZodError } from 'zod';

export class PosController {
  /**
   * Obtiene los productos del inventario de la cafetería
   * Posibles Respuestas:
   * 200 - Éxito
   * 400 - Petición inválida
   * 401 - No autorizado (Middleware)
   * 403 - Rol insuficiente (Middleware)
   * 500 - Error interno
   */
  static async getProductos(req: Request, res: Response) {
    try {
      const token = req.headers.authorization;
      const productos = await getInventoryCafeteria(token);
      return res.status(200).json(productos);
    } catch (error) {
      if (error instanceof AppError) return res.status(error.statusCode).json({ error: error.message });
      if (error instanceof ZodError) return res.status(400).json({ error: 'Error de validación', details: error.issues });
      return res.status(500).json({ error: 'Error interno del servidor' });
    }
  }

  /**
   * Crea un nuevo pedido
   * Posibles Respuestas:
   * 201 - Pedido Creado
   * 400 - Validación fallida o stock insuficiente
   * 401 - No autorizado (Middleware)
   * 403 - Rol insuficiente (Middleware)
   * 500 - Error interno
   */
  static async crearPedido(req: Request, res: Response) {
    try {
      const validated = CreatePedidoSchema.parse(req.body);
      const pedido = await PosService.crearPedido(req, validated);
      return res.status(201).json(pedido);
    } catch (error) {
      if (error instanceof AppError) return res.status(error.statusCode).json({ error: error.message });
      if (error instanceof ZodError) return res.status(400).json({ error: 'Error de validación', details: error.issues });
      return res.status(500).json({ error: 'Error interno del servidor' });
    }
  }

  /**
   * Actualiza un pedido en edición
   * Posibles Respuestas:
   * 200 - Pedido Actualizado
   * 400 - Validación fallida o pedido no en edición
   * 401 - No autorizado (Middleware)
   * 404 - Pedido no encontrado
   * 500 - Error interno
   */
  static async actualizarPedido(req: Request, res: Response) {
    try {
      const { id } = req.params;
      const validated = UpdatePedidoSchema.parse(req.body);
      const pedido = await PosService.actualizarPedido(req, id, validated);
      return res.status(200).json(pedido);
    } catch (error) {
      if (error instanceof AppError) return res.status(error.statusCode).json({ error: error.message });
      if (error instanceof ZodError) return res.status(400).json({ error: 'Error de validación', details: error.issues });
      return res.status(500).json({ error: 'Error interno del servidor' });
    }
  }

  /**
   * Anula un pedido
   * Posibles Respuestas:
   * 200 - Pedido Anulado
   * 400 - Pedido ya pagado
   * 401 - No autorizado (Middleware)
   * 404 - Pedido no encontrado
   * 500 - Error interno
   */
  static async anularPedido(req: Request, res: Response) {
    try {
      const { id } = req.params;
      const pedido = await PosService.anularPedido(req, id);
      return res.status(200).json(pedido);
    } catch (error) {
      if (error instanceof AppError) return res.status(error.statusCode).json({ error: error.message });
      if (error instanceof ZodError) return res.status(400).json({ error: 'Error de validación', details: error.issues });
      return res.status(500).json({ error: 'Error interno del servidor' });
    }
  }

  /**
   * Paga un pedido
   * Posibles Respuestas:
   * 200 - Pedido Pagado
   * 400 - Monto insuficiente o estado inválido
   * 401 - No autorizado (Middleware)
   * 404 - Pedido no encontrado
   * 500 - Error interno
   */
  static async pagarPedido(req: Request, res: Response) {
    try {
      const { id } = req.params;
      const validated = PagoPedidoSchema.parse(req.body);
      const result = await PosService.pagarPedido(req, id, validated);
      return res.status(200).json(result);
    } catch (error) {
      if (error instanceof AppError) return res.status(error.statusCode).json({ error: error.message });
      if (error instanceof ZodError) return res.status(400).json({ error: 'Error de validación', details: error.issues });
      return res.status(500).json({ error: 'Error interno del servidor' });
    }
  }

  /**
   * Cierra el turno de caja
   * Posibles Respuestas:
   * 200 - Caja Cerrada
   * 400 - Error de validación o sin turno abierto
   * 401 - No autorizado (Middleware)
   * 403 - Rol insuficiente (Middleware)
   * 500 - Error interno
   */
  static async cerrarCaja(req: Request, res: Response) {
    try {
      const validated = CierreCajaSchema.parse(req.body);
      const sessionToken = req.headers.authorization!.split(' ')[1];
      const turno = await PosService.cerrarCaja(req, validated.montoCierreFisico, sessionToken);
      return res.status(200).json(turno);
    } catch (error) {
      if (error instanceof AppError) return res.status(error.statusCode).json({ error: error.message });
      if (error instanceof ZodError) return res.status(400).json({ error: 'Error de validación', details: error.issues });
      return res.status(500).json({ error: 'Error interno del servidor' });
    }
  }

  /**
   * Obtiene comprobante de pago
   * Posibles Respuestas:
   * 200 - Comprobante PDF
   * 400 - Petición inválida
   * 401 - No autorizado (Middleware)
   * 404 - Pedido no encontrado
   * 500 - Error interno
   */
  static async getComprobante(req: Request, res: Response) {
    try {
      const { id } = req.params;
      res.setHeader('Content-Type', 'application/pdf');
      res.setHeader('Content-Disposition', `attachment; filename=ticket-${id}.pdf`);
      await generateTicket80mm(id, res);
    } catch (error) {
      if (error instanceof AppError) return res.status(error.statusCode).json({ error: error.message });
      if (error instanceof ZodError) return res.status(400).json({ error: 'Error de validación', details: error.issues });
      return res.status(500).json({ error: 'Error interno del servidor' });
    }
  }

  /**
   * Abre un turno de caja
   * Posibles Respuestas:
   * 201 - Turno Abierto
   * 400 - Monto inválido o turno ya abierto
   * 401 - No autorizado (Middleware)
   * 403 - Rol insuficiente (Middleware)
   * 500 - Error interno
   */
  static async abrirTurno(req: Request, res: Response) {
    try {
      const { montoApertura } = req.body;
      if (typeof montoApertura !== 'number') throw new AppError('Monto de apertura inválido', 400);
      const turno = await PosService.abrirTurno(req, montoApertura);
      return res.status(201).json(turno);
    } catch (error) {
      if (error instanceof AppError) return res.status(error.statusCode).json({ error: error.message });
      if (error instanceof ZodError) return res.status(400).json({ error: 'Error de validación', details: error.issues });
      return res.status(500).json({ error: 'Error interno del servidor' });
    }
  }

  /**
   * Obtiene el estado actual de la caja
   * Posibles Respuestas:
   * 200 - Estado devuelto
   * 401 - No autorizado
   * 500 - Error interno
   */
  static async getEstadoCaja(req: Request, res: Response) {
    try {
      const usuarioId = req.user!.sub;
      const turno = await PosService.getTurnoActivo(usuarioId);
      return res.status(200).json({ isRegisterOpen: !!turno, turno });
    } catch (error) {
      if (error instanceof AppError) return res.status(error.statusCode).json({ error: error.message });
      return res.status(500).json({ error: 'Error interno del servidor' });
    }
  }
}
