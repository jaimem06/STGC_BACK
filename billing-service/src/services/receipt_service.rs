use crate::models::billing::{BusinessInfo, ReceiptOrder};
use printpdf::path::PaintMode;
use printpdf::{
    BuiltinFont, Color, IndirectFontRef, Line, Mm, PdfDocument, PdfLayerReference, Point, Rect,
    Rgb,
};
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

/// Número de comprobante en formato SRI: establecimiento (3 dígitos) -
/// punto de emisión (3 dígitos) - secuencial (9 dígitos). Ej.: 001-001-000000042.
pub fn format_receipt_number(business: &BusinessInfo, number: i64) -> String {
    format!(
        "{}-{}-{number:09}",
        business.establecimiento, business.punto_emision
    )
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

// Paleta institucional STGC (los mismos tonos del frontend, src/app/globals.css):
// café primario, verde secundario, terracota terciario y cremas de superficie.
type PaletteColor = (f32, f32, f32);
const CAFE_OSCURO: PaletteColor = (0.267, 0.165, 0.133); // #442a22
const VERDE: PaletteColor = (0.227, 0.408, 0.263); // #3a6843
const TERRACOTA: PaletteColor = (0.694, 0.439, 0.212); // #b17036
const CREMA: PaletteColor = (0.965, 0.925, 0.882); // #f6ece1
const CREMA_CLARO: PaletteColor = (0.988, 0.949, 0.906); // #fcf2e7
const TINTA: PaletteColor = (0.122, 0.106, 0.078); // #1f1b14
const LINEA: PaletteColor = (0.831, 0.765, 0.745); // #d4c3be
const BLANCO: PaletteColor = (1.0, 1.0, 1.0);

fn color(c: PaletteColor) -> Color {
    Color::Rgb(Rgb::new(c.0, c.1, c.2, None))
}

fn fill_rect(layer: &PdfLayerReference, left: f32, bottom: f32, right: f32, top: f32, c: PaletteColor) {
    layer.set_fill_color(color(c));
    layer.add_rect(Rect::new(Mm(left), Mm(bottom), Mm(right), Mm(top)).with_mode(PaintMode::Fill));
}

fn stroke_rect(layer: &PdfLayerReference, left: f32, bottom: f32, right: f32, top: f32, c: PaletteColor) {
    layer.set_outline_color(color(c));
    layer.add_rect(Rect::new(Mm(left), Mm(bottom), Mm(right), Mm(top)).with_mode(PaintMode::Stroke));
}

fn stroke_line(layer: &PdfLayerReference, x1: f32, y1: f32, x2: f32, y2: f32, c: PaletteColor) {
    layer.set_outline_color(color(c));
    layer.add_line(Line {
        points: vec![
            (Point::new(Mm(x1), Mm(y1)), false),
            (Point::new(Mm(x2), Mm(y2)), false),
        ],
        is_closed: false,
    });
}

// En PDF el color del texto es el "fill color": se fija SIEMPRE justo antes
// de escribir para que un relleno previo no tiña el texto.
fn write_text(
    layer: &PdfLayerReference,
    text: impl AsRef<str>,
    size: f32,
    x: f32,
    y: f32,
    font: &IndirectFontRef,
    c: PaletteColor,
) {
    layer.set_fill_color(color(c));
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

    // Banda superior con la identidad del negocio (café institucional a todo
    // el ancho). Los datos se limitan estrictamente a los de HU12-A.
    fill_rect(layer, 0.0, 262.0, 210.0, 297.0, CAFE_OSCURO);
    let mut business_y = 288.0;
    for line in wrapped_lines("", &business.nombre, 34) {
        write_text(layer, line, 15.0, 14.0, business_y, bold, BLANCO);
        business_y -= 6.5;
    }
    let mut address_y = 271.0;
    for line in wrapped_lines("DIRECCIÓN: ", &business.direccion, 58) {
        write_text(layer, line, 7.5, 14.0, address_y, normal, CREMA);
        address_y -= 4.0;
    }

    // Tarjeta de la factura, con el número en formato SRI destacado.
    fill_rect(layer, 126.0, 266.0, 198.0, 293.0, CREMA_CLARO);
    stroke_rect(layer, 126.0, 266.0, 198.0, 293.0, TERRACOTA);
    write_text(
        layer,
        format!("R.U.C.: {}", business.ruc),
        9.0,
        130.0,
        287.0,
        bold,
        TINTA,
    );
    write_text(layer, "FACTURA", 15.0, 130.0, 278.0, bold, CAFE_OSCURO);
    write_text(
        layer,
        format!("No. {}", format_receipt_number(business, receipt_number)),
        10.0,
        130.0,
        269.5,
        bold,
        TERRACOTA,
    );

    // Panel de datos del cliente sobre fondo crema.
    fill_rect(layer, 12.0, 224.0, 198.0, 250.0, CREMA);
    stroke_rect(layer, 12.0, 224.0, 198.0, 250.0, LINEA);
    write_text(layer, "DATOS DEL CLIENTE", 9.0, 15.0, 244.0, bold, VERDE);
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
        236.5,
        normal,
        TINTA,
    );
    write_text(
        layer,
        format!(
            "IDENTIFICACIÓN: {}",
            order.cliente_cedula.as_deref().unwrap_or_default()
        ),
        7.5,
        132.0,
        236.5,
        normal,
        TINTA,
    );
    write_text(
        layer,
        format!("FECHA DE EMISIÓN: {}", paid_at.format("%Y-%m-%d")),
        7.5,
        15.0,
        229.0,
        normal,
        TINTA,
    );
    write_text(
        layer,
        format!("HORA EXACTA (UTC): {}", paid_at.format("%H:%M:%S%.3f")),
        7.5,
        75.0,
        229.0,
        normal,
        TINTA,
    );
    // Fuente menor y arranque más a la izquierda: el UUID completo del pedido
    // debe caber dentro del panel (borde derecho en x=198).
    write_text(
        layer,
        format!("PEDIDO: {}", order.pedido_id),
        6.5,
        138.0,
        229.0,
        normal,
        TINTA,
    );

    // Cabecera de la tabla de productos: banda café con títulos en blanco.
    write_text(layer, "DETALLE DE PRODUCTOS", 9.0, 12.0, 215.0, bold, CAFE_OSCURO);
    fill_rect(layer, 12.0, 202.0, 198.0, 210.0, CAFE_OSCURO);
    write_text(layer, "CANT.", 7.0, 15.0, 204.5, bold, BLANCO);
    write_text(layer, "DESCRIPCIÓN", 7.0, 31.0, 204.5, bold, BLANCO);
    write_text(layer, "PRECIO UNITARIO", 6.5, 128.0, 204.5, bold, BLANCO);
    write_text(layer, "SUBTOTAL", 7.0, 165.0, 204.5, bold, BLANCO);
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

    // Bloque de formas de pago con cabecera crema.
    fill_rect(layer, 12.0, top - 10.0, 120.0, top, CREMA);
    stroke_rect(layer, 12.0, bottom, 120.0, top, LINEA);
    stroke_line(layer, 12.0, top - 10.0, 120.0, top - 10.0, LINEA);
    stroke_line(layer, 90.0, bottom, 90.0, top, LINEA);
    write_text(layer, "FORMA DE PAGO", 8.0, 15.0, top - 6.5, bold, CAFE_OSCURO);
    write_text(layer, "VALOR", 8.0, 94.0, top - 6.5, bold, CAFE_OSCURO);
    let mut payment_y = top - 16.0;
    for payment in &order.pagos {
        write_text(
            layer,
            payment.metodo_pago.replace('_', " "),
            7.5,
            15.0,
            payment_y,
            normal,
            TINTA,
        );
        write_text(
            layer,
            format!("${:.2}", payment.monto),
            7.5,
            94.0,
            payment_y,
            normal,
            TINTA,
        );
        payment_y -= 6.0;
    }

    // Bloque de totales; la fila TOTAL A PAGAR se destaca en verde.
    fill_rect(layer, 124.0, top - 30.0, 198.0, top - 20.0, VERDE);
    stroke_rect(layer, 124.0, top - 30.0, 198.0, top, LINEA);
    stroke_line(layer, 124.0, top - 10.0, 198.0, top - 10.0, LINEA);
    stroke_line(layer, 124.0, top - 20.0, 198.0, top - 20.0, LINEA);
    stroke_line(layer, 175.0, top - 30.0, 175.0, top, LINEA);
    write_text(layer, "SUBTOTAL", 8.0, 128.0, top - 6.5, normal, TINTA);
    write_text(layer, "IVA", 8.0, 128.0, top - 16.5, normal, TINTA);
    write_text(layer, "TOTAL A PAGAR", 8.0, 128.0, top - 26.5, bold, BLANCO);
    write_text(
        layer,
        format!("${:.2}", order.subtotal),
        8.0,
        179.0,
        top - 6.5,
        normal,
        TINTA,
    );
    write_text(
        layer,
        format!("${:.2}", order.iva),
        8.0,
        179.0,
        top - 16.5,
        normal,
        TINTA,
    );
    write_text(
        layer,
        format!("${:.2}", order.total),
        8.0,
        179.0,
        top - 26.5,
        bold,
        BLANCO,
    );

    // Pie institucional.
    stroke_line(layer, 12.0, 18.0, 198.0, 18.0, LINEA);
    write_text(
        layer,
        "Gracias por su compra - Documento generado electrónicamente por STGC",
        8.0,
        55.0,
        12.0,
        normal,
        TERRACOTA,
    );
}

