-- Alineación de movimientos_stock con el handler create_movement y el modelo MovimientoStock.
-- El INSERT del handler escribe la columna 'usuario_id' y el modelo Rust MovimientoStock
-- deserializa 'numero_factura' desde el RETURNING *. Ninguna de las dos existía en el
-- esquema previo, por lo que cualquier movimiento fallaba (INSERT + decode).
-- Migración idempotente (mismo estilo que el Sprint 2).

ALTER TABLE movimientos_stock ADD COLUMN IF NOT EXISTS usuario_id TEXT NULL;
ALTER TABLE movimientos_stock ADD COLUMN IF NOT EXISTS numero_factura TEXT NULL;
