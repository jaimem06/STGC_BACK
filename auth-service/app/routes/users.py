import logging
from typing import Annotated, List, Optional

from fastapi import APIRouter, Depends, HTTPException, Query, Request, Response, status
from sqlalchemy import delete, func, or_, select, update
from sqlalchemy.ext.asyncio import AsyncSession

from app.config import settings
from app.core import endpoints
from app.dependencies import (
    Db,
    ensure_can_assign_role,
    ensure_can_manage_target,
    get_active_suspension_until,
    require_admin,
    require_manage_users,
)
from app.models import AuthToken, Role, User, UserStatus
from app.schemas.user import UserOut, UserSuspend, UserUpdate
from app.security import get_password_hash
from app.services.audit import record_audit
from app.services.sessions import revoke_user_sessions, utcnow
from app.services.users import (
    ensure_not_last_admin,
    ensure_password_policy,
    ensure_unique_fields,
    get_role_or_400,
    get_user,
    update_user_fields,
)

logger = logging.getLogger(__name__)
router = APIRouter(prefix=endpoints.USERS_PREFIX, tags=["Gestión de Usuarios"])

Manager = Annotated[User, Depends(require_manage_users)]


async def _get_user_or_404(db: AsyncSession, user_id: str) -> User:
    user = await get_user(db, user_id)
    if not user:
        raise HTTPException(status_code=404, detail="Usuario no encontrado")
    return user


def _forbid_self(actor: User, target: User, action: str) -> None:
    if actor.id == target.id:
        raise HTTPException(status_code=400, detail=f"No puedes {action} tu propia cuenta")


@router.get(
    endpoints.USERS_LIST,
    response_model=List[UserOut],
    summary="Listar usuarios",
    description="Lista paginada con filtros. El total se devuelve en la cabecera `X-Total-Count`.",
)
async def list_users(
    response: Response,
    db: Db,
    _: Manager,
    skip: Annotated[int, Query(ge=0)] = 0,
    limit: Annotated[int, Query(ge=1, le=200)] = 50,
    search: Annotated[Optional[str], Query(max_length=100)] = None,
    status_filter: Annotated[Optional[UserStatus], Query(alias="status")] = None,
    role: Annotated[Optional[str], Query(max_length=50)] = None,
):
    conditions = []
    if search:
        conditions.append(or_(
            *(column.icontains(search, autoescape=True)
              for column in (User.email, User.first_name, User.last_name, User.identifier))
        ))
    if status_filter:
        conditions.append(User.status == status_filter)
    if role:
        conditions.append(User.role_id.in_(select(Role.id).where(Role.name == role)))

    total = await db.scalar(select(func.count()).select_from(User).where(*conditions))
    response.headers["X-Total-Count"] = str(total)
    users = await db.scalars(
        select(User).where(*conditions).order_by(User.email).offset(skip).limit(limit)
    )
    return list(users)


@router.get(endpoints.USERS_DETAIL, response_model=UserOut, summary="Obtener usuario")
async def get_user_detail(user_id: str, db: Db, _: Manager):
    return await _get_user_or_404(db, user_id)


@router.patch(
    endpoints.USERS_DETAIL,
    response_model=UserOut,
    summary="Actualizar usuario",
    description=(
        "Modifica datos, rol, estado o contraseña. Un gestor no administrador no puede tocar "
        "usuarios con roles privilegiados ni otorgar esos roles. Cambiar rol, contraseña o "
        "desactivar la cuenta cierra todas sus sesiones."
    ),
)
async def update_user(user_id: str, data: UserUpdate, request: Request, db: Db, actor: Manager):
    target = await _get_user_or_404(db, user_id)
    ensure_can_manage_target(actor, target)
    payload = data.model_dump(exclude_unset=True)
    if not payload:
        raise HTTPException(status_code=400, detail="No se proporcionaron datos para actualizar")

    if actor.id == target.id and ({"role_name", "status", "email_verified"} & payload.keys()):
        raise HTTPException(status_code=400, detail="No puedes cambiar tu propio rol, estado o verificación")

    values: dict = {}
    revoke_sessions = False

    if data.role_name and data.role_name != target.role.name:
        ensure_can_assign_role(actor, data.role_name)
        role = await get_role_or_400(db, data.role_name)
        if data.role_name != settings.admin_role_name:
            await ensure_not_last_admin(db, target)
        values["role_id"] = role.id
        # Los JWT emitidos llevan el rol antiguo: se cierran para que otros servicios no lo sigan aceptando.
        revoke_sessions = True

    if data.status and data.status != target.status:
        if data.status != UserStatus.ACTIVO:
            await ensure_not_last_admin(db, target)
            revoke_sessions = True
        values["status"] = data.status

    if data.email:
        email = data.email.lower()
        if email != target.email:
            await ensure_unique_fields(db, email=email, exclude_user_id=user_id)
            values["email"] = email
            values["email_verified"] = False

    if data.identifier:
        await ensure_unique_fields(db, identifier=data.identifier, exclude_user_id=user_id)
        values["identifier"] = data.identifier
    if data.phone_number:
        await ensure_unique_fields(db, phone_number=data.phone_number, exclude_user_id=user_id)
        values["phone_number"] = data.phone_number
    for field in ("first_name", "last_name"):
        if field in payload:
            values[field] = payload[field]
    if data.email_verified is not None:
        values["email_verified"] = data.email_verified

    if data.password:
        ensure_password_policy(data.password, values.get("email", target.email))
        values["password_hash"] = get_password_hash(data.password)
        values["password_changed_at"] = utcnow()
        revoke_sessions = True

    if not values:
        return target

    await update_user_fields(db, user_id, **values)
    if "email" in values:
        # Los enlaces enviados al email anterior no deben servir para la nueva dirección.
        await db.execute(
            update(AuthToken)
            .where(AuthToken.user_id == user_id, AuthToken.used_at.is_(None))
            .values(used_at=utcnow())
            .execution_options(synchronize_session=False)
        )
        await db.commit()
    if revoke_sessions:
        await revoke_user_sessions(db, user_id)

    changed = sorted(k for k in values if k != "password_hash")
    await record_audit(db, "user_updated", user_id=actor.id, request=request,
                       details={"target_user_id": user_id, "fields": changed})
    return await get_user(db, user_id)