pub fn generate_pdf(
    order: &ReceiptOrder,
    business: &BusinessInfo,
    receipt_number: i64,
) -> Result<Vec<u8>, String> {
    validate_receipt(order, business).map_err(|error| error.to_string())?;
    let (document, page, layer) = PdfDocument::new(
        format!("Factura {}", format_receipt_number(business, receipt_number)),
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
    let mut y = 202.0;

    for (index, item) in order.items.iter().enumerate() {
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
            y = 202.0;
        }

        // Filas cebra: el fondo crema alterno va antes que bordes y texto.
        if index % 2 == 0 {
            fill_rect(&current_layer, 12.0, y - row_height, 198.0, y, CREMA_CLARO);
        }
        stroke_rect(&current_layer, 12.0, y - row_height, 198.0, y, LINEA);
        for x in [28.0, 125.0, 160.0] {
            stroke_line(&current_layer, x, y - row_height, x, y, LINEA);
        }
        write_text(
            &current_layer,
            item.cantidad.to_string(),
            7.5,
            16.0,
            y - 5.5,
            &normal,
            TINTA,
        );
        let mut description_y = y - 5.5;
        for line in description {
            write_text(&current_layer, line, 7.5, 31.0, description_y, &normal, TINTA);
            description_y -= 4.2;
        }
        write_text(
            &current_layer,
            format!("${:.2}", item.precio_unitario),
            7.5,
            130.0,
            y - 5.5,
            &normal,
            TINTA,
        );
        write_text(
            &current_layer,
            format!("${:.2}", item.subtotal),
            7.5,
            166.0,
            y - 5.5,
            &normal,
            TINTA,
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
            "001".into(),
            "001".into(),
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

        assert!(BusinessInfo::new(
            "".into(),
            "1790012345001".into(),
            "Dirección".into(),
            "001".into(),
            "001".into()
        )
        .is_err());
    }

    #[test]
    fn ca4_rejects_invalid_sri_series() {
        // Las series SRI deben ser exactamente 3 dígitos.
        for (estab, punto) in [("1", "001"), ("001", "ABC"), ("0011", "001"), ("", "")] {
            assert!(BusinessInfo::new(
                "Cafetería STGC".into(),
                "1790012345001".into(),
                "Av. Principal 123".into(),
                estab.into(),
                punto.into()
            )
            .is_err());
        }
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
        let business = business();
        assert_eq!(format_receipt_number(&business, 1), "001-001-000000001");
        assert_eq!(format_receipt_number(&business, 2), "001-001-000000002");
        assert_ne!(
            format_receipt_number(&business, 999),
            format_receipt_number(&business, 1000)
        );
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
            "001-001-000000042",
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
