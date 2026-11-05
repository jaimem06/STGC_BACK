from typing import Optional

from starlette.requests import Request

from app.config import settings

MAX_USER_AGENT_LENGTH = 255


def get_client_ip(request: Request) -> str:
    """
    IP real del cliente.

    X-Forwarded-For solo se usa si hay proxies de confianza (TRUSTED_PROXY_COUNT).
    Cada proxy añade la IP que ve al final de la lista, así que la IP fiable es la
    que está N posiciones desde el final; las anteriores las controla el cliente.
    """
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
    ua = request.headers.get("user-agent")
    return ua[:MAX_USER_AGENT_LENGTH] if ua else None
