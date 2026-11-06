"""
Motor y sesiones de base de datos (SQLAlchemy asíncrono + asyncpg).
"""

import uuid
from typing import AsyncIterator

from sqlalchemy.engine import URL, make_url
from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker, create_async_engine

from app.config import settings

# Parámetros de libpq (psql, psycopg) que asyncpg no entiende.
_LIBPQ_ONLY_PARAMS = {
    "sslmode", "channel_binding", "sslrootcert", "sslcert", "sslkey", "sslcrl",
    "connect_timeout", "options", "target_session_attrs", "gssencmode", "application_name",
}


def build_engine_args(database_url: str) -> tuple[URL, dict]:
    """
    Convierte una URL estándar de PostgreSQL (p. ej. la de Neon) en una URL para
    asyncpg y sus argumentos de conexión.
    """
    url = make_url(database_url)
    if url.drivername in ("postgres", "postgresql", "postgresql+psycopg", "postgresql+psycopg2"):
        url = url.set(drivername="postgresql+asyncpg")

    query = dict(url.query)
    connect_args: dict = {}

    sslmode = query.get("sslmode")
    if isinstance(sslmode, tuple):
        sslmode = sslmode[-1]
    if sslmode and sslmode != "disable":
        # asyncpg acepta los mismos modos que libpq: prefer, require, verify-ca, verify-full...
        connect_args["ssl"] = sslmode

    timeout = query.get("connect_timeout")
    if timeout:
        connect_args["timeout"] = float(timeout if not isinstance(timeout, tuple) else timeout[-1])

    url = url.set(query={k: v for k, v in query.items() if k not in _LIBPQ_ONLY_PARAMS})

    # PgBouncer en modo transacción (p. ej. el pooler de Neon): sin caché de sentencias
    # preparadas y con nombres únicos para que no choquen entre conexiones.
    if url.host and "-pooler" in url.host:
        connect_args["statement_cache_size"] = 0
        connect_args["prepared_statement_name_func"] = lambda: f"__asyncpg_{uuid.uuid4()}__"

    return url, connect_args


_url, _connect_args = build_engine_args(settings.database_url)

engine = create_async_engine(
    _url,
    connect_args=_connect_args,
    pool_pre_ping=True,
    pool_size=5,
    max_overflow=10,
    pool_recycle=1800,
)

# expire_on_commit=False: los objetos siguen siendo legibles tras commit (necesario en async).
SessionLocal = async_sessionmaker(engine, expire_on_commit=False)


async def get_db() -> AsyncIterator[AsyncSession]:
    """Dependencia: una sesión por petición."""
    async with SessionLocal() as session:
        yield session
