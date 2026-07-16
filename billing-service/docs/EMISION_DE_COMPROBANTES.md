# Informe funcional y técnico — Emisión de comprobantes

## 1. Alcance

Se incorporó al `billing-service` la funcionalidad denominada **Emisión de comprobantes**, correspondiente internamente a HU12-A. Permite emitir y descargar facturas de venta para pedidos del POS, consume las tablas existentes del esquema PostgreSQL `pos_service`, persiste un snapshot inmutable de los datos de factura y asigna la numeración en la base de datos.

No se modificó la lógica de pedidos, pagos, stock ni turnos del POS. La funcionalidad previa de facturas de inventario se conserva.

## 2. Flujo

1. El cajero solicita `POST /api/billing/comprobantes/{pedido_id}/emitir` con autenticación Bearer.
2. El servicio bloquea el pedido durante la transacción y verifica primero que su estado sea `PAGADO`.
3. Recupera negocio, cliente, fecha/hora de pago, productos, totales y el pago del POS o todos los pagos cuando existe la tabla ampliada `Pago`.
4. Valida que ninguna sección obligatoria esté ausente.
5. PostgreSQL asigna `numero` mediante `billing_service.comprobante_numero_seq` y persiste atómicamente un snapshot JSONB con todos los datos necesarios para reconstruir la factura.
6. Como el pedido ya tiene pago confirmado, la factura emitida queda en estado `PAGADA`.
7. La API devuelve `Comprobante generado con éxito.`, el estado de factura y la URL de descarga.
8. Cuando se solicita `GET /api/billing/comprobantes/{pedido_id}/pdf`, el PDF A4 se genera en memoria desde el snapshot persistido.
9. La respuesta usa únicamente `application/pdf`, fuerza extensión `.pdf` y declara `Cache-Control: no-store, max-age=0`. Ningún archivo o `BYTEA` queda almacenado.

Una segunda emisión del mismo pedido es idempotente: devuelve el número, estado y URL existentes (`creado: false`) sin consumir otra numeración ni crear otro documento.

## 3. Cumplimiento de criterios

| Criterio | Implementación verificable |
|---|---|
| CA1 | El PDF muestra nombre/RUC/dirección, nombre/apellido/cédula, fecha, hora UTC con milisegundos y número completo de pedido. Si falta un dato obligatorio responde 422 y no emite. |
| CA2 | El PDF contiene cada producto con cantidad, precio unitario e importe; subtotal, IVA, total y cada método/monto de pago. |
| CA3 | La respuesta exitosa contiene exactamente `Comprobante generado con éxito.` y `pdf_url`. |
| CA4 | Secuencia PostgreSQL, `UNIQUE(numero)`, `UNIQUE(pedido_id)`, bloqueo `FOR UPDATE` e inserción dentro de una transacción. |
| CA5 | El archivo A4 generado bajo demanda inicia con `%PDF-`; la descarga usa `Content-Type: application/pdf`, extensión `.pdf`, `X-Content-Type-Options: nosniff` y `Cache-Control: no-store`. El PDF no se persiste. |
| CA6 | Todo estado de pedido distinto de `PAGADO` se bloquea antes de crear numeración y responde 409 con `No se puede generar el comprobante de un pedido sin pago confirmado.` |

## 4. Estados de factura

Los estados de factura son independientes de los estados del pedido POS. El catálogo se almacena como el enum PostgreSQL `billing_service.estado_factura` y acepta exclusivamente estos valores, en el orden definido por el proyecto:

| Estado | Descripción exacta |
|---|---|
| `BORRADOR` | La factura se está generando pero aún no se ha emitido formalmente (ideal para preventas o pedidos en mesa). |
| `PENDIENTE` | La factura ha sido emitida pero el pago aún no se ha registrado. |
| `PAGADA` | El monto total ha sido cubierto satisfactoriamente. |
| `ANULADA` | La factura fue cancelada por error de digitación o solicitud del cliente, invalidando el monto pero dejando registro para auditoría. |
| `REEMBOLSADA` | El pago se realizó, pero el dinero fue devuelto al cliente y la factura quedó sin efecto. |

HU12-A exige bloquear la emisión cuando el pedido no está pagado. Por esa razón, el flujo de emisión de esta HU termina exclusivamente en `PAGADA`; no se inventaron transiciones administrativas para anular o reembolsar porque no fueron especificadas.

## 5. Compatibilidad con Punto de Servicio (POS)

El contrato POS 1.0 publica `cliente_nombre` sin una columna separada de apellido y guarda un solo `metodoPago` en `Pedido`. Billing adapta el nombre completo usando la última palabra como apellido y usa el método único por el total. Si una versión ampliada ofrece `cliente_apellido` y la tabla `Pago`, también los consume sin cambiar el contrato.

Los totales e IVA se persisten exactamente como los calcula POS; billing no los recalcula. El análisis completo se encuentra en `docs/COMPATIBILIDAD_POS.md`.

## 6. Alcance de compatibilidad con el formato SRI

La composición visual sigue la jerarquía del Anexo 2 de la ficha técnica del SRI: bloque de emisor y RUC, identificación de la factura y secuencial, datos del comprador, detalle tabular, subtotal/IVA/total y formas de pago.

