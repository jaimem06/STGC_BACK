use sqlx::PgPool;
use tokio::sync::OnceCell;
use tracing::{debug, error, info, warn};

use crate::models::{
    DashboardReport,
    MovimientoReport,
    ReportFilter,
    SalesReportFilter,
    StockReport,
    VentaReport,
    VentaPeriodoReport,
    VentaProductoReport,
    VentaEmpleadoReport,
    VentaEmpleadoDetalleReport,
};

// Este servicio es de SOLO LECTURA sobre las tablas reales del sistema:
// - Ventas: pos_service."Pedido" / "PedidoItem" (estado PAGADO, fecha_pago).
//   fecha_pago es timestamp SIN zona guardado en UTC (Prisma), por eso cada
//   consulta lo convierte con AT TIME ZONE 'UTC' antes de comparar o agrupar.
// - Inventario: public.inventario_items / public.movimientos_stock (inventory-service).
// - Nombres de empleados: tabla users del auth-service (cajero_id = users.id),
//   localizada en tiempo de ejecución porque no siempre vive en `public`.
// Las agrupaciones por día/semana/mes usan la zona horaria de Ecuador para que
// "hoy" coincida con el día calendario local y no con el corte UTC.

/// Zona horaria de negocio (Ecuador continental, sin horario de verano).
/// Expresión SQL que da la fecha/hora local de Ecuador a partir de fecha_pago.
const FECHA_PAGO_EC: &str = "((p.fecha_pago AT TIME ZONE 'UTC') AT TIME ZONE 'America/Guayaquil')";
/// Expresión SQL con el instante real (timestamptz) del pago.
const FECHA_PAGO_UTC: &str = "(p.fecha_pago AT TIME ZONE 'UTC')";

/// Tabla de usuarios del auth-service, resuelta una sola vez por proceso.
/// `None` significa que no está en esta base de datos.
static TABLA_USUARIOS: OnceCell<Option<String>> = OnceCell::const_new();

/// Localiza la tabla de usuarios del auth-service. No se asume `public.users`:
/// según cómo esté desplegado el auth puede vivir en otro esquema o incluso en
/// otra base. Se resuelve en el primer uso y, si no aparece, los reportes
/// degradan a mostrar el id del cajero en vez de devolver 500.
/// `AUTH_USERS_TABLE` (p. ej. `auth_service.users`) permite forzar el valor.
async fn resolver_tabla_usuarios(pool: &PgPool) -> Option<String> {
    if let Ok(forzada) = std::env::var("AUTH_USERS_TABLE") {
        let forzada = forzada.trim();
        if !forzada.is_empty() {
            info!("Tabla de usuarios forzada por AUTH_USERS_TABLE: {forzada}");
            return Some(forzada.to_string());
        }
    }

    let encontrada: Result<Option<(String,)>, _> = sqlx::query_as(
        r#"
        SELECT table_schema
        FROM information_schema.tables
        WHERE table_name = 'users'
          AND table_schema NOT IN ('information_schema', 'pg_catalog')
        ORDER BY (table_schema = 'public') DESC, table_schema
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await;

    match encontrada {
        Ok(Some((esquema,))) => {
            let tabla = format!("\"{esquema}\".users");
            info!("Nombres de empleados tomados de {tabla}");
            Some(tabla)
        }
        Ok(None) => {
            warn!(
                "No se encontró la tabla 'users' en esta base de datos: los reportes \
                 mostrarán el id del cajero en lugar del nombre. Si el auth-service usa \
                 otra base, comparte la misma DATABASE_URL o define AUTH_USERS_TABLE."
            );
            None
        }
        Err(e) => {
            error!("No se pudo localizar la tabla de usuarios: {e}");
            None
        }
    }
}

async fn tabla_usuarios(pool: &PgPool) -> Option<&'static str> {
    TABLA_USUARIOS
        .get_or_init(|| resolver_tabla_usuarios(pool))
        .await
        .as_deref()
}

