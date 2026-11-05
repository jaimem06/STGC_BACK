# Auth Service

Microservicio reutilizable de **autenticación, sesiones, roles (RBAC) y auditoría**. Se conecta a cualquier proyecto que necesite login, gestión de usuarios y control de acceso sin reescribir esa lógica. No contiene modelos de negocio.

## Stack

- **Framework:** FastAPI + Starlette
- **ORM:** Prisma Client Python sobre PostgreSQL
- **Tokens:** PyJWT (HS256 o RS256 con JWKS)
- **Contraseñas:** bcrypt (12 rondas)
- **Rate limiting:** slowapi (memoria o Redis)

## Funcionalidades

**Autenticación**
- Login con access token corto (15 min) y refresh token rotativo (7 días).
- Detección de reutilización de refresh tokens: si se presenta uno ya rotado, se revoca la sesión.
- Verificación en dos pasos (TOTP) compatible con Google Authenticator, Authy o 1Password.
- Logout de la sesión actual o de todas; listado y cierre de sesiones por dispositivo.
- Modo de sesión única opcional.

**Cuentas**
- Alta por gestores, con contraseña o por invitación por correo.
- Auto-registro público opcional con verificación de email.
- Recuperación de contraseña con tokens de un solo uso.
- Perfil propio y cambio de contraseña con aviso por correo.

**Administración**
- Usuarios: listado paginado con búsqueda, edición, suspensión por fechas, desbloqueo, cierre forzado de sesiones, reinicio de MFA y borrado.
- Roles dinámicos. Los roles admin y por defecto están protegidos.
- Auditoría consultable de todos los eventos de seguridad.

**Integración**
- Introspección de tokens y consulta de usuarios para otros servicios.
- Auditoría centralizada.
- JWKS público con RS256.

## Medidas de seguridad

| Amenaza | Medida |
| :--- | :--- |
| Fuerza bruta | Rate limit por IP y bloqueo de cuenta tras N intentos. Los códigos MFA fallidos suman al mismo contador y repetir el login no lo reinicia. |
| Enumeración de usuarios | Misma respuesta y mismo tiempo de bcrypt para emails inexistentes. El bloqueo solo se revela con la contraseña correcta. Respuestas genéricas en recuperación y registro. |
| Robo de token | Access token de vida corta, sesión validada en BD en cada petición, rotación de refresh token. Reutilizar cualquier refresh token antiguo revoca la sesión. |
| Secuestro previo de cuenta | Confirmar el email exige la contraseña de la cuenta. Cambiar el email anula los enlaces pendientes. |
| Confusión de tokens | Claims `iss`, `aud`, `type` y `jti` obligatorios; algoritmo fijo, `none` rechazado. |
| Escalada de privilegios | Solo el admin otorga roles privilegiados o modifica cuentas privilegiadas; nadie cambia su propio rol o estado; nunca se elimina el último admin. |
| Volcado de base de datos | Refresh y tokens de correo guardados como SHA-256; secretos MFA cifrados con Fernet. |
| Contraseñas débiles | Longitud mínima, letra y número, lista de contraseñas comunes, no contener el email, máximo 72 bytes. |
| IP falsificada | `X-Forwarded-For` solo se usa con `TRUSTED_PROXY_COUNT` y se toma desde la derecha. |
| Clave interna por timing | Comparación en tiempo constante. |
| Configuración insegura | Salvo con `ENVIRONMENT=development` o `test`, el servicio no arranca con claves débiles o de ejemplo. En desarrollo usa claves aleatorias temporales. |
| Peticiones gigantes | Límite de tamaño del cuerpo, también para envíos *chunked*. |
| Cabeceras HTTP | `nosniff`, `X-Frame-Options`, CSP, `Referrer-Policy`, HSTS en producción, `Cache-Control: no-store` en la API. |
| Fugas de errores | Manejador global que nunca devuelve trazas al cliente. |

## Endpoints

Todas las rutas van bajo `API_PREFIX` (por defecto `/api`). "Gestor" es `ADMIN_ROLE_NAME` o un rol listado en `USER_MANAGER_ROLES`.

