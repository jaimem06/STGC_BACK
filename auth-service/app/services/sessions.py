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
from sqlalchemy import select, update
from sqlalchemy.ext.asyncio import AsyncSession
from starlette.requests import Request

from app.config import settings
from app.models import RotatedRefreshToken, User, UserSession
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


async def get_session(db: AsyncSession, session_id: str) -> Optional[UserSession]:
    """Sesión con su usuario y rol, siempre leída fresca de la base de datos."""
    return await db.scalar(
        select(UserSession)
        .where(UserSession.id == session_id)
        .execution_options(populate_existing=True)
    )


async def revoke_user_sessions(
    db: AsyncSession, user_id: str, except_session_id: Optional[str] = None
) -> int:
    stmt = (
        update(UserSession)
        .where(UserSession.user_id == user_id, UserSession.revoked_at.is_(None))
        .values(revoked_at=utcnow())
        .execution_options(synchronize_session=False)
    )
    if except_session_id:
        stmt = stmt.where(UserSession.id != except_session_id)
    result = await db.execute(stmt)
    await db.commit()
    return result.rowcount


async def revoke_session(db: AsyncSession, session_id: str) -> None:
    await db.execute(
        update(UserSession)
        .where(UserSession.id == session_id, UserSession.revoked_at.is_(None))
        .values(revoked_at=utcnow())
        .execution_options(synchronize_session=False)
    )
    await db.commit()


async def issue_session(db: AsyncSession, user: User, request: Request) -> dict:
    """Crea una sesión nueva y devuelve el par access/refresh token."""
    if settings.single_session:
        await revoke_user_sessions(db, user.id)

    refresh_token = generate_opaque_token()
    session = UserSession(
        user_id=user.id,
        refresh_token_hash=hash_token(refresh_token),
        user_agent=get_user_agent(request),
        ip_address=get_client_ip(request),
        expires_at=utcnow() + timedelta(days=settings.refresh_token_expire_days),
    )
    db.add(session)
    await db.commit()
    access_token, expires_in = create_access_token(user.id, user.role.name, session.id)
    return _token_response(access_token, expires_in, refresh_token)


def session_is_active(session: UserSession) -> bool:
    return session.revoked_at is None and as_utc(session.expires_at) > utcnow()


async def rotate_refresh_token(db: AsyncSession, refresh_token: str, request: Request) -> dict:
    """Valida un refresh token, lo rota y devuelve un nuevo par de tokens."""
    from app.dependencies import ensure_user_can_access  # evita import circular

    invalid = HTTPException(
        status_code=status.HTTP_401_UNAUTHORIZED,
        detail="Refresh token inválido o expirado",
    )
    presented_hash = hash_token(refresh_token)
    session = await db.scalar(
        select(UserSession)
        .where(UserSession.refresh_token_hash == presented_hash)
        .execution_options(populate_existing=True)
    )

    if session is None:
        rotated = await db.get(RotatedRefreshToken, presented_hash, populate_existing=True)
        if rotated and rotated.session.revoked_at is None:
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
    result = await db.execute(
        update(UserSession)
        .where(
            UserSession.id == session.id,
            UserSession.refresh_token_hash == presented_hash,
            UserSession.revoked_at.is_(None),
        )
        .values(
            refresh_token_hash=hash_token(new_refresh),
            last_used_at=utcnow(),
            ip_address=get_client_ip(request),
            user_agent=get_user_agent(request),
        )
        .execution_options(synchronize_session=False)
    )
    if result.rowcount != 1:
        await db.rollback()
        raise invalid
    db.add(RotatedRefreshToken(token_hash=presented_hash, session_id=session.id))
    await db.commit()

    access_token, expires_in = create_access_token(
        session.user.id, session.user.role.name, session.id
    )
    return _token_response(access_token, expires_in, new_refresh)
