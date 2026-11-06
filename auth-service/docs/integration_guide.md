# Guía de integración

Cómo conectar frontends y otros servicios a este `auth-service`. El servicio es la única fuente de identidad: emite los tokens, guarda las sesiones y centraliza la auditoría.

---

## 1. Frontend

### Login

```http
POST /api/auth/login
{ "email": "ana@empresa.com", "password": "..." }
```

Respuesta normal:

```json
{
  "access_token": "eyJ...",
  "refresh_token": "kZ3...",
  "token_type": "bearer",
  "expires_in": 900,
  "refresh_expires_in": 604800
}
```

Si el usuario tiene verificación en dos pasos, la respuesta es `{ "mfa_required": true, "mfa_token": "..." }`. Pide el código de 6 dígitos y envíalo a `POST /api/auth/mfa/verify` con `{ "mfa_token", "code" }` para recibir los tokens.

### Uso de los tokens

- Envía `Authorization: Bearer <access_token>` en cada petición.
- Cuando el access token expire, llama a `POST /api/auth/refresh` con `{ "refresh_token" }`. Recibirás un par nuevo y debes **reemplazar los dos**: el refresh token anterior deja de servir.
- Si `/refresh` responde `401`, la sesión terminó: envía al usuario al login.
- Guarda el refresh token en memoria o en almacenamiento seguro del dispositivo. En web evita `localStorage` si puedes.

### Respuestas a manejar

| Código | `detail` | Qué hacer |
| :--- | :--- | :--- |
| `401` | `SESSION_INVALIDATED` | La sesión se cerró en otro lugar. Intenta `/refresh`; si falla, al login. |
| `403` | `EMAIL_NOT_VERIFIED` | Ofrece reenviar el correo con `/auth/resend-verification`. |
| `403` | `Cuenta suspendida...` | Muestra el mensaje. |
| `422` | `{ message, errors[] }` | La contraseña no cumple la política: muestra `errors`. |
| `423` | Bloqueo temporal | Demasiados intentos fallidos; espera o pide a un gestor que desbloquee. |
| `429` | Rate limit | Espera los segundos de `Retry-After`. |

### Páginas que el frontend debe tener

- `FRONTEND_RESET_PASSWORD_PATH` (por defecto `/reset-password`): lee `?token=` y llama a `POST /api/auth/reset-password` con `{ token, new_password }`. Sirve también para invitaciones.
- `FRONTEND_VERIFY_EMAIL_PATH` (por defecto `/verify-email`): lee `?token=`, pide la contraseña de la cuenta y llama a `POST /api/auth/verify-email` con `{ token, password }`. Pedir la contraseña impide que alguien registre tu email antes que tú y lo active con su contraseña. Si no coincide, ofrece la recuperación de contraseña.

### Activar MFA desde el perfil

1. `POST /api/auth/mfa/setup` con `{ password }` devuelve `secret` y `otpauth_uri`. Muestra la URI como código QR.
2. El usuario escanea el QR y escribe el código: `POST /api/auth/mfa/enable` con `{ code }`.

---

## 2. Otros servicios del proyecto

Hay dos formas de validar un token. Elige según lo crítica que sea la operación.

### Opción A: validación local (rápida)

Cada servicio verifica la firma del JWT sin llamar al auth-service.

**Con RS256 (recomendado para varios servicios):** configura `JWT_ALGORITHM=RS256` con `JWT_PRIVATE_KEY` y `JWT_PUBLIC_KEY` en el auth-service. Los demás servicios descargan la clave pública de `GET /.well-known/jwks.json` y no necesitan ningún secreto.

```python
import jwt

jwks_client = jwt.PyJWKClient("https://auth.midominio.com/.well-known/jwks.json")


def decode(token: str) -> dict:
    key = jwks_client.get_signing_key_from_jwt(token).key
    claims = jwt.decode(token, key, algorithms=["RS256"],
                        audience="mi-proyecto", issuer="auth-service")
    if claims.get("type") != "access":
        raise jwt.InvalidTokenError("tipo de token incorrecto")
    return claims
```

