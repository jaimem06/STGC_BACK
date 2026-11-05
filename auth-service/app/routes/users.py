import logging
from typing import Annotated, List, Optional

from fastapi import APIRouter, Depends, HTTPException, Query, Request, Response, status
from prisma import Prisma
from prisma.models import User

from app.config import settings
from app.core import endpoints
from app.database import get_db
from app.dependencies import (
    ensure_can_assign_role,
    ensure_can_manage_target,
    get_active_suspension_until,
    require_admin,
    require_manage_users,
)
from app.schemas.user import UserOut, UserStatus, UserSuspend, UserUpdate
from app.security import get_password_hash
from app.services.audit import record_audit
from app.services.sessions import revoke_user_sessions, utcnow
from app.services.users import (
    ensure_not_last_admin,
    ensure_password_policy,
    ensure_unique_fields,
    get_role_or_400,
)

logger = logging.getLogger(__name__)
router = APIRouter(prefix=endpoints.USERS_PREFIX, tags=["Gestión de Usuarios"])

Db = Annotated[Prisma, Depends(get_db)]
Manager = Annotated[User, Depends(require_manage_users)]


async def _get_user_or_404(db: Prisma, user_id: str) -> User:
    user = await db.user.find_unique(where={"id": user_id}, include={"role": True})
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
    where: dict = {}
    if search:
        term = {"contains": search, "mode": "insensitive"}
        where["OR"] = [
            {"email": term}, {"first_name": term}, {"last_name": term}, {"identifier": term},
        ]
    if status_filter:
        where["status"] = status_filter.value
    if role:
        where["role"] = {"is": {"name": role}}

    total = await db.user.count(where=where)
    response.headers["X-Total-Count"] = str(total)
    return await db.user.find_many(
        where=where, include={"role": True}, order={"email": "asc"}, skip=skip, take=limit
    )


@router.get(endpoints.USERS_DETAIL, response_model=UserOut, summary="Obtener usuario")
async def get_user(user_id: str, db: Db, _: Manager):
    return await _get_user_or_404(db, user_id)


@router.patch(
    endpoints.USERS_DETAIL,
    response_model=UserOut,
    summary="Actualizar usuario",
    description=(
        "Modifica datos, rol, estado o contraseña. Un gestor no administrador no puede tocar "
        "usuarios con roles privilegiados ni otorgar esos roles. Cambiar contraseña o desactivar "
        "la cuenta cierra todas sus sesiones."
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

    update: dict = {}
    revoke_sessions = False

    if data.role_name and data.role_name != target.role.name:
        ensure_can_assign_role(actor, data.role_name)
        role = await get_role_or_400(db, data.role_name)
        if data.role_name != settings.admin_role_name:
            await ensure_not_last_admin(db, target)
        update["role_id"] = role.id
        # Los JWT emitidos llevan el rol antiguo: se cierran para que otros servicios no lo sigan aceptando.
        revoke_sessions = True

    if data.status and data.status.value != target.status:
        if data.status != UserStatus.ACTIVO:
            await ensure_not_last_admin(db, target)
            revoke_sessions = True
        update["status"] = data.status.value

    if data.email:
        email = data.email.lower()
        if email != target.email:
            await ensure_unique_fields(db, email=email, exclude_user_id=user_id)
            update["email"] = email
            update["email_verified"] = False

    if data.identifier:
        await ensure_unique_fields(db, identifier=data.identifier, exclude_user_id=user_id)
        update["identifier"] = data.identifier
    if data.phone_number:
        await ensure_unique_fields(db, phone_number=data.phone_number, exclude_user_id=user_id)
        update["phone_number"] = data.phone_number
    for field in ("first_name", "last_name"):
        if field in payload:
            update[field] = payload[field]
    if data.email_verified is not None:
        update["email_verified"] = data.email_verified

    if data.password:
        ensure_password_policy(data.password, update.get("email", target.email))
        update["password_hash"] = get_password_hash(data.password)
        update["password_changed_at"] = utcnow()
        revoke_sessions = True

    if not update:
        return target

    user = await db.user.update(where={"id": user_id}, data=update, include={"role": True})
    if "email" in update:
        # Los enlaces enviados al email anterior no deben servir para la nueva dirección.
        await db.authtoken.update_many(
            where={"user_id": user_id, "used_at": None}, data={"used_at": utcnow()}
        )
    if revoke_sessions:
        await revoke_user_sessions(db, user_id)

    changed = sorted(k for k in update if k != "password_hash")
    await record_audit(db, "user_updated", user_id=actor.id, request=request,
                       details={"target_user_id": user_id, "fields": changed})
    return user


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

    user = await db.user.update(
        where={"id": user_id},
        data={"suspended_from": data.suspended_from, "suspended_until": data.suspended_until},
        include={"role": True},
    )
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
    user = await db.user.update(
        where={"id": user_id}, data={"failed_login_attempts": 0, "locked_until": None},
        include={"role": True},
    )
    await record_audit(db, "user_unlocked", user_id=actor.id, request=request,
                       details={"target_user_id": user_id})
    return user


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
    user = await db.user.update(
        where={"id": user_id},
        data={"mfa_enabled": False, "mfa_secret": None, "mfa_last_used_step": None},
        include={"role": True},
    )
    await revoke_user_sessions(db, user_id)
    await record_audit(db, "user_mfa_reset", user_id=actor.id, request=request,
                       details={"target_user_id": user_id})
    return user


@router.delete(endpoints.USERS_DETAIL, status_code=status.HTTP_204_NO_CONTENT,
               summary="Eliminar usuario (solo admin)",
               description="Borrado definitivo. Su historial de auditoría se conserva sin referencia al usuario.")
async def delete_user(
    user_id: str, request: Request, db: Db, actor: Annotated[User, Depends(require_admin)]
):
    target = await _get_user_or_404(db, user_id)
    _forbid_self(actor, target, "eliminar")
    await ensure_not_last_admin(db, target)
    await db.user.delete(where={"id": user_id})
    await record_audit(db, "user_deleted", user_id=actor.id, request=request,
                       details={"target_user_id": user_id, "email": target.email})
