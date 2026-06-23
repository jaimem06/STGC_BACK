use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateEntradaFacturaDto {
    pub item_id: Uuid,
    pub cantidad: f64,
    pub unidad_medida: String,
    pub numero_factura: String,
    pub fecha_entrada: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MovimientoStockDto {
    pub id: Uuid,
    pub item_id: Uuid,
    pub cantidad: f64,
    pub tipo: String,
    pub fecha: DateTime<Utc>,
    pub motivo: String,
    pub numero_factura: Option<String>,
}
