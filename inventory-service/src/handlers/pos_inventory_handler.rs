use axum::{
    extract::{Path, State, Query},
    http::{StatusCode, header},
    response::IntoResponse,
    Json,
    Extension,
};
use uuid::Uuid;
use sqlx::PgPool;
use serde::Deserialize;
use chrono::{DateTime, Utc, TimeZone};
use crate::models::{
    InventarioItem, MovimientoStock, CreateInventarioItem, UpdateInventarioItem, 
    UpdateEstadoDto, enums::EstadoInventario
};
use crate::utils::audit::enviar_auditoria;

#[derive(Deserialize, utoipa::IntoParams)]
pub struct MovementFilter {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

fn parse_flex_date(date_str: Option<String>) -> Option<DateTime<Utc>> {
    let s = date_str?.trim().to_string();
    if s.is_empty() || s == "null" { return None; }
    if let Ok(dt) = DateTime::parse_from_rfc3339(&s) { return Some(dt.with_timezone(&Utc)); }
    if let Ok(naive_date) = chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
        return naive_date.and_hms_opt(0, 0, 0).map(|dt| Utc.from_utc_datetime(&dt));
    }
    None
}

#[utoipa::path(
    get,
    path = "/inventario/pos",
    tag = "Inventario POS",
    responses((status = 200, body = [InventarioItem]))
)]
pub async fn list_items(State(pool): State<PgPool>) -> Result<Json<Vec<InventarioItem>>, StatusCode> {
    let items = sqlx::query_as::<_, InventarioItem>(
        "SELECT * FROM inventario_items WHERE modulo = 'CAFETERIA' AND is_deleted = false ORDER BY created_at DESC"
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        tracing::error!("Error SQL list_items (POS): {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(items))
}

#[utoipa::path(
    post,
    path = "/inventario/pos/nuevo",
    tag = "Inventario POS",
    request_body = CreateInventarioItem,
    responses((status = 201, body = InventarioItem))
)]
pub async fn create_item(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<CreateInventarioItem>,
) -> Result<(StatusCode, Json<InventarioItem>), StatusCode> {
    let id = Uuid::new_v4();
    let fecha = parse_flex_date(payload.fecha_caducidad);

    let item = sqlx::query_as::<_, InventarioItem>(
        "INSERT INTO inventario_items (id, sku, nombre, cantidad, tipo, estado, unidad_medida, precio, descripcion, fecha_caducidad, modulo, stock_minimo)
         VALUES ($1, $2, $3, 0, $4, $5, $6, $7, $8, $9, 'CAFETERIA', $10)
         RETURNING *"
    )
    .bind(id).bind(&payload.sku).bind(&payload.nombre).bind(&payload.tipo).bind(&payload.estado).bind(&payload.unidad_medida).bind(payload.precio).bind(&payload.descripcion).bind(fecha).bind(payload.stock_minimo.unwrap_or(0.0))
    .fetch_one(&pool).await
    .map_err(|e| {
        tracing::error!("Error al crear item (POS): {:?}", e);
        if let Some(db_err) = e.as_database_error() {
            if db_err.code() == Some(std::borrow::Cow::Borrowed("23505")) {
                return StatusCode::CONFLICT;
            }
        }
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    enviar_auditoria(user_id, "POS_CREATE_ITEM".to_string(), format!("/inventario/pos/{}", item.id), "N/A".to_string());
    Ok((StatusCode::CREATED, Json(item)))
}

#[utoipa::path(
    get,
    path = "/inventario/pos/{id}",
    tag = "Inventario POS",
    params(("id" = Uuid, Path)),
    responses((status = 200, body = InventarioItem))
)]
pub async fn get_item(Path(id): Path<Uuid>, State(pool): State<PgPool>) -> Result<Json<InventarioItem>, StatusCode> {
    let item = sqlx::query_as::<_, InventarioItem>("SELECT * FROM inventario_items WHERE id = $1 AND modulo = 'CAFETERIA' AND is_deleted = false")
        .bind(id).fetch_optional(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(item))
}

