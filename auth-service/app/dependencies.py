import logging
from dataclasses import dataclass
from datetime import datetime
from typing import Annotated, Any, Optional

from fastapi import Depends, HTTPException, status
from fastapi.security import HTTPAuthorizationCredentials, HTTPBearer
from sqlalchemy.ext.asyncio import AsyncSession

from app.config import settings
from app.database import get_db
from app.models import User, UserSession, UserStatus
from app.security import ACCESS_TOKEN_TYPE, decode_token
from app.services.sessions import as_utc, get_session, session_is_active, utcnow

logger = logging.getLogger("auth-service.dependencies")

bearer_scheme = HTTPBearer(auto_error=False, description="Access token JWT emitido por /auth/login")

Db = Annotated[AsyncSession, Depends(get_db)]


# ---------------------------------------------------------------------------
# Estado de la cuenta
# ---------------------------------------------------------------------------

def get_active_suspension_until(user: User) -> Optional[datetime]:
    """Fecha de fin de la suspensión si el usuario está suspendido ahora mismo."""
    if not (user.suspended_from and user.suspended_until):
        return None
    now = utcnow()
    if as_utc(user.suspended_from) <= now <= as_utc(user.suspended_until):
        return as_utc(user.suspended_until)
    return None


def ensure_user_can_access(user: User) -> None:
    """Lanza 403 si el usuario está suspendido o su estado no es ACTIVO."""
    suspended_until = get_active_suspension_until(user)
    if suspended_until:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail=f"Cuenta suspendida hasta {suspended_until.strftime('%Y-%m-%d %H:%M:%S')} UTC",
        )
    if user.status != UserStatus.ACTIVO:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail=f"Acceso denegado: cuenta {user.status.value}",
        )


# ---------------------------------------------------------------------------
# Usuario autenticado
# ---------------------------------------------------------------------------

@dataclass
class AuthContext:
    user: User
    session: UserSession
    claims: dict[str, Any]


def _unauthorized(detail: str = "No autenticado") -> HTTPException:
    return HTTPException(
        status_code=status.HTTP_401_UNAUTHORIZED,
        detail=detail,
        headers={"WWW-Authenticate": "Bearer"},
    )


async def get_auth_context(
    credentials: Annotated[Optional[HTTPAuthorizationCredentials], Depends(bearer_scheme)],
    db: Db,
) -> AuthContext:
    """
    Valida el access token y comprueba contra la base de datos que su sesión
    sigue activa. Así logout, cambio de contraseña o suspensión surten efecto
    inmediato en este servicio aunque el JWT aún no haya expirado.
    """
    if credentials is None or credentials.scheme.lower() != "bearer":
        raise _unauthorized()

    payload = decode_token(credentials.credentials, ACCESS_TOKEN_TYPE)
    if payload is None or not payload.get("sid"):
        raise _unauthorized("Token inválido o expirado")

    session = await get_session(db, payload["sid"])
    if session is None or session.user_id != payload["sub"] or not session_is_active(session):
        raise _unauthorized("SESSION_INVALIDATED")

    ensure_user_can_access(session.user)
    return AuthContext(user=session.user, session=session, claims=payload)


async def get_current_user(ctx: Annotated[AuthContext, Depends(get_auth_context)]) -> User:
    return ctx.user


# ---------------------------------------------------------------------------
# Roles y privilegios
# ---------------------------------------------------------------------------

def is_admin(user: User) -> bool:
    return user.role is not None and user.role.name == settings.admin_role_name


class RoleChecker:
    """
    Exige que el usuario tenga uno de los roles indicados.
    El rol administrador siempre tiene acceso.
    """

    def __init__(self, allowed_roles: list[str]):
        self.allowed_roles = allowed_roles

    def __call__(self, current_user: Annotated[User, Depends(get_current_user)]) -> User:
        if is_admin(current_user) or current_user.role.name in self.allowed_roles:
            return current_user
        logger.warning(
            f"Usuario {current_user.id} con rol {current_user.role.name} intentó acceder "
            f"a un recurso que requiere {self.allowed_roles}"
        )
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="No tienes permisos para esta acción",
        )


require_admin = RoleChecker([settings.admin_role_name])
require_manage_users = RoleChecker(settings.user_manager_role_list)


def ensure_can_manage_target(actor: User, target: User) -> None:
    """
    Reglas anti-escalada de privilegios: un gestor que no es admin no puede modificar
    a usuarios con roles privilegiados (admin u otros gestores).
    """
    if is_admin(actor):
        return
    if target.role.name in settings.privileged_role_names:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Solo un administrador puede modificar a usuarios con roles privilegiados",
        )


def ensure_can_assign_role(actor: User, role_name: str) -> None:
    """Solo el administrador puede otorgar roles privilegiados."""
    if is_admin(actor):
        return
    if role_name in settings.privileged_role_names:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail=f"Solo un administrador puede asignar el rol '{role_name}'",
        )
