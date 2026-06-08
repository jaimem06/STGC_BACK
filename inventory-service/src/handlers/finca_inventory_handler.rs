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
    path = "/inventario/finca",
    tag = "Inventario Finca",
    responses(
        (status = 200, description = "Catálogo de insumos y productos de producción recuperado", body = [InventarioItem]),
        (status = 401, description = "No autorizado: Requiere token Bearer válido"),
        (status = 403, description = "Prohibido: Rol insuficiente para acceder a datos de finca"),
        (status = 500, description = "Error interno al consultar la base de datos"),
        (status = 503, description = "Servicio de base de datos no disponible")
    ),
    summary = "Listar ítems de producción (Finca)",
    description = "Obtiene todos los elementos del inventario pertenecientes al módulo de Finca. Incluye insumos agrícolas y café en sus diferentes fases."
)]
pub async fn list_items(State(pool): State<PgPool>) -> Result<Json<Vec<InventarioItem>>, StatusCode> {
    let items = sqlx::query_as::<_, InventarioItem>(
        "SELECT * FROM inventario_items WHERE modulo = 'FINCA' AND is_deleted = false ORDER BY created_at DESC"
    )
    .fetch_all(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(items))
}

#[utoipa::path(
    post,
    path = "/inventario/finca/nuevo",
    tag = "Inventario Finca",
    request_body = CreateInventarioItem,
    responses(
        (status = 201, description = "Ítem de finca creado exitosamente", body = InventarioItem),
        (status = 400, description = "Datos de entrada mal formados"),
        (status = 401, description = "Autenticación requerida"),
        (status = 409, description = "Error: El código SKU ya existe en el sistema"),
        (status = 422, description = "Faltan campos obligatorios o formato inválido")
    ),
    summary = "Registrar nuevo ítem de finca",
    description = "Crea un nuevo registro en el inventario forzando el módulo a 'FINCA'. Permite asociar códigos de trazabilidad y fases de procesamiento."
)]
pub async fn create_item(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<CreateInventarioItem>,
) -> Result<(StatusCode, Json<InventarioItem>), StatusCode> {
    let id = Uuid::new_v4();
    let fecha = parse_flex_date(payload.fecha_caducidad);
    let item = sqlx::query_as::<_, InventarioItem>(
        "INSERT INTO inventario_items (id, sku, nombre, cantidad, tipo, estado, unidad_medida, precio, descripcion, modulo, stock_minimo, codigo_trazabilidad, calidad, fase_produccion, fecha_caducidad)
         VALUES ($1, $2, $3, 0, $4, $5, $6, $7, $8, 'FINCA', $9, $10, $11, $12, $13)
         RETURNING *"
    )
    .bind(id).bind(&payload.sku).bind(&payload.nombre).bind(&payload.tipo).bind(&payload.estado).bind(&payload.unidad_medida).bind(payload.precio).bind(&payload.descripcion).bind(payload.stock_minimo.unwrap_or(0.0)).bind(payload.codigo_trazabilidad).bind(payload.calidad).bind(payload.fase_produccion).bind(fecha)
    .fetch_one(&pool).await
    .map_err(|e| {
        tracing::error!("Error al crear item (FINCA): {:?}", e);
        if let Some(db_err) = e.as_database_error() {
            if db_err.code() == Some(std::borrow::Cow::Borrowed("23505")) {
                return StatusCode::CONFLICT;
            }
        }
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    enviar_auditoria(user_id, "FINCA_CREATE_ITEM".to_string(), format!("/inventario/finca/{}", item.id), "N/A".to_string());
    Ok((StatusCode::CREATED, Json(item)))
}

#[utoipa::path(
    get,
    path = "/inventario/finca/{id}",
    tag = "Inventario Finca",
    params(("id" = Uuid, Path, description = "UUID único del ítem")),
    responses(
        (status = 200, description = "Detalle del ítem obtenido correctamente", body = InventarioItem),
        (status = 401, description = "No autorizado"),
        (status = 404, description = "El ítem no existe en el módulo de finca"),
        (status = 500, description = "Error interno del servidor"),
        (status = 503, description = "Servicio no disponible")
    ),
    summary = "Obtener detalle de ítem de finca",
    description = "Recupera la ficha técnica completa de un producto o insumo de la finca, incluyendo datos de trazabilidad si existen."
)]
pub async fn get_item(Path(id): Path<Uuid>, State(pool): State<PgPool>) -> Result<Json<InventarioItem>, StatusCode> {
    let item = sqlx::query_as::<_, InventarioItem>("SELECT * FROM inventario_items WHERE id = $1 AND modulo = 'FINCA' AND is_deleted = false")
        .bind(id).fetch_optional(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(item))
}

