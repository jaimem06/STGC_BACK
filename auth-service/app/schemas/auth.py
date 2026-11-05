from datetime import datetime
from typing import Annotated, Literal, Optional

from pydantic import BaseModel, ConfigDict, EmailStr, StringConstraints

from app.schemas.user import Password

OpaqueToken = Annotated[str, StringConstraints(min_length=1, max_length=512)]
TotpCode = Annotated[str, StringConstraints(strip_whitespace=True, pattern=r"^[0-9 ]{6,8}$")]


class LoginRequest(BaseModel):
    email: EmailStr
    password: Password

    model_config = ConfigDict(
        json_schema_extra={"example": {"email": "admin@example.com", "password": "tu_contraseña"}}
    )


class TokenPair(BaseModel):
    access_token: str
    refresh_token: str
    token_type: Literal["bearer"] = "bearer"
    expires_in: int
    refresh_expires_in: int


class MfaChallenge(BaseModel):
    mfa_required: Literal[True] = True
    mfa_token: str
    token_type: Literal["mfa"] = "mfa"


class MfaVerifyRequest(BaseModel):
    mfa_token: OpaqueToken
    code: TotpCode


class RefreshRequest(BaseModel):
    refresh_token: OpaqueToken


class PasswordResetRequest(BaseModel):
    email: EmailStr


class PasswordResetConfirm(BaseModel):
    token: OpaqueToken
    new_password: Password


class EmailVerifyRequest(BaseModel):
    token: OpaqueToken
    password: Password


class ChangePasswordRequest(BaseModel):
    current_password: Password
    new_password: Password


class PasswordConfirm(BaseModel):
    password: Password


class MfaSetupResponse(BaseModel):
    secret: str
    otpauth_uri: str


class MfaCodeRequest(BaseModel):
    code: TotpCode


class MfaDisableRequest(BaseModel):
    password: Password
    code: TotpCode


class SessionOut(BaseModel):
    id: str
    user_agent: Optional[str] = None
    ip_address: Optional[str] = None
    created_at: datetime
    last_used_at: datetime
    expires_at: datetime
    current: bool = False


class MessageResponse(BaseModel):
    message: str


class SignupResponse(BaseModel):
    message: str
    verification_required: bool
