import { Router } from 'express';
import { PosController } from '../controllers/pos.controller';
import { authMiddleware } from '../middlewares/auth.middleware';

const router = Router();

router.use(authMiddleware);

// Gestión de Turno/Caja
router.get('/caja/estado', PosController.getEstadoCaja);
router.get('/caja/resumen', PosController.getResumenCaja);
router.post('/caja/apertura', PosController.abrirTurno);
router.post('/caja/cierre', PosController.cerrarCaja);

// Inventario
router.get('/productos', PosController.getProductos);

// Clientes (reutilización de datos para facturación)
router.get('/clientes', PosController.buscarClientes);

// Pedidos
router.get('/pedidos', PosController.getPedidos); // NUEVA RUTA AGREGADA
router.post('/pedidos', PosController.crearPedido);
router.put('/pedidos/:id', PosController.actualizarPedido);
router.patch('/pedidos/:id/anular', PosController.anularPedido);
router.post('/pedidos/:id/pagar', PosController.pagarPedido);
router.get('/pedidos/:id/comprobante', PosController.getComprobante);

export default router;