use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub role: String,
    pub exp: usize,
    pub session_token: String,
}

pub async fn auth_middleware(
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok());

    let auth_header = if let Some(header) = auth_header {
        header
    } else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    if !auth_header.starts_with("Bearer ") {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token = &auth_header[7..];

    let jwt_secret = env::var("JWT_SECRET").expect("JWT_SECRET must be set");

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|e| {
        tracing::error!("Error decodificando token: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    // Guardamos el user_id (sub) para los handlers y los Claims completos para RBAC.
    req.extensions_mut().insert(token_data.claims.sub.clone());
    req.extensions_mut().insert(token_data.claims.clone());

    Ok(next.run(req).await)
}

/// Extractor opcional para control de acceso por rol (RBAC).
///
/// Se provee disponible pero NO se cablea a los endpoints por defecto: los roles
/// del sistema aún no están mapeados y forzarlo bloquearía usuarios legítimos.
/// Para activarlo en un handler basta con añadir `RequireRole(vec!["ADMIN".into()])`
/// como parámetro y comparar contra `Claims.role`.
#[allow(dead_code)]
pub fn tiene_rol(claims: &Claims, roles_permitidos: &[&str]) -> bool {
    roles_permitidos.iter().any(|r| r.eq_ignore_ascii_case(&claims.role))
}
