from fastapi import FastAPI, Depends, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.openapi.docs import get_redoc_html
from fastapi.staticfiles import StaticFiles
from slowapi import _rate_limit_exceeded_handler
from slowapi.errors import RateLimitExceeded

from app.config import settings
from app.database import db
from app.dependencies import log_user_action, require_all_access
from app.routes import auth, roles, users
from app.limiter import limiter
from app.core import endpoints

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

@app.on_event("startup")
async def startup():
    await db.connect()

@app.on_event("shutdown")
async def shutdown():
    await db.disconnect()

@app.get(endpoints.HEALTH_CHECK, tags=["Health"])
async def health_check():
    return {
        "status": "online",
        "app": settings.app_name,
        "version": settings.app_version
    }

@app.get(
    f"{settings.api_prefix}{endpoints.ADMIN_ONLY_TEST}",
    dependencies=[Depends(require_all_access), Depends(log_user_action("access_admin_area"))],
    tags=["Test"]
)
async def admin_only_route():
    return {"message": "Bienvenido, Administrador o Gerente General"}

if __name__ == "__main__":
    import uvicorn
    uvicorn.run("app.main:app", host="0.0.0.0", port=8000, reload=True)
