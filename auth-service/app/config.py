import re
from typing import Optional

from pydantic import field_validator
from pydantic_settings import BaseSettings, SettingsConfigDict

INSECURE_DEFAULT = "change-me-in-production"
SUPPORTED_JWT_ALGORITHMS = {"HS256", "HS384", "HS512", "RS256", "RS384", "RS512"}


class Settings(BaseSettings):
    """
    Configuración del servicio de autenticación.

    Todo lo que depende del proyecto que consume este servicio (nombre, URL del
    frontend, roles con privilegios, políticas de seguridad, CORS, correo) se
    define aquí mediante variables de entorno, sin tocar el código.
    """

    model_config = SettingsConfigDict(
        env_file=".env", env_file_encoding="utf-8", extra="ignore"
    )

    # --- Aplicación ---
    app_name: str = "auth-service"
    app_display_name: str = "Auth Service"
    app_version: str = "3.0.0"
    api_prefix: str = "/api"
    # Cualquier valor distinto de development/dev/local se valida como producción.
    environment: str = "production"
    debug: bool = False
    docs_enabled: bool = True

    # --- Base de datos ---
    database_url: str

    # --- JWT ---
    secret_key: str = INSECURE_DEFAULT
    jwt_algorithm: str = "HS256"
    # Solo para algoritmos RS*: claves PEM. Permiten que otros servicios validen
    # los tokens con la clave pública (JWKS) sin conocer ningún secreto.
    jwt_private_key: Optional[str] = None
    jwt_public_key: Optional[str] = None
    jwt_issuer: str = "auth-service"
    jwt_audience: str = "auth-service"
    access_token_expire_minutes: int = 15
    refresh_token_expire_days: int = 7
    mfa_token_expire_minutes: int = 5

    # --- Sesiones ---
    # True: cada login cierra las demás sesiones del usuario.
    single_session: bool = False

    # --- Tokens de un solo uso ---
    password_reset_token_expire_minutes: int = 15
    invitation_token_expire_hours: int = 72
    email_verification_token_expire_hours: int = 24
    # Tiempo mínimo entre correos del mismo tipo para un usuario (anti-spam)
    email_resend_cooldown_seconds: int = 60

    # --- Política de contraseñas ---
    password_min_length: int = 8
    password_require_complexity: bool = True  # al menos una letra y un número

    # --- Protección contra fuerza bruta ---
    max_failed_login_attempts: int = 5
    lockout_minutes: int = 15

    # --- Registro ---
    allow_public_registration: bool = False
    require_email_verification: bool = False

    # --- Claves internas y cifrado ---
    internal_api_key: str = INSECURE_DEFAULT
    # Clave Fernet para cifrar secretos MFA. Si se omite se deriva de SECRET_KEY.
    data_encryption_key: Optional[str] = None

    # --- Roles ---
    admin_role_name: str = "ADMIN"
    default_role_name: str = "USER"
    # Roles (además del admin) autorizados a gestionar usuarios y roles. Separados por coma.
    user_manager_roles: str = ""

    # --- Seed inicial ---
    seed_admin_email: str = "admin@example.com"
    seed_admin_password: Optional[str] = None
    seed_extra_roles: str = ""
    seed_reset_admin_password: bool = False

    # --- Red ---
    cors_origins: str = "*"
    # Número de proxies de confianza delante del servicio (Render, Nginx, Cloudflare...).
    # 0 = ignorar X-Forwarded-For (no se puede falsificar la IP).
    trusted_proxy_count: int = 0
    # Almacenamiento del rate limiting. Con varias instancias usar Redis: "redis://host:6379"
    rate_limit_storage_uri: str = "memory://"
    rate_limit_default: str = "200/minute"
    # Tamaño máximo del cuerpo de una petición (bytes). Las peticiones de auth son pequeñas.
    max_request_body_bytes: int = 65536

    # --- Correo (SMTP) ---
    smtp_host: str = "smtp.gmail.com"
    smtp_port: int = 587
    smtp_user: Optional[str] = None
    smtp_password: Optional[str] = None
    smtp_from_email: Optional[str] = None
    smtp_tls: bool = True

    # --- Frontend ---
    frontend_url: str = "http://localhost:3000"
    frontend_reset_password_path: str = "/reset-password"
    frontend_verify_email_path: str = "/verify-email"

    @field_validator(
        "smtp_user", "smtp_password", "smtp_from_email", "smtp_host",
        "frontend_url", "cors_origins", "user_manager_roles", "seed_extra_roles",
        "jwt_private_key", "jwt_public_key", "data_encryption_key", "environment",
        mode="before",
    )
    @classmethod
    def _strip_wrapping_quotes(cls, v):
        """
        Elimina comillas y espacios sobrantes. En paneles como Render las comillas
        escritas alrededor de un valor quedan como parte literal del mismo.
        """
        if isinstance(v, str):
            v = v.strip()
            if len(v) >= 2 and v[0] == v[-1] and v[0] in ("'", '"'):
                v = v[1:-1].strip()
        return v

    @field_validator("jwt_private_key", "jwt_public_key", mode="after")
    @classmethod
    def _unescape_pem(cls, v):
        # Permite definir la clave PEM en una sola línea con "\n" literales.
        if v and "\\n" in v:
            v = v.replace("\\n", "\n")
        return v or None

    @field_validator("jwt_algorithm", mode="after")
    @classmethod
    def _check_algorithm(cls, v: str) -> str:
        v = v.upper()
        if v not in SUPPORTED_JWT_ALGORITHMS:
            raise ValueError(f"JWT_ALGORITHM no soportado: {v}")
        return v

    # --- Helpers derivados ---
    @property
    def is_production(self) -> bool:
        return self.environment.lower() in ("production", "prod")

    @property
    def is_strict(self) -> bool:
        """
        Todo entorno que no sea explícitamente de desarrollo se trata como
        producción a efectos de validación: una configuración insegura impide arrancar.
        """
        return self.environment.lower() not in ("development", "dev", "local")

    @property
    def uses_asymmetric_jwt(self) -> bool:
        return self.jwt_algorithm.startswith("RS")

    @property
    def cors_origin_list(self) -> list[str]:
        return [o.strip() for o in self.cors_origins.split(",") if o.strip()]

    @property
    def user_manager_role_list(self) -> list[str]:
        extra = [r.strip() for r in self.user_manager_roles.split(",") if r.strip()]
        return [self.admin_role_name, *extra]

    @property
    def privileged_role_names(self) -> set[str]:
        """Roles que solo el administrador puede asignar o modificar."""
        return set(self.user_manager_role_list)

    @property
    def protected_role_names(self) -> set[str]:
        """Roles que no pueden renombrarse ni eliminarse."""
        return {self.admin_role_name, self.default_role_name}

    def frontend_link(self, path: str) -> str:
        return f"{self.frontend_url.rstrip('/')}{path}"


