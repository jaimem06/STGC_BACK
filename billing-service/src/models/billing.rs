use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::env;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InvoiceStatus {
    Borrador,
    Pendiente,
    Pagada,
    Anulada,
    Reembolsada,
}

impl InvoiceStatus {
    pub const ALL: [Self; 5] = [
        Self::Borrador,
        Self::Pendiente,
        Self::Pagada,
        Self::Anulada,
        Self::Reembolsada,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Borrador => "BORRADOR",
            Self::Pendiente => "PENDIENTE",
            Self::Pagada => "PAGADA",
            Self::Anulada => "ANULADA",
            Self::Reembolsada => "REEMBOLSADA",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Borrador => "La factura se está generando pero aún no se ha emitido formalmente (ideal para preventas o pedidos en mesa).",
            Self::Pendiente => "La factura ha sido emitida pero el pago aún no se ha registrado.",
            Self::Pagada => "El monto total ha sido cubierto satisfactoriamente.",
            Self::Anulada => "La factura fue cancelada por error de digitación o solicitud del cliente, invalidando el monto pero dejando registro para auditoría.",
            Self::Reembolsada => "El pago se realizó, pero el dinero fue devuelto al cliente y la factura quedó sin efecto.",
        }
    }
}

impl std::str::FromStr for InvoiceStatus {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "BORRADOR" => Ok(Self::Borrador),
            "PENDIENTE" => Ok(Self::Pendiente),
            "PAGADA" => Ok(Self::Pagada),
            "ANULADA" => Ok(Self::Anulada),
            "REEMBOLSADA" => Ok(Self::Reembolsada),
            _ => Err(format!("Estado de factura desconocido: {value}")),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct InvoiceStatusDefinition {
    pub estado: String,
    pub descripcion: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct InvoiceStatusCatalogResponse {
    pub estados: Vec<InvoiceStatusDefinition>,
}

impl InvoiceStatusCatalogResponse {
    pub fn exact_catalog() -> Self {
        Self {
            estados: InvoiceStatus::ALL
                .iter()
                .map(|status| InvoiceStatusDefinition {
                    estado: status.as_str().to_owned(),
                    descripcion: status.description().to_owned(),
                })
                .collect(),
        }
    }
}

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

/// Serie SRI por defecto (primer establecimiento / primer punto de emisión).
/// También cubre los snapshots JSONB persistidos antes de que existieran
/// estos campos (serde los rellena al deserializar).
fn serie_por_defecto() -> String {
    "001".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct BusinessInfo {
    pub nombre: String,
    pub ruc: String,
    pub direccion: String,
    /// Código de establecimiento SRI (3 dígitos, p. ej. "001").
    #[serde(default = "serie_por_defecto")]
    pub establecimiento: String,
    /// Código de punto de emisión SRI (3 dígitos, p. ej. "001").
    #[serde(default = "serie_por_defecto")]
    pub punto_emision: String,
}

fn es_serie_sri(valor: &str) -> bool {
    valor.len() == 3 && valor.chars().all(|c| c.is_ascii_digit())
}

impl BusinessInfo {
    pub fn new(
        nombre: String,
        ruc: String,
        direccion: String,
        establecimiento: String,
        punto_emision: String,
    ) -> Result<Self, String> {
        let business = Self {
            nombre,
            ruc,
            direccion,
            establecimiento,
            punto_emision,
        };
        if business.nombre.trim().is_empty()
            || business.ruc.trim().is_empty()
            || business.direccion.trim().is_empty()
        {
            return Err("BUSINESS_NAME, BUSINESS_RUC y BUSINESS_ADDRESS son obligatorios".into());
        }
        if !es_serie_sri(&business.establecimiento) || !es_serie_sri(&business.punto_emision) {
            return Err(
                "BUSINESS_ESTABLISHMENT y BUSINESS_EMISSION_POINT deben ser exactamente 3 dígitos (p. ej. 001)"
                    .into(),
            );
        }
        Ok(business)
    }

    pub fn from_env() -> Result<Self, String> {
        Self::new(
            env::var("BUSINESS_NAME").unwrap_or_default(),
            env::var("BUSINESS_RUC").unwrap_or_default(),
            env::var("BUSINESS_ADDRESS").unwrap_or_default(),
            env::var("BUSINESS_ESTABLISHMENT").unwrap_or_else(|_| serie_por_defecto()),
            env::var("BUSINESS_EMISSION_POINT").unwrap_or_else(|_| serie_por_defecto()),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct ReceiptItem {
    pub nombre: String,
    pub cantidad: i32,
    pub precio_unitario: f64,
    pub subtotal: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct ReceiptPayment {
    pub metodo_pago: String,
    pub monto: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct ReceiptOrder {
    pub pedido_id: String,
    pub estado: String,
    pub cliente_nombre: Option<String>,
    pub cliente_apellido: Option<String>,
    pub cliente_cedula: Option<String>,
    pub fecha_pago: Option<DateTime<Utc>>,
    pub subtotal: f64,
    pub iva: f64,
    pub total: f64,
    pub items: Vec<ReceiptItem>,
    pub pagos: Vec<ReceiptPayment>,
}

/// Copia histórica autosuficiente de los datos usados para emitir la factura.
/// Se persiste como JSONB; el PDF se deriva de estos datos bajo demanda.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct PersistedInvoiceData {
    pub negocio: BusinessInfo,
    pub pedido: ReceiptOrder,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct EmitReceiptResponse {
    pub message: String,
    pub numero_comprobante: String,
    pub estado_factura: InvoiceStatus,
    pub pdf_url: String,
    pub creado: bool,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BillingErrorResponse {
    pub message: String,
}
