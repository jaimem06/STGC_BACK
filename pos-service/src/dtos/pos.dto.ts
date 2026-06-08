import { z } from 'zod';

export const CreatePedidoItemSchema = z.object({
  productoId: z.string(),
  nombre: z.string(),
  cantidad: z.number().int().positive(),
  precioUnitario: z.number().int().positive(),
});

export const CreatePedidoSchema = z.object({
  cliente: z.string().optional(),
  items: z.array(CreatePedidoItemSchema).min(1),
});

export const UpdatePedidoSchema = z.object({
  cliente: z.string().optional(),
  items: z.array(CreatePedidoItemSchema).optional(),
});

export const PagoPedidoSchema = z.object({
  metodoPago: z.enum(['EFECTIVO', 'TARJETA', 'TRANSFERENCIA', 'OTRO']),
  montoRecibido: z.number().int().positive(),
});

export const CierreCajaSchema = z.object({
  montoCierreFisico: z.number().int().nonnegative(),
});
