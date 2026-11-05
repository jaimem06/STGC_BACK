from typing import Annotated, List

from fastapi import APIRouter, BackgroundTasks, Depends, HTTPException, Request, status
from prisma import Prisma

from app.config import settings
from app.core import endpoints
from app.database import get_db
from app.dependencies import AuthContext, get_auth_context
from app.limiter import limiter
from app.schemas.auth import (
    ChangePasswordRequest,
    MessageResponse,
    MfaCodeRequest,
    MfaDisableRequest,
    MfaSetupResponse,
    PasswordConfirm,
    SessionOut,
)
from app.schemas.user import ProfileUpdate, UserOut
from app.security import (
    decrypt_secret,
    encrypt_secret,
    generate_totp_secret,
    get_password_hash,
    totp_provisioning_uri,
    verify_password,
    verify_totp,
)
from app.services.audit import record_audit
from app.services.sessions import revoke_session, revoke_user_sessions, session_is_active, utcnow
from app.services.users import claim_totp_step, ensure_password_policy, ensure_unique_fields
from app.utils import email as mailer

router = APIRouter(prefix=endpoints.AUTH_PREFIX, tags=["Mi cuenta"])

Ctx = Annotated[AuthContext, Depends(get_auth_context)]
Db = Annotated[Prisma, Depends(get_db)]


@router.get(endpoints.ACCOUNT_ME, response_model=UserOut, summary="Obtener mi perfil")
async def get_me(ctx: Ctx):
    return ctx.user


@router.patch(
    endpoints.ACCOUNT_ME,
    response_model=UserOut,
    summary="Actualizar mi perfil",
    description="Permite cambiar nombre, apellido y teléfono. El email y el rol los gestiona un administrador.",
)
async def update_me(request: Request, data: ProfileUpdate, ctx: Ctx, db: Db):
    update = data.model_dump(exclude_unset=True)
    if not update:
        raise HTTPException(status_code=400, detail="No se proporcionaron datos para actualizar")
    if update.get("phone_number"):
        await ensure_unique_fields(db, phone_number=update["phone_number"], exclude_user_id=ctx.user.id)
    user = await db.user.update(where={"id": ctx.user.id}, data=update, include={"role": True})
    await record_audit(db, "profile_updated", user_id=ctx.user.id, request=request,
                       details={"fields": sorted(update)})
    return user


@router.post(
    endpoints.ACCOUNT_CHANGE_PASSWORD,
    response_model=MessageResponse,
    summary="Cambiar mi contraseña",
    description="Requiere la contraseña actual. Cierra las demás sesiones y conserva la actual.",
)
@limiter.limit("5/minute")
async def change_password(
    request: Request, data: ChangePasswordRequest, background_tasks: BackgroundTasks, ctx: Ctx, db: Db
):
    user = ctx.user
    if not verify_password(data.current_password, user.password_hash):
        await record_audit(db, "password_change_failed", user_id=user.id, request=request)
        raise HTTPException(status_code=400, detail="La contraseña actual no es correcta")
    if verify_password(data.new_password, user.password_hash):
        raise HTTPException(status_code=400, detail="La nueva contraseña debe ser distinta de la actual")
    ensure_password_policy(data.new_password, user.email)

    await db.user.update(
        where={"id": user.id},
        data={"password_hash": get_password_hash(data.new_password), "password_changed_at": utcnow()},
    )
    await revoke_user_sessions(db, user.id, except_session_id=ctx.session.id)
    await record_audit(db, "password_changed", user_id=user.id, request=request)
    background_tasks.add_task(mailer.send_password_changed_email, user.email)
    return MessageResponse(message="Contraseña actualizada correctamente")


# ---------------------------------------------------------------------------
# Sesiones
# ---------------------------------------------------------------------------

@router.get(endpoints.ACCOUNT_SESSIONS, response_model=List[SessionOut], summary="Listar mis sesiones activas")
async def list_sessions(ctx: Ctx, db: Db):
    sessions = await db.usersession.find_many(
        where={"user_id": ctx.user.id, "revoked_at": None, "expires_at": {"gt": utcnow()}},
        order={"last_used_at": "desc"},
    )
    return [
        SessionOut(
            id=s.id, user_agent=s.user_agent, ip_address=s.ip_address, created_at=s.created_at,
            last_used_at=s.last_used_at, expires_at=s.expires_at, current=s.id == ctx.session.id,
        )
        for s in sessions
    ]


