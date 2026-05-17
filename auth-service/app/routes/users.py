from typing import List, Annotated
from fastapi import APIRouter, Depends, HTTPException, status
from prisma import Prisma

from app.database import get_db
from app.dependencies import require_manage_users, log_user_action
from app.schemas.user import UserOut
from pydantic import BaseModel

router = APIRouter(prefix="/users", tags=["User Management"])

class UserUpdateRole(BaseModel):
    role_name: str

@router.get("/", response_model=List[UserOut])
async def list_users(
    db: Annotated[Prisma, Depends(get_db)],
    _ = Depends(require_manage_users)
):
    """Lista todos los usuarios con sus roles (Solo gestores de usuarios)."""
    return await db.user.find_many(include={"role": {"include": {"permissions": True}}})

@router.patch("/{user_id}/role", response_model=UserOut)
async def update_user_role(
    user_id: str,
    role_update: UserUpdateRole,
    db: Annotated[Prisma, Depends(get_db)],
    _ = Depends(require_manage_users),
    __ = Depends(log_user_action("update_user_role"))
):
    """Cambia el rol de un usuario existente (Solo gestores de usuarios)."""
    # Verificar que el rol existe
    role = await db.role.find_unique(where={"name": role_update.role_name})
    if not role:
        raise HTTPException(status_code=400, detail=f"El rol '{role_update.role_name}' no existe")
    
    # Verificar que el usuario existe
    user = await db.user.find_unique(where={"id": user_id})
    if not user:
        raise HTTPException(status_code=404, detail="Usuario no encontrado")

    return await db.user.update(
        where={"id": user_id},
        data={"role_id": role.id},
        include={"role": {"include": {"permissions": True}}}
    )
