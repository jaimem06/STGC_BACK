use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Extension, Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{types::Json as SqlJson, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::models::billing::{
    BillingErrorResponse, BusinessInfo, CambiarEstadoFacturaDto, ComprobanteResumen,
    ComprobantesListResponse, EmitReceiptResponse, InvoiceStatus, InvoiceStatusCatalogResponse,
    PersistedInvoiceData, ReceiptItem, ReceiptOrder, ReceiptPayment,
};
use crate::services::receipt_service::{
    format_receipt_number, generate_pdf, validate_receipt, ReceiptValidationError, SUCCESS_MESSAGE,
};

fn error_response(status: StatusCode, message: impl Into<String>) -> Response {
    (
        status,
        Json(BillingErrorResponse {
            message: message.into(),
        }),
    )
        .into_response()
}

fn internal_error(context: &str, error: impl std::fmt::Display) -> Response {
    tracing::error!(%error, context, "Error al procesar comprobante");
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "No se pudo procesar el comprobante.",
    )
}

fn success_response(
    business: &BusinessInfo,
    pedido_id: &str,
    number: i64,
    invoice_status: InvoiceStatus,
    created: bool,
) -> Response {
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    (
        status,
        Json(EmitReceiptResponse {
            message: SUCCESS_MESSAGE.into(),
            numero_comprobante: format_receipt_number(business, number),
            estado_factura: invoice_status,
            pdf_url: format!("/api/billing/comprobantes/{pedido_id}/pdf"),
            creado: created,
        }),
    )
        .into_response()
}

#[derive(Debug)]
enum InvoiceDataError {
    NotFound,
    Validation(ReceiptValidationError),
    Database(sqlx::Error),
}

impl From<sqlx::Error> for InvoiceDataError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

fn invoice_data_error_response(error: InvoiceDataError) -> Response {
    match error {
        InvoiceDataError::NotFound => {
            error_response(StatusCode::NOT_FOUND, "Pedido no encontrado.")
        }
        InvoiceDataError::Validation(ReceiptValidationError::UnpaidOrder) => error_response(
            StatusCode::CONFLICT,
            crate::services::receipt_service::UNPAID_ORDER_MESSAGE,
        ),
        InvoiceDataError::Validation(error) => {
            tracing::warn!(motivo = %error, "Comprobante rechazado por validación (422)");
            error_response(StatusCode::UNPROCESSABLE_ENTITY, error.to_string())
        }
        InvoiceDataError::Database(error) => internal_error("lectura de datos del POS", error),
    }
}

/// Identificación genérica del consumidor final según el SRI (13 nueves).
const FINAL_CONSUMER_ID: &str = "9999999999999";

/// Venta a consumidor final: sin cédula/RUC real (vacía o solo nueves) o sin
/// nombre del cliente no procede una factura con datos, así que el comprobante
/// se emite a nombre de "Consumidor Final" con la identificación genérica del
/// SRI en lugar de rechazarse con 422.
fn normalize_final_consumer(
    nombre: Option<String>,
    apellido: Option<String>,
    cedula: Option<String>,
) -> (Option<String>, Option<String>, Option<String>) {
    let cedula_texto = cedula.as_deref().unwrap_or("").trim().to_owned();
    let sin_cedula_real = cedula_texto.is_empty() || cedula_texto.chars().all(|c| c == '9');
    let sin_nombre = nombre.as_deref().unwrap_or("").trim().is_empty();

    if sin_cedula_real || sin_nombre {
        (
            Some("Consumidor".to_owned()),
            Some("Final".to_owned()),
            Some(FINAL_CONSUMER_ID.to_owned()),
        )
    } else {
        (nombre, apellido, cedula)
    }
}

/// El contrato POS publica un único campo `cliente_nombre`. Para cumplir CA1 sin
/// exigir una columna inexistente, se interpreta la última palabra como apellido.
/// Versiones ampliadas que sí tienen `cliente_apellido` se combinan en la consulta.
fn split_customer_full_name(full_name: Option<String>) -> (Option<String>, Option<String>) {
    let parts: Vec<&str> = full_name
        .as_deref()
        .unwrap_or("")
        .split_whitespace()
        .collect();

    if parts.len() < 2 {
        return (parts.first().map(|part| (*part).to_owned()), None);
    }

    let surname = parts.last().map(|part| (*part).to_owned());
    let names = Some(parts[..parts.len() - 1].join(" "));
    (names, surname)
}

