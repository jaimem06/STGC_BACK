from starlette.middleware.base import BaseHTTPMiddleware
from starlette.requests import Request
from starlette.responses import JSONResponse
from starlette.types import ASGIApp, Message, Receive, Scope, Send

from app.config import settings
from app.core import endpoints

_BASE_HEADERS = {
    "X-Content-Type-Options": "nosniff",
    "X-Frame-Options": "DENY",
    "Referrer-Policy": "no-referrer",
    "Cross-Origin-Opener-Policy": "same-origin",
    "Permissions-Policy": "geolocation=(), microphone=(), camera=(), payment=()",
}

# Las respuestas de la API son JSON: no necesitan cargar ningún recurso.
_API_CSP = "default-src 'none'; frame-ancestors 'none'"
_TOO_LARGE = "Petición demasiado grande"


class SecurityHeadersMiddleware(BaseHTTPMiddleware):
    async def dispatch(self, request: Request, call_next):
        response = await call_next(request)
        for name, value in _BASE_HEADERS.items():
            response.headers.setdefault(name, value)

        path = request.url.path
        is_docs = path.startswith("/static") or path == endpoints.DOCS
        if not is_docs:
            response.headers.setdefault("Content-Security-Policy", _API_CSP)
        if path.startswith(settings.api_prefix):
            # Nunca cachear tokens ni datos de usuario en proxies o navegadores.
            response.headers["Cache-Control"] = "no-store"
            response.headers["Pragma"] = "no-cache"
        if settings.is_production:
            response.headers.setdefault(
                "Strict-Transport-Security", "max-age=31536000; includeSubDomains"
            )
        return response


class BodySizeLimitMiddleware:
    """
    Limita el tamaño del cuerpo de las peticiones. Comprueba Content-Length y además
    cuenta los bytes recibidos, así que las peticiones chunked tampoco lo superan.
    """

    def __init__(self, app: ASGIApp):
        self.app = app

    async def __call__(self, scope: Scope, receive: Receive, send: Send) -> None:
        if scope["type"] != "http":
            await self.app(scope, receive, send)
            return

        limit = settings.max_request_body_bytes
        declared = dict(scope.get("headers", [])).get(b"content-length")
        if declared is not None and (not declared.isdigit() or int(declared) > limit):
            await self._reject(scope, receive, send)
            return

        received = 0
        response_started = False
        rejected = False

        async def tracking_send(message: Message) -> None:
            nonlocal response_started
            if rejected:
                return  # ya respondimos 413: se descarta lo que la app intente enviar
            if message["type"] == "http.response.start":
                response_started = True
            await send(message)

        async def limited_receive() -> Message:
            nonlocal received, rejected, response_started
            if rejected:
                return {"type": "http.disconnect"}
            message = await receive()
            if message["type"] == "http.request":
                received += len(message.get("body", b""))
                if received > limit:
                    # Se responde 413 directamente y se simula una desconexión para que la
                    # app deje de leer; así funciona aunque haya middlewares intermedios.
                    if not response_started:
                        response_started = True
                        await self._reject(scope, receive, send)
                    rejected = True
                    return {"type": "http.disconnect"}
            return message

        await self.app(scope, limited_receive, tracking_send)

    @staticmethod
    async def _reject(scope: Scope, receive: Receive, send: Send) -> None:
        response = JSONResponse(status_code=413, content={"detail": _TOO_LARGE}, headers=_BASE_HEADERS)
        await response(scope, receive, send)
