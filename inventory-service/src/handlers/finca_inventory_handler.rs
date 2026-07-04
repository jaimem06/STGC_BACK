use axum::{
    extract::{ Path, State, Query },
    http::{ StatusCode, header },
    response::IntoResponse,
    Json,
    Extension,
};
use uuid::Uuid;
use sqlx::PgPool;
use serde::Deserialize;
use chrono::{ DateTime, Utc, TimeZone };
use crate::models::{
    InventarioItem,
    MovimientoStock,
    CreateInventarioItem,
    UpdateInventarioItem,
    UpdateEstadoDto,
    CreateMovimientoDto,
    enums::EstadoInventario,
};
use crate::utils::audit::enviar_auditoria;

#[derive(Deserialize, utoipa::IntoParams)]
pub struct MovementFilter {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

fn parse_flex_date(date_str: Option<String>) -> Option<DateTime<Utc>> {
    let s = date_str?.trim().to_string();
    if s.is_empty() || s == "null" {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(&s) {
        return Some(dt.with_timezone(&Utc));
    }
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
        (
            status = 200,
            description = "Catálogo de insumos y productos de producción recuperado",
            body = [InventarioItem],
        ),
        (status = 401, description = "No autorizado: Requiere token Bearer válido"),
        (status = 403, description = "Prohibido: Rol insuficiente para acceder a datos de finca"),
        (status = 500, description = "Error interno al consultar la base de datos"),
        (status = 503, description = "Servicio de base de datos no disponible")
    ),
    summary = "Listar ítems de producción (Finca)",
    description = "Obtiene todos los elementos del inventario pertenecientes al módulo de Finca. Incluye insumos agrícolas y café en sus diferentes fases."
)]
pub async fn list_items(State(pool): State<PgPool>) -> Result<
    Json<Vec<InventarioItem>>,
    StatusCode
> {
    let items = sqlx
        ::query_as::<_, InventarioItem>(
            "SELECT * FROM inventario_items WHERE modulo = 'FINCA' AND is_deleted = false ORDER BY created_at DESC"
        )
        .fetch_all(&pool).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(items))
}

fn validate_create_item(payload: &CreateInventarioItem) -> Result<(), String> {
    let nombre = payload.nombre.trim();
    if nombre.is_empty() {
        return Err("El nombre del producto es obligatorio.".into());
    }
    if nombre.len() < 3 || nombre.len() > 90 {
        return Err("El nombre debe tener entre 3 y 90 caracteres.".into());
    }

    let sku = payload.sku.trim();
    if sku.is_empty() {
        return Err("El SKU es obligatorio.".into());
    }
    if sku.len() != 6 {
        return Err("El SKU debe tener exactamente 6 caracteres.".into());
    }
    let chars: Vec<char> = sku.chars().collect();
    if !sku.chars().all(|c| (c.is_ascii_uppercase() || c.is_ascii_digit())) {
        return Err("El SKU solo puede contener letras mayúsculas y números.".into());
    }
    let has_3_upper = chars[0..3].iter().all(|c| c.is_ascii_uppercase());
    let has_3_digits = chars[3..6].iter().all(|c| c.is_ascii_digit());
    if !has_3_upper || !has_3_digits {
        return Err("El SKU debe tener 3 letras mayúsculas seguidas de 3 números.".into());
    }

    if payload.precio == 0.0 {
        return Err("El precio debe ser mayor a 0.".into());
    }
    if payload.precio < 0.0 {
        return Err("El precio no puede ser negativo.".into());
    }
    if payload.precio > 10000.0 {
        return Err("El precio no puede superar 10000.".into());
    }
    let precio_str = payload.precio.to_string();
    if let Some(pos) = precio_str.find('.') {
        if precio_str.len() - pos - 1 > 2 {
            return Err("El precio solo puede tener hasta dos decimales.".into());
        }
    }

    if payload.stock_minimo.is_none() {
        return Err("El stock mínimo es obligatorio.".into());
    }
    let minimo = payload.stock_minimo.unwrap();
    if minimo < 0.0 {
        return Err("El stock mínimo no puede ser un número negativo.".into());
    }
    if minimo.fract().abs() > 1e-6 {
        return Err("El stock mínimo debe ser un número entero.".into());
    }
    if minimo > 30.0 {
        return Err("El stock mínimo no puede superar 30.".into());
    }

    let desc = payload.descripcion.as_deref().unwrap_or("").trim();
    if desc.is_empty() {
        return Err("La descripción del producto es obligatoria.".into());
    }
    if desc.len() < 20 || desc.len() > 250 {
        return Err("La descripción debe tener entre 20 y 250 caracteres.".into());
    }

    if let Some(ci) = payload.cantidad_inicial {
        if ci < 0.0 {
            return Err("La cantidad inicial debe ser positiva.".into());
        }
    }

    Ok(())
}

