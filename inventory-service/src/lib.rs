//! Punto de entrada de la librería del servicio de inventario.
//!
//! Expone los módulos internos para que puedan ser consumidos tanto por el
//! binario (`src/main.rs`) como por los tests de integración de `tests/`
//! (p. ej. `tests/movement_paths_test.rs`), que necesitan invocar handlers
//! como `handlers::finca_inventory_handler::create_movement` directamente.

pub mod handlers;
pub mod middleware;
pub mod models;
pub mod routes;
pub mod utils;
