// src/handlers/export_handler.rs
use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    Json,
};
use sqlx::PgPool;
use tracing::error;

use crate::{
    models::{ExportCsvRequest, ExportPdfRequest, ReportFilter, SalesReportFilter},
    services::{export_service, report_service},
};

// ============================================
// EXPORTAR DESDE DATOS DEL FRONTEND
// ============================================

/// Exportar a HTML/PDF desde datos enviados por el frontend
#[utoipa::path(
    post,
    path = "/reports/export/pdf/from-data",
    tag = "Exportaciones",
    request_body = ExportPdfRequest,
    responses(
        (status = 200, description = "Exportación exitosa", content_type = "text/html"),
        (status = 400, description = "Datos inválidos"),
        (status = 401, description = "No autorizado"),
        (status = 500, description = "Error interno")
    ),
    security(("bearer_auth" = []))
)]
pub async fn export_pdf_from_data(
    State(_pool): State<PgPool>,
    Json(payload): Json<ExportPdfRequest>,
) -> Result<Html<String>, (StatusCode, String)> {
    if payload.data.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "No hay datos para exportar".to_string()));
    }

    if payload.headers.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "No se especificaron encabezados".to_string()));
    }

    match export_service::export_pdf_from_data(&payload.data, &payload.headers, &payload.title) {
        Ok(html_content) => Ok(Html(html_content)),
        Err(e) => {
            error!("Error exportando PDF desde datos: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e))
        }
    }
}

/// Exportar a CSV desde datos enviados por el frontend
#[utoipa::path(
    post,
    path = "/reports/export/csv/from-data",
    tag = "Exportaciones",
    request_body = ExportCsvRequest,
    responses(
        (status = 200, description = "Exportación exitosa", content_type = "text/csv"),
        (status = 400, description = "Datos inválidos"),
        (status = 401, description = "No autorizado"),
        (status = 500, description = "Error interno")
    ),
    security(("bearer_auth" = []))
)]
pub async fn export_csv_from_data(
    State(_pool): State<PgPool>,
    Json(payload): Json<ExportCsvRequest>,
) -> Result<Response, (StatusCode, String)> {
    if payload.data.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "No hay datos para exportar".to_string()));
    }

    if payload.headers.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "No se especificaron encabezados".to_string()));
    }

    match export_service::export_csv_from_data(&payload.data, &payload.headers, &payload.title) {
        Ok(csv_data) => {
            let filename = payload.filename
                .unwrap_or_else(|| format!("{}_{}.csv", 
                    payload.title.replace(" ", "_").to_lowercase(),
                    chrono::Local::now().format("%Y%m%d_%H%M%S")
                ));
            Ok(create_csv_response(csv_data, &filename))
        },
        Err(e) => {
            error!("Error exportando CSV desde datos: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e))
        }
    }
}

// ============================================
// EXPORTAR STOCK
// ============================================

