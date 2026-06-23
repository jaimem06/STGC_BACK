#!/usr/bin/env bash
# Script de construcción:

set -o errexit

echo "Iniciando proceso de compilación..."

cargo build --release

echo "Compilación completada exitosamente."
echo "El binario está listo en target/release/inventory-service"
