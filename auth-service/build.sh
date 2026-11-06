#!/usr/bin/env bash
# Build para despliegue (p. ej. Render).
#   Build command:      ./build.sh
#   Pre-deploy command: alembic upgrade head
#   Start command:      uvicorn app.main:app --host 0.0.0.0 --port $PORT --no-server-header
set -o errexit

pip install -r requirements.txt
