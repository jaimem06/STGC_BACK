from typing import Optional
from pydantic import BaseModel

class AuditCreate(BaseModel):
    user_id: str
    action: str
    endpoint: Optional[str] = None
    ip_address: Optional[str] = None