/// Devuelve el `LEFT JOIN` a usuarios y la expresión del nombre visible del
/// empleado. Sin tabla de usuarios el join se omite y el nombre es la columna
/// del id, de modo que la consulta sigue siendo válida.
fn join_usuarios(tabla: Option<&str>, columna_id: &str) -> (String, String) {
    match tabla {
        Some(tabla) => (
            format!("LEFT JOIN {tabla} u ON u.id = {columna_id}"),
            format!(
                "COALESCE(NULLIF(TRIM(CONCAT(u.first_name, ' ', u.last_name)), ''), \
                 u.email, {columna_id})"
            ),
        ),
        None => (String::new(), columna_id.to_string()),
    }
}

/// HU026 - Reporte de stock actual
pub async fn get_stock_report(
    pool: &PgPool,
    _filter: ReportFilter,
) -> Result<Vec<StockReport>, sqlx::Error> {
    debug!("Ejecutando get_stock_report");

    let query = r#"
        SELECT
            id AS item_id,
            sku,
            nombre,
            tipo::text AS tipo,
            cantidad,
            COALESCE(stock_minimo, 0) AS stock_minimo,
            unidad_medida::text AS unidad_medida,
            estado::text AS estado
        FROM public.inventario_items
        WHERE is_deleted = false
        ORDER BY nombre
    "#;

    let result = sqlx::query_as::<_, StockReport>(query).fetch_all(pool).await;

    match &result {
        Ok(rows) => info!("Stock report: {} registros encontrados", rows.len()),
        Err(e) => error!("Error en query de stock: {}", e),
    }

    result
}

/// HU027 - Reporte de movimientos
pub async fn get_movements_report(
    pool: &PgPool,
    filter: ReportFilter,
) -> Result<Vec<MovimientoReport>, sqlx::Error> {
    debug!("Ejecutando get_movements_report");

    let (join_usuario, nombre_usuario) =
        join_usuarios(tabla_usuarios(pool).await, "ms.usuario_id");

    let query = format!(
        r#"
        SELECT
            ms.id AS movimiento_id,
            ms.item_id,
            ii.nombre AS producto,
            ms.tipo::text AS tipo_movimiento,
            ms.cantidad,
            COALESCE(ms.motivo, '') AS motivo,
            {nombre_usuario} AS usuario_id,
            ms.fecha
        FROM public.movimientos_stock ms
        INNER JOIN public.inventario_items ii
            ON ii.id = ms.item_id
        {join_usuario}
        WHERE ($1::timestamptz IS NULL OR ms.fecha >= $1)
            AND ($2::timestamptz IS NULL OR ms.fecha <= $2)
            AND ($3::text IS NULL OR ms.tipo::text = $3)
        ORDER BY ms.fecha DESC
        "#
    );

    let result = sqlx::query_as::<_, MovimientoReport>(&query)
        .bind(filter.fecha_inicio)
        .bind(filter.fecha_fin)
        .bind(filter.tipo)
        .fetch_all(pool)
        .await;

    match &result {
        Ok(rows) => info!("Movements report: {} registros encontrados", rows.len()),
        Err(e) => error!("Error en query de movimientos: {}", e),
    }

    result
}

