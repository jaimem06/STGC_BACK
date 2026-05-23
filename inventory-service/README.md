# Microservicio de Control de Inventario - Proyecto STGC

Este microservicio es parte del **Sistema de Trazabilidad y Gestión de Cafetería (STGC)**. Su objetivo es unificar la lógica de trazabilidad de la finca (Módulo A) con la gestión de inventario y ventas (Módulo B), permitiendo un seguimiento completo desde la cosecha hasta el producto final.

## Características Principales

- **Arquitectura**: Construido con Rust y el framework Axum.
- **Base de Datos**: PostgreSQL con SQLx para consultas asíncronas y seguras.
- **Documentación**: OpenAPI 3.1 visualizada mediante ReDoc.
- **Trazabilidad Unificada**: Cada fase del café (pulpa, secado, tostado, etc.) se gestiona como un lote independiente vinculado en una cadena de trazabilidad.

## Estructura del Proyecto

```text
inventory-service/
├── src/
│   ├── main.rs                 # Punto de entrada, configuración de servidor y DB.
│   ├── handlers/               # Controladores de las solicitudes HTTP.
│   │   ├── inventory_handler.rs    # Lógica de ítems, stock y movimientos.
│   │   └── traceability_handler.rs # Lógica de lotes y transiciones de fase.
│   ├── models/                 # Modelos de datos y enums.
│   │   ├── mod.rs              # Exportación de módulos.
│   │   ├── enums.rs            # Enums compartidos (Estado, Fase, Unidad).
│   │   ├── inventory.rs        # Estructuras de Inventario y Movimientos.
│   │   └── traceability.rs     # Estructuras de Lotes y Calidad.
│   └── routes/                 # Configuración de rutas y documentación.
│       └── mod.rs              # Definición de Router y OpenAPI.
├── .env                        # Variables de entorno (DB_URL, PORT).
└── Cargo.toml                  # Dependencias del proyecto.
```

## Funcionamiento Detallado

### 1. Modelos y Lógica de Negocio
- **Unificación A+B**: El inventario no solo maneja productos terminados, sino también café en proceso. Un "Lote de Café" es una entidad que existe en el inventario.
- **Transiciones de Fase**: Cuando un lote pasa de una fase a otra (ej. de *Secado* a *Tostado*), el sistema cierra el lote anterior y crea uno nuevo, registrando la salida y entrada de stock correspondiente.
- **Trazabilidad**: Se utiliza un `codigo_trazabilidad` único (UUID) que persiste a través de todas las fases, permitiendo reconstruir la historia completa del producto.

### 2. Endpoints Principales (Localizados al Español)
- `GET /inventario`: Lista todos los elementos disponibles.
- `POST /inventario/movimientos`: Registra entradas/salidas de stock.
- `POST /trazabilidad/lotes/{id}/transicion`: Ejecuta el cambio de fase de un lote.
- `GET /doc`: Documentación interactiva y detallada de la API.

## Requisitos Previos

- **Rust**: Versión estable más reciente (MSRV 1.75+).
- **PostgreSQL**: Instancia de base de datos accesible (ej. Neon o local).
- **Cargo**: Gestor de paquetes de Rust.

## Cómo Ejecutar el Servicio

1. **Configurar Entorno**:
   Crea o edita el archivo `.env` en la raíz del servicio:
   ```env
   DATABASE_URL=postgres://usuario:password@host:puerto/base_de_datos
   PORT=3001
   ```

2. **Compilar y Ejecutar**:
   ```bash
   cd inventory-service
   cargo run
   ```

3. **Ver Documentación**:
   Una vez iniciado el servidor, abre tu navegador en:
   [http://localhost:3001/doc](http://localhost:3001/doc)

## Tecnologías Utilizadas

- **Axum**: Framework web ergonómico y modular.
- **SQLx**: Toolkit de SQL asíncrono con chequeo de tipos en tiempo de compilación.
- **Utoipa**: Generación automática de especificaciones OpenAPI desde el código.
- **ReDoc**: Interfaz profesional para la documentación de la API.
- **Serde**: Serialización y deserialización eficiente de JSON.
