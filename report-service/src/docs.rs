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
    ExportCsvRequest,
    ExportPdfRequest,
};

#[derive(OpenApi)]
#[openapi(
    paths(
        // ============================================
        // REPORTES PRINCIPALES
        // ============================================
        crate::handlers::report_handler::get_stock_report,
        crate::handlers::report_handler::get_movements_report,
        crate::handlers::report_handler::get_sales_report,
        crate::handlers::report_handler::get_dashboard,
        
        // ============================================
        // REPORTES POR PERÍODO
        // ============================================
        crate::handlers::report_handler::get_sales_by_day,
        crate::handlers::report_handler::get_sales_by_week,
        crate::handlers::report_handler::get_sales_by_month,
        
        // ============================================
        // REPORTES POR PRODUCTO Y EMPLEADO
        // ============================================
        crate::handlers::report_handler::get_sales_by_product,
        crate::handlers::report_handler::get_sales_by_employee,
        crate::handlers::report_handler::get_sales_by_employee_id,
        crate::handlers::report_handler::get_available_weeks,
        
        // ============================================
        // EXPORTACIONES (solo los que tienen #[utoipa::path])
        // ============================================
        // Los endpoints de exportación no tienen #[utoipa::path] aún,
        // por ahora los comentamos para que compile
        // crate::handlers::export_handler::export_pdf_from_data,
        // crate::handlers::export_handler::export_csv_from_data,
        
        // ============================================
        // TEST
        // ============================================
        crate::handlers::report_handler::test_db_connection,
    ),
    components(
        schemas(
            StockReport,
            MovimientoReport,
            VentaReport,
            DashboardReport,
            ReportFilter,
            SalesReportFilter,
            VentaPeriodoReport,
            VentaProductoReport,
            VentaEmpleadoReport,
            VentaEmpleadoDetalleReport,
            ExportCsvRequest,
            ExportPdfRequest,
        )
    ),
    tags(
        (name = "Reportes", description = "Endpoints de reportes del sistema STGC"),
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