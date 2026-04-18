//! Directive Memory HTTP API.

pub mod auth;
pub mod error;
pub mod routes;
pub mod state;

use axum::http::StatusCode;
use axum::{middleware, routing::{get, post}, Router};
use dm_core::Core;
use state::AppState;

pub fn build_router(core: Core) -> Router {
    let state = AppState { core };
    // The fallback ensures auth middleware fires for unknown /api/* paths too —
    // without it, an unknown path would 404 before auth runs and leak which
    // routes exist to unauthenticated callers.
    let api = Router::new()
        .route("/search", get(routes::search::handler))
        .route("/files", get(routes::files::list))
        .route("/files/*path",
            get(routes::files::read)
                .post(routes::files::write)
                .patch(routes::files::append))
        .route("/facts", post(routes::facts::add))
        .route("/stats", get(routes::stats::stats))
        .route("/reindex", post(routes::stats::reindex))
        .fallback(|| async { StatusCode::NOT_FOUND })
        .layer(middleware::from_fn_with_state(state.clone(), auth::require_api_key))
        .with_state(state);

    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .nest("/api", api)
        .merge(routes::static_ui::router())
}
