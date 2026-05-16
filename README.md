# STGC_BACK - Sistema de Trazabilidad y Gestión de Café

Microservicio base de autenticación, control de acceso por roles (RBAC) y auditoría para la Finca Tierra Fértil. Diseñado con FastAPI, PostgreSQL asíncrono y seguridad JWT de sesión única.

## 🚀 Características Principales

-   **Autenticación JWT:** Implementación segura con tokens de acceso.
-   **Sesión Única:** Control de `session_token` en base de datos para invalidar sesiones previas al iniciar una nueva.
-   **RBAC (Role-Based Access Control):** Jerarquía de roles específica para la industria del café (desde Recolectores hasta Gerencia).
-   **Auditoría Automática:** Registro de acciones de usuario, endpoints visitados e IP de origen.
-   **Stack Moderno:** FastAPI + SQLAlchemy 2.0 + PostgreSQL (asyncpg).
-   **Configuración Segura:** Gestión mediante variables de entorno (`.env`).

## 🛠️ Requisitos

-   Python 3.10+
-   PostgreSQL
-   Git

## 📦 Instalación

1.  **Clonar el repositorio:**
    ```bash
    git clone https://github.com/jaimem06/STGC_BACK.git
    cd STGC_BACK
    ```

2.  **Crear y activar entorno virtual:**
    ```bash
    python -m venv venv
    source venv/bin/activate  # Linux/macOS
    # venv\Scripts\activate  # Windows
    ```

3.  **Instalar dependencias:**
    ```bash
    pip install -r requirements.txt
    ```

4.  **Configurar entorno:**
    Edita el archivo `.env` con tus credenciales de PostgreSQL:
    ```env
    DATABASE_URL="postgresql+asyncpg://usuario:password@localhost:5432/stgc_db"
    SECRET_KEY="tu_clave_secreta"
    ```

## 🚦 Ejecución

Para iniciar el servidor en modo desarrollo:

```bash
uvicorn app.main:app --reload
```

-   **API Docs:** [http://localhost:8000/api/docs](http://localhost:8000/api/docs)
-   **Health Check:** [http://localhost:8000/](http://localhost:8000/)

## 🏗️ Estructura del Proyecto

```text
app/
├── models/          # Modelos de SQLAlchemy (User, AuditLog)
├── security.py      # Lógica de JWT y Hashing
├── dependencies.py  # Inyección de dependencias y RBAC
├── database.py      # Configuración de conexión asíncrona
├── config.py        # Gestión de variables de entorno
└── main.py          # Punto de entrada de FastAPI
```

## 🔐 Jerarquía de Roles

El sistema implementa una lógica de herencia donde los roles **ADMIN** y **GERENTE_GENERAL** tienen acceso automático a todas las rutas protegidas.

**Roles incluidos:**
-   *Administración:* ADMIN, GERENTE_GENERAL, GERENTE_OPERACIONES.
-   *Campo/Planta:* CAPATAZ, SEMBRADOR, RECOLECTOR, CLASIFICADOR, TECNICO_DESPULPADO.
-   *Procesamiento:* ENCARGADO_SECADO, TOSTADOR, GESTOR_CALIDAD.
-   *Logística:* TECNICO_ALMACENAMIENTO, CONTROLADOR_DESPACHO, GESTOR_INVENTARIO.
-   *Servicio:* PRODUCTOR, CATADOR, BARISTA, PERSONAL_COCINA, CAJERO_MESERO.

## 📝 Auditoría

Para registrar una acción en el log de auditoría, simplemente usa la dependencia `log_user_action` en tu endpoint:

```python
@app.post("/items", dependencies=[Depends(log_user_action("crear_item"))])
async def create_item():
    ...
```

## 🛡️ Licencia
Propiedad privada - Finca Tierra Fértil.
