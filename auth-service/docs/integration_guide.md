# Guía de Integración: STGC Microservicios

Esta guía detalla cómo integrar nuevos microservicios con el ecosistema de **STGC**, utilizando el `auth-service` como servicio central para la identidad, pero manteniendo la autonomía y el rendimiento mediante validación descentralizada.

---

## 1. Arquitectura de Auditoría Centralizada

El `auth-service` actúa como el repositorio central de registros de auditoría. Otros servicios deben reportar acciones sensibles a este componente.

### Endpoint Interno: `POST /internal/audit`

- **URL Completa**: `http://auth-service:8000/internal/audit`
- **Seguridad**: Basada en la cabecera `X-Internal-Api-Key`.
- **¿Por qué no usar JWT?**: Para la comunicación *machine-to-machine* (M2M) entre microservicios, una clave de API interna es más simple y persistente que gestionar tokens de usuario que expiran.

### Estructura del Payload (`AuditCreate`)

```json
{
  "user_id": "uuid-del-usuario",
  "action": "Nombre de la acción (ej: 'CREAR_PRODUCTO')",
  "endpoint": "/api/inventario/productos",
  "ip_address": "192.168.1.1"
}
```

### Procesamiento Asíncrono
El `auth-service` utiliza `BackgroundTasks` de FastAPI para procesar estas peticiones. Al recibir la solicitud, el servicio responde inmediatamente con un `201 Created` y encola la escritura en la base de datos, garantizando que el servicio emisor no sufra latencia innecesaria.

---

## 2. Autorización Descentralizada

Para maximizar la escalabilidad y reducir la latencia, los microservicios validan los tokens JWT de forma local.

### Estructura del Payload JWT
El token emitido por el `auth-service` contiene los siguientes campos clave:
- `sub`: Identificador único del usuario (`user_id`).
- `role`: Nombre del rol asignado (ej: `ADMIN`, `OPERADOR`).
- `session_token`: Token único de sesión para control de sesiones activas.
- `exp`: Tiempo de expiración.

### Validación Local
Al compartir la `JWT_SECRET` (clave de firma) entre todos los servicios, cada uno puede verificar la integridad y autenticidad del token sin realizar una petición de red al `auth-service`.

**Ventajas**:
1. **Baja Latencia**: Sin llamadas externas en cada petición.
2. **Alta Disponibilidad**: Si el `auth-service` está temporalmente fuera de línea, los servicios pueden seguir funcionando con sesiones ya emitidas.

---

## 3. Guía de Integración para Nuevos Servicios

Sigue estos pasos para integrar un nuevo microservicio (ej: Servicio de Inventario).

### Paso 1: Configuración
Añade las siguientes variables de entorno a tu servicio:

```env
JWT_SECRET=tu_clave_secreta_compartida
INTERNAL_API_KEY=tu_api_key_interna
AUTH_SERVICE_URL=http://auth-service:8000
```

### Paso 2: Validación de Sesión (FastAPI)
Implementa una dependencia que valide el token recibido en la cabecera `Authorization`.

### Paso 3: Envío de Auditoría
Cada vez que se realice una operación crítica (POST, PUT, DELETE), envía el registro al endpoint interno.

---

## 4. Ejemplos de Código

### Ejemplo: Dependencia de Validación de Rol (Python/FastAPI)

```python
from fastapi import Depends, HTTPException, status
from fastapi.security import OAuth2PasswordBearer
from jose import jwt, JWTError
import os

JWT_SECRET = os.getenv("JWT_SECRET")
ALGORITHM = "HS256"
oauth2_scheme = OAuth2PasswordBearer(tokenUrl="auth/login")

def get_current_user_role(token: str = Depends(oauth2_scheme)):
    try:
        payload = jwt.decode(token, JWT_SECRET, algorithms=[ALGORITHM])
        user_id: str = payload.get("sub")
        role: str = payload.get("role")
        if user_id is None or role is None:
            raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED)
        return {"user_id": user_id, "role": role}
    except JWTError:
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED)

# Uso en un endpoint
@app.post("/items")
def create_item(user = Depends(get_current_user_role)):
    if user["role"] != "ADMIN":
        raise HTTPException(status_code=403, detail="No tienes permisos")
    # Lógica...
```

### Ejemplo: Cliente de Auditoría

```python
import httpx
import os

AUTH_SERVICE_URL = os.getenv("AUTH_SERVICE_URL")
INTERNAL_API_KEY = os.getenv("INTERNAL_API_KEY")

async def send_audit_log(user_id: str, action: str, endpoint: str, ip: str):
    async with httpx.AsyncClient() as client:
        payload = {
            "user_id": user_id,
            "action": action,
            "endpoint": endpoint,
            "ip_address": ip
        }
        headers = {"X-Internal-Api-Key": INTERNAL_API_KEY}
        await client.post(f"{AUTH_SERVICE_URL}/internal/audit", json=payload, headers=headers)
```
