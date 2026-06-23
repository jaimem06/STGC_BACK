-- 1. Crear tipos ENUM personalizados (requeridos por los modelos de Rust)
CREATE TYPE estado_producto AS ENUM ('DISPONIBLE', 'AGOTADO', 'STOCK_BAJO', 'INACTIVO', 'EN_TRANSITO', 'BLOQUEADO', 'CADUCADO');
CREATE TYPE tipo_elemento AS ENUM ('INSUMO', 'PRODUCTO', 'CAFE_PROCESADO');
CREATE TYPE unidad_medida AS ENUM ('QUINTALES', 'ARROBAS', 'LIBRAS');
CREATE TYPE calidad_cafe AS ENUM ('ALTA', 'MEDIA', 'BAJA');
CREATE TYPE clasificacion_insumo AS ENUM ('QUIMICO_FERTILIZANTE', 'QUIMICO_FUNGICIDA', 'ORGANICO');
CREATE TYPE fase_cafe AS ENUM ('PULPA', 'DESPULPADO', 'SECADO', 'TOSTADO', 'MOLIDO');
CREATE TYPE tipo_movimiento AS ENUM ('ENTRADA', 'SALIDA');

-- 2. Crear tabla de Inventario
CREATE TABLE inventario_items (
    id UUID PRIMARY KEY,
    sku TEXT NOT NULL UNIQUE,
    nombre TEXT NOT NULL,
    cantidad DOUBLE PRECISION NOT NULL DEFAULT 0,
    tipo tipo_elemento NOT NULL,
    estado estado_producto NOT NULL,
    unidad_medida unidad_medida NOT NULL,
    precio DOUBLE PRECISION NOT NULL DEFAULT 0,
    descripcion TEXT,
    fecha_caducidad TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 3. Crear tabla de Proveedores
CREATE TABLE proveedores (
    id UUID PRIMARY KEY,
    nombre TEXT NOT NULL,
    contacto TEXT NOT NULL,
    tipo_insumo TEXT NOT NULL
);

-- 4. Crear tabla de Lotes de Café (Trazabilidad)
CREATE TABLE lotes_cafe (
    id UUID PRIMARY KEY,
    variedad TEXT NOT NULL,
    fase fase_cafe NOT NULL,
    cantidad_producida DOUBLE PRECISION NOT NULL,
    costo_produccion DOUBLE PRECISION NOT NULL,
    unidad_medida unidad_medida NOT NULL,
    calidad calidad_cafe NOT NULL,
    codigo_trazabilidad UUID NOT NULL,
    lote_anterior_id UUID REFERENCES lotes_cafe(id),
    fecha_creacion TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 5. Crear tabla de Movimientos de Stock
CREATE TABLE movimientos_stock (
    id UUID PRIMARY KEY,
    item_id UUID NOT NULL REFERENCES inventario_items(id),
    cantidad DOUBLE PRECISION NOT NULL,
    tipo tipo_movimiento NOT NULL,
    fecha TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    motivo TEXT NOT NULL,
    lote_id UUID REFERENCES lotes_cafe(id)
);

-- 6. Función y Trigger para actualizar 'updated_at' automáticamente
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_inventario_items_updated_at
BEFORE UPDATE ON inventario_items
FOR EACH ROW
EXECUTE PROCEDURE update_updated_at_column();
