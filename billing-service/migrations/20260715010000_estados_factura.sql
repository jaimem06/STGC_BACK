-- HU12-A · Catálogo exacto de estados de factura.
-- Restringe la columna `estado` al enum definido por el proyecto y respalda los
-- comprobantes ya emitidos (que siempre representan un pago confirmado).

CREATE TYPE billing_service.estado_factura AS ENUM (
    'BORRADOR',
    'PENDIENTE',
    'PAGADA',
    'ANULADA',
    'REEMBOLSADA'
);

ALTER TABLE billing_service.comprobantes
    ADD COLUMN estado billing_service.estado_factura;

-- Todo comprobante con datos persistidos corresponde a un pedido pagado.
UPDATE billing_service.comprobantes
SET estado = CASE
    WHEN datos IS NOT NULL THEN 'PAGADA'::billing_service.estado_factura
    ELSE 'BORRADOR'::billing_service.estado_factura
END;

ALTER TABLE billing_service.comprobantes
    ALTER COLUMN estado SET NOT NULL;
