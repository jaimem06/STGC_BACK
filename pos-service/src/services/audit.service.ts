import axios from 'axios';
import { Request } from 'express';

export const auditAction = async (
  req: Request,
  action: string
) => {
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

    await axios.post(
      url,
      payload,
      {
        headers: {
          'X-Internal-Api-Key': apiKey,
        },
      }
    );
  } catch (error) {
    // Silencioso ante fallos según requerimiento
    console.error('Audit failure:', error);
  }
};
