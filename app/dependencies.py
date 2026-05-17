from typing import Annotated, Callable

from fastapi import Depends, HTTPException, Request, status
from fastapi.security import OAuth2PasswordBearer
from prisma import Prisma
from prisma.models import User
from prisma.enums import Role

from app.security import decode_access_token
from app.database import get_db


oauth2_scheme = OAuth2PasswordBearer(tokenUrl="/api/auth/login")


def get_client_ip(request: Request) -> str:
    """Get client IP address from request headers"""
    forwarded = request.headers.get("X-Forwarded-For")
    if forwarded:
        return forwarded.split(",")[0].strip()
    return request.client.host if request.client else "unknown"


async def get_current_user(
    token: Annotated[str, Depends(oauth2_scheme)],
    db: Annotated[Prisma, Depends(get_db)],
) -> User:
    """
    Dependency to get current authenticated user and validate single session.
    """
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

    user = await db.user.find_unique(where={"id": user_id})

    if user is None:
        raise credentials_exception

    if user.session_token != session_token:
        raise session_invalidated_exception

    return user


async def get_current_active_user(
    current_user: Annotated[User, Depends(get_current_user)],
) -> User:
    """Dependency to ensure user is active"""
    if not current_user.is_active:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Usuario inactivo",
        )
    return current_user


class RoleChecker:
    """Checker for user roles with inheritance for ADMIN and GERENTE_GENERAL"""

    def __init__(self, allowed_roles: list[Role]):
        self.allowed_roles = allowed_roles

    def __call__(
        self,
        current_user: Annotated[User, Depends(get_current_active_user)],
    ) -> User:
        # ADMIN and GERENTE_GENERAL have access to everything
        if current_user.role in [Role.ADMIN, Role.GERENTE_GENERAL]:
            return current_user

        if current_user.role not in self.allowed_roles:
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail="No tienes permisos suficientes para realizar esta acción",
            )
        return current_user


# Role-specific dependency instances
require_admin = RoleChecker([Role.ADMIN])
require_productor = RoleChecker([Role.PRODUCTOR])
require_catador = RoleChecker([Role.CATADOR])
require_barista = RoleChecker([Role.BARISTA])
require_capataz = RoleChecker([Role.CAPATAZ])
require_tostador = RoleChecker([Role.TOSTADOR])
require_gestor_calidad = RoleChecker([Role.GESTOR_CALIDAD])


from fastapi import Depends, HTTPException, Request, status, BackgroundTasks
...
async def _record_audit_log(db: Prisma, user_id: str, action: str, endpoint: str, ip_address: str):
    """Internal helper to write audit log in background"""
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
        # Prevent audit log failure from crashing the app
        pass


def log_user_action(action: str) -> Callable:
    """
    Dependency factory to log user actions in the AuditLog via BackgroundTasks.
    """
    async def _log_action_dependency(
        request: Request,
        background_tasks: BackgroundTasks,
        current_user: Annotated[User, Depends(get_current_user)],
        db: Annotated[Prisma, Depends(get_db)],
    ):
        ip_address = get_client_ip(request)
        # We add the task to background so the user gets their response faster
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
