from datetime import timedelta
from typing import Annotated
from fastapi import APIRouter, Depends, HTTPException, status, Request
from fastapi.security import OAuth2PasswordRequestForm
from prisma import Prisma

from app.database import get_db
from app.schemas.user import UserCreate, UserOut, Token, UserLogin
from app.security import (
    get_password_hash,
    verify_password,
    create_access_token,
    generate_session_token,
)
from app.config import settings
from app.limiter import limiter

router = APIRouter(prefix="/auth", tags=["Authentication"])

@router.post("/register", response_model=UserOut, status_code=status.HTTP_201_CREATED)
@limiter.limit("5/minute")
async def register(
    request: Request,
    user_in: UserCreate,
    db: Annotated[Prisma, Depends(get_db)]
):
    user_exists = await db.user.find_unique(where={"email": user_in.email})
    if user_exists:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="El email ya está registrado"
        )
    
    # Buscar el rol en la base de datos por su nombre
    role = await db.role.find_unique(where={"name": user_in.role_name})
    if not role:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail=f"El rol '{user_in.role_name}' no existe"
        )
    
    return await db.user.create(
        data={
            "email": user_in.email,
            "password_hash": get_password_hash(user_in.password),
            "role_id": role.id
        },
        include={"role": True}
    )

@router.post(
    "/login", 
    response_model=Token,
    summary="Iniciar sesión",
    description="Autentica al usuario mediante email y contraseña (JSON). Devuelve un JWT de acceso y genera un nuevo token de sesión único."
)
@limiter.limit("10/minute")
async def login(
    request: Request,
    login_data: UserLogin,
    db: Annotated[Prisma, Depends(get_db)]
):
    user = await db.user.find_unique(where={"email": login_data.email})

    if not user or not verify_password(login_data.password, user.password_hash):
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Email o contraseña incorrectos",
            headers={"WWW-Authenticate": "Bearer"},
        )
    
    if not user.is_active:
        raise HTTPException(status_code=status.HTTP_403_FORBIDDEN, detail="Usuario inactivo")

    new_session_token = generate_session_token()
    await db.user.update(
        where={"id": user.id},
        data={"session_token": new_session_token}
    )

    access_token = create_access_token(
        data={"sub": user.id, "session_token": new_session_token},
        expires_delta=timedelta(minutes=settings.access_token_expire_minutes)
    )

    return {"access_token": access_token, "token_type": "bearer"}
