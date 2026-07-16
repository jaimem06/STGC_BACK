// src/services/export_service.rs
use csv::WriterBuilder;
use serde_json::Value;
use tracing::info;

/// Generar HTML para reporte (para exportar a PDF desde el frontend)
pub fn generate_html_report(
    data: &[Value],
    headers: &[String],
    title: &str,
) -> Result<String, String> {
    info!("Generando HTML para reporte: {} registros", data.len());

    if data.is_empty() {
        return Ok("<p>No hay datos disponibles</p>".to_string());
    }

    let mut html = String::new();

    // Documento HTML profesional (sin emojis) con identidad visual STGC.
    html.push_str(r#"<!DOCTYPE html>
<html lang="es">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>"#);
    html.push_str(title);
    html.push_str(r#"</title>
    <style>
        :root {
            --brand: #442a22;
            --brand-soft: #5d4037;
            --accent: #b17036;
            --line: #e7ded4;
            --ink: #1f1b14;
            --muted: #6f625c;
            --bg: #f7f3ee;
        }
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;
            background: var(--bg);
            color: var(--ink);
            padding: 32px 20px;
            -webkit-print-color-adjust: exact;
            print-color-adjust: exact;
        }
        .sheet {
            max-width: 1100px;
            margin: 0 auto;
            background: #ffffff;
            border: 1px solid var(--line);
            border-radius: 14px;
            overflow: hidden;
            box-shadow: 0 10px 40px rgba(31, 27, 20, 0.08);
        }
        .head {
            padding: 28px 36px;
            border-bottom: 3px solid var(--brand);
            display: flex;
            justify-content: space-between;
            align-items: flex-end;
            gap: 24px;
        }
        .brand {
            font-size: 11px;
            letter-spacing: 0.18em;
            text-transform: uppercase;
            color: var(--accent);
            font-weight: 700;
        }
        h1 {
            font-size: 22px;
            color: var(--brand);
            margin-top: 6px;
            font-weight: 800;
            letter-spacing: -0.01em;
        }
        .btn {
            border: 0;
            background: var(--brand);
            color: #ffffff;
            padding: 10px 18px;
            border-radius: 8px;
            font-size: 13px;
            font-weight: 600;
            cursor: pointer;
            white-space: nowrap;
            transition: background 0.15s ease;
        }
        .btn:hover { background: var(--brand-soft); }
        .meta {
            display: flex;
            flex-wrap: wrap;
            gap: 28px;
            padding: 16px 36px;
            background: #faf7f3;
            border-bottom: 1px solid var(--line);
            font-size: 12px;
            color: var(--muted);
        }
        .meta b { color: var(--ink); font-weight: 600; }
        .table-wrap { padding: 12px 24px 24px; overflow-x: auto; }
        table { width: 100%; border-collapse: collapse; font-size: 13px; }
        thead th {
            background: var(--brand);
            color: #ffffff;
            text-align: left;
            padding: 11px 14px;
            font-weight: 600;
            font-size: 11px;
            letter-spacing: 0.04em;
            text-transform: uppercase;
            white-space: nowrap;
        }
        thead th:first-child { border-top-left-radius: 8px; }
        thead th:last-child { border-top-right-radius: 8px; }
        tbody td { padding: 10px 14px; border-bottom: 1px solid var(--line); color: var(--ink); }
        tbody tr:nth-child(even) { background: #faf7f3; }
        tbody tr:last-child td { border-bottom: 0; }
        .foot {
            padding: 18px 36px;
            border-top: 1px solid var(--line);
            font-size: 11px;
            color: var(--muted);
            display: flex;
            justify-content: space-between;
            flex-wrap: wrap;
            gap: 8px;
        }
        @media print {
            body { background: #ffffff; padding: 0; }
            .sheet { box-shadow: none; border: 0; border-radius: 0; max-width: none; }
            .no-print { display: none !important; }
            thead th { background: var(--brand) !important; color: #ffffff !important; }
            tbody tr:nth-child(even) { background: #f4f0ea !important; }
        }
    </style>
</head>
<body>
    <div class="sheet">
        <div class="head">
            <div>
                <div class="brand">STGC &middot; Tierra Fertil</div>
                <h1>"#);
    html.push_str(title);
    html.push_str(r#"</h1>
            </div>
            <button class="btn no-print" onclick="window.print()">Imprimir / Guardar PDF</button>
        </div>
        <div class="meta">
            <span>Generado: <b>"#);
    html.push_str(&chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
    html.push_str(r#"</b></span>
            <span>Registros: <b>"#);
    html.push_str(&data.len().to_string());
    html.push_str(r#"</b></span>
        </div>
        <div class="table-wrap">
            <table>
                <thead>
                    <tr>"#);

    // Encabezados
    for header in headers {
        html.push_str(&format!("<th>{}</th>", header));
    }
    html.push_str(r#"</tr>
                </thead>
                <tbody>"#);

    // Datos - Todos los registros (sin límite)
    for record in data {
        html.push_str("<tr>");
        if let Some(obj) = record.as_object() {
            for header in headers {
                let value = obj.iter()
                    .find(|(key, _)| key.to_lowercase() == header.to_lowercase())
                    .map(|(_, v)| format_value(v))
                    .unwrap_or_else(|| "".to_string());
                html.push_str(&format!("<td>{}</td>", value));
            }
        }
        html.push_str("</tr>");
    }

    html.push_str(r#"</tbody>
            </table>
        </div>
        <div class="foot">
            <span>STGC Report Service &middot; Sistema de Trazabilidad y Gesti&oacute;n Cafetera</span>
            <span>"#);
    html.push_str(&chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
    html.push_str(r#"</span>
        </div>
    </div>
</body>
</html>"#);

    Ok(html)
}

/// Exportar datos a CSV
pub fn export_csv_from_data(
    data: &[Value],
    headers: &[String],
    title: &str,
) -> Result<Vec<u8>, String> {
    info!("Generando CSV desde datos del frontend: {} registros", data.len());

    if data.is_empty() {
        return Err("No hay datos para exportar".to_string());
    }

    // Delimitador `;`: es el separador de listas que Excel espera en locales
    // en español (es-EC), de modo que el archivo abre en columnas al hacer
    // doble clic. `flexible` permite filas de metadatos con menos columnas.
    let mut wtr = WriterBuilder::new()
        .delimiter(b';')
        .terminator(csv::Terminator::CRLF)
        .quote_style(csv::QuoteStyle::Necessary)
        .flexible(true)
        .from_writer(vec![]);

    // Bloque de metadatos, cada dato en su propia fila (legible en la hoja).
    wtr.write_record([title]).map_err(|e| e.to_string())?;
    wtr.write_record([
        "Generado",
        &chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    ])
    .map_err(|e| e.to_string())?;
    wtr.write_record(["Registros", &data.len().to_string()])
        .map_err(|e| e.to_string())?;
    // Fila en blanco para separar los metadatos de la tabla.
    wtr.write_record(Vec::<&str>::new()).map_err(|e| e.to_string())?;

    // Encabezados de la tabla
    wtr.write_record(headers).map_err(|e| e.to_string())?;

    // Filas de datos
    for record in data {
        if let Some(obj) = record.as_object() {
            let row: Vec<String> = headers.iter()
                .map(|header| {
                    obj.iter()
                        .find(|(key, _)| key.to_lowercase() == header.to_lowercase())
                        .map(|(_, v)| format_value(v))
                        .unwrap_or_else(|| "".to_string())
                })
                .collect();
            wtr.write_record(&row).map_err(|e| e.to_string())?;
        }
    }

    let csv_data = wtr.into_inner().map_err(|e| e.to_string())?;

    // BOM UTF-8 para que Excel interprete correctamente los acentos (á, é, ñ...).
    let mut final_data = Vec::with_capacity(csv_data.len() + 3);
    final_data.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    final_data.extend_from_slice(&csv_data);

    info!("CSV generado exitosamente: {} bytes", final_data.len());
    Ok(final_data)
}

/// Exportar a PDF (ahora devuelve HTML con botón de impresión)
pub fn export_pdf_from_data(
    data: &[Value],
    headers: &[String],
    title: &str,
) -> Result<String, String> {
    info!("Generando HTML para PDF: {} registros", data.len());

    if data.is_empty() {
        return Err("No hay datos para exportar".to_string());
    }

    generate_html_report(data, headers, title)
}

/// Formatear valor para CSV/HTML
fn format_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => {
            if s.contains("T") && s.len() > 10 {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
                    return dt.format("%Y-%m-%d %H:%M").to_string();
                }
            }
            s.clone()
        },
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "".to_string(),
        serde_json::Value::Array(a) => format!("[{}]", a.len()),
        serde_json::Value::Object(o) => format!("{}", o.len()),
    }
}
