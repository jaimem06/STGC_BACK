param(
    [switch]$IncludeDatabase,
    [switch]$SkipToolInstall
)

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $projectRoot

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $workspaceCargo = Resolve-Path "$projectRoot\..\..\tools\rust\cargo\bin\cargo.exe" -ErrorAction SilentlyContinue
    if ($workspaceCargo) {
        $env:PATH = "$(Split-Path $workspaceCargo);$env:PATH"
        $env:CARGO_HOME = Resolve-Path "$projectRoot\..\..\tools\rust\cargo"
        $env:RUSTUP_HOME = Resolve-Path "$projectRoot\..\..\tools\rust\rustup"
    } else {
        throw "Cargo no está instalado o no está disponible en PATH."
    }
}

if ($IncludeDatabase -and -not $env:TEST_DATABASE_URL) {
    throw "TEST_DATABASE_URL es obligatorio con -IncludeDatabase y debe apuntar a una base exclusiva de pruebas."
}

if (-not $SkipToolInstall) {
    if (Get-Command rustup -ErrorAction SilentlyContinue) {
        rustup component add llvm-tools-preview | Out-Host
    }
    cargo llvm-cov --version *> $null
    if ($LASTEXITCODE -ne 0) {
        cargo install cargo-llvm-cov --locked
    }
}

$testTail = @()
if ($IncludeDatabase) {
    $testTail = @("--", "--include-ignored")
}

Write-Host "Ejecutando cargo test..."
& cargo test --all-features --no-fail-fast @testTail
if ($LASTEXITCODE -ne 0) { throw "Las pruebas fallaron." }

$coverageDir = Join-Path $projectRoot "coverage"
New-Item -ItemType Directory -Force -Path $coverageDir | Out-Null

Write-Host "Recolectando cobertura con cargo-llvm-cov..."
cargo llvm-cov clean --workspace
& cargo llvm-cov --workspace --all-features --no-report @testTail
if ($LASTEXITCODE -ne 0) { throw "La ejecución instrumentada de cobertura falló." }

cargo llvm-cov report --html --output-dir "$coverageDir"
cargo llvm-cov report --json --output-path "$coverageDir\coverage.json"

$report = Get-Content -Raw "$coverageDir\coverage.json" | ConvertFrom-Json
$totals = $report.data[0].totals
$featureFiles = $report.data[0].files | Where-Object {
    $_.filename -match "receipt_service.rs|comprobantes_handler.rs|models.billing.rs"
}
$featureRows = ($featureFiles | ForEach-Object {
    "| $([System.IO.Path]::GetFileName($_.filename)) | $($_.summary.lines.covered) | $($_.summary.lines.count) | $([math]::Round($_.summary.lines.percent, 2))% |"
}) -join "`n"
$generatedAt = (Get-Date).ToString("yyyy-MM-dd HH:mm:ss K")
$summary = @"
# Informe automatico de cobertura HU12-A

Generado: $generatedAt

| Metrica | Cubierto | Total | Porcentaje |
|---|---:|---:|---:|
| Lineas | $($totals.lines.covered) | $($totals.lines.count) | $([math]::Round($totals.lines.percent, 2))% |
| Funciones | $($totals.functions.covered) | $($totals.functions.count) | $([math]::Round($totals.functions.percent, 2))% |
| Regiones | $($totals.regions.covered) | $($totals.regions.count) | $([math]::Round($totals.regions.percent, 2))% |

## Archivos principales de HU12-A

| Archivo | Lineas cubiertas | Total | Porcentaje |
|---|---:|---:|---:|
$featureRows

- Resultado de pruebas: correcto.
- Reporte HTML: `coverage/html/index.html`.
- Datos JSON: `coverage/coverage.json`.
- Pruebas de base de datos incluidas: $IncludeDatabase.
"@
$summary | Set-Content -Encoding UTF8 "$coverageDir\coverage-summary.md"

Write-Host "Cobertura generada en $coverageDir\html\index.html"
Get-Content "$coverageDir\coverage-summary.md"
