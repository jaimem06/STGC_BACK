"""
Tokens de un solo uso (recuperación de contraseña, invitación, verificación de email).
El token en claro solo viaja en el correo; en la base de datos se guarda su hash.
"""

from datetime import timedelta
from typing import Optional

from prisma import Prisma

from app.security import generate_opaque_token, hash_token
from app.services.sessions import as_utc, utcnow

PASSWORD_RESET = "PASSWORD_RESET"
EMAIL_VERIFICATION = "EMAIL_VERIFICATION"


async def create_one_time_token(
    db: Prisma, user_id: str, purpose: str, lifetime: timedelta
) -> str:
    """Invalida los tokens pendientes del mismo tipo y crea uno nuevo."""
    now = utcnow()
    await db.authtoken.update_many(
        where={"user_id": user_id, "purpose": purpose, "used_at": None},
        data={"used_at": now},
    )
    raw = generate_opaque_token()
    await db.authtoken.create(
        data={
            "user_id": user_id,
            "token_hash": hash_token(raw),
            "purpose": purpose,
            "expires_at": now + lifetime,
        }
    )
    return raw


async def recently_issued(db: Prisma, user_id: str, purpose: str, seconds: int) -> bool:
    """True si ya se emitió un token de este tipo hace menos de `seconds` segundos."""
    if seconds <= 0:
        return False
    since = utcnow() - timedelta(seconds=seconds)
    count = await db.authtoken.count(
        where={"user_id": user_id, "purpose": purpose, "created_at": {"gt": since}}
    )
    return count > 0


async def find_valid_token(db: Prisma, raw: str, purpose: str):
    """Devuelve el token (con su usuario) si existe, es del tipo indicado, no se usó y no expiró."""
    if not raw:
        return None
    token = await db.authtoken.find_unique(
        where={"token_hash": hash_token(raw)}, include={"user": True}
    )
    if not token or token.purpose != purpose or token.used_at is not None:
        return None
    if as_utc(token.expires_at) <= utcnow():
        return None
    return token


async def consume_one_time_token(db: Prisma, raw: str, purpose: str) -> Optional[str]:
    """
    Marca el token como usado si es válido. Devuelve el user_id o None.
    La actualización es condicional, así que un token no puede usarse dos veces
    aunque lleguen dos peticiones simultáneas.
    """
    token = await find_valid_token(db, raw, purpose)
    if token is None:
        return None
    consumed = await db.authtoken.update_many(
        where={"id": token.id, "used_at": None}, data={"used_at": utcnow()}
    )
    return token.user_id if consumed == 1 else None