#[utoipa::path(
    put,
    path = "/inventario/pos/{id}",
    tag = "Inventario POS",
    params(("id" = Uuid, Path)),
    request_body = UpdateInventarioItem,
    responses((status = 200, body = InventarioItem))
)]
pub async fn update_item(
    Path(id): Path<Uuid>,
    State(pool): State<PgPool>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<UpdateInventarioItem>,
) -> Result<Json<InventarioItem>, StatusCode> {
    let fecha = parse_flex_date(payload.fecha_caducidad);

    let item = sqlx::query_as::<_, InventarioItem>(
        "UPDATE inventario_items SET 
            nombre = COALESCE($1, nombre), 
            precio = COALESCE($2, precio), 
            descripcion = COALESCE($3, descripcion), 
            stock_minimo = COALESCE($4, stock_minimo), 
            unidad_medida = COALESCE($5, unidad_medida),
            fecha_caducidad = COALESCE($6, fecha_caducidad),
            updated_at = NOW()
         WHERE id = $7 AND modulo = 'CAFETERIA' AND is_deleted = false RETURNING *"
    )
    .bind(payload.nombre).bind(payload.precio).bind(payload.descripcion).bind(payload.stock_minimo).bind(payload.unidad_medida).bind(fecha).bind(id)
    .fetch_optional(&pool).await.map_err(|e| {
        tracing::error!("Error SQL update_item (POS): {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?.ok_or(StatusCode::NOT_FOUND)?;

    enviar_auditoria(user_id, "POS_UPDATE_ITEM".to_string(), format!("/inventario/pos/{}", id), "N/A".to_string());
    Ok(Json(item))
}

#[utoipa::path(
    patch,
    path = "/inventario/pos/{id}/estado",
    tag = "Inventario POS",
    params(("id" = Uuid, Path)),
    request_body = UpdateEstadoDto,
    responses((status = 200, body = InventarioItem))
)]
pub async fn update_status(
    Path(id): Path<Uuid>,
    State(pool): State<PgPool>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<UpdateEstadoDto>,
) -> Result<Json<InventarioItem>, StatusCode> {
    let item = sqlx::query_as::<_, InventarioItem>(
        "UPDATE inventario_items SET estado = $1, updated_at = NOW() 
         WHERE id = $2 AND modulo = 'CAFETERIA' AND is_deleted = false RETURNING *"
    )
    .bind(&payload.estado).bind(id)
    .fetch_optional(&pool).await.map_err(|e| {
        tracing::error!("Error SQL update_status (POS): {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?.ok_or(StatusCode::NOT_FOUND)?;

    enviar_auditoria(user_id, "POS_UPDATE_STATUS".to_string(), format!("/inventario/pos/{}/estado", id), "N/A".to_string());
    Ok(Json(item))
}

#[utoipa::path(
    delete,
    path = "/inventario/pos/{id}",
    tag = "Inventario POS",
    params(("id" = Uuid, Path)),
    responses((status = 204))
)]
pub async fn delete_item(Path(id): Path<Uuid>, State(pool): State<PgPool>) -> Result<StatusCode, StatusCode> {
    sqlx::query("UPDATE inventario_items SET is_deleted = true WHERE id = $1 AND modulo = 'CAFETERIA' AND is_deleted = false")
        .bind(id).execute(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/inventario/pos/movimientos",
    tag = "Inventario POS",
    request_body = MovimientoStock,
    responses((status = 201, body = MovimientoStock))
)]
pub async fn create_movement(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<MovimientoStock>,
) -> Result<(StatusCode, Json<MovimientoStock>), StatusCode> {
    let mut tx = pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let item = sqlx::query_as::<_, InventarioItem>("SELECT * FROM inventario_items WHERE id = $1 AND modulo = 'CAFETERIA' FOR UPDATE")
        .bind(payload.item_id).fetch_one(&mut *tx).await.map_err(|_| StatusCode::NOT_FOUND)?;
    
    let nueva_cantidad = match payload.tipo {
        crate::models::enums::TipoMovimiento::ENTRADA => item.cantidad + payload.cantidad,
        crate::models::enums::TipoMovimiento::SALIDA => {
            if item.cantidad < payload.cantidad { return Err(StatusCode::BAD_REQUEST); }
            item.cantidad - payload.cantidad
        }
    };
    
    let nuevo_estado = if nueva_cantidad <= 0.0 { EstadoInventario::AGOTADO } else if nueva_cantidad <= item.stock_minimo { EstadoInventario::STOCK_BAJO } else { EstadoInventario::DISPONIBLE };
    
    sqlx::query("UPDATE inventario_items SET cantidad = $1, estado = $2 WHERE id = $3")
        .bind(nueva_cantidad).bind(nuevo_estado).bind(item.id).execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let movimiento = sqlx::query_as::<_, MovimientoStock>("INSERT INTO movimientos_stock (id, item_id, cantidad, tipo, fecha, motivo) VALUES ($1, $2, $3, $4, NOW(), $5) RETURNING *")
        .bind(Uuid::new_v4()).bind(item.id).bind(payload.cantidad).bind(&payload.tipo).bind(&payload.motivo).fetch_one(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    enviar_auditoria(user_id, "POS_MOVEMENT".to_string(), format!("/inventario/pos/movimientos/{}", movimiento.id), "N/A".to_string());
    Ok((StatusCode::CREATED, Json(movimiento)))
}

#[utoipa::path(
    get,
    path = "/inventario/pos/{id}/movimientos",
    tag = "Inventario POS",
    params(("id" = Uuid, Path), MovementFilter),
    responses((status = 200, body = [MovimientoStock]))
)]
pub async fn list_movements(
    Path(id): Path<Uuid>,
    State(pool): State<PgPool>,
    Query(filter): Query<MovementFilter>,
) -> Result<Json<Vec<MovimientoStock>>, StatusCode> {
    let start = parse_flex_date(filter.start_date);
    let end = parse_flex_date(filter.end_date);
    let movements = sqlx::query_as::<_, MovimientoStock>(
        "SELECT m.* FROM movimientos_stock m JOIN inventario_items i ON m.item_id = i.id 
         WHERE i.id = $1 AND i.modulo = 'CAFETERIA' AND (m.fecha >= $2 OR $2 IS NULL) AND (m.fecha <= $3 OR $3 IS NULL) ORDER BY m.fecha DESC"
    )
    .bind(id).bind(start).bind(end)
    .fetch_all(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(movements))
}

#[utoipa::path(
    get,
    path = "/inventario/pos/movimientos/exportar",
    tag = "Inventario POS",
    params(MovementFilter),
    responses((status = 200, description = "CSV"))
)]
pub async fn export_all_movements_csv(
    State(pool): State<PgPool>,
    Query(filter): Query<MovementFilter>,
) -> impl IntoResponse {
    let start = parse_flex_date(filter.start_date);
    let end = parse_flex_date(filter.end_date);
    let movements = sqlx::query_as::<_, MovimientoStock>(
        "SELECT m.* FROM movimientos_stock m JOIN inventario_items i ON m.item_id = i.id 
         WHERE i.modulo = 'CAFETERIA' AND (m.fecha >= $1 OR $1 IS NULL) AND (m.fecha <= $2 OR $2 IS NULL) ORDER BY m.fecha DESC"
    )
    .bind(start).bind(end)
    .fetch_all(&pool).await.unwrap_or_default();

    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record(&["ID", "ItemID", "Cantidad", "Tipo", "Fecha", "Motivo"]).unwrap();
    for m in movements {
        wtr.write_record(&[m.id.to_string(), m.item_id.to_string(), m.cantidad.to_string(), format!("{:?}", m.tipo), m.fecha.to_string(), m.motivo]).unwrap();
    }
    let csv_data = wtr.into_inner().unwrap();
    (StatusCode::OK, [(header::CONTENT_TYPE, "text/csv"), (header::CONTENT_DISPOSITION, "attachment; filename=\"movimientos_pos.csv\"")], csv_data)
}
