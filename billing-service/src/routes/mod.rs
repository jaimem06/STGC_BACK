use axum::{
    routing::{get, post},
    Router,
};
use sqlx::PgPool;
use utoipa::OpenApi;
use utoipa_redoc::{Redoc, Servable};
use tower_http::trace::TraceLayer;

use crate::handlers::facturas_handler;
use crate::handlers::comprobantes_handler;
use crate::models::billing::BusinessInfo;
use crate::middleware::auth::auth_middleware;
use axum::middleware;

#[derive(OpenApi)]
#[openapi(
    paths(
        facturas_handler::registrar_movimiento_factura,
        comprobantes_handler::listar_estados_factura,
        comprobantes_handler::emitir_comprobante,
        comprobantes_handler::descargar_comprobante_pdf
    ),
    components(
        schemas(
            crate::models::billing::CreateEntradaFacturaDto,
            crate::models::billing::InvoiceStatus,
            crate::models::billing::InvoiceStatusDefinition,
            crate::models::billing::InvoiceStatusCatalogResponse,
            crate::models::billing::EmitReceiptResponse,
            crate::models::billing::BillingErrorResponse
        )
    ),
    tags(
        (name = "Billing Service", description = "Servicio oficial para el manejo de facturación y movimientos de stock (Entradas y Salidas)."),
        (name = "Emisión de comprobantes", description = "Emisión y descarga de facturas PDF para pedidos pagados.")
    ),
    info(
        title = "Billing Service API",
        description = "Documentación técnica y detallada del servicio de facturación (Billing Service) para STGC. Este servicio centraliza el registro de movimientos de inventario que están atados a documentos de respaldo como facturas. Su lógica de negocio incluye validación en tiempo real de disponibilidad de stock, conversiones matemáticas precisas de unidades de medida (p. ej. Libras a Quintales) y el control histórico de las transacciones.",
        version = "1.0.0",
        contact(name = "Equipo de Desarrollo STGC")
    )
)]
struct ApiDoc;

pub fn create_router(pool: PgPool, business_info: BusinessInfo) -> Router {
    let api_routes = Router::new()
        .route("/facturas/movimiento", post(facturas_handler::registrar_movimiento_factura))
        .route("/facturas/estados", get(comprobantes_handler::listar_estados_factura))
        .route("/comprobantes/:pedido_id/emitir", post(comprobantes_handler::emitir_comprobante))
        .route("/comprobantes/:pedido_id/pdf", get(comprobantes_handler::descargar_comprobante_pdf))
        .route_layer(middleware::from_fn(auth_middleware))
        .layer(axum::Extension(business_info))
        .with_state(pool);

    Router::new()
        .merge(Redoc::with_url("/docs", ApiDoc::openapi()))
        .nest("/api/billing", api_routes)
        .layer(TraceLayer::new_for_http())
}
