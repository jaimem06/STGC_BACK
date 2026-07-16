use sqlx::PgPool;
use tracing::{debug, error, info};

use crate::models::{
    DashboardReport,
    MovimientoReport,
    ReportFilter,
    StockReport,
    VentaReport,
    VentaPeriodoReport,
    VentaProductoReport,
    VentaEmpleadoReport,
    VentaEmpleadoDetalleReport,
    SalesReportFilter,
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
            ms.cantidad,
            ms.motivo,
            NULL AS usuario_id,
            ms.fecha
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

// ============================================
// NUEVOS REPORTES DE VENTAS
// ============================================

/// HU017 - Reporte de ventas por período (día, semana, mes)
pub async fn get_sales_by_period(
    pool: &PgPool,
    filter: SalesReportFilter,
) -> Result<Vec<VentaPeriodoReport>, sqlx::Error> {
    debug!("🔍 Ejecutando get_sales_by_period con periodo: {:?}", filter.periodo);

    let periodo = filter.periodo.unwrap_or_else(|| "dia".to_string());
    
    let query = match periodo.as_str() {
        "dia" => r#"
            SELECT 
                TO_CHAR(v.fecha, 'YYYY-MM-DD') AS periodo,
                COALESCE(SUM(v.total), 0) AS total_ventas,
                COUNT(DISTINCT v.id) AS numero_ventas,
                COALESCE(SUM(dv.cantidad), 0) AS total_productos_vendidos
            FROM ventas v
            LEFT JOIN detalle_ventas dv ON dv.venta_id = v.id
            WHERE ($1::timestamptz IS NULL OR v.fecha >= $1)
                AND ($2::timestamptz IS NULL OR v.fecha <= $2)
            GROUP BY TO_CHAR(v.fecha, 'YYYY-MM-DD')
            ORDER BY periodo DESC
        "#,
        "semana" => r#"
            SELECT 
                TO_CHAR(DATE_TRUNC('week', v.fecha), 'YYYY-MM-DD') AS periodo,
                COALESCE(SUM(v.total), 0) AS total_ventas,
                COUNT(DISTINCT v.id) AS numero_ventas,
                COALESCE(SUM(dv.cantidad), 0) AS total_productos_vendidos
            FROM ventas v
            LEFT JOIN detalle_ventas dv ON dv.venta_id = v.id
            WHERE ($1::timestamptz IS NULL OR v.fecha >= $1)
                AND ($2::timestamptz IS NULL OR v.fecha <= $2)
            GROUP BY DATE_TRUNC('week', v.fecha)
            ORDER BY periodo DESC
        "#,
        "mes" => r#"
            SELECT 
                TO_CHAR(DATE_TRUNC('month', v.fecha), 'YYYY-MM') AS periodo,
                COALESCE(SUM(v.total), 0) AS total_ventas,
                COUNT(DISTINCT v.id) AS numero_ventas,
                COALESCE(SUM(dv.cantidad), 0) AS total_productos_vendidos
            FROM ventas v
            LEFT JOIN detalle_ventas dv ON dv.venta_id = v.id
            WHERE ($1::timestamptz IS NULL OR v.fecha >= $1)
                AND ($2::timestamptz IS NULL OR v.fecha <= $2)
            GROUP BY DATE_TRUNC('month', v.fecha)
            ORDER BY periodo DESC
        "#,
        _ => r#"
            SELECT 
                TO_CHAR(v.fecha, 'YYYY-MM-DD') AS periodo,
                COALESCE(SUM(v.total), 0) AS total_ventas,
                COUNT(DISTINCT v.id) AS numero_ventas,
                COALESCE(SUM(dv.cantidad), 0) AS total_productos_vendidos
            FROM ventas v
            LEFT JOIN detalle_ventas dv ON dv.venta_id = v.id
            WHERE ($1::timestamptz IS NULL OR v.fecha >= $1)
                AND ($2::timestamptz IS NULL OR v.fecha <= $2)
            GROUP BY TO_CHAR(v.fecha, 'YYYY-MM-DD')
            ORDER BY periodo DESC
        "#
    };

    debug!("📝 Query de período: {}", query);

    let result = sqlx::query_as::<_, VentaPeriodoReport>(query)
        .bind(filter.fecha_inicio)
        .bind(filter.fecha_fin)
        .fetch_all(pool)
        .await;

    match &result {
        Ok(rows) => info!("✅ Ventas por período: {} registros encontrados", rows.len()),
        Err(e) => error!("❌ Error en query de ventas por período: {}", e),
    }

    result
}

/// HU017 - Reporte de ventas por producto
pub async fn get_sales_by_product(
    pool: &PgPool,
    filter: SalesReportFilter,
) -> Result<Vec<VentaProductoReport>, sqlx::Error> {
    debug!("🔍 Ejecutando get_sales_by_product");

    let query = r#"
        SELECT 
            p.id AS producto_id,
            p.nombre AS producto_nombre,
            COALESCE(SUM(dv.cantidad), 0) AS total_vendido,
            COALESCE(SUM(dv.subtotal), 0) AS total_ingresos,
            COUNT(DISTINCT v.id) AS numero_ventas
        FROM productos p
        LEFT JOIN detalle_ventas dv ON dv.producto_id = p.id
        LEFT JOIN ventas v ON v.id = dv.venta_id
        WHERE ($1::timestamptz IS NULL OR v.fecha >= $1)
            AND ($2::timestamptz IS NULL OR v.fecha <= $2)
        GROUP BY p.id, p.nombre
        ORDER BY total_vendido DESC
    "#;

    debug!("📝 Query de productos: {}", query);

    let result = sqlx::query_as::<_, VentaProductoReport>(query)
        .bind(filter.fecha_inicio)
        .bind(filter.fecha_fin)
        .fetch_all(pool)
        .await;

    match &result {
        Ok(rows) => info!("✅ Ventas por producto: {} registros encontrados", rows.len()),
        Err(e) => error!("❌ Error en query de ventas por producto: {}", e),
    }

    result
}

/// HU017 - Reporte de ventas por empleado (todos los empleados)
pub async fn get_sales_by_employee(
    pool: &PgPool,
    filter: SalesReportFilter,
) -> Result<Vec<VentaEmpleadoReport>, sqlx::Error> {
    debug!("🔍 Ejecutando get_sales_by_employee");

    let query = r#"
        SELECT 
            u.id::text AS empleado_id,
            u.nombre AS empleado_nombre,
            COALESCE(SUM(v.total), 0) AS total_ventas,
            COUNT(DISTINCT v.id) AS numero_ventas,
            COALESCE(SUM(dv.cantidad), 0) AS total_productos_vendidos
        FROM usuarios u
        LEFT JOIN ventas v ON v.usuario_id = u.id
        LEFT JOIN detalle_ventas dv ON dv.venta_id = v.id
        WHERE ($1::timestamptz IS NULL OR v.fecha >= $1)
            AND ($2::timestamptz IS NULL OR v.fecha <= $2)
        GROUP BY u.id, u.nombre
        ORDER BY total_ventas DESC
    "#;

    debug!("📝 Query de empleados: {}", query);

    let result = sqlx::query_as::<_, VentaEmpleadoReport>(query)
        .bind(filter.fecha_inicio)
        .bind(filter.fecha_fin)
        .fetch_all(pool)
        .await;

    match &result {
        Ok(rows) => info!("✅ Ventas por empleado: {} registros encontrados", rows.len()),
        Err(e) => error!("❌ Error en query de ventas por empleado: {}", e),
    }

    result
}

/// HU017 - Reporte de ventas de un empleado específico por ID
pub async fn get_sales_by_employee_id(
    pool: &PgPool,
    empleado_id: String,
    filter: SalesReportFilter,
) -> Result<Vec<VentaEmpleadoDetalleReport>, sqlx::Error> {
    debug!("🔍 Ejecutando get_sales_by_employee_id para empleado: {}", empleado_id);

    let query = r#"
        SELECT 
            v.id AS venta_id,
            v.fecha,
            p.nombre AS producto,
            dv.cantidad,
            dv.precio_unitario,
            dv.subtotal,
            v.total
        FROM ventas v
        INNER JOIN detalle_ventas dv ON dv.venta_id = v.id
        INNER JOIN productos p ON p.id = dv.producto_id
        WHERE v.usuario_id = $3::uuid
            AND ($1::timestamptz IS NULL OR v.fecha >= $1)
            AND ($2::timestamptz IS NULL OR v.fecha <= $2)
        ORDER BY v.fecha DESC
    "#;

    debug!("📝 Query de empleado específico: {}", query);

    // Convertir String a UUID
    let empleado_uuid = uuid::Uuid::parse_str(&empleado_id)
        .map_err(|e| {
            error!("❌ Error al parsear UUID: {}", e);
            sqlx::Error::Configuration(format!("ID de empleado inválido: {}", e).into())
        })?;

    let result = sqlx::query_as::<_, VentaEmpleadoDetalleReport>(query)
        .bind(filter.fecha_inicio)
        .bind(filter.fecha_fin)
        .bind(empleado_uuid)
        .fetch_all(pool)
        .await;

    match &result {
        Ok(rows) => info!("✅ Ventas del empleado: {} registros encontrados", rows.len()),
        Err(e) => error!("❌ Error en query de ventas por empleado ID: {}", e),
    }

    result
}