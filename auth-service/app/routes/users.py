from typing import List, Annotated, Optional
import logging
from fastapi import APIRouter, Depends, HTTPException, status
from prisma import Prisma, errors
from pydantic import BaseModel, EmailStr

from app.database import get_db
from app.dependencies import require_manage_users, log_user_action
from app.schemas.user import UserOut, UserStatus
from app.core import endpoints
from app.security import get_password_hash

logger = logging.getLogger(__name__)
router = APIRouter(prefix=endpoints.USERS_PREFIX, tags=["Gestión de Usuarios"])

class UserUpdate(BaseModel):
    role_name: Optional[str] = None
    status: Optional[UserStatus] = None
    email: Optional[EmailStr] = None
    phone_number: Optional[str] = None
    identifier: Optional[str] = None
    password: Optional[str] = None

@router.get(
    endpoints.USERS_LIST, 
    response_model=List[UserOut],
    summary="Listar Usuarios",
    description="Devuelve la lista completa de empleados con su rol y estado. Solo accesible por gestores.",
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
        return await db.user.find_many(include={"role": True})
    except Exception as e:
        logger.error(f"Error al listar usuarios: {str(e)}")
        raise HTTPException(status_code=500, detail="Error al recuperar la lista de usuarios")

@router.patch(
    endpoints.USERS_UPDATE, 
    response_model=UserOut,
    summary="Actualizar Usuario",
    description="Modifica los datos de un empleado (email, teléfono, identificación, rol, estado o contraseña).",
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
    """Actualiza el rol, estado, email, teléfono, identificación o contraseña de un usuario existente."""
    try:
        # 1. Verificar que el usuario existe
        user = await db.user.find_unique(where={"id": user_id})
        if not user:
            raise HTTPException(status_code=404, detail="Usuario no encontrado")

        update_data = {}
        
        # 2. Validar y preparar datos de actualización
        if user_update.role_name:
            role = await db.role.find_unique(where={"name": user_update.role_name})
            if not role:
                raise HTTPException(status_code=400, detail=f"El rol '{user_update.role_name}' no existe")
            update_data["role_id"] = role.id
        
        if user_update.status:
            update_data["status"] = user_update.status

        if user_update.email:
            # Verificar que el email no esté en uso por otro usuario
            existing_email = await db.user.find_first(
                where={
                    "email": user_update.email,
                    "NOT": {"id": user_id}
                }
            )
            if existing_email:
                raise HTTPException(status_code=400, detail="El email ya está en uso por otro usuario")
            update_data["email"] = user_update.email

        if user_update.phone_number:
            existing_phone = await db.user.find_first(
                where={
                    "phone_number": user_update.phone_number,
                    "NOT": {"id": user_id}
                }
            )
            if existing_phone:
                raise HTTPException(status_code=400, detail="El número de teléfono ya está en uso por otro usuario")
            update_data["phone_number"] = user_update.phone_number

        if user_update.identifier:
            existing_id = await db.user.find_first(
                where={
                    "identifier": user_update.identifier,
                    "NOT": {"id": user_id}
                }
            )
            if existing_id:
                raise HTTPException(status_code=400, detail="El identificador (cédula/ID) ya está en uso por otro usuario")
            update_data["identifier"] = user_update.identifier

        if user_update.password:
            update_data["password_hash"] = get_password_hash(user_update.password)

        if not update_data:
            raise HTTPException(status_code=400, detail="No se proporcionaron datos válidos para actualizar")

        # 4. Ejecutar actualización
        return await db.user.update(
            where={"id": user_id},
            data=update_data,
            include={"role": True}
        )

    except HTTPException:
        raise
    except errors.PrismaError as e:
        logger.error(f"Error de base de datos al actualizar usuario: {str(e)}")
        raise HTTPException(status_code=500, detail="Error al actualizar el usuario en la base de datos")
    except Exception as e:
        logger.error(f"Error inesperado al actualizar usuario: {str(e)}")
        raise HTTPException(status_code=500, detail="Error interno del servidor")
