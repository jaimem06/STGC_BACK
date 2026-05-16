# STGC_BACK: Sistema de Trazabilidad y Gestión de Café

Microservicio base especializado en autenticación, control de acceso basado en roles (RBAC) y auditoría para la Finca Tierra Fértil. Esta solución está construida sobre el framework FastAPI, utilizando PostgreSQL de manera asíncrona y seguridad basada en JSON Web Tokens (JWT) con política de sesión única.

## Especificaciones Técnicas

- **Framework:** FastAPI
- **Base de Datos:** PostgreSQL (vía SQLAlchemy 2.0 y asyncpg)
- **Seguridad:** JWT (python-jose) con hashing BCrypt (passlib)
- **Configuración:** Pydantic Settings con soporte para archivos .env
- **Gestión de Sesiones:** Control de token de sesión único por usuario en base de datos

## Funcionalidades Principales

### Autenticación y Seguridad
El sistema implementa un flujo de autenticación seguro mediante JWT. Se incluye un mecanismo de validación de `session_token` que garantiza que solo exista una sesión activa por cuenta de usuario; el inicio de una nueva sesión invalida automáticamente las credenciales de sesiones anteriores.

### Control de Acceso (RBAC)
Se ha implementado una jerarquía de permisos que abarca toda la cadena productiva del café. Los roles de alta dirección (ADMIN y GERENTE_GENERAL) poseen privilegios globales, permitiendo el acceso a endpoints protegidos por roles operativos de menor jerarquía sin necesidad de declaraciones adicionales.

### Sistema de Auditoría
Integración de un middleware de auditoría que registra de forma automática:
- Identificador del usuario
- Acción realizada
- Endpoint solicitado
- Dirección IP de origen
- Marca de tiempo (UTC)

## Guía de Instalación

### Requisitos Previos
- Python 3.10 o superior
- Instancia de PostgreSQL activa
- Herramienta cliente de Git

### Procedimiento de Despliegue Local

1. Clonar el repositorio institucional:
   ```bash
   git clone https://github.com/jaimem06/STGC_BACK.git
   cd STGC_BACK
   ```

2. Configurar el entorno virtual de ejecución:
   ```bash
   python -m venv venv
   source venv/bin/activate  # En sistemas Linux/macOS
   # venv\Scripts\activate   # En sistemas Windows
   ```

3. Instalación de dependencias del sistema:
   ```bash
   pip install -r requirements.txt
   ```

4. Configuración de variables de entorno:
   Debe crearse un archivo `.env` en la raíz del proyecto basándose en el siguiente esquema:
   ```env
   DATABASE_URL="postgresql+asyncpg://usuario:password@localhost:5432/stgc_db"
   SECRET_KEY="su_clave_secreta_institucional"
   APP_NAME="STGC_BACK API"
   DEBUG=False
   ```

## Ejecución del Servicio

Para iniciar el microservicio en entorno de desarrollo:

```bash
uvicorn app.main:app --reload
```

### Acceso a Documentación Técnica
- Interfaz de Swagger (OpenAPI): `http://localhost:8000/api/docs`
- Verificación de estado (Health Check): `http://localhost:8000/`

## Estructura de Directorios

```text
app/
├── models/          # Definición de esquemas de datos (SQLAlchemy)
├── security.py      # Primitivas de seguridad y criptografía
├── dependencies.py  # Inyección de dependencias y lógica RBAC
├── database.py      # Orquestación de la conexión asíncrona a BD
├── config.py        # Modelo de configuración del sistema
└── main.py          # Punto de entrada y configuración de la aplicación
```

## Propiedad Intelectual
Copyright (c) 2026 Finca Tierra Fértil. Todos los derechos reservados.
Información de carácter confidencial y uso restringido.
