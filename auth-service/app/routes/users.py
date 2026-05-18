from typing import List, Annotated, Optional
import logging
from fastapi import APIRouter, Depends, HTTPException, status
from prisma import Prisma, errors

from app.database import get_db
from app.dependencies import require_manage_users, log_user_action
from app.schemas.user import UserOut, UserStatus
from pydantic import BaseModel

logger = logging.getLogger(__name__)
router = APIRouter(prefix="/users", tags=["User Management"])

class UserUpdate(BaseModel):
    role_name: Optional[str] = None
    status: Optional[UserStatus] = None

@router.get(
    "/", 
    response_model=List[UserOut],
    responses={
        403: {"description": "Permisos insuficientes"},
        500: {"description": "Error interno"}
    }
)
async def list_users(
    db: Annotated[Prisma, Depends(get_db)],
    _ = Depends(require_manage_users)
):
    """Lista todos los usuarios con sus roles (Solo gestores de usuarios)."""
    try:
        return await db.user.find_many(include={"role": {"include": {"permissions": True}}})
    except Exception as e:
        logger.error(f"Error al listar usuarios: {str(e)}")
        raise HTTPException(status_code=500, detail="Error al recuperar la lista de usuarios")

@router.patch(
    "/{user_id}", 
    response_model=UserOut,
    responses={
        400: {"description": "Datos de actualización inválidos"},
        404: {"description": "Usuario no encontrado"},
        500: {"description": "Error interno"}
    }
)
async def update_user(
    user_id: str,
    user_update: UserUpdate,
    db: Annotated[Prisma, Depends(get_db)],
    _ = Depends(require_manage_users),
    __ = Depends(log_user_action("update_user"))
):
    """Actualiza el rol o el estado de un usuario existente (Solo gestores de usuarios)."""
    try:
        # 1. Verificar que el usuario existe
        user = await db.user.find_unique(where={"id": user_id})
        if not user:
            raise HTTPException(status_code=404, detail="Usuario no encontrado")

        update_data = {}
        
        # 2. Validar rol si se proporciona
        if user_update.role_name:
            role = await db.role.find_unique(where={"name": user_update.role_name})
            if not role:
                raise HTTPException(status_code=400, detail=f"El rol '{user_update.role_name}' no existe")
            update_data["role_id"] = role.id
        
        # 3. Validar estado si se proporciona
        if user_update.status:
            update_data["status"] = user_update.status

        if not update_data:
            raise HTTPException(status_code=400, detail="No se proporcionaron datos válidos para actualizar")

        # 4. Ejecutar actualización
        return await db.user.update(
            where={"id": user_id},
            data=update_data,
            include={"role": {"include": {"permissions": True}}}
        )

    except HTTPException:
        raise
    except errors.PrismaError as e:
        logger.error(f"Error de base de datos al actualizar usuario: {str(e)}")
        raise HTTPException(status_code=500, detail="Error al actualizar el usuario en la base de datos")
    except Exception as e:
        logger.error(f"Error inesperado al actualizar usuario: {str(e)}")
        raise HTTPException(status_code=500, detail="Error interno del servidor")
