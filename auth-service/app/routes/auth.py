import logging
from datetime import timedelta
from typing import Annotated, Union

from fastapi import APIRouter, BackgroundTasks, Depends, HTTPException, Request, status
from prisma import Prisma
from prisma.models import User

from app.config import settings
from app.core import endpoints
from app.database import get_db
from app.dependencies import (
    AuthContext,
    ensure_can_assign_role,
    ensure_user_can_access,
    get_auth_context,
    require_manage_users,
)
from app.limiter import limiter
from app.schemas.auth import (
    EmailVerifyRequest,
    LoginRequest,
    MessageResponse,
    MfaChallenge,
    MfaVerifyRequest,
    PasswordResetConfirm,
    PasswordResetRequest,
    RefreshRequest,
    SignupResponse,
    TokenPair,
)
from app.schemas.user import UserCreate, UserOut, UserSignup
from app.security import (
    MFA_TOKEN_TYPE,
    burn_password_check,
    create_mfa_token,
    decode_token,
    decrypt_secret,
    get_password_hash,
    unusable_password_hash,
    verify_password,
    verify_totp,
)
from app.services import tokens as one_time
from app.services.audit import record_audit
from app.services.sessions import issue_session, revoke_session, revoke_user_sessions, rotate_refresh_token, utcnow
from app.services.users import (
    claim_totp_step,
    clear_failed_attempts,
    ensure_not_locked,
    ensure_password_policy,
    ensure_unique_fields,
    get_role_or_400,
    register_failed_attempt,
)
from app.utils import email as mailer

logger = logging.getLogger(__name__)
router = APIRouter(prefix=endpoints.AUTH_PREFIX, tags=["Autenticación"])

INVALID_CREDENTIALS = "Credenciales incorrectas"
GENERIC_EMAIL_MESSAGE = "Si el email está registrado, recibirás un correo con las instrucciones"


async def _complete_login(db: Prisma, user: User, request: Request) -> dict:
    await clear_failed_attempts(db, user, mark_login=True)
    tokens = await issue_session(db, user, request)
    await record_audit(db, "login_success", user_id=user.id, request=request)
    return tokens


# ---------------------------------------------------------------------------
# Login, MFA, refresh y logout
# ---------------------------------------------------------------------------

@router.post(
    endpoints.AUTH_LOGIN,
    response_model=Union[TokenPair, MfaChallenge],
    summary="Iniciar sesión",
    description=(
        "Valida email y contraseña. Devuelve access y refresh token, o un `mfa_token` si el "
        "usuario tiene verificación en dos pasos (completar con `/auth/mfa/verify`). "
        f"Tras {settings.max_failed_login_attempts} intentos fallidos la cuenta se bloquea "
        f"{settings.lockout_minutes} minutos."
    ),
    responses={401: {"description": INVALID_CREDENTIALS}, 403: {"description": "Cuenta no activa"},
               423: {"description": "Cuenta bloqueada temporalmente"}},
)
@limiter.limit("10/minute")
async def login(request: Request, data: LoginRequest, db: Annotated[Prisma, Depends(get_db)]):
    email = data.email.lower()
    user = await db.user.find_unique(where={"email": email}, include={"role": True})

    if user is None:
        burn_password_check(data.password)  # mismo tiempo de respuesta que con un usuario real
        await record_audit(db, "login_failed", request=request, details={"email": email, "reason": "unknown_user"})
        raise HTTPException(status_code=401, detail=INVALID_CREDENTIALS)

    if not verify_password(data.password, user.password_hash):
        await register_failed_attempt(db, user, request, "bad_password")
        # Misma respuesta esté o no bloqueada la cuenta: sin la contraseña no se revela nada.
        raise HTTPException(status_code=401, detail=INVALID_CREDENTIALS)

    # Bloqueo, estado y verificación solo se revelan con la contraseña correcta.
    ensure_not_locked(user)
    ensure_user_can_access(user)
    if settings.require_email_verification and not user.email_verified:
        raise HTTPException(status_code=403, detail="EMAIL_NOT_VERIFIED")

    if user.mfa_enabled:
        # El contador NO se reinicia aquí: los códigos MFA fallidos se acumulan
        # hasta completar el login, así repetir el paso de contraseña no da más intentos.
        return MfaChallenge(mfa_token=create_mfa_token(user.id))

    return await _complete_login(db, user, request)


