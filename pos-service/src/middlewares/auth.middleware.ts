import { Request, Response, NextFunction } from 'express';
import jwt from 'jsonwebtoken';
import { AppError } from '../utils/AppError';
import prisma from '../config/database';

interface JWTPayload {
  sub: string;
  role: string;
  session_token: string;
}

declare global {
  namespace Express {
    interface Request {
      user?: JWTPayload;
    }
  }
}

export const authMiddleware = async (
  req: Request,
  res: Response,
  next: NextFunction
) => {
  try {
    const authHeader = req.headers.authorization;
    if (!authHeader || !authHeader.startsWith('Bearer ')) {
      throw new AppError('No autorizado - Token faltante', 401);
    }

    const token = authHeader.split(' ')[1];
    const secret = process.env.JWT_SECRET || 'secret';

    const decoded = jwt.verify(token, secret) as JWTPayload;

    // VALIDACIÓN ESTRICTA DE ROL (Sprint 3)
    if (decoded.role !== 'ADMIN' && decoded.role !== 'CAJERO') {
      throw new AppError('Prohibido - Rol insuficiente', 403);
    }

    // Validar session_token contra BD
    const sessionActive = await prisma.activeSession.findUnique({
      where: { sessionToken: decoded.session_token }
    });

    if (!sessionActive) {
      throw new AppError('Sesión inválida o expirada', 401);
    }

    req.user = decoded;
    next();
  } catch (error) {
    if (error instanceof jwt.JsonWebTokenError) {
      next(new AppError('Token inválido', 401));
    } else {
      next(error);
    }
  }
};
