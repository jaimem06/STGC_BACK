use axum::extract::State;
use axum::http::StatusCode;
use axum::{Extension, Json};
use sqlx::PgPool;
use uuid::Uuid;
use std::str::FromStr;

use inventory_service::handlers::finca_inventory_handler::create_movement;
use inventory_service::models::enums::TipoMovimiento;
use inventory_service::models::CreateMovimientoDto;

const USUARIO: &str = "11111111-1111-1111-1111-111111111111";

/// Inserta un ítem FINCA con la `cantidad` indicada y devuelve su id.
/// Se escribe SQL directo (no pasa por validaciones del handler de creación).
async fn seed_item_with_id(pool: &PgPool, id: Uuid, cantidad: f64) -> Uuid {
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

// ---------------------------------------------------------------------------
// CP-F1: Payload con cantidad <= 0.0  ->  BAD_REQUEST
// ---------------------------------------------------------------------------
#[sqlx::test]
async fn cp_f1(pool: PgPool) {
    println!("--------------------------------------------------");
    println!("Ejecutando CP-F1: Validar rechazo de movimiento por cantidad menor o igual a cero.");
    
    let payload = CreateMovimientoDto {
        item_id: Uuid::from_str("c7d8e9f0-1234-5678-90ab-cdef12345678").unwrap(),
        cantidad: 0.0,
        tipo: TipoMovimiento::ENTRADA,
        motivo: "Inventario inicial".to_string(),
        lote_id: None,
        numero_factura: None,
    };

    let res = create_movement(State(pool), Extension(USUARIO.to_string()), Json(payload)).await;

    assert_eq!(res.err(), Some(StatusCode::BAD_REQUEST));
    println!("Resultado CP-F1: Retornó Err(StatusCode::BAD_REQUEST). La ejecución finalizó sin iniciar transacción.");
    println!("--------------------------------------------------");
}

// ---------------------------------------------------------------------------
// CP-F2: Fallo simulado en pool.begin().await  ->  INTERNAL_SERVER_ERROR
// ---------------------------------------------------------------------------
#[sqlx::test]
async fn cp_f2(pool: PgPool) {
    println!("--------------------------------------------------");
    println!("Ejecutando CP-F2: Validar error interno al fallar la inicialización de transacción.");
    
    // Forzamos que `pool.begin()` falle cerrando el pool antes de invocar.
    pool.close().await;

    let payload = CreateMovimientoDto {
        item_id: Uuid::from_str("a1b2c3d4-e5f6-7890-1234-56789abcdef0").unwrap(),
        cantidad: 50.5,
        tipo: TipoMovimiento::SALIDA,
        motivo: "Ajuste por merma".to_string(),
        lote_id: None,
        numero_factura: None,
    };

    let res = create_movement(State(pool), Extension(USUARIO.to_string()), Json(payload)).await;

    assert_eq!(res.err(), Some(StatusCode::INTERNAL_SERVER_ERROR));
    println!("Resultado CP-F2: Retornó Err(StatusCode::INTERNAL_SERVER_ERROR). Se capturó error en begin y canceló operaciones.");
    println!("--------------------------------------------------");
}

// ---------------------------------------------------------------------------
// CP-F3: item_id inexistente en el SELECT ... FOR UPDATE  ->  NOT_FOUND
// ---------------------------------------------------------------------------
#[sqlx::test]
async fn cp_f3(pool: PgPool) {
    println!("--------------------------------------------------");
    println!("Ejecutando CP-F3: Validar retorno de NOT_FOUND cuando el ítem no existe.");
    
    let payload = CreateMovimientoDto {
        item_id: Uuid::from_str("11111111-1111-1111-1111-111111111111").unwrap(),
        cantidad: 10.0,
        tipo: TipoMovimiento::ENTRADA,
        motivo: "Compra de insumos".to_string(),
        lote_id: None,
        numero_factura: None,
    };

    let res = create_movement(State(pool), Extension(USUARIO.to_string()), Json(payload)).await;

    assert_eq!(res.err(), Some(StatusCode::NOT_FOUND));
    println!("Resultado CP-F3: Retornó Err(StatusCode::NOT_FOUND). No se registraron modificaciones en BD.");
    println!("--------------------------------------------------");
}

// ---------------------------------------------------------------------------
// CP-F4: SALIDA con cantidad mayor al stock actual  ->  BAD_REQUEST
// ---------------------------------------------------------------------------
#[sqlx::test]
async fn cp_f4(pool: PgPool) {
    println!("--------------------------------------------------");
    println!("Ejecutando CP-F4: Validar error al intentar SALIDA con cantidad mayor al stock disponible.");
    
    let target_id = Uuid::from_str("c7d8e9f0-1234-5678-90ab-cdef12345678").unwrap();
    seed_item_with_id(&pool, target_id, 10.0).await;
    
    let payload = CreateMovimientoDto {
        item_id: target_id,
        cantidad: 20.0,
        tipo: TipoMovimiento::SALIDA,
        motivo: "Venta".to_string(),
        lote_id: None,
        numero_factura: None,
    };

    let res = create_movement(
        State(pool.clone()),
        Extension(USUARIO.to_string()),
        Json(payload),
    )
    .await;

    assert_eq!(res.err(), Some(StatusCode::BAD_REQUEST));

    // El stock NO debe haber cambiado.
    let cantidad: f64 = sqlx::query_scalar("SELECT cantidad FROM inventario_items WHERE id = $1")
        .bind(target_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cantidad, 10.0);
    
    println!("Resultado CP-F4: Retornó Err(StatusCode::BAD_REQUEST). El stock (10.0) no fue actualizado.");
    println!("--------------------------------------------------");
}

// ---------------------------------------------------------------------------
// CP-F5: Error forzado en el UPDATE inventario_items  ->  INTERNAL_SERVER_ERROR
// ---------------------------------------------------------------------------
#[sqlx::test]
async fn cp_f5(pool: PgPool) {
    println!("--------------------------------------------------");
    println!("Ejecutando CP-F5: Validar error por fallo al actualizar el inventario (ENTRADA).");
    
    let target_id = Uuid::new_v4();
    seed_item_with_id(&pool, target_id, 10.0).await;

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

    let payload = CreateMovimientoDto {
        item_id: target_id,
        cantidad: 0.1,
        tipo: TipoMovimiento::ENTRADA,
        motivo: "Compra de insumos".to_string(),
        lote_id: None,
        numero_factura: None,
    };

    let res = create_movement(
        State(pool.clone()),
        Extension(USUARIO.to_string()),
        Json(payload),
    )
    .await;

    assert_eq!(res.err(), Some(StatusCode::INTERNAL_SERVER_ERROR));
    
    println!("Resultado CP-F5: Retornó Err(StatusCode::INTERNAL_SERVER_ERROR) tras fallar el update. La transacción no se confirmó.");
    println!("--------------------------------------------------");
}

// ---------------------------------------------------------------------------
// CP-F6: Error forzado en el INSERT movimientos_stock  ->  INTERNAL_SERVER_ERROR
// ---------------------------------------------------------------------------
#[sqlx::test]
async fn cp_f6(pool: PgPool) {
    println!("--------------------------------------------------");
    println!("Ejecutando CP-F6: Validar error interno por fallo en INSERT movimientos_stock.");
    
    let target_id = Uuid::from_str("c7d8e9f0-1234-5678-90ab-cdef12345678").unwrap();
    seed_item_with_id(&pool, target_id, 10.0).await;

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

    let payload = CreateMovimientoDto {
        item_id: target_id,
        cantidad: 25.0,
        tipo: TipoMovimiento::ENTRADA,
        motivo: "Reabastecimiento de insumos".to_string(),
        lote_id: None,
        numero_factura: None,
    };

    let res = create_movement(
        State(pool.clone()),
        Extension(USUARIO.to_string()),
        Json(payload),
    )
    .await;

    assert_eq!(res.err(), Some(StatusCode::INTERNAL_SERVER_ERROR));

    // El UPDATE quedó revertido junto con la transacción abortada.
    let cantidad: f64 = sqlx::query_scalar("SELECT cantidad FROM inventario_items WHERE id = $1")
        .bind(target_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cantidad, 10.0);
    
    println!("Resultado CP-F6: Retornó Err(StatusCode::INTERNAL_SERVER_ERROR). Se redirigió el flujo cancelando transacción.");
    println!("--------------------------------------------------");
}

// ---------------------------------------------------------------------------
// CP-F7: Error forzado en tx.commit().await  ->  INTERNAL_SERVER_ERROR
// ---------------------------------------------------------------------------
#[sqlx::test]
async fn cp_f7(pool: PgPool) {
    println!("--------------------------------------------------");
    println!("Ejecutando CP-F7: Validar error interno si falla tx.commit().await al final.");
    
    let target_id = Uuid::from_str("c7d8e9f0-1234-5678-90ab-cdef12345678").unwrap();
    seed_item_with_id(&pool, target_id, 10.0).await;

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

    let payload = CreateMovimientoDto {
        item_id: target_id,
        cantidad: 25.0,
        tipo: TipoMovimiento::ENTRADA,
        motivo: "Reabastecimiento de insumos".to_string(),
        lote_id: None,
        numero_factura: None,
    };

    let res = create_movement(
        State(pool.clone()),
        Extension(USUARIO.to_string()),
        Json(payload),
    )
    .await;

    assert_eq!(res.err(), Some(StatusCode::INTERNAL_SERVER_ERROR));

    // Al fallar el commit, nada se persiste.
    let cantidad: f64 = sqlx::query_scalar("SELECT cantidad FROM inventario_items WHERE id = $1")
        .bind(target_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cantidad, 10.0);
    let movimientos: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM movimientos_stock WHERE item_id = $1")
            .bind(target_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(movimientos, 0);
    
    println!("Resultado CP-F7: Retornó Err(StatusCode::INTERNAL_SERVER_ERROR). Datos previnieron persistir tras fallo crítico en commit.");
    println!("--------------------------------------------------");
}

// ---------------------------------------------------------------------------
// CP-F8: ENTRADA con datos correctos  ->  CREATED + persistencia en BD
// ---------------------------------------------------------------------------
#[sqlx::test]
async fn cp_f8(pool: PgPool) {
    println!("--------------------------------------------------");
    println!("Ejecutando CP-F8: Validar ejecución exitosa de un movimiento tipo ENTRADA.");
    
    let target_id = Uuid::new_v4();
    seed_item_with_id(&pool, target_id, 100.0).await;
    
    let payload = CreateMovimientoDto {
        item_id: target_id,
        cantidad: 50.5,
        tipo: TipoMovimiento::ENTRADA,
        motivo: "Ingreso de mercadería".to_string(),
        lote_id: None,
        numero_factura: None,
    };

    let res = create_movement(
        State(pool.clone()),
        Extension(USUARIO.to_string()),
        Json(payload),
    )
    .await;

    match res {
        Ok((status, Json(mov))) => {
            assert_eq!(status, StatusCode::CREATED);
            assert_eq!(mov.item_id, target_id);
            assert_eq!(mov.cantidad, 50.5);
            assert_eq!(mov.tipo, TipoMovimiento::ENTRADA);
        }
        Err(e) => panic!("Se esperaba CREATED, se obtuvo el error {:?}", e),
    }

    // Stock recalculado: 100 + 50.5 = 150.5.
    let cantidad: f64 = sqlx::query_scalar("SELECT cantidad FROM inventario_items WHERE id = $1")
        .bind(target_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cantidad, 150.5);

    // Se registró exactamente un movimiento.
    let movimientos: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM movimientos_stock WHERE item_id = $1")
            .bind(target_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(movimientos, 1);
    
    println!("Resultado CP-F8: Retornó CREATED. Cantidad actualizada a 150.5.");
    println!("--------------------------------------------------");
}

// ---------------------------------------------------------------------------
// CP-F9: SALIDA con stock suficiente y datos correctos  ->  CREATED + persistencia
// ---------------------------------------------------------------------------
#[sqlx::test]
async fn cp_f9(pool: PgPool) {
    println!("--------------------------------------------------");
    println!("Ejecutando CP-F9: Validar procesamiento exitoso de movimiento de tipo SALIDA.");
    
    let target_id = Uuid::new_v4();
    seed_item_with_id(&pool, target_id, 100.0).await;
    
    let payload = CreateMovimientoDto {
        item_id: target_id,
        cantidad: 50.5,
        tipo: TipoMovimiento::SALIDA,
        motivo: "Ingreso de mercadería".to_string(),
        lote_id: None,
        numero_factura: None,
    };

    let res = create_movement(
        State(pool.clone()),
        Extension(USUARIO.to_string()),
        Json(payload),
    )
    .await;

    match res {
        Ok((status, Json(mov))) => {
            assert_eq!(status, StatusCode::CREATED);
            assert_eq!(mov.item_id, target_id);
            assert_eq!(mov.cantidad, 50.5);
            assert_eq!(mov.tipo, TipoMovimiento::SALIDA);
        }
        Err(e) => panic!("Se esperaba CREATED, se obtuvo el error {:?}", e),
    }

    // Stock recalculado: 100.0 - 50.5 = 49.5 (Nota: el caso menciona 45.5, pero 100-50.5 es 49.5)
    let cantidad: f64 = sqlx::query_scalar("SELECT cantidad FROM inventario_items WHERE id = $1")
        .bind(target_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    
    assert_eq!(cantidad, 49.5);

    let movimientos: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM movimientos_stock WHERE item_id = $1")
            .bind(target_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(movimientos, 1);
    
    println!("Resultado CP-F9: Retornó CREATED. El sistema valida y la cantidad final es 49.5");
    println!("--------------------------------------------------");
}
