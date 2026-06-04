from datetime import datetime, timezone
from typing import Annotated, Callable
import logging
from fastapi import Depends, HTTPException, Request, status, BackgroundTasks
from fastapi.security import OAuth2PasswordBearer
from prisma import Prisma
from prisma.models import User

from app.security import decode_access_token
from app.database import get_db
from app.config import settings
from app.core import endpoints

logger = logging.getLogger("auth-service.dependencies")
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
        logger.warning("Fallo al decodificar token JWT")
        raise credentials_exception

    user_id: str = payload.get("sub")
    session_token: str = payload.get("session_token")

    if not user_id:
        logger.warning("Token JWT no contiene 'sub' (user_id)")
        raise credentials_exception

    user = await db.user.find_unique(
        where={"id": user_id},
        include={"role": True}
    )

    if user is None:
        logger.warning(f"Usuario {user_id} no encontrado en la base de datos")
        raise credentials_exception

    # Validación de sesión única (si el token de sesión no coincide, la sesión fue invalidada por otro login)
    # Solo validamos si ambos existen para evitar expulsar usuarios con tokens antiguos o tras migraciones
    if session_token and user.session_token and user.session_token != session_token:
        logger.warning(f"Sesión invalidada para usuario {user.email}. DB: {user.session_token} vs JWT: {session_token}")
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="SESSION_INVALIDATED",
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
            logger.info(f"Intento de acceso de usuario suspendido: {current_user.email}")
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail=f"Cuenta suspendida hasta {susp_until.strftime('%Y-%m-%d %H:%M:%S')} UTC",
            )

    # 2. Verificar estado
    if current_user.status != "ACTIVO":
        logger.info(f"Intento de acceso de usuario con estado {current_user.status}: {current_user.email}")
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail=f"Acceso denegado: {current_user.status}",
        )
    
    return current_user

class RoleChecker:
    def __init__(self, allowed_roles: list[str]):
        self.allowed_roles = allowed_roles

    def __call__(
        self,
        current_user: Annotated[User, Depends(get_current_active_user)],
    ) -> User:
        # El ADMIN siempre tiene acceso a todo
        if current_user.role.name == "ADMIN":
            return current_user

        if current_user.role.name not in self.allowed_roles:
            logger.warning(f"Usuario {current_user.email} con rol {current_user.role.name} intentó acceder a recurso que requiere {self.allowed_roles}")
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail=f"No tienes el rol necesario para esta acción. Requerido: {', '.join(self.allowed_roles)}",
            )
        return current_user

# Dependencias de roles comunes
require_all_access = RoleChecker(["ADMIN"])
require_manage_users = RoleChecker(["ADMIN", "GERENTE_GENERAL", "GERENTE_OPERACIONES", "CAPATAZ"])

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
    except Exception as e:
        logger.error(f"Error al registrar auditoría: {e}")

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