#[utoipa::path(
    post,
    path = "/inventario/finca/nuevo",
    tag = "Inventario Finca",
    request_body = CreateInventarioItem,
    security(("bearer_auth" = [])),
    responses(
        (status = 201, description = "Ítem de finca creado exitosamente", body = InventarioItem),
        (status = 400, description = "Datos de entrada mal formados"),
        (status = 401, description = "Autenticación requerida"),
        (status = 409, description = "Error: El código SKU o Nombre ya existe en el sistema"),
        (status = 422, description = "Faltan campos obligatorios o formato inválido")
    ),
    summary = "Registrar nuevo ítem de finca",
    description = "Crea un nuevo registro en el inventario forzando el módulo a 'FINCA'. Permite asociar códigos de trazabilidad y fases de procesamiento."
)]
pub async fn create_item(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<CreateInventarioItem>
) -> Result<impl IntoResponse, StatusCode> {
    if let Err(msg) = validate_create_item(&payload) {
        return Ok(
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": msg }))).into_response()
        );
    }

    let exists_sku = sqlx
        ::query_scalar::<_, i64>(
            "SELECT count(*) FROM inventario_items WHERE sku = $1 AND modulo = 'FINCA'"
        )
        .bind(&payload.sku)
        .fetch_one(&pool).await
        .unwrap_or(0);
    if exists_sku > 0 {
        return Ok(
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "message": "Ya existe un producto con este SKU." })),
            ).into_response()
        );
    }

    let exists_nombre = sqlx
        ::query_scalar::<_, i64>(
            "SELECT count(*) FROM inventario_items WHERE nombre = $1 AND modulo = 'FINCA'"
        )
        .bind(&payload.nombre)
        .fetch_one(&pool).await
        .unwrap_or(0);
    if exists_nombre > 0 {
        return Ok(
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "message": "Ya existe un producto con este nombre." })),
            ).into_response()
        );
    }

    let id = Uuid::new_v4();

    let fecha = match payload.fecha_caducidad {
        Some(ref fecha_str) if !fecha_str.trim().is_empty() => {
            if let Some(f) = parse_flex_date(Some(fecha_str.clone())) {
                let hoy = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
                let hoy_utc = Utc.from_utc_datetime(&hoy);
                if f < hoy_utc {
                    return Ok(
                        (
                            StatusCode::BAD_REQUEST,
                            Json(
                                serde_json::json!({ "message": "La fecha de caducidad no puede ser anterior a la fecha actual." })
                            ),
                        ).into_response()
                    );
                }
                Some(f)
            } else {
                return Ok(
                    (
                        StatusCode::BAD_REQUEST,
                        Json(
                            serde_json::json!({ "message": "El formato de fecha debe ser AAAA-MM-DD." })
                        ),
                    ).into_response()
                );
            }
        }
        _ => None,
    };

    let cantidad_inicial = payload.cantidad_inicial.unwrap_or(0.0);

    let item = sqlx
        ::query_as::<_, InventarioItem>(
            "INSERT INTO inventario_items (id, sku, nombre, cantidad, tipo, estado, unidad_medida, precio, descripcion, modulo, stock_minimo, codigo_trazabilidad, calidad, fase_produccion, fecha_caducidad)
         VALUES ($1, $2, $3, $14, $4, $5, $6, $7, $8, 'FINCA', $9, $10, $11, $12, $13)
         RETURNING *"
        )
        .bind(id)
        .bind(&payload.sku)
        .bind(&payload.nombre)
        .bind(&payload.tipo)
        .bind(&payload.estado)
        .bind(&payload.unidad_medida)
        .bind(payload.precio)
        .bind(&payload.descripcion)
        .bind(payload.stock_minimo.unwrap_or(0.0))
        .bind(payload.codigo_trazabilidad)
        .bind(payload.calidad)
        .bind(payload.fase_produccion)
        .bind(fecha)
        .bind(cantidad_inicial)
        .fetch_one(&pool).await
        .map_err(|e| {
            tracing::error!("Error al crear item (FINCA): {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if cantidad_inicial > 0.0 {
        let _ = sqlx
            ::query(
                "INSERT INTO movimientos_stock (id, item_id, cantidad, tipo, fecha, motivo) VALUES ($1, $2, $3, 'ENTRADA', NOW(), 'Inventario inicial')"
            )
            .bind(Uuid::new_v4())
            .bind(id)
            .bind(cantidad_inicial)
            .execute(&pool).await;
    }

    enviar_auditoria(
        user_id,
        "FINCA_CREATE_ITEM".to_string(),
        format!("/inventario/finca/{}", item.id),
        "N/A".to_string()
    );
    Ok((StatusCode::CREATED, Json(item)).into_response())
}

#[utoipa::path(
    get,
    path = "/inventario/finca/{id}",
    tag = "Inventario Finca",
    params(("id" = Uuid, Path, description = "UUID único del ítem")),
    responses(
        (
            status = 200,
            description = "Detalle del ítem obtenido correctamente",
            body = InventarioItem,
        ),
        (status = 401, description = "No autorizado"),
        (status = 404, description = "El ítem no existe en el módulo de finca"),
        (status = 500, description = "Error interno del servidor"),
        (status = 503, description = "Servicio no disponible")
    ),
    summary = "Obtener detalle de ítem de finca",
    description = "Recupera la ficha técnica completa de un producto o insumo de la finca, incluyendo datos de trazabilidad si existen."
)]
pub async fn get_item(
    Path(id): Path<Uuid>,
    State(pool): State<PgPool>
) -> Result<Json<InventarioItem>, StatusCode> {
    let item = sqlx
        ::query_as::<_, InventarioItem>(
            "SELECT * FROM inventario_items WHERE id = $1 AND modulo = 'FINCA' AND is_deleted = false"
        )
        .bind(id)
        .fetch_optional(&pool).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(item))
}

