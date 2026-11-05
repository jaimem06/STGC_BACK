#!/usr/bin/env bash
# Script de construcción para despliegue (p. ej. Render).
# Comando de arranque recomendado:
#   uvicorn app.main:app --host 0.0.0.0 --port $PORT --no-server-header
set -o errexit

# Guardar los motores de Prisma dentro del proyecto
export PRISMA_BINARY_CACHE_DIR="$(pwd)/.prisma-binaries"

pip install -r requirements.txt
prisma generate
