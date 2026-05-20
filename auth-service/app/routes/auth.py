from datetime import timedelta
from typing import Annotated
import logging
from fastapi import APIRouter, Depends, HTTPException, status, Request
from fastapi.security import OAuth2PasswordRequestForm
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
)
from app.utils.email import send_password_reset_email
from app.config import settings
from app.limiter import limiter
from app.core import endpoints

logger = logging.getLogger(__name__)
router = APIRouter(prefix=endpoints.AUTH_PREFIX, tags=["Authentication"])

@router.post(endpoints.AUTH_REGISTER, response_model=UserOut, status_code=status.HTTP_201_CREATED, dependencies=[Depends(require_manage_users)])
@limiter.limit("5/minute")
async def register(request: Request, user_in: UserCreate, db: Annotated[Prisma, Depends(get_db)]):
    try:
        user_exists = await db.user.find_unique(where={"email": user_in.email})
        if user_exists:
            raise HTTPException(status_code=400, detail="Email ya registrado")
        
        role = await db.role.find_unique(where={"name": user_in.role_name})
        if not role:
            raise HTTPException(status_code=400, detail="Rol inválido")
        
        return await db.user.create(
            data={
                "email": user_in.email,
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
        raise HTTPException(status_code=500, detail="Error en servidor")

@router.post(endpoints.AUTH_LOGIN, response_model=Token)
@limiter.limit("10/minute")
async def login(request: Request, login_data: UserLogin, db: Annotated[Prisma, Depends(get_db)]):
    user = await db.user.find_unique(where={"email": login_data.email})
    if not user or not verify_password(login_data.password, user.password_hash):
        raise HTTPException(status_code=401, detail="Credenciales incorrectas")
    
    if user.status != "ACTIVO":
        raise HTTPException(status_code=403, detail=f"Cuenta {user.status}")

    new_session_token = generate_session_token()
    await db.user.update(where={"id": user.id}, data={"session_token": new_session_token})

    access_token = create_access_token(
        data={"sub": user.id, "session_token": new_session_token},
        expires_delta=timedelta(minutes=settings.access_token_expire_minutes)
    )
    return {"access_token": access_token, "token_type": "bearer"}

@router.get(endpoints.AUTH_ME, response_model=UserOut)
async def get_me(current_user: Annotated[UserOut, Depends(get_current_user)]):
    return current_user

@router.post(endpoints.AUTH_RECOVERY)
@limiter.limit("3/minute")
async def recover_password(request: Request, data: PasswordResetRequest, db: Annotated[Prisma, Depends(get_db)]):
    user = await db.user.find_unique(where={"email": data.email})
    if user:
        token = create_password_reset_token(data.email)
        send_password_reset_email(data.email, token)
    return {"message": "Si existe, se envió correo"}

@router.post(endpoints.AUTH_RESET)
async def reset_password(data: PasswordResetConfirm, db: Annotated[Prisma, Depends(get_db)]):
    email = verify_password_reset_token(data.token)
    if not email:
        raise HTTPException(status_code=400, detail="Token inválido")
    
    await db.user.update(
        where={"email": email},
        data={"password_hash": get_password_hash(data.new_password), "session_token": generate_session_token()}
    )
    return {"message": "Contraseña actualizada"}
