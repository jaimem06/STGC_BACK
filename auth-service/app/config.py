import os
import sys
from typing import Optional
from pydantic_settings import BaseSettings, SettingsConfigDict
from pydantic import Field, ValidationError


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
    jwt_algorithm: str = "HS256"
    access_token_expire_minutes: int = 1440

    # Database - Marcarlo como opcional para evitar crash inmediato al importar
    database_url: Optional[str] = Field(default=None, alias="DATABASE_URL")

    # Email SMTP Settings
    smtp_host: str = "smtp.gmail.com"
    smtp_port: int = 587
    smtp_user: Optional[str] = None
    smtp_password: Optional[str] = None
    smtp_from_email: Optional[str] = None
    smtp_tls: bool = True
    frontend_url: str = "http://localhost:3000"

    debug: bool = False


try:
    settings = Settings()
    # Validación manual crítica para producción
    if not settings.database_url:
        # Intentar obtenerla directamente del entorno si Pydantic falló
        settings.database_url = os.getenv("DATABASE_URL")
        
    if not settings.database_url:
        print("CRITICAL: DATABASE_URL variable is missing in environment.")
        # No salimos aquí para permitir que FastAPI cargue y muestre el error en logs
except ValidationError as e:
    print(f"CRITICAL: Configuration validation error: {e}")
    # En producción es mejor fallar rápido pero con un mensaje claro
    sys.exit(1)
except Exception as e:
    print(f"CRITICAL: Unexpected error during settings load: {e}")
    sys.exit(1)
