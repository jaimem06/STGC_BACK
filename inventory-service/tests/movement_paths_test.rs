//! Cobertura de caminos lógicos (path coverage) de `create_movement`.
//!
//! Complejidad Ciclomática objetivo: V(G) = 9  →  9 caminos independientes (F1..F9).
//!
//! El handler está acoplado a `sqlx::PgPool` y usa transacciones
//! concretas (`pool.begin()`, `&mut *tx`, `tx.commit()`). `sqlx` NO ofrece mocks
//! de transacciones ni hay una abstracción por trait que permita inyectar dobles.
//! Por eso los caminos de fallo de base de datos (F2, F5, F6, F7) se fuerzan de
//! forma DETERMINISTA sobre una base real y efímera provista por `#[sqlx::test]`:
//!
//!   * F2  -> se cierra el pool antes de invocar   => falla `pool.begin()`.
//!   * F5  -> trigger BEFORE UPDATE que lanza error => falla el UPDATE.
//!   * F6  -> trigger BEFORE INSERT que lanza error => falla el INSERT.
//!   * F7  -> CONSTRAINT TRIGGER DEFERRABLE INITIALLY DEFERRED que lanza error
//!            => UPDATE e INSERT terminan bien, pero falla `tx.commit()`.
//!
//! `#[sqlx::test]` crea una base de datos aislada por test, aplica las migraciones
//! de `./migrations` y entrega un `PgPool` limpio. Requiere `DATABASE_URL` apuntando
//! a un Postgres donde se puedan CREAR/DESTRUIR bases (ver README de ejecución).
//!
//! Cada test invoca el handler DIRECTAMENTE (construyendo los extractores de Axum),
//! lo que produce cobertura exacta de las ramas sin levantar un servidor HTTP.

use axum::extract::State;
use axum::http::StatusCode;
use axum::{Extension, Json};
use sqlx::PgPool;
use uuid::Uuid;

use inventory_service::handlers::finca_inventory_handler::create_movement;
use inventory_service::models::enums::TipoMovimiento;
use inventory_service::models::CreateMovimientoDto;

const USUARIO: &str = "11111111-1111-1111-1111-111111111111";

/// Inserta un ítem FINCA con la `cantidad` indicada y devuelve su id.
/// Se escribe SQL directo (no pasa por validaciones del handler de creación).
async fn seed_item(pool: &PgPool, cantidad: f64) -> Uuid {
    let id = Uuid::new_v4();
    let sku = format!("F{}", &id.simple().to_string()[..5]).to_uppercase();
    sqlx::query(
        "INSERT INTO inventario_items
            (id, sku, nombre, cantidad, tipo, estado, unidad_medida, precio, modulo, stock_minimo)
         VALUES ($1, $2, 'Item de Prueba', $3,
                 'INSUMO'::tipo_elemento,
                 'DISPONIBLE'::estado_producto,
                 'LIBRAS'::unidad_medida,
                 100.0,
                 'FINCA'::modulo_inventario,
                 5.0)",
    )
    .bind(id)
    .bind(sku)
    .bind(cantidad)
    .execute(pool)
    .await
    .expect("no se pudo sembrar el ítem de prueba");
    id
}

/// Construye el DTO de movimiento con valores por defecto razonables.
fn dto(item_id: Uuid, cantidad: f64, tipo: TipoMovimiento) -> CreateMovimientoDto {
    CreateMovimientoDto {
        item_id,
        cantidad,
        tipo,
        motivo: "Prueba de camino".to_string(),
        lote_id: None,
    }
}

// ---------------------------------------------------------------------------
// F1: Payload con cantidad <= 0.0  ->  BAD_REQUEST
//     Camino de guarda inicial; retorna antes de tocar la base de datos.
// ---------------------------------------------------------------------------
#[sqlx::test]
async fn f1_cantidad_no_positiva_devuelve_bad_request(pool: PgPool) {
    let payload = dto(Uuid::new_v4(), 0.0, TipoMovimiento::ENTRADA);

    let res = create_movement(State(pool), Extension(USUARIO.to_string()), Json(payload)).await;

    assert_eq!(res.err(), Some(StatusCode::BAD_REQUEST));
}

// ---------------------------------------------------------------------------
// F2: Fallo simulado en pool.begin().await  ->  INTERNAL_SERVER_ERROR
//     Cerramos el pool: la primera operación (begin) falla con PoolClosed.
// ---------------------------------------------------------------------------
#[sqlx::test]
async fn f2_fallo_en_begin_devuelve_500(pool: PgPool) {
    // Forzamos que `pool.begin()` falle cerrando el pool antes de invocar.
    pool.close().await;

    let payload = dto(Uuid::new_v4(), 5.0, TipoMovimiento::ENTRADA);

    let res = create_movement(State(pool), Extension(USUARIO.to_string()), Json(payload)).await;

    assert_eq!(res.err(), Some(StatusCode::INTERNAL_SERVER_ERROR));
}

