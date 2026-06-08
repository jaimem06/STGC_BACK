import { Request, Response, NextFunction } from 'express';
import { AppError } from '../utils/AppError';

export const globalErrorHandler = (
  err: Error,
  req: Request,
  res: Response,
  next: NextFunction
) => {
  if (err instanceof AppError) {
    return res.status(err.statusCode).json({
      status: 'error',
      message: err.message,
    });
  }

  // Errores inesperados
  console.error('ERROR 💥', err);
  return res.status(500).json({
    status: 'error',
    message: 'Algo salió muy mal!',
  });
};
