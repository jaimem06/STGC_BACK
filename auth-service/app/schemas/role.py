from typing import Annotated, Optional

from pydantic import BaseModel, ConfigDict, StringConstraints

RoleName = Annotated[
    str, StringConstraints(strip_whitespace=True, pattern=r"^[A-Za-z0-9_\-]{2,50}$")
]


class RoleCreate(BaseModel):
    name: RoleName
    description: Optional[Annotated[str, StringConstraints(max_length=255)]] = None


class RoleOut(BaseModel):
    id: str
    name: str
    description: Optional[str] = None

    model_config = ConfigDict(from_attributes=True)
