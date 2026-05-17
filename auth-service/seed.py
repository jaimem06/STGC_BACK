import asyncio
from prisma import Prisma

async def main():
    db = Prisma()
    await db.connect()

    # 1. Definir permisos granulares
    permissions = [
        {"name": "all_access", "description": "Acceso total al sistema (Super Rol)"},
        {"name": "manage_users", "description": "Crear, modificar y asignar roles a usuarios"},
        {"name": "view_audit", "description": "Ver logs de auditoría"},
        # Permisos operativos básicos
        {"name": "operaciones_finca", "description": "Acceso a funciones operativas de la finca"},
        {"name": "gestion_inventario", "description": "Acceso a gestión de inventarios"},
        {"name": "gestion_calidad", "description": "Acceso a control de calidad y catación"},
    ]

    print("Actualizando permisos...")
    created_perms = {}
    for perm in permissions:
        p = await db.permission.upsert(
            where={"name": perm["name"]},
            data={
                "create": perm,
                "update": perm,
            }
        )
        created_perms[perm["name"]] = p

    # 2. Definir lista completa de roles solicitados
    # ADMIN será el rol máximo
    roles_data = [
        {"name": "ADMIN", "description": "Super Usuario - Acceso Total", "perms": ["all_access"]},
        {"name": "GERENTE_GENERAL", "description": "Gerencia General - Gestión de Usuarios", "perms": ["manage_users", "view_audit"]},
        {"name": "GERENTE_OPERACIONES", "description": "Gerencia de Operaciones", "perms": ["operaciones_finca"]},
        {"name": "CAPATAZ", "description": "Capataz de campo", "perms": ["operaciones_finca"]},
        {"name": "SEMBRADOR", "description": "Personal de siembra", "perms": []},
        {"name": "RECOLECTOR", "description": "Personal de recolección", "perms": []},
        {"name": "CLASIFICADOR", "description": "Personal de clasificación", "perms": []},
        {"name": "TECNICO_DESPULPADO", "description": "Técnico de despulpado", "perms": []},
        {"name": "ENCARGADO_SECADO", "description": "Encargado de secado", "perms": []},
        {"name": "TOSTADOR", "description": "Tostador de café", "perms": []},
        {"name": "GESTOR_CALIDAD", "description": "Gestor de calidad", "perms": ["gestion_calidad"]},
        {"name": "TECNICO_ALMACENAMIENTO", "description": "Técnico de almacenamiento", "perms": []},
        {"name": "CONTROLADOR_DESPACHO", "description": "Controlador de despacho", "perms": []},
        {"name": "GESTOR_INVENTARIO", "description": "Gestor de inventario", "perms": ["gestion_inventario"]},
        {"name": "PERSONAL_COCINA", "description": "Personal de cocina", "perms": []},
        {"name": "CAJERO_MESERO", "description": "Cajero y mesero", "perms": []},
    ]

    print("Sincronizando roles...")
    for rd in roles_data:
        perms_to_connect = [{"id": created_perms[p_name].id} for p_name in rd["perms"]]
        await db.role.upsert(
            where={"name": rd["name"]},
            data={
                "create": {
                    "name": rd["name"],
                    "description": rd["description"],
                    "permissions": {"connect": perms_to_connect}
                },
                "update": {
                    "description": rd["description"],
                    "permissions": {"set": perms_to_connect}
                }
            }
        )

    await db.disconnect()
    print("Seed actualizado exitosamente con todos los roles de la finca.")

if __name__ == "__main__":
    asyncio.run(main())
