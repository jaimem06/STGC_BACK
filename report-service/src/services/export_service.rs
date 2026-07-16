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
    info!("📄 Generando HTML para reporte: {} registros", data.len());

    if data.is_empty() {
        return Ok("<p>No hay datos disponibles</p>".to_string());
    }

    let mut html = String::new();
    
    // CSS completo para impresión
    html.push_str(r#"
    <!DOCTYPE html>
    <html>
    <head>
        <meta charset="UTF-8">
        <title>"#);
    html.push_str(title);
    html.push_str(r#"</title>
        <style>
            * { margin: 0; padding: 0; box-sizing: border-box; }
            body { 
                font-family: 'Arial', sans-serif; 
                padding: 20px; 
                background: #f8f9fa;
            }
            .container { 
                max-width: 1200px; 
                margin: 0 auto; 
                background: white; 
                padding: 30px; 
                border-radius: 8px;
                box-shadow: 0 2px 10px rgba(0,0,0,0.1);
            }
            h1 { 
                color: #2c3e50; 
                border-bottom: 3px solid #3498db; 
                padding-bottom: 15px;
                font-size: 24px;
                margin-bottom: 10px;
            }
            .subtitle { 
                color: #7f8c8d; 
                font-size: 14px; 
                margin-bottom: 20px; 
                display: flex;
                justify-content: space-between;
            }
            .table-container { 
                overflow-x: auto; 
                margin-top: 20px; 
            }
            table { 
                width: 100%; 
                border-collapse: collapse; 
                font-size: 13px; 
            }
            th { 
                background-color: #2c3e50; 
                color: white; 
                padding: 12px; 
                text-align: left; 
                border: 1px solid #ddd; 
                font-weight: bold;
                white-space: nowrap;
            }
            td { 
                padding: 10px; 
                border: 1px solid #ddd; 
                word-break: break-word;
            }
            tr:nth-child(even) { 
                background-color: #f8f9fa; 
            }
            tr:hover { 
                background-color: #e8f4fd; 
            }
            .footer { 
                margin-top: 30px; 
                font-size: 12px; 
                color: #7f8c8d; 
                text-align: center; 
                border-top: 1px solid #ddd; 
                padding-top: 15px; 
            }
            .badge {
                display: inline-block;
                padding: 3px 8px;
                border-radius: 4px;
                font-size: 11px;
                font-weight: bold;
            }
            .badge-success { background: #d4edda; color: #155724; }
            .badge-danger { background: #f8d7da; color: #721c24; }
            .badge-warning { background: #fff3cd; color: #856404; }

            /* Estilos para impresión */
            @media print {
                body { background: white; padding: 0; }
                .container { box-shadow: none; padding: 20px; }
                .no-print { display: none !important; }
                th { background-color: #2c3e50 !important; color: white !important; }
                tr:nth-child(even) { background-color: #f2f2f2 !important; }
            }
        </style>
    </head>
    <body>
        <div class="container">
            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px;">
                <h1>📊 "#);
    html.push_str(title);
    html.push_str(r#"</h1>
                <div class="no-print">
                    <button onclick="window.print()" style="
                        padding: 10px 20px; 
                        background: #3498db; 
                        color: white; 
                        border: none; 
                        border-radius: 4px; 
                        cursor: pointer;
                        font-size: 14px;
                    ">
                        🖨️ Imprimir / Guardar PDF
                    </button>
                </div>
            </div>
            <div class="subtitle">
                <span>📅 Fecha: "#);
    html.push_str(&chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
    html.push_str(r#"</span>
                <span>📈 Total registros: "#);
    html.push_str(&data.len().to_string());
    html.push_str(r#"</span>
            </div>
            <div class="table-container">
                <table>
                    <thead>
                        <tr>
    "#);

    // Encabezados
    for header in headers {
        html.push_str(&format!("<th>{}</th>", header));
    }
    html.push_str(r#"
                        </tr>
                    </thead>
                    <tbody>
    "#);

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

    html.push_str(r#"
                    </tbody>
                </table>
            </div>
            <div class="footer">
                <p>STGC Report Service v1.0 | Ruta del Café de Loja</p>
                <p>Generado el "#);
    html.push_str(&chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
    html.push_str(r#"</p>
            </div>
        </div>
        <script>
            // Auto-print al cargar (opcional)
            // window.print();
        </script>
    </body>
    </html>
    "#);

    Ok(html)
}

/// Exportar datos a CSV
pub fn export_csv_from_data(
    data: &[Value],
    headers: &[String],
    title: &str,
) -> Result<Vec<u8>, String> {
    info!("📊 Generando CSV desde datos del frontend: {} registros", data.len());

    if data.is_empty() {
        return Err("No hay datos para exportar".to_string());
    }

    let mut wtr = WriterBuilder::new()
        .delimiter(b',')
        .terminator(csv::Terminator::CRLF)
        .quote_style(csv::QuoteStyle::Necessary)
        .from_writer(vec![]);

    // Título como comentario
    let title_line = format!(
        "# {}\n# Fecha: {}\n# Total registros: {}\n",
        title,
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        data.len()
    );

    // Escribir encabezados
    wtr.write_record(headers).map_err(|e| e.to_string())?;

    // Escribir datos
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
    let mut final_data = title_line.into_bytes();
    final_data.extend(csv_data);

    info!("✅ CSV generado exitosamente: {} bytes", final_data.len());
    Ok(final_data)
}

/// Exportar a PDF (ahora devuelve HTML con botón de impresión)
pub fn export_pdf_from_data(
    data: &[Value],
    headers: &[String],
    title: &str,
) -> Result<String, String> {
    info!("📄 Generando HTML para PDF: {} registros", data.len());

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