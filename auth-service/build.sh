#### Script para construir el servicio en Render (Production) ####

#!/usr/bin/env bash
# Salir si ocurre un error
set -o errexit

# Le dice a Prisma que guarde los motores en la carpeta actual del proyecto
export PRISMA_BINARY_CACHE_DIR="$(pwd)/.prisma-binaries"

# Instalar dependencias y compilar Prisma
pip install -r requirements.txt
prisma generate