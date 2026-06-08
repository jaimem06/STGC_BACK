import dotenv from 'dotenv';
dotenv.config();

import express from 'express';
import cors from 'cors';
import helmet from 'helmet';
import morgan from 'morgan';
import posRoutes from './routes/pos.routes';
import { globalErrorHandler } from './middlewares/error.middleware';

const app = express();
const port = process.env.PORT || 3000;

// Middlewares globales
app.use(helmet());
app.use(cors());
app.use(morgan('dev'));
app.use(express.json());

// Rutas
app.use('/api/pos', posRoutes);

// Manejador de errores global (debe ir después de las rutas)
app.use(globalErrorHandler);

app.listen(port, () => {
  console.log(`🚀 POS Microservice running on port ${port}`);
});
