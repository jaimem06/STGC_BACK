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

    # Database Parameters
    db_user: str
    db_password: str
    db_host: str
    db_port: int = 5432
    db_name: str
    db_ssl_mode: str = "require"

    @property
    def database_url(self) -> str:
        """Constructs the async database URL from individual parameters"""
        return (
            f"postgresql+asyncpg://{self.db_user}:{self.db_password}@"
            f"{self.db_host}:{self.db_port}/{self.db_name}?ssl={self.db_ssl_mode}"
        )

    debug: bool = False


settings = Settings()