def _rsa_key_problems(s: Settings) -> list[str]:
    from cryptography.hazmat.primitives import serialization
    from cryptography.hazmat.primitives.asymmetric import rsa

    try:
        private_key = serialization.load_pem_private_key(s.jwt_private_key.encode(), password=None)
        public_key = serialization.load_pem_public_key(s.jwt_public_key.encode())
    except (ValueError, TypeError) as e:
        return [f"No se pudieron leer las claves RSA: {type(e).__name__}"]
    if not isinstance(private_key, rsa.RSAPrivateKey) or private_key.key_size < 2048:
        return ["JWT_PRIVATE_KEY debe ser una clave RSA de al menos 2048 bits"]
    if private_key.public_key().public_numbers() != public_key.public_numbers():
        return ["JWT_PUBLIC_KEY no corresponde a JWT_PRIVATE_KEY"]
    return []


_PLACEHOLDER_FRAGMENTS = (
    "change-me", "changeme", "cambia", "genera", "example", "ejemplo",
    "your-secret", "tu-clave", "tu_clave", "secret_key", "placeholder",
)


def is_weak_secret(value: Optional[str]) -> bool:
    """
    Rechaza secretos cortos, con poca variedad de caracteres, con forma de frase
    (solo minúsculas y guiones) o que contengan texto de ejemplo típico.
    """
    if not value or len(value) < 32 or len(set(value)) < 12:
        return True
    if re.fullmatch(r"[a-z\-_ ]+", value):
        return True
    lowered = value.lower()
    return any(fragment in lowered for fragment in _PLACEHOLDER_FRAGMENTS)


def security_problems(s: Settings) -> list[str]:
    """Devuelve la lista de configuraciones inseguras detectadas."""
    problems: list[str] = []
    if s.uses_asymmetric_jwt:
        if not s.jwt_private_key or not s.jwt_public_key:
            problems.append("JWT_ALGORITHM es RS* pero faltan JWT_PRIVATE_KEY y/o JWT_PUBLIC_KEY")
        else:
            problems.extend(_rsa_key_problems(s))
    if is_weak_secret(s.secret_key):
        problems.append("SECRET_KEY debe ser aleatoria, de al menos 32 caracteres y sin texto de ejemplo")
    if is_weak_secret(s.internal_api_key):
        problems.append("INTERNAL_API_KEY debe ser aleatoria, de al menos 32 caracteres y sin texto de ejemplo")
    if s.data_encryption_key:
        from cryptography.fernet import Fernet
        try:
            Fernet(s.data_encryption_key.encode())
        except (ValueError, TypeError):
            problems.append("DATA_ENCRYPTION_KEY no es una clave Fernet válida")
    if s.secret_key == s.internal_api_key:
        problems.append("SECRET_KEY e INTERNAL_API_KEY no deben ser iguales")
    if s.is_strict and s.debug:
        problems.append("DEBUG debe estar desactivado fuera de desarrollo")
    return problems


def security_warnings(s: Settings) -> list[str]:
    warnings: list[str] = []
    if s.access_token_expire_minutes > 60:
        warnings.append(
            "ACCESS_TOKEN_EXPIRE_MINUTES mayor a 60 aumenta el impacto de un token robado; "
            "usa 15 y renueva con /auth/refresh"
        )
    if "*" in s.cors_origin_list:
        warnings.append("CORS_ORIGINS='*' permite cualquier origen; define los dominios de tus frontends")
    if not s.data_encryption_key:
        warnings.append(
            "DATA_ENCRYPTION_KEY no definida: los secretos MFA se cifran con una clave derivada "
            "de SECRET_KEY; si la rotas, los usuarios deberán reconfigurar MFA"
        )
    if s.is_production and s.rate_limit_storage_uri.startswith("memory"):
        warnings.append("Rate limiting en memoria: con varias instancias usa RATE_LIMIT_STORAGE_URI=redis://...")
    return warnings


settings = Settings()
