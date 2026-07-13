use sqlx::PgPool;

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

    let report = sqlx::query_as::<_, StockReport>(
        r#"
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
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(report)
}

/// HU027 - Reporte de movimientos
pub async fn get_movements_report(
    pool: &PgPool,
    _filter: ReportFilter,
) -> Result<Vec<MovimientoReport>, sqlx::Error> {

    let report = sqlx::query_as::<_, MovimientoReport>(
        r#"
        SELECT
            ms.id AS movimiento_id,
            ms.item_id,
            ii.nombre AS producto,
            ms.tipo::text AS tipo_movimiento,
            ms.cantidad,
            ms.motivo,
            NULL AS usuario_id,
            ms.fecha
        FROM movimientos_stock ms
        INNER JOIN inventario_items ii
            ON ii.id = ms.item_id
        ORDER BY ms.fecha DESC
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(report)
}

/// HU017 - Reporte de ventas
pub async fn get_sales_report(
    pool: &PgPool,
    _filter: ReportFilter,
) -> Result<Vec<VentaReport>, sqlx::Error> {

    let report = sqlx::query_as::<_, VentaReport>(
        r#"
        SELECT
            v.id AS venta_id,
            v.fecha,
            u.nombre AS empleado,
            p.nombre AS producto,
            dv.cantidad,
            dv.precio_unitario,
            dv.subtotal,
            v.total
        FROM ventas v
        INNER JOIN detalle_ventas dv
            ON dv.venta_id = v.id
        INNER JOIN productos p
            ON p.id = dv.producto_id
        INNER JOIN usuarios u
            ON u.id = v.usuario_id
        ORDER BY v.fecha DESC
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(report)
}

/// Dashboard
pub async fn get_dashboard(
    pool: &PgPool,
) -> Result<DashboardReport, sqlx::Error> {

    let dashboard = sqlx::query_as::<_, DashboardReport>(
        r#"
        SELECT
            (SELECT COUNT(*) FROM inventario_items WHERE is_deleted = false) AS total_productos,
            (SELECT COUNT(*) FROM inventario_items WHERE cantidad <= stock_minimo) AS stock_bajo,
            (SELECT COUNT(*) FROM inventario_items WHERE cantidad = 0) AS productos_agotados,
            (SELECT COUNT(*) FROM movimientos_stock) AS total_movimientos,
            COALESCE((SELECT SUM(total) FROM ventas),0) AS total_ventas
        "#
    )
    .fetch_one(pool)
    .await?;

    Ok(dashboard)
}