from typing import Annotated, List

from fastapi import APIRouter, Depends, HTTPException, Request, status
from sqlalchemy import delete, func, select
from sqlalchemy.ext.asyncio import AsyncSession

from app.config import settings
from app.core import endpoints
from app.dependencies import Db, is_admin, require_manage_users
from app.models import Role, User
from app.schemas.role import RoleCreate, RoleOut
from app.services.audit import record_audit
from app.services.users import get_role_by_name

router = APIRouter(prefix=endpoints.ROLES_PREFIX, tags=["Gestión de Roles"])

Manager = Annotated[User, Depends(require_manage_users)]


def _ensure_can_touch_role(actor: User, *names: str) -> None:
    """Un gestor que no es admin no puede crear, modificar ni borrar roles privilegiados."""
    if is_admin(actor):
        return
    if any(n in settings.privileged_role_names for n in names):
        raise HTTPException(status_code=403, detail="Solo un administrador puede gestionar roles privilegiados")


async def _get_role_or_404(db: AsyncSession, role_id: str) -> Role:
    role = await db.get(Role, role_id)
    if not role:
        raise HTTPException(status_code=404, detail="Rol no encontrado")
    return role


@router.get(endpoints.ROLES_LIST, response_model=List[RoleOut], summary="Listar roles")
async def list_roles(db: Db, _: Manager):
    return list(await db.scalars(select(Role).order_by(Role.name)))


@router.get(endpoints.ROLES_DETAIL, response_model=RoleOut, summary="Obtener rol")
async def get_role(role_id: str, db: Db, _: Manager):
    return await _get_role_or_404(db, role_id)


@router.post(endpoints.ROLES_CREATE, response_model=RoleOut, status_code=status.HTTP_201_CREATED,
             summary="Crear rol")
async def create_role(role_in: RoleCreate, request: Request, db: Db, actor: Manager):
    _ensure_can_touch_role(actor, role_in.name)
    if await get_role_by_name(db, role_in.name):
        raise HTTPException(status_code=409, detail="El rol ya existe")
    role = Role(name=role_in.name, description=role_in.description)
    db.add(role)
    await db.commit()
    await record_audit(db, "role_created", user_id=actor.id, request=request, details={"role": role.name})
    return role


@router.put(endpoints.ROLES_DETAIL, response_model=RoleOut, summary="Actualizar rol",
            description="Los roles admin y por defecto no pueden renombrarse.")
async def update_role(role_id: str, role_in: RoleCreate, request: Request, db: Db, actor: Manager):
    role = await _get_role_or_404(db, role_id)
    old_name = role.name
    _ensure_can_touch_role(actor, old_name, role_in.name)

    if role_in.name != old_name:
        if old_name in settings.protected_role_names:
            raise HTTPException(status_code=400, detail=f"El rol '{old_name}' no puede renombrarse")
        if await get_role_by_name(db, role_in.name):
            raise HTTPException(status_code=409, detail="Ya existe un rol con ese nombre")

    role.name = role_in.name
    role.description = role_in.description
    await db.commit()
    await record_audit(db, "role_updated", user_id=actor.id, request=request,
                       details={"role_id": role_id, "from": old_name, "to": role_in.name})
    return role


@router.delete(endpoints.ROLES_DETAIL, status_code=status.HTTP_204_NO_CONTENT, summary="Eliminar rol",
               description="Solo si no tiene usuarios asignados y no es un rol protegido.")
async def delete_role(role_id: str, request: Request, db: Db, actor: Manager):
    role = await _get_role_or_404(db, role_id)
    name = role.name
    _ensure_can_touch_role(actor, name)
    if name in settings.protected_role_names:
        raise HTTPException(status_code=400, detail=f"El rol '{name}' es protegido y no puede eliminarse")
    users = await db.scalar(select(func.count()).select_from(User).where(User.role_id == role_id))
    if users:
        raise HTTPException(status_code=400, detail="No se puede eliminar un rol que tiene usuarios asignados")
    await db.execute(delete(Role).where(Role.id == role_id).execution_options(synchronize_session=False))
    await db.commit()
    await record_audit(db, "role_deleted", user_id=actor.id, request=request, details={"role": name})
