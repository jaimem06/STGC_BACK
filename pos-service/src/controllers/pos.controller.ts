type Request = any;
type Response = any;
import { PosService } from '../services/pos.service';
import { getInventoryCafeteria } from '../services/inventory.service';
import { buscarClientes as buscarClientesService } from '../services/clientes.service';
import { generateTicket80mm } from '../services/pdf.service';
import { CreatePedidoSchema, UpdatePedidoSchema, PagoPedidoSchema, CierreCajaSchema } from '../dtos/pos.dto';
import { AppError } from '../utils/AppError';
import { ZodError } from 'zod';

export class PosController {
  static async getProductos(req: Request, res: Response) {
    try {
      const token = req.headers.authorization;
      const productos = await getInventoryCafeteria(token);
      return res.status(200).json(productos);
    } catch (error) {
      if (error instanceof AppError) return res.status(error.statusCode).json({ error: error.message });
      return res.status(500).json({ error: 'Error interno del servidor' });
    }
  }

  // NUEVA FUNCIÓN PARA TRAER LOS PEDIDOS POR ESTADO (HU005)
  static async getPedidos(req: Request, res: Response) {
    try {
      const estados = req.query.estados as string | undefined;
      const pedidos = await PosService.getPedidos(req, estados);
      return res.status(200).json(pedidos);
    } catch (error) {
      if (error instanceof AppError) return res.status(error.statusCode).json({ error: error.message });
      return res.status(500).json({ error: 'Error interno del servidor' });
    }
  }

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

  static async anularPedido(req: Request, res: Response) {
    try {
      const { id } = req.params;
      const pedido = await PosService.anularPedido(req, id);
      return res.status(200).json(pedido);
    } catch (error) {
      if (error instanceof AppError) return res.status(error.statusCode).json({ error: error.message });
      return res.status(500).json({ error: 'Error interno del servidor' });
    }
  }

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

  static async getComprobante(req: Request, res: Response) {
    try {
      const { id } = req.params;
      res.setHeader('Content-Type', 'application/pdf');
      res.setHeader('Content-Disposition', `attachment; filename=ticket-${id}.pdf`);
      await generateTicket80mm(id, res);
    } catch (error) {
      if (error instanceof AppError) return res.status(error.statusCode).json({ error: error.message });
      return res.status(500).json({ error: 'Error interno del servidor' });
    }
  }

  static async abrirTurno(req: Request, res: Response) {
    try {
      const { montoApertura } = req.body;
      if (typeof montoApertura !== 'number') throw new AppError('Monto de apertura inválido', 400);
      const turno = await PosService.abrirTurno(req, montoApertura);
      return res.status(201).json(turno);
    } catch (error) {
      if (error instanceof AppError) return res.status(error.statusCode).json({ error: error.message });
      return res.status(500).json({ error: 'Error interno del servidor' });
    }
  }

  // NUEVO: BUSCAR CLIENTES YA FACTURADOS PARA REUTILIZAR SUS DATOS
  static async buscarClientes(req: Request, res: Response) {
    try {
      const q = (req.query.q as string) || '';
      const clientes = await buscarClientesService(q);
      return res.status(200).json(clientes);
    } catch (error) {
      console.error('Error buscando clientes:', error);
      return res.status(500).json({ error: 'Error interno del servidor' });
    }
  }

  // Resumen en vivo del turno abierto: apertura, cobrado por método de pago y
  // monto con el que debe cerrarse la caja.
  static async getResumenCaja(req: Request, res: Response) {
    try {
      const usuarioId = req.user!.sub;
      const resumen = await PosService.getResumenTurno(usuarioId);
      return res.status(200).json(resumen);
    } catch (error) {
      if (error instanceof AppError) return res.status(error.statusCode).json({ error: error.message });
      return res.status(500).json({ error: 'Error interno del servidor' });
    }
  }

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