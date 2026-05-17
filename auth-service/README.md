# STGC_BACK: Sistema de Trazabilidad y Gestión de Café

Microservicio base especializado en autenticación, control de acceso basado en roles (RBAC) y auditoría para la Finca Tierra Fértil. Esta solución está optimizada para entornos Python modernos, utilizando Prisma ORM para garantizar la compatibilidad con las versiones más recientes del lenguaje y PostgreSQL asíncrono.

## Especificaciones Técnicas

- **Framework:** FastAPI
- **ORM:** Prisma ORM (Motor de consultas en Rust)
- **Base de Datos:** PostgreSQL (Neon)
- **Seguridad:** JWT con hashing BCrypt y Rate Limiting (slowapi)
- **Gestión de Sesiones:** Control de token de sesión único por usuario

## Funcionalidades Implementadas

### Autenticación y Registro
- **POST /api/auth/register**: Registro de usuarios con validación de roles (Rate Limited).
- **POST /api/auth/login**: Validación y generación de JWT con rotación de `session_token` (Rate Limited).

### Control de Acceso (RBAC)
Jerarquía de permisos optimizada para la cadena productiva del café:
- **Nivel Directivo:** ADMIN, GERENTE_GENERAL (Acceso total heredado).
- **Nivel Operativo:** CAPATAZ, SEMBRADOR, RECOLECTOR, CLASIFICADOR, etc.
- **Nivel Técnico:** TOSTADOR, GESTOR_CALIDAD, CATADOR, BARISTA.

### Auditoría
Middleware asíncrono (Background Tasks) que registra en la tabla `audit_logs`:
- Usuario, acción, endpoint e IP de origen.

## Guía de Instalación y Despliegue

### Requisitos Previos
- Python 3.10+ (Compatible con Python 3.14 Alpha)
- Instancia de PostgreSQL (Neon)
- Git

### Procedimiento de Configuración

1. Clonar el repositorio:
   ```bash
   git clone https://github.com/jaimem06/STGC_BACK.git
   cd STGC_BACK/auth-service
   ```

2. Configurar el entorno virtual:
   ```bash
   python -m venv venv
   source venv/bin/activate
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

5. Sincronizar Base de Datos:
   ```bash
   prisma db push
   ```

## Ejecución del Servicio

```bash
uvicorn app.main:app --reload
```

- **Documentación Técnica (ReDoc):** `http://localhost:8000/docs`
- **Estado del Sistema:** `http://localhost:8000/`

## Estructura de Directorios

```text
auth-service/
├── app/
│   ├── routes/          # Endpoints de autenticación
│   ├── schemas/         # Validaciones Pydantic
│   ├── security.py      # Seguridad y JWT
│   ├── dependencies.py  # RBAC y Auditoría asíncrona
│   ├── database.py      # Cliente Prisma
│   ├── limiter.py       # Configuración Rate Limit
│   └── main.py          # Punto de entrada
├── static/              # Assets de ReDoc (Self-hosted)
├── schema.prisma        # Definición de modelos Prisma
└── .env                 # Configuración local
```