/// Crea el snapshot persistente desde el esquema real del POS.
///
/// Compatibilidad admitida:
/// - POS oficial: `cliente_nombre`, `cliente_cedula` y `metodoPago` en `Pedido`.
/// - POS ampliado: `cliente_apellido` y tabla `Pago` con pagos múltiples.
///
/// A propósito NO exige que el pedido esté pagado: se usa también para
/// construir facturas BORRADOR/PENDIENTE de pedidos aún en edición (HU de
/// ciclo de vida de estados). Quien necesite exigir un pago confirmado real
/// (generar el PDF) valida por su cuenta con `validate_receipt`.
async fn load_invoice_data(
    tx: &mut Transaction<'_, Postgres>,
    pedido_id: &str,
    business: &BusinessInfo,
) -> Result<PersistedInvoiceData, InvoiceDataError> {
    let row = sqlx::query_as::<
        _,
        (
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<DateTime<Utc>>,
            f64,
            f64,
            f64,
            Option<String>,
        ),
    >(
        r#"SELECT p.id,
                  p.estado::text,
                  NULLIF(BTRIM(p.cliente_nombre), ''),
                  -- Apellido leído por separado; el `to_jsonb` tolera esquemas del POS
                  -- que aún no tengan la columna `cliente_apellido`.
                  NULLIF(BTRIM(to_jsonb(p)->>'cliente_apellido'), ''),
                  p.cliente_cedula,
                  -- El POS almacena `fecha_pago` como `timestamp` sin zona (default de
                  -- Prisma). sqlx exige `timestamptz` para decodificar en DateTime<Utc>,
                  -- así que lo interpretamos explícitamente como UTC.
                  p.fecha_pago AT TIME ZONE 'UTC',
                  p.subtotal,
                  p.iva,
                  p.total,
                  NULLIF(to_jsonb(p)->>'metodoPago', '')
           FROM pos_service."Pedido" AS p
           WHERE p.id = $1
           FOR UPDATE"#,
    )
    .bind(pedido_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(InvoiceDataError::NotFound)?;

    // Si el POS guardó el apellido, se usa tal cual (nombre + apellido separados).
    // Para pedidos antiguos sin apellido, se cae al heurístico de partir el nombre.
    let (customer_name, customer_surname) = match row.3.clone() {
        Some(apellido) => (row.2.clone(), Some(apellido)),
        None => split_customer_full_name(row.2.clone()),
    };
    let (customer_name, customer_surname, customer_id) =
        normalize_final_consumer(customer_name, customer_surname, row.4.clone());
    let pos_payment_method = row.9;
    let mut order = ReceiptOrder {
        pedido_id: row.0,
        estado: row.1,
        cliente_nombre: customer_name,
        cliente_apellido: customer_surname,
        cliente_cedula: customer_id,
        fecha_pago: row.5,
        subtotal: row.6,
        iva: row.7,
        total: row.8,
        items: Vec::new(),
        pagos: Vec::new(),
    };

    order.items = sqlx::query_as::<_, (String, i32, f64, f64)>(
        r#"SELECT nombre, cantidad, "precioUnitario", subtotal
           FROM pos_service."PedidoItem"
           WHERE "pedidoId" = $1
           ORDER BY id"#,
    )
    .bind(pedido_id)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .map(|row| ReceiptItem {
        nombre: row.0,
        cantidad: row.1,
        precio_unitario: row.2,
        subtotal: row.3,
    })
    .collect();

    let payment_table_exists =
        sqlx::query_scalar::<_, bool>(r#"SELECT to_regclass('pos_service."Pago"') IS NOT NULL"#)
            .fetch_one(&mut **tx)
            .await?;

    if payment_table_exists {
        order.pagos = sqlx::query_as::<_, (String, f64)>(
            r#"SELECT "metodoPago"::text, monto
               FROM pos_service."Pago"
               WHERE "pedidoId" = $1
               ORDER BY fecha_pago, id"#,
        )
        .bind(pedido_id)
        .fetch_all(&mut **tx)
        .await?
        .into_iter()
        .map(|row| ReceiptPayment {
            metodo_pago: row.0,
            monto: row.1,
        })
        .collect();
    }

    // El POS oficial admite un solo método y no tiene tabla Pago.
    if order.pagos.is_empty() {
        if let Some(method) = pos_payment_method {
            order.pagos.push(ReceiptPayment {
                metodo_pago: method,
                monto: order.total,
            });
        }
    }

    Ok(PersistedInvoiceData {
        negocio: business.clone(),
        pedido: order,
    })
}

#[utoipa::path(
    get,
    path = "/api/billing/facturas/estados",
    tag = "Emisión de comprobantes",
    responses(
        (status = 200, description = "Catálogo exacto de estados de factura", body = InvoiceStatusCatalogResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn listar_estados_factura() -> Json<InvoiceStatusCatalogResponse> {
    Json(InvoiceStatusCatalogResponse::exact_catalog())
}

#[derive(Debug, Deserialize)]
pub struct ListarComprobantesQuery {
    /// Filtra por nombre, apellido, cédula del cliente o número de comprobante.
    #[serde(default)]
    pub q: Option<String>,
    /// Instante inicial (inclusive, RFC 3339) del rango de fechas a listar.
    /// El POS lo usa para mostrar únicamente las facturas del día en curso;
    /// el corte lo calcula el cliente en su propia zona horaria.
    #[serde(default)]
    pub desde: Option<DateTime<Utc>>,
    /// Instante final (exclusivo, RFC 3339) del rango de fechas a listar.
    #[serde(default)]
    pub hasta: Option<DateTime<Utc>>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/api/billing/comprobantes",
    tag = "Emisión de comprobantes",
    params(
        ("q" = Option<String>, Query, description = "Filtra por nombre, apellido, cédula del cliente o número de comprobante"),
        ("desde" = Option<String>, Query, description = "Instante inicial inclusive (RFC 3339) del rango de fechas"),
        ("hasta" = Option<String>, Query, description = "Instante final exclusivo (RFC 3339) del rango de fechas"),
        ("limit" = Option<i64>, Query, description = "Máximo de resultados (por defecto 20, máx 100)"),
        ("offset" = Option<i64>, Query, description = "Desplazamiento para paginar")
    ),
    responses(
        (status = 200, description = "Historial de facturas emitidas por el cajero autenticado", body = ComprobantesListResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn listar_mis_comprobantes(
    Extension(cajero_id): Extension<String>,
    Extension(business): Extension<BusinessInfo>,
    Query(params): Query<ListarComprobantesQuery>,
    State(pool): State<PgPool>,
) -> Response {
    let limit = params.limit.unwrap_or(20).clamp(1, 100);
    let offset = params.offset.unwrap_or(0).max(0);
    let filtro = params.q.unwrap_or_default().trim().to_string();
    let filtro_like = format!("%{filtro}%");

    // El historial es por cajero (HU: "las facturaciones que ha hecho ese
    // usuario"), así que se filtra por el cajero_id real del pedido en el
    // POS, no por lo persistido en `datos` (que puede faltar en filas
    // antiguas pendientes de rehidratación).
    //
    // El rango de fechas se aplica sobre la fecha de pago y, si la factura aún
    // no se ha cobrado (BORRADOR/PENDIENTE), sobre la fecha de creación del
    // pedido; así el POS puede mostrar solo el trabajo del día en curso.
    // Ambas columnas son `timestamp` sin zona (default de Prisma) con valores
    // en UTC, de ahí el `AT TIME ZONE 'UTC'` antes de comparar instantes.
    let filter_clause = r#"p.cajero_id = $1
             AND ($2 = '' OR p.cliente_nombre ILIKE $3 OR to_jsonb(p)->>'cliente_apellido' ILIKE $3
                  OR p.cliente_cedula ILIKE $3 OR c.numero::text ILIKE $3)
             AND ($4::timestamptz IS NULL
                  OR (COALESCE(p.fecha_pago, p."fechaCreacion") AT TIME ZONE 'UTC') >= $4::timestamptz)
             AND ($5::timestamptz IS NULL
                  OR (COALESCE(p.fecha_pago, p."fechaCreacion") AT TIME ZONE 'UTC') < $5::timestamptz)"#;

    let listado_sql = format!(
        r#"SELECT c.numero, c.pedido_id, c.estado::text, c.datos, c.motivo_estado, c.actualizado
           FROM billing_service.comprobantes c
           JOIN pos_service."Pedido" p ON p.id = c.pedido_id
           WHERE {filter_clause}
           ORDER BY c.numero DESC
           LIMIT $6 OFFSET $7"#
    );
    let rows = sqlx::query_as::<
        _,
        (
            i64,
            String,
            String,
            Option<SqlJson<PersistedInvoiceData>>,
            Option<String>,
            Option<DateTime<Utc>>,
        ),
    >(&listado_sql)
    .bind(&cajero_id)
    .bind(&filtro)
    .bind(&filtro_like)
    .bind(params.desde)
    .bind(params.hasta)
    .bind(limit)
    .bind(offset)
    .fetch_all(&pool)
    .await;

    let rows = match rows {
        Ok(rows) => rows,
        Err(error) => return internal_error("listado de comprobantes", error),
    };

    let total_sql = format!(
        r#"SELECT COUNT(*)
           FROM billing_service.comprobantes c
           JOIN pos_service."Pedido" p ON p.id = c.pedido_id
           WHERE {filter_clause}"#
    );
    let total = sqlx::query_scalar::<_, i64>(&total_sql)
        .bind(&cajero_id)
        .bind(&filtro)
        .bind(&filtro_like)
        .bind(params.desde)
        .bind(params.hasta)
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

    let comprobantes: Vec<ComprobanteResumen> = rows
        .into_iter()
        .map(|(numero, pedido_id, estado_texto, datos, motivo_estado, actualizado)| {
            let estado_factura = estado_texto.parse::<InvoiceStatus>().unwrap_or(InvoiceStatus::Pagada);
            let (cliente_nombre, cliente_apellido, cliente_cedula, fecha_pago, total) = match datos {
                Some(SqlJson(datos)) => (
                    datos.pedido.cliente_nombre,
                    datos.pedido.cliente_apellido,
                    datos.pedido.cliente_cedula,
                    datos.pedido.fecha_pago,
                    Some(datos.pedido.total),
                ),
                None => (None, None, None, None, None),
            };
            ComprobanteResumen {
                numero_comprobante: format_receipt_number(&business, numero),
                pdf_url: format!("/api/billing/comprobantes/{pedido_id}/pdf"),
                pedido_id,
                estado_factura,
                cliente_nombre,
                cliente_apellido,
                cliente_cedula,
                fecha_pago,
                total,
                motivo_estado,
                actualizado,
            }
        })
        .collect();

    Json(ComprobantesListResponse { comprobantes, total }).into_response()
}

#[derive(Debug, Deserialize)]
pub struct EmitirComprobanteQuery {
    /// Si es `true`, emite formalmente la factura aunque el pedido no esté
    /// pagado (queda PENDIENTE: "emitida pero el pago aún no se ha
    /// registrado"). Si es `false` (por defecto), un pedido sin pagar produce
    /// un BORRADOR (preventa/pedido en mesa, "aún no se ha emitido
    /// formalmente"). Un pedido ya PAGADO siempre resulta en PAGADA sin
    /// importar este parámetro.
    #[serde(default)]
    pub formal: bool,
}

/// Determina a qué estado debe quedar la factura según el estado ACTUAL del
/// pedido en el POS (fuente de verdad), independientemente de si ya existía
/// un comprobante previo.
fn estado_objetivo_desde_pedido(estado_pedido: &str, formal: bool) -> InvoiceStatus {
    match estado_pedido {
        "PAGADO" => InvoiceStatus::Pagada,
        "ANULADO" => InvoiceStatus::Anulada,
        _ if formal => InvoiceStatus::Pendiente,
        _ => InvoiceStatus::Borrador,
    }
}

#[utoipa::path(
    post,
    path = "/api/billing/comprobantes/{pedido_id}/emitir",
    tag = "Emisión de comprobantes",
    params(
        ("pedido_id" = String, Path, description = "Identificador del pedido"),
        ("formal" = Option<bool>, Query, description = "true = emitir formalmente aunque no esté pagado (PENDIENTE); false = borrador (BORRADOR)")
    ),
    responses(
        (status = 201, description = "Datos de factura generados y persistidos; PDF disponible bajo demanda si queda PAGADA", body = EmitReceiptResponse),
        (status = 200, description = "Factura ya existente (o transicionada de estado); se devuelve la numeración vigente", body = EmitReceiptResponse),
        (status = 404, description = "Pedido no encontrado", body = BillingErrorResponse),
        (status = 409, description = "El pedido fue anulado y no tenía factura previa", body = BillingErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn emitir_comprobante(
    Path(pedido_id): Path<String>,
    Query(params): Query<EmitirComprobanteQuery>,
    State(pool): State<PgPool>,
    Extension(business): Extension<BusinessInfo>,
) -> Response {
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(error) => return internal_error("inicio de transacción", error),
    };

    let invoice_data = match load_invoice_data(&mut tx, &pedido_id, &business).await {
        Ok(data) => data,
        Err(error) => return invoice_data_error_response(error),
    };

    let existing = match sqlx::query_as::<
        _,
        (i64, String, Option<SqlJson<PersistedInvoiceData>>),
    >(
        "SELECT numero, estado::text, datos FROM billing_service.comprobantes WHERE pedido_id = $1 FOR UPDATE",
    )
    .bind(&pedido_id)
    .fetch_optional(&mut *tx)
    .await
    {
        Ok(existing) => existing,
        Err(error) => return internal_error("consulta de factura existente", error),
    };

    // Un pedido anulado sin factura previa no tiene nada que facturar; si ya
    // existía una factura, sí se le permite avanzar a ANULADA más abajo (para
    // mantenerla en sincronía con el pedido) en vez de rechazar de plano.
    if invoice_data.pedido.estado == "ANULADO" && existing.is_none() {
        if let Err(error) = tx.rollback().await {
            tracing::warn!(%error, "No se pudo revertir transacción tras pedido anulado");
        }
        return error_response(
            StatusCode::CONFLICT,
            "El pedido fue anulado; no hay factura que emitir.",
        );
    }

    let estado_objetivo = estado_objetivo_desde_pedido(&invoice_data.pedido.estado, params.formal);

    if let Some((number, status_name, stored_data)) = existing {
        let estado_actual = match status_name.parse::<InvoiceStatus>() {
            Ok(status) => status,
            Err(error) => return internal_error("lectura del estado de factura", error),
        };

        // Solo se transiciona si el nuevo estado es distinto y la transición
        // es válida (BORRADOR→PENDIENTE→PAGADA→{ANULADA,REEMBOLSADA}); un
        // estado terminal (ANULADA/REEMBOLSADA) nunca se sobrescribe aquí.
        let estado_final = if estado_actual == estado_objetivo {
            estado_actual
        } else if estado_actual.puede_transicionar_a(estado_objetivo) {
            estado_objetivo
        } else {
            estado_actual
        };

        // BORRADOR/PENDIENTE representan una factura que "se está generando":
        // el pedido detrás todavía puede editarse (más productos, cambio de
        // cliente), así que su snapshot se refresca en cada llamada. Una vez
        // PAGADA (o más allá) el snapshot se congela como registro histórico.
        let debe_refrescar_datos = stored_data.is_none()
            || estado_final != estado_actual
            || matches!(estado_final, InvoiceStatus::Borrador | InvoiceStatus::Pendiente);

        if debe_refrescar_datos {
            if let Err(error) = sqlx::query(
                "UPDATE billing_service.comprobantes
                 SET datos = $1, requiere_rehidratacion = FALSE,
                     estado = $2::billing_service.estado_factura, actualizado = NOW()
                 WHERE pedido_id = $3",
            )
            .bind(SqlJson(&invoice_data))
            .bind(estado_final.as_str())
            .bind(&pedido_id)
            .execute(&mut *tx)
            .await
            {
                return internal_error("actualización de datos de factura", error);
            }
        }

        if let Err(error) = tx.commit().await {
            return internal_error("confirmación de factura existente", error);
        }
        return success_response(&business, &pedido_id, number, estado_final, false);
    }

    let number = match sqlx::query_scalar::<_, i64>(
        r#"INSERT INTO billing_service.comprobantes (id, pedido_id, estado, datos)
           VALUES ($1, $2, $3::billing_service.estado_factura, $4)
           RETURNING numero"#,
    )
    .bind(Uuid::new_v4())
    .bind(&pedido_id)
    .bind(estado_objetivo.as_str())
    .bind(SqlJson(&invoice_data))
    .fetch_one(&mut *tx)
    .await
    {
        Ok(number) => number,
        Err(error) => return internal_error("persistencia de datos de factura", error),
    };

    if let Err(error) = tx.commit().await {
        return internal_error("confirmación de emisión", error);
    }

    success_response(&business, &pedido_id, number, estado_objetivo, true)
}

/// Transiciona una factura existente a un estado terminal/administrativo
/// (ANULADA o REEMBOLSADA), dejando `motivo` y `actualizado_por` para
/// auditoría. Reutilizado por `anular_comprobante` y `reembolsar_comprobante`.
async fn cambiar_estado_factura(
    pool: &PgPool,
    user_id: &str,
    pedido_id: &str,
    destino: InvoiceStatus,
    motivo: &str,
) -> Result<(i64, InvoiceStatus), Response> {
    let motivo = motivo.trim();
    if motivo.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Debes indicar el motivo para dejar registro de auditoría.",
        ));
    }

    let existing = sqlx::query_as::<_, (i64, String)>(
        "SELECT numero, estado::text FROM billing_service.comprobantes WHERE pedido_id = $1",
    )
    .bind(pedido_id)
    .fetch_optional(pool)
    .await;

    let (numero, estado_actual_texto) = match existing {
        Ok(Some(row)) => row,
        Ok(None) => {
            return Err(error_response(
                StatusCode::NOT_FOUND,
                "Comprobante no encontrado.",
            ))
        }
        Err(error) => return Err(internal_error("lectura de estado de factura", error)),
    };

    let estado_actual = match estado_actual_texto.parse::<InvoiceStatus>() {
        Ok(estado) => estado,
        Err(error) => return Err(internal_error("lectura del estado de factura", error)),
    };

    if !estado_actual.puede_transicionar_a(destino) {
        return Err(error_response(
            StatusCode::CONFLICT,
            format!(
                "No se puede pasar de {} a {}.",
                estado_actual.as_str(),
                destino.as_str()
            ),
        ));
    }

    if let Err(error) = sqlx::query(
        "UPDATE billing_service.comprobantes
         SET estado = $1::billing_service.estado_factura, motivo_estado = $2,
             actualizado_por = $3, actualizado = NOW()
         WHERE pedido_id = $4",
    )
    .bind(destino.as_str())
    .bind(motivo)
    .bind(user_id)
    .bind(pedido_id)
    .execute(pool)
    .await
    {
        return Err(internal_error("actualización de estado de factura", error));
    }

    Ok((numero, destino))
}

#[utoipa::path(
    post,
    path = "/api/billing/comprobantes/{pedido_id}/anular",
    tag = "Emisión de comprobantes",
    params(("pedido_id" = String, Path, description = "Identificador del pedido")),
    request_body(content = CambiarEstadoFacturaDto, description = "Motivo de la anulación (obligatorio, para auditoría)", content_type = "application/json"),
    responses(
        (status = 200, description = "Factura anulada", body = EmitReceiptResponse),
        (status = 400, description = "Falta el motivo", body = BillingErrorResponse),
        (status = 404, description = "Comprobante no encontrado", body = BillingErrorResponse),
        (status = 409, description = "El estado actual no permite anular", body = BillingErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn anular_comprobante(
    Path(pedido_id): Path<String>,
    Extension(user_id): Extension<String>,
    Extension(business): Extension<BusinessInfo>,
    State(pool): State<PgPool>,
    Json(payload): Json<CambiarEstadoFacturaDto>,
) -> Response {
    match cambiar_estado_factura(&pool, &user_id, &pedido_id, InvoiceStatus::Anulada, &payload.motivo).await {
        Ok((numero, estado)) => success_response(&business, &pedido_id, numero, estado, false),
        Err(response) => response,
    }
}

#[utoipa::path(
    post,
    path = "/api/billing/comprobantes/{pedido_id}/reembolsar",
    tag = "Emisión de comprobantes",
    params(("pedido_id" = String, Path, description = "Identificador del pedido")),
    request_body(content = CambiarEstadoFacturaDto, description = "Motivo del reembolso (obligatorio, para auditoría)", content_type = "application/json"),
    responses(
        (status = 200, description = "Factura marcada como reembolsada", body = EmitReceiptResponse),
        (status = 400, description = "Falta el motivo", body = BillingErrorResponse),
        (status = 404, description = "Comprobante no encontrado", body = BillingErrorResponse),
        (status = 409, description = "Solo una factura PAGADA puede reembolsarse", body = BillingErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn reembolsar_comprobante(
    Path(pedido_id): Path<String>,
    Extension(user_id): Extension<String>,
    Extension(business): Extension<BusinessInfo>,
    State(pool): State<PgPool>,
    Json(payload): Json<CambiarEstadoFacturaDto>,
) -> Response {
    match cambiar_estado_factura(&pool, &user_id, &pedido_id, InvoiceStatus::Reembolsada, &payload.motivo).await {
        Ok((numero, estado)) => success_response(&business, &pedido_id, numero, estado, false),
        Err(response) => response,
    }
}

#[utoipa::path(
    get,
    path = "/api/billing/comprobantes/{pedido_id}/pdf",
    tag = "Emisión de comprobantes",
    params(("pedido_id" = String, Path, description = "Identificador del pedido")),
    responses(
        (status = 200, description = "PDF generado temporalmente y bajo demanda", content_type = "application/pdf"),
        (status = 404, description = "Factura no encontrada", body = BillingErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn descargar_comprobante_pdf(
    Path(pedido_id): Path<String>,
    State(pool): State<PgPool>,
    Extension(business): Extension<BusinessInfo>,
) -> Response {
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(error) => return internal_error("inicio de descarga", error),
    };

    let stored = sqlx::query_as::<_, (i64, Option<SqlJson<PersistedInvoiceData>>)>(
        "SELECT numero, datos FROM billing_service.comprobantes WHERE pedido_id = $1",
    )
    .bind(&pedido_id)
    .fetch_optional(&mut *tx)
    .await;

    let (number, stored_data) = match stored {
        Ok(Some(row)) => row,
        Ok(None) => return error_response(StatusCode::NOT_FOUND, "Comprobante no encontrado."),
        Err(error) => return internal_error("lectura de datos de factura", error),
    };

    let invoice_data = if let Some(SqlJson(data)) = stored_data {
        data
    } else {
        // Compatibilidad única para filas de la versión que almacenaba BYTEA.
        let data = match load_invoice_data(&mut tx, &pedido_id, &business).await {
            Ok(data) => data,
            Err(error) => return invoice_data_error_response(error),
        };
        if let Err(error) = sqlx::query(
            "UPDATE billing_service.comprobantes SET datos = $1, requiere_rehidratacion = FALSE, estado = 'PAGADA'::billing_service.estado_factura WHERE pedido_id = $2",
        )
        .bind(SqlJson(&data))
        .bind(&pedido_id)
        .execute(&mut *tx)
        .await
        {
            return internal_error("rehidratación de factura histórica", error);
        }
        data
    };

    if let Err(error) = tx.commit().await {
        return internal_error("confirmación previa a descarga", error);
    }

    // Una factura BORRADOR/PENDIENTE nunca pasa esta validación (no hay pago
    // real todavía): es un rechazo esperado, no un error interno, así que se
    // responde con el mismo 409/422 que usa el resto del handler.
    if let Err(error) = validate_receipt(&invoice_data.pedido, &invoice_data.negocio) {
        return invoice_data_error_response(InvoiceDataError::Validation(error));
    }

    // El PDF vive únicamente en memoria durante esta solicitud.
    let pdf = match generate_pdf(&invoice_data.pedido, &invoice_data.negocio, number) {
        Ok(pdf) => pdf,
        Err(error) => return internal_error("generación PDF bajo demanda", error),
    };

    // La serie sale del snapshot persistido, coherente con el contenido del PDF.
    let filename = format!(
        "comprobante-{}.pdf",
        format_receipt_number(&invoice_data.negocio, number)
    );
    let disposition = match HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")) {
        Ok(value) => value,
        Err(error) => return internal_error("nombre de archivo", error),
    };
    let mut response = Response::new(Body::from(pdf));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/pdf"),
    );
    response
        .headers_mut()
        .insert(header::CONTENT_DISPOSITION, disposition);
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estado_objetivo_refleja_el_estado_real_del_pedido() {
        // Pagado siempre gana, sin importar `formal`.
        assert_eq!(estado_objetivo_desde_pedido("PAGADO", false), InvoiceStatus::Pagada);
        assert_eq!(estado_objetivo_desde_pedido("PAGADO", true), InvoiceStatus::Pagada);
        // Anulado en el POS -> anulado en la factura.
        assert_eq!(estado_objetivo_desde_pedido("ANULADO", false), InvoiceStatus::Anulada);
        // Sin pagar: borrador por defecto, pendiente si se pide formalmente.
        assert_eq!(estado_objetivo_desde_pedido("EN_EDICION", false), InvoiceStatus::Borrador);
        assert_eq!(estado_objetivo_desde_pedido("EN_EDICION", true), InvoiceStatus::Pendiente);
        assert_eq!(estado_objetivo_desde_pedido("PENDIENTE_PAGO", true), InvoiceStatus::Pendiente);
    }

    #[test]
    fn ca3_success_contract_contains_confirmation_and_pdf_url() {
        let business = BusinessInfo::new(
            "Negocio".into(),
            "1790012345001".into(),
            "Dirección".into(),
            "001".into(),
            "001".into(),
        )
        .unwrap();
        let response = success_response(&business, "PED-1", 7, InvoiceStatus::Pagada, true);
        assert_eq!(response.status(), StatusCode::CREATED);
        let payload = EmitReceiptResponse {
            message: SUCCESS_MESSAGE.into(),
            numero_comprobante: format_receipt_number(&business, 7),
            estado_factura: InvoiceStatus::Pagada,
            pdf_url: "/api/billing/comprobantes/PED-1/pdf".into(),
            creado: true,
        };
        let json = serde_json::to_value(payload).unwrap();
        assert_eq!(json["message"], SUCCESS_MESSAGE);
        assert_eq!(json["estado_factura"], "PAGADA");
        assert_eq!(json["pdf_url"], "/api/billing/comprobantes/PED-1/pdf");
    }

    #[test]
    fn ventas_sin_identificacion_se_emiten_a_consumidor_final() {
        // Sin cédula → consumidor final.
        assert_eq!(
            normalize_final_consumer(Some("Juan".into()), None, None),
            (
                Some("Consumidor".into()),
                Some("Final".into()),
                Some(FINAL_CONSUMER_ID.into())
            )
        );
        // Cédula genérica de nueves (10 o 13) → consumidor final.
        assert_eq!(
            normalize_final_consumer(Some("Consumidor Final".into()), None, Some("9999999999".into())),
            (
                Some("Consumidor".into()),
                Some("Final".into()),
                Some(FINAL_CONSUMER_ID.into())
            )
        );
        // Sin nombre → consumidor final aunque haya cédula.
        assert_eq!(
            normalize_final_consumer(None, None, Some("1712345678".into())),
            (
                Some("Consumidor".into()),
                Some("Final".into()),
                Some(FINAL_CONSUMER_ID.into())
            )
        );
        // Cliente con datos completos → se conservan tal cual.
        assert_eq!(
            normalize_final_consumer(
                Some("Ana".into()),
                Some("Pérez".into()),
                Some("1712345678".into())
            ),
            (
                Some("Ana".into()),
                Some("Pérez".into()),
                Some("1712345678".into())
            )
        );
    }

    #[test]
    fn pos_full_name_is_adapted_without_requiring_cliente_apellido() {
        assert_eq!(
            split_customer_full_name(Some("María Fernanda López".into())),
            (Some("María Fernanda".into()), Some("López".into()))
        );
        assert_eq!(
            split_customer_full_name(Some("Ana".into())),
            (Some("Ana".into()), None)
        );
        assert_eq!(split_customer_full_name(None), (None, None));
    }

    #[test]
    fn invoice_status_catalog_matches_the_project_definition_exactly() {
        let catalog = InvoiceStatusCatalogResponse::exact_catalog();
        let actual: Vec<(&str, &str)> = catalog
            .estados
            .iter()
            .map(|entry| (entry.estado.as_str(), entry.descripcion.as_str()))
            .collect();

        assert_eq!(
            actual,
            vec![
                (
                    "BORRADOR",
                    "La factura se está generando pero aún no se ha emitido formalmente (ideal para preventas o pedidos en mesa).",
                ),
                (
                    "PENDIENTE",
                    "La factura ha sido emitida pero el pago aún no se ha registrado.",
                ),
                (
                    "PAGADA",
                    "El monto total ha sido cubierto satisfactoriamente.",
                ),
                (
                    "ANULADA",
                    "La factura fue cancelada por error de digitación o solicitud del cliente, invalidando el monto pero dejando registro para auditoría.",
                ),
                (
                    "REEMBOLSADA",
                    "El pago se realizó, pero el dinero fue devuelto al cliente y la factura quedó sin efecto.",
                ),
            ]
        );
    }

    #[test]
    fn migrations_persist_data_and_remove_pdf_blobs() {
        // El esquema efectivo lo garantiza ensure_billing_schema (main.rs): sin
        // columnas de PDF persistido y con datos JSONB + rehidratación opcional.
        let initial = include_str!("../../migrations/20260715000000_hu12a_comprobantes.sql");
        let bootstrap = include_str!("../../src/main.rs");

        let initial_normalized = initial.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(initial_normalized.contains("datos JSONB NOT NULL"));
        assert!(!initial_normalized.contains("pdf BYTEA"));
        assert!(!bootstrap.contains("pdf BYTEA"));
        assert!(bootstrap.contains("requiere_rehidratacion BOOLEAN NOT NULL DEFAULT FALSE"));
        assert!(bootstrap.contains("CHECK (datos IS NOT NULL OR requiere_rehidratacion)"));
    }

    #[test]
    fn invoice_status_migration_restricts_the_database_catalog() {
        let migration = include_str!("../../migrations/20260715010000_estados_factura.sql");

        for status in InvoiceStatus::ALL {
            assert!(
                migration.contains(&format!("'{}'", status.as_str())),
                "la migración debe incluir {}",
                status.as_str()
            );
        }
        assert!(migration.contains("ALTER COLUMN estado SET NOT NULL"));
        assert!(migration.contains("WHEN datos IS NOT NULL THEN 'PAGADA'"));
    }

    #[tokio::test]
    #[ignore = "requiere TEST_DATABASE_URL y las migraciones aplicadas en una base exclusiva de pruebas"]
    async fn ca4_database_sequence_is_consecutive_and_constraints_exist() {
        let database_url = std::env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL es obligatorio para la prueba de integración");
        let pool = PgPool::connect(&database_url).await.unwrap();

        let first = sqlx::query_scalar::<_, i64>(
            "SELECT nextval('billing_service.comprobante_numero_seq')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let second = sqlx::query_scalar::<_, i64>(
            "SELECT nextval('billing_service.comprobante_numero_seq')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(second, first + 1);

        let unique_constraints = sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*)
               FROM pg_constraint
               WHERE conrelid = 'billing_service.comprobantes'::regclass
                 AND contype = 'u'"#,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(unique_constraints >= 2);
    }
}
