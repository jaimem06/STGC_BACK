from datetime import datetime
from typing import Annotated, List, Optional

from fastapi import APIRouter, Depends, Query, Response
from prisma import Prisma

from app.core import endpoints
from app.database import get_db
from app.dependencies import require_admin
from app.schemas.audit import AuditOut

router = APIRouter(
    prefix=endpoints.AUDIT_PREFIX,
    tags=["Auditoría"],
    dependencies=[Depends(require_admin)],
)


@router.get(
    endpoints.AUDIT_LIST,
    response_model=List[AuditOut],
    summary="Consultar auditoría (solo admin)",
    description="Eventos más recientes primero. El total se devuelve en la cabecera `X-Total-Count`.",
)
async def list_audit_logs(
    response: Response,
    db: Annotated[Prisma, Depends(get_db)],
    user_id: Annotated[Optional[str], Query(max_length=64)] = None,
    action: Annotated[Optional[str], Query(max_length=100)] = None,
    date_from: Optional[datetime] = None,
    date_to: Optional[datetime] = None,
    skip: Annotated[int, Query(ge=0)] = 0,
    limit: Annotated[int, Query(ge=1, le=500)] = 100,
):
    where: dict = {}
    if user_id:
        where["user_id"] = user_id
    if action:
        where["action"] = action
    if date_from or date_to:
        where["created_at"] = {}
        if date_from:
            where["created_at"]["gte"] = date_from
        if date_to:
            where["created_at"]["lte"] = date_to

    response.headers["X-Total-Count"] = str(await db.auditlog.count(where=where))
    return await db.auditlog.find_many(where=where, order={"created_at": "desc"}, skip=skip, take=limit)
