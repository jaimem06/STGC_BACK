use axum::{
    routing::{get, post, patch},
    Router,
};
use sqlx::PgPool;
use utoipa::OpenApi;
use utoipa_redoc::{Redoc, Servable};

use crate::handlers::{pos_inventory_handler, finca_inventory_handler, traceability_handler};
use crate::models::*;

#[derive(OpenApi)]
#[openapi(
    paths(
        pos_inventory_handler::list_items,
        pos_inventory_handler::get_item,
        pos_inventory_handler::create_item,
        pos_inventory_handler::update_item,
        pos_inventory_handler::update_status,
        pos_inventory_handler::delete_item,
        pos_inventory_handler::create_movement,
        pos_inventory_handler::list_movements,
        pos_inventory_handler::export_all_movements_csv,
        finca_inventory_handler::list_items,
        finca_inventory_handler::get_item,
        finca_inventory_handler::create_item,
        finca_inventory_handler::update_item,
        finca_inventory_handler::update_status,
        finca_inventory_handler::delete_item,
        finca_inventory_handler::create_movement,
        finca_inventory_handler::list_movements,
        finca_inventory_handler::export_all_movements_csv,
        traceability_handler::list_lots,
        traceability_handler::transition_lot_phase,
        traceability_handler::get_traceability_history,
    ),
    components(
        schemas(
            InventarioItem, CreateInventarioItem, UpdateInventarioItem, UpdateEstadoDto,
            MovimientoStock, Proveedor, LoteCafe,
            EstadoInventario, TipoElemento, UnidadMedida, CalidadCafe,
            ClasificacionInsumo, FaseCafe, TipoMovimiento, ModuloInventario
        )
    ),
    tags(
        (name = "Inventario Modulo 2", description = "Operaciones de inventario específicas para la Cafetería y Puntos de Venta (Insumos perecederos, productos finales)."),
        (name = "Inventario Finca", description = "Gestión de insumos agrícolas y producción primaria de café."),
        (name = "Trazabilidad", description = "Seguimiento de la cadena de suministro desde la cosecha hasta el procesamiento.")
    ),
    info(
        title = "Sistema de Gestión y Trazabilidad de Café (STGC) - API de Inventario",
        version = "2.0.0",
        description = "Documentación técnica de los endpoints de inventario y trazabilidad. Desarrollado en Rust (Axum, SQLx)."
    )
)]
pub struct ApiDoc;

pub fn create_router(pool: PgPool) -> Router {
    let api_routes = Router::new()
        // --- Inventario Modulo 2 ---
        .route("/inventario/pos", get(pos_inventory_handler::list_items))
        .route("/inventario/pos/nuevo", post(pos_inventory_handler::create_item))
        .route("/inventario/pos/movimientos", post(pos_inventory_handler::create_movement))
        .route("/inventario/pos/movimientos/exportar", get(pos_inventory_handler::export_all_movements_csv))
        .route("/inventario/pos/:id", get(pos_inventory_handler::get_item).put(pos_inventory_handler::update_item).delete(pos_inventory_handler::delete_item))
        .route("/inventario/pos/:id/estado", patch(pos_inventory_handler::update_status))
        .route("/inventario/pos/:id/movimientos", get(pos_inventory_handler::list_movements))

        // --- INVENTARIO FINCA ---
        .route("/inventario/finca", get(finca_inventory_handler::list_items))
        .route("/inventario/finca/nuevo", post(finca_inventory_handler::create_item))
        .route("/inventario/finca/movimientos", post(finca_inventory_handler::create_movement))
        .route("/inventario/finca/movimientos/exportar", get(finca_inventory_handler::export_all_movements_csv))
        .route("/inventario/finca/:id", get(finca_inventory_handler::get_item).put(finca_inventory_handler::update_item).delete(finca_inventory_handler::delete_item))
        .route("/inventario/finca/:id/estado", patch(finca_inventory_handler::update_status))
        .route("/inventario/finca/:id/movimientos", get(finca_inventory_handler::list_movements))

        // --- TRAZABILIDAD ---
        .route("/trazabilidad/lotes", get(traceability_handler::list_lots))
        .route("/trazabilidad/lotes/:id/transicion", post(traceability_handler::transition_lot_phase))
        .route("/trazabilidad/historial/:codigo_trazabilidad", get(traceability_handler::get_traceability_history))
        
        .layer(axum::middleware::from_fn(crate::middleware::auth::auth_middleware));

    Router::new()
        .merge(api_routes)
        .merge(Redoc::with_url("/docs", ApiDoc::openapi()))
        .with_state(pool)
}
