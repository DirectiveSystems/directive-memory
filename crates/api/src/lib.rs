//! Directive Memory HTTP API.

pub mod auth;
pub mod error;
pub mod routes;
pub mod state;

use axum::http::StatusCode;
use axum::{middleware, routing::get, Router};
use dm_core::Core;
use state::AppState;

pub fn build_router(core: Core) -> Router {
    let state = AppState { core };
    // Placeholder /api route so the middleware layer attaches correctly even
    // before real routes land in Task 12. It is replaced with the full suite there.
    // The fallback ensures auth middleware fires for unknown /api/* paths too —
    // without it, an unknown path would 404 before auth runs.
    let api = Router::new()
        .route("/_placeholder", get(|| async { "" }))
        .fallback(|| async { StatusCode::NOT_FOUND })
        .layer(middleware::from_fn_with_state(state.clone(), auth::require_api_key))
        .with_state(state.clone());

    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .nest("/api", api)
        .with_state(state)
}
