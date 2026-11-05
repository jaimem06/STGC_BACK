"""
Gestión de sesiones con refresh tokens rotativos y detección de reutilización.

- Cada login crea una fila en user_sessions con el hash del refresh token.
- Cada /refresh entrega un refresh token nuevo e invalida el anterior.
- Se guarda el hash de cada refresh token rotado. Si alguien presenta cualquiera
  de ellos (señal de robo), la sesión completa se revoca y tanto el atacante
  como la víctima deben volver a entrar.
"""

from datetime import datetime, timedelta, timezone
from typing import Optional

from fastapi import HTTPException, status
from prisma import Prisma
from prisma.models import User
from starlette.requests import Request

from app.config import settings
from app.security import create_access_token, generate_opaque_token, hash_token
from app.services.audit import record_audit
from app.utils.request import get_client_ip, get_user_agent


def utcnow() -> datetime:
    return datetime.now(timezone.utc)


def as_utc(value: Optional[datetime]) -> Optional[datetime]:
    if value is None:
        return None
    return value.replace(tzinfo=timezone.utc) if value.tzinfo is None else value


def _token_response(access_token: str, expires_in: int, refresh_token: str) -> dict:
    return {
        "access_token": access_token,
        "refresh_token": refresh_token,
        "token_type": "bearer",
        "expires_in": expires_in,
        "refresh_expires_in": settings.refresh_token_expire_days * 86400,
    }


async def revoke_user_sessions(
    db: Prisma, user_id: str, except_session_id: Optional[str] = None
) -> int:
    where: dict = {"user_id": user_id, "revoked_at": None}
    if except_session_id:
        where["NOT"] = {"id": except_session_id}
    return await db.usersession.update_many(where=where, data={"revoked_at": utcnow()})


async def revoke_session(db: Prisma, session_id: str) -> None:
    await db.usersession.update_many(
        where={"id": session_id, "revoked_at": None}, data={"revoked_at": utcnow()}
    )


async def issue_session(db: Prisma, user: User, request: Request) -> dict:
    """Crea una sesión nueva y devuelve el par access/refresh token."""
    if settings.single_session:
        await revoke_user_sessions(db, user.id)

    refresh_token = generate_opaque_token()
    session = await db.usersession.create(
        data={
            "user_id": user.id,
            "refresh_token_hash": hash_token(refresh_token),
            "user_agent": get_user_agent(request),
            "ip_address": get_client_ip(request),
            "expires_at": utcnow() + timedelta(days=settings.refresh_token_expire_days),
        }
    )
    access_token, expires_in = create_access_token(user.id, user.role.name, session.id)
    return _token_response(access_token, expires_in, refresh_token)


def session_is_active(session) -> bool:
    return session.revoked_at is None and as_utc(session.expires_at) > utcnow()


async def rotate_refresh_token(db: Prisma, refresh_token: str, request: Request) -> dict:
    """Valida un refresh token, lo rota y devuelve un nuevo par de tokens."""
    from app.dependencies import ensure_user_can_access  # evita import circular

    invalid = HTTPException(
        status_code=status.HTTP_401_UNAUTHORIZED,
        detail="Refresh token inválido o expirado",
    )
    presented_hash = hash_token(refresh_token)
    session = await db.usersession.find_unique(
        where={"refresh_token_hash": presented_hash},
        include={"user": {"include": {"role": True}}},
    )

    if session is None:
        rotated = await db.rotatedrefreshtoken.find_unique(
            where={"token_hash": presented_hash}, include={"session": True}
        )
        if rotated and rotated.session and rotated.session.revoked_at is None:
            await revoke_session(db, rotated.session_id)
            await record_audit(
                db, "refresh_token_reuse_detected",
                user_id=rotated.session.user_id, request=request,
                details={"session_id": rotated.session_id},
            )
        raise invalid

    if not session_is_active(session):
        raise invalid

    ensure_user_can_access(session.user)

    new_refresh = generate_opaque_token()
    # Actualización condicional: si dos peticiones rotan a la vez, solo una gana.
    updated = await db.usersession.update_many(
        where={"id": session.id, "refresh_token_hash": presented_hash, "revoked_at": None},
        data={
            "refresh_token_hash": hash_token(new_refresh),
            "last_used_at": utcnow(),
            "ip_address": get_client_ip(request),
            "user_agent": get_user_agent(request),
        },
    )
    if updated != 1:
        raise invalid
    await db.rotatedrefreshtoken.create(data={"token_hash": presented_hash, "session_id": session.id})

    access_token, expires_in = create_access_token(
        session.user.id, session.user.role.name, session.id
    )
    return _token_response(access_token, expires_in, new_refresh)
