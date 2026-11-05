from datetime import timedelta
from typing import Optional

from fastapi import HTTPException, status
from prisma import Prisma
from prisma.models import User
from starlette.requests import Request

from app.config import settings
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


async def ensure_unique_fields(
    db: Prisma,
    *,
    email: Optional[str] = None,
    identifier: Optional[str] = None,
    phone_number: Optional[str] = None,
    exclude_user_id: Optional[str] = None,
) -> None:
    checks = [
        ("email", email, "El email ya está registrado"),
        ("identifier", identifier, "El identificador ya está en uso"),
        ("phone_number", phone_number, "El número de teléfono ya está registrado"),
    ]
    for field, value, message in checks:
        if not value:
            continue
        where: dict = {field: value}
        if exclude_user_id:
            where["NOT"] = {"id": exclude_user_id}
        if await db.user.find_first(where=where):
            raise HTTPException(status_code=status.HTTP_409_CONFLICT, detail=message)


async def get_role_or_400(db: Prisma, role_name: str):
    role = await db.role.find_unique(where={"name": role_name})
    if not role:
        raise HTTPException(status_code=400, detail=f"El rol '{role_name}' no existe")
    return role


async def ensure_not_last_admin(db: Prisma, target: User) -> None:
    """Impide dejar el sistema sin ningún administrador activo."""
    if target.role.name != settings.admin_role_name:
        return
    others = await db.user.count(
        where={
            "role": {"is": {"name": settings.admin_role_name}},
            "status": "ACTIVO",
            "NOT": {"id": target.id},
        }
    )
    if others == 0:
        raise HTTPException(
            status_code=400,
            detail="Operación no permitida: es el último administrador activo",
        )


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


async def register_failed_attempt(db: Prisma, user: User, request: Request, reason: str) -> None:
    """Suma un intento fallido (atómico) y bloquea la cuenta al alcanzar el límite."""
    updated = await db.user.update(
        where={"id": user.id}, data={"failed_login_attempts": {"increment": 1}}
    )
    await record_audit(db, "login_failed", user_id=user.id, request=request, details={"reason": reason})
    if updated and updated.failed_login_attempts >= settings.max_failed_login_attempts:
        await db.user.update(
            where={"id": user.id},
            data={
                "failed_login_attempts": 0,
                "locked_until": utcnow() + timedelta(minutes=settings.lockout_minutes),
            },
        )
        await record_audit(
            db, "account_locked", user_id=user.id, request=request,
            details={"minutes": settings.lockout_minutes},
        )


async def claim_totp_step(db: Prisma, user_id: str, step: int) -> bool:
    """
    Registra el step TOTP como usado de forma atómica. Si dos peticiones presentan
    el mismo código a la vez, solo una lo consigue.
    """
    claimed = await db.user.update_many(
        where={
            "id": user_id,
            "OR": [{"mfa_last_used_step": None}, {"mfa_last_used_step": {"lt": step}}],
        },
        data={"mfa_last_used_step": step},
    )
    return claimed == 1


async def clear_failed_attempts(db: Prisma, user: User, *, mark_login: bool = False) -> None:
    data: dict = {"failed_login_attempts": 0, "locked_until": None}
    if mark_login:
        data["last_login_at"] = utcnow()
    await db.user.update(where={"id": user.id}, data=data)
