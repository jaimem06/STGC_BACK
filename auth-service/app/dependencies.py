from datetime import datetime, timezone
from typing import Annotated, Callable
from fastapi import Depends, HTTPException, Request, status, BackgroundTasks
from fastapi.security import OAuth2PasswordBearer
from prisma import Prisma
from prisma.models import User

from app.security import decode_access_token
from app.database import get_db
from app.config import settings
from app.core import endpoints

oauth2_scheme = OAuth2PasswordBearer(tokenUrl=f"{settings.api_prefix}{endpoints.AUTH_PREFIX}{endpoints.AUTH_LOGIN}")

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
        detail="Credenciales inválidas",
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
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Sesión inválida o cuenta no encontrada",
            headers={"WWW-Authenticate": "Bearer"},
        )

    return user

async def get_current_active_user(
    current_user: Annotated[User, Depends(get_current_user)],
) -> User:
    # 1. Verificar si el usuario está suspendido por fechas
    now = datetime.now(timezone.utc)
    
    if current_user.suspended_from and current_user.suspended_until:
        susp_from = current_user.suspended_from.replace(tzinfo=timezone.utc) if current_user.suspended_from.tzinfo is None else current_user.suspended_from
        susp_until = current_user.suspended_until.replace(tzinfo=timezone.utc) if current_user.suspended_until.tzinfo is None else current_user.suspended_until

        if susp_from <= now <= susp_until:
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail=f"Cuenta suspendida hasta {susp_until.strftime('%Y-%m-%d %H:%M:%S')} UTC",
            )

    # 2. Verificar estado
    if current_user.status != "ACTIVO":
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail=f"Acceso denegado: {current_user.status}",
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

        if "all_access" in user_permissions:
            return current_user

        for perm in self.required_permissions:
            if perm not in user_permissions:
                raise HTTPException(
                    status_code=status.HTTP_403_FORBIDDEN,
                    detail=f"Falta permiso: {perm}",
                )
        return current_user

require_all_access = PermissionChecker(["all_access"])
require_manage_users = PermissionChecker(["manage_users"])

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
            _record_audit_log, db, current_user.id, action, str(request.url.path), ip_address
        )
        return None
    return _log_action_dependency
