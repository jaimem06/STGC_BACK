import prisma from '../config/database';

/** Cédula/RUC genérica de consumidor final (solo nueves): nunca se guarda como cliente real. */
function esCedulaConsumidorFinal(cedula: string): boolean {
  const c = cedula.trim();
  return c === '' || /^9+$/.test(c);
}

/** Busca clientes ya facturados por cédula (prefijo) o por nombre/apellido, para autocompletar. */
export async function buscarClientes(query: string) {
  const q = query.trim();
  if (q.length < 2) return [];

  return prisma.cliente.findMany({
    where: {
      OR: [
        { cedula: { startsWith: q } },
        { nombre: { contains: q, mode: 'insensitive' } },
        { apellido: { contains: q, mode: 'insensitive' } },
      ],
    },
    orderBy: { ultimoUso: 'desc' },
    take: 8,
  });
}

/**
 * Guarda o actualiza el cliente para poder reutilizarlo en futuras
 * facturaciones y evitar que el cajero vuelva a digitar sus datos.
 * Best-effort: nunca debe interrumpir un cobro ya confirmado.
 */
export async function guardarClienteParaReutilizar(
  nombre?: string | null,
  apellido?: string | null,
  cedula?: string | null
): Promise<void> {
  const ced = (cedula || '').trim();
  const nom = (nombre || '').trim();
  const ape = (apellido || '').trim();
  if (!ced || esCedulaConsumidorFinal(ced) || !nom) return;

  try {
    await prisma.cliente.upsert({
      where: { cedula: ced },
      create: { cedula: ced, nombre: nom, apellido: ape, vecesUsado: 1, ultimoUso: new Date() },
      update: { nombre: nom, apellido: ape, vecesUsado: { increment: 1 }, ultimoUso: new Date() },
    });
  } catch (error) {
    console.error('No se pudo guardar el cliente para reutilización futura:', error);
  }
}
