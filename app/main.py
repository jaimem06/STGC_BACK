from fastapi import FastAPI, Depends, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.openapi.docs import get_redoc_html
from fastapi.staticfiles import StaticFiles
from slowapi import Limiter, _rate_limit_exceeded_handler
from slowapi.util import get_remote_address
from slowapi.errors import RateLimitExceeded

from app.config import settings
from app.database import db
from app.dependencies import log_user_action, require_admin
from app.routes import auth

# Inicializar el limitador de peticiones por IP
limiter = Limiter(key_func=get_remote_address)

app = FastAPI(
    title=settings.app_name,
    version=settings.app_version,
    openapi_url="/openapi.json",
    docs_url=None,
    redoc_url=None,
)

# Configurar el estado del limitador y el manejador de errores
app.state.limiter = limiter
app.add_exception_handler(RateLimitExceeded, _rate_limit_exceeded_handler)

# Servir archivos estáticos localmente para evitar problemas de CDN
app.mount("/static", StaticFiles(directory="static"), name="static")

# Configure CORS (En producción, restringir a orígenes específicos)
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Custom ReDoc route using LOCAL assets
@app.get("/docs", include_in_schema=False)
async def custom_redoc_html():
    return get_redoc_html(
        openapi_url=app.openapi_url,
        title=f"{app.title} - ReDoc",
        redoc_js_url="/static/redoc.standalone.js",
    )

# Include Routers
app.include_router(auth.router, prefix=settings.api_prefix)


@app.on_event("startup")
async def startup():
    # Connect to the database
    await db.connect()


@app.on_event("shutdown")
async def shutdown():
    # Disconnect from the database
    await db.disconnect()


@app.get("/", tags=["Health"])
async def health_check():
    return {
        "status": "online",
        "app": settings.app_name,
        "version": settings.app_version
    }


@app.get(
    f"{settings.api_prefix}/admin-only",
    dependencies=[Depends(require_admin), Depends(log_user_action("access_admin_area"))],
    tags=["Test"]
)
async def admin_only_route():
    return {"message": "Bienvenido, Administrador o Gerente General"}


if __name__ == "__main__":
    import uvicorn
    uvicorn.run("app.main:app", host="0.0.0.0", port=8000, reload=True)
