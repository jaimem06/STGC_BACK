from datetime import timedelta
from typing import Annotated
from fastapi import APIRouter, Depends, HTTPException, status, Request
from fastapi.security import OAuth2PasswordRequestForm
from prisma import Prisma
from prisma.enums import Role

from app.database import get_db
from app.schemas.user import UserCreate, UserOut, Token
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
    user = await db.user.find_unique(where={"email": user_in.email})
    if user:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="El email ya está registrado"
        )
    
    return await db.user.create(
        data={
            "email": user_in.email,
            "password_hash": get_password_hash(user_in.password),
            "role": Role(user_in.role) if user_in.role else Role.CAJERO_MESERO
        }
    )

@router.post("/login", response_model=Token)
@limiter.limit("10/minute")
async def login(
    request: Request,
    form_data: Annotated[OAuth2PasswordRequestForm, Depends()],
    db: Annotated[Prisma, Depends(get_db)]
):
    user = await db.user.find_unique(where={"email": form_data.username})

    if not user or not verify_password(form_data.password, user.password_hash):
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