fn convert_unit(
    cantidad: f64,
    from: &crate::models::enums::UnidadMedida,
    to: &crate::models::enums::UnidadMedida
) -> Result<f64, String> {
    if from == to {
        return Ok(cantidad);
    }
    use crate::models::enums::UnidadMedida;
    let is_mass = |u: &UnidadMedida|
        matches!(
            u,
            UnidadMedida::QUINTALES |
                UnidadMedida::ARROBAS |
                UnidadMedida::LIBRAS |
                UnidadMedida::KILOGRAMOS
        );

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
        Ok((result * 100.0).round() / 100.0)
    } else {
        Err(format!("No se puede convertir de {:?} a {:?}", from, to))
    }
}

#[utoipa::path(
    put,
    path = "/inventario/finca/{id}",
    tag = "Inventario Finca",
    params(("id" = Uuid, Path, description = "UUID del ítem a modificar")),
    request_body = UpdateInventarioItem,
    security(("bearer_auth" = [])),
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
    Json(payload): Json<UpdateInventarioItem>
) -> Result<Json<InventarioItem>, StatusCode> {
    let fecha = parse_flex_date(payload.fecha_caducidad.clone());

    let current_item = sqlx
        ::query_as::<_, InventarioItem>(
            "SELECT * FROM inventario_items WHERE id = $1 AND modulo = 'FINCA' AND is_deleted = false"
        )
        .bind(id)
        .fetch_optional(&pool).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let current_item = match current_item {
        Some(item) => item,
        None => {
            return Err(StatusCode::NOT_FOUND);
        }
    };

    let mut new_cantidad = current_item.cantidad;
    if let Some(ref new_um) = payload.unidad_medida {
        if new_um != &current_item.unidad_medida {
            new_cantidad = match
                convert_unit(current_item.cantidad, &current_item.unidad_medida, new_um)
            {
                Ok(c) => c,
                Err(_) => {
                    return Err(StatusCode::BAD_REQUEST);
                }
            };
        }
    }

    let item = sqlx
        ::query_as::<_, InventarioItem>(
            "UPDATE inventario_items SET 
            nombre = COALESCE($1, nombre), 
            precio = COALESCE($2, precio), 
            descripcion = COALESCE($3, descripcion), 
            stock_minimo = COALESCE($4, stock_minimo), 
            unidad_medida = COALESCE($5, unidad_medida),
            fecha_caducidad = COALESCE($6, fecha_caducidad),
            cantidad = $7,
            updated_at = NOW()
         WHERE id = $8 AND modulo = 'FINCA' AND is_deleted = false RETURNING *"
        )
        .bind(payload.nombre)
        .bind(payload.precio)
        .bind(payload.descripcion)
        .bind(payload.stock_minimo)
        .bind(payload.unidad_medida)
        .bind(fecha)
        .bind(new_cantidad)
        .bind(id)
        .fetch_optional(&pool).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    enviar_auditoria(
        user_id,
        "FINCA_UPDATE_ITEM".to_string(),
        format!("/inventario/finca/{}", id),
        "N/A".to_string()
    );
    Ok(Json(item))
}

