use axum::{
    middleware,
    routing::{get, post},
    Router,
    response::Html,
};
use sqlx::PgPool;
use utoipa::OpenApi;

use crate::{
    handlers::{report_handler, export_handler},
    middleware::auth::auth_middleware,
    docs::ApiDoc,
};

pub fn create_router(pool: PgPool) -> Router {
    let openapi = ApiDoc::openapi();

    let redoc_html = format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>STGC Report Service API</title>
            <meta charset="utf-8"/>
            <meta name="viewport" content="width=device-width, initial-scale=1">
            <link href="https://fonts.googleapis.com/css?family=Montserrat:300,400,700|Roboto:300,400,700" rel="stylesheet">
            <style>
                body {{ margin: 0; padding: 0; }}
                redoc {{ display: block; height: 100vh; }}
            </style>
        </head>
        <body>
            <redoc spec-url='/openapi.json'></redoc>
            <script src="https://cdn.redoc.ly/redoc/latest/bundles/redoc.standalone.js"> </script>
        </body>
        </html>
        "#
    );

    // ============================================
    // ENDPOINTS DE REPORTES (con autenticación)
    // ============================================
    let report_routes = Router::new()
        // Reportes principales
        .route("/reports/stock", get(report_handler::get_stock_report))
        .route("/reports/movements", get(report_handler::get_movements_report))
        .route("/reports/sales", get(report_handler::get_sales_report))
        .route("/reports/dashboard", get(report_handler::get_dashboard))
        // Ventas por período
        .route("/reports/sales/by-day", get(report_handler::get_sales_by_day))
        .route("/reports/sales/by-week", get(report_handler::get_sales_by_week))
        .route("/reports/sales/by-month", get(report_handler::get_sales_by_month))
        // Ventas por producto y empleado
        .route("/reports/sales/by-product", get(report_handler::get_sales_by_product))
        .route("/reports/sales/by-employee", get(report_handler::get_sales_by_employee))
        .route("/reports/sales/by-employee/:empleado_id", get(report_handler::get_sales_by_employee_id))
        // Semanas disponibles
        .route("/reports/sales/weeks", get(report_handler::get_available_weeks))
        // Exportaciones desde DB (para compatibilidad)
        .route("/reports/export/stock/csv", get(export_handler::export_stock_csv))
        .route("/reports/export/stock/pdf", get(export_handler::export_stock_pdf))
        .route("/reports/export/sales/csv", get(export_handler::export_sales_csv))
        .route("/reports/export/sales/pdf", get(export_handler::export_sales_pdf))
        .route("/reports/export/movements/csv", get(export_handler::export_movements_csv))
        .route("/reports/export/movements/pdf", get(export_handler::export_movements_pdf))
        .route("/reports/export/sales-by-product/csv", get(export_handler::export_sales_by_product_csv))
        .route("/reports/export/sales-by-product/pdf", get(export_handler::export_sales_by_product_pdf))
        .route("/reports/export/sales-by-employee/csv", get(export_handler::export_sales_by_employee_csv))
        .route("/reports/export/sales-by-employee/pdf", get(export_handler::export_sales_by_employee_pdf))
        // NUEVOS: Exportación desde datos del frontend
        .route("/reports/export/pdf/from-data", post(export_handler::export_pdf_from_data))
        .route("/reports/export/csv/from-data", post(export_handler::export_csv_from_data))
        .layer(middleware::from_fn(auth_middleware));

    // ============================================
    // ENDPOINTS DE PRUEBA (sin autenticación)
    // ============================================
    let test_routes = Router::new()
        .route("/test/db", get(report_handler::test_db_connection));

    // ============================================
    // DOCUMENTACIÓN
    // ============================================
    let docs_routes = Router::new()
        .route("/docs", get(|| async { Html(redoc_html) }))
        .route("/openapi.json", get(|| async { axum::response::Json(openapi) }));

    Router::new()
        .merge(report_routes)
        .merge(test_routes)
        .merge(docs_routes)
        .with_state(pool)
}