from typing import Optional, List
from pydantic import BaseModel, EmailStr, ConfigDict

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

class UserCreate(UserBase):
    password: str
    role_name: str

class UserOut(UserBase):
    id: str
    role: RoleOut
    is_active: bool

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
