use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, ToSchema, IntoParams)]
pub struct ReportFilter {
    #[serde(default)]
    pub fecha_inicio: Option<DateTime<Utc>>,

    #[serde(default)]
    pub fecha_fin: Option<DateTime<Utc>>,

    #[serde(default)]
    pub producto_id: Option<String>,

    #[serde(default)]
    pub empleado_id: Option<String>,

    #[serde(default)]
    pub tipo: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, IntoParams)]
pub struct SalesReportFilter {
    #[serde(default)]
    pub fecha_inicio: Option<DateTime<Utc>>,

    #[serde(default)]
    pub fecha_fin: Option<DateTime<Utc>>,

    #[serde(default)]
    pub producto_id: Option<String>,

    #[serde(default)]
    pub empleado_id: Option<String>,

    #[serde(default)]
    pub periodo: Option<String>, // "dia", "semana", "mes"
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
    pub venta_id: String,
    pub fecha: DateTime<Utc>,
    pub empleado: String,
    pub producto: String,
    pub cantidad: f64,
    pub precio_unitario: f64,
    pub subtotal: f64,
    pub total: f64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, FromRow)]
pub struct DashboardReport {
    pub total_productos: i64,
    pub stock_bajo: i64,
    pub productos_agotados: i64,
    pub total_movimientos: i64,
    pub total_ventas: f64,
}

// ============================================
// NUEVOS MODELOS PARA REPORTES DE VENTAS
// ============================================

/// Reporte de ventas por período (día, semana, mes)
#[derive(Debug, Serialize, Deserialize, ToSchema, FromRow)]
pub struct VentaPeriodoReport {
    pub periodo: String,
    pub total_ventas: f64,
    pub numero_ventas: i64,
    pub total_productos_vendidos: f64,
}

/// Reporte de ventas por producto
#[derive(Debug, Serialize, Deserialize, ToSchema, FromRow)]
pub struct VentaProductoReport {
    pub producto_id: String,
    pub producto_nombre: String,
    pub total_vendido: f64,
    pub total_ingresos: f64,
    pub numero_ventas: i64,
}

/// Reporte de ventas por empleado (todos los empleados)
#[derive(Debug, Serialize, Deserialize, ToSchema, FromRow)]
pub struct VentaEmpleadoReport {
    pub empleado_id: Option<String>,
    pub empleado_nombre: String,
    pub total_ventas: f64,
    pub numero_ventas: i64,
    pub total_productos_vendidos: f64,
}

/// Reporte de ventas de un empleado específico
#[derive(Debug, Serialize, Deserialize, ToSchema, FromRow)]
pub struct VentaEmpleadoDetalleReport {
    pub venta_id: String,
    pub fecha: DateTime<Utc>,
    pub producto: String,
    pub cantidad: f64,
    pub precio_unitario: f64,
    pub subtotal: f64,
    pub total: f64,
}