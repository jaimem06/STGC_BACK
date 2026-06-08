import { z } from 'zod';

export const CreatePedidoItemSchema = z.object({
  productoId: z.string(),
  nombre: z.string(),
  cantidad: z.number().int().positive(),
  precioUnitario: z.number().positive(),
});

export const CreatePedidoSchema = z.object({
  cliente_nombre: z.string().optional(),
  cliente_cedula: z.string().optional(),
  items: z.array(CreatePedidoItemSchema).min(1),
});

export const UpdatePedidoSchema = z.object({
  cliente_nombre: z.string().optional(),
  cliente_cedula: z.string().optional(),
  items: z.array(CreatePedidoItemSchema).optional(),
});

export const PagoPedidoSchema = z.object({
  metodoPago: z.enum(['EFECTIVO', 'TARJETA_CREDITO', 'TARJETA_DEBITO', 'TRANSFERENCIA']),
  montoRecibido: z.number().positive(),
});

export const CierreCajaSchema = z.object({
  montoCierreFisico: z.number().nonnegative(),
});