@router.patch(
    endpoints.USERS_SUSPEND,
    response_model=UserOut,
    summary="Suspender usuario",
    description=(
        "Define un periodo de suspensión. Durante ese rango el usuario no puede entrar y sus "
        "tokens dejan de funcionar. Enviar ambos campos en null levanta la suspensión."
    ),
)
async def suspend_user(user_id: str, data: UserSuspend, request: Request, db: Db, actor: Manager):
    target = await _get_user_or_404(db, user_id)
    ensure_can_manage_target(actor, target)
    if data.suspended_from:
        _forbid_self(actor, target, "suspender")
        await ensure_not_last_admin(db, target)

    await update_user_fields(
        db, user_id, suspended_from=data.suspended_from, suspended_until=data.suspended_until
    )
    user = await get_user(db, user_id)
    if get_active_suspension_until(user):
        await revoke_user_sessions(db, user_id)
    action = "user_suspended" if data.suspended_from else "user_unsuspended"
    await record_audit(db, action, user_id=actor.id, request=request, details={
        "target_user_id": user_id,
        "from": data.suspended_from, "until": data.suspended_until,
    })
    return user


@router.post(endpoints.USERS_UNLOCK, response_model=UserOut, summary="Desbloquear usuario",
             description="Quita el bloqueo temporal por intentos fallidos de login.")
async def unlock_user(user_id: str, request: Request, db: Db, actor: Manager):
    target = await _get_user_or_404(db, user_id)
    ensure_can_manage_target(actor, target)
    await update_user_fields(db, user_id, failed_login_attempts=0, locked_until=None)
    await record_audit(db, "user_unlocked", user_id=actor.id, request=request,
                       details={"target_user_id": user_id})
    return await get_user(db, user_id)


@router.post(endpoints.USERS_REVOKE_SESSIONS, status_code=status.HTTP_204_NO_CONTENT,
             summary="Cerrar todas las sesiones de un usuario")
async def revoke_sessions(user_id: str, request: Request, db: Db, actor: Manager):
    target = await _get_user_or_404(db, user_id)
    ensure_can_manage_target(actor, target)
    await revoke_user_sessions(db, user_id)
    await record_audit(db, "user_sessions_revoked", user_id=actor.id, request=request,
                       details={"target_user_id": user_id})


@router.delete(endpoints.USERS_RESET_MFA, response_model=UserOut, summary="Restablecer MFA de un usuario",
               description="Desactiva la verificación en dos pasos (p. ej. si el usuario perdió su teléfono).")
async def reset_user_mfa(user_id: str, request: Request, db: Db, actor: Manager):
    target = await _get_user_or_404(db, user_id)
    ensure_can_manage_target(actor, target)
    await update_user_fields(db, user_id, mfa_enabled=False, mfa_secret=None, mfa_last_used_step=None)
    await revoke_user_sessions(db, user_id)
    await record_audit(db, "user_mfa_reset", user_id=actor.id, request=request,
                       details={"target_user_id": user_id})
    return await get_user(db, user_id)


@router.delete(endpoints.USERS_DETAIL, status_code=status.HTTP_204_NO_CONTENT,
               summary="Eliminar usuario (solo admin)",
               description="Borrado definitivo. Su historial de auditoría se conserva sin referencia al usuario.")
async def delete_user(
    user_id: str, request: Request, db: Db, actor: Annotated[User, Depends(require_admin)]
):
    target = await _get_user_or_404(db, user_id)
    _forbid_self(actor, target, "eliminar")
    await ensure_not_last_admin(db, target)
    email = target.email
    # Las sesiones y tokens se borran en cascada; la auditoría queda con user_id en NULL.
    await db.execute(delete(User).where(User.id == user_id).execution_options(synchronize_session=False))
    await db.commit()
    db.expunge_all()
    await record_audit(db, "user_deleted", user_id=actor.id, request=request,
                       details={"target_user_id": user_id, "email": email})
