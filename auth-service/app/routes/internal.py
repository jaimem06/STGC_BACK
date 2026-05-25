from fastapi import APIRouter, Depends, HTTPException, Security, status, BackgroundTasks
from fastapi.security import APIKeyHeader
from app.config import settings
from app.database import db
from app.schemas.audit import AuditCreate

router = APIRouter(prefix="/internal", tags=["Servicios Internos"])

API_KEY_NAME = "X-Internal-Api-Key"
api_key_header = APIKeyHeader(name=API_KEY_NAME, auto_error=False)

async def get_internal_api_key(
    api_key: str = Security(api_key_header),
):
    if api_key == settings.internal_api_key:
        return api_key
    raise HTTPException(
        status_code=status.HTTP_403_FORBIDDEN,
        detail="No se pudieron validar las credenciales internas",
    )

async def _record_audit_log(user_id: str, action: str, endpoint: str, ip_address: str):
    try:
        await db.auditlog.create(
            data={
                "user_id": user_id,
                "action": action,
                "endpoint": endpoint,
                "ip_address": ip_address,
            }
        )
    except Exception as e:
        # Registramos en consola como respaldo
        print(f"Error en la tarea en segundo plano para el registro de auditoría: {e}")

@router.post(
    "/audit", 
    status_code=status.HTTP_201_CREATED,
    summary="Registrar Log de Auditoría",
    description="""
Permite que otros microservicios del ecosistema STGC registren eventos de auditoría de forma centralizada.

### Instrucciones de uso:
1. **Seguridad**: Se requiere el header `X-Internal-Api-Key` con la clave secreta compartida definida en las variables de entorno.
2. **Asincronismo**: La escritura en la base de datos se realiza en segundo plano utilizando `BackgroundTasks`. El endpoint responde de inmediato con un estado de éxito, lo que evita retrasos en el microservicio solicitante.
3. **Casos de uso**: Ideal para registrar creación de pedidos, movimientos de inventario, pesajes de café, etc., asociándolos siempre a un ID de usuario del servicio de autenticación.
    """
)
async def create_audit_log(
    audit_data: AuditCreate,
    background_tasks: BackgroundTasks,
    _ = Depends(get_internal_api_key)
):
    """
    Punto de conexión para que otros microservicios registren auditorías de forma centralizada.
    """
    background_tasks.add_task(
        _record_audit_log, 
        audit_data.user_id, 
        audit_data.action, 
        audit_data.endpoint, 
        audit_data.ip_address
    )
    return {"status": "success", "message": "Registro de auditoría en cola"}