@router.delete(
    endpoints.ACCOUNT_SESSION_DETAIL,
    status_code=status.HTTP_204_NO_CONTENT,
    summary="Cerrar una de mis sesiones",
)
async def revoke_my_session(session_id: str, request: Request, ctx: Ctx, db: Db):
    session = await db.usersession.find_unique(where={"id": session_id})
    if session is None or session.user_id != ctx.user.id or not session_is_active(session):
        raise HTTPException(status_code=404, detail="Sesión no encontrada")
    await revoke_session(db, session_id)
    await record_audit(db, "session_revoked", user_id=ctx.user.id, request=request,
                       details={"session_id": session_id})


# ---------------------------------------------------------------------------
# Verificación en dos pasos (TOTP)
# ---------------------------------------------------------------------------

@router.post(
    endpoints.ACCOUNT_MFA_SETUP,
    response_model=MfaSetupResponse,
    summary="Iniciar configuración MFA",
    description=(
        "Requiere la contraseña. Devuelve el secreto y la URI `otpauth://` para generar el QR. "
        "La MFA no se activa hasta confirmar un código en `/auth/mfa/enable`."
    ),
)
@limiter.limit("5/minute")
async def mfa_setup(request: Request, data: PasswordConfirm, ctx: Ctx, db: Db):
    user = ctx.user
    if user.mfa_enabled:
        raise HTTPException(status_code=400, detail="La verificación en dos pasos ya está activa")
    if not verify_password(data.password, user.password_hash):
        raise HTTPException(status_code=400, detail="Contraseña incorrecta")

    secret = generate_totp_secret()
    await db.user.update(
        where={"id": user.id},
        data={"mfa_secret": encrypt_secret(secret), "mfa_last_used_step": None},
    )
    return MfaSetupResponse(
        secret=secret,
        otpauth_uri=totp_provisioning_uri(secret, user.email, settings.app_display_name),
    )


@router.post(endpoints.ACCOUNT_MFA_ENABLE, response_model=MessageResponse, summary="Activar MFA")
@limiter.limit("10/minute")
async def mfa_enable(
    request: Request, data: MfaCodeRequest, background_tasks: BackgroundTasks, ctx: Ctx, db: Db
):
    user = ctx.user
    if user.mfa_enabled:
        raise HTTPException(status_code=400, detail="La verificación en dos pasos ya está activa")
    secret = decrypt_secret(user.mfa_secret) if user.mfa_secret else None
    if not secret:
        raise HTTPException(status_code=400, detail="Primero inicia la configuración en /auth/mfa/setup")
    step = verify_totp(secret, data.code)
    if step is None:
        raise HTTPException(status_code=400, detail="Código de verificación inválido")

    await db.user.update(where={"id": user.id}, data={"mfa_enabled": True, "mfa_last_used_step": step})
    await revoke_user_sessions(db, user.id, except_session_id=ctx.session.id)
    await record_audit(db, "mfa_enabled", user_id=user.id, request=request)
    background_tasks.add_task(mailer.send_mfa_changed_email, user.email, True)
    return MessageResponse(message="Verificación en dos pasos activada")


@router.post(
    endpoints.ACCOUNT_MFA_DISABLE,
    response_model=MessageResponse,
    summary="Desactivar MFA",
    description="Requiere la contraseña y un código válido de la app autenticadora.",
)
@limiter.limit("5/minute")
async def mfa_disable(
    request: Request, data: MfaDisableRequest, background_tasks: BackgroundTasks, ctx: Ctx, db: Db
):
    user = ctx.user
    if not user.mfa_enabled or not user.mfa_secret:
        raise HTTPException(status_code=400, detail="La verificación en dos pasos no está activa")
    if not verify_password(data.password, user.password_hash):
        raise HTTPException(status_code=400, detail="Contraseña incorrecta")
    secret = decrypt_secret(user.mfa_secret)
    step = verify_totp(secret, data.code, user.mfa_last_used_step) if secret else None
    if step is None or not await claim_totp_step(db, user.id, step):
        raise HTTPException(status_code=400, detail="Código de verificación inválido")

    await db.user.update(
        where={"id": user.id},
        data={"mfa_enabled": False, "mfa_secret": None, "mfa_last_used_step": None},
    )
    await record_audit(db, "mfa_disabled", user_id=user.id, request=request)
    background_tasks.add_task(mailer.send_mfa_changed_email, user.email, False)
    return MessageResponse(message="Verificación en dos pasos desactivada")
