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

from sqlalchemy import select, update
from sqlalchemy.dialects.postgresql import insert

from app.config import settings
from app.database import SessionLocal, engine
from app.models import Role, User, UserSession
from app.security import get_password_hash, password_policy_errors
from app.services.sessions import utcnow


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

    async with SessionLocal() as db:
        print("Sincronizando roles...")
        for rd in _base_roles():
            stmt = insert(Role).values(name=rd["name"], description=rd["description"])
            await db.execute(stmt.on_conflict_do_update(
                index_elements=[Role.name], set_={"description": stmt.excluded.description}
            ))
            print(f"  - {rd['name']}")
        await db.commit()

        admin_role = await db.scalar(select(Role).where(Role.name == settings.admin_role_name))
        existing = await db.scalar(select(User).where(User.email == email))
        if existing is None:
            db.add(User(
                email=email,
                password_hash=get_password_hash(password),
                role_id=admin_role.id,
                email_verified=True,
            ))
            await db.commit()
            print(f"Administrador {email} creado.")
        elif settings.seed_reset_admin_password:
            await db.execute(
                update(User).where(User.id == existing.id).values(
                    password_hash=get_password_hash(password), role_id=admin_role.id,
                    failed_login_attempts=0, locked_until=None,
                ).execution_options(synchronize_session=False)
            )
            await db.execute(
                update(UserSession)
                .where(UserSession.user_id == existing.id, UserSession.revoked_at.is_(None))
                .values(revoked_at=utcnow())
                .execution_options(synchronize_session=False)
            )
            await db.commit()
            print(f"Contraseña de {email} restablecida y sesiones cerradas.")
        else:
            print(f"El administrador {email} ya existe; su contraseña no se modificó.")

    print("Seed completado exitosamente.")
    return 0


async def run() -> int:
    try:
        return await main()
    except Exception as e:
        print(f"Error durante el seed: {type(e).__name__}: {e}")
        return 1
    finally:
        await engine.dispose()


if __name__ == "__main__":
    sys.exit(asyncio.run(run()))
