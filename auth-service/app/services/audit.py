import json
import logging
from typing import Any, Optional

from prisma import Prisma
from starlette.requests import Request

from app.utils.request import get_client_ip, get_user_agent

logger = logging.getLogger("auth-service.audit")

MAX_DETAILS_LENGTH = 2000


async def record_audit(
    db: Prisma,
    action: str,
    *,
    user_id: Optional[str] = None,
    request: Optional[Request] = None,
    details: Optional[dict[str, Any]] = None,
    endpoint: Optional[str] = None,
    ip_address: Optional[str] = None,
    user_agent: Optional[str] = None,
) -> None:
    """
    Registra un evento de auditoría. Nunca lanza excepciones: un fallo de auditoría
    no debe romper el flujo principal, pero queda registrado en los logs.
    """
    if request is not None:
        endpoint = endpoint or request.url.path
        ip_address = ip_address or get_client_ip(request)
        user_agent = user_agent or get_user_agent(request)

    serialized = None
    if details:
        serialized = json.dumps(details, ensure_ascii=False, default=str)[:MAX_DETAILS_LENGTH]

    try:
        await db.auditlog.create(
            data={
                "user_id": user_id,
                "action": action,
                "endpoint": endpoint,
                "ip_address": ip_address,
                "user_agent": user_agent,
                "details": serialized,
            }
        )
    except Exception as e:
        logger.error(f"Error al registrar auditoría '{action}': {e}")
