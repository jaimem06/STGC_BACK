use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use super::enums::{FaseCafe, UnidadMedida, CalidadCafe};

#[derive(Debug, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct LoteCafe {
    pub id: Uuid,
    pub variedad: String,
    pub fase: FaseCafe,
    pub cantidad_producida: f64,
    pub costo_produccion: f64,
    pub unidad_medida: UnidadMedida,
    pub calidad: CalidadCafe,
    pub codigo_trazabilidad: Uuid,
    pub lote_anterior_id: Option<Uuid>, // Para mantener la cadena de trazabilidad entre fases
    pub fecha_creacion: DateTime<Utc>,
}

/* 
INTEGRACIÓN MÓDULO A Y B:
Los Lotes de Café (Módulo A) se integran con el Inventario (Módulo B) 
a través de MovimientoStock. Cada vez que un lote cambia de fase (ej: Secado a Tostado):
1. Se registra la salida de stock del lote en la fase anterior.
2. Se registra la entrada de stock del nuevo lote en la fase actual.
3. El `codigo_trazabilidad` permite rastrear el origen desde la pulpa hasta el producto final.
*/
