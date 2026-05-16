# STGC_BACK: Sistema de Trazabilidad y Gestión de Café

Microservicio base especializado en autenticación, control de acceso basado en roles (RBAC) y auditoría para la Finca Tierra Fértil. Esta solución está optimizada para entornos Python modernos, utilizando Prisma ORM para garantizar la compatibilidad con las versiones más recientes del lenguaje y PostgreSQL asíncrono.

## Especificaciones Técnicas

- **Framework:** FastAPI
- **ORM:** Prisma ORM (Motor de consultas en Rust)
- **Base de Datos:** PostgreSQL (Neon)
- **Seguridad:** JWT (python-jose) con hashing BCrypt (passlib)
- **Configuración:** Pydantic Settings con soporte para archivos .env y parámetros descompuestos
- **Gestión de Sesiones:** Control de token de sesión único por usuario para evitar accesos concurrentes

## Funcionalidades Implementadas

### Autenticación y Registro
- **POST /api/auth/register**: Registro de nuevos usuarios con validación de roles.
- **POST /api/auth/login**: Validación de credenciales y generación de JWT con rotación de `session_token`.

### Control de Acceso (RBAC)
Jerarquía de permisos optimizada para la cadena productiva del café:
- **Nivel Directivo:** ADMIN, GERENTE_GENERAL (Acceso total heredado).
- **Nivel Operativo:** CAPATAZ, SEMBRADOR, RECOLECTOR, CLASIFICADOR, TECNICO_DESPULPADO, etc.
- **Nivel Técnico:** TOSTADOR, GESTOR_CALIDAD, CATADOR, BARISTA.

### Auditoría Institucional
Middleware que registra automáticamente en la tabla `audit_logs`:
- Usuario y acción realizada.
- Endpoint y dirección IP de origen.
- Sincronización automática con el motor de Prisma.

## Guía de Instalación y Despliegue

### Requisitos Previos
- Python 3.10 o superior (Compatible con Python 3.14 Alpha)
- Instancia de PostgreSQL (Recomendado: Neon)
- Git

### Procedimiento de Configuración

1. Clonar el repositorio:
   ```bash
   git clone https://github.com/jaimem06/STGC_BACK.git
   cd STGC_BACK
   ```

2. Configurar el entorno virtual:
   ```bash
   python -m venv venv
   source venv/bin/activate  # Linux/macOS
   ```

3. Instalar dependencias:
   ```bash
   pip install -r requirements.txt
   ```

4. Configurar variables de entorno (`.env`):
   ```env
   DATABASE_URL="postgresql://usuario:password@host/neondb?sslmode=require"
   SECRET_KEY="tu_clave_secreta"
   ```

5. Generar Cliente Prisma y Sincronizar Base de Datos:
   ```bash
   prisma db push
   ```

## Ejecución del Servicio

Para iniciar el microservicio en entorno de desarrollo:

```bash
uvicorn app.main:app --reload
```

- **Documentación Interactiva:** `http://localhost:8000/api/docs`
- **Estado del Sistema:** `http://localhost:8000/`

## Estructura de Directorios

```text
STGC_BACK/
├── app/
│   ├── routes/          # Endpoints de autenticación y lógica
│   ├── schemas/         # Validaciones Pydantic
│   ├── security.py      # Seguridad y JWT
│   ├── dependencies.py  # RBAC y Auditoría
│   ├── database.py      # Cliente Prisma
│   └── main.py          # Punto de entrada FastAPI
├── schema.prisma        # Definición única de modelos de datos
└── .env                 # Parámetros de configuración local
```

## Propiedad Intelectual
Copyright (c) 2026 Finca Tierra Fértil. Todos los derechos reservados.
Documentación técnica confidencial para uso exclusivo institucional.