@router.post(
    endpoints.AUTH_MFA_VERIFY,
    response_model=TokenPair,
    summary="Completar login con código MFA",
    description="Recibe el `mfa_token` del login y el código de 6 dígitos de la app autenticadora.",
)
@limiter.limit("10/minute")
async def mfa_verify(request: Request, data: MfaVerifyRequest, db: Annotated[Prisma, Depends(get_db)]):
    payload = decode_token(data.mfa_token, MFA_TOKEN_TYPE)
    if payload is None:
        raise HTTPException(status_code=401, detail="mfa_token inválido o expirado")

    user = await db.user.find_unique(where={"id": payload["sub"]}, include={"role": True})
    if user is None or not user.mfa_enabled or not user.mfa_secret:
        raise HTTPException(status_code=401, detail="mfa_token inválido o expirado")

    ensure_not_locked(user)
    ensure_user_can_access(user)

    secret = decrypt_secret(user.mfa_secret)
    step = verify_totp(secret, data.code, user.mfa_last_used_step) if secret else None
    if step is None or not await claim_totp_step(db, user.id, step):
        await register_failed_attempt(db, user, request, "bad_mfa_code")
        raise HTTPException(status_code=401, detail="Código de verificación inválido")

    return await _complete_login(db, user, request)


@router.post(
    endpoints.AUTH_REFRESH,
    response_model=TokenPair,
    summary="Renovar tokens",
    description=(
        "Entrega un access token nuevo y un refresh token nuevo. El refresh token anterior "
        "queda invalidado; si se reutiliza, la sesión se revoca por posible robo."
    ),
)
@limiter.limit("30/minute")
async def refresh(request: Request, data: RefreshRequest, db: Annotated[Prisma, Depends(get_db)]):
    return await rotate_refresh_token(db, data.refresh_token, request)


@router.post(endpoints.AUTH_LOGOUT, status_code=status.HTTP_204_NO_CONTENT, summary="Cerrar sesión actual")
async def logout(
    request: Request,
    ctx: Annotated[AuthContext, Depends(get_auth_context)],
    db: Annotated[Prisma, Depends(get_db)],
):
    await revoke_session(db, ctx.session.id)
    await record_audit(db, "logout", user_id=ctx.user.id, request=request)


@router.post(
    endpoints.AUTH_LOGOUT_ALL,
    status_code=status.HTTP_204_NO_CONTENT,
    summary="Cerrar todas las sesiones",
    description="Revoca todas las sesiones del usuario en todos los dispositivos, incluida la actual.",
)
async def logout_all(
    request: Request,
    ctx: Annotated[AuthContext, Depends(get_auth_context)],
    db: Annotated[Prisma, Depends(get_db)],
):
    await revoke_user_sessions(db, ctx.user.id)
    await record_audit(db, "logout_all", user_id=ctx.user.id, request=request)


# ---------------------------------------------------------------------------
# Alta de usuarios
# ---------------------------------------------------------------------------

@router.post(
    endpoints.AUTH_REGISTER,
    response_model=UserOut,
    status_code=status.HTTP_201_CREATED,
    summary="Registrar usuario (gestores)",
    description=(
        "Crea una cuenta. Si no se envía `password`, el usuario recibe un correo de invitación "
        "para definirla. Solo un administrador puede otorgar roles privilegiados."
    ),
)
@limiter.limit("20/minute")
async def register(
    request: Request,
    user_in: UserCreate,
    background_tasks: BackgroundTasks,
    db: Annotated[Prisma, Depends(get_db)],
    actor: Annotated[User, Depends(require_manage_users)],
):
    email = user_in.email.lower()
    role_name = user_in.role_name or settings.default_role_name
    ensure_can_assign_role(actor, role_name)
    await ensure_unique_fields(
        db, email=email, identifier=user_in.identifier, phone_number=user_in.phone_number
    )
    role = await get_role_or_400(db, role_name)

    if user_in.password:
        ensure_password_policy(user_in.password, email)
        password_hash = get_password_hash(user_in.password)
    else:
        password_hash = unusable_password_hash()

    user = await db.user.create(
        data={
            "email": email,
            "first_name": user_in.first_name,
            "last_name": user_in.last_name,
            "identifier": user_in.identifier,
            "phone_number": user_in.phone_number,
            "password_hash": password_hash,
            "role_id": role.id,
            "status": user_in.status.value,
            # Con contraseña definida por el gestor el email se da por verificado;
            # por invitación se verifica cuando el usuario usa el enlace.
            "email_verified": bool(user_in.password),
        },
        include={"role": True},
    )

    if not user_in.password:
        token = await one_time.create_one_time_token(
            db, user.id, one_time.PASSWORD_RESET,
            timedelta(hours=settings.invitation_token_expire_hours),
        )
        background_tasks.add_task(mailer.send_invitation_email, email, token)

    await record_audit(
        db, "user_created", user_id=actor.id, request=request,
        details={"target_user_id": user.id, "role": role_name, "invited": not user_in.password},
    )
    return user


