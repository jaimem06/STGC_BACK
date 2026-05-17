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
    jwt_algorithm: str = "HS256"
    access_token_expire_minutes: int = 1440

    # Database
    database_url: str

    debug: bool = False


settings = Settings()