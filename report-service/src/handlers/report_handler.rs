use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use sqlx::{PgPool, Row};  // <--- Agregar Row aquí
use tracing::error;

use crate::{
    models::{
        DashboardReport,
        MovimientoReport,
        ReportFilter,
        StockReport,
        VentaReport,
    },
    services::report_service,
};

/// HU026 - Reporte de stock actual
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

/// HU027 - Reporte de movimientos
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

/// HU017 - Reporte de ventas
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

/// ENDPOINT DE PRUEBA - Test de conexión a DB
pub async fn test_db_connection(
    State(pool): State<PgPool>,
) -> Result<String, (StatusCode, String)> {
    match sqlx::query("SELECT 1 as test").fetch_one(&pool).await {
        Ok(row) => {
            let value: i32 = row.get("test");  // <--- Ahora funciona con Row importado
            Ok(format!("✅ Conexión a DB exitosa! Test: {}", value))
        },
        Err(e) => {
            error!("❌ Error en test_db_connection: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {}", e)))
        }
    }
}