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
        (name = "Billing Service", description = "Servicio oficial para el manejo de facturación y movimientos de stock (Entradas y Salidas).")
    ),
    info(
        title = "Billing Service API",
        description = "Documentación técnica y detallada del servicio de facturación (Billing Service) para STGC. Este servicio centraliza el registro de movimientos de inventario que están atados a documentos de respaldo como facturas. Su lógica de negocio incluye validación en tiempo real de disponibilidad de stock, conversiones matemáticas precisas de unidades de medida (p. ej. Libras a Quintales) y el control histórico de las transacciones.",
        version = "1.0.0",
        contact(name = "Equipo de Desarrollo STGC")
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
