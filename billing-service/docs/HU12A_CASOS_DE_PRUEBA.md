# Casos de prueba y trazabilidad — HU12-A

## 1. Base y supuestos

- Base: HU12-A y CA1–CA6 suministrados.
- API: `GET /api/billing/facturas/estados`, `POST /api/billing/comprobantes/{pedido_id}/emitir` y `GET /api/billing/comprobantes/{pedido_id}/pdf`.
- Estados POS: `EN_EDICION`, `PENDIENTE_PAGO`, `PAGADO`, `ANULADO`.
- Estados de factura: `BORRADOR`, `PENDIENTE`, `PAGADA`, `ANULADA`, `REEMBOLSADA`.
- Contrato POS oficial: nombre completo en `cliente_nombre` y pago único en `Pedido.metodoPago`.
- La integración usa una base exclusiva con `pos_service` y la migración HU12-A.
- Técnicas: partición de equivalencia, transición de estados, tabla de decisión, casos de uso y error guessing.

## 2. Condiciones de prueba

| TCnd ID | Base | Condición | Riesgo/intención |
|---|---|---|---|
| CND-01 | CA1 | Datos fiscales, cliente y metadatos completos/incompletos | Soporte legal incompleto. |
| CND-02 | CA2 | Uno/múltiples productos y pagos | Detalle o importes omitidos. |
| CND-03 | CA3 | Confirmación posterior a persistencia | Informar éxito sin documento. |
| CND-04 | CA4 | Emisiones consecutivas, concurrentes y repetidas | Números duplicados. |
| CND-05 | CA5 | Firma, MIME, nombre y parseabilidad PDF | Formato incorrecto/corrupto. |
| CND-06 | CA6 | `PAGADO` frente a estados no pagados | Emitir sin pago. |
| CND-07 | CA1–CA6 | Fallos DB/PDF y atomicidad | Comprobante parcial. |
| CND-08 | API | Pedido inexistente y autenticación | Acceso indebido. |
| CND-09 | Estados de factura | Catálogo exacto y estado posterior a emisión | Valores distintos a la definición del proyecto o confusión con estados POS. |
| CND-10 | Persistencia | Snapshot completo sin columnas PDF/BYTEA | Pérdida histórica o crecimiento innecesario de BD/servidor. |
| CND-11 | Compatibilidad POS | Esquema oficial y variante ampliada | Consultas a columnas/tablas inexistentes. |

## 3. Matriz de casos

