from typing import Annotated
import logging
from fastapi import APIRouter, Depends, HTTPException, status
from prisma import Prisma

from app.database import get_db
from app.dependencies import log_user_action, require_manage_users
from app.schemas.user import UserSuspend, UserOut
from app.core import endpoints

logger = logging.getLogger(__name__)
router = APIRouter(prefix=endpoints.USERS_PREFIX, tags=["Users"])

@router.patch(
    endpoints.USERS_SUSPEND,
    response_model=UserOut,
    dependencies=[Depends(require_manage_users), Depends(log_user_action("suspend_user"))],
    summary="Suspender usuario temporalmente",
    description="Establece un periodo de tiempo durante el cual el usuario no podrá acceder al sistema."
)
async def suspend_user(
    user_id: str,
    suspend_data: UserSuspend,
    db: Annotated[Prisma, Depends(get_db)]
):
    try:
        user = await db.user.find_unique(where={"id": user_id})
        if not user:
            raise HTTPException(status_code=404, detail="Usuario no encontrado")

        updated_user = await db.user.update(
            where={"id": user_id},
            data={
                "suspended_from": suspend_data.suspended_from,
                "suspended_until": suspend_data.suspended_until,
                "status": "SUSPENDIDO" if suspend_data.suspended_until else user.status
            },
            include={"role": True}
        )
        return updated_user
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error al suspender usuario: {str(e)}")
        raise HTTPException(status_code=500, detail="Error interno al procesar la suspensión")