#[utoipa::path(
    put,
    path = "/inventario/finca/{id}",
    tag = "Inventario Finca",
    params(("id" = Uuid, Path, description = "UUID del ítem a modificar")),
    request_body = UpdateInventarioItem,
    responses(
        (status = 200, description = "Información actualizada exitosamente", body = InventarioItem),
        (status = 400, description = "Error en el formato de actualización"),
        (status = 401, description = "Acceso denegado"),
        (status = 404, description = "Ítem no encontrado o pertenece a otro módulo"),
        (status = 500, description = "Fallo al escribir en la base de datos")
    ),
    summary = "Actualizar atributos del ítem",
    description = "Modifica los metadatos de un producto de finca. No altera el stock físico (use el endpoint de movimientos)."
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
         WHERE id = $7 AND modulo = 'FINCA' AND is_deleted = false RETURNING *"
    )
    .bind(payload.nombre).bind(payload.precio).bind(payload.descripcion).bind(payload.stock_minimo).bind(payload.unidad_medida).bind(fecha).bind(id)
    .fetch_optional(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;

    enviar_auditoria(user_id, "FINCA_UPDATE_ITEM".to_string(), format!("/inventario/finca/{}", id), "N/A".to_string());
    Ok(Json(item))
}

#[utoipa::path(
    patch,
    path = "/inventario/finca/{id}/estado",
    tag = "Inventario Finca",
    params(("id" = Uuid, Path, description = "ID del ítem")),
    request_body = UpdateEstadoDto,
    responses(
        (status = 200, description = "Estado actualizado", body = InventarioItem),
        (status = 400, description = "Estado no válido para flujo de finca"),
        (status = 401, description = "No autorizado"),
        (status = 404, description = "Ítem no encontrado"),
        (status = 422, description = "Error de validación de esquema")
    ),
    summary = "Modificar estado del ítem (Finca)",
    description = "HU025: Actualización manual de estados operativos (ej. EN_TRANSITO o BLOQUEADO)."
)]
pub async fn update_status(
    Path(id): Path<Uuid>,
    State(pool): State<PgPool>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<UpdateEstadoDto>,
) -> Result<Json<InventarioItem>, StatusCode> {
    let item = sqlx::query_as::<_, InventarioItem>(
        "UPDATE inventario_items SET estado = $1, updated_at = NOW() 
         WHERE id = $2 AND modulo = 'FINCA' AND is_deleted = false RETURNING *"
    )
    .bind(&payload.estado).bind(id)
    .fetch_optional(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;

    enviar_auditoria(user_id, "FINCA_UPDATE_STATUS".to_string(), format!("/inventario/finca/{}/estado", id), "N/A".to_string());
    Ok(Json(item))
}

#[utoipa::path(
    delete,
    path = "/inventario/finca/{id}",
    tag = "Inventario Finca",
    params(("id" = Uuid, Path, description = "ID del ítem")),
    responses(
        (status = 204, description = "Ítem eliminado correctamente"),
        (status = 401, description = "No autorizado"),
        (status = 403, description = "Permisos de administración requeridos"),
        (status = 404, description = "Ítem inexistente"),
        (status = 500, description = "Error interno")
    ),
    summary = "Eliminar ítem de finca (Borrado Lógico)",
    description = "HU024: Marca el ítem como eliminado ocultándolo de las listas generales pero preservando sus datos."
)]
pub async fn delete_item(Path(id): Path<Uuid>, State(pool): State<PgPool>) -> Result<StatusCode, StatusCode> {
    sqlx::query("UPDATE inventario_items SET is_deleted = true WHERE id = $1 AND modulo = 'FINCA' AND is_deleted = false")
        .bind(id).execute(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/inventario/finca/movimientos",
    tag = "Inventario Finca",
    request_body = MovimientoStock,
    responses(
        (status = 201, description = "Movimiento registrado y stock recalculado", body = MovimientoStock),
        (status = 400, description = "Operación no permitida: Stock insuficiente para salida"),
        (status = 401, description = "No autorizado"),
        (status = 404, description = "Ítem destino no existe"),
        (status = 500, description = "Error crítico transaccional")
    ),
    summary = "Registrar entrada/salida de finca",
    description = "Registra una transacción (cosecha, compra de insumos, merma) y actualiza automáticamente la cantidad disponible."
)]
pub async fn create_movement(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<MovimientoStock>,
) -> Result<(StatusCode, Json<MovimientoStock>), StatusCode> {
    let mut tx = pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let item = sqlx::query_as::<_, InventarioItem>("SELECT * FROM inventario_items WHERE id = $1 AND modulo = 'FINCA' FOR UPDATE")
        .bind(payload.item_id).fetch_one(&mut *tx).await.map_err(|_| StatusCode::NOT_FOUND)?;
    let nueva_cantidad = match payload.tipo {
        crate::models::enums::TipoMovimiento::ENTRADA => item.cantidad + payload.cantidad,
        crate::models::enums::TipoMovimiento::SALIDA => {
            if item.cantidad < payload.cantidad { return Err(StatusCode::BAD_REQUEST); }
            item.cantidad - payload.cantidad
        }
    };
    let nuevo_estado = if nueva_cantidad <= 0.0 { EstadoInventario::AGOTADO } else if nueva_cantidad <= item.stock_minimo { EstadoInventario::STOCK_BAJO } else { EstadoInventario::DISPONIBLE };
    sqlx::query("UPDATE inventario_items SET cantidad = $1, estado = $2 WHERE id = $3").bind(nueva_cantidad).bind(nuevo_estado).bind(item.id).execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let movimiento = sqlx::query_as::<_, MovimientoStock>("INSERT INTO movimientos_stock (id, item_id, cantidad, tipo, fecha, motivo) VALUES ($1, $2, $3, $4, NOW(), $5) RETURNING *")
        .bind(Uuid::new_v4()).bind(item.id).bind(payload.cantidad).bind(&payload.tipo).bind(&payload.motivo).fetch_one(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    enviar_auditoria(user_id, "FINCA_MOVEMENT".to_string(), format!("/inventario/finca/movimientos/{}", movimiento.id), "N/A".to_string());
    Ok((StatusCode::CREATED, Json(movimiento)))
}

#[utoipa::path(
    get,
    path = "/inventario/finca/{id}/movimientos",
    tag = "Inventario Finca",
    params(("id" = Uuid, Path, description = "ID del ítem"), MovementFilter),
    responses(
        (status = 200, description = "Historial cronológico de producción recuperado", body = [MovimientoStock]),
        (status = 400, description = "Parámetros de fecha ISO-8601 inválidos"),
        (status = 401, description = "No autorizado"),
        (status = 404, description = "Ítem no encontrado"),
        (status = 500, description = "Error interno")
    ),
    summary = "Obtener historial de ítem (Finca)",
    description = "Lista cronológicamente todos los movimientos de un producto o insumo de finca con filtros de fecha opcionales."
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
         WHERE i.id = $1 AND i.modulo = 'FINCA' AND (m.fecha >= $2 OR $2 IS NULL) AND (m.fecha <= $3 OR $3 IS NULL) ORDER BY m.fecha DESC"
    )
    .bind(id).bind(start).bind(end)
    .fetch_all(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(movements))
}

