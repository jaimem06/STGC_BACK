from typing import Optional
from pydantic import BaseModel, EmailStr, ConfigDict
from app.models.user import RoleEnum

class UserBase(BaseModel):
    email: EmailStr

class UserCreate(UserBase):
    password: str
    role: Optional[RoleEnum] = RoleEnum.CAJERO_MESERO

class UserOut(UserBase):
    id: str
    role: RoleEnum
    is_active: bool

    model_config = ConfigDict(from_attributes=True)

class Token(BaseModel):
    access_token: str
    token_type: str

class TokenData(BaseModel):
    user_id: Optional[str] = None
    session_token: Optional[str] = None
