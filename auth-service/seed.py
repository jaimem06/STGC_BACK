"""
Seed inicial del servicio de autenticación.

Crea los roles base y el usuario administrador. Configurable por variables de entorno:

  ADMIN_ROLE_NAME            Rol con acceso total (por defecto: ADMIN)
  DEFAULT_ROLE_NAME          Rol básico para usuarios normales (por defecto: USER)
  SEED_EXTRA_ROLES           Roles adicionales separados por coma, p. ej.
                             "MANAGER:Gestor del sistema,EDITOR" (descripción opcional)
  SEED_ADMIN_EMAIL           Email del administrador inicial
  SEED_ADMIN_PASSWORD        Contraseña del administrador inicial (obligatoria, debe cumplir la política)
  SEED_RESET_ADMIN_PASSWORD  true para sobrescribir la contraseña si el admin ya existe

Uso:  python seed.py
"""

import asyncio
import sys
from datetime import datetime, timezone

from prisma import Prisma

from app.config import settings
from app.security import get_password_hash, password_policy_errors


def _base_roles() -> list[dict]:
    roles = [
        {"name": settings.admin_role_name, "description": "Administrador - acceso total"},
        {"name": settings.default_role_name, "description": "Usuario estándar"},
    ]
    for raw in settings.seed_extra_roles.split(","):
        raw = raw.strip()
        if not raw:
            continue
        name, _, description = raw.partition(":")
        roles.append({"name": name.strip(), "description": description.strip() or None})
    return roles


async def main() -> int:
    password = settings.seed_admin_password
    email = settings.seed_admin_email.lower()
    if not password:
        print("Define SEED_ADMIN_PASSWORD en el entorno antes de ejecutar el seed.")
        return 1
    errors = password_policy_errors(password, email)
    if errors:
        print("SEED_ADMIN_PASSWORD no cumple la política: " + "; ".join(errors))
        return 1

    db = Prisma(datasource={"url": settings.database_url})
    try:
        await db.connect(timeout=60)
    except Exception as e:
        print(f"Error al conectar a la base de datos: {e}")
        return 1

    try:
        print("Sincronizando roles...")
        for rd in _base_roles():
            await db.role.upsert(
                where={"name": rd["name"]},
                data={
                    "create": {"name": rd["name"], "description": rd["description"]},
                    "update": {"description": rd["description"]},
                },
            )
            print(f"  - {rd['name']}")

        admin_role = await db.role.find_unique(where={"name": settings.admin_role_name})
        existing = await db.user.find_unique(where={"email": email})
        if existing is None:
            await db.user.create(
                data={
                    "email": email,
                    "password_hash": get_password_hash(password),
                    "role_id": admin_role.id,
                    "email_verified": True,
                }
            )
            print(f"Administrador {email} creado.")
        elif settings.seed_reset_admin_password:
            await db.user.update(
                where={"id": existing.id},
                data={"password_hash": get_password_hash(password), "role_id": admin_role.id,
                      "failed_login_attempts": 0, "locked_until": None},
            )
            await db.usersession.update_many(
                where={"user_id": existing.id, "revoked_at": None},
                data={"revoked_at": datetime.now(timezone.utc)},
            )
            print(f"Contraseña de {email} restablecida y sesiones cerradas.")
        else:
            print(f"El administrador {email} ya existe; su contraseña no se modificó.")
    finally:
        await db.disconnect()

    print("Seed completado exitosamente.")
    return 0


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
