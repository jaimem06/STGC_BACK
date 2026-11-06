"""
Endpoints machine-to-machine para otros servicios del proyecto.
Protegidos con la cabecera X-Internal-Api-Key; nunca deben exponerse a navegadores.
"""

from fastapi import APIRouter, BackgroundTasks, Depends, HTTPException, Request, Security, status
from fastapi.security import APIKeyHeader

from app.config import settings
from app.core import endpoints
from app.dependencies import Db, get_active_suspension_until
from app.models import UserStatus
from app.schemas.audit import AuditCreate, IntrospectRequest, IntrospectResponse
from app.schemas.user import UserPublic
from app.security import ACCESS_TOKEN_TYPE, constant_time_equals, decode_token
from app.services.audit import record_audit_background
from app.services.sessions import get_session, session_is_active
from app.services.users import get_user

API_KEY_NAME = "X-Internal-Api-Key"
api_key_header = APIKeyHeader(name=API_KEY_NAME, auto_error=False)


async def get_internal_api_key(api_key: str = Security(api_key_header)):
    # Comparación en tiempo constante para no filtrar la clave por timing.
    if constant_time_equals(api_key, settings.internal_api_key):
        return api_key
    raise HTTPException(
        status_code=status.HTTP_403_FORBIDDEN,
        detail="No se pudieron validar las credenciales internas",
    )


router = APIRouter(
    prefix=endpoints.INTERNAL_PREFIX,
    tags=["Servicios Internos"],
    dependencies=[Depends(get_internal_api_key)],
)


@router.post(
    endpoints.INTERNAL_AUDIT,
    status_code=status.HTTP_201_CREATED,
    summary="Registrar evento de auditoría",
    description=(
        f"Otros servicios registran eventos de forma centralizada. Requiere `{API_KEY_NAME}`. "
        "La escritura se hace en segundo plano."
    ),
)
async def create_audit_log(audit_data: AuditCreate, background_tasks: BackgroundTasks,
                           request: Request, db: Db):
    if audit_data.user_id and not await get_user(db, audit_data.user_id):
        raise HTTPException(status_code=400, detail="user_id no existe")
    background_tasks.add_task(
        record_audit_background,
        audit_data.action,
        user_id=audit_data.user_id,
        endpoint=audit_data.endpoint,
        ip_address=audit_data.ip_address,
        # source_ip va al final para que el llamante no pueda sobrescribirlo.
        details={**(audit_data.details or {}),
                 "source_ip": request.client.host if request.client else None},
    )
    return {"status": "success", "message": "Registro de auditoría en cola"}


@router.post(
    endpoints.INTERNAL_INTROSPECT,
    response_model=IntrospectResponse,
    summary="Validar un access token",
    description=(
        "Verifica firma y expiración y además consulta la base de datos: detecta tokens de sesiones "
        "cerradas, usuarios suspendidos o desactivados. Devuelve `active: false` ante cualquier fallo."
    ),
)
async def introspect(data: IntrospectRequest, db: Db):
    inactive = IntrospectResponse(active=False)
    payload = decode_token(data.token, ACCESS_TOKEN_TYPE)
    if payload is None or not payload.get("sid"):
        return inactive
    session = await get_session(db, payload["sid"])
    if session is None or session.user_id != payload["sub"] or not session_is_active(session):
        return inactive
    user = session.user
    if user.status != UserStatus.ACTIVO or get_active_suspension_until(user):
        return inactive
    return IntrospectResponse(
        active=True, sub=user.id, email=user.email, role=user.role.name,
        sid=session.id, status=user.status.value, exp=int(payload["exp"]),
    )


@router.get(endpoints.INTERNAL_USER, response_model=UserPublic, summary="Datos básicos de un usuario")
async def get_user_public(user_id: str, db: Db):
    user = await get_user(db, user_id)
    if not user:
        raise HTTPException(status_code=404, detail="Usuario no encontrado")
    return user