@router.post(
    endpoints.AUTH_SIGNUP,
    response_model=SignupResponse,
    status_code=status.HTTP_201_CREATED,
    summary="Auto-registro público",
    description=(
        "Disponible solo con `ALLOW_PUBLIC_REGISTRATION=true`. Asigna `DEFAULT_ROLE_NAME`. "
        "Con `REQUIRE_EMAIL_VERIFICATION=true` envía un correo de confirmación y la respuesta "
        "es idéntica exista o no el email (evita enumeración)."
    ),
)
@limiter.limit("5/minute")
async def signup(
    request: Request,
    data: UserSignup,
    background_tasks: BackgroundTasks,
    db: Annotated[Prisma, Depends(get_db)],
):
    if not settings.allow_public_registration:
        raise HTTPException(status_code=403, detail="El registro público está deshabilitado")

    email = data.email.lower()
    ensure_password_policy(data.password, email)
    verification = settings.require_email_verification

    if await db.user.find_unique(where={"email": email}):
        burn_password_check(data.password)  # mismo coste que crear la cuenta (sin pista por tiempos)
        if verification:
            await record_audit(db, "signup_existing_email", request=request, details={"email": email})
            return SignupResponse(message="Revisa tu correo para confirmar la cuenta", verification_required=True)
        raise HTTPException(status_code=409, detail="El email ya está registrado")

    role = await db.role.find_unique(where={"name": settings.default_role_name})
    if role is None:
        logger.error("DEFAULT_ROLE_NAME '%s' no existe; ejecuta seed.py", settings.default_role_name)
        raise HTTPException(status_code=500, detail="Registro no disponible")

    user = await db.user.create(
        data={
            "email": email,
            "first_name": data.first_name,
            "last_name": data.last_name,
            "password_hash": get_password_hash(data.password),
            "role_id": role.id,
            # Nadie ha demostrado aún que controla este correo.
            "email_verified": False,
        }
    )
    if verification:
        token = await one_time.create_one_time_token(
            db, user.id, one_time.EMAIL_VERIFICATION,
            timedelta(hours=settings.email_verification_token_expire_hours),
        )
        background_tasks.add_task(mailer.send_verification_email, email, token)

    await record_audit(db, "signup", user_id=user.id, request=request)
    if verification:
        return SignupResponse(message="Revisa tu correo para confirmar la cuenta", verification_required=True)
    return SignupResponse(message="Cuenta creada correctamente", verification_required=False)


# ---------------------------------------------------------------------------
# Verificación de email
# ---------------------------------------------------------------------------

@router.post(
    endpoints.AUTH_VERIFY_EMAIL,
    response_model=MessageResponse,
    summary="Confirmar email",
    description=(
        "Requiere el token del correo y la contraseña de la cuenta. Pedir la contraseña impide "
        "el secuestro previo: si un tercero registró antes tu email con su contraseña, el enlace "
        "no le sirve y tú recuperas la cuenta con `/auth/password-recovery`."
    ),
)
@limiter.limit("10/minute")
async def verify_email(request: Request, data: EmailVerifyRequest, db: Annotated[Prisma, Depends(get_db)]):
    token = await one_time.find_valid_token(db, data.token, one_time.EMAIL_VERIFICATION)
    if token is None:
        raise HTTPException(status_code=400, detail="Token inválido o expirado")
    if not verify_password(data.password, token.user.password_hash):
        await record_audit(db, "email_verification_bad_password", user_id=token.user_id, request=request)
        raise HTTPException(
            status_code=400,
            detail="La contraseña no coincide. Si no creaste esta cuenta, usa la recuperación de contraseña.",
        )
    user_id = await one_time.consume_one_time_token(db, data.token, one_time.EMAIL_VERIFICATION)
    if not user_id:
        raise HTTPException(status_code=400, detail="Token inválido o expirado")
    await db.user.update(where={"id": user_id}, data={"email_verified": True})
    await record_audit(db, "email_verified", user_id=user_id, request=request)
    return MessageResponse(message="Correo confirmado correctamente")


