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
    pub user_id: String,
    pub role: String,
    pub exp: usize,
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
    .map_err(|_| StatusCode::UNAUTHORIZED)?;

    req.extensions_mut().insert(token_data.claims.user_id);
    
    Ok(next.run(req).await)
}
