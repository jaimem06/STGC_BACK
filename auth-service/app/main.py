from fastapi import FastAPI, Depends, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.openapi.docs import get_redoc_html
from fastapi.staticfiles import StaticFiles
from slowapi import _rate_limit_exceeded_handler
from slowapi.errors import RateLimitExceeded
import logging
import sys

from app.config import settings
from app.database import db
from app.dependencies import log_user_action, require_all_access
from app.routes import auth, roles, users, internal
from app.limiter import limiter
from app.core import endpoints

# Configuración básica de logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(levelname)s - %(message)s",
    stream=sys.stdout
)
logger = logging.getLogger("auth-service")

app = FastAPI(
    title=settings.app_name,
    version=settings.app_version,
    openapi_url="/openapi.json",
    docs_url=None,
    redoc_url=None,
)

app.state.limiter = limiter
app.add_exception_handler(RateLimitExceeded, _rate_limit_exceeded_handler)

app.mount("/static", StaticFiles(directory="static"), name="static")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

@app.get(endpoints.DOCS, include_in_schema=False)
async def custom_redoc_html():
    return get_redoc_html(
        openapi_url=app.openapi_url,
        title=f"{app.title} - ReDoc",
        redoc_js_url="/static/redoc.standalone.js",
    )

app.include_router(auth.router, prefix=settings.api_prefix)
app.include_router(roles.router, prefix=settings.api_prefix)
app.include_router(users.router, prefix=settings.api_prefix)
app.include_router(internal.router, prefix=settings.api_prefix)

@app.on_event("startup")
async def startup():
    import asyncio
    retries = 3
    for i in range(retries):
        try:
            # Timeout aumentado para Cold Start de Neon
            await db.connect(timeout=30)
            logger.info("Database connected successfully")
            return
        except Exception as e:
            if i < retries - 1:
                logger.warning(f"Database connection attempt {i+1} failed. Retrying in 2s...")
                await asyncio.sleep(2)
            else:
                logger.error(f"Could not connect to database after {retries} attempts: {e}")
                raise e

@app.on_event("shutdown")
async def shutdown():
    await db.disconnect()

@app.get(
    endpoints.HEALTH_CHECK, 
    tags=["Salud"],
    summary="Comprobar Estado del Servidor",
    description="Verifica si el servicio de autenticación está en línea y devuelve la versión actual."
)
async def health_check():
    return {
        "status": "online",
        "app": settings.app_name,
        "version": settings.app_version
    }

@app.get(
    f"{settings.api_prefix}{endpoints.ADMIN_ONLY_TEST}",
    dependencies=[Depends(require_all_access), Depends(log_user_action("access_admin_area"))],
    tags=["Pruebas"],
    summary="Ruta de Prueba para Administradores",
    description="Endpoint de prueba para validar el acceso restringido a usuarios con el rol ADMIN."
)
async def admin_only_route():
    return {"message": "Bienvenido, Administrador o Gerente General"}

if __name__ == "__main__":
    import uvicorn
    uvicorn.run("app.main:app", host="0.0.0.0", port=8000, reload=True)