// ---------------------------------------------------------------------------
// F3: item_id inexistente en el SELECT ... FOR UPDATE  ->  NOT_FOUND
//     begin() ok, pero fetch_one no encuentra la fila (RowNotFound).
// ---------------------------------------------------------------------------
#[sqlx::test]
async fn f3_item_inexistente_devuelve_not_found(pool: PgPool) {
    let payload = dto(Uuid::new_v4(), 5.0, TipoMovimiento::ENTRADA);

    let res = create_movement(State(pool), Extension(USUARIO.to_string()), Json(payload)).await;

    assert_eq!(res.err(), Some(StatusCode::NOT_FOUND));
}

// ---------------------------------------------------------------------------
// F4: SALIDA con cantidad mayor al stock actual  ->  BAD_REQUEST
//     Rama de negocio dentro de la transacción (item.cantidad < payload.cantidad).
// ---------------------------------------------------------------------------
#[sqlx::test]
async fn f4_salida_supera_stock_devuelve_bad_request(pool: PgPool) {
    let item_id = seed_item(&pool, 10.0).await;
    let payload = dto(item_id, 999.0, TipoMovimiento::SALIDA);

    let res = create_movement(
        State(pool.clone()),
        Extension(USUARIO.to_string()),
        Json(payload),
    )
    .await;

    assert_eq!(res.err(), Some(StatusCode::BAD_REQUEST));

    // El stock NO debe haber cambiado (la transacción se descarta).
    let cantidad: f64 = sqlx::query_scalar("SELECT cantidad FROM inventario_items WHERE id = $1")
        .bind(item_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cantidad, 10.0);
}

// ---------------------------------------------------------------------------
// F5: ENTRADA, error forzado en el UPDATE inventario_items  ->  INTERNAL_SERVER_ERROR
//     Un trigger BEFORE UPDATE lanza una excepción cuando el handler intenta
//     actualizar el stock. El SELECT FOR UPDATE previo sí tiene éxito.
// ---------------------------------------------------------------------------
#[sqlx::test]
async fn f5_error_en_update_devuelve_500(pool: PgPool) {
    let item_id = seed_item(&pool, 10.0).await;

    sqlx::raw_sql(
        "CREATE OR REPLACE FUNCTION forzar_error_update() RETURNS trigger AS $$
         BEGIN
             RAISE EXCEPTION 'F5: fallo forzado en UPDATE inventario_items';
         END;
         $$ LANGUAGE plpgsql;

         CREATE TRIGGER trg_forzar_error_update
             BEFORE UPDATE ON inventario_items
             FOR EACH ROW EXECUTE FUNCTION forzar_error_update();",
    )
    .execute(&pool)
    .await
    .expect("no se pudo crear el trigger de fallo de UPDATE");

    let payload = dto(item_id, 5.0, TipoMovimiento::ENTRADA);

    let res = create_movement(
        State(pool.clone()),
        Extension(USUARIO.to_string()),
        Json(payload),
    )
    .await;

    assert_eq!(res.err(), Some(StatusCode::INTERNAL_SERVER_ERROR));
}

// ---------------------------------------------------------------------------
// F6: ENTRADA, error forzado en el INSERT movimientos_stock  ->  INTERNAL_SERVER_ERROR
//     Un trigger BEFORE INSERT sobre movimientos_stock lanza excepción.
//     El UPDATE de inventario_items sí se ejecuta con éxito antes del fallo.
// ---------------------------------------------------------------------------
#[sqlx::test]
async fn f6_error_en_insert_devuelve_500(pool: PgPool) {
    let item_id = seed_item(&pool, 10.0).await;

    sqlx::raw_sql(
        "CREATE OR REPLACE FUNCTION forzar_error_insert() RETURNS trigger AS $$
         BEGIN
             RAISE EXCEPTION 'F6: fallo forzado en INSERT movimientos_stock';
         END;
         $$ LANGUAGE plpgsql;

         CREATE TRIGGER trg_forzar_error_insert
             BEFORE INSERT ON movimientos_stock
             FOR EACH ROW EXECUTE FUNCTION forzar_error_insert();",
    )
    .execute(&pool)
    .await
    .expect("no se pudo crear el trigger de fallo de INSERT");

    let payload = dto(item_id, 5.0, TipoMovimiento::ENTRADA);

    let res = create_movement(
        State(pool.clone()),
        Extension(USUARIO.to_string()),
        Json(payload),
    )
    .await;

    assert_eq!(res.err(), Some(StatusCode::INTERNAL_SERVER_ERROR));

    // El UPDATE quedó revertido junto con la transacción abortada.
    let cantidad: f64 = sqlx::query_scalar("SELECT cantidad FROM inventario_items WHERE id = $1")
        .bind(item_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cantidad, 10.0);
}

