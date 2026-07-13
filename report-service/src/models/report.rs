use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ReportFilter {
    #[serde(default)]
    pub fecha_inicio: Option<DateTime<Utc>>,

    #[serde(default)]
    pub fecha_fin: Option<DateTime<Utc>>,

    #[serde(default)]
    pub producto_id: Option<Uuid>,

    #[serde(default)]
    pub empleado_id: Option<String>,

    #[serde(default)]
    pub tipo: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, FromRow)]
pub struct StockReport {
    pub item_id: Uuid,
    pub sku: String,
    pub nombre: String,
    pub tipo: String,
    pub cantidad: f64,
    pub stock_minimo: f64,
    pub unidad_medida: String,
    pub estado: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, FromRow)]
pub struct MovimientoReport {
    pub movimiento_id: Uuid,
    pub item_id: Uuid,
    pub producto: String,
    pub tipo_movimiento: String,
    pub cantidad: f64,
    pub motivo: String,
    pub usuario_id: Option<String>,
    pub fecha: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, FromRow)]
pub struct VentaReport {
    pub venta_id: Uuid,
    pub fecha: DateTime<Utc>,
    pub empleado: String,
    pub producto: String,
    pub cantidad: f64,
    pub precio_unitario: f64,
    pub subtotal: f64,
    pub total: f64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, FromRow)]  // <--- FromRow añadido
pub struct DashboardReport {
    pub total_productos: i64,
    pub stock_bajo: i64,
    pub productos_agotados: i64,
    pub total_movimientos: i64,
    pub total_ventas: f64,
}