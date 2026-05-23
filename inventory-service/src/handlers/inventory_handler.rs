use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;
use sqlx::PgPool;
use crate::models::{InventarioItem, MovimientoStock};

#[utoipa::path(
    get,
    path = "/inventario",
    tag = "Inventario",
    responses(
        (status = 200, description = "Lista todos los ítems del inventario (insumos, productos y café)", body = [InventarioItem]),
        (status = 500, description = "Error interno al consultar la base de datos")
    ),
    summary = "Listar Inventario",
    description = "Obtiene una lista completa de todos los elementos registrados en el inventario, incluyendo insumos agrícolas, productos terminados y café en sus distintas fases de proceso."
)]
pub async fn list_items(
    State(_pool): State<PgPool>,
) -> Result<Json<Vec<InventarioItem>>, StatusCode> {
    Ok(Json(vec![]))
}

#[utoipa::path(
    get,
    path = "/inventario/{id}",
    tag = "Inventario",
    params(
        ("id" = Uuid, Path, description = "ID único del ítem en el inventario")
    ),
    responses(
        (status = 200, description = "Detalles detallados del ítem solicitado", body = InventarioItem),
        (status = 400, description = "Formato de ID inválido"),
        (status = 404, description = "El ítem no existe en el sistema"),
        (status = 500, description = "Error interno del servidor")
    ),
    summary = "Obtener Detalle de Ítem",
    description = "Consulta la información técnica, cantidad disponible, estado actual y fechas relevantes de un ítem específico mediante su identificador único."
)]
pub async fn get_item(
    Path(_id): Path<Uuid>,
    State(_pool): State<PgPool>,
) -> Result<Json<InventarioItem>, StatusCode> {
    Err(StatusCode::NOT_FOUND)
}

#[utoipa::path(
    post,
    path = "/inventario/movimientos",
    tag = "Inventario",
    request_body = MovimientoStock,
    responses(
        (status = 201, description = "El movimiento de stock ha sido registrado correctamente", body = MovimientoStock),
        (status = 400, description = "Datos del movimiento inválidos, cantidad insuficiente o inconsistencias en el tipo de movimiento"),
        (status = 404, description = "El ítem o lote referenciado no existe"),
        (status = 500, description = "Error interno al procesar la transacción")
    ),
    summary = "Registrar Movimiento de Stock",
    description = "Registra una entrada o salida de inventario. Este endpoint es fundamental para actualizar las existencias físicas y documentar el motivo de cada cambio (compra, venta, merma, ajuste)."
)]
pub async fn create_movement(
    State(_pool): State<PgPool>,
    Json(_payload): Json<MovimientoStock>,
) -> Result<(StatusCode, Json<MovimientoStock>), StatusCode> {
    Ok((StatusCode::CREATED, Json(_payload)))
}
