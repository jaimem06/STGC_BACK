"""
Modelos SQLAlchemy del servicio de autenticación.

Reproducen exactamente el esquema existente (nombres de tablas, índices, claves
foráneas y tipos), de modo que funcionan sobre bases creadas antes con Prisma.
Cualquier cambio aquí debe ir acompañado de una revisión de Alembic.
"""

import enum
import uuid
from datetime import datetime, timezone
from typing import Optional

from sqlalchemy import Boolean, Enum, ForeignKey, Index, Integer, Text, text
from sqlalchemy.dialects.postgresql import TIMESTAMP
from sqlalchemy.orm import DeclarativeBase, Mapped, mapped_column, relationship
from sqlalchemy.types import TypeDecorator

NOW = text("CURRENT_TIMESTAMP")


def new_id() -> str:
    return str(uuid.uuid4())


def utcnow() -> datetime:
    return datetime.now(timezone.utc)


class UTCDateTime(TypeDecorator):
    """
    Columnas TIMESTAMP(3) sin zona horaria que siempre almacenan UTC.
    La aplicación trabaja con datetimes conscientes de zona (UTC); aquí se
    convierten al guardar y se marcan como UTC al leer.
    """

    impl = TIMESTAMP
    cache_ok = True

    def __init__(self):
        super().__init__(precision=3)

    def process_bind_param(self, value: Optional[datetime], dialect):
        if value is None or value.tzinfo is None:
            return value
        return value.astimezone(timezone.utc).replace(tzinfo=None)

    def process_result_value(self, value: Optional[datetime], dialect):
        if value is None or value.tzinfo is not None:
            return value
        return value.replace(tzinfo=timezone.utc)


class UserStatus(str, enum.Enum):
    ACTIVO = "ACTIVO"
    INACTIVO = "INACTIVO"
    SUSPENDIDO = "SUSPENDIDO"
    PENDIENTE = "PENDIENTE"


class TokenPurpose(str, enum.Enum):
    PASSWORD_RESET = "PASSWORD_RESET"
    EMAIL_VERIFICATION = "EMAIL_VERIFICATION"


class Base(DeclarativeBase):
    pass


def _fk(target: str, name: str, ondelete: str) -> ForeignKey:
    return ForeignKey(target, name=name, ondelete=ondelete, onupdate="CASCADE")


class Role(Base):
    __tablename__ = "roles"
    __table_args__ = (Index("roles_name_key", "name", unique=True),)

    id: Mapped[str] = mapped_column(Text, primary_key=True, default=new_id)
    name: Mapped[str] = mapped_column(Text)
    description: Mapped[Optional[str]] = mapped_column(Text)
    created_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=utcnow, server_default=NOW)


class User(Base):
    __tablename__ = "users"
    __table_args__ = (
        Index("users_email_key", "email", unique=True),
        Index("users_identifier_key", "identifier", unique=True),
        Index("users_phone_number_key", "phone_number", unique=True),
        Index("users_role_id_idx", "role_id"),
    )

    id: Mapped[str] = mapped_column(Text, primary_key=True, default=new_id)
    email: Mapped[str] = mapped_column(Text)
    first_name: Mapped[Optional[str]] = mapped_column(Text)
    last_name: Mapped[Optional[str]] = mapped_column(Text)
    identifier: Mapped[Optional[str]] = mapped_column(Text)
    phone_number: Mapped[Optional[str]] = mapped_column(Text)
    password_hash: Mapped[str] = mapped_column(Text)
    role_id: Mapped[str] = mapped_column(Text, _fk("roles.id", "users_role_id_fkey", "RESTRICT"))
    status: Mapped[UserStatus] = mapped_column(
        Enum(UserStatus, name="UserStatus"), default=UserStatus.ACTIVO, server_default=text("'ACTIVO'")
    )
    email_verified: Mapped[bool] = mapped_column(Boolean, default=False, server_default=text("false"))
    suspended_from: Mapped[Optional[datetime]] = mapped_column(UTCDateTime())
    suspended_until: Mapped[Optional[datetime]] = mapped_column(UTCDateTime())
    failed_login_attempts: Mapped[int] = mapped_column(Integer, default=0, server_default=text("0"))
    locked_until: Mapped[Optional[datetime]] = mapped_column(UTCDateTime())
    last_login_at: Mapped[Optional[datetime]] = mapped_column(UTCDateTime())
    password_changed_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=utcnow, server_default=NOW)
    mfa_enabled: Mapped[bool] = mapped_column(Boolean, default=False, server_default=text("false"))
    mfa_secret: Mapped[Optional[str]] = mapped_column(Text)  # cifrado con Fernet
    mfa_last_used_step: Mapped[Optional[int]] = mapped_column(Integer)
    created_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=utcnow, server_default=NOW)
    updated_at: Mapped[datetime] = mapped_column(
        UTCDateTime(), default=utcnow, onupdate=utcnow, server_default=NOW
    )

    # El rol siempre se carga junto al usuario (se necesita en casi todas las peticiones).
    role: Mapped[Role] = relationship(lazy="joined", innerjoin=True)


