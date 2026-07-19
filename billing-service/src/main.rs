use dotenvy::dotenv;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod handlers;
mod middleware;
mod models;
mod routes;
mod services;
mod utils;

use models::billing::BusinessInfo;

/// Crea, de forma idempotente, el esquema `billing_service` con el estado final
/// esperado por los handlers: enum de estados, secuencia de numeración y la tabla
/// de comprobantes con sus restricciones. Es seguro ejecutarlo en cada arranque.
async fn ensure_billing_schema(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    const BOOTSTRAP_SQL: &str = r#"
CREATE SCHEMA IF NOT EXISTS billing_service;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_type t
        JOIN pg_namespace n ON n.oid = t.typnamespace
        WHERE n.nspname = 'billing_service' AND t.typname = 'estado_factura'
    ) THEN
        CREATE TYPE billing_service.estado_factura AS ENUM (
            'BORRADOR', 'PENDIENTE', 'PAGADA', 'ANULADA', 'REEMBOLSADA'
        );
    END IF;
END
$$;

CREATE SEQUENCE IF NOT EXISTS billing_service.comprobante_numero_seq
    AS BIGINT START WITH 1 INCREMENT BY 1 MINVALUE 1;

CREATE TABLE IF NOT EXISTS billing_service.comprobantes (
    id                     UUID PRIMARY KEY,
    pedido_id              TEXT   NOT NULL,
    numero                 BIGINT NOT NULL DEFAULT nextval('billing_service.comprobante_numero_seq'),
    estado                 billing_service.estado_factura NOT NULL DEFAULT 'PAGADA',
    datos                  JSONB,
    requiere_rehidratacion BOOLEAN NOT NULL DEFAULT FALSE,
    creado                 TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT comprobantes_pedido_id_key    UNIQUE (pedido_id),
    CONSTRAINT comprobantes_numero_key       UNIQUE (numero),
    CONSTRAINT comprobantes_numero_positivo  CHECK (numero > 0),
    CONSTRAINT comprobantes_datos_presentes  CHECK (datos IS NOT NULL OR requiere_rehidratacion)
);

ALTER SEQUENCE billing_service.comprobante_numero_seq
    OWNED BY billing_service.comprobantes.numero;
"#;

    sqlx::raw_sql(BOOTSTRAP_SQL).execute(pool).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Cargar variables de entorno
    dotenv().ok();

    // Configurar trazado (logging)
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            "inventory_service=debug,tower_http=debug,axum::rejection=trace".into()
        }))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Configurar conexión a la base de datos
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL debe estar configurada");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("No se pudo conectar a la base de datos");

    // Las migraciones del esquema de inventario las administra el inventory-service.
    // El esquema propio de facturación (`billing_service`) vive en la misma base
    // compartida con el POS y aquí garantizamos, de forma idempotente, que exista
    // antes de aceptar tráfico. Sin esto, `emitir_comprobante` fallaba con 500 al
    // insertar en una tabla inexistente.
    if let Err(error) = ensure_billing_schema(&pool).await {
        panic!("No se pudo preparar el esquema billing_service: {error}");
    }

    // Si falta configuración fiscal se bloquea únicamente la nueva emisión (422);
    // las facturas de inventario existentes pueden seguir operando.
    let business_info = BusinessInfo::from_env().unwrap_or_else(|error| {
        tracing::warn!(%error, "Emisión HU12-A deshabilitada hasta configurar los datos fiscales");
        BusinessInfo {
            nombre: String::new(),
            ruc: String::new(),
            direccion: String::new(),
            establecimiento: "001".into(),
            punto_emision: "001".into(),
        }
    });

    // Crear el router
    let app = routes::create_router(pool, business_info);

    // Configurar CORS
    let origins = env::var("ALLOWED_ORIGIN")
        .unwrap_or_else(|_| "http://localhost:3001,http://localhost:3000,https://stgc-web.onrender.com,https://pos-service-oo47.onrender.com".to_string());
    
    let mut cors = tower_http::cors::CorsLayer::new();
    
    if origins == "*" {
        cors = cors.allow_origin(tower_http::cors::Any);
    } else {
        let allowed_origins: Vec<axum::http::HeaderValue> = origins
            .split(',')
            .map(|s| s.trim().parse().unwrap())
            .collect();
        cors = cors.allow_origin(allowed_origins);
    }

    let cors = cors
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::PATCH,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
        ]);

    let app = app.layer(cors);

    // Iniciar el servidor
    let port = env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3001);
    
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Servidor escuchando en http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
