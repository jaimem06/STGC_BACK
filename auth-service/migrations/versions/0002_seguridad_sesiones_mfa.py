"""Seguridad 3.0: sesiones con refresh token, tokens de un solo uso, bloqueo, MFA y auditoría ampliada

Solo modifica tablas de autenticación, así que es segura en una base compartida.
Elimina users.session_token: todos los usuarios deberán iniciar sesión de nuevo.

Revision ID: 0002
Revises: 0001
Create Date: 2026-11-05
"""
from typing import Sequence, Union

from alembic import op

revision: str = "0002"
down_revision: Union[str, None] = "0001"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None

UPGRADE = [
    """CREATE TYPE "TokenPurpose" AS ENUM ('PASSWORD_RESET', 'EMAIL_VERIFICATION')""",
    """ALTER TABLE "audit_logs" DROP CONSTRAINT "audit_logs_user_id_fkey\"""",
    """DROP INDEX "users_session_token_key\"""",
    """ALTER TABLE "users" DROP COLUMN "session_token",
        ADD COLUMN "created_at" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
        ADD COLUMN "email_verified" BOOLEAN NOT NULL DEFAULT false,
        ADD COLUMN "failed_login_attempts" INTEGER NOT NULL DEFAULT 0,
        ADD COLUMN "last_login_at" TIMESTAMP(3),
        ADD COLUMN "locked_until" TIMESTAMP(3),
        ADD COLUMN "mfa_enabled" BOOLEAN NOT NULL DEFAULT false,
        ADD COLUMN "mfa_last_used_step" INTEGER,
        ADD COLUMN "mfa_secret" TEXT,
        ADD COLUMN "password_changed_at" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
        ADD COLUMN "updated_at" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP""",
    """ALTER TABLE "audit_logs" ADD COLUMN "details" TEXT,
        ADD COLUMN "user_agent" TEXT,
        ALTER COLUMN "user_id" DROP NOT NULL""",
    """CREATE TABLE "user_sessions" (
        "id" TEXT NOT NULL,
        "user_id" TEXT NOT NULL,
        "refresh_token_hash" TEXT NOT NULL,
        "user_agent" TEXT,
        "ip_address" TEXT,
        "created_at" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
        "last_used_at" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
        "expires_at" TIMESTAMP(3) NOT NULL,
        "revoked_at" TIMESTAMP(3),
        CONSTRAINT "user_sessions_pkey" PRIMARY KEY ("id")
    )""",
    """CREATE TABLE "rotated_refresh_tokens" (
        "token_hash" TEXT NOT NULL,
        "session_id" TEXT NOT NULL,
        "rotated_at" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
        CONSTRAINT "rotated_refresh_tokens_pkey" PRIMARY KEY ("token_hash")
    )""",
    """CREATE TABLE "auth_tokens" (
        "id" TEXT NOT NULL,
        "user_id" TEXT NOT NULL,
        "token_hash" TEXT NOT NULL,
        "purpose" "TokenPurpose" NOT NULL,
        "expires_at" TIMESTAMP(3) NOT NULL,
        "used_at" TIMESTAMP(3),
        "created_at" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
        CONSTRAINT "auth_tokens_pkey" PRIMARY KEY ("id")
    )""",
    """CREATE UNIQUE INDEX "user_sessions_refresh_token_hash_key" ON "user_sessions"("refresh_token_hash")""",
    """CREATE INDEX "user_sessions_user_id_idx" ON "user_sessions"("user_id")""",
    """CREATE INDEX "rotated_refresh_tokens_session_id_idx" ON "rotated_refresh_tokens"("session_id")""",
    """CREATE UNIQUE INDEX "auth_tokens_token_hash_key" ON "auth_tokens"("token_hash")""",
    """CREATE INDEX "auth_tokens_user_id_purpose_idx" ON "auth_tokens"("user_id", "purpose")""",
    """CREATE INDEX "users_role_id_idx" ON "users"("role_id")""",
    """CREATE INDEX "audit_logs_user_id_idx" ON "audit_logs"("user_id")""",
    """CREATE INDEX "audit_logs_action_idx" ON "audit_logs"("action")""",
    """CREATE INDEX "audit_logs_created_at_idx" ON "audit_logs"("created_at")""",
    """ALTER TABLE "user_sessions" ADD CONSTRAINT "user_sessions_user_id_fkey" FOREIGN KEY ("user_id")
       REFERENCES "users"("id") ON DELETE CASCADE ON UPDATE CASCADE""",
    """ALTER TABLE "rotated_refresh_tokens" ADD CONSTRAINT "rotated_refresh_tokens_session_id_fkey"
       FOREIGN KEY ("session_id") REFERENCES "user_sessions"("id") ON DELETE CASCADE ON UPDATE CASCADE""",
    """ALTER TABLE "auth_tokens" ADD CONSTRAINT "auth_tokens_user_id_fkey" FOREIGN KEY ("user_id")
       REFERENCES "users"("id") ON DELETE CASCADE ON UPDATE CASCADE""",
    """ALTER TABLE "audit_logs" ADD CONSTRAINT "audit_logs_user_id_fkey" FOREIGN KEY ("user_id")
       REFERENCES "users"("id") ON DELETE SET NULL ON UPDATE CASCADE""",
    # Las cuentas existentes las crearon administradores: se consideran verificadas.
    """UPDATE "users" SET "email_verified" = true""",
]

DOWNGRADE = [
    """DROP TABLE "auth_tokens\"""",
    """DROP TABLE "rotated_refresh_tokens\"""",
    """DROP TABLE "user_sessions\"""",
    """DROP TYPE "TokenPurpose\"""",
    """DROP INDEX "users_role_id_idx\"""",
    """DROP INDEX "audit_logs_user_id_idx\"""",
    """DROP INDEX "audit_logs_action_idx\"""",
    """DROP INDEX "audit_logs_created_at_idx\"""",
    """ALTER TABLE "audit_logs" DROP CONSTRAINT "audit_logs_user_id_fkey\"""",
    # El esquema anterior no admite eventos sin usuario.
    """DELETE FROM "audit_logs" WHERE "user_id" IS NULL""",
    """ALTER TABLE "audit_logs" DROP COLUMN "details", DROP COLUMN "user_agent",
        ALTER COLUMN "user_id" SET NOT NULL""",
    """ALTER TABLE "audit_logs" ADD CONSTRAINT "audit_logs_user_id_fkey" FOREIGN KEY ("user_id")
       REFERENCES "users"("id") ON DELETE CASCADE ON UPDATE CASCADE""",
    """ALTER TABLE "users" DROP COLUMN "created_at", DROP COLUMN "email_verified",
        DROP COLUMN "failed_login_attempts", DROP COLUMN "last_login_at", DROP COLUMN "locked_until",
        DROP COLUMN "mfa_enabled", DROP COLUMN "mfa_last_used_step", DROP COLUMN "mfa_secret",
        DROP COLUMN "password_changed_at", DROP COLUMN "updated_at",
        ADD COLUMN "session_token" TEXT""",
    """CREATE UNIQUE INDEX "users_session_token_key" ON "users"("session_token")""",
]


def upgrade() -> None:
    for statement in UPGRADE:
        op.execute(statement)


def downgrade() -> None:
    for statement in DOWNGRADE:
        op.execute(statement)
