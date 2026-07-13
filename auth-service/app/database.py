from prisma import Prisma

db = Prisma()

async def get_db():
    """Dependency for getting Prisma client"""
    yield db
