from typing import Optional, List
from pydantic import BaseModel, EmailStr, ConfigDict
from enum import Enum

class UserStatus(str, Enum):
    ACTIVO = "ACTIVO"
    INACTIVO = "INACTIVO"
    SUSPENDIDO = "SUSPENDIDO"
    PENDIENTE = "PENDIENTE"

class RoleOut(BaseModel):
    id: str
    name: str
    description: Optional[str] = None

    model_config = ConfigDict(from_attributes=True)

class UserBase(BaseModel):
    email: EmailStr

class UserCreate(UserBase):
    password: str
    role_name: str
    first_name: Optional[str] = None
    last_name: Optional[str] = None
    identifier: Optional[str] = None
    phone_number: Optional[str] = None
    status: Optional[UserStatus] = UserStatus.ACTIVO

class UserOut(UserBase):
    id: str
    role: RoleOut
    status: UserStatus
    first_name: Optional[str] = None
    last_name: Optional[str] = None
    identifier: Optional[str] = None
    phone_number: Optional[str] = None

    model_config = ConfigDict(from_attributes=True)

class UserLogin(BaseModel):
    email: EmailStr
    password: str

    model_config = ConfigDict(
        json_schema_extra={
            "example": {
                "email": "admin@finca.com",
                "password": "tu_contraseña_segura"
            }
        }
    )

class Token(BaseModel):
    access_token: str
    token_type: str

    model_config = ConfigDict(
        json_schema_extra={
            "example": {
                "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
                "token_type": "bearer"
            }
        }
    )

class TokenData(BaseModel):
    user_id: Optional[str] = None
    session_token: Optional[str] = None

class PasswordResetRequest(BaseModel):
    email: EmailStr

class PasswordResetConfirm(BaseModel):
    token: str
    new_password: str
