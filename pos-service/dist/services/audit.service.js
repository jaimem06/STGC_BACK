"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.auditAction = void 0;
const axios_1 = __importDefault(require("axios"));
const auditAction = async (req, action) => {
    try {
        const url = `${process.env.AUTH_SERVICE_URL}/internal/audit`;
        const apiKey = process.env.INTERNAL_API_KEY;
        // Payload exacto según requerimiento Sprint 3
        const payload = {
            user_id: req.user?.sub,
            action: action,
            endpoint: req.originalUrl || req.url,
            ip_address: req.ip || req.socket.remoteAddress
        };
        await axios_1.default.post(url, payload, {
            headers: {
                'X-Internal-Api-Key': apiKey,
            },
        });
    }
    catch (error) {
        // Silencioso ante fallos según requerimiento
        console.error('Audit failure:', error);
    }
};
exports.auditAction = auditAction;
//# sourceMappingURL=audit.service.js.map