/// Exportar reporte de stock a CSV
#[utoipa::path(
    get,
    path = "/reports/export/stock/csv",
    tag = "Exportaciones",
    params(ReportFilter),
    responses(
        (status = 200, description = "Exportación exitosa", content_type = "text/csv"),
        (status = 401, description = "No autorizado"),
        (status = 500, description = "Error interno")
    ),
    security(("bearer_auth" = []))
)]
pub async fn export_stock_csv(
    State(pool): State<PgPool>,
    Query(filter): Query<ReportFilter>,
) -> Result<Response, (StatusCode, String)> {
    match report_service::get_stock_report(&pool, filter).await {
        Ok(data) => {
            let headers = vec!["ID", "SKU", "Nombre", "Tipo", "Cantidad", "Stock Mínimo", "Unidad", "Estado"];
            let headers_str: Vec<String> = headers.iter().map(|s| s.to_string()).collect();
            let json_data: Vec<serde_json::Value> = data.iter()
                .map(|r| serde_json::to_value(r).unwrap())
                .collect();
            
            let csv_data = export_service::export_csv_from_data(&json_data, &headers_str, "Reporte de Stock")
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
            Ok(create_csv_response(csv_data, "reporte_stock.csv"))
        },
        Err(e) => {
            error!("Error exportando stock a CSV: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

/// Exportar reporte de stock a HTML/PDF
#[utoipa::path(
    get,
    path = "/reports/export/stock/pdf",
    tag = "Exportaciones",
    params(ReportFilter),
    responses(
        (status = 200, description = "Exportación exitosa", content_type = "text/html"),
        (status = 401, description = "No autorizado"),
        (status = 500, description = "Error interno")
    ),
    security(("bearer_auth" = []))
)]
pub async fn export_stock_pdf(
    State(pool): State<PgPool>,
    Query(filter): Query<ReportFilter>,
) -> Result<Html<String>, (StatusCode, String)> {
    match report_service::get_stock_report(&pool, filter).await {
        Ok(data) => {
            let headers = vec!["ID", "SKU", "Nombre", "Tipo", "Cantidad", "Stock Mínimo", "Unidad", "Estado"];
            let headers_str: Vec<String> = headers.iter().map(|s| s.to_string()).collect();
            let json_data: Vec<serde_json::Value> = data.iter()
                .map(|r| serde_json::to_value(r).unwrap())
                .collect();
            
            let html_content = export_service::export_pdf_from_data(&json_data, &headers_str, "Reporte de Stock")
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
            Ok(Html(html_content))
        },
        Err(e) => {
            error!("Error exportando stock a PDF: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

// ============================================
// EXPORTAR VENTAS
// ============================================

/// Exportar reporte de ventas a CSV
#[utoipa::path(
    get,
    path = "/reports/export/sales/csv",
    tag = "Exportaciones",
    params(ReportFilter),
    responses(
        (status = 200, description = "Exportación exitosa", content_type = "text/csv"),
        (status = 401, description = "No autorizado"),
        (status = 500, description = "Error interno")
    ),
    security(("bearer_auth" = []))
)]
pub async fn export_sales_csv(
    State(pool): State<PgPool>,
    Query(filter): Query<ReportFilter>,
) -> Result<Response, (StatusCode, String)> {
    match report_service::get_sales_report(&pool, filter).await {
        Ok(data) => {
            let headers = vec!["Venta ID", "Fecha", "Empleado", "Producto", "Cantidad", "Precio Unit.", "Subtotal", "Total"];
            let headers_str: Vec<String> = headers.iter().map(|s| s.to_string()).collect();
            let json_data: Vec<serde_json::Value> = data.iter()
                .map(|r| serde_json::to_value(r).unwrap())
                .collect();
            
            let csv_data = export_service::export_csv_from_data(&json_data, &headers_str, "Reporte de Ventas")
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
            Ok(create_csv_response(csv_data, "reporte_ventas.csv"))
        },
        Err(e) => {
            error!("Error exportando ventas a CSV: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

/// Exportar reporte de ventas a HTML/PDF
#[utoipa::path(
    get,
    path = "/reports/export/sales/pdf",
    tag = "Exportaciones",
    params(ReportFilter),
    responses(
        (status = 200, description = "Exportación exitosa", content_type = "text/html"),
        (status = 401, description = "No autorizado"),
        (status = 500, description = "Error interno")
    ),
    security(("bearer_auth" = []))
)]
pub async fn export_sales_pdf(
    State(pool): State<PgPool>,
    Query(filter): Query<ReportFilter>,
) -> Result<Html<String>, (StatusCode, String)> {
    match report_service::get_sales_report(&pool, filter).await {
        Ok(data) => {
            let headers = vec!["Venta ID", "Fecha", "Empleado", "Producto", "Cantidad", "Precio Unit.", "Subtotal", "Total"];
            let headers_str: Vec<String> = headers.iter().map(|s| s.to_string()).collect();
            let json_data: Vec<serde_json::Value> = data.iter()
                .map(|r| serde_json::to_value(r).unwrap())
                .collect();
            
            let html_content = export_service::export_pdf_from_data(&json_data, &headers_str, "Reporte de Ventas")
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
            Ok(Html(html_content))
        },
        Err(e) => {
            error!("Error exportando ventas a PDF: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

// ============================================
// EXPORTAR MOVIMIENTOS
// ============================================

/// Exportar reporte de movimientos a CSV
#[utoipa::path(
    get,
    path = "/reports/export/movements/csv",
    tag = "Exportaciones",
    params(ReportFilter),
    responses(
        (status = 200, description = "Exportación exitosa", content_type = "text/csv"),
        (status = 401, description = "No autorizado"),
        (status = 500, description = "Error interno")
    ),
    security(("bearer_auth" = []))
)]
pub async fn export_movements_csv(
    State(pool): State<PgPool>,
    Query(filter): Query<ReportFilter>,
) -> Result<Response, (StatusCode, String)> {
    match report_service::get_movements_report(&pool, filter).await {
        Ok(data) => {
            let headers = vec!["Movimiento ID", "Item ID", "Producto", "Tipo", "Cantidad", "Motivo", "Usuario", "Fecha"];
            let headers_str: Vec<String> = headers.iter().map(|s| s.to_string()).collect();
            let json_data: Vec<serde_json::Value> = data.iter()
                .map(|r| serde_json::to_value(r).unwrap())
                .collect();
            
            let csv_data = export_service::export_csv_from_data(&json_data, &headers_str, "Reporte de Movimientos")
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
            Ok(create_csv_response(csv_data, "reporte_movimientos.csv"))
        },
        Err(e) => {
            error!("Error exportando movimientos a CSV: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

/// Exportar reporte de movimientos a HTML/PDF
#[utoipa::path(
    get,
    path = "/reports/export/movements/pdf",
    tag = "Exportaciones",
    params(ReportFilter),
    responses(
        (status = 200, description = "Exportación exitosa", content_type = "text/html"),
        (status = 401, description = "No autorizado"),
        (status = 500, description = "Error interno")
    ),
    security(("bearer_auth" = []))
)]
pub async fn export_movements_pdf(
    State(pool): State<PgPool>,
    Query(filter): Query<ReportFilter>,
) -> Result<Html<String>, (StatusCode, String)> {
    match report_service::get_movements_report(&pool, filter).await {
        Ok(data) => {
            let headers = vec!["Movimiento ID", "Item ID", "Producto", "Tipo", "Cantidad", "Motivo", "Usuario", "Fecha"];
            let headers_str: Vec<String> = headers.iter().map(|s| s.to_string()).collect();
            let json_data: Vec<serde_json::Value> = data.iter()
                .map(|r| serde_json::to_value(r).unwrap())
                .collect();
            
            let html_content = export_service::export_pdf_from_data(&json_data, &headers_str, "Reporte de Movimientos")
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
            Ok(Html(html_content))
        },
        Err(e) => {
            error!("Error exportando movimientos a PDF: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

// ============================================
// EXPORTAR VENTAS POR PRODUCTO
// ============================================

/// Exportar ventas por producto a CSV
#[utoipa::path(
    get,
    path = "/reports/export/sales-by-product/csv",
    tag = "Exportaciones",
    params(SalesReportFilter),
    responses(
        (status = 200, description = "Exportación exitosa", content_type = "text/csv"),
        (status = 401, description = "No autorizado"),
        (status = 500, description = "Error interno")
    ),
    security(("bearer_auth" = []))
)]
pub async fn export_sales_by_product_csv(
    State(pool): State<PgPool>,
    Query(filter): Query<SalesReportFilter>,
) -> Result<Response, (StatusCode, String)> {
    match report_service::get_sales_by_product(&pool, filter).await {
        Ok(data) => {
            let headers = vec!["Producto ID", "Producto", "Total Vendido", "Total Ingresos", "Número de Ventas"];
            let headers_str: Vec<String> = headers.iter().map(|s| s.to_string()).collect();
            let json_data: Vec<serde_json::Value> = data.iter()
                .map(|r| serde_json::to_value(r).unwrap())
                .collect();
            
            let csv_data = export_service::export_csv_from_data(&json_data, &headers_str, "Ventas por Producto")
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
            Ok(create_csv_response(csv_data, "ventas_por_producto.csv"))
        },
        Err(e) => {
            error!("Error exportando ventas por producto a CSV: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

/// Exportar ventas por producto a HTML/PDF
#[utoipa::path(
    get,
    path = "/reports/export/sales-by-product/pdf",
    tag = "Exportaciones",
    params(SalesReportFilter),
    responses(
        (status = 200, description = "Exportación exitosa", content_type = "text/html"),
        (status = 401, description = "No autorizado"),
        (status = 500, description = "Error interno")
    ),
    security(("bearer_auth" = []))
)]
pub async fn export_sales_by_product_pdf(
    State(pool): State<PgPool>,
    Query(filter): Query<SalesReportFilter>,
) -> Result<Html<String>, (StatusCode, String)> {
    match report_service::get_sales_by_product(&pool, filter).await {
        Ok(data) => {
            let headers = vec!["Producto ID", "Producto", "Total Vendido", "Total Ingresos", "Número de Ventas"];
            let headers_str: Vec<String> = headers.iter().map(|s| s.to_string()).collect();
            let json_data: Vec<serde_json::Value> = data.iter()
                .map(|r| serde_json::to_value(r).unwrap())
                .collect();
            
            let html_content = export_service::export_pdf_from_data(&json_data, &headers_str, "Ventas por Producto")
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
            Ok(Html(html_content))
        },
        Err(e) => {
            error!("Error exportando ventas por producto a PDF: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

// ============================================
// EXPORTAR VENTAS POR EMPLEADO
// ============================================

/// Exportar ventas por empleado a CSV
#[utoipa::path(
    get,
    path = "/reports/export/sales-by-employee/csv",
    tag = "Exportaciones",
    params(SalesReportFilter),
    responses(
        (status = 200, description = "Exportación exitosa", content_type = "text/csv"),
        (status = 401, description = "No autorizado"),
        (status = 500, description = "Error interno")
    ),
    security(("bearer_auth" = []))
)]
pub async fn export_sales_by_employee_csv(
    State(pool): State<PgPool>,
    Query(filter): Query<SalesReportFilter>,
) -> Result<Response, (StatusCode, String)> {
    match report_service::get_sales_by_employee(&pool, filter).await {
        Ok(data) => {
            let headers = vec!["Empleado ID", "Empleado", "Total Ventas", "Número de Ventas", "Total Productos Vendidos"];
            let headers_str: Vec<String> = headers.iter().map(|s| s.to_string()).collect();
            let json_data: Vec<serde_json::Value> = data.iter()
                .map(|r| serde_json::to_value(r).unwrap())
                .collect();
            
            let csv_data = export_service::export_csv_from_data(&json_data, &headers_str, "Ventas por Empleado")
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
            Ok(create_csv_response(csv_data, "ventas_por_empleado.csv"))
        },
        Err(e) => {
            error!("Error exportando ventas por empleado a CSV: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

/// Exportar ventas por empleado a HTML/PDF
#[utoipa::path(
    get,
    path = "/reports/export/sales-by-employee/pdf",
    tag = "Exportaciones",
    params(SalesReportFilter),
    responses(
        (status = 200, description = "Exportación exitosa", content_type = "text/html"),
        (status = 401, description = "No autorizado"),
        (status = 500, description = "Error interno")
    ),
    security(("bearer_auth" = []))
)]
pub async fn export_sales_by_employee_pdf(
    State(pool): State<PgPool>,
    Query(filter): Query<SalesReportFilter>,
) -> Result<Html<String>, (StatusCode, String)> {
    match report_service::get_sales_by_employee(&pool, filter).await {
        Ok(data) => {
            let headers = vec!["Empleado ID", "Empleado", "Total Ventas", "Número de Ventas", "Total Productos Vendidos"];
            let headers_str: Vec<String> = headers.iter().map(|s| s.to_string()).collect();
            let json_data: Vec<serde_json::Value> = data.iter()
                .map(|r| serde_json::to_value(r).unwrap())
                .collect();
            
            let html_content = export_service::export_pdf_from_data(&json_data, &headers_str, "Ventas por Empleado")
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
            Ok(Html(html_content))
        },
        Err(e) => {
            error!("Error exportando ventas por empleado a PDF: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

// ============================================
// FUNCIONES AUXILIARES
// ============================================

fn create_csv_response(data: Vec<u8>, filename: &str) -> Response {
    let disposition = format!("attachment; filename=\"{}\"", filename);
    let length = data.len().to_string();
    
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (header::CONTENT_DISPOSITION, &disposition as &str),
            (header::CONTENT_LENGTH, &length as &str),
        ],
        data,
    )
        .into_response()
}