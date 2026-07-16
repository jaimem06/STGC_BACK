#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

INCLUDE_DATABASE="${INCLUDE_DATABASE:-false}"
if [[ "$INCLUDE_DATABASE" == "true" && -z "${TEST_DATABASE_URL:-}" ]]; then
  echo "TEST_DATABASE_URL es obligatorio cuando INCLUDE_DATABASE=true." >&2
  exit 1
fi

rustup component add llvm-tools-preview
if ! cargo llvm-cov --version >/dev/null 2>&1; then
  cargo install cargo-llvm-cov --locked
fi

TEST_TAIL=()
if [[ "$INCLUDE_DATABASE" == "true" ]]; then
  TEST_TAIL=(-- --include-ignored)
fi

cargo test --all-features --no-fail-fast "${TEST_TAIL[@]}"
mkdir -p coverage
cargo llvm-cov clean --workspace
cargo llvm-cov --workspace --all-features --no-report "${TEST_TAIL[@]}"
cargo llvm-cov report --html --output-dir coverage
cargo llvm-cov report --json --output-path coverage/coverage.json

python - <<'PY'
import datetime
import json
from pathlib import Path

report = json.loads(Path("coverage/coverage.json").read_text(encoding="utf-8"))
totals = report["data"][0]["totals"]
rows = []
for key, label in (("lines", "Líneas"), ("functions", "Funciones"), ("regions", "Regiones")):
    value = totals[key]
    rows.append(f"| {label} | {value['covered']} | {value['count']} | {value['percent']:.2f}% |")
feature_rows = []
for file in report["data"][0]["files"]:
    if any(name in file["filename"] for name in ("receipt_service.rs", "comprobantes_handler.rs", "models\\billing.rs", "models/billing.rs")):
        value = file["summary"]["lines"]
        feature_rows.append(f"| {Path(file['filename']).name} | {value['covered']} | {value['count']} | {value['percent']:.2f}% |")
summary = "\n".join([
    "# Informe automático de cobertura HU12-A",
    "",
    f"Generado: {datetime.datetime.now().astimezone().isoformat(timespec='seconds')}",
    "",
    "| Métrica | Cubierto | Total | Porcentaje |",
    "|---|---:|---:|---:|",
    *rows,
    "",
    "## Archivos principales de HU12-A",
    "",
    "| Archivo | Líneas cubiertas | Total | Porcentaje |",
    "|---|---:|---:|---:|",
    *feature_rows,
    "",
    "- Resultado de pruebas: correcto.",
    "- Reporte HTML: `coverage/html/index.html`.",
    "- Datos JSON: `coverage/coverage.json`.",
])
Path("coverage/coverage-summary.md").write_text(summary + "\n", encoding="utf-8")
print(summary)
PY
