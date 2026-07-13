use sqlx::PgPool;
use tracing::{debug, error, info};

use crate::models::{
    DashboardReport,
    MovimientoReport,
    ReportFilter,
    StockReport,
    VentaReport,
};

/// HU026 - Reporte de stock actual
pub async fn get_stock_report(
    pool: &PgPool,
    _filter: ReportFilter,
) -> Result<Vec<StockReport>, sqlx::Error> {
    debug!("🔍 Ejecutando get_stock_report");

    let query = r#"
        SELECT
            id AS item_id,
            sku,
            nombre,
            tipo::text AS tipo,
            cantidad,
            stock_minimo,
            unidad_medida::text AS unidad_medida,
            estado::text AS estado
        FROM inventario_items
        WHERE is_deleted = false
        ORDER BY nombre
    "#;

    debug!("📝 Query: {}", query);

    let result = sqlx::query_as::<_, StockReport>(query).fetch_all(pool).await;

    match &result {
        Ok(rows) => info!("✅ Stock report: {} registros encontrados", rows.len()),
        Err(e) => error!("❌ Error en query de stock: {}", e),
    }

    result
}

/// HU027 - Reporte de movimientos
pub async fn get_movements_report(
    pool: &PgPool,
    _filter: ReportFilter,
) -> Result<Vec<MovimientoReport>, sqlx::Error> {
    debug!("🔍 Ejecutando get_movements_report");

    let query = r#"
        SELECT
            ms.id AS movimiento_id,
            ms.item_id,
            ii.nombre AS producto,
            ms.tipo::text AS tipo_movimiento,
            ms.cantidad AS cantidad,
            ms.motivo,
            NULL AS usuario_id,
            ms.fecha AS fecha
        FROM movimientos_stock ms
        INNER JOIN inventario_items ii
            ON ii.id = ms.item_id
        ORDER BY ms.fecha DESC
    "#;

    debug!("📝 Query: {}", query);

    let result = sqlx::query_as::<_, MovimientoReport>(query)
        .fetch_all(pool)
        .await;

    match &result {
        Ok(rows) => info!("✅ Movements report: {} registros encontrados", rows.len()),
        Err(e) => error!("❌ Error en query de movimientos: {}", e),
    }

    result
}

/// HU017 - Reporte de ventas
pub async fn get_sales_report(
    pool: &PgPool,
    _filter: ReportFilter,
) -> Result<Vec<VentaReport>, sqlx::Error> {
    debug!("🔍 Ejecutando get_sales_report");

    let query = r#"
        SELECT
            v.id AS venta_id,
            v.fecha,
            u.nombre AS empleado,
            p.nombre AS producto,
            dv.cantidad AS cantidad,
            dv.precio_unitario AS precio_unitario,
            dv.subtotal AS subtotal,
            v.total AS total
        FROM ventas v
        INNER JOIN detalle_ventas dv
            ON dv.venta_id = v.id
        INNER JOIN productos p
            ON p.id = dv.producto_id
        INNER JOIN usuarios u
            ON u.id = v.usuario_id
        ORDER BY v.fecha DESC
    "#;

    debug!("📝 Query: {}", query);

    let result = sqlx::query_as::<_, VentaReport>(query).fetch_all(pool).await;

    match &result {
        Ok(rows) => info!("✅ Sales report: {} registros encontrados", rows.len()),
        Err(e) => error!("❌ Error en query de ventas: {}", e),
    }

    result
}

/// Dashboard
pub async fn get_dashboard(
    pool: &PgPool,
) -> Result<DashboardReport, sqlx::Error> {
    debug!("🔍 Ejecutando get_dashboard");

    let query = r#"
        SELECT
            (SELECT COUNT(*) FROM inventario_items WHERE is_deleted = false) AS total_productos,
            (SELECT COUNT(*) FROM inventario_items WHERE cantidad <= stock_minimo) AS stock_bajo,
            (SELECT COUNT(*) FROM inventario_items WHERE cantidad = 0) AS productos_agotados,
            (SELECT COUNT(*) FROM movimientos_stock) AS total_movimientos,
            COALESCE((SELECT SUM(total) FROM ventas), 0) AS total_ventas
    "#;

    debug!("📝 Query: {}", query);

    let result = sqlx::query_as::<_, DashboardReport>(query)
        .fetch_one(pool)
        .await;

    match &result {
        Ok(_) => info!("✅ Dashboard generado exitosamente"),
        Err(e) => error!("❌ Error en query de dashboard: {}", e),
    }

    result
}