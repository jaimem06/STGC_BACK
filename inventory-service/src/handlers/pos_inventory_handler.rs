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
    UpdateEstadoDto, CreateMovimientoDto, enums::{EstadoInventario, UnidadMedida}
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
    tag = "Inventario Modulo 2",
    responses(
        (status = 200, description = "Catálogo de productos de cafetería recuperado", body = [InventarioItem]),
        (status = 401, description = "No autorizado"),
        (status = 500, description = "Error interno")
    ),
    summary = "Listar ítems de cafetería/POS",
    description = "Obtiene todos los elementos del inventario pertenecientes al módulo de CAFETERIA."
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

fn validate_create_item(payload: &CreateInventarioItem) -> Result<(), String> {
    let nombre = payload.nombre.trim();
    if nombre.is_empty() { return Err("El nombre del producto es obligatorio.".into()); }
    if nombre.len() < 3 || nombre.len() > 90 { return Err("El nombre debe tener entre 3 y 90 caracteres.".into()); }

    let sku = payload.sku.trim();
    if sku.is_empty() { return Err("El SKU es obligatorio.".into()); }
    if sku.len() != 6 { return Err("El SKU debe tener exactamente 6 caracteres.".into()); }
    let chars: Vec<char> = sku.chars().collect();
    if !sku.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
        return Err("El SKU solo puede contener letras mayúsculas y números.".into());
    }
    let has_3_upper = chars[0..3].iter().all(|c| c.is_ascii_uppercase());
    let has_3_digits = chars[3..6].iter().all(|c| c.is_ascii_digit());
    if !has_3_upper || !has_3_digits {
        return Err("El SKU debe tener 3 letras mayúsculas seguidas de 3 números.".into());
    }

    if payload.precio == 0.0 { return Err("El precio debe ser mayor a 0.".into()); }
    if payload.precio < 0.0 { return Err("El precio no puede ser negativo.".into()); }
    if payload.precio > 10000.0 { return Err("El precio no puede superar 10000.".into()); }
    let precio_str = payload.precio.to_string();
    if let Some(pos) = precio_str.find('.') {
        if precio_str.len() - pos - 1 > 2 {
            return Err("El precio solo puede tener hasta dos decimales.".into());
        }
    }

    if payload.stock_minimo.is_none() { return Err("El stock mínimo es obligatorio.".into()); }
    let minimo = payload.stock_minimo.unwrap();
    if minimo < 0.0 { return Err("El stock mínimo no puede ser un número negativo.".into()); }
    if minimo.fract().abs() > 1e-6 { return Err("El stock mínimo debe ser un número entero.".into()); }
    if minimo > 30.0 { return Err("El stock mínimo no puede superar 30.".into()); }

    let desc = payload.descripcion.as_deref().unwrap_or("").trim();
    if desc.is_empty() { return Err("La descripción del producto es obligatoria.".into()); }
    if desc.len() < 20 || desc.len() > 250 { return Err("La descripción debe tener entre 20 y 250 caracteres.".into()); }

    if let Some(ci) = payload.cantidad_inicial {
        if ci <= 0.0 { return Err("La cantidad debe ser mayor a 0.".into()); }
        if ci > 10000.0 { return Err("La cantidad no puede superar 10000.".into()); }
        let ci_str = ci.to_string();
        if let Some(pos) = ci_str.find('.') {
            if ci_str.len() - pos - 1 > 2 {
                return Err("La cantidad debe ser un número con hasta dos decimales.".into());
            }
        }
    }

    Ok(())
}

