import axios from 'axios';

interface InventoryItemPayload {
  id: string;
  nombre: string;
  precio: number;
  cantidad: number;
  estado: string;
  tipo: string;
  is_deleted: boolean;
}

const getHeaders = (token?: string) => {
  return token ? { Authorization: token } : {};
};

export const getInventoryCafeteria = async (token?: string) => {
  try {
    const inventoryUrl = process.env.INVENTORY_SERVICE_URL || 'http://localhost:3000';
    const response = await axios.get<InventoryItemPayload[]>(`${inventoryUrl}/inventario/pos`, {
      headers: getHeaders(token)
    });

    const productosDisponibles = response.data.filter(item => 
      item.estado === 'DISPONIBLE' && 
      item.tipo === 'PRODUCTO' && 
      !item.is_deleted
    );

    return productosDisponibles.map(item => ({
      id: item.id,
      nombre: item.nombre,
      precio: item.precio,
      stock: item.cantidad
    }));
  } catch (error) {
    console.error('Error fetching inventory from microservice:', error);
    return [];
  }
};

export const validateStock = async (productoId: string, cantidad: number, token?: string) => {
  const inventory = await getInventoryCafeteria(token);
  const producto = inventory.find(p => p.id === productoId);
  return producto && producto.stock >= cantidad;
};

/**
 * Descuenta del inventario (movimiento SALIDA) los items de un pedido pagado.
 * Best-effort: el cobro ya está confirmado, así que un fallo aquí no lo revierte;
 * cada item que no se pudo descontar se devuelve como advertencia para que el
 * cajero lo vea y el stock se ajuste manualmente.
 */
export const descontarStockVenta = async (
  items: Array<{ productoId: string; nombre: string; cantidad: number }>,
  pedidoId: string,
  token?: string
): Promise<string[]> => {
  const inventoryUrl = process.env.INVENTORY_SERVICE_URL || 'http://localhost:3000';
  const advertencias: string[] = [];

  for (const item of items) {
    try {
      await axios.post(
        `${inventoryUrl}/inventario/pos/movimientos`,
        {
          item_id: item.productoId,
          cantidad: item.cantidad,
          tipo: 'SALIDA',
          motivo: `Venta POS - Pedido ${pedidoId}`
        },
        { headers: getHeaders(token), timeout: 5000 }
      );
    } catch (error: any) {
      const status = error?.response?.status;
      const detalle =
        status === 400 ? 'stock insuficiente'
        : status === 404 ? 'producto no encontrado en inventario'
        : 'servicio de inventario no disponible';
      advertencias.push(`No se descontó stock de "${item.nombre}" (${detalle}).`);
      console.error(
        `Fallo al descontar stock de ${item.productoId} (pedido ${pedidoId}):`,
        status ?? error?.message
      );
    }
  }

  return advertencias;
};
