use dotenvy::dotenv;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod handlers;
mod middleware;
mod models;
mod routes;
mod utils;

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

    // Crear el router
    let app = routes::create_router(pool);

    // Iniciar el servidor
    let addr = SocketAddr::from(([0, 0, 0, 0], 3001));
    tracing::debug!("Escuchando en {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
