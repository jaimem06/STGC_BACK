# Lista de validación manual — HU12-A

## Entorno levantado

| Componente | Dirección/estado esperado |
|---|---|
| PostgreSQL 16 | Contenedor `stgc-hu12a-postgres`, puerto `5432`, base `stgc_test` |
| Billing service | `http://127.0.0.1:3002` |
| ReDoc | `http://127.0.0.1:3002/docs` |
| Logs | `G:\Sandbox\STGC\outputs\hu12a-manual\billing.stderr.log` |

Auth y POS no necesitan ejecutarse para esta validación aislada: el esquema POS y sus pedidos están en PostgreSQL, y se genera un JWT local firmado con el secreto del billing de prueba.

## Datos preparados

| Pedido | Estado | Uso |
|---|---|---|
| `HU12A-PAID-001` | PAGADO | Prueba de humo ya emitida como `COMP-00000001` |
| `HU12A-PAID-002` | PAGADO | Prueba de humo ya emitida como `COMP-00000002` |
| `HU12A-PAID-003` | PAGADO | Emisión manual anterior, ya emitida |
| `HU12A-PAID-004` | PAGADO | Factura histórica rehidratada como `COMP-00000005` |
| `HU12A-PAID-005` | PAGADO | Prueba on-demand ya emitida como `COMP-00000006` |
| `HU12A-PAID-006` | PAGADO | Primera emisión manual disponible, todavía no emitida |
| `HU12A-PAID-007` | PAGADO | Validación manual de secuencia, todavía no emitida |
| `HU12A-UNPAID-001` | PENDIENTE_PAGO | Bloqueo CA6 |
| `HU12A-INVALID-001` | PAGADO, sin apellido | Datos obligatorios/422 |

## Preparación de la sesión PowerShell

```powershell
Set-Location 'G:\Sandbox\STGC\STGC_BACK.worktrees\pos-service\pos-service'

$tokenScript = @'
const jwt = require('jsonwebtoken');
console.log(jwt.sign({
  sub: 'manual-cashier',
  role: 'CAJERO',
  session_token: 'hu12a-manual',
  exp: Math.floor(Date.now() / 1000) + 7200
}, 'hu12a-manual-secret', { algorithm: 'HS256' }));
'@

$token = ($tokenScript | node -).Trim()
$headers = @{ Authorization = "Bearer $token" }
$base = 'http://127.0.0.1:3002/api/billing'
$output = 'G:\Sandbox\STGC\outputs\hu12a-manual'
```

## Checklist ejecutable

### MAN-00 — Servicios disponibles

- [ ] `docker ps --filter name=stgc-hu12a-postgres` muestra el contenedor como `Up (healthy)`.
- [ ] Abrir `http://127.0.0.1:3002/docs`; debe responder 200 y mostrar las rutas de comprobantes.

### MAN-00A — Catálogo exacto de estados de factura

```powershell
$catalogo = Invoke-RestMethod -Method Get `
  -Uri "$base/facturas/estados" `
  -Headers $headers
$catalogo.estados | Format-Table
```

- [ ] Devuelve, en este orden: `BORRADOR`, `PENDIENTE`, `PAGADA`, `ANULADA`, `REEMBOLSADA`.
- [ ] Cada descripción coincide literalmente con la definición documentada en `EMISION_DE_COMPROBANTES.md`.
- [ ] No aparecen estados del pedido POS, como `PAGADO` o `PENDIENTE_PAGO`.

### MAN-01 — Autenticación obligatoria

```powershell
curl.exe -i -X POST "$base/comprobantes/HU12A-PAID-006/emitir"
```

- [ ] Responde `401 Unauthorized`.
- [ ] No crea ningún comprobante.

### MAN-02 — Bloqueo de pedido sin pago (CA6)

```powershell
curl.exe -i -X POST `
  -H "Authorization: Bearer $token" `
  "$base/comprobantes/HU12A-UNPAID-001/emitir"
```

- [ ] Responde `409 Conflict`.
- [ ] El cuerpo contiene exactamente: `No se puede generar el comprobante de un pedido sin pago confirmado.`
- [ ] No se asigna número ni se crea fila para el pedido.

### MAN-03 — Datos obligatorios incompletos (CA1)

```powershell
curl.exe -i -X POST `
  -H "Authorization: Bearer $token" `
  "$base/comprobantes/HU12A-INVALID-001/emitir"
```

- [ ] Responde `422 Unprocessable Entity`.
- [ ] Informa que falta el apellido del cliente.
- [ ] No genera PDF ni comprobante parcial.

### MAN-04 — Primera emisión válida (CA1–CA5)

```powershell
$respuesta1 = Invoke-WebRequest -UseBasicParsing -Method Post `
  -Uri "$base/comprobantes/HU12A-PAID-006/emitir" `
  -Headers $headers
$emision1 = $respuesta1.Content | ConvertFrom-Json
$respuesta1.StatusCode
$emision1 | Format-List
```

- [ ] Primera ejecución responde `201 Created`.
- [ ] `message` es `Comprobante generado con éxito.`
- [ ] `creado` es `true`.
- [ ] `numero_comprobante` cumple `COMP-########`.
- [ ] `estado_factura` es `PAGADA`.
- [ ] `pdf_url` apunta al mismo pedido.

### MAN-05 — Exportación exclusivamente PDF (CA5)

