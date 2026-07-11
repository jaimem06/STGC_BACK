-- Sprint 3 - Historiales de auditoría (HU019 precios, HU025 estados).
-- Migración idempotente (mismo estilo que Sprint 2).

-- 1. Historial de precios (HU019): trazabilidad de cada cambio de precio.
CREATE TABLE IF NOT EXISTS historial_precios (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    item_id UUID NOT NULL REFERENCES inventario_items(id),
    precio_anterior DOUBLE PRECISION NOT NULL,
    precio_nuevo DOUBLE PRECISION NOT NULL,
    motivo TEXT,
    usuario_id TEXT,
    fecha_cambio TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_historial_precios_item ON historial_precios(item_id);

-- 2. Historial de estados (HU025): trazabilidad de transiciones manuales y automáticas.
CREATE TABLE IF NOT EXISTS historial_estados (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    item_id UUID NOT NULL REFERENCES inventario_items(id),
    estado_anterior estado_producto NOT NULL,
    estado_nuevo estado_producto NOT NULL,
    motivo TEXT,
    usuario_id TEXT,
    fecha TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_historial_estados_item ON historial_estados(item_id);
