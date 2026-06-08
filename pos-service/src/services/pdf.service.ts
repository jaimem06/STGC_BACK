import PDFDocument from 'pdfkit';
import { Response } from 'express';
import prisma from '../config/database';

export const generateTicket80mm = async (pedidoId: string, res: Response) => {
  const pedido = await prisma.pedido.findUnique({
    where: { id: pedidoId },
    include: { items: true }
  });

  if (!pedido) throw new Error('Pedido no encontrado');

  const doc = new PDFDocument({
    size: [226, 600], // Aproximadamente 80mm de ancho
    margins: { top: 10, bottom: 10, left: 10, right: 10 }
  });

  doc.pipe(res);

  // Cabecera
  doc.fontSize(12).text('STGC - POS CAFETERIA', { align: 'center' });
  doc.fontSize(8).text('----------------------------------', { align: 'center' });
  doc.text(`Fecha: ${pedido.fechaCreacion.toLocaleString()}`);
  doc.text(`Pedido ID: ${pedido.id.substring(0, 8)}`);
  if (pedido.cliente) doc.text(`Cliente: ${pedido.cliente}`);
  doc.text('----------------------------------');

  // Items
  pedido.items.forEach(item => {
    doc.text(`${item.cantidad} x ${item.nombre.substring(0, 20)}`);
    doc.text(`$${(item.precioUnitario / 100).toFixed(2)} -> $${(item.subtotal / 100).toFixed(2)}`, { align: 'right' });
  });

  doc.text('----------------------------------');
  
  // Totales
  doc.fontSize(10).text(`SUBTOTAL: $${(pedido.subtotal / 100).toFixed(2)}`, { align: 'right' });
  doc.text(`IVA: $${(pedido.iva / 100).toFixed(2)}`, { align: 'right' });
  doc.fontSize(12).text(`TOTAL: $${(pedido.total / 100).toFixed(2)}`, { align: 'right' });

  doc.text('----------------------------------', { align: 'center' });
  doc.fontSize(8).text('¡Gracias por su compra!', { align: 'center' });

  doc.end();
};