Por instrucción del proyecto, el documento muestra exclusivamente los datos exigidos por HU12-A. No incorpora clave de acceso, número o fecha de autorización, ambiente, tipo de emisión, establecimiento, punto de emisión, códigos tributarios, descuentos, subsidios, guía de remisión ni información adicional.

En consecuencia, el PDF es compatible visualmente con la organización de una representación impresa del SRI, pero no se presenta como un RIDE autorizado ni como comprobante tributario electrónico válido ante el SRI mientras el proyecto no proporcione los campos, firma y proceso de autorización obligatorios.

## 7. Contrato API

### Consultar estados de factura

```http
GET /api/billing/facturas/estados
Authorization: Bearer <token>
```

Devuelve `200 OK` con los cinco estados y sus descripciones exactas:

```json
{
  "estados": [
    {
      "estado": "BORRADOR",
      "descripcion": "La factura se está generando pero aún no se ha emitido formalmente (ideal para preventas o pedidos en mesa)."
    }
  ]
}
```

### Emitir

```http
POST /api/billing/comprobantes/550e8400-e29b-41d4-a716-446655440000/emitir
Authorization: Bearer <token>
```

Primera emisión: `201 Created`. Reemisión idempotente: `200 OK`.

```json
{
  "message": "Comprobante generado con éxito.",
  "numero_comprobante": "COMP-00000001",
  "estado_factura": "PAGADA",
  "pdf_url": "/api/billing/comprobantes/550e8400-e29b-41d4-a716-446655440000/pdf",
  "creado": true
}
```

| Estado HTTP | Condición |
|---:|---|
| 404 | Pedido inexistente. |
| 409 | Pedido distinto de `PAGADO`; incluye el mensaje exacto de CA6. |
| 422 | Pedido pagado sin datos obligatorios, productos o pagos válidos. |
| 500 | Error de infraestructura/generación, sin exponer información sensible. |

### Descargar bajo demanda

```http
GET /api/billing/comprobantes/{pedido_id}/pdf
Authorization: Bearer <token>
```

La respuesta correcta es binaria con `Content-Type: application/pdf`, `Content-Disposition: attachment; filename="comprobante-COMP-00000001.pdf"` y `Cache-Control: no-store, max-age=0`. Cada solicitud genera una instancia temporal en memoria desde los datos persistidos.

## 8. Persistencia y concurrencia

La migración `20260715000000_hu12a_comprobantes.sql` crea el esquema `billing_service`, la secuencia monotónica y la tabla con snapshot JSONB y restricciones únicas por pedido y número. La migración `20260715010000_estados_factura.sql` crea el enum exacto. La migración `20260715020000_pdf_temporal_on_demand.sql` elimina `pdf` y `pdf_sha256` de instalaciones anteriores y habilita la rehidratación única de sus datos históricos.

El snapshot contiene la información del negocio, cliente, transacción, productos, totales y pagos. La descarga nunca vuelve a consultar POS para facturas nuevas, por lo que una modificación posterior no altera el documento histórico.

Las secuencias de PostgreSQL pueden dejar saltos cuando una transacción se revierte; esto evita reutilizar números. La garantía es orden monotónico y ausencia de repetidos, no numeración sin huecos.

## 9. Configuración y despliegue

```env
BUSINESS_NAME=STGC Cafetería
BUSINESS_RUC=1790012345001
BUSINESS_ADDRESS=Av. Principal 123, Ecuador
```

Si falta cualquiera, el servicio conserva las funciones previas pero bloquea la emisión con 422, porque CA1 impide crear un comprobante incompleto. Antes del despliegue se deben aplicar las tres migraciones de emisión en la misma base donde existe `pos_service`.

## 10. Archivos modificados y justificación

| Archivo/área | Cambio | Motivo |
|---|---|---|
| `src/services/receipt_service.rs` | Validación, formato secuencial y render PDF. | Aislar reglas HU12-A y probarlas sin DB. |
| `src/handlers/comprobantes_handler.rs` | Adaptación POS, snapshot transaccional, estado, catálogo y PDF bajo demanda. | Compatibilidad y eliminación de blobs persistidos. |
| `src/models/billing.rs` | Enum, snapshot y contratos tipados de estados, emisión y errores. | Persistir todos los datos de factura y restringir estados. |
| `src/routes/mod.rs` / `src/main.rs` | Rutas, OpenAPI y carga obligatoria de datos fiscales. | Integración mínima. |
| Migraciones de emisión | Secuencia, snapshot JSONB, retiro de BYTEA, restricciones, enum e índice. | Unicidad, persistencia de datos y PDF temporal. |
| `Cargo.toml` / `Cargo.lock` | `printpdf`, soporte JSON de SQLx y `lopdf` de prueba. | Generar PDF en memoria, persistir JSONB y verificar el documento. |
| `.env.example` | Variables fiscales. | Configuración reproducible. |
| `src/handlers/facturas_handler.rs` | Dos macros SQLx cambiaron a consultas enlazadas equivalentes. | Compilar/probar sin DB durante build; no cambia SQL ni conducta. |
| `scripts/`, `docs/`, `.gitignore` | Automatización, informes y exclusión de derivados. | Evidencia repetible. |

No se eliminó código ni se cambiaron reglas de inventario, POS, autenticación o auditoría.
