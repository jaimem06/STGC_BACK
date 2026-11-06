"""
Entorno de Alembic (asíncrono, asyncpg).

- La URL se toma de DATABASE_URL a través de app.config.settings.
- La tabla de versiones se llama auth_alembic_version para no chocar con otros
  servicios que también usen Alembic en una base compartida.
- Solo se comparan las tablas de este servicio: un autogenerate nunca propondrá
  borrar tablas ajenas que existan en la misma base.
"""

import asyncio
from logging.config import fileConfig

from alembic import context
from sqlalchemy.ext.asyncio import create_async_engine

from app.config import settings
from app.database import build_engine_args
from app.models import Base

config = context.config
if config.config_file_name is not None:
    fileConfig(config.config_file_name)

target_metadata = Base.metadata
VERSION_TABLE = "auth_alembic_version"
URL, CONNECT_ARGS = build_engine_args(settings.database_url)


def include_object(obj, name, type_, reflected, compare_to):
    # Ignora tablas (y sus índices) que existen en la base pero no pertenecen a este servicio.
    if type_ == "table" and reflected and compare_to is None:
        return False
    if type_ in ("index", "unique_constraint", "foreign_key_constraint") and reflected:
        table = getattr(obj, "table", None)
        if table is not None and table.name not in target_metadata.tables:
            return False
    return True


def _configure(**kwargs) -> None:
    context.configure(
        target_metadata=target_metadata,
        version_table=VERSION_TABLE,
        include_object=include_object,
        compare_type=True,
        **kwargs,
    )


def run_migrations_offline() -> None:
    """Genera el SQL sin conectarse (alembic upgrade head --sql)."""
    _configure(url=URL.render_as_string(hide_password=False), literal_binds=True,
               dialect_opts={"paramstyle": "named"})
    with context.begin_transaction():
        context.run_migrations()


def _run_sync(connection) -> None:
    _configure(connection=connection)
    with context.begin_transaction():
        context.run_migrations()


async def run_migrations_online() -> None:
    engine = create_async_engine(URL, connect_args=CONNECT_ARGS)
    try:
        async with engine.connect() as connection:
            await connection.run_sync(_run_sync)
    finally:
        await engine.dispose()


if context.is_offline_mode():
    run_migrations_offline()
else:
    asyncio.run(run_migrations_online())
