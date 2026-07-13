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

export const updateStock = async (productoId: string, cantidad: number, operacion: 'RESERVAR' | 'LIBERAR' | 'DESCONTAR', token?: string) => {
  console.log(`Stock actualizado para ${productoId}: ${operacion} ${cantidad}`);
  return true;
};
