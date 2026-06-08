"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.generateTicket80mm = void 0;
const pdfkit_1 = __importDefault(require("pdfkit"));
const database_1 = __importDefault(require("../config/database"));
const generateTicket80mm = async (pedidoId, res) => {
    const pedido = await database_1.default.pedido.findUnique({
        where: { id: pedidoId },
        include: { items: true }
    });
    if (!pedido)
        throw new Error('Pedido no encontrado');
    const doc = new pdfkit_1.default({
        size: [226, 600], // Aproximadamente 80mm de ancho
        margins: { top: 10, bottom: 10, left: 10, right: 10 }
    });
    doc.pipe(res);
    // Cabecera
    doc.fontSize(12).text('STGC - POS CAFETERIA', { align: 'center' });
    doc.fontSize(8).text('----------------------------------', { align: 'center' });
    doc.text(`Fecha: ${pedido.fechaCreacion.toLocaleString()}`);
    doc.text(`Pedido ID: ${pedido.id.substring(0, 8)}`);
    if (pedido.cliente_nombre)
        doc.text(`Cliente: ${pedido.cliente_nombre}`);
    if (pedido.cliente_cedula)
        doc.text(`Cédula: ${pedido.cliente_cedula}`);
    doc.text('----------------------------------');
    // Items
    pedido.items.forEach(item => {
        doc.text(`${item.cantidad} x ${item.nombre.substring(0, 20)}`);
        doc.text(`$${item.precioUnitario.toFixed(2)} -> $${item.subtotal.toFixed(2)}`, { align: 'right' });
    });
    doc.text('----------------------------------');
    // Totales
    doc.fontSize(10).text(`SUBTOTAL: $${pedido.subtotal.toFixed(2)}`, { align: 'right' });
    doc.text(`IVA: $${pedido.iva.toFixed(2)}`, { align: 'right' });
    doc.fontSize(12).text(`TOTAL: $${pedido.total.toFixed(2)}`, { align: 'right' });
    doc.text('----------------------------------', { align: 'center' });
    doc.fontSize(8).text('¡Gracias por su compra!', { align: 'center' });
    doc.end();
};
exports.generateTicket80mm = generateTicket80mm;
//# sourceMappingURL=pdf.service.js.map