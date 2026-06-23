use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
    Extension,
};
use uuid::Uuid;
use sqlx::PgPool;
use chrono::{DateTime, Utc, TimeZone};
use crate::models::billing::CreateEntradaFacturaDto;
use crate::utils::audit::enviar_auditoria;

fn parse_date(date_str: Option<String>) -> Option<DateTime<Utc>> {
    let s = date_str?.trim().to_string();
    if s.is_empty() { return None; }
    if let Ok(dt) = DateTime::parse_from_rfc3339(&s) { return Some(dt.with_timezone(&Utc)); }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S") {
        return Some(Utc.from_utc_datetime(&naive));
    }
    if let Ok(naive_date) = chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
        let naive = naive_date.and_hms_opt(0, 0, 0).unwrap();
        return Some(Utc.from_utc_datetime(&naive));
    }
    None
}

fn validate_factura(payload: &CreateEntradaFacturaDto) -> Result<(), String> {
    let nf = payload.numero_factura.trim();
    if nf.is_empty() { return Err("El número de factura es obligatorio.".into()); }
    if nf.len() != 17 { return Err("El número de factura debe tener exactamente 17 caracteres.".into()); }
    if !nf.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("El número de factura solo puede contener letras y números.".into());
    }

    if payload.cantidad <= 0.0 { return Err("La cantidad debe ser mayor a 0.".into()); }
    if payload.cantidad > 10000.0 { return Err("La cantidad no puede superar 10000.".into()); }

    let um = payload.unidad_medida.trim();
    if um.is_empty() { return Err("La unidad de medida es obligatoria.".into()); }
    let valid_ums = ["QUINTALES", "ARROBAS", "LIBRAS", "UNIDADES", "LITROS", "KILOGRAMOS"];
    if !valid_ums.contains(&um) {
        return Err("La unidad de medida no es válida. Seleccione una de las opciones del catálogo: QUINTALES, ARROBAS, LIBRAS, UNIDADES, LITROS o KILOGRAMOS.".into());
    }

    let tipo = payload.tipo.trim();
    if tipo.is_empty() { return Err("El tipo de movimiento es obligatorio.".into()); }
    if tipo != "ENTRADA" && tipo != "SALIDA" { return Err("El tipo de movimiento debe ser ENTRADA o SALIDA.".into()); }

    Ok(())
}

#[utoipa::path(
    post,
    path = "/billing/facturas/movimiento",
    tag = "Facturación",
    request_body = CreateEntradaFacturaDto,
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 201, description = "Movimiento registrado correctamente"),
        (status = 400, description = "Errores de validación")
    )
)]
pub async fn registrar_movimiento_factura(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<CreateEntradaFacturaDto>,
) -> Result<impl IntoResponse, StatusCode> {
    if let Err(msg) = validate_factura(&payload) {
        return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": msg }))).into_response());
    }

    let fecha = match payload.fecha_entrada {
        Some(ref f) => {
            if f.trim().is_empty() {
                return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "La fecha de entrada es obligatoria." }))).into_response());
            }
            if let Some(dt) = parse_date(Some(f.clone())) {
                if dt > Utc::now() {
                    return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "La fecha no puede ser futura." }))).into_response());
                }
                
                let first_date = sqlx::query_scalar::<_, DateTime<Utc>>("SELECT MIN(created_at) FROM inventario_items")
                    .fetch_optional(&pool).await.unwrap_or(None);
                
                if let Some(fd) = first_date {
                    if dt < fd {
                        return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "La fecha no puede ser anterior a la fecha del primer producto registrado." }))).into_response());
                    }
                }

                dt
            } else {
                return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "El formato de fecha debe ser AAAA-MM-DD HH:MM:SS." }))).into_response());
            }
        },
        None => Utc::now(),
    };

    let item_caducidad = sqlx::query_scalar::<_, Option<DateTime<Utc>>>("SELECT fecha_caducidad FROM inventario_items WHERE id = $1")
        .bind(payload.item_id).fetch_optional(&pool).await.unwrap_or(None);
    
    if let Some(Some(caducidad)) = item_caducidad {
        if fecha > caducidad {
            return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "La fecha de entrada no puede ser posterior a la fecha de caducidad del producto." }))).into_response());
        }
    }

    let exists_factura = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM movimientos_stock WHERE numero_factura = $1 AND item_id = $2"
    )
    .bind(&payload.numero_factura).bind(payload.item_id)
    .fetch_one(&pool).await.unwrap_or(0);

    if exists_factura > 0 {
        return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "La factura ya fue registrada anteriormente para este producto." }))).into_response());
    }

    let mut tx = pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let is_salida = payload.tipo.trim() == "SALIDA";

    if is_salida {
        let current_stock = sqlx::query_scalar::<_, f64>("SELECT cantidad FROM inventario_items WHERE id = $1")
            .bind(payload.item_id).fetch_optional(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        if let Some(stock) = current_stock {
            if stock < payload.cantidad {
                tx.rollback().await.ok();
                return Ok((StatusCode::BAD_REQUEST, Json(serde_json::json!({ "message": "Stock insuficiente para realizar la salida." }))).into_response());
            }
        } else {
            tx.rollback().await.ok();
            return Ok((StatusCode::NOT_FOUND, Json(serde_json::json!({ "message": "Producto no encontrado." }))).into_response());
        }
    }

    let multiplier = if is_salida { -1.0 } else { 1.0 };
    let delta = payload.cantidad * multiplier;

    let item_updated = sqlx::query!(
        "UPDATE inventario_items SET cantidad = cantidad + $1 WHERE id = $2 RETURNING id",
        delta,
        payload.item_id
    )
    .fetch_optional(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if item_updated.is_none() {
        return Ok((StatusCode::NOT_FOUND, Json(serde_json::json!({ "message": "Producto no encontrado." }))).into_response());
    }

    let id_mov = Uuid::new_v4();
    let tipo = payload.tipo.trim();
    let motivo = if is_salida { "Salida con factura proveedor" } else { "Ingreso con factura proveedor" };

    sqlx::query(
        "INSERT INTO movimientos_stock (id, item_id, cantidad, tipo, fecha, motivo, numero_factura)
         VALUES ($1, $2, $3, $4::tipo_movimiento, $5, $6, $7)"
    )
    .bind(id_mov)
    .bind(payload.item_id)
    .bind(payload.cantidad)
    .bind(tipo)
    .bind(fecha)
    .bind(motivo)
    .bind(payload.numero_factura)
    .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    enviar_auditoria(user_id, format!("BILLING_REGISTRAR_{}", tipo), format!("/billing/facturas/movimiento/{}", id_mov), "N/A".to_string());

    Ok((StatusCode::CREATED, Json(serde_json::json!({ "message": format!("{} registrada correctamente.", if is_salida { "Salida" } else { "Entrada" }), "id": id_mov }))).into_response())
}
