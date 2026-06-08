"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.updateStock = exports.validateStock = exports.getInventoryCafeteria = void 0;
// Simulación de consulta a inventario central
const getInventoryCafeteria = async () => {
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
    }
    catch (error) {
        console.error('Error fetching inventory:', error);
        return [];
    }
};
exports.getInventoryCafeteria = getInventoryCafeteria;
const validateStock = async (productoId, cantidad) => {
    // Simulación de validación de stock
    const inventory = await (0, exports.getInventoryCafeteria)();
    const producto = inventory.find(p => p.id === productoId);
    return producto && producto.stock >= cantidad;
};
exports.validateStock = validateStock;
const updateStock = async (productoId, cantidad, operacion) => {
    // Simulación de actualización de stock en el servicio de inventario
    console.log(`Stock actualizado para ${productoId}: ${operacion} ${cantidad}`);
    return true;
};
exports.updateStock = updateStock;
//# sourceMappingURL=inventory.service.js.map