use axum::{
    routing::{get, post},
    Router,
};
use sqlx::PgPool;
use utoipa::OpenApi;
use utoipa_redoc::{Redoc, Servable};

use crate::handlers::{inventory_handler, traceability_handler};
use crate::models::*;

#[derive(OpenApi)]
#[openapi(
    paths(
        inventory_handler::list_items,
        inventory_handler::get_item,
        inventory_handler::create_movement,
        traceability_handler::list_lots,
        traceability_handler::transition_lot_phase,
        traceability_handler::get_traceability_history,
    ),
    components(
        schemas(
            InventarioItem, MovimientoStock, Proveedor, LoteCafe,
            EstadoProducto, TipoElemento, UnidadMedida, CalidadCafe,
            ClasificacionInsumo, FaseCafe, TipoMovimiento
        )
    ),
    tags(
        (name = "Inventario", description = "Operaciones relacionadas con el control de stock físico e insumos."),
        (name = "Trazabilidad", description = "Seguimiento detallado de la cadena de valor del café por lotes.")
    ),
    info(
        title = "inventory-service",
        version = "1.0.0",
        description = "Este microservicio gestiona el inventario unificado del Sistema de Trazabilidad y Gestión de Cafetería (STGC). 
                       Permite el seguimiento desde la cosecha y fases de procesamiento del café hasta la venta final, integrando 
                       los módulos de Trazabilidad de Finca y Gestión de Ventas/POS."
    )
)]
pub struct ApiDoc;

pub fn create_router(pool: PgPool) -> Router {
    let inventory_routes = Router::new()
        .route("/", get(inventory_handler::list_items))
        .route("/{id}", get(inventory_handler::get_item))
        .route("/movimientos", post(inventory_handler::create_movement));

    let traceability_routes = Router::new()
        .route("/lotes", get(traceability_handler::list_lots))
        .route("/lotes/{id}/transicion", post(traceability_handler::transition_lot_phase))
        .route("/historial/{codigo_trazabilidad}", get(traceability_handler::get_traceability_history));

    Router::new()
        .nest("/inventario", inventory_routes)
        .nest("/trazabilidad", traceability_routes)
        .merge(Redoc::with_url("/docs", ApiDoc::openapi()))
        .layer(axum::middleware::from_fn(crate::middleware::auth::auth_middleware))
        .with_state(pool)
}
