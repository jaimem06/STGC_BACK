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
    const secret = process.env.JWT_SECRET;
    if (!secret) throw new AppError('JWT_SECRET no configurado', 500);

    const decoded = jwt.verify(token, secret) as JWTPayload;

    const allowedRoles = ['ADMIN', 'CAJERO_MESERO', 'GERENTE_GENERAL', 'GERENTE_OPERACIONES'];
    if (!allowedRoles.includes(decoded.role)) {
      throw new AppError('Prohibido - Rol insuficiente', 403);
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
