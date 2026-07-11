use dotenvy::dotenv;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Los módulos viven ahora en la librería del crate (src/lib.rs) para poder
// ser reutilizados por los tests de integración. El binario los consume desde allí.
use inventory_service::routes;

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

    // Ejecutar migraciones de base de datos automáticamente al inicio
    tracing::info!("Ejecutando migraciones de base de datos...");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Fallo al ejecutar las migraciones de base de datos");
    tracing::info!("Migraciones completadas exitosamente.");

    // HU025: tarea programada que marca como CADUCADO los ítems cuya fecha de
    // caducidad ya venció. Corre al arranque y luego cada hora.
    let caducidad_pool = pool.clone();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            ticker.tick().await;
            // Registrar en la bitácora antes de sobrescribir el estado.
            let _ = sqlx::query(
                "INSERT INTO historial_estados (item_id, estado_anterior, estado_nuevo, motivo, usuario_id)
                 SELECT id, estado, 'CADUCADO', 'Caducidad automática', 'SYSTEM'
                 FROM inventario_items
                 WHERE fecha_caducidad < NOW() AND estado <> 'CADUCADO' AND is_deleted = false",
            )
            .execute(&caducidad_pool)
            .await;

            match sqlx::query(
                "UPDATE inventario_items SET estado = 'CADUCADO', updated_at = NOW()
                 WHERE fecha_caducidad < NOW() AND estado <> 'CADUCADO' AND is_deleted = false",
            )
            .execute(&caducidad_pool)
            .await
            {
                Ok(res) if res.rows_affected() > 0 => {
                    tracing::info!("Job CADUCADO: {} ítem(s) marcados como caducados.", res.rows_affected());
                }
                Ok(_) => {}
                Err(e) => tracing::error!("Job CADUCADO falló: {:?}", e),
            }
        }
    });

    // Crear el router
    let app = routes::create_router(pool);

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
