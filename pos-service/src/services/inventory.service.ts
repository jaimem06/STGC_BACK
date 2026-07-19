import axios from 'axios';

interface InventoryItemPayload {
  id: string;
  sku: string;
  nombre: string;
  precio: number;
  cantidad: number;
  stock_minimo: number;
  estado: string;
  tipo: string;
  is_deleted: boolean;
}

const ESTADOS_EN_CATALOGO = ['DISPONIBLE', 'STOCK_BAJO', 'AGOTADO'];

const getHeaders = (token?: string) => {
  return token ? { Authorization: token } : {};
};

// El inventory-service (Render, plan gratuito) puede tardar 30-50s en
// "despertar" tras estar inactivo. Un timeout corto lo reporta como "no
// disponible" cuando en realidad solo está arrancando, y el descuento de
// stock nunca llega a intentarse de verdad. Esperamos lo suficiente y
// reintentamos los fallos de conexión antes de darnos por vencidos.
const INVENTORY_TIMEOUT_MS = 45_000;
const INVENTORY_MAX_INTENTOS = 3;
const esperar = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/**
 * POST al inventory-service con reintentos ante fallos de conexión/timeout
 * (cold start, red inestable). Las respuestas reales del servicio (400 stock
 * insuficiente, 404 producto inexistente) NO se reintentan: ya son la
 * respuesta definitiva y reintentar solo retrasaría el resultado correcto.
 */
async function postInventarioConReintentos(url: string, payload: object, token?: string) {
  let ultimoError: any;
  for (let intento = 1; intento <= INVENTORY_MAX_INTENTOS; intento++) {
    try {
      return await axios.post(url, payload, {
        headers: getHeaders(token),
        timeout: INVENTORY_TIMEOUT_MS,
      });
    } catch (error: any) {
      ultimoError = error;
      if (error?.response?.status !== undefined) throw error;
      if (intento < INVENTORY_MAX_INTENTOS) {
        console.warn(
          `Intento ${intento}/${INVENTORY_MAX_INTENTOS} sin respuesta del inventory-service, reintentando...`,
          error?.message
        );
        await esperar(2000 * intento);
      }
    }
  }
  throw ultimoError;
}

export const getInventoryCafeteria = async (token?: string) => {
  try {
    const inventoryUrl = process.env.INVENTORY_SERVICE_URL || 'http://localhost:3000';
    const response = await axios.get<InventoryItemPayload[]>(`${inventoryUrl}/inventario/pos`, {
      headers: getHeaders(token)
    });

    const productosDelCatalogo = response.data.filter(item =>
      ESTADOS_EN_CATALOGO.includes(item.estado) &&
      item.tipo === 'PRODUCTO' &&
      !item.is_deleted
    );

    return productosDelCatalogo.map(item => ({
      id: item.id,
      sku: item.sku,
      nombre: item.nombre,
      precio: item.precio,
      stock: item.cantidad,
      stockMinimo: item.stock_minimo,
      estado: item.estado
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
      await postInventarioConReintentos(
        `${inventoryUrl}/inventario/pos/movimientos`,
        {
          item_id: item.productoId,
          cantidad: item.cantidad,
          tipo: 'SALIDA',
          motivo: `Venta POS - Pedido ${pedidoId}`
        },
        token
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
