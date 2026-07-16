// src/models/export.rs
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

/// Solicitud de exportación a PDF desde datos del frontend
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ExportPdfRequest {
    /// Título del reporte
    pub title: String,
    
    /// Encabezados de la tabla (ordenados)
    pub headers: Vec<String>,
    
    /// Datos a exportar (lo que el frontend está mostrando)
    pub data: Vec<Value>,
    
    /// Nombre del archivo (opcional)
    #[serde(default)]
    pub filename: Option<String>,
    
    /// Columnas a incluir (opcional, si no se envía, se usan todas)
    #[serde(default)]
    pub columns: Option<Vec<String>>,
}

/// Solicitud de exportación a CSV desde datos del frontend
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ExportCsvRequest {
    /// Título del reporte
    pub title: String,
    
    /// Encabezados de la tabla (ordenados)
    pub headers: Vec<String>,
    
    /// Datos a exportar (lo que el frontend está mostrando)
    pub data: Vec<Value>,
    
    /// Nombre del archivo (opcional)
    #[serde(default)]
    pub filename: Option<String>,
}