**Con HS256:** comparte `SECRET_KEY` y usa `jwt.decode(token, SECRET_KEY, algorithms=["HS256"], audience=..., issuer=...)`. Cualquier servicio con el secreto podría emitir tokens, por eso RS256 es preferible.

Valida siempre `audience`, `issuer` y `type == "access"`. Los claims disponibles son `sub` (id de usuario), `role`, `sid` (sesión), `exp`, `iat` y `jti`.

> **Límite de la validación local:** un token sigue pareciendo válido hasta que expira, aunque el usuario cierre sesión, cambie la contraseña o sea suspendido. Con `ACCESS_TOKEN_EXPIRE_MINUTES=15` ese margen es como mucho de 15 minutos.

### Opción B: introspección (estado en tiempo real)

Para operaciones sensibles, pregunta al auth-service. La respuesta refleja logout, suspensiones y desactivaciones al instante.

```python
import httpx

async def introspect(token: str) -> dict:
    async with httpx.AsyncClient(timeout=5) as client:
        resp = await client.post(
            f"{AUTH_SERVICE_URL}/api/internal/introspect",
            json={"token": token},
            headers={"X-Internal-Api-Key": INTERNAL_API_KEY},
        )
        resp.raise_for_status()
        data = resp.json()
    if not data["active"]:
        raise PermissionError("Token no activo")
    return data  # sub, email, role, sid, status, exp
```

### Datos de un usuario

`GET /api/internal/users/{id}` con la misma cabecera devuelve `id`, `email`, nombres, `role` y `status`.

### Auditoría centralizada

```http
POST /api/internal/audit
X-Internal-Api-Key: ...
{
  "user_id": "uuid-del-usuario",
  "action": "CREAR_PRODUCTO",
  "endpoint": "/api/productos",
  "ip_address": "203.0.113.7",
  "details": { "sku": "A-100" }
}
```

`user_id` es opcional y, si se envía, debe existir. La escritura se hace en segundo plano.

> Los endpoints `/internal/*` son solo para comunicación entre servidores. Nunca envíes `INTERNAL_API_KEY` a un navegador o app móvil.

---

## 3. Roles por proyecto

- Los roles se crean con `python seed.py` (`ADMIN_ROLE_NAME`, `DEFAULT_ROLE_NAME`, `SEED_EXTRA_ROLES`) o con la API `/roles`.
- `USER_MANAGER_ROLES` define qué roles, además del admin, gestionan usuarios y roles. Esos gestores no pueden otorgar ni modificar roles privilegiados.
- Un gestor sí tiene control total sobre las cuentas no privilegiadas: puede cambiar su contraseña, email o MFA y asignarles cualquier rol no privilegiado. Si un rol de negocio es muy poderoso en tus servicios, inclúyelo en `USER_MANAGER_ROLES` para que solo el admin lo otorgue.
- Cambiar el rol de un usuario cierra sus sesiones, así los demás servicios nunca ven el rol antiguo en un token vigente.
- Los permisos de negocio se deciden en cada servicio a partir del claim `role`.

## 4. Migrar desde la versión anterior

- Ejecuta `alembic stamp 0001` y luego `alembic upgrade head`. Si ya habías aplicado a mano el SQL de seguridad, basta con `alembic stamp 0002`. Solo se tocan las tablas de autenticación.
- Todos los usuarios deberán iniciar sesión de nuevo, porque desaparece `session_token`.
- El login ahora también devuelve `refresh_token`. Los frontends antiguos siguen funcionando, pero su sesión durará `ACCESS_TOKEN_EXPIRE_MINUTES` hasta que implementen `/refresh`.
- `GET /users/` devuelve como máximo `limit` usuarios (50 por defecto, hasta 200). Usa `skip` y `X-Total-Count` para paginar.
- Duplicados en altas o edición responden `409` en lugar de `400`.