/// HU017 - Reporte de ventas general (una fila por producto vendido)
pub async fn get_sales_report(
    pool: &PgPool,
    filter: ReportFilter,
) -> Result<Vec<VentaReport>, sqlx::Error> {
    debug!("Ejecutando get_sales_report");

    let (join_usuario, nombre_empleado) =
        join_usuarios(tabla_usuarios(pool).await, "p.cajero_id");

    let query = format!(
        r#"
        SELECT
            p.id AS venta_id,
            {FECHA_PAGO_UTC} AS fecha,
            {nombre_empleado} AS empleado,
            pi.nombre AS producto,
            pi.cantidad::float8 AS cantidad,
            pi."precioUnitario" AS precio_unitario,
            pi.subtotal AS subtotal,
            p.total AS total
        FROM pos_service."Pedido" p
        INNER JOIN pos_service."PedidoItem" pi
            ON pi."pedidoId" = p.id
        {join_usuario}
        WHERE p.estado::text = 'PAGADO'
            AND p.fecha_pago IS NOT NULL
            AND ($1::timestamptz IS NULL OR {FECHA_PAGO_UTC} >= $1)
            AND ($2::timestamptz IS NULL OR {FECHA_PAGO_UTC} <= $2)
            AND ($3::text IS NULL OR pi."productoId" = $3)
            AND ($4::text IS NULL OR p.cajero_id = $4)
        ORDER BY p.fecha_pago DESC
        "#
    );


    let result = sqlx::query_as::<_, VentaReport>(&query)
        .bind(filter.fecha_inicio)
        .bind(filter.fecha_fin)
        .bind(filter.producto_id)
        .bind(filter.empleado_id)
        .fetch_all(pool)
        .await;

    match &result {
        Ok(rows) => info!("Sales report: {} registros encontrados", rows.len()),
        Err(e) => error!("Error en query de ventas: {}", e),
    }

    result
}

/// Dashboard
pub async fn get_dashboard(
    pool: &PgPool,
) -> Result<DashboardReport, sqlx::Error> {
    debug!("Ejecutando get_dashboard");

    let query = r#"
        SELECT
            (SELECT COUNT(*) FROM public.inventario_items WHERE is_deleted = false) AS total_productos,
            (SELECT COUNT(*) FROM public.inventario_items WHERE is_deleted = false AND cantidad <= COALESCE(stock_minimo, 0)) AS stock_bajo,
            (SELECT COUNT(*) FROM public.inventario_items WHERE is_deleted = false AND cantidad = 0) AS productos_agotados,
            (SELECT COUNT(*) FROM public.movimientos_stock) AS total_movimientos,
            COALESCE((SELECT SUM(p.total) FROM pos_service."Pedido" p WHERE p.estado::text = 'PAGADO'), 0)::float8 AS total_ventas
    "#;

    let result = sqlx::query_as::<_, DashboardReport>(query)
        .fetch_one(pool)
        .await;

    match &result {
        Ok(_) => info!("Dashboard generado exitosamente"),
        Err(e) => error!("Error en query de dashboard: {}", e),
    }

    result
}

// ============================================
// REPORTES DE VENTAS (fuente: pedidos PAGADOS del POS)
// ============================================

/// HU017 - Reporte de ventas por período (día, semana, mes)
pub async fn get_sales_by_period(
    pool: &PgPool,
    filter: SalesReportFilter,
) -> Result<Vec<VentaPeriodoReport>, sqlx::Error> {
    debug!("Ejecutando get_sales_by_period con periodo: {:?}", filter.periodo);

    let periodo = filter.periodo.unwrap_or_else(|| "dia".to_string());

    // La expresión del período se agrupa en hora local de Ecuador. El total de
    // unidades sale de una subconsulta agregada por pedido para no duplicar
    // p.total al unir con los items.
    let periodo_expr = match periodo.as_str() {
        "semana" => format!("TO_CHAR(DATE_TRUNC('week', {FECHA_PAGO_EC}), 'YYYY-MM-DD')"),
        "mes" => format!("TO_CHAR(DATE_TRUNC('month', {FECHA_PAGO_EC}), 'YYYY-MM')"),
        _ => format!("TO_CHAR({FECHA_PAGO_EC}, 'YYYY-MM-DD')"),
    };

    let query = format!(
        r#"
        SELECT
            {periodo_expr} AS periodo,
            COALESCE(SUM(p.total), 0)::float8 AS total_ventas,
            COUNT(*) AS numero_ventas,
            COALESCE(SUM(it.unidades), 0)::float8 AS total_productos_vendidos
        FROM pos_service."Pedido" p
        LEFT JOIN (
            SELECT "pedidoId", SUM(cantidad)::float8 AS unidades
            FROM pos_service."PedidoItem"
            GROUP BY "pedidoId"
        ) it ON it."pedidoId" = p.id
        WHERE p.estado::text = 'PAGADO'
            AND p.fecha_pago IS NOT NULL
            AND ($1::timestamptz IS NULL OR {FECHA_PAGO_UTC} >= $1)
            AND ($2::timestamptz IS NULL OR {FECHA_PAGO_UTC} <= $2)
        GROUP BY 1
        ORDER BY periodo DESC
        "#
    );

    let result = sqlx::query_as::<_, VentaPeriodoReport>(&query)
        .bind(filter.fecha_inicio)
        .bind(filter.fecha_fin)
        .fetch_all(pool)
        .await;

    match &result {
        Ok(rows) => info!("Ventas por período: {} registros encontrados", rows.len()),
        Err(e) => error!("Error en query de ventas por período: {}", e),
    }

    result
}

