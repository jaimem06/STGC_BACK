import { Request, Response, NextFunction } from 'express';
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

export class PosController {
  static async getProductos(req: Request, res: Response, next: NextFunction) {
    try {
      const productos = await getInventoryCafeteria();
      res.json(productos);
    } catch (error) {
      next(error);
    }
  }

  static async crearPedido(req: Request, res: Response, next: NextFunction) {
    try {
      const validated = CreatePedidoSchema.parse(req.body);
      const pedido = await PosService.crearPedido(req.user!.sub, validated);
      res.status(201).json(pedido);
    } catch (error) {
      next(error);
    }
  }

  static async actualizarPedido(req: Request, res: Response, next: NextFunction) {
    try {
      const { id } = req.params;
      const validated = UpdatePedidoSchema.parse(req.body);
      const pedido = await PosService.actualizarPedido(req.user!.sub, id, validated);
      res.json(pedido);
    } catch (error) {
      next(error);
    }
  }

  static async anularPedido(req: Request, res: Response, next: NextFunction) {
    try {
      const { id } = req.params;
      const pedido = await PosService.anularPedido(req.user!.sub, id);
      res.json(pedido);
    } catch (error) {
      next(error);
    }
  }

  static async pagarPedido(req: Request, res: Response, next: NextFunction) {
    try {
      const { id } = req.params;
      const validated = PagoPedidoSchema.parse(req.body);
      const result = await PosService.pagarPedido(req.user!.sub, id, validated);
      res.json(result);
    } catch (error) {
      next(error);
    }
  }

  static async cerrarCaja(req: Request, res: Response, next: NextFunction) {
    try {
      const validated = CierreCajaSchema.parse(req.body);
      const sessionToken = req.headers.authorization!.split(' ')[1];
      const turno = await PosService.cerrarCaja(req.user!.sub, validated.montoCierreFisico, sessionToken);
      res.json(turno);
    } catch (error) {
      next(error);
    }
  }

  static async getComprobante(req: Request, res: Response, next: NextFunction) {
    try {
      const { id } = req.params;
      res.setHeader('Content-Type', 'application/pdf');
      res.setHeader('Content-Disposition', `attachment; filename=ticket-${id}.pdf`);
      await generateTicket80mm(id, res);
    } catch (error) {
      next(error);
    }
  }

  static async abrirTurno(req: Request, res: Response, next: NextFunction) {
    try {
      const { montoApertura } = req.body;
      if (typeof montoApertura !== 'number') throw new AppError('Monto de apertura inválido');
      const turno = await PosService.abrirTurno(req.user!.sub, montoApertura);
      res.status(201).json(turno);
    } catch (error) {
      next(error);
    }
  }
}
