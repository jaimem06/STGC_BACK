# Auth Service

Microservicio reutilizable de **autenticación, sesiones, roles (RBAC) y auditoría**. Se conecta a cualquier proyecto que necesite login, gestión de usuarios y control de acceso sin reescribir esa lógica. No contiene modelos de negocio.

## Stack

- **Framework:** FastAPI + Starlette
- **Base de datos:** PostgreSQL con SQLAlchemy 2 asíncrono (asyncpg)
- **Migraciones:** Alembic
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
- Backends intermedios (BFF) que reenvían la IP y el navegador reales del usuario, para que el rate limit, las sesiones y la auditoría no los confundan con el servidor.
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
| IP falsificada | `X-Forwarded-For` solo se usa con `TRUSTED_PROXY_COUNT` y se toma desde la derecha. `X-Client-IP` solo se acepta junto a una `X-Internal-Api-Key` válida. |
| Clave interna por timing | Comparación en tiempo constante. |
| Configuración insegura | Salvo con `ENVIRONMENT=development`, `dev` o `local`, el servicio no arranca con claves débiles o de ejemplo. En desarrollo usa claves aleatorias temporales. |
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
2. Copiar `.env.example` a `.env`. Completar al menos `DATABASE_URL`, `SECRET_KEY`, `INTERNAL_API_KEY`, `DATA_ENCRYPTION_KEY` y `SEED_ADMIN_PASSWORD`. Detalle de cada una en [Variables de entorno](#variables-de-entorno).
3. Crear las tablas: `alembic upgrade head`.
4. Roles base y administrador: `python seed.py`.
5. Ejecutar: `uvicorn app.main:app --reload`. La documentación queda en `http://localhost:8000/docs`.

## Variables de entorno

Se leen de `.env` o del entorno del servidor, y `.env.example` sirve de plantilla. Los valores por defecto son los de `app/config.py`.

Genera las claves aleatorias con:

```bash
python -c "import secrets; print(secrets.token_urlsafe(48))"
```

En la columna "Por defecto":
- **obligatoria**: el servicio no arranca sin ella.
- **producción**: sin un valor seguro no arranca, salvo con `ENVIRONMENT=development`.

### Aplicación

| Variable | Por defecto | Descripción |
| :--- | :--- | :--- |
| `APP_NAME` | `auth-service` | Identificador técnico que aparece en la respuesta de `GET /`. |
| `APP_DISPLAY_NAME` | `Auth Service` | Nombre visible del proyecto. Se muestra en el título de `/docs`, en los correos y como nombre de la cuenta en la app de autenticación (MFA). |
| `API_PREFIX` | `/api` | Prefijo de todas las rutas. Con `/api`, el login queda en `/api/auth/login`. |
| `ENVIRONMENT` | `production` | Con `development`, `dev` o `local`, los problemas de configuración solo generan avisos y las claves débiles se reemplazan por claves temporales. Cualquier otro valor es estricto. `production` o `prod` activan además HSTS. |
| `DEBUG` | `False` | Con `True`, el nivel de log pasa a DEBUG. Fuera de desarrollo bloquea el arranque. |
| `DOCS_ENABLED` | `True` | Con `False` se ocultan `/docs` y `/openapi.json`. |

### Base de datos

| Variable | Por defecto | Descripción |
| :--- | :--- | :--- |
| `DATABASE_URL` | **obligatoria** | URL de PostgreSQL; Alembic usa la misma. Acepta el formato de Neon (`postgresql://...?sslmode=require&channel_binding=require`) y la adapta a asyncpg. Si el host contiene `-pooler`, ajusta la conexión para PgBouncer. |

### Tokens JWT

| Variable | Por defecto | Descripción |
| :--- | :--- | :--- |
| `SECRET_KEY` | **producción** | Firma los access tokens con HS*. Debe tener al menos 32 caracteres aleatorios. Si la cambias, los access tokens vigentes dejan de valer y los clientes los renuevan con `/auth/refresh`. Sin `DATA_ENCRYPTION_KEY`, cambiarla también invalida el MFA ya configurado. |
| `JWT_ALGORITHM` | `HS256` | `HS256`, `HS384` y `HS512` firman con `SECRET_KEY`. `RS256`, `RS384` y `RS512` firman con un par de claves RSA y publican la clave pública en `/.well-known/jwks.json`. |
| `JWT_PRIVATE_KEY` | — | Solo con RS*. Clave privada PEM de al menos 2048 bits. Puede ir en una sola línea con `\n`. |
| `JWT_PUBLIC_KEY` | — | Solo con RS*. Clave pública PEM correspondiente a la privada. |
| `JWT_ISSUER` | `auth-service` | Claim `iss` de los tokens. Los servicios consumidores deben validarlo. |
| `JWT_AUDIENCE` | `auth-service` | Claim `aud`. Usa un valor distinto en cada proyecto, por ejemplo `certiface`, para que el token de un proyecto no sirva en otro. |
| `ACCESS_TOKEN_EXPIRE_MINUTES` | `15` | Vida del access token. Un valor mayor de 60 genera un aviso. |
| `REFRESH_TOKEN_EXPIRE_DAYS` | `7` | Vida de la sesión. Si no se renueva en ese plazo, el usuario tiene que iniciar sesión de nuevo. |
| `MFA_TOKEN_EXPIRE_MINUTES` | `5` | Tiempo para escribir el código TOTP después de un login con MFA. |

### Sesiones, correos y contraseñas

| Variable | Por defecto | Descripción |
| :--- | :--- | :--- |
| `SINGLE_SESSION` | `False` | Con `True`, cada login cierra las demás sesiones del usuario. |
| `PASSWORD_RESET_TOKEN_EXPIRE_MINUTES` | `15` | Validez del enlace de recuperación de contraseña. |
| `INVITATION_TOKEN_EXPIRE_HOURS` | `72` | Validez del enlace de invitación que envía `/auth/register` cuando no se indica contraseña. |
| `EMAIL_VERIFICATION_TOKEN_EXPIRE_HOURS` | `24` | Validez del enlace de confirmación de email. |
| `EMAIL_RESEND_COOLDOWN_SECONDS` | `60` | Espera mínima entre dos correos del mismo tipo al mismo usuario. |
| `PASSWORD_MIN_LENGTH` | `8` | Longitud mínima de la contraseña. El máximo es siempre 72 bytes, el límite de bcrypt. |
| `PASSWORD_REQUIRE_COMPLEXITY` | `True` | Exige al menos una letra y un número. |
| `MAX_FAILED_LOGIN_ATTEMPTS` | `5` | Intentos fallidos, de contraseña o de código MFA, antes de bloquear la cuenta. |
| `LOCKOUT_MINUTES` | `15` | Duración del bloqueo. Un gestor puede quitarlo antes con `/users/{id}/unlock`. |
| `ALLOW_PUBLIC_REGISTRATION` | `False` | Habilita `POST /auth/signup`. Quien se registra recibe `DEFAULT_ROLE_NAME`. |
| `REQUIRE_EMAIL_VERIFICATION` | `False` | Impide iniciar sesión hasta confirmar el email. Los usuarios que un gestor crea con contraseña y el admin del seed ya cuentan como verificados. |

### Claves internas y cifrado

| Variable | Por defecto | Descripción |
| :--- | :--- | :--- |
| `INTERNAL_API_KEY` | **producción** | Clave de la cabecera `X-Internal-Api-Key` para `/internal/*` y para que un backend de confianza reenvíe la IP del usuario (`X-Client-IP`). Debe ser distinta de `SECRET_KEY` y nunca debe llegar a un navegador. |
| `DATA_ENCRYPTION_KEY` | derivada de `SECRET_KEY` | Clave Fernet que cifra los secretos MFA. Se recomienda definirla para poder rotar `SECRET_KEY` sin que los usuarios pierdan el MFA. Se genera con `python -c "from cryptography.fernet import Fernet; print(Fernet.generate_key().decode())"`. |

### Roles

| Variable | Por defecto | Descripción |
| :--- | :--- | :--- |
| `ADMIN_ROLE_NAME` | `ADMIN` | Rol con acceso total. Solo él elimina usuarios, consulta la auditoría y otorga roles privilegiados. No se puede renombrar ni eliminar. |
| `DEFAULT_ROLE_NAME` | `USER` | Rol que se asigna cuando no se indica otro, incluido el auto-registro. No se puede renombrar ni eliminar. |
| `USER_MANAGER_ROLES` | vacío | Roles, separados por comas, que además del admin gestionan usuarios y roles. No pueden otorgar roles privilegiados ni modificar cuentas privilegiadas. Si está vacía, solo el admin gestiona. |

### Seed inicial (`python seed.py`)

| Variable | Por defecto | Descripción |
| :--- | :--- | :--- |
| `SEED_ADMIN_EMAIL` | `admin@example.com` | Email del administrador que crea el seed. |
| `SEED_ADMIN_PASSWORD` | — | Obligatoria para ejecutar el seed y debe cumplir la política de contraseñas. El servicio en marcha no la usa. |
| `SEED_EXTRA_ROLES` | vacío | Roles adicionales con el formato `NOMBRE:Descripción`, separados por comas. Si un rol ya existe, se actualiza su descripción. |
| `SEED_RESET_ADMIN_PASSWORD` | `False` | Con `True`, sobrescribe la contraseña del admin si ya existe. Después vuelve a ponerla en `False`. |

### Red y límites

| Variable | Por defecto | Descripción |
| :--- | :--- | :--- |
| `CORS_ORIGINS` | `*` | Dominios de los frontends, separados por comas. Con `*` se genera un aviso; en producción define siempre los dominios. |
| `TRUSTED_PROXY_COUNT` | `0` | Número de proxies delante del servicio. Con `0` se ignora `X-Forwarded-For`. Detrás de Render, FastAPI Cloud o Nginx usa `1`; con Cloudflare y Render, `2`. |
| `RATE_LIMIT_STORAGE_URI` | `memory://` | Dónde se guardan los contadores del rate limit. Con varias instancias, usa `redis://host:6379`. |
| `RATE_LIMIT_DEFAULT` | `200/minute` | Límite por IP para las rutas que no tienen uno propio. El login, la recuperación y el registro tienen límites más estrictos. |
| `MAX_REQUEST_BODY_BYTES` | `65536` | Tamaño máximo del cuerpo de una petición. Las peticiones más grandes reciben `413`. |

### Correo (SMTP)

Sin `SMTP_USER` o `SMTP_PASSWORD`, el servicio funciona pero no envía correos: ni de recuperación, ni invitaciones, ni de verificación, ni avisos. Cada envío fallido queda registrado en el log.

| Variable | Por defecto | Descripción |
| :--- | :--- | :--- |
| `SMTP_HOST` | `smtp.gmail.com` | Servidor SMTP. |
| `SMTP_PORT` | `587` | `465` usa SSL directo. Cualquier otro puerto usa STARTTLS si `SMTP_TLS=True`. |
| `SMTP_USER` | — | Usuario SMTP. |
| `SMTP_PASSWORD` | — | Contraseña SMTP. En Gmail es una contraseña de aplicación, no la de la cuenta. |
| `SMTP_FROM_EMAIL` | valor de `SMTP_USER` | Remitente que ven los usuarios. |
| `SMTP_TLS` | `True` | Activa STARTTLS en los puertos distintos de 465. |

### Frontend

| Variable | Por defecto | Descripción |
| :--- | :--- | :--- |
| `FRONTEND_URL` | `http://localhost:3000` | URL base del frontend que se usa en los enlaces de los correos. |
| `FRONTEND_RESET_PASSWORD_PATH` | `/reset-password` | Página que recibe `?token=` para restablecer la contraseña o aceptar una invitación. |
| `FRONTEND_VERIFY_EMAIL_PATH` | `/verify-email` | Página que recibe `?token=` para confirmar el email. |

## Migraciones con Alembic

Alembic solo gestiona las tablas de este servicio y guarda su versión en `auth_alembic_version`. Puede convivir en una base compartida con otras tablas sin tocarlas.

| Situación de la base | Comando |
| :--- | :--- |
| Nueva y vacía | `alembic upgrade head` |
| Creada con la versión original (con `users.session_token`) | `alembic stamp 0001` y luego `alembic upgrade head` |
| Ya actualizada con el SQL de seguridad de la versión 3.0 | `alembic stamp 0002` |

Para cambiar el esquema más adelante:

```bash
# 1. Edita app/models.py
alembic revision --autogenerate -m "descripcion del cambio"
# 2. Revisa el archivo generado en migrations/versions/
alembic upgrade head
```

Otros comandos útiles:
- `alembic current` muestra la versión aplicada.
- `alembic check` confirma que la base coincide con los modelos.
- `alembic upgrade head --sql` genera el SQL sin ejecutarlo.

### Producción

En Render:
- **Build command:** `./build.sh`
- **Pre-deploy command:** `alembic upgrade head`
- **Start command:**
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
│   ├── middleware.py       # Cabeceras de seguridad y límite de tamaño
│   ├── models.py           # Modelos SQLAlchemy (tablas)
│   ├── database.py         # Motor y sesiones de base de datos
│   └── main.py             # Aplicación FastAPI
├── migrations/             # Alembic: env.py y versions/
├── alembic.ini
├── docs/integration_guide.md
└── seed.py
```
