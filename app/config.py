from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    """Application settings for STGC"""
    model_config = SettingsConfigDict(
        env_file=".env", env_file_encoding="utf-8", extra="ignore"
    )

    # API
    app_name: str = "STGC_BACK API"
    app_version: str = "1.0.0"
    api_prefix: str = "/api"

    # Security
    secret_key: str = "change-me-in-production"
    jwt_algorithm: str = "HS256"
    access_token_expire_minutes: int = 1440

    # PostgreSQL Database
    # Format: postgresql+asyncpg://user:password@host:port/dbname
    database_url: str = "postgresql+asyncpg://postgres:postgres@localhost:5432/stgc_db"

    debug: bool = False


settings = Settings()
