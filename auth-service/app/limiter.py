from slowapi import Limiter

from app.config import settings
from app.utils.request import get_client_ip

# Instancia central para evitar importaciones circulares.
# La clave es la IP real del cliente (respetando TRUSTED_PROXY_COUNT).
limiter = Limiter(
    key_func=get_client_ip,
    storage_uri=settings.rate_limit_storage_uri,
    default_limits=[settings.rate_limit_default],
)
