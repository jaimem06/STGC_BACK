from typing import List, Annotated
import logging
from fastapi import APIRouter, Depends, HTTPException, status
from prisma import Prisma, errors

from app.database import get_db
from app.dependencies import require_manage_users, log_user_action, require_all_access
from app.schemas.user import RoleOut, PermissionOut
from pydantic import BaseModel

logger = logging.getLogger(__name__)
router = APIRouter(prefix="/roles", tags=["Role Management"])

class RoleCreate(BaseModel):
    name: str
    description: str | None = None
    permission_ids: List[str] = []

class PermissionCreate(BaseModel):
    name: str
    description: str | None = None

@router.get(
    "/", 
    response_model=List[RoleOut], 
    dependencies=[Depends(require_manage_users)],
    responses={500: {"description": "Error interno"}}
)
async def list_roles(db: Annotated[Prisma, Depends(get_db)]):
    """Lista todos los roles con sus permisos."""
    try:
        return await db.role.find_many(include={"permissions": True})
    except Exception as e:
        logger.error(f"Error al listar roles: {str(e)}")
        raise HTTPException(status_code=500, detail="Error al recuperar los roles")

@router.post(
    "/", 
    response_model=RoleOut, 
    status_code=status.HTTP_201_CREATED,
    responses={
        400: {"description": "El rol ya existe"},
        403: {"description": "Permisos insuficientes"},
        500: {"description": "Error interno"}
    }
)
async def create_role(
    role_in: RoleCreate,
    db: Annotated[Prisma, Depends(get_db)],
    _ = Depends(require_all_access),
    __ = Depends(log_user_action("create_role"))
):
    """Crea un nuevo rol y le asigna permisos (Solo ADMIN)."""
    try:
        existing = await db.role.find_unique(where={"name": role_in.name})
        if existing:
            raise HTTPException(status_code=400, detail="El rol ya existe")
        
        return await db.role.create(
            data={
                "name": role_in.name,
                "description": role_in.description,
                "permissions": {
                    "connect": [{"id": pid} for pid in role_in.permission_ids]
                }
            },
            include={"permissions": True}
        )
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error al crear rol: {str(e)}")
        raise HTTPException(status_code=500, detail="Error interno al crear el rol")

@router.get(
    "/permissions", 
    response_model=List[PermissionOut], 
    dependencies=[Depends(require_manage_users)],
    responses={500: {"description": "Error interno"}}
)
async def list_permissions(db: Annotated[Prisma, Depends(get_db)]):
    """Lista todos los permisos disponibles."""
    try:
        return await db.permission.find_many()
    except Exception as e:
        logger.error(f"Error al listar permisos: {str(e)}")
        raise HTTPException(status_code=500, detail="Error al recuperar los permisos")

@router.post(
    "/permissions", 
    response_model=PermissionOut, 
    status_code=status.HTTP_201_CREATED,
    responses={
        400: {"description": "El permiso ya existe"},
        500: {"description": "Error interno"}
    }
)
async def create_permission(
    perm_in: PermissionCreate,
    db: Annotated[Prisma, Depends(get_db)],
    _ = Depends(require_all_access),
    __ = Depends(log_user_action("create_permission"))
):
    """Crea un nuevo permiso granular (Solo ADMIN)."""
    try:
        existing = await db.permission.find_unique(where={"name": perm_in.name})
        if existing:
            raise HTTPException(status_code=400, detail="El permiso ya existe")
        
        return await db.permission.create(
            data={
                "name": perm_in.name,
                "description": perm_in.description
            }
        )
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error al crear permiso: {str(e)}")
        raise HTTPException(status_code=500, detail="Error interno al crear el permiso")

@router.put(
    "/{role_id}", 
    response_model=RoleOut,
    responses={
        404: {"description": "Rol no encontrado"},
        500: {"description": "Error interno"}
    }
)
async def update_role(
    role_id: str,
    role_in: RoleCreate,
    db: Annotated[Prisma, Depends(get_db)],
    _ = Depends(require_all_access),
    __ = Depends(log_user_action("update_role"))
):
    """Actualiza un rol y sincroniza sus permisos."""
    try:
        role = await db.role.find_unique(where={"id": role_id})
        if not role:
            raise HTTPException(status_code=404, detail="Rol no encontrado")
        
        return await db.role.update(
            where={"id": role_id},
            data={
                "name": role_in.name,
                "description": role_in.description,
                "permissions": {
                    "set": [{"id": pid} for pid in role_in.permission_ids]
                }
            },
            include={"permissions": True}
        )
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error al actualizar rol: {str(e)}")
        raise HTTPException(status_code=500, detail="Error interno al actualizar el rol")

@router.delete(
    "/{role_id}", 
    status_code=status.HTTP_204_NO_CONTENT,
    responses={
        400: {"description": "El rol tiene usuarios asociados"},
        404: {"description": "Rol no encontrado"}
    }
)
async def delete_role(
    role_id: str,
    db: Annotated[Prisma, Depends(get_db)],
    _ = Depends(require_all_access),
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
