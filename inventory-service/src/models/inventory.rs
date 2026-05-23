use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use super::enums::{EstadoProducto, TipoElemento, UnidadMedida, TipoMovimiento};

#[derive(Debug, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct InventarioItem {
    pub id: Uuid,
    pub sku: String,
    pub nombre: String,
    pub cantidad: f64,
    pub tipo: TipoElemento,
    pub estado: EstadoProducto,
    pub unidad_medida: UnidadMedida,
    pub precio: f64,
    pub descripcion: Option<String>,
    pub fecha_caducidad: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct MovimientoStock {
    pub id: Uuid,
    pub item_id: Uuid,
    pub cantidad: f64,
    pub tipo: TipoMovimiento,
    pub fecha: DateTime<Utc>,
    pub motivo: String,
    pub lote_id: Option<Uuid>, // Relación opcional con un Lote de Café
}

#[derive(Debug, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct Proveedor {
    pub id: Uuid,
    pub nombre: String,
    pub contacto: String,
    pub tipo_insumo: String, // Podría ser un enum o ClasificacionInsumo
}
