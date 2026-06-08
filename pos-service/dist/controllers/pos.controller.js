"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.PosController = void 0;
const pos_service_1 = require("../services/pos.service");
const inventory_service_1 = require("../services/inventory.service");
const pdf_service_1 = require("../services/pdf.service");
const pos_dto_1 = require("../dtos/pos.dto");
const AppError_1 = require("../utils/AppError");
class PosController {
    static async getProductos(req, res, next) {
        try {
            const productos = await (0, inventory_service_1.getInventoryCafeteria)();
            res.json(productos);
        }
        catch (error) {
            next(error);
        }
    }
    static async crearPedido(req, res, next) {
        try {
            const validated = pos_dto_1.CreatePedidoSchema.parse(req.body);
            const pedido = await pos_service_1.PosService.crearPedido(req, validated);
            res.status(201).json(pedido);
        }
        catch (error) {
            next(error);
        }
    }
    static async actualizarPedido(req, res, next) {
        try {
            const { id } = req.params;
            const validated = pos_dto_1.UpdatePedidoSchema.parse(req.body);
            const pedido = await pos_service_1.PosService.actualizarPedido(req, id, validated);
            res.json(pedido);
        }
        catch (error) {
            next(error);
        }
    }
    static async anularPedido(req, res, next) {
        try {
            const { id } = req.params;
            const pedido = await pos_service_1.PosService.anularPedido(req, id);
            res.json(pedido);
        }
        catch (error) {
            next(error);
        }
    }
    static async pagarPedido(req, res, next) {
        try {
            const { id } = req.params;
            const validated = pos_dto_1.PagoPedidoSchema.parse(req.body);
            const result = await pos_service_1.PosService.pagarPedido(req, id, validated);
            res.json(result);
        }
        catch (error) {
            next(error);
        }
    }
    static async cerrarCaja(req, res, next) {
        try {
            const validated = pos_dto_1.CierreCajaSchema.parse(req.body);
            const sessionToken = req.headers.authorization.split(' ')[1];
            const turno = await pos_service_1.PosService.cerrarCaja(req, validated.montoCierreFisico, sessionToken);
            res.json(turno);
        }
        catch (error) {
            next(error);
        }
    }
    static async getComprobante(req, res, next) {
        try {
            const { id } = req.params;
            res.setHeader('Content-Type', 'application/pdf');
            res.setHeader('Content-Disposition', `attachment; filename=ticket-${id}.pdf`);
            await (0, pdf_service_1.generateTicket80mm)(id, res);
        }
        catch (error) {
            next(error);
        }
    }
    static async abrirTurno(req, res, next) {
        try {
            const { montoApertura } = req.body;
            if (typeof montoApertura !== 'number')
                throw new AppError_1.AppError('Monto de apertura inválido');
            const turno = await pos_service_1.PosService.abrirTurno(req, montoApertura);
            res.status(201).json(turno);
        }
        catch (error) {
            next(error);
        }
    }
}
exports.PosController = PosController;
//# sourceMappingURL=pos.controller.js.map