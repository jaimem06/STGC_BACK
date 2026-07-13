use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use sqlx::PgPool;

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
) -> Result<Json<Vec<StockReport>>, StatusCode> {
    let report = report_service::get_stock_report(&pool, filter)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(report))
}

/// HU027 - Reporte de movimientos
pub async fn get_movements_report(
    State(pool): State<PgPool>,
    Query(filter): Query<ReportFilter>,
) -> Result<Json<Vec<MovimientoReport>>, StatusCode> {
    let report = report_service::get_movements_report(&pool, filter)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(report))
}

/// HU017 - Reporte de ventas
pub async fn get_sales_report(
    State(pool): State<PgPool>,
    Query(filter): Query<ReportFilter>,
) -> Result<Json<Vec<VentaReport>>, StatusCode> {
    let report = report_service::get_sales_report(&pool, filter)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(report))
}

/// Dashboard de reportes
pub async fn get_dashboard(
    State(pool): State<PgPool>,
) -> Result<Json<DashboardReport>, StatusCode> {
    let report = report_service::get_dashboard(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(report))
}