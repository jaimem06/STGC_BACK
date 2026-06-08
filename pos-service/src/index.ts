import dotenv from 'dotenv';
dotenv.config();
// @ts-expect-error Temporary fallback until @types/express is installed.
import express from 'express';
// @ts-expect-error Missing type declarations for cors
import cors from 'cors';
import helmet from 'helmet';
// @ts-expect-error Missing type declarations for morgan
import morgan from 'morgan';
import redoc from 'redoc-express';
import posRoutes from './routes/pos.routes';
import { globalErrorHandler } from './middlewares/error.middleware';
import path from 'path';

const app = express();
const port = process.env.PORT || 3000;

app.use(helmet({
  contentSecurityPolicy: false,
}));
app.use(cors());
app.use(morgan('dev'));
app.use(express.json());

app.get('/docs/openapi.json', (_req: express.Request, res: express.Response) => {
  res.sendFile(path.join(__dirname, 'openapi.json'));
});

app.get('/docs', redoc({
  title: 'POS Service API Docs',
  specUrl: '/docs/openapi.json'
}));

app.use('/api/pos', posRoutes);

app.use(globalErrorHandler);

app.listen(port, () => {
  console.log(`🚀 POS Microservice running on port ${port}`);
  console.log(`📖 Documentation available at http://localhost:${port}/docs`);
});
