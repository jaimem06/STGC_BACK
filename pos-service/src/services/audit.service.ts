import axios from 'axios';

export const auditAction = async (
  usuarioId: string,
  accion: string,
  detalles: any
) => {
  try {
    const url = `${process.env.AUTH_SERVICE_URL}/internal/audit`;
    const apiKey = process.env.INTERNAL_API_KEY;

    await axios.post(
      url,
      {
        usuario_id: usuarioId,
        accion,
        detalles,
        timestamp: new Date().toISOString(),
      },
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
