-- Sprint 2 Updates - Idempotent Migration

-- 1. Add modulo_inventario ENUM safely
DO $$ 
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'modulo_inventario') THEN
        CREATE TYPE modulo_inventario AS ENUM ('FINCA', 'CAFETERIA');
    END IF;
END $$;

-- 2. Update and Expand Enums safely
ALTER TYPE estado_producto ADD VALUE IF NOT EXISTS 'DISPONIBLE';
ALTER TYPE estado_producto ADD VALUE IF NOT EXISTS 'AGOTADO';
ALTER TYPE estado_producto ADD VALUE IF NOT EXISTS 'STOCK_BAJO';
ALTER TYPE estado_producto ADD VALUE IF NOT EXISTS 'INACTIVO';
ALTER TYPE estado_producto ADD VALUE IF NOT EXISTS 'EN_TRANSITO';
ALTER TYPE estado_producto ADD VALUE IF NOT EXISTS 'BLOQUEADO';
ALTER TYPE estado_producto ADD VALUE IF NOT EXISTS 'CADUCADO';

ALTER TYPE unidad_medida ADD VALUE IF NOT EXISTS 'UNIDADES';
ALTER TYPE unidad_medida ADD VALUE IF NOT EXISTS 'LITROS';
ALTER TYPE unidad_medida ADD VALUE IF NOT EXISTS 'KILOGRAMOS';

-- 3. Extend inventario_items table safely (Usando TIMESTAMPTZ para compatibilidad con chrono::DateTime<Utc>)
ALTER TABLE inventario_items ADD COLUMN IF NOT EXISTS modulo modulo_inventario NOT NULL DEFAULT 'FINCA';
ALTER TABLE inventario_items ALTER COLUMN modulo DROP DEFAULT;
ALTER TABLE inventario_items ADD COLUMN IF NOT EXISTS stock_minimo DOUBLE PRECISION DEFAULT 0;
ALTER TABLE inventario_items ADD COLUMN IF NOT EXISTS is_deleted BOOLEAN DEFAULT FALSE;
ALTER TABLE inventario_items ADD COLUMN IF NOT EXISTS fecha_caducidad TIMESTAMPTZ NULL;
ALTER TABLE inventario_items ADD COLUMN IF NOT EXISTS codigo_trazabilidad UUID NULL;
ALTER TABLE inventario_items ADD COLUMN IF NOT EXISTS calidad calidad_cafe NULL;
ALTER TABLE inventario_items ADD COLUMN IF NOT EXISTS fase_produccion fase_cafe NULL;