// ---------------------------------------------------------------------------
// F7: ENTRADA, error forzado en tx.commit().await  ->  INTERNAL_SERVER_ERROR
//     UPDATE e INSERT tienen éxito dentro de la transacción; un CONSTRAINT
//     TRIGGER DEFERRABLE INITIALLY DEFERRED se evalúa recién en el COMMIT y
//     lanza la excepción, haciendo fallar el commit.
// ---------------------------------------------------------------------------
#[sqlx::test]
async fn f7_error_en_commit_devuelve_500(pool: PgPool) {
    let item_id = seed_item(&pool, 10.0).await;

    sqlx::raw_sql(
        "CREATE OR REPLACE FUNCTION forzar_error_commit() RETURNS trigger AS $$
         BEGIN
             RAISE EXCEPTION 'F7: fallo forzado en COMMIT';
         END;
         $$ LANGUAGE plpgsql;

         CREATE CONSTRAINT TRIGGER trg_forzar_error_commit
             AFTER INSERT ON movimientos_stock
             DEFERRABLE INITIALLY DEFERRED
             FOR EACH ROW EXECUTE FUNCTION forzar_error_commit();",
    )
    .execute(&pool)
    .await
    .expect("no se pudo crear el constraint trigger diferido");

    let payload = dto(item_id, 5.0, TipoMovimiento::ENTRADA);

    let res = create_movement(
        State(pool.clone()),
        Extension(USUARIO.to_string()),
        Json(payload),
    )
    .await;

    assert_eq!(res.err(), Some(StatusCode::INTERNAL_SERVER_ERROR));

    // Al fallar el commit, nada se persiste.
    let cantidad: f64 = sqlx::query_scalar("SELECT cantidad FROM inventario_items WHERE id = $1")
        .bind(item_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cantidad, 10.0);
    let movimientos: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM movimientos_stock WHERE item_id = $1")
            .bind(item_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(movimientos, 0);
}

// ---------------------------------------------------------------------------
// F8: ENTRADA con datos correctos  ->  CREATED + persistencia en BD
// ---------------------------------------------------------------------------
#[sqlx::test]
async fn f8_entrada_correcta_devuelve_created(pool: PgPool) {
    let item_id = seed_item(&pool, 10.0).await;
    let payload = dto(item_id, 5.0, TipoMovimiento::ENTRADA);

    let res = create_movement(
        State(pool.clone()),
        Extension(USUARIO.to_string()),
        Json(payload),
    )
    .await;

    match res {
        Ok((status, Json(mov))) => {
            assert_eq!(status, StatusCode::CREATED);
            assert_eq!(mov.item_id, item_id);
            assert_eq!(mov.cantidad, 5.0);
            assert_eq!(mov.tipo, TipoMovimiento::ENTRADA);
        }
        Err(e) => panic!("Se esperaba CREATED, se obtuvo el error {:?}", e),
    }

    // Stock recalculado: 10 + 5 = 15.
    let cantidad: f64 = sqlx::query_scalar("SELECT cantidad FROM inventario_items WHERE id = $1")
        .bind(item_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cantidad, 15.0);

    // Se registró exactamente un movimiento.
    let movimientos: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM movimientos_stock WHERE item_id = $1")
            .bind(item_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(movimientos, 1);
}

// ---------------------------------------------------------------------------
// F9: SALIDA con stock suficiente y datos correctos  ->  CREATED + persistencia
// ---------------------------------------------------------------------------
#[sqlx::test]
async fn f9_salida_correcta_devuelve_created(pool: PgPool) {
    let item_id = seed_item(&pool, 20.0).await;
    let payload = dto(item_id, 8.0, TipoMovimiento::SALIDA);

    let res = create_movement(
        State(pool.clone()),
        Extension(USUARIO.to_string()),
        Json(payload),
    )
    .await;

    match res {
        Ok((status, Json(mov))) => {
            assert_eq!(status, StatusCode::CREATED);
            assert_eq!(mov.item_id, item_id);
            assert_eq!(mov.cantidad, 8.0);
            assert_eq!(mov.tipo, TipoMovimiento::SALIDA);
        }
        Err(e) => panic!("Se esperaba CREATED, se obtuvo el error {:?}", e),
    }

    // Stock recalculado: 20 - 8 = 12.
    let cantidad: f64 = sqlx::query_scalar("SELECT cantidad FROM inventario_items WHERE id = $1")
        .bind(item_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cantidad, 12.0);

    let movimientos: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM movimientos_stock WHERE item_id = $1")
            .bind(item_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(movimientos, 1);
}
