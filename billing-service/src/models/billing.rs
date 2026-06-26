use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateEntradaFacturaDto {
    /// ID único del ítem en el inventario al que aplica la factura.
    #[schema(example = "d290f1ee-6c54-4b01-90e6-d701748f0851")]
    pub item_id: Uuid,
    
    /// Cantidad a ingresar o retirar. Será automáticamente convertida a la unidad de medida base del ítem.
    #[schema(example = 100.5)]
    pub cantidad: f64,
    
    /// Unidad de medida en la que está expresada la cantidad.
    #[schema(example = "QUINTALES")]
    pub unidad_medida: String,
    
    /// Código de factura o documento de respaldo. Debe tener exactamente 17 caracteres alfanuméricos únicos por producto.
    #[schema(example = "FACT1234567890123")]
    pub numero_factura: String,
    
    /// Fecha de emisión o registro de la factura. Formato AAAA-MM-DD HH:MM:SS. Si no se envía, se toma la fecha actual.
    #[schema(example = "2023-12-01 10:30:00")]
    pub fecha_entrada: Option<String>,
    
    /// Fecha de caducidad aplicable a este lote. Requerido o recomendado para productos perecederos.
    #[schema(example = "2025-12-31 23:59:59")]
    pub fecha_caducidad: Option<String>,
    
    /// Determina la dirección del movimiento en el stock. Valores permitidos: ENTRADA, SALIDA.
    #[schema(example = "ENTRADA")]
    pub tipo: String,
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
