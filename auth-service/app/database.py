from prisma import Prisma

from app.config import settings

# La URL se pasa explícitamente para que siempre venga de la configuración validada.
db = Prisma(datasource={"url": settings.database_url})


async def get_db():
    """Dependencia que entrega el cliente Prisma compartido."""
    yield db
