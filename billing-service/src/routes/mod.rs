use axum::{
    routing::post,
    Router,
};
use sqlx::PgPool;
use utoipa::OpenApi;
use utoipa_redoc::{Redoc, Servable};
use tower_http::trace::TraceLayer;

use crate::handlers::facturas_handler;
use crate::middleware::auth::auth_middleware;
use axum::middleware;

#[derive(OpenApi)]
#[openapi(
    paths(
        facturas_handler::registrar_movimiento_factura
    ),
    components(
        schemas(crate::models::billing::CreateEntradaFacturaDto)
    ),
    tags(
        (name = "Facturación", description = "API de facturación y asociación con inventario")
    )
)]
struct ApiDoc;

pub fn create_router(pool: PgPool) -> Router {
    let api_routes = Router::new()
        .route("/facturas/movimiento", post(facturas_handler::registrar_movimiento_factura))
        .route_layer(middleware::from_fn(auth_middleware))
        .with_state(pool);

    Router::new()
        .merge(Redoc::with_url("/docs", ApiDoc::openapi()))
        .nest("/api/billing", api_routes)
        .layer(TraceLayer::new_for_http())
}