/// HU017 - Reporte de ventas por producto
pub async fn get_sales_by_product(
    pool: &PgPool,
    filter: SalesReportFilter,
) -> Result<Vec<VentaProductoReport>, sqlx::Error> {
    debug!("Ejecutando get_sales_by_product");

    let query = format!(
        r#"
        SELECT
            pi."productoId" AS producto_id,
            COALESCE(MAX(ii.nombre), MAX(pi.nombre)) AS producto_nombre,
            COALESCE(SUM(pi.cantidad), 0)::float8 AS total_vendido,
            COALESCE(SUM(pi.subtotal), 0)::float8 AS total_ingresos,
            COUNT(DISTINCT p.id) AS numero_ventas
        FROM pos_service."PedidoItem" pi
        INNER JOIN pos_service."Pedido" p
            ON p.id = pi."pedidoId"
            AND p.estado::text = 'PAGADO'
            AND p.fecha_pago IS NOT NULL
        LEFT JOIN public.inventario_items ii
            ON ii.id::text = pi."productoId"
        WHERE ($1::timestamptz IS NULL OR {FECHA_PAGO_UTC} >= $1)
            AND ($2::timestamptz IS NULL OR {FECHA_PAGO_UTC} <= $2)
        GROUP BY pi."productoId"
        ORDER BY total_vendido DESC
        "#
    );

    let result = sqlx::query_as::<_, VentaProductoReport>(&query)
        .bind(filter.fecha_inicio)
        .bind(filter.fecha_fin)
        .fetch_all(pool)
        .await;

    match &result {
        Ok(rows) => info!("Ventas por producto: {} registros encontrados", rows.len()),
        Err(e) => error!("Error en query de ventas por producto: {}", e),
    }

    result
}

/// HU017 - Reporte de ventas por empleado (todos los empleados con ventas)
pub async fn get_sales_by_employee(
    pool: &PgPool,
    filter: SalesReportFilter,
) -> Result<Vec<VentaEmpleadoReport>, sqlx::Error> {
    debug!("Ejecutando get_sales_by_employee");

    let tabla = tabla_usuarios(pool).await;
    let (join_usuario, nombre_empleado) = join_usuarios(tabla, "p.cajero_id");
    // Sin join a usuarios el nombre es el propio cajero_id, ya agrupado.
    let group_by_usuario = if tabla.is_some() {
        ", u.first_name, u.last_name, u.email"
    } else {
        ""
    };

    let query = format!(
        r#"
        SELECT
            p.cajero_id AS empleado_id,
            {nombre_empleado} AS empleado_nombre,
            COALESCE(SUM(p.total), 0)::float8 AS total_ventas,
            COUNT(*) AS numero_ventas,
            COALESCE(SUM(it.unidades), 0)::float8 AS total_productos_vendidos
        FROM pos_service."Pedido" p
        {join_usuario}
        LEFT JOIN (
            SELECT "pedidoId", SUM(cantidad)::float8 AS unidades
            FROM pos_service."PedidoItem"
            GROUP BY "pedidoId"
        ) it ON it."pedidoId" = p.id
        WHERE p.estado::text = 'PAGADO'
            AND p.fecha_pago IS NOT NULL
            AND ($1::timestamptz IS NULL OR {FECHA_PAGO_UTC} >= $1)
            AND ($2::timestamptz IS NULL OR {FECHA_PAGO_UTC} <= $2)
        GROUP BY p.cajero_id{group_by_usuario}
        ORDER BY total_ventas DESC
        "#
    );

    let result = sqlx::query_as::<_, VentaEmpleadoReport>(&query)
        .bind(filter.fecha_inicio)
        .bind(filter.fecha_fin)
        .fetch_all(pool)
        .await;

    match &result {
        Ok(rows) => info!("Ventas por empleado: {} registros encontrados", rows.len()),
        Err(e) => error!("Error en query de ventas por empleado: {}", e),
    }

    result
}

