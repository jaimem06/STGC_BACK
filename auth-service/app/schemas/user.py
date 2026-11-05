from datetime import datetime
from enum import Enum
from typing import Annotated, Optional

from pydantic import BaseModel, ConfigDict, EmailStr, StringConstraints, model_validator

from app.schemas.role import RoleOut, RoleName

# Límites de tamaño en todas las entradas para evitar abusos y datos basura.
Name = Annotated[str, StringConstraints(strip_whitespace=True, min_length=1, max_length=100)]
Identifier = Annotated[str, StringConstraints(strip_whitespace=True, min_length=1, max_length=50)]
Phone = Annotated[
    str,
    StringConstraints(strip_whitespace=True, pattern=r"^\+?[0-9][0-9 ()\-.]{5,29}$"),
]
# La política completa se valida en el servidor (app.security.password_policy_errors).
Password = Annotated[str, StringConstraints(min_length=1, max_length=128)]


class UserStatus(str, Enum):
    ACTIVO = "ACTIVO"
    INACTIVO = "INACTIVO"
    SUSPENDIDO = "SUSPENDIDO"
    PENDIENTE = "PENDIENTE"


class UserCreate(BaseModel):
    """Alta de usuario por un gestor."""
    email: EmailStr
    # Si se omite, el usuario recibe un correo de invitación para definir su contraseña.
    password: Optional[Password] = None
    # Si se omite, se asigna DEFAULT_ROLE_NAME.
    role_name: Optional[RoleName] = None
    first_name: Optional[Name] = None
    last_name: Optional[Name] = None
    identifier: Optional[Identifier] = None
    phone_number: Optional[Phone] = None
    status: UserStatus = UserStatus.ACTIVO


class UserSignup(BaseModel):
    """Auto-registro público (solo si ALLOW_PUBLIC_REGISTRATION=true)."""
    email: EmailStr
    password: Password
    first_name: Optional[Name] = None
    last_name: Optional[Name] = None


class UserUpdate(BaseModel):
    email: Optional[EmailStr] = None
    first_name: Optional[Name] = None
    last_name: Optional[Name] = None
    identifier: Optional[Identifier] = None
    phone_number: Optional[Phone] = None
    role_name: Optional[RoleName] = None
    status: Optional[UserStatus] = None
    password: Optional[Password] = None
    email_verified: Optional[bool] = None


class ProfileUpdate(BaseModel):
    """Campos que el propio usuario puede cambiar."""
    first_name: Optional[Name] = None
    last_name: Optional[Name] = None
    phone_number: Optional[Phone] = None


class UserSuspend(BaseModel):
    """Periodo de suspensión. Enviar ambos campos en null levanta la suspensión."""
    suspended_from: Optional[datetime] = None
    suspended_until: Optional[datetime] = None

    @model_validator(mode="after")
    def _validate_range(self):
        if (self.suspended_from is None) != (self.suspended_until is None):
            raise ValueError("suspended_from y suspended_until deben enviarse juntos o ambos en null")
        if self.suspended_from and self.suspended_until and self.suspended_until <= self.suspended_from:
            raise ValueError("suspended_until debe ser posterior a suspended_from")
        return self


class UserOut(BaseModel):
    id: str
    email: EmailStr
    role: RoleOut
    status: UserStatus
    first_name: Optional[str] = None
    last_name: Optional[str] = None
    identifier: Optional[str] = None
    phone_number: Optional[str] = None
    email_verified: bool = False
    mfa_enabled: bool = False
    suspended_from: Optional[datetime] = None
    suspended_until: Optional[datetime] = None
    locked_until: Optional[datetime] = None
    last_login_at: Optional[datetime] = None
    created_at: Optional[datetime] = None

    model_config = ConfigDict(from_attributes=True)


class UserPublic(BaseModel):
    """Datos mínimos de un usuario para otros servicios."""
    id: str
    email: EmailStr
    first_name: Optional[str] = None
    last_name: Optional[str] = None
    role: RoleOut
    status: UserStatus

    model_config = ConfigDict(from_attributes=True)
