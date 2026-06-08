use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;
use sqlx::PgPool;
use crate::models::{LoteCafe, FaseCafe};

#[utoipa::path(
    get,
    path = "/trazabilidad/lotes",
    tag = "Trazabilidad",
    responses(
        (status = 200, description = "Lista de lotes recuperada exitosamente", body = [LoteCafe]),
        (status = 401, description = "No autorizado: Sesión no válida"),
        (status = 403, description = "Prohibido: Sin permisos de lectura"),
        (status = 500, description = "Error interno al recuperar los lotes de café"),
        (status = 503, description = "Servicio de base de datos no disponible")
    ),
    summary = "Listar Lotes de Café",
    description = "Recupera todos los lotes de café registrados, permitiendo visualizar en qué fase de procesamiento se encuentra cada uno (pulpa, secado, tostado, etc.)."
)]
pub async fn list_lots(
    State(pool): State<PgPool>,
) -> Result<Json<Vec<LoteCafe>>, StatusCode> {
    let lots = sqlx::query_as::<_, LoteCafe>("SELECT * FROM lotes_cafe")
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            tracing::error!("Error al listar lotes: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(lots))
}

#[utoipa::path(
    post,
    path = "/trazabilidad/lotes/{id}/transicion",
    tag = "Trazabilidad",
    params(
        ("id" = Uuid, Path, description = "ID del lote que va a cambiar de fase")
    ),
    request_body = FaseCafe,
    responses(
        (status = 200, description = "El lote ha transicionado exitosamente a la nueva fase", body = LoteCafe),
        (status = 400, description = "Transición inválida o parámetros incorrectos"),
        (status = 401, description = "No autorizado"),
        (status = 404, description = "El lote origen especificado no existe"),
        (status = 500, description = "Error crítico durante la creación del nuevo lote")
    ),
    summary = "Transicionar Fase de Lote",
    description = "Inicia el cambio de un lote de una fase a otra. Este proceso mantiene el vínculo de trazabilidad y actualiza el inventario físico automáticamente."
)]
pub async fn transition_lot_phase(
    Path(id): Path<Uuid>,
    State(pool): State<PgPool>,
    Json(nueva_fase): Json<FaseCafe>,
) -> Result<Json<LoteCafe>, StatusCode> {
    let mut tx = pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let lote_origen = sqlx::query_as::<_, LoteCafe>("SELECT * FROM lotes_cafe WHERE id = $1")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let nuevo_id = Uuid::new_v4();
    let nuevo_lote = sqlx::query_as::<_, LoteCafe>(
        "INSERT INTO lotes_cafe (id, variedad, fase, cantidad_producida, costo_produccion, unidad_medida, calidad, codigo_trazabilidad, lote_anterior_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING *"
    )
    .bind(nuevo_id)
    .bind(&lote_origen.variedad)
    .bind(&nueva_fase)
    .bind(lote_origen.cantidad_producida)
    .bind(lote_origen.costo_produccion)
    .bind(&lote_origen.unidad_medida)
    .bind(&lote_origen.calidad)
    .bind(lote_origen.codigo_trazabilidad)
    .bind(Some(lote_origen.id))
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("Error al crear nuevo lote: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(nuevo_lote))
}

#[utoipa::path(
    get,
    path = "/trazabilidad/historial/{codigo_trazabilidad}",
    tag = "Trazabilidad",
    params(
        ("codigo_trazabilidad" = Uuid, Path, description = "Código único de trazabilidad del lote original")
    ),
    responses(
        (status = 200, description = "Cadena completa de trazabilidad recuperada", body = [LoteCafe]),
        (status = 400, description = "Formato de código de trazabilidad inválido"),
        (status = 401, description = "No autorizado"),
        (status = 404, description = "No se encontró historial para el código proporcionado"),
        (status = 500, description = "Error interno al reconstruir el historial")
    ),
    summary = "Consultar Historial de Trazabilidad",
    description = "Obtiene toda la historia de un lote de café, mostrando todas las fases por las que ha pasado desde su origen."
)]
pub async fn get_traceability_history(
    Path(codigo_trazabilidad): Path<Uuid>,
    State(pool): State<PgPool>,
) -> Result<Json<Vec<LoteCafe>>, StatusCode> {
    let history = sqlx::query_as::<_, LoteCafe>(
        "SELECT * FROM lotes_cafe WHERE codigo_trazabilidad = $1 ORDER BY fecha_creacion ASC"
    )
    .bind(codigo_trazabilidad)
    .fetch_all(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if history.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(Json(history))
}
