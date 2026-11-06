"""
Tokens de un solo uso (recuperación de contraseña, invitación, verificación de email).
El token en claro solo viaja en el correo; en la base de datos se guarda su hash.
"""

from datetime import timedelta
from typing import Optional

from sqlalchemy import func, select, update
from sqlalchemy.ext.asyncio import AsyncSession

from app.models import AuthToken, TokenPurpose
from app.security import generate_opaque_token, hash_token
from app.services.sessions import as_utc, utcnow

PASSWORD_RESET = TokenPurpose.PASSWORD_RESET
EMAIL_VERIFICATION = TokenPurpose.EMAIL_VERIFICATION


async def invalidate_pending_tokens(
    db: AsyncSession, user_id: str, purpose: Optional[TokenPurpose] = None
) -> None:
    stmt = (
        update(AuthToken)
        .where(AuthToken.user_id == user_id, AuthToken.used_at.is_(None))
        .values(used_at=utcnow())
        .execution_options(synchronize_session=False)
    )
    if purpose is not None:
        stmt = stmt.where(AuthToken.purpose == purpose)
    await db.execute(stmt)
    await db.commit()


async def create_one_time_token(
    db: AsyncSession, user_id: str, purpose: TokenPurpose, lifetime: timedelta
) -> str:
    """Invalida los tokens pendientes del mismo tipo y crea uno nuevo."""
    await invalidate_pending_tokens(db, user_id, purpose)
    raw = generate_opaque_token()
    db.add(AuthToken(
        user_id=user_id,
        token_hash=hash_token(raw),
        purpose=purpose,
        expires_at=utcnow() + lifetime,
    ))
    await db.commit()
    return raw


async def recently_issued(db: AsyncSession, user_id: str, purpose: TokenPurpose, seconds: int) -> bool:
    """True si ya se emitió un token de este tipo hace menos de `seconds` segundos."""
    if seconds <= 0:
        return False
    since = utcnow() - timedelta(seconds=seconds)
    count = await db.scalar(
        select(func.count()).select_from(AuthToken).where(
            AuthToken.user_id == user_id,
            AuthToken.purpose == purpose,
            AuthToken.created_at > since,
        )
    )
    return bool(count)


async def find_valid_token(db: AsyncSession, raw: str, purpose: TokenPurpose) -> Optional[AuthToken]:
    """Devuelve el token (con su usuario) si existe, es del tipo indicado, no se usó y no expiró."""
    if not raw:
        return None
    token = await db.scalar(
        select(AuthToken)
        .where(AuthToken.token_hash == hash_token(raw))
        .execution_options(populate_existing=True)
    )
    if not token or token.purpose != purpose or token.used_at is not None:
        return None
    if as_utc(token.expires_at) <= utcnow():
        return None
    return token


async def consume_one_time_token(db: AsyncSession, raw: str, purpose: TokenPurpose) -> Optional[str]:
    """
    Marca el token como usado si es válido. Devuelve el user_id o None.
    La actualización es condicional, así que un token no puede usarse dos veces
    aunque lleguen dos peticiones simultáneas.
    """
    token = await find_valid_token(db, raw, purpose)
    if token is None:
        return None
    result = await db.execute(
        update(AuthToken)
        .where(AuthToken.id == token.id, AuthToken.used_at.is_(None))
        .values(used_at=utcnow())
        .execution_options(synchronize_session=False)
    )
    await db.commit()
    return token.user_id if result.rowcount == 1 else None
