# STGC_BACK: Sistema de Trazabilidad y Gestión de Café

Microservicio base especializado en autenticación, control de acceso basado en roles dinámicos (RBAC) y auditoría para la Finca Tierra Fértil. Esta solución está optimizada para entornos Python modernos, utilizando Prisma ORM para garantizar la compatibilidad con las versiones más recientes del lenguaje y PostgreSQL asíncrono.

## Especificaciones Técnicas

- **Framework:** FastAPI
- **ORM:** Prisma ORM (Motor de consultas en Rust)
- **Base de Datos:** PostgreSQL (Neon)
- **Seguridad:** JWT con hashing BCrypt, Rate Limiting (slowapi) y control de sesión única
- **Configuración:** Pydantic Settings con soporte para archivos .env

## Funcionalidades Implementadas (Base + Mejoras)

### Gestión de Identidad y Acceso
- **Autenticación Segura:** Inicio de sesión y registro con validación de roles dinámicos.
- **Recuperación de Contraseña:** Flujo completo de recuperación mediante tokens firmados y notificaciones por correo.
- **Perfil de Usuario:** Endpoint `/me` que permite verificar la identidad, roles y permisos detallados del usuario actual.
- **Sesión Única:** Control de `session_token` para invalidar sesiones previas al iniciar una nueva.

### Gestión de Usuarios Mejorada
- **Información Extendida:** Campos para nombres, apellidos, identificador único (Cédula/ID) y número de teléfono.
- **Suspensión Temporal:** Lógica para suspender usuarios mediante fechas de inicio y fin (`suspended_from`, `suspended_until`). El sistema bloquea automáticamente el acceso durante este periodo e informa de la fecha de desbloqueo.

### Gestión de Roles y Permisos Dinámicos
- **CRUD de Roles:** Capacidad de crear, actualizar y eliminar roles del sistema.
- **Lógica de Eliminación Segura:** Al eliminar un rol, todos los usuarios asignados a él son reasignados automáticamente al rol predeterminado (**CAJERO_MESERO**) para evitar usuarios huérfanos.

### Auditoría Institucional
- **Middleware Asíncrono:** Registro automático de acciones, endpoints e IPs en segundo plano (Background Tasks).

## Endpoints Principales

| Método | Ruta | Descripción |
| :--- | :--- | :--- |
| `POST` | `/api/auth/register` | Registro de nuevos usuarios con datos personales. |
| `POST` | `/api/auth/login` | Autenticación y obtención de token JWT. |
| `GET` | `/api/auth/me` | Obtener perfil completo y permisos del usuario actual. |
| `POST` | `/api/auth/password-recovery` | Solicitar enlace de recuperación de contraseña. |
| `PATCH` | `/api/users/{id}/suspend` | Establecer periodo de suspensión para un usuario. |
| `DELETE` | `/api/roles/{id}` | Eliminar rol y reasignar usuarios al rol básico. |

## Guía de Instalación y Despliegue

### Requisitos Previos
- Python 3.10+ (Compatible con Python 3.14 Alpha)
- Instancia de PostgreSQL (Recomendado: Neon)

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

- **Documentación ReDoc:** `http://localhost:8000/docs`
- **Estado del Sistema:** `http://localhost:8000/`

## Estructura de Directorios

```text
auth-service/
├── app/
│   ├── routes/          # Endpoints de autenticación, usuarios y roles
│   ├── schemas/         # Modelos de validación Pydantic
│   ├── security.py      # Lógica de seguridad y JWT
│   ├── dependencies.py  # RBAC y Auditoría asíncrona
│   ├── database.py      # Cliente Prisma
│   ├── limiter.py       # Configuración de Rate Limiting
│   └── main.py          # Punto de entrada FastAPI
├── static/              # Assets de ReDoc (Servidos localmente)
├── schema.prisma        # Definición central de modelos de datos
└── .env                 # Parámetros de configuración
```
