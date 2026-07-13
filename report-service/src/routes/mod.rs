use axum::{
    middleware,
    routing::get,
    Router,
};
use sqlx::PgPool;

use crate::{
    handlers::report_handler,
    middleware::auth::auth_middleware,
};

pub fn create_router(pool: PgPool) -> Router {
    Router::new()
        // Endpoints normales con autenticación
        .route("/reports/stock", get(report_handler::get_stock_report))
        .route("/reports/movements", get(report_handler::get_movements_report))
        .route("/reports/sales", get(report_handler::get_sales_report))
        .route("/reports/dashboard", get(report_handler::get_dashboard))
        .layer(middleware::from_fn(auth_middleware))
        // Endpoint de prueba SIN autenticación (solo para diagnóstico)
        .route("/test/db", get(report_handler::test_db_connection))
        .with_state(pool)
}