import { z } from 'zod';

export const CreatePedidoItemSchema = z.object({
  productoId: z.string(),
  nombre: z.string(),
  cantidad: z.number().int().positive(),
  precioUnitario: z.number().positive(),
});

export const CreatePedidoSchema = z.object({
  cliente_nombre: z.string().optional(),
  cliente_apellido: z.string().optional(), // Añadido
  cliente_cedula: z.string().optional(),
  items: z.array(CreatePedidoItemSchema).min(1),
});

export const UpdatePedidoSchema = z.object({
  cliente_nombre: z.string().optional(),
  cliente_apellido: z.string().optional(), // Añadido
  cliente_cedula: z.string().optional(),
  items: z.array(CreatePedidoItemSchema).optional(),
});

export const PagoItemSchema = z.object({
  metodoPago: z.enum(['EFECTIVO', 'TARJETA_CREDITO', 'TARJETA_DEBITO', 'TRANSFERENCIA', 'DE_UNA', 'AHORITA']),
  monto: z.number().positive(),
  referencia_pago: z.string().optional()
});

export const PagoPedidoSchema = z.object({
  pagos: z.array(PagoItemSchema).min(1),
});

export const CierreCajaSchema = z.object({
  montoCierreFisico: z.number().nonnegative(),
});