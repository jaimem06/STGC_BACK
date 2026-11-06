from datetime import timedelta
from typing import Optional

from fastapi import HTTPException, status
from sqlalchemy import func, or_, select, update
from sqlalchemy.ext.asyncio import AsyncSession
from starlette.requests import Request

from app.config import settings
from app.models import Role, User, UserStatus
from app.security import password_policy_errors
from app.services.audit import record_audit
from app.services.sessions import as_utc, utcnow


def ensure_password_policy(password: str, email: Optional[str] = None) -> None:
    errors = password_policy_errors(password, email)
    if errors:
        raise HTTPException(
            status_code=status.HTTP_422_UNPROCESSABLE_CONTENT,
            detail={"message": "La contraseña no cumple la política de seguridad", "errors": errors},
        )


async def get_user(db: AsyncSession, user_id: str) -> Optional[User]:
    """Usuario con su rol, siempre leído fresco de la base de datos."""
    return await db.scalar(
        select(User).where(User.id == user_id).execution_options(populate_existing=True)
    )


async def get_user_by_email(db: AsyncSession, email: str) -> Optional[User]:
    return await db.scalar(
        select(User).where(User.email == email).execution_options(populate_existing=True)
    )


async def ensure_unique_fields(
    db: AsyncSession,
    *,
    email: Optional[str] = None,
    identifier: Optional[str] = None,
    phone_number: Optional[str] = None,
    exclude_user_id: Optional[str] = None,
) -> None:
    checks = [
        (User.email, email, "El email ya está registrado"),
        (User.identifier, identifier, "El identificador ya está en uso"),
        (User.phone_number, phone_number, "El número de teléfono ya está registrado"),
    ]
    for column, value, message in checks:
        if not value:
            continue
        stmt = select(User.id).where(column == value)
        if exclude_user_id:
            stmt = stmt.where(User.id != exclude_user_id)
        if await db.scalar(stmt.limit(1)):
            raise HTTPException(status_code=status.HTTP_409_CONFLICT, detail=message)


async def get_role_by_name(db: AsyncSession, role_name: str) -> Optional[Role]:
    return await db.scalar(select(Role).where(Role.name == role_name))


async def get_role_or_400(db: AsyncSession, role_name: str) -> Role:
    role = await get_role_by_name(db, role_name)
    if not role:
        raise HTTPException(status_code=400, detail=f"El rol '{role_name}' no existe")
    return role


async def ensure_not_last_admin(db: AsyncSession, target: User) -> None:
    """Impide dejar el sistema sin ningún administrador activo."""
    if target.role.name != settings.admin_role_name:
        return
    others = await db.scalar(
        select(func.count()).select_from(User).join(User.role).where(
            Role.name == settings.admin_role_name,
            User.status == UserStatus.ACTIVO,
            User.id != target.id,
        )
    )
    if not others:
        raise HTTPException(
            status_code=400,
            detail="Operación no permitida: es el último administrador activo",
        )


async def update_user_fields(db: AsyncSession, user_id: str, **values) -> None:
    await db.execute(
        update(User).where(User.id == user_id).values(**values)
        .execution_options(synchronize_session=False)
    )
    await db.commit()


# ---------------------------------------------------------------------------
# Bloqueo por intentos fallidos
# ---------------------------------------------------------------------------

def ensure_not_locked(user: User) -> None:
    locked_until = as_utc(user.locked_until)
    if locked_until and locked_until > utcnow():
        minutes = max(1, int((locked_until - utcnow()).total_seconds() // 60) + 1)
        raise HTTPException(
            status_code=status.HTTP_423_LOCKED,
            detail=f"Cuenta bloqueada temporalmente por intentos fallidos. Inténtalo en {minutes} minuto(s).",
        )


async def register_failed_attempt(db: AsyncSession, user: User, request: Request, reason: str) -> None:
    """Suma un intento fallido (atómico) y bloquea la cuenta al alcanzar el límite."""
    attempts = await db.scalar(
        update(User)
        .where(User.id == user.id)
        .values(failed_login_attempts=User.failed_login_attempts + 1)
        .returning(User.failed_login_attempts)
        .execution_options(synchronize_session=False)
    )
    await db.commit()
    await record_audit(db, "login_failed", user_id=user.id, request=request, details={"reason": reason})
    if attempts is not None and attempts >= settings.max_failed_login_attempts:
        await update_user_fields(
            db, user.id,
            failed_login_attempts=0,
            locked_until=utcnow() + timedelta(minutes=settings.lockout_minutes),
        )
        await record_audit(
            db, "account_locked", user_id=user.id, request=request,
            details={"minutes": settings.lockout_minutes},
        )


async def claim_totp_step(db: AsyncSession, user_id: str, step: int) -> bool:
    """
    Registra el step TOTP como usado de forma atómica. Si dos peticiones presentan
    el mismo código a la vez, solo una lo consigue.
    """
    result = await db.execute(
        update(User)
        .where(
            User.id == user_id,
            or_(User.mfa_last_used_step.is_(None), User.mfa_last_used_step < step),
        )
        .values(mfa_last_used_step=step)
        .execution_options(synchronize_session=False)
    )
    await db.commit()
    return result.rowcount == 1


async def clear_failed_attempts(db: AsyncSession, user: User, *, mark_login: bool = False) -> None:
    values: dict = {"failed_login_attempts": 0, "locked_until": None}
    if mark_login:
        values["last_login_at"] = utcnow()
    await update_user_fields(db, user.id, **values)