| Método | Ruta | Acceso | Descripción |
| :--- | :--- | :--- | :--- |
| `POST` | `/auth/login` | Público | Tokens, o `mfa_token` si el usuario tiene MFA. |
| `POST` | `/auth/mfa/verify` | Público | Completa el login con el código TOTP. |
| `POST` | `/auth/refresh` | Público | Rota el refresh token. |
| `POST` | `/auth/logout` | Autenticado | Cierra la sesión actual. |
| `POST` | `/auth/logout-all` | Autenticado | Cierra todas las sesiones. |
| `POST` | `/auth/signup` | Público | Auto-registro (si está habilitado). |
| `POST` | `/auth/verify-email` | Público | Confirma el email con el token del correo. |
| `POST` | `/auth/resend-verification` | Público | Reenvía el correo de confirmación. |
| `POST` | `/auth/password-recovery` | Público | Envía el correo de recuperación. |
| `POST` | `/auth/reset-password` | Público | Define la contraseña con el token de recuperación o invitación. |
| `GET` / `PATCH` | `/auth/me` | Autenticado | Ver o editar el perfil propio. |
| `POST` | `/auth/change-password` | Autenticado | Cambia la contraseña propia. |
| `GET` | `/auth/sessions` | Autenticado | Lista las sesiones activas. |
| `DELETE` | `/auth/sessions/{id}` | Autenticado | Cierra una sesión propia. |
| `POST` | `/auth/mfa/setup` | Autenticado | Genera el secreto TOTP. |
| `POST` | `/auth/mfa/enable` | Autenticado | Activa MFA con un código. |
| `POST` | `/auth/mfa/disable` | Autenticado | Desactiva MFA con contraseña y código. |
| `POST` | `/auth/register` | Gestor | Crea un usuario o envía una invitación. |
| `GET` | `/users/` | Gestor | Lista paginada con `skip`, `limit`, `search`, `status`, `role`. |
| `GET` / `PATCH` | `/users/{id}` | Gestor | Ver o editar un usuario. |
| `DELETE` | `/users/{id}` | Admin | Elimina un usuario. |
| `PATCH` | `/users/{id}/suspend` | Gestor | Define o levanta una suspensión. |
| `POST` | `/users/{id}/unlock` | Gestor | Quita el bloqueo por intentos fallidos. |
| `POST` | `/users/{id}/revoke-sessions` | Gestor | Cierra todas sus sesiones. |
| `DELETE` | `/users/{id}/mfa` | Gestor | Reinicia su MFA. |
| `GET` / `POST` | `/roles/` | Gestor | Lista o crea roles. |
| `GET` / `PUT` / `DELETE` | `/roles/{id}` | Gestor | Ver, editar o eliminar un rol. |
| `GET` | `/audit-logs/` | Admin | Consulta la auditoría con filtros. |
| `POST` | `/internal/introspect` | API key | Valida un access token contra la BD. |
| `GET` | `/internal/users/{id}` | API key | Datos básicos de un usuario. |
| `POST` | `/internal/audit` | API key | Registra un evento desde otro servicio. |
| `GET` | `/` y `/health/ready` | Público | Estado del servicio y de la BD. |
| `GET` | `/.well-known/jwks.json` | Público | Clave pública (solo RS256). |

Los listados devuelven el total en la cabecera `X-Total-Count`.

## Puesta en marcha

1. Entorno e instalación:
   ```bash
   python -m venv .venv
   source .venv/bin/activate      # Windows: .venv\Scripts\activate
   pip install -r requirements.txt
   ```
2. Copiar `.env.example` a `.env`. Completar al menos `DATABASE_URL`, `SECRET_KEY`, `INTERNAL_API_KEY`, `DATA_ENCRYPTION_KEY` y `SEED_ADMIN_PASSWORD`.
3. Crear las tablas:
   - **Base nueva y dedicada:** `prisma generate && prisma db push`.
   - **Base existente o compartida con otras tablas:** `prisma generate` y luego aplicar los SQL de `migrations/`. Nunca uses `db push` en una base compartida: borra las tablas que no están en `schema.prisma`.
4. Roles base y administrador: `python seed.py`.
5. Ejecutar: `uvicorn app.main:app --reload`. La documentación queda en `http://localhost:8000/docs`.

### Producción

```bash
uvicorn app.main:app --host 0.0.0.0 --port $PORT --no-server-header
```

- `ENVIRONMENT` vale `production` por defecto: activa HSTS y bloquea el arranque con claves débiles.
- Detrás de Render, Nginx o similar define `TRUSTED_PROXY_COUNT=1`. Sin ello todos los clientes comparten la IP del proxy, y un atacante agotaría el rate limit de login para todos.
- Con más de una instancia usa `RATE_LIMIT_STORAGE_URI=redis://...`.
- Define `CORS_ORIGINS` con los dominios reales de tus frontends.

## Estructura

```text
auth-service/
├── app/
│   ├── core/endpoints.py   # Rutas centralizadas
│   ├── routes/             # auth, account, users, roles, audit, internal
│   ├── schemas/            # Modelos Pydantic
│   ├── services/           # Sesiones, tokens de un solo uso, usuarios, auditoría
│   ├── utils/              # Correo e IP del cliente
│   ├── config.py           # Settings y validación de seguridad
│   ├── dependencies.py     # Usuario actual, RBAC y reglas anti-escalada
│   ├── security.py         # Contraseñas, JWT, TOTP y cifrado
│   ├── middleware.py       # Cabeceras de seguridad
│   └── main.py             # Aplicación FastAPI
├── migrations/             # SQL para bases existentes
├── docs/integration_guide.md
├── schema.prisma
└── seed.py
```
