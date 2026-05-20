from typing import Annotated
import logging
from fastapi import APIRouter, Depends, HTTPException, status
from prisma import Prisma

from app.database import get_db
from app.dependencies import log_user_action, require_manage_users
from app.core import endpoints

logger = logging.getLogger(__name__)
router = APIRouter(prefix=endpoints.ROLES_PREFIX, tags=["Roles"])

DEFAULT_ROLE_NAME = "CAJERO_MESERO"

@router.delete(
    endpoints.ROLES_DELETE,
    status_code=status.HTTP_200_OK,
    dependencies=[Depends(require_manage_users), Depends(log_user_action("delete_role"))],
    summary="Eliminar un rol y reasignar usuarios",
    description="Elimina un rol del sistema. Todos los usuarios que tenían este rol serán reasignados automáticamente al rol predeterminado (CAJERO_MESERO)."
)
async def delete_role(
    role_id: str,
    db: Annotated[Prisma, Depends(get_db)]
):
    try:
        # 1. Buscar el rol a eliminar
        role = await db.role.find_unique(where={"id": role_id})
        if not role:
            raise HTTPException(status_code=404, detail="Rol no encontrado")

        # 2. No permitir eliminar el rol por defecto o roles críticos
        if role.name == DEFAULT_ROLE_NAME or role.name == "ADMIN":
            raise HTTPException(
                status_code=400, 
                detail=f"No se puede eliminar el rol '{role.name}' por ser crítico para el sistema."
            )

        # 3. Buscar el rol por defecto para la reasignación
        default_role = await db.role.find_unique(where={"name": DEFAULT_ROLE_NAME})
        if not default_role:
            # Si no existe, lo creamos preventivamente
            default_role = await db.role.create(
                data={"name": DEFAULT_ROLE_NAME, "description": "Rol básico asignado por defecto"}
            )

        # 4. Reasignar usuarios al rol por defecto
        # Prisma Python no soporta update_many directo con relaciones en todas las versiones, 
        # pero aquí actualizamos el role_id de los usuarios.
        await db.user.update_many(
            where={"role_id": role_id},
            data={"role_id": default_role.id}
        )

        # 5. Eliminar el rol
        await db.role.delete(where={"id": role_id})

        return {"message": f"Rol '{role.name}' eliminado. Los usuarios han sido reasignados a '{DEFAULT_ROLE_NAME}'."}

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error al eliminar rol: {str(e)}")
        raise HTTPException(status_code=500, detail="Error interno al procesar la eliminación del rol")
