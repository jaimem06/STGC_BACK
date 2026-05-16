from prisma import Prisma

db = Prisma(auto_connect=True)

async def get_db():
    """Dependency for getting Prisma client"""
    # Prisma client is typically used as a singleton in the app
    # but we yield it here for consistency with the FastAPI pattern.
    yield db
