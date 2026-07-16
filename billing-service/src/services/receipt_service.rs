use crate::models::billing::{BusinessInfo, ReceiptOrder};
use printpdf::{BuiltinFont, IndirectFontRef, Line, Mm, PdfDocument, PdfLayerReference, Point};
use std::fmt;
use std::io::{BufWriter, Cursor};

pub const UNPAID_ORDER_MESSAGE: &str =
    "No se puede generar el comprobante de un pedido sin pago confirmado.";
pub const SUCCESS_MESSAGE: &str = "Comprobante generado con éxito.";

#[derive(Debug, PartialEq)]
pub enum ReceiptValidationError {
    UnpaidOrder,
    MissingData(String),
}

impl fmt::Display for ReceiptValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnpaidOrder => write!(f, "{UNPAID_ORDER_MESSAGE}"),
            Self::MissingData(message) => write!(f, "{message}"),
        }
    }
}

pub fn format_receipt_number(number: i64) -> String {
    format!("COMP-{number:08}")
}

pub fn validate_receipt(
    order: &ReceiptOrder,
    business: &BusinessInfo,
) -> Result<(), ReceiptValidationError> {
    if order.estado != "PAGADO" {
        return Err(ReceiptValidationError::UnpaidOrder);
    }

    let mut missing = Vec::new();
    if business.nombre.trim().is_empty() {
        missing.push("nombre del negocio");
    }
    if business.ruc.trim().is_empty() {
        missing.push("RUC del negocio");
    }
    if business.direccion.trim().is_empty() {
        missing.push("dirección del negocio");
    }
    if order.pedido_id.trim().is_empty() {
        missing.push("número de pedido");
    }
    if order
        .cliente_nombre
        .as_deref()
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        missing.push("nombre del cliente");
    }
    if order
        .cliente_apellido
        .as_deref()
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        missing.push("apellido del cliente");
    }
    if order
        .cliente_cedula
        .as_deref()
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        missing.push("cédula del cliente");
    }
    if order.fecha_pago.is_none() {
        missing.push("fecha y hora de pago");
    }
    if order.items.is_empty() {
        missing.push("productos");
    }
    if order.pagos.is_empty() {
        missing.push("métodos de pago");
    }

    if !order.subtotal.is_finite()
        || !order.iva.is_finite()
        || !order.total.is_finite()
        || order.subtotal < 0.0
        || order.iva < 0.0
        || order.total < 0.0
    {
        missing.push("totales válidos");
    }
    if order.items.iter().any(|item| {
        item.nombre.trim().is_empty()
            || item.cantidad <= 0
            || !item.precio_unitario.is_finite()
            || !item.subtotal.is_finite()
            || item.precio_unitario < 0.0
            || item.subtotal < 0.0
    }) {
        missing.push("detalle válido de productos");
    }
    if order.pagos.iter().any(|payment| {
        payment.metodo_pago.trim().is_empty() || !payment.monto.is_finite() || payment.monto <= 0.0
    }) {
        missing.push("detalle válido de pagos");
    }

    let item_total: f64 = order.items.iter().map(|item| item.subtotal).sum();
    if (item_total - order.subtotal).abs() > 0.01
        || (order.subtotal + order.iva - order.total).abs() > 0.01
    {
        missing.push("consistencia de subtotal, IVA y total");
    }
    let payment_total: f64 = order.pagos.iter().map(|payment| payment.monto).sum();
    if (payment_total - order.total).abs() > 0.01 {
        missing.push("consistencia entre pagos y total");
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(ReceiptValidationError::MissingData(format!(
            "No se puede generar el comprobante: faltan {}.",
            missing.join(", ")
        )))
    }
}

