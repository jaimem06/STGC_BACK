"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.CierreCajaSchema = exports.PagoPedidoSchema = exports.UpdatePedidoSchema = exports.CreatePedidoSchema = exports.CreatePedidoItemSchema = void 0;
const zod_1 = require("zod");
exports.CreatePedidoItemSchema = zod_1.z.object({
    productoId: zod_1.z.string(),
    nombre: zod_1.z.string(),
    cantidad: zod_1.z.number().int().positive(),
    precioUnitario: zod_1.z.number().positive(),
});
exports.CreatePedidoSchema = zod_1.z.object({
    cliente_nombre: zod_1.z.string().optional(),
    cliente_cedula: zod_1.z.string().optional(),
    items: zod_1.z.array(exports.CreatePedidoItemSchema).min(1),
});
exports.UpdatePedidoSchema = zod_1.z.object({
    cliente_nombre: zod_1.z.string().optional(),
    cliente_cedula: zod_1.z.string().optional(),
    items: zod_1.z.array(exports.CreatePedidoItemSchema).optional(),
});
exports.PagoPedidoSchema = zod_1.z.object({
    metodoPago: zod_1.z.enum(['EFECTIVO', 'TARJETA_CREDITO', 'TARJETA_DEBITO', 'TRANSFERENCIA']),
    montoRecibido: zod_1.z.number().positive(),
});
exports.CierreCajaSchema = zod_1.z.object({
    montoCierreFisico: zod_1.z.number().nonnegative(),
});
//# sourceMappingURL=pos.dto.js.map