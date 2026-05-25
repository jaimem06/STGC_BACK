import asyncio
from prisma import Prisma
from app.security import get_password_hash

async def main():
    db = Prisma()
    try:
        await db.connect(timeout=60)
    except Exception as e:
        print(f"Error al conectar a la base de datos: {e}")
        return

    # Definir lista completa de roles solicitados sin permisos
    roles_data = [
        {"name": "ADMIN", "description": "Super Usuario - Acceso Total"},
        {"name": "GERENTE_GENERAL", "description": "Gerencia General - Gestión de Usuarios"},
        {"name": "GERENTE_OPERACIONES", "description": "Gerencia de Operaciones - Gestión de Usuarios"},
        {"name": "CAPATAZ", "description": "Capataz de campo"},
        {"name": "SEMBRADOR", "description": "Personal de siembra"},
        {"name": "RECOLECTOR", "description": "Personal de recolección"},
        {"name": "CLASIFICADOR", "description": "Personal de clasificación"},
        {"name": "TECNICO_DESPULPADO", "description": "Técnico de despulpado"},
        {"name": "ENCARGADO_SECADO", "description": "Encargado de secado"},
        {"name": "TOSTADOR", "description": "Tostador de café"},
        {"name": "GESTOR_CALIDAD", "description": "Gestor de calidad"},
        {"name": "TECNICO_ALMACENAMIENTO", "description": "Técnico de almacenamiento"},
        {"name": "CONTROLADOR_DESPACHO", "description": "Controlador de despacho"},
        {"name": "GESTOR_INVENTARIO", "description": "Gestor de inventario"},
        {"name": "PERSONAL_COCINA", "description": "Personal de cocina"},
        {"name": "CAJERO_MESERO", "description": "Cajero y mesero"},
    ]

    print("Sincronizando roles...")
    for rd in roles_data:
        await db.role.upsert(
            where={"name": rd["name"]},
            data={
                "create": {
                    "name": rd["name"],
                    "description": rd["description"]
                },
                "update": {
                    "description": rd["description"]
                }
            }
        )

    print("Sincronizando usuario administrador...")
    admin_role = await db.role.find_unique(where={"name": "ADMIN"})
    if admin_role:
        password_plain = "admin@admin"
        hashed_password = get_password_hash(password_plain)
        
        await db.user.upsert(
            where={"email": "admin@stgc.local"},
            data={
                "create": {
                    "email": "admin@stgc.local",
                    "password_hash": hashed_password,
                    "role_id": admin_role.id,
                },
                "update": {
                    "password_hash": hashed_password,
                    "role_id": admin_role.id,
                },
            },
        )
        print(f"Usuario admin@stgc.local sincronizado con contraseña: {password_plain}")

    await db.disconnect()
    print("Seed completado exitosamente.")

if __name__ == "__main__":
    asyncio.run(main())
