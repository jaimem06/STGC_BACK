use serde::Serialize;
use std::env;

#[derive(Serialize)]
struct AuditPayload {
    user_id: String,
    action: String,
    endpoint: String,
    ip: String,
}

pub fn enviar_auditoria(user_id: String, action: String, endpoint: String, ip: String) {
    let internal_api_key = env::var("INTERNAL_API_KEY").unwrap_or_default();
    let auth_service_url = env::var("AUTH_SERVICE_URL").unwrap_or_default();
    let audit_endpoint = format!("{}internal/audit", auth_service_url);

    let payload = AuditPayload {
        user_id,
        action,
        endpoint,
        ip,
    };

    tokio::spawn(async move {
        // Respaldo duro: si el servicio de auditoría no responde, dejamos constancia
        // del payload completo en los logs de STDOUT para no perder la traza.
        let fallback = serde_json::to_string(&payload).unwrap_or_default();

        let client = reqwest::Client::new();
        let res = client
            .post(&audit_endpoint)
            .header("X-Internal-Api-Key", internal_api_key)
            .json(&payload)
            .send()
            .await;

        match res {
            Ok(response) => {
                if !response.status().is_success() {
                    tracing::error!(
                        "Error enviando auditoría: Status {}. Payload: {}",
                        response.status(),
                        fallback
                    );
                }
            }
            Err(e) => {
                tracing::error!("Error enviando auditoría: {}. Payload: {}", e, fallback);
            }
        }
    });
}
