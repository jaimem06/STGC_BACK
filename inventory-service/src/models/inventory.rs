use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use super::enums::{EstadoInventario, TipoElemento, UnidadMedida, TipoMovimiento, ModuloInventario, CalidadCafe, FaseCafe};

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateInventarioItem {
    pub sku: String,
    pub nombre: String,
    pub tipo: TipoElemento,
    pub estado: EstadoInventario,
    pub unidad_medida: UnidadMedida,
    pub precio: f64,
    #[serde(default)]
    pub descripcion: Option<String>,
    #[serde(default)]
    pub fecha_caducidad: Option<String>,
    pub modulo: ModuloInventario,
    #[serde(default)]
    pub stock_minimo: Option<f64>,
    #[serde(default)]
    pub codigo_trazabilidad: Option<Uuid>,
    #[serde(default)]
    pub calidad: Option<CalidadCafe>,
    #[serde(default)]
    pub fase_produccion: Option<FaseCafe>,
    #[serde(default)]
    pub cantidad_inicial: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct InventarioItem {
    pub id: Uuid,
    pub sku: String,
    pub nombre: String,
    pub cantidad: f64,
    pub tipo: TipoElemento,
    pub estado: EstadoInventario,
    pub unidad_medida: UnidadMedida,
    pub precio: f64,
    pub descripcion: Option<String>,
    pub fecha_caducidad: Option<DateTime<Utc>>,
    pub modulo: ModuloInventario,
    pub stock_minimo: f64,
    pub is_deleted: bool,
    pub codigo_trazabilidad: Option<Uuid>,
    pub calidad: Option<CalidadCafe>,
    pub fase_produccion: Option<FaseCafe>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateInventarioItem {
    #[serde(default)]
    pub nombre: Option<String>,
    #[serde(default)]
    pub estado: Option<EstadoInventario>,
    #[serde(default)]
    pub precio: Option<f64>,
    #[serde(default)]
    pub descripcion: Option<String>,
    #[serde(default)]
    pub stock_minimo: Option<f64>,
    #[serde(default)]
    pub unidad_medida: Option<UnidadMedida>,
    #[serde(default)]
    pub fecha_caducidad: Option<String>,
    /// Motivo del cambio de precio (HU019). Se registra en historial_precios si el precio cambia.
    #[serde(default)]
    pub motivo: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateEstadoDto {
    pub estado: EstadoInventario,
    /// Motivo del cambio manual de estado (HU025). Se registra en historial_estados.
    #[serde(default)]
    pub motivo: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct MovimientoStock {
    pub id: Uuid,
    pub item_id: Uuid,
    pub cantidad: f64,
    pub tipo: TipoMovimiento,
    pub fecha: DateTime<Utc>,
    pub motivo: String,
    #[serde(default)]
    pub lote_id: Option<Uuid>,
    pub numero_factura: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateMovimientoDto {
    pub item_id: Uuid,
    pub cantidad: f64,
    pub tipo: TipoMovimiento,
    pub motivo: String,
    #[serde(default)]
    pub lote_id: Option<Uuid>,
    /// Número de factura/documento asociado a la entrada (HU021, EC-04).
    #[serde(default)]
    pub numero_factura: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct AlertaStock {
    pub item_id: Uuid,
    pub nombre: String,
    pub cantidad_actual: f64,
    pub stock_minimo: f64,
    pub mensaje: String,
}

/// Registro histórico de cambios de precio (HU019).
#[derive(Debug, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct HistorialPrecio {
    pub id: Uuid,
    pub item_id: Uuid,
    pub precio_anterior: f64,
    pub precio_nuevo: f64,
    pub motivo: Option<String>,
    pub usuario_id: Option<String>,
    pub fecha_cambio: DateTime<Utc>,
}

/// Registro histórico de cambios de estado (HU025).
#[derive(Debug, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct HistorialEstado {
    pub id: Uuid,
    pub item_id: Uuid,
    pub estado_anterior: EstadoInventario,
    pub estado_nuevo: EstadoInventario,
    pub motivo: Option<String>,
    pub usuario_id: Option<String>,
    pub fecha: DateTime<Utc>,
}

/// Métricas consolidadas de inventario para el dashboard (HU028).
#[derive(Debug, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct StockStats {
    pub total_items: i64,
    pub disponibles: i64,
    pub stock_bajo: i64,
    pub agotados: i64,
    pub valor_total: f64,
    pub num_lotes: i64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct Proveedor {
    pub id: Uuid,
    pub nombre: String,
    pub contacto: String,
    pub tipo_insumo: String,
}
