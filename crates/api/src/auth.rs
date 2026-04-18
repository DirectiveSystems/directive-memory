use axum::body::Body;
use axum::extract::State;
use axum::http::{header, Request, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use crate::state::AppState;

pub async fn require_api_key(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let expected = &state.core.config.api_key;
    // Guard-rail: refuse all /api/* requests if the operator forgot to set a key.
    if expected.is_empty() { return Err(StatusCode::UNAUTHORIZED); }

    let ok = req.headers().get("x-api-key")
        .and_then(|v| v.to_str().ok()).map(|v| v == expected).unwrap_or(false)
    || req.headers().get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|v| v == expected).unwrap_or(false);

    if !ok { return Err(StatusCode::UNAUTHORIZED); }
    Ok(next.run(req).await)
}
