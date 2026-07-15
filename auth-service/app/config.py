from typing import Optional
from pydantic import field_validator
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    """Application settings for auth-service"""
    model_config = SettingsConfigDict(
        env_file=".env", env_file_encoding="utf-8", extra="ignore"
    )

    # API
    app_name: str = "auth-service"
    app_version: str = "1.1.0"
    api_prefix: str = "/api"

    # Security
    secret_key: str = "change-me-in-production"
    internal_api_key: str = "change-me-in-production"
    jwt_algorithm: str = "HS256"
    access_token_expire_minutes: int = 1440

    # Database
    database_url: str

    # Email SMTP Settings
    smtp_host: str = "smtp.gmail.com"
    smtp_port: int = 587
    smtp_user: Optional[str] = None
    smtp_password: Optional[str] = None
    smtp_from_email: Optional[str] = None
    smtp_tls: bool = True
    frontend_url: str = "https://stgc-front.onrender.com"

    debug: bool = False

    @field_validator(
        "smtp_user", "smtp_password", "smtp_from_email", "smtp_host", "frontend_url",
        mode="before",
    )
    @classmethod
    def _strip_wrapping_quotes(cls, v):
        """
        Elimina comillas y espacios sobrantes en los valores.

        En el dashboard de Render, si el valor de una variable se escribe entre
        comillas (p. ej. porque el App Password de Gmail contiene espacios), las
        comillas quedan como parte LITERAL del valor —a diferencia de un archivo
        .env, donde python-dotenv las elimina—. Esto rompía silenciosamente el
        login SMTP (SMTPAuthenticationError 535). Aquí las limpiamos siempre.
        """
        if isinstance(v, str):
            v = v.strip()
            if len(v) >= 2 and v[0] == v[-1] and v[0] in ("'", '"'):
                v = v[1:-1].strip()
        return v

settings = Settings()