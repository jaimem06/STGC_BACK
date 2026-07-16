-- HU12-A · Persistencia de comprobantes emitidos.
-- El comprobante vive en su propio esquema `billing_service`, dentro de la misma
-- base compartida con el POS (`pos_service`). La numeración es una secuencia de
-- base de datos para garantizar consecutividad y unicidad (CA4).

CREATE SCHEMA IF NOT EXISTS billing_service;

CREATE SEQUENCE IF NOT EXISTS billing_service.comprobante_numero_seq
    AS BIGINT
    START WITH 1
    INCREMENT BY 1
    MINVALUE 1;

CREATE TABLE IF NOT EXISTS billing_service.comprobantes (
    id         UUID PRIMARY KEY,
    pedido_id  TEXT   NOT NULL,
    numero     BIGINT NOT NULL DEFAULT nextval('billing_service.comprobante_numero_seq'),
    datos      JSONB  NOT NULL,
    creado     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT comprobantes_pedido_id_key UNIQUE (pedido_id),
    CONSTRAINT comprobantes_numero_key    UNIQUE (numero),
    CONSTRAINT comprobantes_numero_positivo CHECK (numero > 0)
);

-- La secuencia pertenece a la columna: se limpia junto con la tabla.
ALTER SEQUENCE billing_service.comprobante_numero_seq
    OWNED BY billing_service.comprobantes.numero;
