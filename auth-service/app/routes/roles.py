from typing import List, Annotated
import logging
from fastapi import APIRouter, Depends, HTTPException, status
from prisma import Prisma, errors

from app.database import get_db
from app.dependencies import require_manage_users, log_user_action, require_all_access
from app.schemas.user import RoleOut
from app.core import endpoints
from pydantic import BaseModel

logger = logging.getLogger(__name__)
router = APIRouter(prefix=endpoints.ROLES_PREFIX, tags=["Gestión de Roles"])

class RoleCreate(BaseModel):
    name: str
    description: str | None = None

@router.get(
    endpoints.ROLES_LIST, 
    response_model=List[RoleOut], 
    dependencies=[Depends(require_manage_users)],
    summary="Listar Roles",
    description="Devuelve la lista completa de roles definidos en el sistema.",
    responses={500: {"description": "Error interno"}}
)
async def list_roles(db: Annotated[Prisma, Depends(get_db)]):
    """Lista todos los roles."""
    try:
        return await db.role.find_many()
    except Exception as e:
        logger.error(f"Error al listar roles: {str(e)}")
        raise HTTPException(status_code=500, detail="Error al recuperar los roles")

@router.post(
    endpoints.ROLES_CREATE, 
    response_model=RoleOut, 
    status_code=status.HTTP_201_CREATED,
    summary="Crear Nuevo Rol",
    description="Crea un nuevo rol en el sistema. Reservado para Administradores y Gestores Autorizados.",
    responses={
        400: {"description": "El rol ya existe"},
        403: {"description": "Permisos insuficientes"},
        500: {"description": "Error interno"}
    }
)
async def create_role(
    role_in: RoleCreate,
    db: Annotated[Prisma, Depends(get_db)],
    _ = Depends(require_manage_users),
    __ = Depends(log_user_action("create_role"))
):
    """Crea un nuevo rol (ADMIN y Gestores Autorizados)."""
    try:
        existing = await db.role.find_unique(where={"name": role_in.name})
        if existing:
            raise HTTPException(status_code=400, detail="El rol ya existe")
        
        return await db.role.create(
            data={
                "name": role_in.name,
                "description": role_in.description,
            }
        )
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error al crear rol: {str(e)}")
        raise HTTPException(status_code=500, detail="Error interno al crear el rol")

@router.put(
    endpoints.ROLES_UPDATE, 
    response_model=RoleOut,
    summary="Actualizar Rol",
    description="Modifica el nombre o descripción de un rol existente.",
    responses={
        404: {"description": "Rol no encontrado"},
        500: {"description": "Error interno"}
    }
)
async def update_role(
    role_id: str,
    role_in: RoleCreate,
    db: Annotated[Prisma, Depends(get_db)],
    _ = Depends(require_manage_users),
    __ = Depends(log_user_action("update_role"))
):
    """Actualiza un rol."""
    try:
        role = await db.role.find_unique(where={"id": role_id})
        if not role:
            raise HTTPException(status_code=404, detail="Rol no encontrado")
        
        return await db.role.update(
            where={"id": role_id},
            data={
                "name": role_in.name,
                "description": role_in.description,
            }
        )
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error al actualizar rol: {str(e)}")
        raise HTTPException(status_code=500, detail="Error interno al actualizar el rol")

@router.delete(
    endpoints.ROLES_DELETE, 
    status_code=status.HTTP_204_NO_CONTENT,
    summary="Eliminar Rol",
    description="Elimina un rol del sistema siempre que no tenga usuarios asociados.",
    responses={
        400: {"description": "El rol tiene usuarios asociados"},
        404: {"description": "Rol no encontrado"}
    }
)
async def delete_role(
    role_id: str,
    db: Annotated[Prisma, Depends(get_db)],
    _ = Depends(require_manage_users),
    __ = Depends(log_user_action("delete_role"))
):
    """Elimina un rol (Solo si no tiene usuarios)."""
    try:
        role = await db.role.find_unique(where={"id": role_id})
        if not role:
            raise HTTPException(status_code=404, detail="Rol no encontrado")

        users_count = await db.user.count(where={"role_id": role_id})
        if users_count > 0:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST, 
                detail="No se puede eliminar un rol que tiene usuarios asignados"
            )
        
        await db.role.delete(where={"id": role_id})
        return None
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error al eliminar rol: {str(e)}")
        raise HTTPException(status_code=500, detail="Error interno al eliminar el rol")
