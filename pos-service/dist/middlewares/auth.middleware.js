"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.authMiddleware = void 0;
const jsonwebtoken_1 = __importDefault(require("jsonwebtoken"));
const AppError_1 = require("../utils/AppError");
const database_1 = __importDefault(require("../config/database"));
const authMiddleware = async (req, res, next) => {
    try {
        const authHeader = req.headers.authorization;
        if (!authHeader || !authHeader.startsWith('Bearer ')) {
            throw new AppError_1.AppError('No autorizado - Token faltante', 401);
        }
        const token = authHeader.split(' ')[1];
        const secret = process.env.JWT_SECRET || 'secret';
        const decoded = jsonwebtoken_1.default.verify(token, secret);
        // VALIDACIÓN ESTRICTA DE ROL (Sprint 3)
        if (decoded.role !== 'ADMIN' && decoded.role !== 'CAJERO') {
            throw new AppError_1.AppError('Prohibido - Rol insuficiente', 403);
        }
        // Validar session_token contra BD
        const sessionActive = await database_1.default.activeSession.findUnique({
            where: { sessionToken: decoded.session_token }
        });
        if (!sessionActive) {
            throw new AppError_1.AppError('Sesión inválida o expirada', 401);
        }
        req.user = decoded;
        next();
    }
    catch (error) {
        if (error instanceof jsonwebtoken_1.default.JsonWebTokenError) {
            next(new AppError_1.AppError('Token inválido', 401));
        }
        else {
            next(error);
        }
    }
};
exports.authMiddleware = authMiddleware;
//# sourceMappingURL=auth.middleware.js.map