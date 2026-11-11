import hmac
import ipaddress
from typing import Optional

from starlette.requests import Request

from app.config import settings

MAX_USER_AGENT_LENGTH = 255

# Cabeceras con las que un backend de confianza (BFF) reenvía los datos del usuario final.
# Solo se aceptan si la petición trae una X-Internal-Api-Key válida.
CLIENT_IP_HEADER = "x-client-ip"
CLIENT_USER_AGENT_HEADER = "x-client-user-agent"


def _from_trusted_backend(request: Request) -> bool:
    key = request.headers.get("x-internal-api-key")
    if not key or not settings.internal_api_key:
        return False
    return hmac.compare_digest(key.encode("utf-8"), settings.internal_api_key.encode("utf-8"))


def _forwarded_client_ip(request: Request) -> Optional[str]:
    value = request.headers.get(CLIENT_IP_HEADER, "").strip()
    if not value or not _from_trusted_backend(request):
        return None
    try:
        return str(ipaddress.ip_address(value))
    except ValueError:
        return None


def get_client_ip(request: Request) -> str:
    """
    IP real del cliente.

    1. Un backend de confianza (con X-Internal-Api-Key válida) puede indicarla en X-Client-IP.
       Así el rate limit, las sesiones y la auditoría ven al usuario final y no al servidor
       intermedio, por el que pasan todos los usuarios.
    2. X-Forwarded-For solo se usa si hay proxies de confianza (TRUSTED_PROXY_COUNT).
       Cada proxy añade la IP que ve al final de la lista, así que la IP fiable es la
       que está N posiciones desde el final; las anteriores las controla el cliente.
    """
    forwarded_by_backend = _forwarded_client_ip(request)
    if forwarded_by_backend:
        return forwarded_by_backend
    direct = request.client.host if request.client else "unknown"
    hops = settings.trusted_proxy_count
    if hops <= 0:
        return direct
    forwarded = request.headers.get("x-forwarded-for")
    if not forwarded:
        return direct
    chain = [ip.strip() for ip in forwarded.split(",") if ip.strip()]
    # Si la cadena es más corta que los proxies declarados, no es fiable: se usa la conexión directa.
    if len(chain) < hops:
        return direct
    return chain[-hops]


def get_user_agent(request: Optional[Request]) -> Optional[str]:
    if request is None:
        return None
    ua = None
    if request.headers.get(CLIENT_USER_AGENT_HEADER) and _from_trusted_backend(request):
        ua = request.headers.get(CLIENT_USER_AGENT_HEADER)
    ua = ua or request.headers.get("user-agent")
    return ua[:MAX_USER_AGENT_LENGTH] if ua else None
