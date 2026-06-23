# Microservicio de Facturación (Billing Service) - Proyecto STGC

Este microservicio es parte del **Sistema de Trazabilidad y Gestión de Cafetería (STGC)**. Su objetivo es manejar de forma desacoplada la validación, registro y trazabilidad de facturas de proveedores (ingresos de inventario), aislando la lógica de negocio contable-fiscal de la simple gestión de bodegas.

## Características Principales

- **Arquitectura**: Construido con Rust y el framework Axum (corriendo en el puerto 3002).
- **Integración Transparente**: Interactúa con la misma base de datos del inventario para garantizar la consistencia en el momento de crear entradas con factura.
- **Transacciones Seguras (ACID)**: Utiliza `sqlx::Transaction` para asegurar que el incremento de stock y el registro del historial (movimiento de la entrada) se lleven a cabo atómicamente.
- **Reglas de Negocio Estrictas**: Valida en profundidad la longitud, formato e integridad de la factura, la fecha de caducidad del producto en el inventario y las restricciones de cantidades.
- **Documentación Activa**: API documentada mediante OpenAPI 3.1 visualizada a través de ReDoc.

## Estructura del Proyecto

```text
billing-service/
├── src/
│   ├── main.rs                 # Punto de entrada, configuración de servidor y pool de BD.
│   ├── handlers/               # Controladores de las solicitudes HTTP.
│   │   └── facturas_handler.rs # Lógica de validación, actualización de inventarios y registro de facturas.
│   ├── middleware/             # Middlewares de seguridad (Validación JWT contra auth-service).
│   ├── models/                 # Estructuras de datos (DTOs).
│   │   └── billing.rs          # Payload de entrada de factura y modelos enlazados.
│   ├── routes/                 # Configuración de rutas y documentación OpenAPI.
│   │   └── mod.rs              # Definición de Router.
│   └── utils/                  # Utilidades compartidas (Auditoría, validaciones, parseo de fechas).
├── .env                        # Variables de entorno (DB_URL, AUTH_URL, PORT).
└── Cargo.toml                  # Dependencias del proyecto.
```

## Funcionamiento Detallado

### 1. Registro de Entrada con Factura
- **Validación del Documento**: Comprueba rigurosamente que el `numero_factura` sea de 17 caracteres exactos y puramente alfanumérico.
- **Validaciones de Producto/Fecha**: El servicio intercepta la fecha de la factura para comprobar que no sea futura, no sea anterior al origen de la base de datos, y que **nunca exceda la fecha de caducidad** del ítem afectado.
- **Gestión de Duplicados**: Ejecuta consultas de control para bloquear la reutilización de un número de factura para un mismo producto en diferentes entradas.
- **Actualización de Base de Datos**: Atómicamente, incrementa el stock en `inventario_items` e inserta el evento en `movimientos_stock` vinculando permanentemente la factura con la trazabilidad del producto.

### 2. Endpoints Principales (Localizados al Español)
- `POST /api/billing/facturas/entrada`: Recibe un DTO con los detalles del ingreso y la factura. Devuelve el identificador del movimiento exitoso.
- `GET /api/billing/docs`: Documentación interactiva y detallada de la API.

## Requisitos Previos

- **Rust**: Versión estable más reciente (MSRV 1.75+).
- **PostgreSQL**: Instancia compartida con `inventory-service`.
- **Servicio de Autenticación**: El microservicio Auth (`auth-service`) debe estar en línea para poder comprobar el token Bearer.

## Cómo Ejecutar el Servicio

1. **Configurar Entorno**:
   Asegúrate de copiar el archivo `.env` o configurarlo basándote en la infraestructura del inventario:
   ```env
   DATABASE_URL=postgres://usuario:password@host:puerto/base_de_datos
   JWT_SECRET=tu_secreto
   AUTH_SERVICE_URL=http://localhost:3000/api/auth/validate
   PORT=3002
   ```
   *(Nota: Este microservicio **no debe correr migraciones**, confía en la estructura de datos mantenida por el `inventory-service`)*.

2. **Compilar y Ejecutar**:
   ```bash
   cd billing-service
   cargo run
   ```

3. **Ver Documentación**:
   Una vez iniciado el servidor, abre tu navegador en:
   [http://localhost:3002/api/billing/docs](http://localhost:3002/api/billing/docs)

## Tecnologías Utilizadas

- **Axum**: Framework web ligero, rápido y predecible.
- **SQLx**: Acceso a bases de datos PostgreSQL de forma asíncrona y transaccional.
- **Utoipa / ReDoc**: Para la especificación automática del API.
- **Chrono**: Procesamiento robusto de fechas y validación de caducidad temporal.
