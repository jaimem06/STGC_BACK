use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
    Extension,
};
use uuid::Uuid;
use sqlx::PgPool;
use crate::models::{InventarioItem, MovimientoStock, CreateInventarioItem};
use crate::utils::audit::enviar_auditoria;

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
    State(pool): State<PgPool>,
) -> Result<Json<Vec<InventarioItem>>, StatusCode> {
    let items = sqlx::query_as::<_, InventarioItem>("SELECT * FROM inventario_items")
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            tracing::error!("Error al listar items: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(items))
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
    Path(id): Path<Uuid>,
    State(pool): State<PgPool>,
) -> Result<Json<InventarioItem>, StatusCode> {
    let item = sqlx::query_as::<_, InventarioItem>("SELECT * FROM inventario_items WHERE id = $1")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| {
            tracing::error!("Error al obtener item: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(item))
}

#[utoipa::path(
    post,
    path = "/inventario",
    tag = "Inventario",
    request_body = CreateInventarioItem,
    responses(
        (status = 201, description = "El ítem ha sido creado exitosamente", body = InventarioItem),
        (status = 400, description = "Datos del ítem inválidos o SKU duplicado"),
        (status = 500, description = "Error interno al crear el ítem")
    ),
    summary = "Crear Ítem de Inventario",
    description = "Registra un nuevo elemento en el inventario, ya sea un insumo, un producto terminado o un lote de café inicial."
)]
pub async fn create_item(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<CreateInventarioItem>,
) -> Result<(StatusCode, Json<InventarioItem>), StatusCode> {
    let id = Uuid::new_v4();
    
    let item = sqlx::query_as::<_, InventarioItem>(
        "INSERT INTO inventario_items (id, sku, nombre, cantidad, tipo, estado, unidad_medida, precio, descripcion, fecha_caducidad)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         RETURNING *"
    )
    .bind(id)
    .bind(&payload.sku)
    .bind(&payload.nombre)
    .bind(0.0) // Cantidad inicial siempre 0, se actualiza con movimientos
    .bind(&payload.tipo)
    .bind(&payload.estado)
    .bind(&payload.unidad_medida)
    .bind(payload.precio)
    .bind(&payload.descripcion)
    .bind(payload.fecha_caducidad)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        tracing::error!("Error al crear item: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    enviar_auditoria(
        user_id,
        "CREACION_ITEM_INVENTARIO".to_string(),
        format!("/inventario/{}", item.id),
        "N/A".to_string(),
    );

    Ok((StatusCode::CREATED, Json(item)))
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
    State(pool): State<PgPool>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<MovimientoStock>,
) -> Result<(StatusCode, Json<MovimientoStock>), StatusCode> {
    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!("Error al iniciar transacción: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // 1. Verificar existencia y cantidad del item
    let item = sqlx::query_as::<_, InventarioItem>("SELECT * FROM inventario_items WHERE id = $1 FOR UPDATE")
        .bind(payload.item_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // 2. Calcular nueva cantidad
    let nueva_cantidad = match payload.tipo {
        crate::models::enums::TipoMovimiento::ENTRADA => item.cantidad + payload.cantidad,
        crate::models::enums::TipoMovimiento::SALIDA => {
            if item.cantidad < payload.cantidad {
                return Err(StatusCode::BAD_REQUEST);
            }
            item.cantidad - payload.cantidad
        }
    };

    // 3. Actualizar inventario
    sqlx::query("UPDATE inventario_items SET cantidad = $1 WHERE id = $2")
        .bind(nueva_cantidad)
        .bind(payload.item_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // 4. Registrar movimiento
    let id = Uuid::new_v4();
    let movimiento = sqlx::query_as::<_, MovimientoStock>(
        "INSERT INTO movimientos_stock (id, item_id, cantidad, tipo, fecha, motivo, lote_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING *"
    )
    .bind(id)
    .bind(payload.item_id)
    .bind(payload.cantidad)
    .bind(&payload.tipo)
    .bind(payload.fecha)
    .bind(&payload.motivo)
    .bind(payload.lote_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    enviar_auditoria(
        user_id,
        format!("MOVIMIENTO_STOCK_{:?}", movimiento.tipo),
        format!("/inventario/movimientos/{}", movimiento.id),
        "N/A".to_string(),
    );

    Ok((StatusCode::CREATED, Json(movimiento)))
}
