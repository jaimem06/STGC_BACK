from datetime import timedelta
from typing import Annotated
import logging
from fastapi import APIRouter, Depends, HTTPException, status, Request
from prisma import Prisma, errors

from app.database import get_db
from app.dependencies import log_user_action, require_manage_users
from app.schemas.user import UserCreate, UserOut, Token, UserLogin, PasswordResetRequest, PasswordResetConfirm
from app.security import (
    get_password_hash,
    verify_password,
    create_access_token,
    generate_session_token,
    create_password_reset_token,
    verify_password_reset_token,
)
from app.utils.email import send_password_reset_email
from app.config import settings
from app.limiter import limiter
from app.core import endpoints

logger = logging.getLogger(__name__)
router = APIRouter(prefix=endpoints.AUTH_PREFIX, tags=["Authentication"])

@router.post(
    endpoints.AUTH_REGISTER, 
    response_model=UserOut, 
    status_code=status.HTTP_201_CREATED,
    dependencies=[Depends(require_manage_users)],
    responses={
        400: {"description": "Error en los datos de entrada o el email ya existe"},
        403: {"description": "Permisos insuficientes"},
        500: {"description": "Error interno del servidor"}
    }
)
@limiter.limit("5/minute")
async def register(
    request: Request,
    user_in: UserCreate,
    db: Annotated[Prisma, Depends(get_db)]
):
    """
    Registra un nuevo usuario en el sistema.
    Requiere un email único y un nombre de rol válido.
    Solo accesible por ADMIN, GERENTE_GENERAL y GERENTE_OPERACIONES.
    """
    try:
        # 1. Validar si el usuario ya existe
        user_exists = await db.user.find_unique(where={"email": user_in.email})
        if user_exists:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail="El email ya está registrado"
            )
        
        # 2. Validar existencia del rol
        role = await db.role.find_unique(where={"name": user_in.role_name})
        if not role:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail=f"El rol '{user_in.role_name}' no existe en el sistema"
            )
        
        # 3. Crear el usuario
        new_user = await db.user.create(
            data={
                "email": user_in.email,
                "password_hash": get_password_hash(user_in.password),
                "role_id": role.id,
                "status": user_in.status if user_in.status else "ACTIVO"
            },
            include={"role": True}
        )
        return new_user

    except HTTPException:
        raise
    except errors.PrismaError as e:
        logger.error(f"Error de Prisma en registro: {str(e)}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="Error al procesar la solicitud en la base de datos"
        )
    except Exception as e:
        logger.error(f"Error inesperado en registro: {str(e)}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="Ha ocurrido un error inesperado"
        )

@router.post(
    endpoints.AUTH_LOGIN, 
    response_model=Token,
    summary="Iniciar sesión",
    description="Autentica al usuario mediante email y contraseña (JSON). Devuelve un JWT de acceso.",
    responses={
        401: {"description": "Credenciales inválidas"},
        403: {"description": "Usuario inactivo o suspendido"},
        500: {"description": "Error interno del servidor"}
    }
)
@limiter.limit("10/minute")
async def login(
    request: Request,
    login_data: UserLogin,
    db: Annotated[Prisma, Depends(get_db)]
):
    try:
        # 1. Buscar usuario
        user = await db.user.find_unique(where={"email": login_data.email})

        # 2. Validar credenciales
        if not user or not verify_password(login_data.password, user.password_hash):
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail="Email o contraseña incorrectos",
                headers={"WWW-Authenticate": "Bearer"},
            )
        
        # 3. Validar estado del usuario
        if user.status != "ACTIVO":
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN, 
                detail=f"Acceso denegado. El usuario se encuentra en estado: {user.status}"
            )

        # 4. Actualizar token de sesión (Rotación para seguridad)
        new_session_token = generate_session_token()
        await db.user.update(
            where={"id": user.id},
            data={"session_token": new_session_token}
        )

        # 5. Generar JWT
        access_token = create_access_token(
            data={"sub": user.id, "session_token": new_session_token},
            expires_delta=timedelta(minutes=settings.access_token_expire_minutes)
        )

        return {"access_token": access_token, "token_type": "bearer"}

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error en login: {str(e)}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="Error interno durante la autenticación"
        )

@router.post(
    endpoints.AUTH_RECOVERY,
    status_code=status.HTTP_200_OK,
    summary="Solicitar recuperación de contraseña",
    responses={500: {"description": "Error interno"}}
)
@limiter.limit("3/minute")
async def recover_password(
    request: Request,
    data: PasswordResetRequest,
    db: Annotated[Prisma, Depends(get_db)]
):
    """
    Genera un token de recuperación y lo envía al correo si el usuario existe.
    """
    try:
        user = await db.user.find_unique(where={"email": data.email})
        
        # Por seguridad, siempre devolvemos 200 aunque el usuario no exista
        if user:
            token = create_password_reset_token(data.email)
            send_password_reset_email(data.email, token)
            
        return {"message": "Si el correo está registrado, recibirás un enlace de recuperación"}
    except Exception as e:
        logger.error(f"Error en recuperación de contraseña: {str(e)}")
        raise HTTPException(status_code=500, detail="Error al procesar la recuperación")

@router.post(
    endpoints.AUTH_RESET,
    status_code=status.HTTP_200_OK,
    summary="Restablecer contraseña con token",
    responses={
        400: {"description": "Token inválido o expirado"},
        404: {"description": "Usuario no encontrado"}
    }
)
async def reset_password(
    data: PasswordResetConfirm,
    db: Annotated[Prisma, Depends(get_db)]
):
    """
    Valida el token de recuperación y actualiza la contraseña del usuario.
    """
    email = verify_password_reset_token(data.token)
    if not email:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="El token es inválido o ha expirado"
        )
    
    try:
        user = await db.user.find_unique(where={"email": email})
        if not user:
            raise HTTPException(status_code=404, detail="Usuario no encontrado")
        
        # Actualizar contraseña y rotar session_token por seguridad
        await db.user.update(
            where={"email": email},
            data={
                "password_hash": get_password_hash(data.new_password),
                "session_token": generate_session_token()
            }
        )
        return {"message": "Contraseña actualizada exitosamente"}
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error al resetear contraseña: {str(e)}")
        raise HTTPException(status_code=500, detail="Error interno al actualizar la contraseña")
