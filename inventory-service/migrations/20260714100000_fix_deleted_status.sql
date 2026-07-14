UPDATE inventario_items
SET estado = 'INACTIVO'
WHERE is_deleted = true AND estado <> 'INACTIVO';
