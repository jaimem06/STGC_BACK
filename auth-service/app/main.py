import asyncio
import logging
import secrets
import sys
from contextlib import asynccontextmanager

from fastapi import FastAPI, HTTPException, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.openapi.docs import get_redoc_html
from fastapi.responses import JSONResponse
from fastapi.staticfiles import StaticFiles
from slowapi.errors import RateLimitExceeded
from slowapi.middleware import SlowAPIMiddleware
from sqlalchemy import text

from app.config import is_weak_secret, security_problems, security_warnings, settings
from app.core import endpoints
from app.database import engine
from app.limiter import limiter
from app.middleware import BodySizeLimitMiddleware, SecurityHeadersMiddleware
from app.routes import account, audit, auth, internal, roles, users
from app.security import get_jwks

logging.basicConfig(
    level=logging.DEBUG if settings.debug else logging.INFO,
    format="%(asctime)s - %(levelname)s - %(name)s - %(message)s",
    stream=sys.stdout,
)
logger = logging.getLogger("auth-service")


def check_configuration() -> None:
    """
    Fuera de desarrollo, una configuración insegura impide arrancar el servicio.
    En desarrollo, las claves débiles se sustituyen por claves aleatorias temporales
    para que nunca se firme nada con un texto de ejemplo conocido.
    """
    for warning in security_warnings(settings):
        logger.warning("Seguridad: %s", warning)
    problems = security_problems(settings)
    if not problems:
        return
    if settings.is_strict:
        raise RuntimeError("Configuración insegura: " + "; ".join(problems))
    for name in ("secret_key", "internal_api_key"):
        if is_weak_secret(getattr(settings, name)):
            setattr(settings, name, secrets.token_urlsafe(48))
            logger.warning("%s débil: se usa una clave aleatoria temporal (los tokens no sobreviven reinicios)",
                           name.upper())
    for problem in problems:
        logger.warning("Seguridad (bloquearía el arranque en producción): %s", problem)


async def _ping_database() -> None:
    async with engine.connect() as connection:
        await connection.execute(text("SELECT 1"))


@asynccontextmanager
async def lifespan(_: FastAPI):
    check_configuration()
    retries = 3
    for attempt in range(1, retries + 1):
        try:
            # Bases serverless (Neon) pueden tardar en despertar: se reintenta.
            await asyncio.wait_for(_ping_database(), timeout=30)
            logger.info("Base de datos conectada")
            break
        except Exception as e:
            if attempt < retries:
                logger.warning(f"Intento {attempt} de conexión a la base de datos falló. Reintentando en 2s...")
                await asyncio.sleep(2)
            else:
                logger.error(f"No se pudo conectar a la base de datos tras {retries} intentos: {type(e).__name__}")
                raise
    yield
    await engine.dispose()


app = FastAPI(
    title=settings.app_display_name,
    version=settings.app_version,
    description="Servicio de autenticación, sesiones, roles y auditoría reutilizable entre proyectos.",
    openapi_url="/openapi.json" if settings.docs_enabled else None,
    docs_url=None,
    redoc_url=None,
    lifespan=lifespan,
)

app.state.limiter = limiter


@app.exception_handler(RateLimitExceeded)
async def _rate_limit_exceeded_handler(request: Request, exc: RateLimitExceeded):
    return JSONResponse(
        status_code=429,
        content={"detail": "Demasiadas peticiones. Inténtalo de nuevo más tarde."},
        headers={"Retry-After": "60"},
    )


@app.exception_handler(Exception)
async def _unhandled_exception_handler(request: Request, exc: Exception):
    # Nunca devolver trazas ni mensajes internos al cliente.
    logger.exception("Error no controlado en %s %s", request.method, request.url.path)
    return JSONResponse(status_code=500, content={"detail": "Error interno del servidor"})


# Orden: el último middleware añadido es el primero en ejecutarse.
app.add_middleware(SlowAPIMiddleware)
origins = settings.cors_origin_list
app.add_middleware(
    CORSMiddleware,
    allow_origins=origins,
    # Los navegadores rechazan credenciales junto con "*"; solo se habilitan con orígenes explícitos.
    allow_credentials="*" not in origins,
    allow_methods=["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"],
    allow_headers=["Authorization", "Content-Type", "X-Internal-Api-Key"],
    expose_headers=["X-Total-Count", "Retry-After"],
    max_age=600,
)
app.add_middleware(SecurityHeadersMiddleware)
app.add_middleware(BodySizeLimitMiddleware)

if settings.docs_enabled:
    app.mount("/static", StaticFiles(directory="static"), name="static")

    @app.get(endpoints.DOCS, include_in_schema=False)
    async def custom_redoc_html():
        return get_redoc_html(
            openapi_url=app.openapi_url,
            title=f"{app.title} - ReDoc",
            redoc_js_url="/static/redoc.standalone.js",
        )


for module in (auth, account, users, roles, audit, internal):
    app.include_router(module.router, prefix=settings.api_prefix)


@app.get(endpoints.HEALTH_CHECK, tags=["Salud"], summary="Estado del servicio")
@limiter.exempt
async def health_check():
    return {"status": "online", "app": settings.app_name, "version": settings.app_version}


@app.get(endpoints.HEALTH_READY, tags=["Salud"], summary="Disponibilidad (incluye base de datos)")
@limiter.exempt
async def readiness():
    try:
        await asyncio.wait_for(_ping_database(), timeout=5)
    except Exception:
        return JSONResponse(status_code=503, content={"status": "unavailable", "database": "down"})
    return {"status": "ready", "database": "up"}


@app.get(
    endpoints.JWKS,
    tags=["Salud"],
    summary="Claves públicas JWT (JWKS)",
    description="Solo disponible con JWT_ALGORITHM RS*. Permite a otros servicios validar tokens sin secretos.",
)
async def jwks():
    keys = get_jwks()
    if keys is None:
        raise HTTPException(status_code=404, detail="JWKS no disponible con algoritmos simétricos")
    return JSONResponse(content=keys, headers={"Cache-Control": "public, max-age=3600"})


if __name__ == "__main__":
    import uvicorn

    uvicorn.run("app.main:app", host="0.0.0.0", port=8000, reload=True)
