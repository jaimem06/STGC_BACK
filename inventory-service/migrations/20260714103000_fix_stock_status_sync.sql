-- Migration to recalculate and fix the 'estado' of active products
-- based on their current 'cantidad' and 'stock_minimo'

UPDATE inventario_items
SET estado = (CASE
    WHEN cantidad <= 0 THEN 'AGOTADO'
    WHEN cantidad <= COALESCE(stock_minimo, 0) THEN 'STOCK_BAJO'
    ELSE 'DISPONIBLE'
END)::estado_producto
WHERE is_deleted = false 
  AND estado NOT IN ('INACTIVO', 'BLOQUEADO', 'EN_TRANSITO', 'CADUCADO');
