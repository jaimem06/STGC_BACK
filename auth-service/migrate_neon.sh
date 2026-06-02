#!/bin/bash

# ==============================================================================
# SCRIPT DE AUTOMATIZACIÓN: UNIFICACIÓN Y MIGRACIÓN NEON DB
# ==============================================================================
# Este script automatiza la introspección de Trazabilidad, la actualización
# del esquema en Producción y la migración de datos de tablas específicas.
# ==============================================================================

set -e # Detener el script si ocurre algún error

# Colores para la terminal
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# --- CONFIGURACIÓN DE CONEXIONES ---
URL_PRODUCCION="postgresql://neondb_owner:npg_Fqpske37cLnX@ep-super-bonus-acnalzxr.sa-east-1.aws.neon.tech/neondb?sslmode=require&channel_binding=require"
URL_TRAZABILIDAD="postgresql://neondb_owner:npg_Fqpske37cLnX@ep-cool-frog-acyfdgft.sa-east-1.aws.neon.tech/neondb?sslmode=require&channel_binding=require"

# --- CONFIGURACIÓN DE TABLAS A MIGRAR ---
# Orden jerárquico para respetar Foreign Keys:
TABLAS_MOVIL=("catalogo" "parcelas" "semillas" "lotes" "asignacion_personal" "estado_etapa")

echo -e "${BLUE}>>> Iniciando proceso de unificación y migración...${NC}\n"

# ------------------------------------------------------------------------------
# PASO 1: INTROSPECCIÓN (SALTADO - YA FUSIONADO MANUALMENTE)
# ------------------------------------------------------------------------------
echo -e "${BLUE}>>> Saltando Paso 1 (Esquema ya unificado manualmente)...${NC}\n"
# export DATABASE_URL="$URL_TRAZABILIDAD"
# npx prisma@5 db pull

# ------------------------------------------------------------------------------
# PASO 2: MIGRACIÓN DEL ESQUEMA (SALTADO - YA COMPLETADO)
# ------------------------------------------------------------------------------
echo -e "${BLUE}>>> Saltando Paso 2 (Esquema ya sincronizado)...${NC}\n"
# export DATABASE_URL="$URL_PRODUCCION"
# npx prisma@5 db push

echo -e "${GREEN}✔ Esquema sincronizado en Producción.${NC}\n"

# ------------------------------------------------------------------------------
# PASO 3: EXTRACCIÓN E INYECCIÓN DE DATOS (Data Migration)
# ------------------------------------------------------------------------------
echo -e "${YELLOW}[PASO 3] Migrando datos de las tablas especificadas...${NC}"

DUMP_FILE="dump_trazabilidad.sql"

# Limpiar archivo de dump si ya existe
rm -f "$DUMP_FILE"

for TABLA in "${TABLAS_MOVIL[@]}"
do
    echo -e "${BLUE}--- Exportando datos de la tabla: $TABLA ---${NC}"
    # pg_dump: -a (solo datos), -t (tabla específica)
    pg_dump "$URL_TRAZABILIDAD" -a -t "$TABLA" >> "$DUMP_FILE"
done

if [ -f "$DUMP_FILE" ]; then
    echo -e "${BLUE}--- Inyectando datos en PRODUCCIÓN ---${NC}"
    # psql para inyectar los datos. 
    # Nota: Se asume que las tablas ya existen en destino gracias al Paso 2.
    psql "$URL_PRODUCCION" -f "$DUMP_FILE"
    
    echo -e "${YELLOW}--- Limpiando archivos temporales ---${NC}"
    rm "$DUMP_FILE"
    echo -e "${GREEN}✔ Migración de datos completada con éxito.${NC}\n"
else
    echo -e "${RED}⚠ No se generó ningún archivo de datos. Revisa los nombres de las tablas.${NC}\n"
fi

echo -e "${GREEN}================================================================${NC}"
echo -e "${GREEN}      PROCESO FINALIZADO CORRECTAMENTE EN NEON DB               ${NC}"
echo -e "${GREEN}================================================================${NC}"