#[utoipa::path(
    post,
    path = "/inventario/pos/nuevo",
    tag = "Inventario Modulo 2",
    request_body = CreateInventarioItem,
    security(
        ("bearer_auth" = [])
    ),
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

    let exists_sku = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM inventario_items WHERE sku = $1 AND modulo = 'CAFETERIA'")
        .bind(&payload.sku).fetch_one(&pool).await.unwrap_or(0);
    if exists_sku > 0 {
        return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "Ya existe un producto con este SKU." }))).into_response());
    }

    let exists_nombre = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM inventario_items WHERE nombre = $1 AND modulo = 'CAFETERIA'")
        .bind(&payload.nombre).fetch_one(&pool).await.unwrap_or(0);
    if exists_nombre > 0 {
        return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "Ya existe un producto con este nombre." }))).into_response());
    }

    let id = Uuid::new_v4();
    
    let fecha = match payload.fecha_caducidad {
        Some(ref fecha_str) if !fecha_str.trim().is_empty() => {
            if let Some(f) = parse_flex_date(Some(fecha_str.clone())) {
                let hoy = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
                let hoy_utc = Utc.from_utc_datetime(&hoy);
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
    .bind(id).bind(&payload.sku).bind(&payload.nombre).bind(&payload.tipo).bind(&payload.estado).bind(&payload.unidad_medida).bind(payload.precio).bind(&payload.descripcion).bind(fecha).bind(payload.stock_minimo.unwrap_or(0.0)).bind(cantidad_inicial)
    .fetch_one(&pool).await
    .map_err(|e| {
        tracing::error!("Error al crear item (POS): {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if cantidad_inicial > 0.0 {
        let _ = sqlx::query(
            "INSERT INTO movimientos_stock (id, item_id, cantidad, tipo, fecha, motivo) VALUES ($1, $2, $3, 'ENTRADA', NOW(), 'Inventario inicial')"
        )
        .bind(Uuid::new_v4()).bind(id).bind(cantidad_inicial)
        .execute(&pool).await;
    }

    enviar_auditoria(user_id, "POS_CREATE_ITEM".to_string(), format!("/inventario/pos/{}", item.id), "N/A".to_string());
    Ok((StatusCode::CREATED, Json(item)).into_response())
}

fn convert_unit(cantidad: f64, from: &UnidadMedida, to: &UnidadMedida) -> Result<f64, String> {
    if from == to { return Ok(cantidad); }
    
    let is_mass = |u: &UnidadMedida| matches!(u, UnidadMedida::QUINTALES | UnidadMedida::ARROBAS | UnidadMedida::LIBRAS | UnidadMedida::KILOGRAMOS);
    
    if is_mass(from) && is_mass(to) {
        let in_lb = match from {
            UnidadMedida::LIBRAS => cantidad,
            UnidadMedida::ARROBAS => cantidad * 25.0,
            UnidadMedida::QUINTALES => cantidad * 100.0,
            UnidadMedida::KILOGRAMOS => cantidad * 2.20462,
            _ => unreachable!(),
        };
        let result = match to {
            UnidadMedida::LIBRAS => in_lb,
            UnidadMedida::ARROBAS => in_lb / 25.0,
            UnidadMedida::QUINTALES => in_lb / 100.0,
            UnidadMedida::KILOGRAMOS => in_lb / 2.20462,
            _ => unreachable!(),
        };
        Ok((result * 100.0).round() / 100.0) // Redondear a 2 decimales
    } else {
        Err(format!("No se puede convertir de {:?} a {:?}", from, to))
    }
}

#[utoipa::path(
    put,
    path = "/inventario/pos/{id}",
    tag = "Inventario Modulo 2",
    params(("id" = Uuid, Path, description = "ID del ítem")),
    request_body = UpdateInventarioItem,
    security(
        ("bearer_auth" = [])
    ),
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
    if let Some(ref nombre) = payload.nombre {
        let n = nombre.trim();
        if n.is_empty() { return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "El nombre del producto es obligatorio." }))).into_response()); }
        if n.len() < 3 || n.len() > 90 { return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "El nombre debe tener entre 3 y 90 caracteres." }))).into_response()); }

        let exists_nombre = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM inventario_items WHERE nombre = $1 AND id != $2 AND modulo = 'CAFETERIA'")
            .bind(n).bind(id).fetch_one(&pool).await.unwrap_or(0);
        if exists_nombre > 0 {
            return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "Ya existe un producto con este nombre." }))).into_response());
        }
    }

    if let Some(ref desc) = payload.descripcion {
        let d = desc.trim();
        if d.is_empty() { return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "La descripción del producto es obligatoria." }))).into_response()); }
        if d.len() < 20 || d.len() > 250 { return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "La descripción debe tener entre 20 y 250 caracteres." }))).into_response()); }
    }

    if let Some(precio) = payload.precio {
        if precio == 0.0 { return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "El precio debe ser mayor a 0." }))).into_response()); }
        if precio < 0.0 { return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "El precio no puede ser negativo." }))).into_response()); }
        if precio > 10000.0 { return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "El precio no puede superar 10000." }))).into_response()); }
        let precio_str = precio.to_string();
        if let Some(pos) = precio_str.find('.') {
            if precio_str.len() - pos - 1 > 2 {
                return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "El precio solo puede tener hasta dos decimales." }))).into_response());
            }
        }
    }

    if let Some(minimo) = payload.stock_minimo {
        if minimo < 0.0 { return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "El stock mínimo no puede ser un número negativo." }))).into_response()); }
        if minimo.fract().abs() > 1e-6 { return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "El stock mínimo debe ser un número entero." }))).into_response()); }
        if minimo > 30.0 { return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "El stock mínimo no puede superar 30." }))).into_response()); }
    }

    let fecha = match payload.fecha_caducidad {
        Some(ref fecha_str) if !fecha_str.trim().is_empty() => {
            if let Some(f) = parse_flex_date(Some(fecha_str.clone())) {
                let hoy = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
                let hoy_utc = Utc.from_utc_datetime(&hoy);
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

    let item = sqlx::query_as::<_, InventarioItem>(
        "UPDATE inventario_items SET nombre = COALESCE($1, nombre), precio = COALESCE($2, precio), 
         descripcion = COALESCE($3, descripcion), stock_minimo = COALESCE($4, stock_minimo), 
         unidad_medida = COALESCE($5, unidad_medida), fecha_caducidad = COALESCE($6, fecha_caducidad),
         cantidad = $7,
         updated_at = NOW()
         WHERE id = $8 AND modulo = 'CAFETERIA' AND is_deleted = false RETURNING *"
    )
    .bind(payload.nombre).bind(payload.precio).bind(payload.descripcion).bind(payload.stock_minimo).bind(payload.unidad_medida).bind(fecha).bind(new_cantidad).bind(id)
    .fetch_optional(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;

    enviar_auditoria(user_id, "POS_UPDATE_ITEM".to_string(), format!("/inventario/pos/{}", id), "N/A".to_string());
    Ok(Json(item).into_response())
}

#[utoipa::path(
    patch,
    path = "/inventario/pos/{id}/estado",
    tag = "Inventario Modulo 2",
    params(("id" = Uuid, Path, description = "ID del ítem")),
    request_body = UpdateEstadoDto,
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Estado actualizado", body = InventarioItem),
        (status = 404, description = "No encontrado")
    ),
    summary = "Actualizar estado operativo (POS)",
    description = "Permite cambiar manualmente el estado (ej. DISPONIBLE a AGOTADO)."
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
    .fetch_optional(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;

    enviar_auditoria(user_id, "POS_UPDATE_STATUS".to_string(), format!("/inventario/pos/{}/estado", id), "N/A".to_string());
    Ok(Json(item))
}

#[utoipa::path(
    delete,
    path = "/inventario/pos/{id}",
    tag = "Inventario Modulo 2",
    params(("id" = Uuid, Path, description = "ID del ítem")),
    responses(
        (status = 204, description = "Eliminado con éxito"),
        (status = 404, description = "No encontrado")
    ),
    summary = "Eliminar ítem (Borrado lógico POS)",
    description = "Marca un ítem como eliminado para el punto de venta."
)]
pub async fn delete_item(Path(id): Path<Uuid>, State(pool): State<PgPool>) -> Result<StatusCode, StatusCode> {
    sqlx::query("UPDATE inventario_items SET is_deleted = true WHERE id = $1 AND modulo = 'CAFETERIA'")
        .bind(id).execute(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/inventario/pos/movimientos",
    tag = "Inventario Modulo 2",
    request_body = CreateMovimientoDto,
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 201, description = "Movimiento registrado", body = MovimientoStock),
        (status = 400, description = "Stock insuficiente"),
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
    
    let movimiento = sqlx::query_as::<_, MovimientoStock>(
        "INSERT INTO movimientos_stock (id, item_id, cantidad, tipo, fecha, motivo, lote_id) VALUES ($1, $2, $3, $4, NOW(), $5, $6) RETURNING *"
    )
    .bind(Uuid::new_v4()).bind(item.id).bind(payload.cantidad).bind(&payload.tipo).bind(&payload.motivo).bind(payload.lote_id).fetch_one(&mut *tx).await.map_err(|e| {
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
    security(
        ("bearer_auth" = [])
    ),
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
    wtr.write_record(&["ID", "ItemID", "Cantidad", "Tipo", "Fecha", "Motivo", "LoteID"]).unwrap();
    for m in movements {
        wtr.write_record(&[
            m.id.to_string(), 
            m.item_id.to_string(), 
            m.cantidad.to_string(), 
            format!("{:?}", m.tipo), 
            m.fecha.to_rfc3339(), 
            m.motivo,
            m.lote_id.map(|u| u.to_string()).unwrap_or_default()
        ]).unwrap();
    }
    let csv_data = wtr.into_inner().unwrap();
    
    enviar_auditoria(user_id, "POS_EXPORT_CSV".to_string(), "/inventario/pos/movimientos/exportar".to_string(), "N/A".to_string());
    
    (StatusCode::OK, [
        (header::CONTENT_TYPE, "text/csv"), 
        (header::CONTENT_DISPOSITION, "attachment; filename=\"movimientos_pos.csv\"")
    ], csv_data)
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