| ID | HU/CA | Escenario | Precondiciones/datos | Pasos | Resultado esperado | Nivel/tipo | Prioridad | Automatización |
|---|---|---|---|---|---|---|---|---|
| CP-01 | CA1–CA5 | Emisión completa | `PAGADO`, datos completos, 1 producto/efectivo | POST y GET de `pdf_url` | 201, mensaje/número y snapshot; PDF temporal con todos los campos | Integración/Aceptación | Alta | Condicional DB |
| CP-02 | CA1 | Negocio completo | nombre/RUC/dirección | Generar y extraer texto | Aparecen los tres valores | Componente/caja blanca | Alta | Sí |
| CP-03 | CA1 | Cliente incompleto | un campo nulo/vacío por ejecución | Emitir | 422; no persiste | Componente/negativa | Alta | Sí lógica; DB condicional |
| CP-04 | CA1 | Metadatos exactos | pago `2026-07-15T14:05:06.123Z` | Leer PDF | Fecha, hora con ms UTC y pedido completo | Componente/funcional | Alta | Sí |
| CP-05 | CA2 | Detalle/totales | 2 × 5; subtotal 10; IVA 1.50; total 11.50 | Leer PDF | Todos los valores visibles | Componente/funcional | Alta | Sí |
| CP-06 | CA2 | Pagos múltiples | efectivo 5.50 + débito 6.00 | Leer PDF | Ambos métodos/montos | Componente/funcional | Alta | Sí |
| CP-07 | CA2 | Sin productos/pagos | listas vacías | Validar/emisión | 422; no número | Componente/negativa | Alta | Sí lógica |
| CP-08 | CA3 | Confirmación | commit correcto | POST | Mensaje exacto y URL | Componente/API | Alta | Sí contrato |
| CP-09 | CA4 | Dos ventas consecutivas | pedidos A/B | Emitir ambos | B=A+1, únicos | Integración/datos | Alta | Sí con `TEST_DATABASE_URL` |
| CP-10 | CA4 | Concurrencia | dos pedidos pagados | POST paralelos | Sin números duplicados | Integración/concurrencia | Alta | Condicional |
| CP-11 | CA4 | Reemisión | comprobante existente | Repetir POST | 200, `creado=false`, mismo número, una fila | Integración/regresión | Alta | Condicional |
| CP-12 | CA5 | Exportación PDF bajo demanda | comprobante emitido | GET repetido | `%PDF-`, MIME PDF, `.pdf`, `nosniff`, `no-store`; cero BYTEA/archivo persistido | Integración/API | Alta | Firma sí; HTTP condicional |
| CP-13 | CA5 | PDF corrupto | fixture corrupto | GET | 500; no entrega como PDF | Integración/negativa | Media | Condicional |
| CP-14 | CA6 | Estados no pagados | edición, pendiente, anulado, inválido | POST por estado | 409, mensaje exacto, cero filas | Componente/transición | Alta | Sí lógica; DB condicional |
| CP-15 | API | Pedido inexistente | ID aleatorio | POST/GET | 404 | Integración/negativa | Media | Condicional |
| CP-16 | API | Token ausente/inválido | sin Bearer/Bearer inválido | POST/GET | Middleware rechaza; no emite | Sistema/seguridad | Alta | Condicional auth |
| CP-17 | CA1–CA5 | Texto largo | campos >42 caracteres | Generar/revisar | Ajuste multilínea sin truncar | Componente/límite | Media | Lógica sí; visual condicional |
| CP-18 | CA3/CA4 | Falla DB/PDF | error inyectado | Emitir | 500 y rollback sin fila parcial | Integración/recuperación | Alta | Condicional |
| CP-19 | Estados de factura | Catálogo exacto | Token válido | GET `/facturas/estados` | 200; cinco estados en el orden y con las descripciones definidas | Componente/API | Alta | Sí |
| CP-20 | Estados de factura | Emisión de pedido pagado | Pedido `PAGADO` completo | POST emitir | 201/200; `estado_factura=PAGADA`; DB en `PAGADA` | Integración/regresión | Alta | Contrato sí; DB condicional |
| CP-21 | Persistencia | Snapshot autosuficiente | Emitir factura | Consultar `datos` JSONB | Contiene negocio, pedido, cliente, productos, totales y pagos | Integración/DB | Alta | Condicional |
| CP-22 | Persistencia | Ausencia de PDF persistido | Migraciones aplicadas | Inspeccionar columnas | No existen `pdf` ni `pdf_sha256`; sí existe `datos` | Migración/caja blanca | Alta | Sí contrato; DB condicional |
| CP-23 | POS oficial | Nombre completo sin apellido separado | `cliente_nombre=María Fernanda López` | Emitir | Persiste nombre `María Fernanda` y apellido `López` | Componente/compatibilidad | Alta | Sí |
| CP-24 | POS oficial | Pago único en pedido | Sin tabla `Pago`, `metodoPago=EFECTIVO` | Emitir | Persiste un pago por el total | Integración/compatibilidad | Alta | Condicional |
| CP-25 | POS ampliado | Pagos múltiples | Tabla `Pago` con dos filas | Emitir | Persiste ambas formas y montos | Integración/regresión | Media | Condicional |

## 4. Tabla de decisión

| Estado | ¿Emite? | HTTP | Mensaje CA6 |
|---|---:|---:|---|
| `PAGADO` con datos completos | Sí | 201/200 | No aplica |
| `EN_EDICION` | No | 409 | Exacto |
| `PENDIENTE_PAGO` | No | 409 | Exacto |
| `ANULADO` | No | 409 | Exacto |
| Otro/no válido | No | 409 | Exacto |

## 5. Trazabilidad

| Criterio | Casos | Cobertura | Brecha |
|---|---|---|---|
| CA1 | CP-01–CP-04, CP-17 | Cubierto | Integración requiere DB. |
| CA2 | CP-01, CP-05–CP-07 | Cubierto | Ninguna en lógica pura. |
| CA3 | CP-08, CP-18 | Cubierto | Atomicidad requiere DB. |
| CA4 | CP-09–CP-11, CP-18 | Parcialmente cubierto | Ejecutar integración DB/concurrencia. |
| CA5 | CP-01, CP-12–CP-13 | Cubierto | Cabeceras requieren integración. |
| CA6 | CP-14 | Cubierto | Persistencia negativa requiere integración. |

Resumen: 6 criterios; 5 cubiertos por componente/contrato y 1 parcialmente cubierto hasta ejecutar integración DB. No se declara 100% de aceptación sin esa evidencia.

El requisito complementario de estados queda cubierto por CP-19 y CP-20. La persistencia y compatibilidad POS quedan cubiertas por CP-21–CP-25. Los estados se restringen en PostgreSQL mediante el enum `billing_service.estado_factura`.

## 6. Ejecución

```powershell
.\scripts\test_and_coverage.ps1
$env:TEST_DATABASE_URL='postgresql://.../stgc_test'
.\scripts\test_and_coverage.ps1 -IncludeDatabase
```

Linux/macOS: `./scripts/test_and_coverage.sh`; con DB: `INCLUDE_DATABASE=true TEST_DATABASE_URL=... ./scripts/test_and_coverage.sh`.
