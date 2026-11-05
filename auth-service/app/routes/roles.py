from typing import Annotated, List

from fastapi import APIRouter, Depends, HTTPException, Request, status
from prisma import Prisma
from prisma.models import User

from app.config import settings
from app.core import endpoints
from app.database import get_db
from app.dependencies import is_admin, require_manage_users
from app.schemas.role import RoleCreate, RoleOut
from app.services.audit import record_audit

router = APIRouter(prefix=endpoints.ROLES_PREFIX, tags=["Gestión de Roles"])

Db = Annotated[Prisma, Depends(get_db)]
Manager = Annotated[User, Depends(require_manage_users)]


def _ensure_can_touch_role(actor: User, *names: str) -> None:
    """Un gestor que no es admin no puede crear, modificar ni borrar roles privilegiados."""
    if is_admin(actor):
        return
    if any(n in settings.privileged_role_names for n in names):
        raise HTTPException(status_code=403, detail="Solo un administrador puede gestionar roles privilegiados")


async def _get_role_or_404(db: Prisma, role_id: str):
    role = await db.role.find_unique(where={"id": role_id})
    if not role:
        raise HTTPException(status_code=404, detail="Rol no encontrado")
    return role


@router.get(endpoints.ROLES_LIST, response_model=List[RoleOut], summary="Listar roles")
async def list_roles(db: Db, _: Manager):
    return await db.role.find_many(order={"name": "asc"})


@router.get(endpoints.ROLES_DETAIL, response_model=RoleOut, summary="Obtener rol")
async def get_role(role_id: str, db: Db, _: Manager):
    return await _get_role_or_404(db, role_id)


@router.post(endpoints.ROLES_CREATE, response_model=RoleOut, status_code=status.HTTP_201_CREATED,
             summary="Crear rol")
async def create_role(role_in: RoleCreate, request: Request, db: Db, actor: Manager):
    _ensure_can_touch_role(actor, role_in.name)
    if await db.role.find_unique(where={"name": role_in.name}):
        raise HTTPException(status_code=409, detail="El rol ya existe")
    role = await db.role.create(data={"name": role_in.name, "description": role_in.description})
    await record_audit(db, "role_created", user_id=actor.id, request=request, details={"role": role.name})
    return role


@router.put(endpoints.ROLES_DETAIL, response_model=RoleOut, summary="Actualizar rol",
            description="Los roles admin y por defecto no pueden renombrarse.")
async def update_role(role_id: str, role_in: RoleCreate, request: Request, db: Db, actor: Manager):
    role = await _get_role_or_404(db, role_id)
    _ensure_can_touch_role(actor, role.name, role_in.name)

    if role_in.name != role.name:
        if role.name in settings.protected_role_names:
            raise HTTPException(status_code=400, detail=f"El rol '{role.name}' no puede renombrarse")
        if await db.role.find_unique(where={"name": role_in.name}):
            raise HTTPException(status_code=409, detail="Ya existe un rol con ese nombre")

    updated = await db.role.update(
        where={"id": role_id}, data={"name": role_in.name, "description": role_in.description}
    )
    await record_audit(db, "role_updated", user_id=actor.id, request=request,
                       details={"role_id": role_id, "from": role.name, "to": role_in.name})
    return updated


@router.delete(endpoints.ROLES_DETAIL, status_code=status.HTTP_204_NO_CONTENT, summary="Eliminar rol",
               description="Solo si no tiene usuarios asignados y no es un rol protegido.")
async def delete_role(role_id: str, request: Request, db: Db, actor: Manager):
    role = await _get_role_or_404(db, role_id)
    _ensure_can_touch_role(actor, role.name)
    if role.name in settings.protected_role_names:
        raise HTTPException(status_code=400, detail=f"El rol '{role.name}' es protegido y no puede eliminarse")
    if await db.user.count(where={"role_id": role_id}) > 0:
        raise HTTPException(status_code=400, detail="No se puede eliminar un rol que tiene usuarios asignados")
    await db.role.delete(where={"id": role_id})
    await record_audit(db, "role_deleted", user_id=actor.id, request=request, details={"role": role.name})