#[utoipa::path(
    patch,
    path = "/inventario/finca/{id}/estado",
    tag = "Inventario Finca",
    params(("id" = Uuid, Path, description = "ID del ítem")),
    request_body = UpdateEstadoDto,
    security(("bearer_auth" = [])),
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
    Json(payload): Json<UpdateEstadoDto>
) -> Result<Json<InventarioItem>, StatusCode> {
    let item = sqlx
        ::query_as::<_, InventarioItem>(
            "UPDATE inventario_items SET estado = $1, updated_at = NOW() 
         WHERE id = $2 AND modulo = 'FINCA' AND is_deleted = false RETURNING *"
        )
        .bind(&payload.estado)
        .bind(id)
        .fetch_optional(&pool).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    enviar_auditoria(
        user_id,
        "FINCA_UPDATE_STATUS".to_string(),
        format!("/inventario/finca/{}/estado", id),
        "N/A".to_string()
    );
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
pub async fn delete_item(
    Path(id): Path<Uuid>,
    State(pool): State<PgPool>
) -> Result<StatusCode, StatusCode> {
    sqlx
        ::query(
            "UPDATE inventario_items SET is_deleted = true WHERE id = $1 AND modulo = 'FINCA' AND is_deleted = false"
        )
        .bind(id)
        .execute(&pool).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/inventario/finca/movimientos",
    tag = "Inventario Finca",
    request_body = CreateMovimientoDto,
    security(("bearer_auth" = [])),
    responses(
        (
            status = 201,
            description = "Movimiento registrado y stock recalculado",
            body = MovimientoStock,
        ),
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
    Json(payload): Json<CreateMovimientoDto>
) -> Result<(StatusCode, Json<MovimientoStock>), StatusCode> {
    if payload.cantidad <= 0.0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut tx = pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let item = sqlx
        ::query_as::<_, InventarioItem>(
            "SELECT * FROM inventario_items WHERE id = $1 AND modulo = 'FINCA' FOR UPDATE"
        )
        .bind(payload.item_id)
        .fetch_one(&mut *tx).await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let nueva_cantidad = match payload.tipo {
        crate::models::enums::TipoMovimiento::ENTRADA => { item.cantidad + payload.cantidad }
        crate::models::enums::TipoMovimiento::SALIDA => {
            if item.cantidad < payload.cantidad {
                return Err(StatusCode::BAD_REQUEST);
            }
            item.cantidad - payload.cantidad
        }
    };

    let nuevo_estado = determinar_estado_inventario(nueva_cantidad, item.stock_minimo);

    sqlx
        ::query(
            "UPDATE inventario_items SET cantidad = $1, estado = $2, updated_at = NOW() WHERE id = $3"
        )
        .bind(nueva_cantidad)
        .bind(nuevo_estado)
        .bind(item.id)
        .execute(&mut *tx).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let movimiento_id = Uuid::new_v4();
    let movimiento = sqlx
        ::query_as::<_, MovimientoStock>(
            "INSERT INTO movimientos_stock (id, item_id, cantidad, tipo, fecha, motivo, lote_id, usuario_id) \
             VALUES ($1, $2, $3, $4, NOW(), $5, $6, $7) RETURNING *"
        )
        .bind(movimiento_id)
        .bind(item.id)
        .bind(payload.cantidad)
        .bind(&payload.tipo)
        .bind(&payload.motivo)
        .bind(payload.lote_id)
        .bind(&user_id)
        .fetch_one(&mut *tx).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tokio::spawn(async move {
        enviar_auditoria(
            user_id,
            "FINCA_MOVEMENT".to_string(),
            format!("/inventario/finca/movimientos/{}", movimiento_id),
            format!("{:?}", payload.tipo)
        );
    });

    Ok((StatusCode::CREATED, Json(movimiento)))
}

fn determinar_estado_inventario(cantidad: f64, stock_minimo: f64) -> EstadoInventario {
    match cantidad {
        q if q <= 0.0 => EstadoInventario::AGOTADO,
        q if q <= stock_minimo => EstadoInventario::STOCK_BAJO,
        _ => EstadoInventario::DISPONIBLE,
    }
}

#[utoipa::path(
    get,
    path = "/inventario/finca/{id}/movimientos",
    tag = "Inventario Finca",
    params(("id" = Uuid, Path, description = "ID del ítem"), MovementFilter),
    responses(
        (
            status = 200,
            description = "Historial cronológico de producción recuperado",
            body = [MovimientoStock],
        ),
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
    Query(filter): Query<MovementFilter>
) -> Result<Json<Vec<MovimientoStock>>, StatusCode> {
    let start = parse_flex_date(filter.start_date);
    let end = parse_flex_date(filter.end_date);

    let movements = sqlx
        ::query_as::<_, MovimientoStock>(
            "SELECT m.* FROM movimientos_stock m JOIN inventario_items i ON m.item_id = i.id 
         WHERE i.id = $1 AND i.modulo = 'FINCA' AND (m.fecha >= $2 OR $2 IS NULL) AND (m.fecha <= $3 OR $3 IS NULL) ORDER BY m.fecha DESC"
        )
        .bind(id)
        .bind(start)
        .bind(end)
        .fetch_all(&pool).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(movements))
}

#[utoipa::path(
    get,
    path = "/inventario/finca/movimientos/exportar",
    tag = "Inventario Finca",
    params(MovementFilter),
    security(("bearer_auth" = [])),
    responses(
        (
            status = 200,
            description = "CSV de auditoría de finca generado",
            content_type = "text/csv",
        ),
        (status = 401, description = "No autorizado"),
        (status = 403, description = "Faltan permisos de exportación"),
        (status = 500, description = "Error al serializar datos a CSV"),
        (status = 503, description = "Servicio ocupado")
    ),
    summary = "Exportar auditoría general finca (CSV)",
    description = "HU026: Genera un archivo CSV con el reporte total de movimientos del módulo de finca para control administrativo. Requiere autenticación."
)]
pub async fn export_all_movements_csv(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<String>,
    Query(filter): Query<MovementFilter>
) -> impl IntoResponse {
    let start = parse_flex_date(filter.start_date);
    let end = parse_flex_date(filter.end_date);

    let movements = sqlx
        ::query_as::<_, MovimientoStock>(
            "SELECT m.* FROM movimientos_stock m JOIN inventario_items i ON m.item_id = i.id 
         WHERE i.modulo = 'FINCA' AND (m.fecha >= $1 OR $1 IS NULL) AND (m.fecha <= $2 OR $2 IS NULL) ORDER BY m.fecha DESC"
        )
        .bind(start)
        .bind(end)
        .fetch_all(&pool).await
        .unwrap_or_default();

    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record(&["ID", "ItemID", "Cantidad", "Tipo", "Fecha", "Motivo"]).unwrap();
    for m in movements {
        wtr.write_record(
            &[
                m.id.to_string(),
                m.item_id.to_string(),
                m.cantidad.to_string(),
                format!("{:?}", m.tipo),
                m.fecha.to_rfc3339(),
                m.motivo,
            ]
        ).unwrap();
    }
    let csv_data = wtr.into_inner().unwrap();

    enviar_auditoria(
        user_id,
        "FINCA_EXPORT_CSV".to_string(),
        "/inventario/finca/movimientos/exportar".to_string(),
        "N/A".to_string()
    );

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/csv"),
            (header::CONTENT_DISPOSITION, "attachment; filename=\"movimientos_finca.csv\""),
        ],
        csv_data,
    )
}
