from typing import List, Annotated
from fastapi import APIRouter, Depends, HTTPException, status
from prisma import Prisma

from app.database import get_db
from app.dependencies import require_manage_users, log_user_action
from app.schemas.user import RoleOut, PermissionOut
from pydantic import BaseModel

router = APIRouter(prefix="/roles", tags=["Role Management"])

class RoleCreate(BaseModel):
    name: str
    description: str | None = None
    permission_ids: List[str] = []

class PermissionCreate(BaseModel):
    name: str
    description: str | None = None

@router.get("/", response_model=List[RoleOut], dependencies=[Depends(require_manage_users)])
async def list_roles(db: Annotated[Prisma, Depends(get_db)]):
    """Lista todos los roles con sus permisos."""
    return await db.role.find_many(include={"permissions": True})

@router.post("/", response_model=RoleOut, status_code=status.HTTP_201_CREATED)
async def create_role(
    role_in: RoleCreate,
    db: Annotated[Prisma, Depends(get_db)],
    _ = Depends(require_manage_users),
    __ = Depends(log_user_action("create_role"))
):
    """Crea un nuevo rol y le asigna permisos."""
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

@router.get("/permissions", response_model=List[PermissionOut], dependencies=[Depends(require_manage_users)])
async def list_permissions(db: Annotated[Prisma, Depends(get_db)]):
    """Lista todos los permisos disponibles en el sistema."""
    return await db.permission.find_many()

@router.post("/permissions", response_model=PermissionOut, status_code=status.HTTP_201_CREATED)
async def create_permission(
    perm_in: PermissionCreate,
    db: Annotated[Prisma, Depends(get_db)],
    _ = Depends(require_manage_users),
    __ = Depends(log_user_action("create_permission"))
):
    """Crea un nuevo permiso granular."""
    existing = await db.permission.find_unique(where={"name": perm_in.name})
    if existing:
        raise HTTPException(status_code=400, detail="El permiso ya existe")
    
    return await db.permission.create(
        data={
            "name": perm_in.name,
            "description": perm_in.description
        }
    )

@router.put("/{role_id}", response_model=RoleOut)
async def update_role(
    role_id: str,
    role_in: RoleCreate,
    db: Annotated[Prisma, Depends(get_db)],
    _ = Depends(require_manage_users),
    __ = Depends(log_user_action("update_role"))
):
    """Actualiza un rol y sincroniza sus permisos."""
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

@router.delete("/{role_id}", status_code=status.HTTP_204_NO_CONTENT)
async def delete_role(
    role_id: str,
    db: Annotated[Prisma, Depends(get_db)],
    _ = Depends(require_manage_users),
    __ = Depends(log_user_action("delete_role"))
):
    """Elimina un rol (solo si no tiene usuarios asociados)."""
    users_count = await db.user.count(where={"role_id": role_id})
    if users_count > 0:
        raise HTTPException(status_code=400, detail="No se puede eliminar un rol que tiene usuarios asignados")
    
    await db.role.delete(where={"id": role_id})
    return None