```powershell
$pdf = Join-Path $output 'comprobante-HU12A-PAID-006.pdf'
$descarga = Invoke-WebRequest -UseBasicParsing `
  -Uri "http://127.0.0.1:3002$($emision1.pdf_url)" `
  -Headers $headers -OutFile $pdf -PassThru

$bytes = [IO.File]::ReadAllBytes($pdf)
$firma = [Text.Encoding]::ASCII.GetString($bytes[0..4])
$descarga.Headers['Content-Type']
$descarga.Headers['Content-Disposition']
$descarga.Headers['Cache-Control']
$firma
```

- [ ] `Content-Type` es `application/pdf`.
- [ ] `Content-Disposition` termina en `.pdf`.
- [ ] `Cache-Control` es `no-store, max-age=0`.
- [ ] La firma es `%PDF-`.
- [ ] El documento usa una página A4 (`210 × 297 mm`).
- [ ] El archivo abre sin advertencias ni corrupción.

### MAN-06 — Inspección visual del contenido (CA1 y CA2)

Abrir `G:\Sandbox\STGC\outputs\hu12a-manual\comprobante-HU12A-PAID-006.pdf`.

- [ ] Negocio: `Cafeteria STGC Manual`.
- [ ] RUC: `1790012345001`.
- [ ] Dirección: `Av. Principal 123, Quito`.
- [ ] Cliente: `Lucia Navas`, cédula `0945678901`.
- [ ] Pedido: `HU12A-PAID-006`.
- [ ] Fecha y hora exacta de la transacción visibles.
- [ ] Producto: 1 × `Espresso doble`, precio unitario `$7.00`, importe `$7.00`.
- [ ] Subtotal `$7.00`, IVA `$1.05`, total `$8.05`.
- [ ] Método de pago `TRANSFERENCIA: $8.05`.
- [ ] La información está organizada visualmente en bloques tipo RIDE: negocio y comprobante, cliente y transacción, productos, pagos y totales.
- [ ] No aparecen campos ajenos a la HU, como clave de acceso, autorización SRI, ambiente, tipo de emisión, establecimiento, punto de emisión, descuentos o subsidios.

### MAN-07 — Reemisión idempotente (CA4)

```powershell
$respuestaReemision = Invoke-WebRequest -UseBasicParsing -Method Post `
  -Uri "$base/comprobantes/HU12A-PAID-006/emitir" `
  -Headers $headers
$reemision = $respuestaReemision.Content | ConvertFrom-Json
$respuestaReemision.StatusCode
$reemision | Format-List
```

- [ ] Responde `200 OK`.
- [ ] `creado` es `false`.
- [ ] Devuelve el mismo número que `$emision1.numero_comprobante`.
- [ ] No crea un segundo comprobante para el mismo pedido.

### MAN-08 — Secuencia de una nueva venta (CA4)

```powershell
$emision2 = Invoke-RestMethod -Method Post `
  -Uri "$base/comprobantes/HU12A-PAID-007/emitir" `
  -Headers $headers
$emision2 | Format-List

$n1 = [int]($emision1.numero_comprobante -replace 'COMP-', '')
$n2 = [int]($emision2.numero_comprobante -replace 'COMP-', '')
$n2 -eq ($n1 + 1)
```

- [ ] La comparación final devuelve `True`.
- [ ] Los números son distintos y consecutivos.

### MAN-09 — Persistencia de datos sin PDF y unicidad (CA4/CA5)

```powershell
docker exec stgc-hu12a-postgres psql -U stgc -d stgc_test -c `
  "SELECT pedido_id, numero, estado, datos IS NOT NULL AS datos_persistidos,
          jsonb_array_length(datos->'pedido'->'items') AS productos,
          jsonb_array_length(datos->'pedido'->'pagos') AS pagos
   FROM billing_service.comprobantes ORDER BY numero;"

docker exec stgc-hu12a-postgres psql -U stgc -d stgc_test -c `
  "SELECT column_name FROM information_schema.columns
   WHERE table_schema='billing_service' AND table_name='comprobantes'
   ORDER BY ordinal_position;"
```

- [ ] Hay una sola fila por pedido emitido.
- [ ] No existen números repetidos.
- [ ] `datos_persistidos` es `true` y los arreglos de productos/pagos no están vacíos.
- [ ] La tabla contiene `datos` y no contiene columnas `pdf` ni `pdf_sha256`.
- [ ] Descargar el PDF nuevamente no cambia el tamaño de la fila ni crea archivos en el servidor.

### MAN-10 — Pedido inexistente

```powershell
curl.exe -i -X POST `
  -H "Authorization: Bearer $token" `
  "$base/comprobantes/NO-EXISTE/emitir"
```

- [ ] Responde `404 Not Found` con `Pedido no encontrado.`

## Cobertura de aceptación manual

| Criterio | Evidencia manual |
|---|---|
| CA1 | MAN-03, MAN-04, MAN-06 |
| CA2 | MAN-04, MAN-06 |
| CA3 | MAN-04 |
| CA4 | MAN-07, MAN-08, MAN-09 |
| CA5 | MAN-05, MAN-06 |
| CA6 | MAN-02 |

## Detener el entorno

Cuando terminen las pruebas:

```powershell
$billingPid = (Get-NetTCPConnection -LocalPort 3002 -State Listen).OwningProcess
Stop-Process -Id $billingPid
docker stop stgc-hu12a-postgres
```
