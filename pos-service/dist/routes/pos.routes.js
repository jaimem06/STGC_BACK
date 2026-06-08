"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
const express_1 = require("express");
const pos_controller_1 = require("../controllers/pos.controller");
const auth_middleware_1 = require("../middlewares/auth.middleware");
const router = (0, express_1.Router)();
// Todas las rutas del POS requieren autenticación
router.use(auth_middleware_1.authMiddleware);
// Gestión de Turno/Caja
router.post('/caja/apertura', pos_controller_1.PosController.abrirTurno);
router.post('/caja/cierre', pos_controller_1.PosController.cerrarCaja);
// Inventario
router.get('/productos', pos_controller_1.PosController.getProductos);
// Pedidos
router.post('/pedidos', pos_controller_1.PosController.crearPedido);
router.put('/pedidos/:id', pos_controller_1.PosController.actualizarPedido);
router.patch('/pedidos/:id/anular', pos_controller_1.PosController.anularPedido);
router.post('/pedidos/:id/pagar', pos_controller_1.PosController.pagarPedido);
router.get('/pedidos/:id/comprobante', pos_controller_1.PosController.getComprobante);
exports.default = router;
//# sourceMappingURL=pos.routes.js.map