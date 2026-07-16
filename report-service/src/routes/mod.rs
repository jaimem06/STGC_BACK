use axum::{
    middleware,
    routing::get,
    Router,
    response::Html,
};
use sqlx::PgPool;
use utoipa::OpenApi;

use crate::{
    handlers::report_handler,
    middleware::auth::auth_middleware,
    docs::ApiDoc,
};

pub fn create_router(pool: PgPool) -> Router {
    let openapi = ApiDoc::openapi();

    // HTML para ReDoc (usando CDN)
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
                body {{
                    margin: 0;
                    padding: 0;
                }}
                redoc {{
                    display: block;
                    height: 100vh;
                }}
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
        // Reportes de ventas por período
        .route("/reports/sales/by-day", get(report_handler::get_sales_by_day))
        .route("/reports/sales/by-week", get(report_handler::get_sales_by_week))
        .route("/reports/sales/by-month", get(report_handler::get_sales_by_month))
        // Reportes de ventas por producto y empleado
        .route("/reports/sales/by-product", get(report_handler::get_sales_by_product))
        .route("/reports/sales/by-employee", get(report_handler::get_sales_by_employee))
        // Reporte de ventas por empleado específico (con path param)
        .route("/reports/sales/by-employee/:empleado_id", get(report_handler::get_sales_by_employee_id))
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
        .route("/docs", get(|| async { 
            Html(redoc_html) 
        }))
        .route("/openapi.json", get(|| async { 
            axum::response::Json(openapi) 
        }));

    // ============================================
    // COMBINAR TODAS LAS RUTAS
    // ============================================
    Router::new()
        .merge(report_routes)
        .merge(test_routes)
        .merge(docs_routes)
        .with_state(pool)
}