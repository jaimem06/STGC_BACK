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
use chrono::Utc;
use crate::models::{
    InventarioItem, MovimientoStock, CreateInventarioItem, UpdateInventarioItem,
    UpdateEstadoDto, CreateMovimientoDto, HistorialPrecio, HistorialEstado, AlertaStock, StockStats,
};
use crate::utils::audit::enviar_auditoria;
use crate::utils::inventory_helpers::{
    convert_unit, determinar_estado_inventario, parse_flex_date, validar_transicion_estado,
    validate_create_item, validate_update_item,
};

#[derive(Deserialize, utoipa::IntoParams)]
pub struct MovementFilter {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct ListQuery {
    /// Máximo de registros a devolver. Si se omite, devuelve todos.
    pub limit: Option<i64>,
    /// Desplazamiento para paginación. Por defecto 0.
    pub offset: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/inventario/pos",
    tag = "Inventario Modulo 2",
    params(ListQuery),
    responses(
        (status = 200, description = "Catálogo de productos de cafetería recuperado", body = [InventarioItem]),
        (status = 401, description = "No autorizado"),
        (status = 500, description = "Error interno")
    ),
    summary = "Listar ítems de cafetería/POS",
    description = "Obtiene los elementos del inventario del módulo de CAFETERIA con paginación opcional."
)]
pub async fn list_items(
    State(pool): State<PgPool>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<InventarioItem>>, StatusCode> {
    let items = sqlx::query_as::<_, InventarioItem>(
        "SELECT * FROM inventario_items WHERE modulo = 'CAFETERIA' AND is_deleted = false \
         ORDER BY created_at DESC LIMIT $1 OFFSET COALESCE($2, 0)"
    )
    .bind(q.limit)
    .bind(q.offset)
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
    tag = "Inventario Modulo 2",
    request_body = CreateInventarioItem,
    security(("bearer_auth" = [])),
    responses(
        (status = 201, description = "Ítem de POS creado exitosamente", body = InventarioItem),
        (status = 400, description = "Datos de entrada inválidos"),
        (status = 401, description = "Autenticación requerida"),
        (status = 409, description = "SKU o Nombre duplicado"),
        (status = 500, description = "Error interno")
    ),
    summary = "Registrar nuevo ítem de cafetería",
    description = "Crea un nuevo producto o insumo en el inventario del punto de venta."
)]
pub async fn create_item(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<CreateInventarioItem>,
) -> Result<impl IntoResponse, StatusCode> {
    if let Err(msg) = validate_create_item(&payload) {
        return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": msg }))).into_response());
    }

    // EC-05: el SKU es único globalmente (coincide con el constraint UNIQUE de la BD).
    let exists_sku = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM inventario_items WHERE sku = $1")
        .bind(payload.sku.trim()).fetch_one(&pool).await.unwrap_or(0);
    if exists_sku > 0 {
        return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "Ya existe un producto con este SKU." }))).into_response());
    }

    // HU023: comparar el nombre trimeado para evitar duplicados con espacios.
    let exists_nombre = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM inventario_items WHERE nombre = $1 AND modulo = 'CAFETERIA'")
        .bind(payload.nombre.trim()).fetch_one(&pool).await.unwrap_or(0);
    if exists_nombre > 0 {
        return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "Ya existe un producto con este nombre." }))).into_response());
    }

    let id = Uuid::new_v4();

    let fecha = match payload.fecha_caducidad {
        Some(ref fecha_str) if !fecha_str.trim().is_empty() => {
            if let Some(f) = parse_flex_date(Some(fecha_str.clone())) {
                let hoy = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
                let hoy_utc = chrono::TimeZone::from_utc_datetime(&Utc, &hoy);
                if f < hoy_utc {
                    return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "La fecha de caducidad no puede ser anterior a la fecha actual." }))).into_response());
                }
                Some(f)
            } else {
                return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "El formato de fecha debe ser AAAA-MM-DD." }))).into_response());
            }
        },
        _ => None,
    };

    let cantidad_inicial = payload.cantidad_inicial.unwrap_or(0.0);

    let item = sqlx::query_as::<_, InventarioItem>(
        "INSERT INTO inventario_items (id, sku, nombre, cantidad, tipo, estado, unidad_medida, precio, descripcion, fecha_caducidad, modulo, stock_minimo)
         VALUES ($1, $2, $3, $11, $4, $5, $6, $7, $8, $9, 'CAFETERIA', $10)
         RETURNING *"
    )
    .bind(id).bind(payload.sku.trim()).bind(payload.nombre.trim()).bind(&payload.tipo).bind(&payload.estado).bind(&payload.unidad_medida).bind(payload.precio).bind(&payload.descripcion).bind(fecha).bind(payload.stock_minimo.unwrap_or(0.0)).bind(cantidad_inicial)
    .fetch_one(&pool).await
    .map_err(|e| {
        tracing::error!("Error al crear item (POS): {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // HU022: registrar el inventario inicial como movimiento auditable.
    if cantidad_inicial > 0.0 {
        let _ = sqlx::query(
            "INSERT INTO movimientos_stock (id, item_id, cantidad, tipo, fecha, motivo, usuario_id) VALUES ($1, $2, $3, 'ENTRADA', NOW(), 'Inventario inicial', $4)"
        )
        .bind(Uuid::new_v4()).bind(id).bind(cantidad_inicial).bind(&user_id)
        .execute(&pool).await;
    }

    enviar_auditoria(user_id, "POS_CREATE_ITEM".to_string(), format!("/inventario/pos/{}", item.id), "N/A".to_string());
    Ok((StatusCode::CREATED, Json(item)).into_response())
}

#[utoipa::path(
    put,
    path = "/inventario/pos/{id}",
    tag = "Inventario Modulo 2",
    params(("id" = Uuid, Path, description = "ID del ítem")),
    request_body = UpdateInventarioItem,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Ítem actualizado", body = InventarioItem),
        (status = 404, description = "No encontrado"),
        (status = 500, description = "Error interno")
    ),
    summary = "Actualizar atributos del ítem (POS)",
    description = "Modifica los datos descriptivos de un producto de cafetería."
)]
pub async fn update_item(
    Path(id): Path<Uuid>,
    State(pool): State<PgPool>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<UpdateInventarioItem>,
) -> Result<impl IntoResponse, StatusCode> {
    if let Err(msg) = validate_update_item(&payload) {
        return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": msg }))).into_response());
    }

    if let Some(ref nombre) = payload.nombre {
        let n = nombre.trim();
        let exists_nombre = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM inventario_items WHERE nombre = $1 AND id != $2 AND modulo = 'CAFETERIA'")
            .bind(n).bind(id).fetch_one(&pool).await.unwrap_or(0);
        if exists_nombre > 0 {
            return Ok((StatusCode::CONFLICT, Json(serde_json::json!({ "message": "Ya existe un producto con este nombre." }))).into_response());
        }
    }

    let fecha = match payload.fecha_caducidad {
        Some(ref fecha_str) if !fecha_str.trim().is_empty() => {
            if let Some(f) = parse_flex_date(Some(fecha_str.clone())) {
                let hoy = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
                let hoy_utc = chrono::TimeZone::from_utc_datetime(&Utc, &hoy);
                if f < hoy_utc {
                    return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "La fecha de caducidad no puede ser anterior a la fecha actual." }))).into_response());
                }
                Some(f)
            } else {
                return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "El formato de fecha debe ser AAAA-MM-DD." }))).into_response());
            }
        },
        _ => None,
    };

    let current_item = sqlx::query_as::<_, InventarioItem>(
        "SELECT * FROM inventario_items WHERE id = $1 AND modulo = 'CAFETERIA' AND is_deleted = false"
    ).bind(id).fetch_optional(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let current_item = match current_item {
        Some(item) => item,
        None => return Ok((StatusCode::NOT_FOUND, Json(serde_json::json!({ "message": "Producto no encontrado." }))).into_response()),
    };

    let mut new_cantidad = current_item.cantidad;
    if let Some(ref new_um) = payload.unidad_medida {
        if new_um != &current_item.unidad_medida {
            new_cantidad = match convert_unit(current_item.cantidad, &current_item.unidad_medida, new_um) {
                Ok(c) => c,
                Err(e) => return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": e }))).into_response()),
            };
        }
    }

    let nombre_trim = payload.nombre.as_ref().map(|n| n.trim().to_string());

    let mut tx = pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // HU019: si el precio cambia, registrar la bitácora antes de sobrescribir.
    if let Some(nuevo_precio) = payload.precio {
        if (nuevo_precio - current_item.precio).abs() > f64::EPSILON {
            sqlx::query(
                "INSERT INTO historial_precios (item_id, precio_anterior, precio_nuevo, motivo, usuario_id) VALUES ($1, $2, $3, $4, $5)"
            )
            .bind(id).bind(current_item.precio).bind(nuevo_precio).bind(&payload.motivo).bind(&user_id)
            .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }

    let nuevo_stock_minimo = payload.stock_minimo.unwrap_or(current_item.stock_minimo);
    let nuevo_estado = determinar_estado_inventario(new_cantidad, nuevo_stock_minimo);

    let item = sqlx::query_as::<_, InventarioItem>(
        "UPDATE inventario_items SET nombre = COALESCE($1, nombre), precio = COALESCE($2, precio),
         descripcion = COALESCE($3, descripcion), stock_minimo = COALESCE($4, stock_minimo),
         unidad_medida = COALESCE($5, unidad_medida), fecha_caducidad = COALESCE($6, fecha_caducidad),
         cantidad = $7, estado = $8, updated_at = NOW()
         WHERE id = $9 AND modulo = 'CAFETERIA' AND is_deleted = false RETURNING *"
    )
    .bind(nombre_trim).bind(payload.precio).bind(payload.descripcion).bind(payload.stock_minimo).bind(payload.unidad_medida).bind(fecha).bind(new_cantidad).bind(&nuevo_estado).bind(id)
    .fetch_optional(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let item = match item {
        Some(i) => i,
        None => return Ok((StatusCode::NOT_FOUND, Json(serde_json::json!({ "message": "Producto no encontrado." }))).into_response()),
    };

    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    enviar_auditoria(user_id, "POS_UPDATE_ITEM".to_string(), format!("/inventario/pos/{}", id), "N/A".to_string());
    Ok(Json(item).into_response())
}

#[utoipa::path(
    patch,
    path = "/inventario/pos/{id}/estado",
    tag = "Inventario Modulo 2",
    params(("id" = Uuid, Path, description = "ID del ítem")),
    request_body = UpdateEstadoDto,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Estado actualizado", body = InventarioItem),
        (status = 404, description = "No encontrado"),
        (status = 422, description = "Transición de estado inviable")
    ),
    summary = "Actualizar estado operativo (POS)",
    description = "Permite cambiar manualmente el estado respetando las reglas de negocio (HU025)."
)]
pub async fn update_status(
    Path(id): Path<Uuid>,
    State(pool): State<PgPool>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<UpdateEstadoDto>,
) -> Result<impl IntoResponse, StatusCode> {
    let current = sqlx::query_as::<_, InventarioItem>(
        "SELECT * FROM inventario_items WHERE id = $1 AND modulo = 'CAFETERIA' AND is_deleted = false"
    ).bind(id).fetch_optional(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let current = match current {
        Some(i) => i,
        None => return Ok((StatusCode::NOT_FOUND, Json(serde_json::json!({ "message": "Producto no encontrado." }))).into_response()),
    };

    // HU025: validar la transición de estado según la matemática del inventario.
    if let Err(msg) = validar_transicion_estado(&payload.estado, &current) {
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, Json(serde_json::json!({ "message": msg }))).into_response());
    }

    let mut tx = pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if current.estado != payload.estado {
        sqlx::query(
            "INSERT INTO historial_estados (item_id, estado_anterior, estado_nuevo, motivo, usuario_id) VALUES ($1, $2, $3, $4, $5)"
        )
        .bind(id).bind(current.estado).bind(payload.estado).bind(&payload.motivo).bind(&user_id)
        .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    let item = sqlx::query_as::<_, InventarioItem>(
        "UPDATE inventario_items SET estado = $1, updated_at = NOW()
         WHERE id = $2 AND modulo = 'CAFETERIA' AND is_deleted = false RETURNING *"
    )
    .bind(&payload.estado).bind(id)
    .fetch_one(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    enviar_auditoria(user_id, "POS_UPDATE_STATUS".to_string(), format!("/inventario/pos/{}/estado", id), "N/A".to_string());
    Ok(Json(item).into_response())
}

#[utoipa::path(
    delete,
    path = "/inventario/pos/{id}",
    tag = "Inventario Modulo 2",
    params(("id" = Uuid, Path, description = "ID del ítem")),
    responses(
        (status = 204, description = "Dado de baja con éxito"),
        (status = 404, description = "No encontrado")
    ),
    summary = "Dar de baja ítem (Borrado lógico POS)",
    description = "Marca un ítem como eliminado (is_deleted) y lo pasa a INACTIVO."
)]
pub async fn delete_item(
    Path(id): Path<Uuid>,
    State(pool): State<PgPool>,
    Extension(user_id): Extension<String>,
) -> Result<StatusCode, StatusCode> {
    // HU024: filtrar is_deleted=false y verificar filas afectadas; pasar a INACTIVO.
    let result = sqlx::query(
        "UPDATE inventario_items SET is_deleted = true, estado = 'INACTIVO', updated_at = NOW() \
         WHERE id = $1 AND modulo = 'CAFETERIA' AND is_deleted = false"
    )
    .bind(id).execute(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    enviar_auditoria(user_id, "POS_DELETE_ITEM".to_string(), format!("/inventario/pos/{}", id), "N/A".to_string());
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/inventario/pos/movimientos",
    tag = "Inventario Modulo 2",
    request_body = CreateMovimientoDto,
    security(("bearer_auth" = [])),
    responses(
        (status = 201, description = "Movimiento registrado", body = MovimientoStock),
        (status = 400, description = "Cantidad inválida o stock insuficiente"),
        (status = 404, description = "Ítem no encontrado")
    ),
    summary = "Registrar movimiento de stock (POS)",
    description = "Registra una entrada o salida y actualiza el stock actual."
)]
pub async fn create_movement(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<CreateMovimientoDto>,
) -> Result<(StatusCode, Json<MovimientoStock>), StatusCode> {
    // EC-02: validar cantidad positiva (antes solo lo hacía Finca).
    if payload.cantidad <= 0.0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut tx = pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // EC-01: no permitir movimientos sobre ítems dados de baja.
    let item = sqlx::query_as::<_, InventarioItem>("SELECT * FROM inventario_items WHERE id = $1 AND modulo = 'CAFETERIA' AND is_deleted = false FOR UPDATE")
        .bind(payload.item_id).fetch_one(&mut *tx).await.map_err(|_| StatusCode::NOT_FOUND)?;

    let nueva_cantidad = match payload.tipo {
        crate::models::enums::TipoMovimiento::ENTRADA => item.cantidad + payload.cantidad,
        crate::models::enums::TipoMovimiento::SALIDA => {
            if item.cantidad < payload.cantidad { return Err(StatusCode::BAD_REQUEST); }
            item.cantidad - payload.cantidad
        }
    };

    let nuevo_estado = determinar_estado_inventario(nueva_cantidad, item.stock_minimo);

    sqlx::query("UPDATE inventario_items SET cantidad = $1, estado = $2, updated_at = NOW() WHERE id = $3")
        .bind(nueva_cantidad).bind(nuevo_estado).bind(item.id).execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // HU025: registrar el cambio de estado automático en la bitácora.
    if nuevo_estado != item.estado {
        sqlx::query(
            "INSERT INTO historial_estados (item_id, estado_anterior, estado_nuevo, motivo, usuario_id) VALUES ($1, $2, $3, $4, $5)"
        )
        .bind(item.id).bind(item.estado).bind(nuevo_estado).bind("Cambio automático por movimiento de stock").bind(&user_id)
        .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    // EC-03/EC-04: incluir usuario_id y numero_factura en el registro del movimiento.
    let movimiento = sqlx::query_as::<_, MovimientoStock>(
        "INSERT INTO movimientos_stock (id, item_id, cantidad, tipo, fecha, motivo, lote_id, usuario_id, numero_factura) \
         VALUES ($1, $2, $3, $4, NOW(), $5, $6, $7, $8) RETURNING *"
    )
    .bind(Uuid::new_v4()).bind(item.id).bind(payload.cantidad).bind(&payload.tipo).bind(&payload.motivo).bind(payload.lote_id).bind(&user_id).bind(&payload.numero_factura)
    .fetch_one(&mut *tx).await.map_err(|e| {
        tracing::error!("Error SQL insert_movement (POS): {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    enviar_auditoria(user_id, "POS_MOVEMENT".to_string(), format!("/inventario/pos/movimientos/{}", movimiento.id), "N/A".to_string());
    Ok((StatusCode::CREATED, Json(movimiento)))
}

#[utoipa::path(
    get,
    path = "/inventario/pos/{id}/movimientos",
    tag = "Inventario Modulo 2",
    params(("id" = Uuid, Path, description = "ID del ítem"), MovementFilter),
    responses(
        (status = 200, description = "Lista de movimientos", body = [MovimientoStock]),
        (status = 404, description = "No encontrado")
    ),
    summary = "Historial de movimientos por ítem (POS)",
    description = "Obtiene los movimientos históricos de un producto específico en cafetería."
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
    tag = "Inventario Modulo 2",
    params(MovementFilter),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Archivo CSV con movimientos", content_type = "text/csv"),
        (status = 401, description = "No autorizado"),
        (status = 403, description = "Permisos insuficientes")
    ),
    summary = "Exportar auditoría general POS (CSV)",
    description = "Genera un reporte en CSV de todos los movimientos de la cafetería. Requiere autenticación."
)]
pub async fn export_all_movements_csv(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<String>,
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
    wtr.write_record(["ID", "ItemID", "Cantidad", "Tipo", "Fecha", "Motivo", "LoteID", "NumeroFactura"]).unwrap();
    for m in movements {
        wtr.write_record([
            m.id.to_string(),
            m.item_id.to_string(),
            m.cantidad.to_string(),
            format!("{:?}", m.tipo),
            m.fecha.to_rfc3339(),
            m.motivo,
            m.lote_id.map(|u| u.to_string()).unwrap_or_default(),
            m.numero_factura.unwrap_or_default(),
        ]).unwrap();
    }
    let csv_data = wtr.into_inner().unwrap();

    enviar_auditoria(user_id, "POS_EXPORT_CSV".to_string(), "/inventario/pos/movimientos/exportar".to_string(), "N/A".to_string());

    (StatusCode::OK, [
        (header::CONTENT_TYPE, "text/csv"),
        (header::CONTENT_DISPOSITION, "attachment; filename=\"movimientos_pos.csv\"")
    ], csv_data)
}

// --- HU024: papelera (baja / restauración) ---

/// Lista los ítems dados de baja (is_deleted = true) del módulo POS.
pub async fn list_deleted(State(pool): State<PgPool>) -> Result<Json<Vec<InventarioItem>>, StatusCode> {
    let items = sqlx::query_as::<_, InventarioItem>(
        "SELECT * FROM inventario_items WHERE modulo = 'CAFETERIA' AND is_deleted = true ORDER BY updated_at DESC"
    )
    .fetch_all(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(items))
}

/// Restaura un ítem dado de baja, recalculando su estado según el stock actual.
pub async fn restore_item(
    Path(id): Path<Uuid>,
    State(pool): State<PgPool>,
    Extension(user_id): Extension<String>,
) -> Result<Json<InventarioItem>, StatusCode> {
    let item = sqlx::query_as::<_, InventarioItem>(
        "SELECT * FROM inventario_items WHERE id = $1 AND modulo = 'CAFETERIA' AND is_deleted = true"
    ).bind(id).fetch_optional(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let nuevo_estado = determinar_estado_inventario(item.cantidad, item.stock_minimo);
    let restored = sqlx::query_as::<_, InventarioItem>(
        "UPDATE inventario_items SET is_deleted = false, estado = $1, updated_at = NOW() WHERE id = $2 RETURNING *"
    )
    .bind(nuevo_estado).bind(id)
    .fetch_one(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    enviar_auditoria(user_id, "POS_RESTORE_ITEM".to_string(), format!("/inventario/pos/{}/restaurar", id), "N/A".to_string());
    Ok(Json(restored))
}

// --- HU028: analítica de inventario ---

/// Lista los ítems cuyo stock está en o por debajo del mínimo configurado.
pub async fn list_alertas_stock(State(pool): State<PgPool>) -> Result<Json<Vec<AlertaStock>>, StatusCode> {
    let alertas = sqlx::query_as::<_, AlertaStock>(
        "SELECT id as item_id, nombre, cantidad as cantidad_actual, stock_minimo,
            'Stock por debajo del mínimo' as mensaje
         FROM inventario_items
         WHERE modulo = 'CAFETERIA' AND is_deleted = false AND cantidad > 0 AND cantidad <= stock_minimo
         ORDER BY (cantidad / NULLIF(stock_minimo, 0)) ASC NULLS FIRST"
    )
    .fetch_all(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(alertas))
}

/// Métricas consolidadas de inventario para el dashboard.
pub async fn get_stats(State(pool): State<PgPool>) -> Result<Json<StockStats>, StatusCode> {
    let stats = sqlx::query_as::<_, StockStats>(
        "SELECT
            COUNT(*) as total_items,
            COUNT(*) FILTER (WHERE estado = 'DISPONIBLE') as disponibles,
            COUNT(*) FILTER (WHERE estado = 'STOCK_BAJO') as stock_bajo,
            COUNT(*) FILTER (WHERE estado = 'AGOTADO') as agotados,
            COALESCE(SUM(cantidad * precio), 0) as valor_total,
            (SELECT COUNT(*) FROM lotes_cafe) as num_lotes
         FROM inventario_items WHERE modulo = 'CAFETERIA' AND is_deleted = false"
    )
    .fetch_one(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(stats))
}

// --- HU019 / HU025: bitácoras ---

/// Historial de cambios de precio de un ítem (HU019).
pub async fn list_price_history(
    Path(id): Path<Uuid>,
    State(pool): State<PgPool>,
) -> Result<Json<Vec<HistorialPrecio>>, StatusCode> {
    let history = sqlx::query_as::<_, HistorialPrecio>(
        "SELECT * FROM historial_precios WHERE item_id = $1 ORDER BY fecha_cambio DESC"
    ).bind(id).fetch_all(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(history))
}

/// Historial de cambios de estado de un ítem (HU025).
pub async fn list_status_history(
    Path(id): Path<Uuid>,
    State(pool): State<PgPool>,
) -> Result<Json<Vec<HistorialEstado>>, StatusCode> {
    let history = sqlx::query_as::<_, HistorialEstado>(
        "SELECT * FROM historial_estados WHERE item_id = $1 ORDER BY fecha DESC"
    ).bind(id).fetch_all(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(history))
}

pub async fn get_item_with_pool(pool: &PgPool, id: Uuid) -> Result<InventarioItem, StatusCode> {
    sqlx::query_as::<_, InventarioItem>("SELECT * FROM inventario_items WHERE id = $1 AND modulo = 'CAFETERIA' AND is_deleted = false")
        .bind(id).fetch_optional(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)
}

#[utoipa::path(
    get,
    path = "/inventario/pos/{id}",
    tag = "Inventario Modulo 2",
    params(("id" = Uuid, Path, description = "ID del ítem")),
    responses(
        (status = 200, description = "Detalle del ítem", body = InventarioItem),
        (status = 404, description = "No encontrado")
    ),
    summary = "Obtener detalle de ítem (POS)",
    description = "Recupera la información completa de un producto de cafetería."
)]
pub async fn get_item(Path(id): Path<Uuid>, State(pool): State<PgPool>) -> Result<Json<InventarioItem>, StatusCode> {
    Ok(Json(get_item_with_pool(&pool, id).await?))
}
