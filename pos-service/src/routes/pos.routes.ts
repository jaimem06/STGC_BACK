import { Router } from 'express';
import { PosController } from '../controllers/pos.controller';
import { authMiddleware } from '../middlewares/auth.middleware';

const router = Router();

// Todas las rutas del POS requieren autenticación
router.use(authMiddleware);

// Gestión de Turno/Caja
router.get('/caja/estado', PosController.getEstadoCaja);
router.post('/caja/apertura', PosController.abrirTurno);
router.post('/caja/cierre', PosController.cerrarCaja);

// Inventario
router.get('/productos', PosController.getProductos);

// Pedidos
router.post('/pedidos', PosController.crearPedido);
router.put('/pedidos/:id', PosController.actualizarPedido);
router.patch('/pedidos/:id/anular', PosController.anularPedido);
router.post('/pedidos/:id/pagar', PosController.pagarPedido);
router.get('/pedidos/:id/comprobante', PosController.getComprobante);

export default router;
