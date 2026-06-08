import axios from 'axios';

// Simulación de consulta a inventario central
export const getInventoryCafeteria = async () => {
  try {
    // En un escenario real, esto consultaría al microservicio de inventario
    // const response = await axios.get(`${process.env.INVENTORY_SERVICE_URL}/productos?categoria=CAFETERIA`);
    // return response.data;
    
    // Mock para desarrollo
    return [
      { id: 'prod-1', nombre: 'Café Espresso', precio: 1500, stock: 50 },
      { id: 'prod-2', nombre: 'Media Luna', precio: 800, stock: 20 },
      { id: 'prod-3', nombre: 'Jugo Natural', precio: 2000, stock: 15 },
    ];
  } catch (error) {
    console.error('Error fetching inventory:', error);
    return [];
  }
};

export const validateStock = async (productoId: string, cantidad: number) => {
  // Simulación de validación de stock
  const inventory = await getInventoryCafeteria();
  const producto = inventory.find(p => p.id === productoId);
  return producto && producto.stock >= cantidad;
};

export const updateStock = async (productoId: string, cantidad: number, operacion: 'RESERVAR' | 'LIBERAR' | 'DESCONTAR') => {
  // Simulación de actualización de stock en el servicio de inventario
  console.log(`Stock actualizado para ${productoId}: ${operacion} ${cantidad}`);
  return true;
};
