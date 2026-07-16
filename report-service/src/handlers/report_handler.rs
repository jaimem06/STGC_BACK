use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use sqlx::{PgPool, Row};
use tracing::error;

use crate::{
    models::{
        DashboardReport,
        MovimientoReport,
        ReportFilter,
        SalesReportFilter,
        StockReport,
        VentaReport,
        VentaPeriodoReport,
        VentaProductoReport,
        VentaEmpleadoReport,
        VentaEmpleadoDetalleReport,
    },
    services::report_service,
};

/// Reporte de stock actual
#[utoipa::path(
    get,
    path = "/reports/stock",
    tag = "Reportes",
    params(
        ReportFilter
    ),
    responses(
        (
            status = 200,
            description = "Reporte de stock generado exitosamente",
            body = [StockReport]
        ),
        (
            status = 401,
            description = "No autorizado - Token inválido o ausente"
        ),
        (
            status = 500,
            description = "Error interno del servidor"
        )
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_stock_report(
    State(pool): State<PgPool>,
    Query(filter): Query<ReportFilter>,
) -> Result<Json<Vec<StockReport>>, (StatusCode, String)> {
    match report_service::get_stock_report(&pool, filter).await {
        Ok(report) => {
            tracing::info!("✅ Reporte de stock generado exitosamente: {} registros", report.len());
            Ok(Json(report))
        },
        Err(e) => {
            error!("❌ Error en get_stock_report: {}", e);
            let error_msg = format!("Error al obtener reporte de stock: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, error_msg))
        }
    }
}

/// Reporte de movimientos
#[utoipa::path(
    get,
    path = "/reports/movements",
    tag = "Reportes",
    params(
        ReportFilter
    ),
    responses(
        (
            status = 200,
            description = "Reporte de movimientos generado exitosamente",
            body = [MovimientoReport]
        ),
        (
            status = 401,
            description = "No autorizado - Token inválido o ausente"
        ),
        (
            status = 500,
            description = "Error interno del servidor"
        )
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_movements_report(
    State(pool): State<PgPool>,
    Query(filter): Query<ReportFilter>,
) -> Result<Json<Vec<MovimientoReport>>, (StatusCode, String)> {
    match report_service::get_movements_report(&pool, filter).await {
        Ok(report) => {
            tracing::info!("✅ Reporte de movimientos generado exitosamente: {} registros", report.len());
            Ok(Json(report))
        },
        Err(e) => {
            error!("❌ Error en get_movements_report: {}", e);
            let error_msg = format!("Error al obtener reporte de movimientos: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, error_msg))
        }
    }
}

/// Reporte de ventas
#[utoipa::path(
    get,
    path = "/reports/sales",
    tag = "Reportes",
    params(
        ReportFilter
    ),
    responses(
        (
            status = 200,
            description = "Reporte de ventas generado exitosamente",
            body = [VentaReport]
        ),
        (
            status = 401,
            description = "No autorizado - Token inválido o ausente"
        ),
        (
            status = 500,
            description = "Error interno del servidor"
        )
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_sales_report(
    State(pool): State<PgPool>,
    Query(filter): Query<ReportFilter>,
) -> Result<Json<Vec<VentaReport>>, (StatusCode, String)> {
    match report_service::get_sales_report(&pool, filter).await {
        Ok(report) => {
            tracing::info!("✅ Reporte de ventas generado exitosamente: {} registros", report.len());
            Ok(Json(report))
        },
        Err(e) => {
            error!("❌ Error en get_sales_report: {}", e);
            let error_msg = format!("Error al obtener reporte de ventas: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, error_msg))
        }
    }
}

/// Dashboard de reportes
#[utoipa::path(
    get,
    path = "/reports/dashboard",
    tag = "Reportes",
    responses(
        (
            status = 200,
            description = "Dashboard generado exitosamente",
            body = DashboardReport
        ),
        (
            status = 401,
            description = "No autorizado - Token inválido o ausente"
        ),
        (
            status = 500,
            description = "Error interno del servidor"
        )
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_dashboard(
    State(pool): State<PgPool>,
) -> Result<Json<DashboardReport>, (StatusCode, String)> {
    match report_service::get_dashboard(&pool).await {
        Ok(report) => {
            tracing::info!("✅ Dashboard generado exitosamente");
            Ok(Json(report))
        },
        Err(e) => {
            error!("❌ Error en get_dashboard: {}", e);
            let error_msg = format!("Error al obtener dashboard: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, error_msg))
        }
    }
}

// ============================================
// NUEVOS ENDPOINTS DE VENTAS
// ============================================

/// Reporte de ventas totales por día
#[utoipa::path(
    get,
    path = "/reports/sales/by-day",
    tag = "Reportes",
    params(
        SalesReportFilter
    ),
    responses(
        (
            status = 200,
            description = "Reporte de ventas por día generado exitosamente",
            body = [VentaPeriodoReport]
        ),
        (
            status = 401,
            description = "No autorizado - Token inválido o ausente"
        ),
        (
            status = 500,
            description = "Error interno del servidor"
        )
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_sales_by_day(
    State(pool): State<PgPool>,
    Query(filter): Query<SalesReportFilter>,
) -> Result<Json<Vec<VentaPeriodoReport>>, (StatusCode, String)> {
    let mut filter_with_period = filter;
    filter_with_period.periodo = Some("dia".to_string());
    
    match report_service::get_sales_by_period(&pool, filter_with_period).await {
        Ok(report) => {
            tracing::info!("✅ Reporte de ventas por día generado exitosamente: {} registros", report.len());
            Ok(Json(report))
        },
        Err(e) => {
            error!("❌ Error en get_sales_by_day: {}", e);
            let error_msg = format!("Error al obtener reporte de ventas por día: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, error_msg))
        }
    }
}

/// Reporte de ventas totales por semana
#[utoipa::path(
    get,
    path = "/reports/sales/by-week",
    tag = "Reportes",
    params(
        SalesReportFilter
    ),
    responses(
        (
            status = 200,
            description = "Reporte de ventas por semana generado exitosamente",
            body = [VentaPeriodoReport]
        ),
        (
            status = 401,
            description = "No autorizado - Token inválido o ausente"
        ),
        (
            status = 500,
            description = "Error interno del servidor"
        )
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_sales_by_week(
    State(pool): State<PgPool>,
    Query(filter): Query<SalesReportFilter>,
) -> Result<Json<Vec<VentaPeriodoReport>>, (StatusCode, String)> {
    let mut filter_with_period = filter;
    filter_with_period.periodo = Some("semana".to_string());
    
    match report_service::get_sales_by_period(&pool, filter_with_period).await {
        Ok(report) => {
            tracing::info!("✅ Reporte de ventas por semana generado exitosamente: {} registros", report.len());
            Ok(Json(report))
        },
        Err(e) => {
            error!("❌ Error en get_sales_by_week: {}", e);
            let error_msg = format!("Error al obtener reporte de ventas por semana: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, error_msg))
        }
    }
}

/// Reporte de ventas totales por mes
#[utoipa::path(
    get,
    path = "/reports/sales/by-month",
    tag = "Reportes",
    params(
        SalesReportFilter
    ),
    responses(
        (
            status = 200,
            description = "Reporte de ventas por mes generado exitosamente",
            body = [VentaPeriodoReport]
        ),
        (
            status = 401,
            description = "No autorizado - Token inválido o ausente"
        ),
        (
            status = 500,
            description = "Error interno del servidor"
        )
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_sales_by_month(
    State(pool): State<PgPool>,
    Query(filter): Query<SalesReportFilter>,
) -> Result<Json<Vec<VentaPeriodoReport>>, (StatusCode, String)> {
    let mut filter_with_period = filter;
    filter_with_period.periodo = Some("mes".to_string());
    
    match report_service::get_sales_by_period(&pool, filter_with_period).await {
        Ok(report) => {
            tracing::info!("✅ Reporte de ventas por mes generado exitosamente: {} registros", report.len());
            Ok(Json(report))
        },
        Err(e) => {
            error!("❌ Error en get_sales_by_month: {}", e);
            let error_msg = format!("Error al obtener reporte de ventas por mes: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, error_msg))
        }
    }
}

/// Reporte de ventas por producto
#[utoipa::path(
    get,
    path = "/reports/sales/by-product",
    tag = "Reportes",
    params(
        SalesReportFilter
    ),
    responses(
        (
            status = 200,
            description = "Reporte de ventas por producto generado exitosamente",
            body = [VentaProductoReport]
        ),
        (
            status = 401,
            description = "No autorizado - Token inválido o ausente"
        ),
        (
            status = 500,
            description = "Error interno del servidor"
        )
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_sales_by_product(
    State(pool): State<PgPool>,
    Query(filter): Query<SalesReportFilter>,
) -> Result<Json<Vec<VentaProductoReport>>, (StatusCode, String)> {
    match report_service::get_sales_by_product(&pool, filter).await {
        Ok(report) => {
            tracing::info!("✅ Reporte de ventas por producto generado exitosamente: {} registros", report.len());
            Ok(Json(report))
        },
        Err(e) => {
            error!("❌ Error en get_sales_by_product: {}", e);
            let error_msg = format!("Error al obtener reporte de ventas por producto: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, error_msg))
        }
    }
}

/// Reporte de ventas por empleado (todos los empleados)
#[utoipa::path(
    get,
    path = "/reports/sales/by-employee",
    tag = "Reportes",
    params(
        SalesReportFilter
    ),
    responses(
        (
            status = 200,
            description = "Reporte de ventas por empleado generado exitosamente",
            body = [VentaEmpleadoReport]
        ),
        (
            status = 401,
            description = "No autorizado - Token inválido o ausente"
        ),
        (
            status = 500,
            description = "Error interno del servidor"
        )
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_sales_by_employee(
    State(pool): State<PgPool>,
    Query(filter): Query<SalesReportFilter>,
) -> Result<Json<Vec<VentaEmpleadoReport>>, (StatusCode, String)> {
    match report_service::get_sales_by_employee(&pool, filter).await {
        Ok(report) => {
            tracing::info!("✅ Reporte de ventas por empleado generado exitosamente: {} registros", report.len());
            Ok(Json(report))
        },
        Err(e) => {
            error!("❌ Error en get_sales_by_employee: {}", e);
            let error_msg = format!("Error al obtener reporte de ventas por empleado: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, error_msg))
        }
    }
}

/// Reporte de ventas de un empleado específico por ID
#[utoipa::path(
    get,
    path = "/reports/sales/by-employee/{empleado_id}",
    tag = "Reportes",
    params(
        ("empleado_id" = String, Path, description = "ID del empleado"),
        SalesReportFilter
    ),
    responses(
        (
            status = 200,
            description = "Reporte de ventas del empleado generado exitosamente",
            body = [VentaEmpleadoDetalleReport]
        ),
        (
            status = 401,
            description = "No autorizado - Token inválido o ausente"
        ),
        (
            status = 404,
            description = "Empleado no encontrado"
        ),
        (
            status = 500,
            description = "Error interno del servidor"
        )
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_sales_by_employee_id(
    State(pool): State<PgPool>,
    Path(empleado_id): Path<String>,
    Query(filter): Query<SalesReportFilter>,
) -> Result<Json<Vec<VentaEmpleadoDetalleReport>>, (StatusCode, String)> {
    match report_service::get_sales_by_employee_id(&pool, empleado_id.clone(), filter).await {
        Ok(report) => {
            if report.is_empty() {
                return Err((StatusCode::NOT_FOUND, format!("No se encontraron ventas para el empleado con ID: {}", empleado_id)));
            }
            tracing::info!("✅ Reporte de ventas del empleado generado exitosamente: {} registros", report.len());
            Ok(Json(report))
        },
        Err(e) => {
            error!("❌ Error en get_sales_by_employee_id: {}", e);
            let error_msg = format!("Error al obtener reporte de ventas del empleado: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, error_msg))
        }
    }
}

/// Obtener lista de semanas disponibles con ventas
#[utoipa::path(
    get,
    path = "/reports/sales/weeks",
    tag = "Reportes",
    responses(
        (
            status = 200,
            description = "Lista de semanas disponibles",
            body = Vec<serde_json::Value>
        ),
        (
            status = 401,
            description = "No autorizado - Token inválido o ausente"
        ),
        (
            status = 500,
            description = "Error interno del servidor"
        )
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_available_weeks(
    State(pool): State<PgPool>,
) -> Result<Json<Vec<serde_json::Value>>, (StatusCode, String)> {
    match report_service::get_available_weeks(&pool).await {
        Ok(weeks) => {
            tracing::info!("✅ Semanas disponibles: {} registros", weeks.len());
            Ok(Json(weeks))
        },
        Err(e) => {
            error!("❌ Error en get_available_weeks: {}", e);
            let error_msg = format!("Error al obtener semanas disponibles: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, error_msg))
        }
    }
}

/// ENDPOINT DE PRUEBA - Test de conexión a DB
#[utoipa::path(
    get,
    path = "/test/db",
    tag = "Pruebas",
    responses(
        (
            status = 200,
            description = "Conexión a base de datos exitosa",
            body = String
        ),
        (
            status = 500,
            description = "Error de conexión"
        )
    )
)]
pub async fn test_db_connection(
    State(pool): State<PgPool>,
) -> Result<String, (StatusCode, String)> {
    match sqlx::query("SELECT 1 as test").fetch_one(&pool).await {
        Ok(row) => {
            let value: i32 = row.get("test");
            Ok(format!("✅ Conexión a DB exitosa! Test: {}", value))
        },
        Err(e) => {
            error!("❌ Error en test_db_connection: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {}", e)))
        }
    }
}