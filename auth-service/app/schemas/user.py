import re
from typing import Optional, List
from pydantic import BaseModel, EmailStr, ConfigDict, field_validator
from enum import Enum

# Regex para validaciones
EMAIL_REGEX = r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$"
PASSWORD_REGEX = r"^(?=.*[a-z])(?=.*[A-Z])(?=.*\d)(?=.*[@$!%*?&])[A-Za-z\d@$!%*?&]{8,}$"

class UserStatus(str, Enum):
    ACTIVO = "ACTIVO"
    INACTIVO = "INACTIVO"
    SUSPENDIDO = "SUSPENDIDO"
    PENDIENTE = "PENDIENTE"

class PermissionOut(BaseModel):
    id: str
    name: str
    description: Optional[str] = None

    model_config = ConfigDict(from_attributes=True)

class RoleOut(BaseModel):
    id: str
    name: str
    description: Optional[str] = None
    permissions: Optional[List[PermissionOut]] = None

    model_config = ConfigDict(from_attributes=True)

class UserBase(BaseModel):
    email: EmailStr

    @field_validator("email")
    @classmethod
    def validate_email(cls, v: str) -> str:
        if not re.match(EMAIL_REGEX, v):
            raise ValueError("El formato del correo electrónico no es válido")
        return v

class UserCreate(UserBase):
    password: str
    role_name: str
    status: Optional[UserStatus] = UserStatus.ACTIVO

    @field_validator("password")
    @classmethod
    def validate_password(cls, v: str) -> str:
        if not re.match(PASSWORD_REGEX, v):
            raise ValueError(
                "La contraseña debe tener al menos 8 caracteres, incluyendo una letra mayúscula, "
                "una minúscula, un número y un carácter especial (@$!%*?&)"
            )
        return v

class UserOut(UserBase):
    id: str
    role: RoleOut
    status: UserStatus

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

    @field_validator("new_password")
    @classmethod
    def validate_new_password(cls, v: str) -> str:
        if not re.match(PASSWORD_REGEX, v):
            raise ValueError(
                "La nueva contraseña debe tener al menos 8 caracteres, incluyendo una letra mayúscula, "
                "una minúscula, un número y un carácter especial (@$!%*?&)"
            )
        return v
