// src/docs.rs
use utoipa::OpenApi;

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

#[derive(OpenApi)]
#[openapi(
    paths(
        // Reportes principales
        crate::handlers::report_handler::get_stock_report,
        crate::handlers::report_handler::get_movements_report,
        crate::handlers::report_handler::get_sales_report,
        crate::handlers::report_handler::get_dashboard,
        // Reportes de ventas por período
        crate::handlers::report_handler::get_sales_by_day,
        crate::handlers::report_handler::get_sales_by_week,
        crate::handlers::report_handler::get_sales_by_month,
        // Reportes de ventas por producto y empleado
        crate::handlers::report_handler::get_sales_by_product,
        crate::handlers::report_handler::get_sales_by_employee,
        crate::handlers::report_handler::get_sales_by_employee_id,
        // Test
        crate::handlers::report_handler::test_db_connection,
    ),
    components(
        schemas(
            // Modelos existentes
            StockReport,
            MovimientoReport,
            VentaReport,
            DashboardReport,
            ReportFilter,
            // Nuevos modelos
            SalesReportFilter,
            VentaPeriodoReport,
            VentaProductoReport,
            VentaEmpleadoReport,
            VentaEmpleadoDetalleReport,
        )
    ),
    tags(
        (name = "Reportes", description = "Endpoints de reportes del sistema STGC - Ruta del Café de Loja"),
        (name = "Pruebas", description = "Endpoints de prueba y diagnóstico")
    ),
    info(
        title = "Report Service API",
        description = "Servicio de reportes para el sistema STGC - Ruta del Café de Loja",
        version = "1.0.0",
        contact(
            name = "STGC Team",
            email = "stgc@example.com"
        )
    )
)]
pub struct ApiDoc;