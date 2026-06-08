"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
const dotenv_1 = __importDefault(require("dotenv"));
dotenv_1.default.config();
const express_1 = __importDefault(require("express"));
const cors_1 = __importDefault(require("cors"));
const helmet_1 = __importDefault(require("helmet"));
const morgan_1 = __importDefault(require("morgan"));
const redoc_express_1 = __importDefault(require("redoc-express"));
const pos_routes_1 = __importDefault(require("./routes/pos.routes"));
const error_middleware_1 = require("./middlewares/error.middleware");
const path_1 = __importDefault(require("path"));
const app = (0, express_1.default)();
const port = process.env.PORT || 3000;
// Middlewares globales
app.use((0, helmet_1.default)({
    contentSecurityPolicy: false, // Necesario para ReDoc si se carga desde CDN
}));
app.use((0, cors_1.default)());
app.use((0, morgan_1.default)('dev'));
app.use(express_1.default.json());
// Documentación ReDoc (Sprint 3)
app.get('/docs/openapi.json', (req, res) => {
    res.sendFile(path_1.default.join(__dirname, 'openapi.json'));
});
app.get('/docs', (0, redoc_express_1.default)({
    title: 'POS Service API Docs',
    specUrl: '/docs/openapi.json'
}));
// Rutas
app.use('/api/pos', pos_routes_1.default);
// Manejador de errores global (debe ir después de las rutas)
app.use(error_middleware_1.globalErrorHandler);
app.listen(port, () => {
    console.log(`🚀 POS Microservice running on port ${port}`);
    console.log(`📖 Documentation available at http://localhost:${port}/docs`);
});
//# sourceMappingURL=index.js.map