class UserSession(Base):
    """Una sesión por dispositivo/login. El refresh token solo se guarda como hash SHA-256."""

    __tablename__ = "user_sessions"
    __table_args__ = (
        Index("user_sessions_refresh_token_hash_key", "refresh_token_hash", unique=True),
        Index("user_sessions_user_id_idx", "user_id"),
    )

    id: Mapped[str] = mapped_column(Text, primary_key=True, default=new_id)
    user_id: Mapped[str] = mapped_column(Text, _fk("users.id", "user_sessions_user_id_fkey", "CASCADE"))
    refresh_token_hash: Mapped[str] = mapped_column(Text)
    user_agent: Mapped[Optional[str]] = mapped_column(Text)
    ip_address: Mapped[Optional[str]] = mapped_column(Text)
    created_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=utcnow, server_default=NOW)
    last_used_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=utcnow, server_default=NOW)
    expires_at: Mapped[datetime] = mapped_column(UTCDateTime())
    revoked_at: Mapped[Optional[datetime]] = mapped_column(UTCDateTime())

    user: Mapped[User] = relationship(lazy="joined", innerjoin=True)


class RotatedRefreshToken(Base):
    """Hashes de refresh tokens ya rotados: si se reutiliza uno, la sesión se revoca."""

    __tablename__ = "rotated_refresh_tokens"
    __table_args__ = (Index("rotated_refresh_tokens_session_id_idx", "session_id"),)

    token_hash: Mapped[str] = mapped_column(Text, primary_key=True)
    session_id: Mapped[str] = mapped_column(
        Text, _fk("user_sessions.id", "rotated_refresh_tokens_session_id_fkey", "CASCADE")
    )
    rotated_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=utcnow, server_default=NOW)

    session: Mapped[UserSession] = relationship(lazy="joined", innerjoin=True)


class AuthToken(Base):
    """Tokens de un solo uso (recuperación, invitación, verificación de email)."""

    __tablename__ = "auth_tokens"
    __table_args__ = (
        Index("auth_tokens_token_hash_key", "token_hash", unique=True),
        Index("auth_tokens_user_id_purpose_idx", "user_id", "purpose"),
    )

    id: Mapped[str] = mapped_column(Text, primary_key=True, default=new_id)
    user_id: Mapped[str] = mapped_column(Text, _fk("users.id", "auth_tokens_user_id_fkey", "CASCADE"))
    token_hash: Mapped[str] = mapped_column(Text)
    purpose: Mapped[TokenPurpose] = mapped_column(Enum(TokenPurpose, name="TokenPurpose"))
    expires_at: Mapped[datetime] = mapped_column(UTCDateTime())
    used_at: Mapped[Optional[datetime]] = mapped_column(UTCDateTime())
    created_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=utcnow, server_default=NOW)

    user: Mapped[User] = relationship(lazy="joined", innerjoin=True)


class AuditLog(Base):
    __tablename__ = "audit_logs"
    __table_args__ = (
        Index("audit_logs_user_id_idx", "user_id"),
        Index("audit_logs_action_idx", "action"),
        Index("audit_logs_created_at_idx", "created_at"),
    )

    id: Mapped[str] = mapped_column(Text, primary_key=True, default=new_id)
    # Null en eventos sin usuario identificado (p. ej. login fallido con email inexistente)
    user_id: Mapped[Optional[str]] = mapped_column(
        Text, _fk("users.id", "audit_logs_user_id_fkey", "SET NULL")
    )
    action: Mapped[str] = mapped_column(Text)
    endpoint: Mapped[Optional[str]] = mapped_column(Text)
    ip_address: Mapped[Optional[str]] = mapped_column(Text)
    user_agent: Mapped[Optional[str]] = mapped_column(Text)
    details: Mapped[Optional[str]] = mapped_column(Text)
    created_at: Mapped[datetime] = mapped_column(UTCDateTime(), default=utcnow, server_default=NOW)
