from datetime import datetime
from typing import Annotated, List, Optional

from fastapi import APIRouter, Depends, Query, Response
from sqlalchemy import func, select

from app.core import endpoints
from app.dependencies import Db, require_admin
from app.models import AuditLog
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
    db: Db,
    user_id: Annotated[Optional[str], Query(max_length=64)] = None,
    action: Annotated[Optional[str], Query(max_length=100)] = None,
    date_from: Optional[datetime] = None,
    date_to: Optional[datetime] = None,
    skip: Annotated[int, Query(ge=0)] = 0,
    limit: Annotated[int, Query(ge=1, le=500)] = 100,
):
    conditions = []
    if user_id:
        conditions.append(AuditLog.user_id == user_id)
    if action:
        conditions.append(AuditLog.action == action)
    if date_from:
        conditions.append(AuditLog.created_at >= date_from)
    if date_to:
        conditions.append(AuditLog.created_at <= date_to)

    total = await db.scalar(select(func.count()).select_from(AuditLog).where(*conditions))
    response.headers["X-Total-Count"] = str(total)
    logs = await db.scalars(
        select(AuditLog).where(*conditions)
        .order_by(AuditLog.created_at.desc(), AuditLog.id)
        .offset(skip).limit(limit)
    )
    return list(logs)
