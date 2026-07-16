# Informe de cobertura — HU12-A

El reporte se genera con `scripts/test_and_coverage.ps1` o `.sh` usando `cargo test` y `cargo-llvm-cov`.

Salidas locales derivadas (no versionadas):

- `coverage/html/index.html`: detalle por archivo/línea.
- `coverage/coverage.json`: datos procesables.
- `coverage/coverage-summary.md`: resumen automático de líneas, funciones y regiones.

La cobertura instrumental no sustituye la trazabilidad de CA1–CA6. Las pruebas de componente cubren validación, contenido/parseabilidad PDF, pagos múltiples, confirmación, formato numérico y contrato de migración. La prueba ignorada valida numeración real al proporcionar una base exclusiva mediante `TEST_DATABASE_URL`.

Quality gate sugerido: cero fallos en `cargo test`, al menos 80% de líneas del módulo nuevo y evidencia de integración para los comportamientos DB/HTTP.

## Última ejecución local

Ejecución del 2026-07-15 (sin base de integración automatizada): 14 pruebas correctas, 0 fallidas y 1 prueba DB omitida. Cobertura global del binario: 49.60% de líneas (624/1258). `receipt_service.rs` alcanzó 91.76% (479/522), `comprobantes_handler.rs` 27.09% (107/395) y `billing.rs` 71.70% (38/53). Las pruebas nuevas validan el catálogo de estados, adaptación de nombre del POS y migraciones sin blobs PDF. Adicionalmente se ejecutaron validaciones manuales reales contra PostgreSQL para las variantes POS oficial y ampliada, persistencia JSONB y descarga on-demand sin crecimiento de tabla.