#[utoipa::path(
    get,
    path = "/inventario/finca/movimientos/exportar",
    tag = "Inventario Finca",
    params(MovementFilter),
    responses(
        (status = 200, description = "CSV de auditoría de finca generado", content_type = "text/csv"),
        (status = 401, description = "No autorizado"),
        (status = 403, description = "Faltan permisos de exportación"),
        (status = 500, description = "Error al serializar datos a CSV"),
        (status = 503, description = "Servicio ocupado")
    ),
    summary = "Exportar auditoría general finca (CSV)",
    description = "HU026: Genera un archivo CSV con el reporte total de movimientos del módulo de finca para control administrativo."
)]
pub async fn export_all_movements_csv(
    State(pool): State<PgPool>,
    Query(filter): Query<MovementFilter>,
) -> impl IntoResponse {
    let start = parse_flex_date(filter.start_date);
    let end = parse_flex_date(filter.end_date);

    let movements = sqlx::query_as::<_, MovimientoStock>(
        "SELECT m.* FROM movimientos_stock m JOIN inventario_items i ON m.item_id = i.id 
         WHERE i.modulo = 'FINCA' AND (m.fecha >= $1 OR $1 IS NULL) AND (m.fecha <= $2 OR $2 IS NULL) ORDER BY m.fecha DESC"
    )
    .bind(start).bind(end)
    .fetch_all(&pool).await.unwrap_or_default();

    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record(&["ID", "ItemID", "Cantidad", "Tipo", "Fecha", "Motivo"]).unwrap();
    for m in movements {
        wtr.write_record(&[m.id.to_string(), m.item_id.to_string(), m.cantidad.to_string(), format!("{:?}", m.tipo), m.fecha.to_string(), m.motivo]).unwrap();
    }
    let csv_data = wtr.into_inner().unwrap();
    (StatusCode::OK, [(header::CONTENT_TYPE, "text/csv"), (header::CONTENT_DISPOSITION, "attachment; filename=\"movimientos_finca.csv\"")], csv_data)
}
