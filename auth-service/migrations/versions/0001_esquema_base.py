"""Esquema base: roles, usuarios con session_token y auditoría (versión anterior a la 3.0)

Corresponde a las bases creadas con la versión original del servicio. Si tu base ya
tiene estas tablas, no ejecutes esta revisión: márcala con `alembic stamp 0001`.

Revision ID: 0001
Revises:
Create Date: 2026-11-05
"""
from typing import Sequence, Union

from alembic import op

revision: str = "0001"
down_revision: Union[str, None] = None
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None

UPGRADE = [
    """CREATE TYPE "UserStatus" AS ENUM ('ACTIVO', 'INACTIVO', 'SUSPENDIDO', 'PENDIENTE')""",
    """CREATE TABLE "roles" (
        "id" TEXT NOT NULL,
        "name" TEXT NOT NULL,
        "description" TEXT,
        "created_at" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
        CONSTRAINT "roles_pkey" PRIMARY KEY ("id")
    )""",
    """CREATE TABLE "users" (
        "id" TEXT NOT NULL,
        "email" TEXT NOT NULL,
        "first_name" TEXT,
        "last_name" TEXT,
        "identifier" TEXT,
        "phone_number" TEXT,
        "password_hash" TEXT NOT NULL,
        "role_id" TEXT NOT NULL,
        "status" "UserStatus" NOT NULL DEFAULT 'ACTIVO',
        "suspended_from" TIMESTAMP(3),
        "suspended_until" TIMESTAMP(3),
        "session_token" TEXT,
        CONSTRAINT "users_pkey" PRIMARY KEY ("id")
    )""",
    """CREATE TABLE "audit_logs" (
        "id" TEXT NOT NULL,
        "user_id" TEXT NOT NULL,
        "action" TEXT NOT NULL,
        "endpoint" TEXT,
        "ip_address" TEXT,
        "created_at" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
        CONSTRAINT "audit_logs_pkey" PRIMARY KEY ("id")
    )""",
    """CREATE UNIQUE INDEX "roles_name_key" ON "roles"("name")""",
    """CREATE UNIQUE INDEX "users_email_key" ON "users"("email")""",
    """CREATE UNIQUE INDEX "users_identifier_key" ON "users"("identifier")""",
    """CREATE UNIQUE INDEX "users_phone_number_key" ON "users"("phone_number")""",
    """CREATE UNIQUE INDEX "users_session_token_key" ON "users"("session_token")""",
    """ALTER TABLE "users" ADD CONSTRAINT "users_role_id_fkey" FOREIGN KEY ("role_id")
       REFERENCES "roles"("id") ON DELETE RESTRICT ON UPDATE CASCADE""",
    """ALTER TABLE "audit_logs" ADD CONSTRAINT "audit_logs_user_id_fkey" FOREIGN KEY ("user_id")
       REFERENCES "users"("id") ON DELETE CASCADE ON UPDATE CASCADE""",
]

DOWNGRADE = [
    """DROP TABLE "audit_logs\"""",
    """DROP TABLE "users\"""",
    """DROP TABLE "roles\"""",
    """DROP TYPE "UserStatus\"""",
]


def upgrade() -> None:
    for statement in UPGRADE:
        op.execute(statement)


def downgrade() -> None:
    for statement in DOWNGRADE:
        op.execute(statement)