fn wrapped_lines(prefix: &str, value: &str, max_chars: usize) -> Vec<String> {
    let text = format!("{prefix}{value}");
    if text.chars().count() <= max_chars {
        return vec![text];
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if word.chars().count() > max_chars {
            if !current.is_empty() {
                lines.push(current);
                current = String::new();
            }
            let characters: Vec<char> = word.chars().collect();
            for chunk in characters.chunks(max_chars) {
                lines.push(chunk.iter().collect());
            }
            continue;
        }
        let next_len =
            current.chars().count() + usize::from(!current.is_empty()) + word.chars().count();
        if next_len > max_chars && !current.is_empty() {
            lines.push(current);
            current = String::new();
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn draw_line(layer: &PdfLayerReference, x1: f32, y1: f32, x2: f32, y2: f32) {
    layer.add_line(Line {
        points: vec![
            (Point::new(Mm(x1), Mm(y1)), false),
            (Point::new(Mm(x2), Mm(y2)), false),
        ],
        is_closed: false,
    });
}

fn draw_rect(layer: &PdfLayerReference, left: f32, bottom: f32, right: f32, top: f32) {
    layer.add_line(Line {
        points: vec![
            (Point::new(Mm(left), Mm(bottom)), false),
            (Point::new(Mm(left), Mm(top)), false),
            (Point::new(Mm(right), Mm(top)), false),
            (Point::new(Mm(right), Mm(bottom)), false),
        ],
        is_closed: true,
    });
}

fn write_text(
    layer: &PdfLayerReference,
    text: impl AsRef<str>,
    size: f32,
    x: f32,
    y: f32,
    font: &IndirectFontRef,
) {
    layer.use_text(text.as_ref(), size, Mm(x), Mm(y), font);
}

fn render_header(
    layer: &PdfLayerReference,
    normal: &IndirectFontRef,
    bold: &IndirectFontRef,
    order: &ReceiptOrder,
    business: &BusinessInfo,
    receipt_number: i64,
) {
    let paid_at = order.fecha_pago.expect("validated payment date");
    layer.set_outline_thickness(0.6);

    // Distribución inspirada en el RIDE del SRI, limitada estrictamente a los datos de HU12-A.
    draw_rect(layer, 12.0, 250.0, 118.0, 285.0);
    draw_rect(layer, 122.0, 250.0, 198.0, 285.0);

    let mut business_y = 278.0;
    for line in wrapped_lines("", &business.nombre, 48) {
        write_text(layer, line, 11.0, 16.0, business_y, bold);
        business_y -= 5.0;
    }
    let mut address_y = 259.0;
    for line in wrapped_lines("DIRECCIÓN: ", &business.direccion, 62) {
        write_text(layer, line, 7.5, 16.0, address_y, normal);
        address_y -= 4.0;
    }

    write_text(
        layer,
        format!("R.U.C.: {}", business.ruc),
        10.0,
        126.0,
        278.0,
        bold,
    );
    write_text(layer, "FACTURA", 15.0, 126.0, 267.0, bold);
    write_text(
        layer,
        format!("No. {}", format_receipt_number(receipt_number)),
        10.0,
        126.0,
        257.0,
        bold,
    );

    draw_rect(layer, 12.0, 220.0, 198.0, 246.0);
    write_text(layer, "DATOS DEL CLIENTE", 9.0, 15.0, 241.0, bold);
    let full_name = format!(
        "{} {}",
        order.cliente_nombre.as_deref().unwrap_or_default(),
        order.cliente_apellido.as_deref().unwrap_or_default()
    );
    write_text(
        layer,
        format!("NOMBRES Y APELLIDOS: {full_name}"),
        7.5,
        15.0,
        234.0,
        normal,
    );
    write_text(
        layer,
        format!(
            "IDENTIFICACIÓN: {}",
            order.cliente_cedula.as_deref().unwrap_or_default()
        ),
        7.5,
        132.0,
        234.0,
        normal,
    );
    write_text(
        layer,
        format!("FECHA DE EMISIÓN: {}", paid_at.format("%Y-%m-%d")),
        7.5,
        15.0,
        226.0,
        normal,
    );
    write_text(
        layer,
        format!("HORA EXACTA (UTC): {}", paid_at.format("%H:%M:%S%.3f")),
        7.5,
        75.0,
        226.0,
        normal,
    );
    write_text(
        layer,
        format!("PEDIDO: {}", order.pedido_id),
        7.5,
        142.0,
        226.0,
        normal,
    );

    write_text(layer, "DETALLE DE PRODUCTOS", 9.0, 12.0, 214.0, bold);
    draw_rect(layer, 12.0, 200.0, 198.0, 210.0);
    for x in [28.0, 125.0, 160.0] {
        draw_line(layer, x, 200.0, x, 210.0);
    }
    write_text(layer, "CANT.", 7.0, 15.0, 204.0, bold);
    write_text(layer, "DESCRIPCIÓN", 7.0, 31.0, 204.0, bold);
    write_text(layer, "PRECIO UNITARIO", 6.5, 128.0, 204.0, bold);
    write_text(layer, "SUBTOTAL", 7.0, 165.0, 204.0, bold);
}

fn render_totals_and_payments(
    layer: &PdfLayerReference,
    normal: &IndirectFontRef,
    bold: &IndirectFontRef,
    order: &ReceiptOrder,
    top: f32,
) {
    let payment_height = 12.0 + order.pagos.len() as f32 * 6.0;
    let bottom = top - payment_height.max(30.0);
    draw_rect(layer, 12.0, bottom, 120.0, top);
    draw_line(layer, 12.0, top - 10.0, 120.0, top - 10.0);
    draw_line(layer, 90.0, bottom, 90.0, top);
    write_text(layer, "FORMA DE PAGO", 8.0, 15.0, top - 6.5, bold);
    write_text(layer, "VALOR", 8.0, 94.0, top - 6.5, bold);
    let mut payment_y = top - 16.0;
    for payment in &order.pagos {
        write_text(
            layer,
            payment.metodo_pago.replace('_', " "),
            7.5,
            15.0,
            payment_y,
            normal,
        );
        write_text(
            layer,
            format!("${:.2}", payment.monto),
            7.5,
            94.0,
            payment_y,
            normal,
        );
        payment_y -= 6.0;
    }

    draw_rect(layer, 124.0, top - 30.0, 198.0, top);
    draw_line(layer, 124.0, top - 10.0, 198.0, top - 10.0);
    draw_line(layer, 124.0, top - 20.0, 198.0, top - 20.0);
    draw_line(layer, 175.0, top - 30.0, 175.0, top);
    write_text(layer, "SUBTOTAL", 8.0, 128.0, top - 6.5, normal);
    write_text(layer, "IVA", 8.0, 128.0, top - 16.5, normal);
    write_text(layer, "TOTAL A PAGAR", 8.0, 128.0, top - 26.5, bold);
    write_text(
        layer,
        format!("${:.2}", order.subtotal),
        8.0,
        179.0,
        top - 6.5,
        normal,
    );
    write_text(
        layer,
        format!("${:.2}", order.iva),
        8.0,
        179.0,
        top - 16.5,
        normal,
    );
    write_text(
        layer,
        format!("${:.2}", order.total),
        8.0,
        179.0,
        top - 26.5,
        bold,
    );
}

pub fn generate_pdf(
    order: &ReceiptOrder,
    business: &BusinessInfo,
    receipt_number: i64,
) -> Result<Vec<u8>, String> {
    validate_receipt(order, business).map_err(|error| error.to_string())?;
    let (document, page, layer) = PdfDocument::new(
        format!("Factura {}", format_receipt_number(receipt_number)),
        Mm(210.0),
        Mm(297.0),
        "Factura",
    );
    let normal = document
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| e.to_string())?;
    let bold = document
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| e.to_string())?;
    let mut current_layer = document.get_page(page).get_layer(layer);
    render_header(
        &current_layer,
        &normal,
        &bold,
        order,
        business,
        receipt_number,
    );
    let mut y = 200.0;

    for item in &order.items {
        let description = wrapped_lines("", &item.nombre, 52);
        let row_height = (description.len() as f32 * 4.2 + 3.5).max(8.0);
        if y - row_height < 80.0 {
            let (next_page, next_layer) = document.add_page(Mm(210.0), Mm(297.0), "Factura");
            current_layer = document.get_page(next_page).get_layer(next_layer);
            render_header(
                &current_layer,
                &normal,
                &bold,
                order,
                business,
                receipt_number,
            );
            y = 200.0;
        }

        draw_rect(&current_layer, 12.0, y - row_height, 198.0, y);
        for x in [28.0, 125.0, 160.0] {
            draw_line(&current_layer, x, y - row_height, x, y);
        }
        write_text(
            &current_layer,
            item.cantidad.to_string(),
            7.5,
            16.0,
            y - 5.5,
            &normal,
        );
        let mut description_y = y - 5.5;
        for line in description {
            write_text(&current_layer, line, 7.5, 31.0, description_y, &normal);
            description_y -= 4.2;
        }
        write_text(
            &current_layer,
            format!("${:.2}", item.precio_unitario),
            7.5,
            130.0,
            y - 5.5,
            &normal,
        );
        write_text(
            &current_layer,
            format!("${:.2}", item.subtotal),
            7.5,
            166.0,
            y - 5.5,
            &normal,
        );
        y -= row_height;
    }

    render_totals_and_payments(&current_layer, &normal, &bold, order, y - 5.0);

    let mut bytes = Cursor::new(Vec::new());
    document
        .save(&mut BufWriter::new(&mut bytes))
        .map_err(|error| format!("No se pudo generar el PDF: {error}"))?;
    Ok(bytes.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::billing::{ReceiptItem, ReceiptPayment};
    use chrono::{TimeZone, Utc};
    use lopdf::Document;

    fn paid_order() -> ReceiptOrder {
        ReceiptOrder {
            pedido_id: "PED-000123".into(),
            estado: "PAGADO".into(),
            cliente_nombre: Some("Ana".into()),
            cliente_apellido: Some("Pérez".into()),
            cliente_cedula: Some("1712345678".into()),
            fecha_pago: Some(Utc.with_ymd_and_hms(2026, 7, 15, 14, 5, 6).unwrap()),
            subtotal: 10.00,
            iva: 1.50,
            total: 11.50,
            items: vec![ReceiptItem {
                nombre: "Café americano".into(),
                cantidad: 2,
                precio_unitario: 5.00,
                subtotal: 10.00,
            }],
            pagos: vec![ReceiptPayment {
                metodo_pago: "EFECTIVO".into(),
                monto: 11.50,
            }],
        }
    }

    fn business() -> BusinessInfo {
        BusinessInfo::new(
            "Cafetería STGC".into(),
            "1790012345001".into(),
            "Av. Principal 123".into(),
        )
        .unwrap()
    }

    #[test]
    fn ca6_blocks_every_non_paid_state_with_exact_message() {
        for state in ["EN_EDICION", "PENDIENTE_PAGO", "ANULADO", ""] {
            let mut order = paid_order();
            order.estado = state.into();
            let error = validate_receipt(&order, &business()).unwrap_err();
            assert_eq!(error, ReceiptValidationError::UnpaidOrder);
            assert_eq!(error.to_string(), UNPAID_ORDER_MESSAGE);
        }
    }

    #[test]
    fn ca1_rejects_missing_mandatory_business_customer_or_transaction_data() {
        let mut order = paid_order();
        order.cliente_apellido = Some(" ".into());
        order.fecha_pago = None;
        let error = validate_receipt(&order, &business())
            .unwrap_err()
            .to_string();
        assert!(error.contains("apellido del cliente"));
        assert!(error.contains("fecha y hora de pago"));

        assert!(BusinessInfo::new("".into(), "1790012345001".into(), "Dirección".into()).is_err());
    }

    #[test]
    fn ca2_rejects_empty_or_invalid_product_and_payment_details() {
        let mut order = paid_order();
        order.items.clear();
        order.pagos.clear();
        let error = validate_receipt(&order, &business())
            .unwrap_err()
            .to_string();
        assert!(error.contains("productos"));
        assert!(error.contains("métodos de pago"));
    }

    #[test]
    fn ca2_rejects_inconsistent_order_and_payment_totals() {
        let mut order = paid_order();
        order.total = 99.0;
        let error = validate_receipt(&order, &business())
            .unwrap_err()
            .to_string();
        assert!(error.contains("consistencia de subtotal, IVA y total"));
        assert!(error.contains("consistencia entre pagos y total"));
    }

    #[test]
    fn ca1_long_unbroken_values_are_wrapped_without_truncation() {
        let value = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789ABCDEFGHIJKLMN";
        let lines = wrapped_lines("", value, 20);
        assert!(lines.len() >= 3);
        assert_eq!(lines.concat(), value);
        assert!(lines.iter().all(|line| line.chars().count() <= 20));
    }

    #[test]
    fn ca4_formats_sequential_number_without_losing_uniqueness() {
        assert_eq!(format_receipt_number(1), "COMP-00000001");
        assert_eq!(format_receipt_number(2), "COMP-00000002");
        assert_ne!(format_receipt_number(999), format_receipt_number(1000));
    }

    #[test]
    fn ca4_migration_enforces_database_sequence_and_unique_constraints() {
        let migration = include_str!("../../migrations/20260715000000_hu12a_comprobantes.sql");
        assert!(migration.contains("DEFAULT nextval('billing_service.comprobante_numero_seq')"));
        assert!(migration.contains("UNIQUE (pedido_id)"));
        assert!(migration.contains("UNIQUE (numero)"));
        assert!(migration.contains("CHECK (numero > 0)"));
    }

    #[test]
    fn ca1_ca2_ca5_pdf_contains_all_required_sections_and_is_parseable() {
        let bytes = generate_pdf(&paid_order(), &business(), 42).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));

        let pdf = Document::load_mem(&bytes).unwrap();
        let pages: Vec<u32> = pdf.get_pages().keys().copied().collect();
        let text = pdf.extract_text(&pages).unwrap();
        for expected in [
            "Cafeter",
            "1790012345001",
            "Av. Principal 123",
            "PED-000123",
            "Ana",
            "rez",
            "1712345678",
            "PRODUCTOS",
            "Caf",
            "2",
            "5.00",
            "10.00",
            "IVA",
            "1.50",
            "11.50",
            "EFECTIVO",
            "COMP-00000042",
        ] {
            assert!(
                text.contains(expected),
                "PDF no contiene {expected:?}. Texto: {text}"
            );
        }

        for excluded in [
            "CLAVE DE ACCESO",
            "NÚMERO DE AUTORIZACIÓN",
            "AMBIENTE",
            "OBLIGADO A LLEVAR CONTABILIDAD",
            "SUBSIDIO",
            "DESCUENTO",
        ] {
            assert!(
                !text.contains(excluded),
                "El PDF agregó un dato no solicitado por la HU: {excluded}"
            );
        }
    }

    #[test]
    fn ca2_pdf_supports_multiple_payment_methods() {
        let mut order = paid_order();
        order.pagos = vec![
            ReceiptPayment {
                metodo_pago: "EFECTIVO".into(),
                monto: 5.50,
            },
            ReceiptPayment {
                metodo_pago: "TARJETA_DEBITO".into(),
                monto: 6.00,
            },
        ];
        let bytes = generate_pdf(&order, &business(), 43).unwrap();
        let pdf = Document::load_mem(&bytes).unwrap();
        let pages: Vec<u32> = pdf.get_pages().keys().copied().collect();
        let text = pdf.extract_text(&pages).unwrap();
        assert!(text.contains("EFECTIVO"));
        assert!(text.contains("TARJETA DEBITO"));
        assert!(text.contains("5.50"));
        assert!(text.contains("6.00"));
    }
}
