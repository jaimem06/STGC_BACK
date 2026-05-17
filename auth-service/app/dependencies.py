from typing import Annotated, Callable
from fastapi import Depends, HTTPException, Request, status, BackgroundTasks
from fastapi.security import OAuth2PasswordBearer
from prisma import Prisma
from prisma.models import User

from app.security import decode_access_token
from app.database import get_db

oauth2_scheme = OAuth2PasswordBearer(tokenUrl="/api/auth/login")

def get_client_ip(request: Request) -> str:
    forwarded = request.headers.get("X-Forwarded-For")
    if forwarded:
        return forwarded.split(",")[0].strip()
    return request.client.host if request.client else "unknown"

async def get_current_user(
    token: Annotated[str, Depends(oauth2_scheme)],
    db: Annotated[Prisma, Depends(get_db)],
) -> User:
    credentials_exception = HTTPException(
        status_code=status.HTTP_401_UNAUTHORIZED,
        detail="No se pudieron validar las credenciales",
        headers={"WWW-Authenticate": "Bearer"},
    )

    session_invalidated_exception = HTTPException(
        status_code=status.HTTP_401_UNAUTHORIZED,
        detail="Sesión invalidada. Alguien más inició sesión con tu cuenta.",
        headers={"WWW-Authenticate": "Bearer"},
    )

    payload = decode_access_token(token)
    if payload is None:
        raise credentials_exception

    user_id: str = payload.get("sub")
    session_token: str = payload.get("session_token")

    if not user_id or not session_token:
        raise credentials_exception

    user = await db.user.find_unique(
        where={"id": user_id},
        include={"role": {"include": {"permissions": True}}}
    )

    if user is None or user.session_token != session_token:
        raise session_invalidated_exception if user else credentials_exception

    return user

async def get_current_active_user(
    current_user: Annotated[User, Depends(get_current_user)],
) -> User:
    if not current_user.is_active:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Usuario inactivo",
        )
    return current_user

class PermissionChecker:
    def __init__(self, required_permissions: list[str]):
        self.required_permissions = required_permissions

    def __call__(
        self,
        current_user: Annotated[User, Depends(get_current_active_user)],
    ) -> User:
        user_permissions = [p.name for p in current_user.role.permissions]

        # El permiso 'all_access' otorga acceso total independientemente de lo requerido
        if "all_access" in user_permissions:
            return current_user

        for perm in self.required_permissions:
            if perm not in user_permissions:
                raise HTTPException(
                    status_code=status.HTTP_403_FORBIDDEN,
                    detail=f"No tienes el permiso necesario: {perm}",
                )
        return current_user

# Dependencias de permisos comunes
require_all_access = PermissionChecker(["all_access"])
require_manage_users = PermissionChecker(["manage_users"])
require_productor = PermissionChecker(["productor_actions"])
require_catador = PermissionChecker(["catador_actions"])
require_barista = PermissionChecker(["barista_actions"])
require_capataz = PermissionChecker(["capataz_actions"])
require_tostador = PermissionChecker(["tostador_actions"])
require_gestor_calidad = PermissionChecker(["calidad_actions"])

async def _record_audit_log(db: Prisma, user_id: str, action: str, endpoint: str, ip_address: str):
    try:
        await db.auditlog.create(
            data={
                "user_id": user_id,
                "action": action,
                "endpoint": endpoint,
                "ip_address": ip_address,
            }
        )
    except Exception:
        pass

def log_user_action(action: str) -> Callable:
    async def _log_action_dependency(
        request: Request,
        background_tasks: BackgroundTasks,
        current_user: Annotated[User, Depends(get_current_user)],
        db: Annotated[Prisma, Depends(get_db)],
    ):
        ip_address = get_client_ip(request)
        background_tasks.add_task(
            _record_audit_log, 
            db, 
            current_user.id, 
            action, 
            str(request.url.path), 
            ip_address
        )
        return None
    return _log_action_dependency
