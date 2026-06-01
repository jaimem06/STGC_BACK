from datetime import timedelta
from typing import Annotated
import logging
from fastapi import APIRouter, Depends, HTTPException, status, Request, BackgroundTasks
from prisma import Prisma, errors

from app.database import get_db
from app.dependencies import log_user_action, require_manage_users, get_current_user
from app.schemas.user import UserCreate, UserOut, Token, UserLogin, PasswordResetRequest, PasswordResetConfirm
from app.security import (
    get_password_hash,
    verify_password,
    create_access_token,
    generate_session_token,
    create_password_reset_token,
    verify_password_reset_token,
    get_email_from_token_unverified,
)
from app.utils.email import send_password_reset_email
from app.config import settings
from app.limiter import limiter
from app.core import endpoints

logger = logging.getLogger(__name__)
router = APIRouter(prefix=endpoints.AUTH_PREFIX, tags=["Autenticación"])

@router.post(
    endpoints.AUTH_REGISTER, 
    response_model=UserOut, 
    status_code=status.HTTP_201_CREATED, 
    dependencies=[Depends(require_manage_users)],
    summary="Registrar Usuario",
    description="Crea una nueva cuenta de usuario. Requiere permisos de gestión de usuarios (ADMIN o Gerencia)."
)
@limiter.limit("5/minute")
async def register(request: Request, user_in: UserCreate, db: Annotated[Prisma, Depends(get_db)]):
    try:
        # Normalizar email
        email_lower = user_in.email.lower()
        
        # 1. Verificar Email
        user_exists = await db.user.find_unique(where={"email": email_lower})
        if user_exists:
            raise HTTPException(status_code=400, detail="El email ya está registrado")
        
        if user_in.identifier:
            id_exists = await db.user.find_unique(where={"identifier": user_in.identifier})
            if id_exists:
                raise HTTPException(status_code=400, detail="El identificador (cédula/ID) ya está en uso")

        if user_in.phone_number:
            phone_exists = await db.user.find_unique(where={"phone_number": user_in.phone_number})
            if phone_exists:
                raise HTTPException(status_code=400, detail="El número de teléfono ya está registrado")
        
        role = await db.role.find_unique(where={"name": user_in.role_name})
        if not role:
            raise HTTPException(status_code=400, detail="Rol inválido")
        
        return await db.user.create(
            data={
                "email": email_lower,
                "first_name": user_in.first_name,
                "last_name": user_in.last_name,
                "identifier": user_in.identifier,
                "phone_number": user_in.phone_number,
                "password_hash": get_password_hash(user_in.password),
                "role_id": role.id,
                "status": user_in.status if user_in.status else "ACTIVO"
            },
            include={"role": True}
        )
    except HTTPException: raise
    except Exception as e:
        logger.error(f"Registration error: {e}")
        raise HTTPException(status_code=500, detail="Error interno en servidor")

@router.post(
    endpoints.AUTH_LOGIN, 
    response_model=Token,
    summary="Iniciar Sesión",
    description="Autentica al usuario y devuelve un token JWT. Soporta formato JSON."
)
@limiter.limit("10/minute")
async def login(request: Request, login_data: UserLogin, db: Annotated[Prisma, Depends(get_db)]):
    email_lower = login_data.email.lower()
    user = await db.user.find_unique(
        where={"email": email_lower},
        include={"role": True}
    )
    if not user or not verify_password(login_data.password, user.password_hash):
        raise HTTPException(status_code=401, detail="Credenciales incorrectas")
    
    if user.status != "ACTIVO":
        raise HTTPException(status_code=403, detail=f"Cuenta {user.status}")

    new_session_token = generate_session_token()
    await db.user.update(where={"id": user.id}, data={"session_token": new_session_token})

    access_token = create_access_token(
        data={
            "sub": user.id, 
            "role": user.role.name,
            "session_token": new_session_token
        },
        expires_delta=timedelta(minutes=settings.access_token_expire_minutes)
    )
    return {"access_token": access_token, "token_type": "bearer"}

@router.get(
    endpoints.AUTH_ME, 
    response_model=UserOut,
    summary="Obtener Perfil Actual",
    description="Devuelve la información del usuario autenticado basado en el token JWT."
)
async def get_me(current_user: Annotated[UserOut, Depends(get_current_user)]):
    return current_user

@router.post(
    endpoints.AUTH_RECOVERY,
    summary="Recuperar Contraseña",
    description="Envía un correo electrónico con un enlace de recuperación si el usuario existe."
)
@limiter.limit("3/minute")
async def recover_password(
    request: Request, 
    data: PasswordResetRequest, 
    db: Annotated[Prisma, Depends(get_db)],
    background_tasks: BackgroundTasks
):
    email_lower = data.email.lower()
    user = await db.user.find_unique(where={"email": email_lower})
    if user:
        token = create_password_reset_token(email_lower, user.password_hash)
        background_tasks.add_task(send_password_reset_email, email_lower, token)
    return {"message": "En caso de que el email exista, se ha enviado un enlace de recuperación"}

@router.post(
    endpoints.AUTH_RESET,
    summary="Restablecer Contraseña",
    description="Cambia la contraseña utilizando un token de recuperación válido."
)
async def reset_password(data: PasswordResetConfirm, db: Annotated[Prisma, Depends(get_db)]):
    email = get_email_from_token_unverified(data.token)
    if not email:
        raise HTTPException(status_code=400, detail="Token inválido")

    user = await db.user.find_unique(where={"email": email})
    if not user:
        raise HTTPException(status_code=400, detail="Usuario no encontrado")

    verified_email = verify_password_reset_token(data.token, user.password_hash)
    if not verified_email:
        raise HTTPException(status_code=400, detail="Token inválido o ya utilizado")

    await db.user.update(
        where={"email": email},
        data={
            "password_hash": get_password_hash(data.new_password), 
            "session_token": generate_session_token()
        }
    )
    return {"message": "Contraseña actualizada exitosamente"}