/// HU017 - Reporte de ventas de un empleado específico por ID
pub async fn get_sales_by_employee_id(
    pool: &PgPool,
    empleado_id: String,
    filter: SalesReportFilter,
) -> Result<Vec<VentaEmpleadoDetalleReport>, sqlx::Error> {
    debug!("Ejecutando get_sales_by_employee_id para empleado: {}", empleado_id);

    // cajero_id es TEXT (el sub del JWT del auth-service): se compara tal cual,
    // sin exigir que sea un UUID válido.
    let query = format!(
        r#"
        SELECT
            p.id AS venta_id,
            {FECHA_PAGO_UTC} AS fecha,
            pi.nombre AS producto,
            pi.cantidad::float8 AS cantidad,
            pi."precioUnitario" AS precio_unitario,
            pi.subtotal AS subtotal,
            p.total AS total
        FROM pos_service."Pedido" p
        INNER JOIN pos_service."PedidoItem" pi
            ON pi."pedidoId" = p.id
        WHERE p.estado::text = 'PAGADO'
            AND p.fecha_pago IS NOT NULL
            AND p.cajero_id = $3
            AND ($1::timestamptz IS NULL OR {FECHA_PAGO_UTC} >= $1)
            AND ($2::timestamptz IS NULL OR {FECHA_PAGO_UTC} <= $2)
        ORDER BY p.fecha_pago DESC
        "#
    );

    let result = sqlx::query_as::<_, VentaEmpleadoDetalleReport>(&query)
        .bind(filter.fecha_inicio)
        .bind(filter.fecha_fin)
        .bind(empleado_id)
        .fetch_all(pool)
        .await;

    match &result {
        Ok(rows) => info!("Ventas del empleado: {} registros encontrados", rows.len()),
        Err(e) => error!("Error en query de ventas por empleado ID: {}", e),
    }

    result
}

/// Obtener lista de semanas disponibles con ventas
pub async fn get_available_weeks(
    pool: &PgPool,
) -> Result<Vec<serde_json::Value>, sqlx::Error> {
    debug!("Ejecutando get_available_weeks");

    let query = format!(
        r#"
        SELECT DISTINCT
            TO_CHAR(DATE_TRUNC('week', {FECHA_PAGO_EC}), 'YYYY-MM-DD') AS inicio_semana,
            TO_CHAR(DATE_TRUNC('week', {FECHA_PAGO_EC}) + INTERVAL '6 days', 'YYYY-MM-DD') AS fin_semana,
            EXTRACT(WEEK FROM {FECHA_PAGO_EC})::integer AS numero_semana,
            TO_CHAR(DATE_TRUNC('week', {FECHA_PAGO_EC}), 'YYYY') AS año
        FROM pos_service."Pedido" p
        WHERE p.estado::text = 'PAGADO'
            AND p.fecha_pago IS NOT NULL
        ORDER BY inicio_semana DESC
        "#
    );

    let result = sqlx::query_as::<_, (String, String, i32, String)>(&query)
        .fetch_all(pool)
        .await?;

    let weeks: Vec<serde_json::Value> = result
        .into_iter()
        .map(|(inicio, fin, numero, año)| {
            serde_json::json!({
                "inicio": inicio,
                "fin": fin,
                "numero_semana": numero,
                "año": año,
                "label": format!("Semana {} ({})", numero, año)
            })
        })
        .collect();

    Ok(weeks)
}
