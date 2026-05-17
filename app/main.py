from fastapi import FastAPI, Depends
from fastapi.middleware.cors import CORSMiddleware

from app.config import settings
from app.database import db
from app.dependencies import log_user_action, require_admin
from app.routes import auth

app = FastAPI(
    title=settings.app_name,
    version=settings.app_version,
    openapi_url=f"{settings.api_prefix}/openapi.json",
    docs_url=None,
    redoc_url=f"{settings.api_prefix}/docs",
)

# Configure CORS
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
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