@router.post(
    endpoints.AUTH_RESEND_VERIFICATION,
    response_model=MessageResponse,
    summary="Reenviar correo de confirmación",
)
@limiter.limit("3/minute")
async def resend_verification(
    request: Request,
    data: PasswordResetRequest,
    background_tasks: BackgroundTasks,
    db: Annotated[Prisma, Depends(get_db)],
):
    email = data.email.lower()
    user = await db.user.find_unique(where={"email": email})
    if user and not user.email_verified and not await one_time.recently_issued(
        db, user.id, one_time.EMAIL_VERIFICATION, settings.email_resend_cooldown_seconds
    ):
        token = await one_time.create_one_time_token(
            db, user.id, one_time.EMAIL_VERIFICATION,
            timedelta(hours=settings.email_verification_token_expire_hours),
        )
        background_tasks.add_task(mailer.send_verification_email, email, token)
    return MessageResponse(message=GENERIC_EMAIL_MESSAGE)


# ---------------------------------------------------------------------------
# Recuperación de contraseña
# ---------------------------------------------------------------------------

@router.post(
    endpoints.AUTH_RECOVERY,
    response_model=MessageResponse,
    summary="Solicitar recuperación de contraseña",
    description="La respuesta es la misma exista o no el email (evita enumeración de usuarios).",
)
@limiter.limit("3/minute")
async def recover_password(
    request: Request,
    data: PasswordResetRequest,
    background_tasks: BackgroundTasks,
    db: Annotated[Prisma, Depends(get_db)],
):
    email = data.email.lower()
    user = await db.user.find_unique(where={"email": email})
    if user is None:
        logger.info("Recuperación solicitada para email inexistente")
    elif await one_time.recently_issued(
        db, user.id, one_time.PASSWORD_RESET, settings.email_resend_cooldown_seconds
    ):
        logger.info("Recuperación ignorada por cooldown para el usuario %s", user.id)
    else:
        token = await one_time.create_one_time_token(
            db, user.id, one_time.PASSWORD_RESET,
            timedelta(minutes=settings.password_reset_token_expire_minutes),
        )
        background_tasks.add_task(mailer.send_password_reset_email, email, token)
        await record_audit(db, "password_reset_requested", user_id=user.id, request=request)
    return MessageResponse(message=GENERIC_EMAIL_MESSAGE)


@router.post(
    endpoints.AUTH_RESET,
    response_model=MessageResponse,
    summary="Restablecer contraseña",
    description=(
        "Cambia la contraseña con el token recibido por correo (recuperación o invitación). "
        "El token es de un solo uso, confirma el email y cierra todas las sesiones abiertas."
    ),
)
@limiter.limit("10/minute")
async def reset_password(
    request: Request,
    data: PasswordResetConfirm,
    background_tasks: BackgroundTasks,
    db: Annotated[Prisma, Depends(get_db)],
):
    token = await one_time.find_valid_token(db, data.token, one_time.PASSWORD_RESET)
    if token is None:
        raise HTTPException(status_code=400, detail="Token inválido o expirado")

    # La política se valida antes de consumir el token para no gastarlo en vano.
    ensure_password_policy(data.new_password, token.user.email)
    user_id = await one_time.consume_one_time_token(db, data.token, one_time.PASSWORD_RESET)
    if not user_id:
        raise HTTPException(status_code=400, detail="Token inválido o expirado")

    await db.user.update(
        where={"id": user_id},
        data={
            "password_hash": get_password_hash(data.new_password),
            "password_changed_at": utcnow(),
            "failed_login_attempts": 0,
            "locked_until": None,
            "email_verified": True,
        },
    )
    await revoke_user_sessions(db, user_id)
    await record_audit(db, "password_reset", user_id=user_id, request=request)
    background_tasks.add_task(mailer.send_password_changed_email, token.user.email)
    return MessageResponse(message="Contraseña actualizada exitosamente")
