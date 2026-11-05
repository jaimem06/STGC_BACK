from datetime import datetime
from typing import Annotated, Any, Optional

from pydantic import BaseModel, ConfigDict, StringConstraints


class AuditCreate(BaseModel):
    user_id: Optional[Annotated[str, StringConstraints(max_length=64)]] = None
    action: Annotated[str, StringConstraints(strip_whitespace=True, min_length=1, max_length=100)]
    endpoint: Optional[Annotated[str, StringConstraints(max_length=255)]] = None
    ip_address: Optional[Annotated[str, StringConstraints(max_length=64)]] = None
    details: Optional[dict[str, Any]] = None


class AuditOut(BaseModel):
    id: str
    user_id: Optional[str] = None
    action: str
    endpoint: Optional[str] = None
    ip_address: Optional[str] = None
    user_agent: Optional[str] = None
    details: Optional[str] = None
    created_at: datetime

    model_config = ConfigDict(from_attributes=True)


class IntrospectRequest(BaseModel):
    token: Annotated[str, StringConstraints(min_length=1, max_length=4096)]


class IntrospectResponse(BaseModel):
    """Formato inspirado en RFC 7662 (OAuth 2.0 Token Introspection)."""
    active: bool
    sub: Optional[str] = None
    email: Optional[str] = None
    role: Optional[str] = None
    sid: Optional[str] = None
    status: Optional[str] = None
    exp: Optional[int] = None
